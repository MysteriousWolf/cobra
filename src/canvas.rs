//! The dot grid.

use crate::Rgb;

/// Braille bit for dot `(dx, dy)` inside a cell, indexed `[dy][dx]`.
///
/// Unicode braille orders dots column-major with the bottom row appended last
/// (U+2800 + bits: 1 4 / 2 5 / 3 6 / 7 8).
const BITS: [[u8; 2]; 4] = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];

/// Dots per cell horizontally.
pub const DOTS_X: u16 = 2;
/// Dots per cell vertically.
pub const DOTS_Y: u16 = 4;

/// One terminal cell of a canvas: which of its eight dots are set, and the colour the
/// cell should take when it can only have one (text fallback).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    /// Braille dot bits (`U+2800 + bits` is the glyph).
    pub bits: u8,
    /// The most frequent colour among the set dots, `None` when the cell is empty.
    pub color: Option<Rgb>,
}

impl Cell {
    /// The braille glyph for this cell.
    #[inline]
    pub fn glyph(&self) -> char {
        braille(self.bits)
    }
}

/// Braille glyph for a dot bit pattern.
#[inline]
pub fn braille(bits: u8) -> char {
    // Every value in U+2800..=U+28FF is a valid scalar.
    char::from_u32(0x2800 + bits as u32).unwrap_or('\u{2800}')
}

/// A grid of individually coloured braille dots.
///
/// The canvas is sized in terminal cells; each cell holds a 2×4 block of dots, so a
/// `cols × rows` canvas has `2·cols × 4·rows` addressable dots with `(0, 0)` at the top
/// left. Coordinates are `i32` so callers can draw partially off-canvas shapes without
/// clamping; out-of-range dots are ignored.
///
/// Storage is one `u32` per dot (`0` = unset, otherwise `0xFF_RR_GG_BB`), so a full
/// 200×50-cell canvas is 320 KiB and never allocates after construction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Canvas {
    cols: u16,
    rows: u16,
    dots: Vec<u32>,
}

impl Canvas {
    /// Creates an empty canvas of `cols × rows` cells.
    pub fn new(cols: u16, rows: u16) -> Self {
        Self { cols, rows, dots: vec![0; cols as usize * DOTS_X as usize * rows as usize * DOTS_Y as usize] }
    }

    /// Width in cells.
    #[inline]
    pub fn cols(&self) -> u16 {
        self.cols
    }

    /// Height in cells.
    #[inline]
    pub fn rows(&self) -> u16 {
        self.rows
    }

    /// Width in dots (`2 · cols`).
    #[inline]
    pub fn width(&self) -> i32 {
        self.cols as i32 * DOTS_X as i32
    }

    /// Height in dots (`4 · rows`).
    #[inline]
    pub fn height(&self) -> i32 {
        self.rows as i32 * DOTS_Y as i32
    }

    /// Unsets every dot. Keeps the allocation.
    pub fn clear(&mut self) {
        self.dots.fill(0);
    }

