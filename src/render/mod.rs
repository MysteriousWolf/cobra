//! Frame encoding for each protocol, and the [`Renderer`] that drives it.

mod iterm2;
mod kitty;
mod text;

#[cfg_attr(not(feature = "ratatui"), allow(unused_imports))]
pub(crate) use text::Quantizer;

use std::io::{self, Write};
use std::sync::atomic::{AtomicU32, Ordering};

use crate::raster::Raster;
use crate::{Canvas, Protocol, Terminal};

/// Rendering options.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// Dot diameter as a fraction of its 2×4 slot, `0..=1`. Default `0.7`, close to
    /// what monospace fonts draw for braille; `COBRA_DOT` overrides it in
    /// [`Options::from_env`] so the image dots can be matched to the font's glyphs
    /// (`cargo run --example calibrate` shows them side by side).
    pub dot_size: f32,
    /// Print the braille text underneath the image so the canvas can still be copied
    /// as text. Only meaningful for image protocols. Default `false`.
    pub copy_text: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { dot_size: 0.7, copy_text: false }
    }
}

impl Options {
    /// Defaults, with `COBRA_DOT` (a fraction such as `0.8`) applied to `dot_size`.
    pub fn from_env() -> Self {
        let dot_size = std::env::var("COBRA_DOT").ok().and_then(|s| s.trim().parse::<f32>().ok());
        Self { dot_size: dot_size.map_or(0.7, |d| d.clamp(0.05, 1.0)), ..Self::default() }
    }
}

/// Where a frame goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    /// At the cursor; scrolls if needed and leaves the cursor on the line below.
    Flow,
    /// At an absolute 0-based `(col, row)`; cursor position is preserved.
    At(u16, u16),
    /// Kitty *virtual* placement: transmits the image for placeholder cells written
    /// elsewhere (see [`crate::ratatui`]). Draws nothing on other protocols.
    Virtual,
}

/// Turns canvases into frames for one terminal.
///
/// Owns every scratch buffer it needs, so after the first frame rendering does not
/// allocate. One renderer per canvas (or per widget) is the intended granularity: it
/// carries the kitty image id that identifies "this picture" across frames.
pub struct Renderer {
    term: Terminal,
    opts: Options,
    id: u32,
    raster: Raster,
    scratch: Vec<u8>,
    payload: Vec<u8>,
    out: Vec<u8>,
}

impl Renderer {
    /// Creates a renderer for `term` with [`Options::from_env`].
    pub fn new(term: Terminal) -> Self {
        Self::with_options(term, Options::from_env())
    }

