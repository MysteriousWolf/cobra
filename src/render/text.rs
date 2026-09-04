//! Braille glyphs with per-cell truecolor foreground.

use crate::render::Placement;
use crate::{Canvas, Rgb};

pub(super) fn frame(canvas: &Canvas, cols: u16, rows: u16, placement: Placement, out: &mut Vec<u8>) {
    let mut buf = [0u8; 4];
    for row in 0..rows {
        if let Placement::At(c, r) = placement {
            out.extend_from_slice(format!("\x1b[{};{}H", r + row + 1, c + 1).as_bytes());
        }
        let mut current: Option<Rgb> = None;
        for col in 0..cols {
            let cell = canvas.cell(col, row);
            if let Some(c) = cell.color.filter(|_| cell.color != current) {
                out.extend_from_slice(format!("\x1b[38;2;{};{};{}m", c.r, c.g, c.b).as_bytes());
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