    #[inline]
    fn index(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.width() && y < self.height())
            .then(|| y as usize * self.width() as usize + x as usize)
    }

    /// Sets dot `(x, y)` to `color`. Out-of-range dots are ignored.
    #[inline]
    pub fn set(&mut self, x: i32, y: i32, color: Rgb) {
        if let Some(i) = self.index(x, y) {
            self.dots[i] = color.packed();
        }
    }

    /// Unsets dot `(x, y)`.
    #[inline]
    pub fn unset(&mut self, x: i32, y: i32) {
        if let Some(i) = self.index(x, y) {
            self.dots[i] = 0;
        }
    }

    /// The colour of dot `(x, y)`, `None` if unset or out of range.
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> Option<Rgb> {
        let v = self.index(x, y).map(|i| self.dots[i])?;
        (v != 0).then(|| Rgb::from_packed(v))
    }

    /// Draws a one-dot-wide line with Bresenham's algorithm.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgb) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = ((x1 - x0).signum(), (y1 - y0).signum());
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        loop {
            self.set(x, y, color);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Fills a disc of radius `r` (in dots) centred on `(cx, cy)`.
    pub fn disc(&mut self, cx: f32, cy: f32, r: f32, color: Rgb) {
        let r2 = r * r;
        for y in (cy - r).floor() as i32..=(cy + r).ceil() as i32 {
            for x in (cx - r).floor() as i32..=(cx + r).ceil() as i32 {
                let (ex, ey) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                if ex * ex + ey * ey <= r2 {
                    self.set(x, y, color);
                }
            }
        }
    }

    /// Packed colours of the eight dots of cell `(col, row)`, row-major (`0` = unset).
    #[inline]
    pub(crate) fn cell_dots(&self, col: u16, row: u16) -> [u32; 8] {
        let w = self.width() as usize;
        let base = row as usize * DOTS_Y as usize * w + col as usize * DOTS_X as usize;
        let mut out = [0u32; 8];
        for dy in 0..DOTS_Y as usize {
            let i = base + dy * w;
            out[dy * 2] = self.dots[i];
            out[dy * 2 + 1] = self.dots[i + 1];
        }
        out
    }

    /// Cell `(col, row)` as a glyph plus its dominant colour.
    pub fn cell(&self, col: u16, row: u16) -> Cell {
        let dots = self.cell_dots(col, row);
        let mut bits = 0u8;
        let (mut best, mut best_n) = (0u32, 0u8);
        for (i, &v) in dots.iter().enumerate() {
            if v == 0 {
                continue;
            }
            bits |= BITS[i / 2][i % 2];
            // At most eight dots: a quadratic count is cheaper than hashing.
            let n = dots[i..].iter().filter(|&&o| o == v).count() as u8;
            if n > best_n {
                best = v;
                best_n = n;
            }
        }
        Cell { bits, color: (best != 0).then(|| Rgb::from_packed(best)) }
    }

    /// Iterates over all cells, row-major.
    pub fn cells(&self) -> impl Iterator<Item = Cell> + '_ {
        (0..self.rows).flat_map(move |r| (0..self.cols).map(move |c| self.cell(c, r)))
    }

    /// Plain braille text, one line per row, no colour. This is what a user gets when they
    /// copy the canvas out of a terminal running the text fallback.
    pub fn to_text(&self) -> String {
        let mut s = String::with_capacity((self.cols as usize * 3 + 1) * self.rows as usize);
        for r in 0..self.rows {
            for c in 0..self.cols {
                s.push(self.cell(c, r).glyph());
            }
            s.push('\n');
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_match_unicode_layout() {
        let mut c = Canvas::new(1, 1);
        c.set(0, 0, Rgb::hex(0xff0000));
        c.set(1, 3, Rgb::hex(0x00ff00));
        assert_eq!(c.cell(0, 0).bits, 0x81);
        assert_eq!(c.cell(0, 0).glyph(), '⢁');
    }

    #[test]
    fn dominant_colour_wins() {
        let mut c = Canvas::new(1, 1);
        let (a, b) = (Rgb::hex(1), Rgb::hex(2));
        c.set(0, 0, a);
        c.set(1, 0, b);
        c.set(0, 1, b);
        assert_eq!(c.cell(0, 0).color, Some(b));
        assert_eq!(Canvas::new(2, 2).cell(1, 1).color, None);
    }

    #[test]
    fn out_of_range_is_ignored() {
        let mut c = Canvas::new(2, 1);
        c.set(-1, 0, Rgb::hex(1));
        c.set(4, 0, Rgb::hex(1));
        c.set(0, 4, Rgb::hex(1));
        assert!(c.cells().all(|cell| cell.bits == 0));
        assert_eq!(c.get(99, 99), None);
    }

    #[test]
    fn line_and_text() {
        let mut c = Canvas::new(2, 1);
        c.line(0, 0, 3, 0, Rgb::hex(0xffffff));
        assert_eq!(c.to_text(), "⠉⠉\n");
    }
}
