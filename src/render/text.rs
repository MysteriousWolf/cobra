//! Braille glyphs with a per-cell foreground: truecolor SGR for [`Color::Rgb`],
//! `38;5;n` for palette entries and the default foreground for [`Color::Foreground`].
//!
//! On terminals with fewer colours every dot is first quantised to the nearest colour
//! the terminal has ([`Color::quantize`]) and only then the dominant colour of the cell
//! is picked, so two near-identical shades that land on the same palette entry count
//! together rather than splitting the vote.

use crate::render::Placement;
use crate::text::{Attrs, TextStyle};
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

    /// A text style with both its colours quantised.
    #[inline]
    pub(crate) fn style(&mut self, style: TextStyle) -> TextStyle {
        if self.is_identity() {
            return style;
        }
        let mut map = |c: Option<Color>| c.map(|c| Color::from_packed(self.dot(c.packed())));
        TextStyle { fg: map(style.fg), bg: map(style.bg), attrs: style.attrs }
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
        let mut current = TextStyle::default();
        for col in 0..cols {
            let text = canvas.text_at(col as i32, row as i32);
            if text.is_continuation() {
                continue; // the double-width character before it already covered this cell
            }
            let (ch, style) = if text.is_empty() {
                let cell = q.cell(canvas, col, row);
                let Some(color) = cell.color else {
                    // A blank cell shows nothing, so it keeps the current colour and
                    // costs no SGR — but a background or an attribute would bleed
                    // across it, so those are dropped.
                    if current.bg.is_some() || current.attrs != Attrs::NONE {
                        out.extend_from_slice(b"\x1b[0m");
                        current = TextStyle::default();
                    }
                    out.extend_from_slice(cell.glyph().encode_utf8(&mut buf).as_bytes());
                    continue;
                };
                (cell.glyph(), TextStyle { fg: Some(color), ..TextStyle::default() })
            } else {
                (text.ch, q.style(text.style))
            };
            apply(&mut current, style, out);
            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
        }
        out.extend_from_slice(b"\x1b[0m");
        if placement == Placement::Flow {
            out.extend_from_slice(b"\r\n");
        }
    }
}

/// Writes just the text layer, positioned, for the image protocols: the raster leaves
/// those cells transparent and the characters are printed over the picture.
///
/// For [`Placement::Flow`] the cursor must be saved at the frame's top-left corner;
/// each run returns there and steps out to its own cell.
pub(super) fn overlay(
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
        let mut col = 0;
        while col < cols {
            if canvas.text_at(col as i32, row as i32).is_empty() {
                col += 1;
                continue;
            }
            match placement {
                Placement::At(c, r) => {
                    out.extend_from_slice(format!("\x1b[{};{}H", r + row + 1, c + col + 1).as_bytes())
                }
                _ => {
                    out.extend_from_slice(b"\x1b8\r");
                    if row > 0 {
                        out.extend_from_slice(format!("\x1b[{row}B").as_bytes());
                    }
                    if col > 0 {
                        out.extend_from_slice(format!("\x1b[{col}C").as_bytes());
                    }
                }
            }
            let mut current = TextStyle::default();
            while col < cols {
                let text = canvas.text_at(col as i32, row as i32);
                if text.is_empty() {
                    break;
                }
                col += 1;
                if text.is_continuation() {
                    continue;
                }
                apply(&mut current, q.style(text.style), out);
                out.extend_from_slice(text.ch.encode_utf8(&mut buf).as_bytes());
            }
            out.extend_from_slice(b"\x1b[0m");
        }
    }
}

/// Emits the difference between two styles. Colours alone are one SGR; anything else
/// resets and sets the lot, which is shorter than tracking each attribute off again.
fn apply(current: &mut TextStyle, next: TextStyle, out: &mut Vec<u8>) {
    if *current == next {
        return;
    }
    if current.attrs != next.attrs || (current.bg != next.bg && next.bg.is_none()) {
        out.extend_from_slice(b"\x1b[0m");
        *current = TextStyle::default();
        for (bit, code) in [
            (Attrs::BOLD, "1"),
            (Attrs::DIM, "2"),
            (Attrs::ITALIC, "3"),
            (Attrs::UNDERLINE, "4"),
            (Attrs::REVERSE, "7"),
        ] {
            if next.attrs.has(bit) {
                out.extend_from_slice(format!("\x1b[{code}m").as_bytes());
            }
        }
    }
    if current.fg != next.fg {
        sgr(next.fg.unwrap_or(Color::Foreground), out);
    }
    if current.bg != next.bg
        && let Some(bg) = next.bg
    {
        sgr_bg(bg, out);
    }
    *current = next;
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

/// Background SGR for a colour, mirroring [`sgr`].
pub(crate) fn sgr_bg(color: Color, out: &mut Vec<u8>) {
    match color {
        Color::Rgb(Rgb { r, g, b }) => out.extend_from_slice(format!("\x1b[48;2;{r};{g};{b}m").as_bytes()),
        Color::Indexed(i @ 0..=7) => out.extend_from_slice(format!("\x1b[{}m", 40 + i).as_bytes()),
        Color::Indexed(i @ 8..=15) => out.extend_from_slice(format!("\x1b[{}m", 92 + i).as_bytes()),
        Color::Indexed(i) => out.extend_from_slice(format!("\x1b[48;5;{i}m").as_bytes()),
        Color::Foreground => out.extend_from_slice(b"\x1b[49m"),
    }
}