    /// Creates a renderer with explicit options.
    pub fn with_options(term: Terminal, opts: Options) -> Self {
        static NEXT_ID: AtomicU32 = AtomicU32::new(0x00C0_B7A0);
        Self {
            term,
            opts,
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed) & 0x00FF_FFFF,
            raster: Raster::default(),
            scratch: Vec::new(),
            payload: Vec::new(),
            out: Vec::new(),
        }
    }

    /// The terminal this renderer targets.
    #[inline]
    pub fn terminal(&self) -> &Terminal {
        &self.term
    }

    /// Current options.
    #[inline]
    pub fn options(&self) -> Options {
        self.opts
    }

    /// Replaces the options.
    pub fn set_options(&mut self, opts: Options) {
        self.opts = opts;
    }

    /// Kitty image id used for this renderer's frames (24-bit).
    #[inline]
    pub fn image_id(&self) -> u32 {
        self.id
    }

    /// Renders `canvas` at the cursor and leaves the cursor at the start of the line
    /// below it.
    pub fn render(&mut self, canvas: &Canvas, w: &mut impl Write) -> io::Result<()> {
        let bytes = self.encode(canvas, Placement::Flow);
        w.write_all(bytes)
    }

    /// Renders `canvas` at an absolute cell position, preserving the cursor.
    pub fn render_at(&mut self, canvas: &Canvas, col: u16, row: u16, w: &mut impl Write) -> io::Result<()> {
        let bytes = self.encode(canvas, Placement::At(col, row));
        w.write_all(bytes)
    }

    /// Encodes a frame without writing it. The returned slice is valid until the next
    /// call and is exactly what [`render`](Self::render) would write.
    pub fn encode(&mut self, canvas: &Canvas, placement: Placement) -> &[u8] {
        self.encode_view(canvas, placement, canvas.cols(), canvas.rows())
    }

    /// Like [`encode`](Self::encode) but only the top-left `cols × rows` cells.
    pub fn encode_view(&mut self, canvas: &Canvas, placement: Placement, cols: u16, rows: u16) -> &[u8] {
        let (cols, rows) = (cols.min(canvas.cols()), rows.min(canvas.rows()));
        self.out.clear();
        if cols == 0 || rows == 0 {
            return &self.out;
        }
        let (depth, palette) = (self.term.depth, &self.term.palette);
        if self.term.protocol == Protocol::Text {
            match placement {
                Placement::Flow => text::frame(canvas, cols, rows, placement, depth, palette, &mut self.out),
                Placement::At(..) => {
                    self.out.extend_from_slice(b"\x1b7");
                    text::frame(canvas, cols, rows, placement, depth, palette, &mut self.out);
                    self.out.extend_from_slice(b"\x1b8");
                }
                Placement::Virtual => {}
            }
            return &self.out;
        }

        let (w, h) = self.raster.draw(canvas, self.term.cell, self.opts.dot_size, &self.term.palette, cols, rows);
        let rgba = &self.raster.rgba;
        self.payload.clear();
        // LZ77 distances in pixels: one pixel (runs), one cell (dither patterns),
        // one row and one dot row (vertical repetition).
        let (cw, ch) = (self.term.cell.width as usize, self.term.cell.height as usize);
        let dists = [1, cw, w as usize, w as usize * (ch / 4).max(1)];

        // Position and, for flow placement, make room so the image never overlaps
        // what was already on screen.
        match placement {
            Placement::Flow => {
                for _ in 0..rows {
                    self.out.extend_from_slice(b"\r\n");
                }
                self.out.extend_from_slice(format!("\x1b[{rows}A").as_bytes());
                if self.opts.copy_text {
                    text::frame(canvas, cols, rows, Placement::Flow, depth, palette, &mut self.out);
                    self.out.extend_from_slice(format!("\x1b[{rows}A").as_bytes());
                }
            }
            Placement::At(col, row) => {
                self.out.extend_from_slice(b"\x1b7");
                if self.opts.copy_text {
                    text::frame(canvas, cols, rows, placement, depth, palette, &mut self.out);
                }
                self.out.extend_from_slice(format!("\x1b[{};{}H", row + 1, col + 1).as_bytes());
            }
            Placement::Virtual => {}
        }
        if placement != Placement::Virtual {
            self.out.extend_from_slice(b"\x1b7");
        }

        match self.term.protocol {
            Protocol::Kitty => {
                let bytes = dists.map(|d| d * 4);
                crate::encode::deflate::zlib(rgba, &bytes, &mut self.payload);
                let virt = (placement == Placement::Virtual).then_some((cols, rows));
                kitty::frame(&self.payload, w, h, self.id, virt, &mut self.scratch, &mut self.out);
            }
            Protocol::Iterm2 => {
                crate::encode::png::encode(rgba, w, h, &dists, &mut self.scratch, &mut self.payload);
                iterm2::frame(&self.payload, w, h, &mut self.out);
            }
            Protocol::Sixel => {
                crate::encode::sixel::encode(rgba, w as usize, h as usize, &mut self.scratch, &mut self.out);
            }
            Protocol::Text => unreachable!(),
        }

        match placement {
            Placement::Flow => self.out.extend_from_slice(format!("\x1b8\r\x1b[{rows}B").as_bytes()),
            Placement::At(..) => self.out.extend_from_slice(b"\x1b8"),
            Placement::Virtual => {}
        }
        &self.out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CellSize, Rgb};

    fn canvas() -> Canvas {
        let mut c = Canvas::new(3, 2);
        c.set(0, 0, Rgb::hex(0xff0000));
        c.set(5, 7, Rgb::hex(0x00ff00));
        c
    }

    #[test]
    fn text_frames() {
        let mut r = Renderer::with_options(Terminal::text(), Options::default());
        let s = String::from_utf8(r.encode(&canvas(), Placement::Flow).to_vec()).unwrap();
        assert_eq!(s, "\x1b[38;2;255;0;0m⠁⠀⠀\x1b[0m\r\n⠀⠀\x1b[38;2;0;255;0m⢀\x1b[0m\r\n");
        let s = String::from_utf8(r.encode(&canvas(), Placement::At(4, 2)).to_vec()).unwrap();
        assert!(s.starts_with("\x1b7\x1b[3;5H") && s.ends_with("\x1b8"));
        assert!(r.encode(&canvas(), Placement::Virtual).is_empty());
    }

    #[test]
    fn text_frames_use_palette_sgr() {
        let mut c = Canvas::new(2, 1);
        c.set(0, 0, crate::Color::Indexed(12));
        c.set(2, 0, crate::Color::Foreground);
        let mut r = Renderer::new(Terminal::text());
        let s = String::from_utf8(r.encode(&c, Placement::Flow).to_vec()).unwrap();
        assert_eq!(s, "\x1b[94m⠁\x1b[39m⠁\x1b[0m\r\n");
    }

    #[test]
    fn text_frames_quantise_to_depth() {
        use crate::{Color, Depth};
        let mut c = Canvas::new(2, 1);
        // Two reds that both round to ANSI bright red outvote three dots of one blue.
        c.set(0, 0, Rgb::hex(0xff0000));
        c.set(1, 0, Rgb::hex(0xfe0000));
        c.set(0, 1, Rgb::hex(0xfd0000));
        c.set(1, 1, Rgb::hex(0xfc0000));
        for y in 0..3 {
            c.set(0, y + 1, Rgb::hex(0x0000ff));
        }
        c.set(2, 0, Color::Indexed(200));
        let frame = |d: Depth| {
            let mut r = Renderer::new(Terminal::text().with_depth(d));
            String::from_utf8(r.encode(&c, Placement::Flow).to_vec()).unwrap()
        };
        assert_eq!(frame(Depth::Ansi16), "\x1b[91m⡟\x1b[95m⠁\x1b[0m\r\n");
        assert_eq!(frame(Depth::Ansi256), "\x1b[38;5;196m⡟\x1b[38;5;200m⠁\x1b[0m\r\n");
        assert_eq!(frame(Depth::Mono), "\x1b[39m⡟⠁\x1b[0m\r\n");
        assert!(frame(Depth::TrueColor).starts_with("\x1b[38;2;0;0;255m⡟"));
    }

    #[test]
    fn image_frames_have_expected_envelopes() {
        let cell = CellSize { width: 8, height: 16 };
        let mut k = Renderer::new(Terminal::new(Protocol::Kitty, cell));
        let s = String::from_utf8(k.encode(&canvas(), Placement::Flow).to_vec()).unwrap();
        assert!(s.starts_with("\r\n\r\n\x1b[2A\x1b7\x1b_G"));
        assert!(s.contains("s=24,v=32") && s.contains("o=z") && s.ends_with("\x1b\\\x1b8\r\x1b[2B"));
        let v = String::from_utf8(k.encode(&canvas(), Placement::Virtual).to_vec()).unwrap();
        assert!(v.starts_with("\x1b_G") && v.contains("U=1,c=3,r=2") && v.ends_with("\x1b\\"));

        let mut i = Renderer::new(Terminal::new(Protocol::Iterm2, cell));
        let s = i.encode(&canvas(), Placement::At(0, 0));
        assert!(s.starts_with(b"\x1b7\x1b[1;1H\x1b7\x1b]1337;File=inline=1"));

        let mut x = Renderer::new(Terminal::new(Protocol::Sixel, cell));
        let s = x.encode_view(&canvas(), Placement::Flow, 1, 1);
        assert!(s.windows(9).any(|w| w == b"\x1bP0;1;0q\""));
        let s = String::from_utf8_lossy(s).into_owned();
        assert!(s.contains("\"1;1;8;16"));
    }

    #[test]
    fn render_does_not_allocate_after_warmup() {
        let mut r = Renderer::new(Terminal::new(Protocol::Kitty, CellSize { width: 9, height: 18 }));
        let c = canvas();
        r.encode(&c, Placement::Flow);
        let caps = (r.out.capacity(), r.payload.capacity(), r.scratch.capacity(), r.raster.rgba.capacity());
        r.encode(&c, Placement::Flow);
        assert_eq!(caps, (r.out.capacity(), r.payload.capacity(), r.scratch.capacity(), r.raster.rgba.capacity()));
    }
}
