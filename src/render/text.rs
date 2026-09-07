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
        Canvas::cell_of(dots, canvas.cell_prio(col, row))
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
            cursor_to(r + row + 1, c + 1, out);
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
                Placement::At(c, r) => cursor_to(r + row + 1, c + col + 1, out),
                _ => {
                    out.extend_from_slice(b"\x1b8\r");
                    if row > 0 {
                        step(row, b'B', out);
                    }
                    if col > 0 {
                        step(col, b'C', out);
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

/// `CUP`: moves the cursor to a 1-based row and column.
pub(super) fn cursor_to(row: u16, col: u16, out: &mut Vec<u8>) {
    out.extend_from_slice(b"\x1b[");
    num(row as u32, out);
    out.push(b';');
    num(col as u32, out);
    out.push(b'H');
}

/// A relative cursor move: `n` cells in the direction `dir` (`A`–`D`).
pub(super) fn step(n: u16, dir: u8, out: &mut Vec<u8>) {
    out.extend_from_slice(b"\x1b[");
    num(n as u32, out);
    out.push(dir);
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
            (Attrs::BOLD, b"\x1b[1m".as_slice()),
            (Attrs::DIM, b"\x1b[2m"),
            (Attrs::ITALIC, b"\x1b[3m"),
            (Attrs::UNDERLINE, b"\x1b[4m"),
            (Attrs::REVERSE, b"\x1b[7m"),
        ] {
            if next.attrs.has(bit) {
                out.extend_from_slice(code);
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

/// Appends a decimal number. A frame emits one SGR per colour change and one escape
/// per row, so these are written straight into the output buffer rather than through
/// `format!`, which would allocate a `String` for each of them.
#[inline]
pub(crate) fn num(n: u32, out: &mut Vec<u8>) {
    let mut buf = [0u8; 10];
    let mut i = buf.len();
    let mut n = n;
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    out.extend_from_slice(&buf[i..]);
}

/// Foreground SGR for a colour. ANSI indices use the classic `30–37` / `90–97` codes,
/// which even 16-colour terminals understand.
pub(crate) fn sgr(color: Color, out: &mut Vec<u8>) {
    color_sgr(color, b"\x1b[38;2;", 30, b"\x1b[38;5;", b"\x1b[39m", out);
}

/// Background SGR for a colour, mirroring [`sgr`].
pub(crate) fn sgr_bg(color: Color, out: &mut Vec<u8>) {
    color_sgr(color, b"\x1b[48;2;", 40, b"\x1b[48;5;", b"\x1b[49m", out);
}

/// The two differ only in their prefixes: backgrounds are foregrounds plus ten.
#[inline]
fn color_sgr(color: Color, truecolor: &[u8], base: u32, indexed: &[u8], default: &[u8], out: &mut Vec<u8>) {
    match color {
        Color::Rgb(Rgb { r, g, b }) => {
            out.extend_from_slice(truecolor);
            for (i, c) in [r, g, b].into_iter().enumerate() {
                if i > 0 {
                    out.push(b';');
                }
                num(c as u32, out);
            }
            out.push(b'm');
        }
        // 0–7 are `base + i`, 8–15 the bright `base + 52 + i`.
        Color::Indexed(i @ 0..=15) => {
            out.extend_from_slice(b"\x1b[");
            num(base + if i < 8 { i as u32 } else { 52 + i as u32 }, out);
            out.push(b'm');
        }
        Color::Indexed(i) => {
            out.extend_from_slice(indexed);
            num(i as u32, out);
            out.push(b'm');
        }
        Color::Foreground => out.extend_from_slice(default),
    }
}
