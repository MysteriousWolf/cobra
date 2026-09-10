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
    window: crate::encode::deflate::Window,
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
        // A renderer owns a block of ids, not one: an oversized frame is sent as
        // several images and each needs an id of its own that no other renderer will
        // reuse. The block is aligned, so a tile's id never leaves the 24 bits.
        static NEXT_ID: AtomicU32 = AtomicU32::new(0x00C0_B780);
        Self {
            term,
            opts,
            id: NEXT_ID.fetch_add(MAX_TILES as u32, Ordering::Relaxed) & 0x00FF_FFFF,
            raster: Raster::default(),
            scratch: Vec::new(),
            payload: Vec::new(),
            window: crate::encode::deflate::Window::default(),
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
        if self.term.protocol != Protocol::Text {
            // An image that reaches past the last row or column is drawn partly, and
            // partly visible images have crashed Ghostty (ghostty-org/ghostty#4266).
            // Through tmux there is a second reason: the image is drawn by a terminal
            // that knows nothing of the pane, so whatever leaves the pane lands on the
            // outer screen. Keep the picture inside what is known to be visible.
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
        let mut wrapped = false;
        match self.term.protocol {
            Protocol::Kitty => {
                let bytes = dists.map(|d| d * 4);
                crate::encode::deflate::zlib(&mut self.window, rgba, &bytes, &mut self.payload);
                let virt = placement == Placement::Virtual;
                // Text cells are printed after the image, so on kitty the image has to
                // sit below the text layer for them to show.
                let z = if canvas.has_text() { -1 } else { 0 };
                // A virtual placement is positioned by the placeholder cells printed
                // for it, not by the cursor, so it cannot be split this way: its tiles
                // would all land on the same cells.
                let cap = max_payload();
                let bands = if virt || cap == 0 || rows < 2 || self.payload.len() <= cap {
                    1
                } else {
                    self.payload.len().div_ceil(cap).min(rows as usize).min(MAX_TILES)
                };
                if bands == 1 {
                    kitty::frame(&self.payload, w, h, self.id, (cols, rows), virt, z, &mut self.scratch, &mut self.out);
                } else {
                    // Rows split as evenly as they divide, the remainder going to the
                    // first bands. Each band is whole cell rows, so every tile lands on
                    // a cell boundary and the placements tile the same area the one
                    // image would have covered.
                    let (base, rem) = (rows as usize / bands, rows as usize % bands);
                    let (mut row0, mut walked) = (0usize, 0u16);
                    for k in 0..bands {
                        let n = base + usize::from(k < rem);
                        if k > 0 {
                            // Between tiles the cursor walks down the pane, so these
                            // moves must stay outside tmux's passthrough: wrapped, they
                            // would move the outer terminal's cursor instead of the
                            // pane's, and every tile would land in the same place.
                            text::step(walked_step(base, rem, k), b'B', &mut self.out);
                            walked += walked_step(base, rem, k);
                            if self.term.passthrough {
                                self.out.extend_from_slice(b"\x1b[1X");
                            }
                        }
                        // The raster is row-major, so a band of rows is one slice.
                        let (y0, y1) = (row0 * h as usize / rows as usize, (row0 + n) * h as usize / rows as usize);
                        self.payload.clear();
                        let band = &rgba[y0 * w as usize * 4..y1 * w as usize * 4];
                        crate::encode::deflate::zlib(&mut self.window, band, &bytes, &mut self.payload);
                        let from = self.out.len();
                        let id = (self.id + k as u32) & 0x00FF_FFFF;
                        let cells = (cols, n as u16);
                        kitty::frame(
                            &self.payload,
                            w,
                            (y1 - y0) as u32,
                            id,
                            cells,
                            virt,
                            z,
                            &mut self.scratch,
                            &mut self.out,
                        );
                        if self.term.passthrough {
                            passthrough(&mut self.out, from, &mut self.scratch);
                        }
                        row0 += n;
                    }
                    // Every tile wrapped itself, and the moves between them were left
                    // out of the wrapping on purpose.
                    wrapped = self.term.passthrough;
                    // Back to where the first tile started, so what follows sees the
                    // cursor exactly where a single image would have left it. A zero
                    // step is not written: terminals read `CSI 0 A` as one row.
                    if walked > 0 {
                        text::step(walked, b'A', &mut self.out);
                    }
                }
            }
            Protocol::Iterm2 => {
                crate::encode::png::encode(&mut self.window, rgba, w, h, &dists, &mut self.scratch, &mut self.payload);
                iterm2::frame(&self.payload, cols, rows, &mut self.out);
            }
            Protocol::Sixel => {
                crate::encode::sixel::encode(rgba, w as usize, h as usize, &mut self.scratch, &mut self.out);
            }
            Protocol::Text => unreachable!(),
        }
        if self.term.passthrough && !wrapped {
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

/// Most images one oversized frame is split into. Also the stride between the image
/// ids of two renderers, so the tiles of one never collide with another's.
const MAX_TILES: usize = 64;

/// Largest compressed payload to put in a single kitty image, in bytes.
///
/// Ghostty 1.3.1 segfaults in its reader thread on a frame whose payload is tens of
/// kilobytes, while the same picture at a few kilobytes draws fine; the exact limit
/// is not known, so this sits nearer the size that is known to work than the one
/// that is known to crash. `COBRA_MAX_PAYLOAD` overrides it, and `0` turns the
/// splitting off.
const MAX_PAYLOAD: usize = 8 * 1024;

/// Rows in band `k - 1`, which is how far the cursor moves to reach band `k`.
fn walked_step(base: usize, rem: usize, k: usize) -> u16 {
    (base + usize::from(k - 1 < rem)) as u16
}

fn max_payload() -> usize {
    static CHOSEN: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *CHOSEN.get_or_init(|| {
        std::env::var("COBRA_MAX_PAYLOAD").ok().and_then(|s| s.trim().parse::<usize>().ok()).unwrap_or(MAX_PAYLOAD)
    })
}

/// Rewraps every escape sequence in `out[from..]` (an APC, DCS or OSC, as the image
/// protocols emit) in tmux's passthrough: `ESC P tmux ;` then the sequence with each
/// `ESC` doubled, then `ESC \`. tmux unwraps it and hands it to its own terminal
/// untouched. One wrapper per sequence, so a kitty frame's chunks stay separate.
///
/// Detection wraps its queries with it too, so that the terminal tmux draws on
/// answers them, which is the one way to learn what that terminal can draw.
pub(crate) fn passthrough(out: &mut Vec<u8>, from: usize, scratch: &mut Vec<u8>) {
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
    fn images_never_reach_past_the_screen() {
        let cell = CellSize { width: 8, height: 16 };
        for passthrough in [true, false] {
            let small =
                Terminal { cols: 2, rows: 1, ..Terminal::new(Protocol::Kitty, cell).with_passthrough(passthrough) };
            let mut r = Renderer::new(small);
            let s = String::from_utf8(r.encode(&canvas(), Placement::Flow).to_vec()).unwrap();
            assert!(s.starts_with("\r\n\x1b[1A"), "room for one row only: {s:?}");
            assert!(s.contains("s=16,v=16"), "2×1 cells of 8×16 px: {s:?}");
            let s = String::from_utf8(r.encode(&canvas(), Placement::At(1, 0)).to_vec()).unwrap();
            assert!(s.contains("s=8,v=16"), "one column left: {s:?}");
            assert!(r.encode(&canvas(), Placement::At(2, 0)).is_empty(), "nothing fits, nothing sent");
        }
        // An unknown screen clips nothing: the whole canvas goes out.
        let mut unknown = Renderer::new(Terminal::new(Protocol::Kitty, cell));
        let s = String::from_utf8(unknown.encode(&canvas(), Placement::Flow).to_vec()).unwrap();
        assert!(s.contains("s=24,v=32"), "{s:?}");
    }

    fn canvas() -> Canvas {
        let mut c = Canvas::new(3, 2);
        c.set(0, 0, Rgb::hex(0xff0000));
        c.set(5, 7, Rgb::hex(0x00ff00));
        c
    }

    /// A canvas whose dots all differ: nothing repeats, so the payload stays large
    /// however well it is compressed, which is what forces the split.
    fn noisy(cols: u16, rows: u16) -> Canvas {
        let mut c = Canvas::new(cols, rows);
        let (w, h) = (cols as u32 * 2, rows as u32 * 4);
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        for y in 0..h {
            for x in 0..w {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                c.set(x as i32, y as i32, Rgb::hex((seed >> 24) as u32 & 0x00FF_FFFF));
            }
        }
        c
    }

    /// Every `a=T` starts an image; the chunks after it carry `m=` alone.
    fn images(s: &str) -> Vec<(u32, u16)> {
        s.match_indices("\x1b_Ga=T")
            .map(|(i, _)| {
                let head = &s[i..s[i..].find(';').map_or(s.len(), |e| i + e)];
                let field = |k: &str| {
                    head.split(',')
                        .find_map(|f| f.strip_prefix(k))
                        .unwrap_or_else(|| panic!("no {k} in {head:?}"))
                        .parse()
                        .unwrap()
                };
                (field("i="), field("r=") as u16)
            })
            .collect()
    }

    #[test]
    fn an_oversized_frame_is_split_into_tiles() {
        let cell = CellSize { width: 8, height: 16 };
        let mut r = Renderer::new(Terminal::new(Protocol::Kitty, cell));
        let (cols, rows) = (60u16, 40u16);
        let s = String::from_utf8(r.encode(&noisy(cols, rows), Placement::Flow).to_vec()).unwrap();
        let tiles = images(&s);
        assert!(tiles.len() > 1, "a frame this detailed does not fit in one image: {}", tiles.len());
        assert!(tiles.len() <= MAX_TILES, "{}", tiles.len());
        assert_eq!(tiles.iter().map(|&(_, r)| r).sum::<u16>(), rows, "the tiles cover exactly the rows reserved");
        let ids: std::collections::BTreeSet<u32> = tiles.iter().map(|&(i, _)| i).collect();
        assert_eq!(ids.len(), tiles.len(), "each tile needs an id of its own");
        assert!(
            ids.iter().all(|i| (r.image_id()..r.image_id() + MAX_TILES as u32).contains(i)),
            "ids stay in the block"
        );
        // The cursor walks down to each tile and all the way back, so what follows
        // starts where a single image would have left it.
        let down: u16 = s.matches("\x1b[").filter(|_| true).count() as u16;
        let _ = down;
        assert!(s.contains(&format!("\x1b[{}A", rows - tiles.last().unwrap().1)), "the walk is undone: {s:.120?}");
    }

    #[test]
    fn a_frame_that_fits_stays_one_image() {
        let cell = CellSize { width: 8, height: 16 };
        let mut r = Renderer::new(Terminal::new(Protocol::Kitty, cell));
        let s = String::from_utf8(r.encode(&canvas(), Placement::Flow).to_vec()).unwrap();
        assert_eq!(images(&s).len(), 1, "nothing to split");
        assert_eq!(images(&s)[0].0, r.image_id(), "and it uses the renderer's own id");
    }

    #[test]
    fn tiles_move_the_pane_cursor_not_the_outer_one() {
        let cell = CellSize { width: 8, height: 16 };
        let mut r = Renderer::new(Terminal::new(Protocol::Kitty, cell).with_passthrough(true));
        let s = String::from_utf8(r.encode(&noisy(60, 40), Placement::Flow).to_vec()).unwrap();
        assert!(images(&s).len() > 1, "this frame should have been split");
        assert!(!s.contains("\x1bPtmux;\x1b\x1b["), "a wrapped cursor move would move the outer terminal");
        assert!(s.contains("\x1b[1X\x1bPtmux;\x1b\x1b_G"), "each tile still syncs the pane cursor first");
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
        assert!(s.contains("c=3,r=2") && !s.contains("U=1"), "the picture covers its 3×2 cells: {s:?}");
        let v = String::from_utf8(k.encode(&canvas(), Placement::Virtual).to_vec()).unwrap();
        assert!(v.starts_with("\x1b_G") && v.contains("c=3,r=2,U=1") && v.ends_with("\x1b\\"));

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
        let caps = |r: &Renderer| {
            (
                r.out.capacity(),
                r.payload.capacity(),
                r.scratch.capacity(),
                r.raster.rgba.capacity(),
                r.window.capacity(),
            )
        };
        let before = caps(&r);
        r.encode(&c, Placement::Flow);
        assert_eq!(before, caps(&r));

        // A frame split into tiles compresses once per tile, and the search tables are
        // a quarter of a megabyte: reallocating them per tile is what this catches.
        let big = noisy(60, 40);
        r.encode(&big, Placement::Flow);
        let before = caps(&r);
        r.encode(&big, Placement::Flow);
        assert_eq!(before, caps(&r));
    }
}
