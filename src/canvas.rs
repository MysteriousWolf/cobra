//! The dot grid.

use crate::Color;

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
    pub color: Option<Color>,
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

/// Ordered-dither threshold for dot `(x, y)`: a 4×4 Bayer matrix scaled to `0..1`
/// (sixteen evenly spaced levels). A dot is drawn when its coverage exceeds this.
#[inline]
pub fn bayer(x: i32, y: i32) -> f32 {
    const M: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    (M[(y & 3) as usize][(x & 3) as usize] as f32 + 0.5) / 16.0
}

/// A grid of individually coloured braille dots.
///
/// The canvas is sized in terminal cells; each cell holds a 2×4 block of dots, so a
/// `cols × rows` canvas has `2·cols × 4·rows` addressable dots with `(0, 0)` at the top
/// left. Coordinates are `i32` so callers can draw partially off-canvas shapes without
/// clamping; out-of-range dots are ignored.
///
/// Storage is one `u32` per dot (`0` = unset, otherwise a tagged [`Color`]), so a full
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
    ///
    /// Accepts an [`Rgb`](crate::Rgb), a `0xRRGGBB` literal, an `(r, g, b)` tuple or a
    /// [`Color`] (for palette colours).
    #[inline]
    pub fn set(&mut self, x: i32, y: i32, color: impl Into<Color>) {
        if let Some(i) = self.index(x, y) {
            self.dots[i] = color.into().packed();
        }
    }

    /// Sets dot `(x, y)` to `color` with probability `coverage` (`0..=1`) using an
    /// ordered 4×4 Bayer pattern, which is what dithering means on a dot matrix:
    /// dots are on or off, so partial coverage is spread evenly across the area.
    /// Dots that the pattern skips are left unchanged.
    #[inline]
    pub fn set_dithered(&mut self, x: i32, y: i32, color: impl Into<Color>, coverage: f32) {
        if coverage > bayer(x, y) {
            self.set(x, y, color);
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
    pub fn get(&self, x: i32, y: i32) -> Option<Color> {
        let v = self.index(x, y).map(|i| self.dots[i])?;
        (v != 0).then(|| Color::from_packed(v))
    }

    /// Draws a one-dot-wide line with Bresenham's algorithm.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: impl Into<Color>) {
        let color = color.into();
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
    pub fn disc(&mut self, cx: f32, cy: f32, r: f32, color: impl Into<Color>) {
        self.disc_dithered(cx, cy, r, color, 1.0);
    }

    /// Fills a disc like [`disc`](Self::disc) but only `coverage` (`0..=1`) of its
    /// dots, in an ordered pattern; see [`set_dithered`](Self::set_dithered).
    pub fn disc_dithered(&mut self, cx: f32, cy: f32, r: f32, color: impl Into<Color>, coverage: f32) {
        let color = color.into();
        let r2 = r * r;
        for y in (cy - r).floor() as i32..=(cy + r).ceil() as i32 {
            for x in (cx - r).floor() as i32..=(cx + r).ceil() as i32 {
                let (ex, ey) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                if ex * ex + ey * ey <= r2 {
                    self.set_dithered(x, y, color, coverage);
                }
            }
        }
    }

    /// Unsets every dot inside a disc of radius `r` centred on `(cx, cy)`; a cleared
    /// ring around a shape separates it from what is behind it on any background.
    pub fn clear_disc(&mut self, cx: f32, cy: f32, r: f32) {
        let r2 = r * r;
        for y in (cy - r).floor() as i32..=(cy + r).ceil() as i32 {
            for x in (cx - r).floor() as i32..=(cx + r).ceil() as i32 {
                let (ex, ey) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                if ex * ex + ey * ey <= r2 {
                    self.unset(x, y);
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
        Cell { bits, color: (best != 0).then(|| Color::from_packed(best)) }
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
    use crate::Rgb;

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
        assert_eq!(c.cell(0, 0).color, Some(Color::Rgb(b)));
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

#[cfg(test)]
mod dither_tests {
    use super::*;

    #[test]
    fn coverage_matches_dot_count() {
        for (coverage, expect) in [(0.0, 0), (0.25, 4), (0.5, 8), (1.0, 16)] {
            let mut c = Canvas::new(2, 1);
            for y in 0..4 {
                for x in 0..4 {
                    c.set_dithered(x, y, 0xffffffu32, coverage);
                }
            }
            let n = c.cells().map(|cell| cell.bits.count_ones()).sum::<u32>();
            assert_eq!(n, expect, "coverage {coverage}");
        }
    }

    #[test]
    fn palette_colours_round_trip() {
        let mut c = Canvas::new(1, 1);
        c.set(0, 0, Color::Indexed(4));
        c.set(1, 0, Color::Foreground);
        assert_eq!(c.get(0, 0), Some(Color::Indexed(4)));
        assert_eq!(c.get(1, 0), Some(Color::Foreground));
    }
}
