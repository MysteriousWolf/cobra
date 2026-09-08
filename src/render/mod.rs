//! Frame encoding for each protocol, and the [`Renderer`] that drives it.

mod iterm2;
mod kitty;
mod text;

use text::History;
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
    /// The last text frame, for [`Placement::At`] to send only what changed.
    history: History,
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
            history: History::default(),
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

    /// Forgets the last frame drawn with [`render_at`](Self::render_at), so the next
    /// one is sent in full. Text-protocol frames at a fixed position send only the
    /// cells that changed since the previous frame at the same position; call this
    /// after the screen was cleared or drawn over by something else.
    pub fn invalidate(&mut self) {
        self.history.forget();
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
        let (mut cols, mut rows) = (cols.min(canvas.cols()), rows.min(canvas.rows()));
        self.out.clear();
        if self.term.passthrough && self.term.protocol != Protocol::Text {
            // Through tmux the image is drawn by a terminal that knows nothing of the
            // pane, so whatever leaves the pane hangs off the outer screen, where
            // partly visible images have crashed Ghostty (ghostty-org/ghostty#4266).
            // Keep the picture inside the pane.
            let (col, row) = match placement {
                Placement::At(col, row) => (col, row),
                Placement::Flow | Placement::Virtual => (0, 0),
            };
            if self.term.cols > 0 {
                cols = cols.min(self.term.cols.saturating_sub(col));
            }
            if self.term.rows > 0 {
                rows = rows.min(self.term.rows.saturating_sub(row));
            }
        }
        if cols == 0 || rows == 0 {
            return &self.out;
        }
        let (depth, palette) = (self.term.depth, &self.term.palette);
        if self.term.protocol == Protocol::Text {
            match placement {
                Placement::Flow => {
                    text::frame(canvas, cols, rows, placement, depth, palette, None, &mut self.out);
                }
                Placement::At(..) => {
                    self.out.extend_from_slice(b"\x1b7");
                    let history = Some(&mut self.history);
                    text::frame(canvas, cols, rows, placement, depth, palette, history, &mut self.out);
                    self.out.extend_from_slice(b"\x1b8");
                }
                Placement::Virtual => {}
            }
            return &self.out;
        }

        let (w, h) = self.raster.draw(canvas, self.term.cell, self.opts.dot_size, &self.term.palette, cols, rows, None);
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
                text::step(rows, b'A', &mut self.out);
                if self.opts.copy_text {
                    text::frame(canvas, cols, rows, Placement::Flow, depth, palette, None, &mut self.out);
                    text::step(rows, b'A', &mut self.out);
                }
            }
            Placement::At(col, row) => {
                // The one save of the cursor: DECSC has a single slot, so saving again
                // for the image would lose the caller's position.
                self.out.extend_from_slice(b"\x1b7");
                if self.opts.copy_text {
                    text::frame(canvas, cols, rows, placement, depth, palette, None, &mut self.out);
                }
                text::cursor_to(row + 1, col + 1, &mut self.out);
            }
            Placement::Virtual => {}
        }
        if placement == Placement::Flow {
            self.out.extend_from_slice(b"\x1b7");
        }
        if self.term.passthrough && placement != Placement::Virtual {
            // tmux hands a passthrough to its terminal as-is, at wherever that
            // terminal's cursor happens to be: cursor moves in the pane are tracked
            // lazily and only realised when a cell is drawn. After the scroll above
            // that is the bottom of the pane, so the image would start there and hang
            // off the screen. Erasing one cell (ECH) makes tmux position the outer
            // cursor at the pane's before drawing; the cell is the image's origin,
            // which the image covers.
            self.out.extend_from_slice(b"\x1b[1X");
        }

        let image_from = self.out.len();
        match self.term.protocol {
            Protocol::Kitty => {
                let bytes = dists.map(|d| d * 4);
                crate::encode::deflate::zlib(rgba, &bytes, &mut self.payload);
                let virt = (placement == Placement::Virtual).then_some((cols, rows));
                // Text cells are printed after the image, so on kitty the image has to
                // sit below the text layer for them to show.
                let z = if canvas.has_text() { -1 } else { 0 };
                kitty::frame(&self.payload, w, h, self.id, virt, z, &mut self.scratch, &mut self.out);
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
        if self.term.passthrough {
            passthrough(&mut self.out, image_from, &mut self.scratch);
        }

        if canvas.has_text() && placement != Placement::Virtual {
            text::overlay(canvas, cols, rows, placement, depth, palette, &mut self.out);
        }

        match placement {
            Placement::Flow => {
                self.out.extend_from_slice(b"\x1b8\r");
                text::step(rows, b'B', &mut self.out);
            }
            Placement::At(..) => self.out.extend_from_slice(b"\x1b8"),
            Placement::Virtual => {}
        }
        &self.out
    }
}

