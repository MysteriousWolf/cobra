//! Braille glyphs with a per-cell foreground: truecolor SGR for [`Color::Rgb`],
//! `38;5;n` for palette entries and the default foreground for [`Color::Foreground`].
//!
//! On terminals with fewer colours every dot is first quantised to the nearest colour
//! the terminal has ([`Color::quantize`]) and only then the dominant colour of the cell
//! is picked, so two near-identical shades that land on the same palette entry count
//! together rather than splitting the vote.

use crate::render::Placement;
use crate::{Canvas, Color, Depth, Palette, Rgb};

/// Quantises packed dots to a colour depth, remembering recent answers. Frames use a
/// handful of colours, so a small direct-mapped cache turns the nearest-colour search
/// into one multiply and a compare per dot.
pub(crate) struct Quantizer<'a> {
    depth: Depth,
    palette: &'a Palette,
    cache: [(u32, u32); 256],
}

impl<'a> Quantizer<'a> {
    pub(crate) fn new(depth: Depth, palette: &'a Palette) -> Self {
        Self { depth, palette, cache: [(0, 0); 256] }
    }

    /// `true` when nothing needs quantising.
    #[inline]
    pub(crate) fn is_identity(&self) -> bool {
        self.depth == Depth::TrueColor
    }

    #[inline]
    pub(crate) fn dot(&mut self, packed: u32) -> u32 {
        if packed == 0 || self.is_identity() {
            return packed;
        }
        let slot = (packed.wrapping_mul(0x9E37_79B1) >> 24) as usize;
        let (k, v) = self.cache[slot];
        if k == packed {
            return v;
        }
        let v = Color::from_packed(packed).quantize(self.depth, self.palette).packed();
        self.cache[slot] = (packed, v);
        v
    }

    /// [`Canvas::cell`] after quantising every dot.
    #[inline]
    pub(crate) fn cell(&mut self, canvas: &Canvas, col: u16, row: u16) -> crate::Cell {
        let mut dots = canvas.cell_dots(col, row);
        if !self.is_identity() {
            for d in &mut dots {
                *d = self.dot(*d);
            }
        }
        Canvas::cell_of(dots)
    }
}

pub(super) fn frame(
    canvas: &Canvas,
    cols: u16,
    rows: u16,
    placement: Placement,
    depth: Depth,
    palette: &Palette,
    out: &mut Vec<u8>,
) {
    let mut buf = [0u8; 4];
    let mut q = Quantizer::new(depth, palette);
    for row in 0..rows {
        if let Placement::At(c, r) = placement {
            out.extend_from_slice(format!("\x1b[{};{}H", r + row + 1, c + 1).as_bytes());
        }
        let mut current: Option<Color> = None;
        for col in 0..cols {
            let cell = q.cell(canvas, col, row);
            if let Some(c) = cell.color.filter(|_| cell.color != current) {
                sgr(c, out);
                current = Some(c);
            }
            out.extend_from_slice(cell.glyph().encode_utf8(&mut buf).as_bytes());
        }
        out.extend_from_slice(b"\x1b[0m");
        if placement == Placement::Flow {
            out.extend_from_slice(b"\r\n");
        }
    }
}

/// Foreground SGR for a colour. ANSI indices use the classic `30–37` / `90–97` codes,
/// which even 16-colour terminals understand.
pub(crate) fn sgr(color: Color, out: &mut Vec<u8>) {
    match color {
        Color::Rgb(Rgb { r, g, b }) => out.extend_from_slice(format!("\x1b[38;2;{r};{g};{b}m").as_bytes()),
        Color::Indexed(i @ 0..=7) => out.extend_from_slice(format!("\x1b[{}m", 30 + i).as_bytes()),
        Color::Indexed(i @ 8..=15) => out.extend_from_slice(format!("\x1b[{}m", 82 + i).as_bytes()),
        Color::Indexed(i) => out.extend_from_slice(format!("\x1b[38;5;{i}m").as_bytes()),
        Color::Foreground => out.extend_from_slice(b"\x1b[39m"),
    }
}