/// Rewraps every escape sequence in `out[from..]` (an APC, DCS or OSC, as the image
/// protocols emit) in tmux's passthrough: `ESC P tmux ;` then the sequence with each
/// `ESC` doubled, then `ESC \`. tmux unwraps it and hands it to its own terminal
/// untouched. One wrapper per sequence, so a kitty frame's chunks stay separate.
fn passthrough(out: &mut Vec<u8>, from: usize, scratch: &mut Vec<u8>) {
    scratch.clear();
    scratch.extend_from_slice(&out[from..]);
    out.truncate(from);
    let mut rest: &[u8] = scratch;
    while let Some(start) = rest.iter().position(|&b| b == 0x1b) {
        let seq = &rest[start..];
        // An OSC may end in BEL; everything else in ST. Both may end in ST.
        let osc = seq.get(1) == Some(&b']');
        let end = seq
            .iter()
            .enumerate()
            .skip(2)
            .find(|&(i, &b)| (osc && b == 0x07) || (b == b'\\' && seq[i - 1] == 0x1b))
            .map_or(seq.len(), |(i, _)| i + 1);
        out.extend_from_slice(b"\x1bPtmux;");
        for &b in &seq[..end] {
            if b == 0x1b {
                out.push(0x1b);
            }
            out.push(b);
        }
        out.extend_from_slice(b"\x1b\\");
        rest = &seq[end..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CellSize, Palette, Rgb};

    #[test]
    fn passthrough_wraps_each_image_sequence_and_nothing_else() {
        let cell = CellSize { width: 8, height: 16 };
        let mut k = Renderer::new(Terminal::new(Protocol::Kitty, cell).with_passthrough(true));
        let mut c = canvas();
        c.print(0, 1, "ok", Rgb::hex(0xffffff));
        let s = String::from_utf8(k.encode(&c, Placement::At(2, 3)).to_vec()).unwrap();
        assert!(s.contains("\x1b[4;3H\x1b[1X\x1bPtmux;\x1b\x1b_G"), "cursor sync, then the wrapped APC: {s:?}");
        assert!(s.contains("\x1b\x1b\\\x1b\\"), "its ST is doubled and the wrapper closed");
        assert!(!s.contains("\x1bPtmux;\x1b\x1b["), "cursor moves are not wrapped");
        assert!(s.contains("\x1b[5;3H") && s.contains("ok"), "the text overlay is plain");
        let v = String::from_utf8(k.encode(&c, Placement::Virtual).to_vec()).unwrap();
        assert!(v.starts_with("\x1bPtmux;\x1b\x1b_G"), "a virtual placement needs no cursor: {v:?}");
        let mut i = Renderer::new(Terminal::new(Protocol::Iterm2, cell).with_passthrough(true));
        let s = String::from_utf8(i.encode(&canvas(), Placement::Flow).to_vec()).unwrap();
        assert!(
            s.contains("\x1b7\x1b[1X\x1bPtmux;\x1b\x1b]1337;") && s.contains("\x07\x1b\\"),
            "OSC ends in BEL: {s:?}"
        );
        let mut x = Renderer::new(Terminal::new(Protocol::Sixel, cell).with_passthrough(true));
        let s = String::from_utf8_lossy(x.encode(&canvas(), Placement::Flow)).into_owned();
        assert!(s.contains("\x1b[1X\x1bPtmux;\x1b\x1bP0;1;0q"), "{s:?}");
        let plain = Renderer::new(Terminal::new(Protocol::Kitty, cell)).encode(&canvas(), Placement::Flow).to_vec();
        assert!(!plain.windows(6).any(|w| w == b"\x1bPtmux"));
        assert!(!plain.windows(4).any(|w| w == b"\x1b[1X"), "no cursor sync outside tmux");
    }

    #[test]
    fn passthrough_keeps_the_image_inside_the_pane() {
        let cell = CellSize { width: 8, height: 16 };
        let pane = Terminal { cols: 2, rows: 1, ..Terminal::new(Protocol::Kitty, cell).with_passthrough(true) };
        let mut r = Renderer::new(pane);
        let s = String::from_utf8(r.encode(&canvas(), Placement::Flow).to_vec()).unwrap();
        assert!(s.starts_with("\r\n\x1b[1A"), "room for one row only: {s:?}");
        assert!(s.contains("s=16,v=16"), "2×1 cells of 8×16 px: {s:?}");
        let s = String::from_utf8(r.encode(&canvas(), Placement::At(1, 0)).to_vec()).unwrap();
        assert!(s.contains("s=8,v=16"), "one column left of the pane: {s:?}");
        assert!(r.encode(&canvas(), Placement::At(2, 0)).is_empty(), "nothing fits, nothing sent");
        // Without tmux the pane size does not clip: the terminal scrolls for itself.
        let mut plain = Renderer::new(Terminal { cols: 2, rows: 1, ..Terminal::new(Protocol::Kitty, cell) });
        let s = String::from_utf8(plain.encode(&canvas(), Placement::Flow).to_vec()).unwrap();
        assert!(s.contains("s=24,v=32"), "{s:?}");
    }

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
        assert_eq!(s, "\x1b[38;2;255;0;0m⠁\x1b[0m\x1b[K\r\n⠀⠀\x1b[38;2;0;255;0m⢀\x1b[0m\r\n");
        let s = String::from_utf8(r.encode(&canvas(), Placement::At(4, 2)).to_vec()).unwrap();
        assert!(s.starts_with("\x1b7\x1b[3;5H") && s.ends_with("\x1b8"));
        assert!(r.encode(&canvas(), Placement::Virtual).is_empty());
    }

    #[test]
    fn fixed_frames_send_only_what_changed() {
        let mut r = Renderer::with_options(Terminal::text(), Options::default());
        let mut c = canvas();
        let full = String::from_utf8(r.encode(&c, Placement::At(4, 2)).to_vec()).unwrap();
        assert!(full.contains("\x1b[3;5H") && full.contains("\x1b[4;5H"), "both rows: {full:?}");
        let same = String::from_utf8(r.encode(&c, Placement::At(4, 2)).to_vec()).unwrap();
        assert_eq!(same, "\x1b7\x1b8", "nothing changed, nothing sent");
        c.set(5, 0, Rgb::hex(0x0000ff));
        let diff = String::from_utf8(r.encode(&c, Placement::At(4, 2)).to_vec()).unwrap();
        assert_eq!(diff, "\x1b7\x1b[3;7H\x1b[38;2;0;0;255m⠈\x1b[0m\x1b8", "just the changed cell");
        let moved = String::from_utf8(r.encode(&c, Placement::At(0, 0)).to_vec()).unwrap();
        assert!(moved.contains("\x1b[1;1H") && moved.contains("\x1b[2;1H"), "another place is a full frame");
        r.invalidate();
        let again = String::from_utf8(r.encode(&c, Placement::At(0, 0)).to_vec()).unwrap();
        assert_eq!(again, moved);
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
        assert!(
            s.starts_with(b"\x1b7\x1b[1;1H\x1b]1337;File=inline=1"),
            "one cursor save, so `render_at` really restores it"
        );

        let mut x = Renderer::new(Terminal::new(Protocol::Sixel, cell));
        let s = x.encode_view(&canvas(), Placement::Flow, 1, 1);
        assert!(s.windows(9).any(|w| w == b"\x1bP0;1;0q\""));
        let s = String::from_utf8_lossy(s).into_owned();
        assert!(s.contains("\"1;1;8;16"));
    }

    #[test]
    fn text_frames_print_real_characters() {
        let mut c = Canvas::new(4, 1);
        c.set(0, 0, Rgb::hex(0xff0000));
        c.print(1, 0, "hi", crate::TextStyle::new(Rgb::hex(0x00ff00)).on(Rgb::hex(0x000010)).bold());
        let mut r = Renderer::new(Terminal::text());
        let s = String::from_utf8(r.encode(&c, Placement::Flow).to_vec()).unwrap();
        assert_eq!(s, "\x1b[38;2;255;0;0m⠁\x1b[0m\x1b[1m\x1b[38;2;0;255;0m\x1b[48;2;0;0;16mhi\x1b[0m⠀\x1b[0m\r\n");
        assert_eq!(c.to_text(), "⠁hi⠀\n");
    }

    #[test]
    fn image_frames_print_text_over_the_picture() {
        let mut c = Canvas::new(3, 2);
        c.print(1, 1, "ok", crate::Color::Foreground);
        let mut k = Renderer::new(Terminal::new(Protocol::Kitty, CellSize { width: 8, height: 16 }));
        let s = String::from_utf8(k.encode(&c, Placement::Flow).to_vec()).unwrap();
        assert!(s.contains("z=-1"), "the image goes below the text layer");
        // Back to the saved origin, down a row, right a column, then the characters.
        assert!(s.contains("\x1b8\r\x1b[1B\x1b[1C\x1b[39mok\x1b[0m"), "{s:?}");
        let mut i = Renderer::new(Terminal::new(Protocol::Iterm2, CellSize { width: 8, height: 16 }));
        let s = String::from_utf8(i.encode(&c, Placement::At(4, 2)).to_vec()).unwrap();
        assert!(s.contains("\x1b[4;6H\x1b[39mok\x1b[0m"), "{s:?}");
        // The cells the characters take are left transparent for them to show through.
        let mut raster = crate::raster::Raster::default();
        raster.draw(&c, CellSize { width: 8, height: 16 }, 1.0, &Palette::default(), 3, 2, None);
        assert!(raster.rgba.iter().all(|&b| b == 0));
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
