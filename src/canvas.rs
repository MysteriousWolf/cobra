//! The dot grid.

use crate::render::Quantizer;
use crate::text::TextCell;
use crate::{Color, Depth, Paint, Palette, Transform};

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
    /// The most frequent colour among the set dots, `None` when the cell is empty. On
    /// a canvas [flattened](crate::Layers::flatten) from layers only the dots of the
    /// topmost layer in the cell vote, so what is in front keeps its colour.
    pub color: Option<Color>,
}

impl Cell {
    /// The braille glyph for this cell.
    #[inline]
    pub fn glyph(&self) -> char {
        braille(self.bits)
    }

    /// Whether dot `(dx, dy)` of the cell (`dx` in `0..2`, `dy` in `0..4`) is set.
    #[inline]
    pub fn dot(&self, dx: u16, dy: u16) -> bool {
        dx < DOTS_X && dy < DOTS_Y && self.bits & BITS[dy as usize][dx as usize] != 0
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
///
/// On top of the dots sits a [text layer](crate::text): real characters, one per cell,
/// written with [`print`](Self::print). It stays unallocated until something is
/// printed, and a cell that holds a character shows it instead of its dots.
///
/// Several canvases stacked, with occlusion and effects between them, are a
/// [`Layers`](crate::Layers); flattening one gives back a plain canvas.
#[derive(Clone, Debug)]
pub struct Canvas {
    cols: u16,
    rows: u16,
    pub(crate) dots: Vec<u32>,
    /// Empty, or one entry per cell; see [`crate::text`].
    pub(crate) text: Vec<TextCell>,
    /// Empty, or one entry per dot: the layer each dot was flattened from, which
    /// decides the colour of a cell in the text fallback; see [`crate::layer`].
    pub(crate) prio: Vec<u8>,
    /// The box `(x0, y0, x1, y1)` every dot written since the last clear lies in:
    /// a bound on where set dots can be, so a scan for them stays small.
    pub(crate) dirty: Option<(i32, i32, i32, i32)>,
    /// Where drawing lands; the identity outside [`with`](Self::with).
    pub(crate) transform: Transform,
}

impl PartialEq for Canvas {
    /// Two canvases are equal when they show the same thing: size, dots and text.
    fn eq(&self, other: &Self) -> bool {
        self.cols == other.cols
            && self.rows == other.rows
            && self.dots == other.dots
            && self.text == other.text
            && self.prio == other.prio
    }
}

impl Eq for Canvas {}

impl Canvas {
    /// Creates an empty canvas of `cols × rows` cells.
    pub fn new(cols: u16, rows: u16) -> Self {
        let dots = vec![0; cols as usize * DOTS_X as usize * rows as usize * DOTS_Y as usize];
        Self { cols, rows, dots, text: Vec::new(), prio: Vec::new(), dirty: None, transform: Transform::IDENTITY }
    }

    /// Draws everything in `f` through `t`: the coordinates every primitive takes
    /// are local, and `t` says where they land. Nested calls compose, inner first,
    /// so a hand drawn inside an arm drawn inside a body moves with all three.
    /// [`set`](Self::set) and [`unset`](Self::unset) map their dot through it too;
    /// whole-cell operations ([`print`](Self::print), [`span`](Self::span), the
    /// mask operations) and reads ([`get`](Self::get)) are in canvas coordinates.
    ///
    /// A translation or a flip is exact; under a rotation or a scale, boxes,
    /// ellipses and rings are drawn as paths, and stroke widths scale with the
    /// transform.
    ///
    /// ```
    /// use cobra::{Canvas, Rgb, Transform};
    ///
    /// let mut canvas = Canvas::new(20, 5);
    /// let leg = |c: &mut Canvas| c.fill_rect(-1.0, 0.0, 2.0, 8.0, Rgb::hex(0xffa657));
    /// for (x, angle) in [(10.0, -0.3), (14.0, 0.3)] {
    ///     canvas.with(Transform::at(x, 8.0).rotate(angle), leg); // rotated, then placed
    /// }
    /// ```
    pub fn with(&mut self, t: Transform, f: impl FnOnce(&mut Canvas)) {
        let outer = self.transform;
        self.transform = t.then(&outer);
        f(self);
        self.transform = outer;
    }

    /// The transform drawing currently lands through; the identity by default.
    #[inline]
    pub fn transform(&self) -> Transform {
        self.transform
    }

    /// Notes that dots in `x0..x1 × y0..y1` may have been written. The box is
    /// clipped to the canvas here, once, so a caller can hand in the box its
    /// geometry makes (a line to a point off the canvas, say) and the bound never
    /// reaches past the dots that exist.
    #[inline]
    pub(crate) fn mark(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let (x0, y0) = (x0.max(0), y0.max(0));
        let (x1, y1) = (x1.min(self.width()), y1.min(self.height()));
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        self.dirty = Some(match self.dirty {
            Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
            None => (x0, y0, x1, y1),
        });
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

    /// Unsets every dot and empties the text layer. Keeps the allocations.
    pub fn clear(&mut self) {
        // Only what may have been written needs unsetting.
        match self.dirty {
            Some((x0, y0, x1, y1)) if (x1 - x0) * (y1 - y0) * 2 < self.width() * self.height() => {
                let w = self.width() as usize;
                for y in y0..y1 {
                    self.dots[y as usize * w + x0 as usize..y as usize * w + x1 as usize].fill(0);
                }
            }
            _ => self.dots.fill(0),
        }
        self.dirty = None;
        self.text.clear();
        self.prio.clear();
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
        let (x, y) = self.map_dot(x, y);
        if let Some(i) = self.index(x, y) {
            self.dots[i] = color.into().packed();
            self.mark(x, y, x + 1, y + 1);
        }
    }

    /// Where dot `(x, y)` lands under the current transform.
    #[inline]
    fn map_dot(&self, x: i32, y: i32) -> (i32, i32) {
        if self.transform.is_identity() {
            return (x, y);
        }
        let (fx, fy) = self.transform.apply((x as f32 + 0.5, y as f32 + 0.5));
        (fx.floor() as i32, fy.floor() as i32)
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

    /// Unsets dot `(x, y)`; like [`set`](Self::set), under the current transform.
    #[inline]
    pub fn unset(&mut self, x: i32, y: i32) {
        let (x, y) = self.map_dot(x, y);
        if let Some(i) = self.index(x, y) {
            self.dots[i] = 0;
        }
    }

    /// The colour of dot `(x, y)`, `None` if unset or out of range. Reads are in
    /// canvas coordinates: the transform of [`with`](Self::with) is not applied.
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> Option<Color> {
        let v = self.index(x, y).map(|i| self.dots[i])?;
        (v != 0).then(|| Color::from_packed(v))
    }

    /// Draws a one-dot-wide line with Bresenham's algorithm.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: impl Into<Color>) {
        let packed = color.into().packed();
        let ((x0, y0), (x1, y1)) = (self.map_dot(x0, y0), self.map_dot(x1, y1));
        self.mark(x0.min(x1), y0.min(y1), x0.max(x1) + 1, y0.max(y1) + 1);
        self.bresenham(x0, y0, x1, y1, |c, x, y| {
            if let Some(i) = c.index(x, y) {
                c.dots[i] = packed;
            }
        });
    }

    /// Walks Bresenham's line from `(x0, y0)` to `(x1, y1)`, calling `plot` per dot.
    #[inline]
    pub(crate) fn bresenham(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, mut plot: impl FnMut(&mut Self, i32, i32)) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = ((x1 - x0).signum(), (y1 - y0).signum());
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        loop {
            plot(self, x, y);
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
    pub fn disc(&mut self, cx: f32, cy: f32, r: f32, paint: impl Into<Paint>) {
        self.fill_ellipse(cx, cy, r, r, paint);
    }

    /// Fills a disc like [`disc`](Self::disc) but only `coverage` (`0..=1`) of its
    /// dots, in an ordered pattern; see [`set_dithered`](Self::set_dithered).
    pub fn disc_dithered(&mut self, cx: f32, cy: f32, r: f32, color: impl Into<Color>, coverage: f32) {
        self.fill_ellipse(cx, cy, r, r, Paint::dithered(color, coverage));
    }

    /// Unsets every dot inside a disc of radius `r` centred on `(cx, cy)`; a cleared
    /// ring around a shape separates it from what is behind it on any background.
    pub fn clear_disc(&mut self, cx: f32, cy: f32, r: f32) {
        self.fill_ellipse(cx, cy, r, r, Paint::erase());
    }

    /// The box `(x0, y0, x1, y1)` (exclusive on the far side) around the set dots,
    /// `None` when there are none.
    pub(crate) fn dot_bounds(&self) -> Option<(i32, i32, i32, i32)> {
        let (bx0, by0, bx1, by1) = self.dirty?;
        let w = self.width() as usize;
        let (mut x0, mut y0, mut x1, mut y1) = (bx1, by1, bx0, by0);
        for y in by0..by1 {
            let row = &self.dots[y as usize * w + bx0 as usize..y as usize * w + bx1 as usize];
            let Some(first) = row.iter().position(|&d| d != 0) else { continue };
            let last = row.iter().rposition(|&d| d != 0).unwrap_or(first);
            (x0, x1) = (x0.min(bx0 + first as i32), x1.max(bx0 + last as i32 + 1));
            (y0, y1) = (y0.min(y), y + 1);
        }
        (x0 < x1 && y0 < y1).then_some((x0, y0, x1, y1))
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

    /// Layer of each of the eight dots of cell `(col, row)`, row-major; all zero on
    /// a canvas that was not flattened from layers.
    #[inline]
    pub(crate) fn cell_prio(&self, col: u16, row: u16) -> [u8; 8] {
        let mut out = [0u8; 8];
        if self.prio.is_empty() {
            return out;
        }
        let w = self.width() as usize;
        let base = row as usize * DOTS_Y as usize * w + col as usize * DOTS_X as usize;
        for dy in 0..DOTS_Y as usize {
            let i = base + dy * w;
            out[dy * 2] = self.prio[i];
            out[dy * 2 + 1] = self.prio[i + 1];
        }
        out
    }

    /// Cell `(col, row)` as a glyph plus its dominant colour.
    #[inline]
    pub fn cell(&self, col: u16, row: u16) -> Cell {
        Self::cell_of(self.cell_dots(col, row), self.cell_prio(col, row))
    }

    /// Glyph and dominant colour of eight packed dots (row-major), each tagged with
    /// the layer it came from: only the dots of the topmost layer present vote, so
    /// a cell shows what is in front of everything else in it.
    pub(crate) fn cell_of(dots: [u32; 8], prio: [u8; 8]) -> Cell {
        let mut bits = 0u8;
        let (mut best, mut best_n, mut best_p) = (0u32, 0u8, 0u8);
        for (i, &v) in dots.iter().enumerate() {
            if v == 0 {
                continue;
            }
            bits |= BITS[i / 2][i % 2];
            let p = prio[i];
            if p < best_p {
                continue;
            }
            // At most eight dots: a quadratic count is cheaper than hashing.
            let n = dots[i..].iter().zip(&prio[i..]).filter(|&(&o, &q)| o == v && q == p).count() as u8;
            if p > best_p || n > best_n {
                (best, best_n, best_p) = (v, n, p);
            }
        }
        Cell { bits, color: (best != 0).then(|| Color::from_packed(best)) }
    }

    /// Iterates over all cells, row-major.
    pub fn cells(&self) -> impl Iterator<Item = Cell> + '_ {
        (0..self.rows).flat_map(move |r| (0..self.cols).map(move |c| self.cell(c, r)))
    }

    /// The canvas as the text protocol shows it on a terminal of `depth`: every dot
    /// first quantised to the colours the terminal has, then every set dot of a cell
    /// in the cell's one dominant colour, since a glyph can only have one. Printed
    /// characters are kept, their styles quantised the same way. Render this on a
    /// graphical terminal to see what users of plain ones will get, or compare it
    /// with the original to judge a colour scheme.
    ///
    /// ```
    /// use cobra::{Canvas, Color, Depth, Palette, Rgb};
    ///
    /// let mut canvas = Canvas::new(2, 1);
    /// canvas.set(0, 0, Rgb::hex(0xff0000));
    /// canvas.set(1, 0, Rgb::hex(0x0000ff));
    /// canvas.set(0, 1, Rgb::hex(0x0000ff));
    /// let text = canvas.fallback(Depth::Ansi16, &Palette::default());
    /// assert_eq!(text.get(0, 0), Some(Color::Indexed(4)), "blue outvoted red, and became ANSI blue");
    /// ```
    pub fn fallback(&self, depth: Depth, palette: &Palette) -> Canvas {
        let mut out = Canvas::new(self.cols, self.rows);
        let mut q = Quantizer::new(depth, palette);
        for row in 0..self.rows {
            for col in 0..self.cols {
                let cell = q.cell(self, col, row);
                let Some(color) = cell.color else { continue };
                for dy in 0..DOTS_Y {
                    for dx in 0..DOTS_X {
                        if cell.dot(dx, dy) {
                            out.set((col * DOTS_X + dx) as i32, (row * DOTS_Y + dy) as i32, color);
                        }
                    }
                }
            }
        }
        if self.has_text() {
            out.text = self.text.iter().map(|cell| TextCell { style: q.style(cell.style), ..*cell }).collect();
        }
        out
    }

    /// Copies `src` onto this canvas with its top-left dot at `(x, y)`: every set dot
    /// of `src`, and every printed character, which lands in the cell containing
    /// its top-left dot. Unset dots of `src` leave what is here alone, so a sprite
    /// drawn once is placed anywhere at the cost of copying it. Not transformed by
    /// [`with`](Self::with); dots that land off the canvas are dropped.
    pub fn blit(&mut self, src: &Canvas, x: i32, y: i32) {
        if let Some((x0, y0, x1, y1)) = src.dot_bounds() {
            let (dx0, dy0) = ((x0 + x).max(0), (y0 + y).max(0));
            let (dx1, dy1) = ((x1 + x).min(self.width()), (y1 + y).min(self.height()));
            if dx0 < dx1 && dy0 < dy1 {
                self.mark(dx0, dy0, dx1, dy1);
                let (sw, w) = (src.width() as usize, self.width() as usize);
                for dy in dy0..dy1 {
                    let from = &src.dots[(dy - y) as usize * sw + (dx0 - x) as usize..][..(dx1 - dx0) as usize];
                    let to = &mut self.dots[dy as usize * w + dx0 as usize..][..(dx1 - dx0) as usize];
                    for (t, &f) in to.iter_mut().zip(from) {
                        if f != 0 {
                            *t = f;
                        }
                    }
                }
            }
        }
        if src.has_text() {
            let (dcol, drow) = (x.div_euclid(DOTS_X as i32), y.div_euclid(DOTS_Y as i32));
            for row in 0..src.rows as i32 {
                for col in 0..src.cols as i32 {
                    if let Some(cell) = src.text_cell(col, row)
                        && !cell.is_continuation()
                    {
                        self.put(col + dcol, row + drow, cell);
                        if crate::text::char_width(cell.ch) == 2 {
                            self.put(col + dcol + 1, row + drow, TextCell { ch: TextCell::CONTINUATION, ..cell });
                        }
                    }
                }
            }
        }
    }

    /// Plain text, one line per row, no colour: braille glyphs for the dots, and the
    /// real character wherever one was [printed](Self::print). This is what a user gets
    /// when they copy the canvas out of a terminal running the text fallback.
    pub fn to_text(&self) -> String {
        let mut s = String::with_capacity((self.cols as usize * 3 + 1) * self.rows as usize);
        for r in 0..self.rows {
            for c in 0..self.cols {
                let text = self.text_at(c as i32, r as i32);
                match text.ch {
                    '\0' => s.push(self.cell(c, r).glyph()),
                    TextCell::CONTINUATION => {}
                    ch => s.push(ch),
                }
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
    fn a_higher_layer_outvotes_a_lower_one() {
        let (a, b) = (Color::Rgb(Rgb::hex(1)), Color::Rgb(Rgb::hex(2)));
        let dots = [a.packed(), b.packed(), b.packed(), b.packed(), 0, 0, 0, 0];
        assert_eq!(Canvas::cell_of(dots, [0; 8]).color, Some(b), "no layers: the count decides");
        assert_eq!(Canvas::cell_of(dots, [1, 0, 0, 0, 0, 0, 0, 0]).color, Some(a), "one dot in front wins");
        assert_eq!(Canvas::cell_of(dots, [1, 1, 0, 0, 0, 0, 0, 0]).color, Some(a), "a tie in front goes to the first");
        assert_eq!(Canvas::cell_of(dots, [1, 1, 1, 0, 0, 0, 0, 0]).color, Some(b), "the count decides within a layer");
        assert_eq!(Canvas::cell_of(dots, [1, 1, 1, 0, 0, 0, 0, 0]).bits, 0x1b);
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
    fn a_line_off_the_canvas_keeps_the_written_box_on_it() {
        // `line` marks the box its endpoints make; off-canvas endpoints must not
        // leave that box outside the dots, or `clear` and the bounds scan index past
        // them (a panic in debug, the wrong slice in release).
        let mut c = Canvas::new(10, 5);
        c.line(-5, 2, 3, 2, Rgb::hex(0xffffff));
        c.line(15, 30, 25, -9, Rgb::hex(0xffffff));
        assert_eq!(c.dirty, Some((0, 0, 20, 20)), "clamped to the canvas");
        assert!(c.dot_bounds().is_some_and(|(x0, y0, x1, y1)| x0 >= 0 && y0 >= 0 && x1 <= 20 && y1 <= 20));
        c.clear();
        assert!(c.cells().all(|cell| cell.bits == 0));
        assert_eq!(c.dirty, None);
        // A line entirely off the canvas writes nothing and marks nothing.
        c.line(-9, -9, -2, -3, Rgb::hex(0xffffff));
        assert_eq!(c.dirty, None);
        c.clear();
    }

    #[test]
    fn line_and_text() {
        let mut c = Canvas::new(2, 1);
        c.line(0, 0, 3, 0, Rgb::hex(0xffffff));
        assert_eq!(c.to_text(), "⠉⠉\n");
    }
}

#[cfg(test)]
mod copy_tests {
    use super::*;
    use crate::Rgb;

    #[test]
    fn the_fallback_gives_a_cell_one_colour() {
        let (red, blue) = (Rgb::hex(0xff0000), Rgb::hex(0x0000ff));
        let mut c = Canvas::new(2, 1);
        c.set(0, 0, red);
        c.set(1, 0, blue);
        c.set(0, 1, blue);
        c.set(2, 0, red);
        c.print(1, 0, "x", crate::TextStyle::new(red).on(blue));
        let f = c.fallback(Depth::TrueColor, &Palette::default());
        assert_eq!(f.get(0, 0), Some(Color::Rgb(blue)), "the dominant colour");
        assert_eq!(f.get(1, 0), Some(Color::Rgb(blue)));
        assert_eq!(f.get(1, 1), None, "unset dots stay unset");
        assert_eq!(f.get(2, 0), Some(Color::Rgb(red)));
        assert_eq!(f.cell(0, 0).bits, c.cell(0, 0).bits, "the glyphs are the same");
        assert_eq!(f.text_cell(1, 0).unwrap().ch, 'x');
        let q = c.fallback(Depth::Ansi16, &Palette::default());
        assert_eq!(q.get(0, 0), Some(Color::Indexed(4)), "and it became ANSI blue");
        assert_eq!(q.text_cell(1, 0).unwrap().style.fg, Some(Color::Indexed(9)), "text is quantised too");
        assert_eq!(c.fallback(Depth::Mono, &Palette::default()).get(0, 0), Some(Color::Foreground));
        assert!(Canvas::new(3, 3).fallback(Depth::Ansi256, &Palette::default()).cells().all(|c| c.bits == 0));
    }

    #[test]
    fn blit_copies_set_dots_and_text() {
        let mut sprite = Canvas::new(2, 1);
        sprite.fill_rect(0.0, 0.0, 4.0, 4.0, Rgb::hex(1));
        sprite.unset(1, 1);
        sprite.print(0, 0, "ab", Rgb::hex(2));
        let mut c = Canvas::new(4, 2);
        c.fill_rect(0.0, 0.0, 8.0, 8.0, Rgb::hex(3));
        c.blit(&sprite, 3, 2);
        assert_eq!(c.get(3, 2), Some(Color::Rgb(Rgb::hex(1))));
        assert_eq!(c.get(6, 5), Some(Color::Rgb(Rgb::hex(1))));
        assert_eq!(c.get(4, 3), Some(Color::Rgb(Rgb::hex(3))), "an unset dot of the sprite shows what was there");
        assert_eq!(c.get(7, 2), Some(Color::Rgb(Rgb::hex(3))));
        assert_eq!(c.text_cell(1, 0).map(|t| t.ch), Some('a'), "text lands in the cell of its top-left dot");
        assert_eq!(c.text_cell(2, 0).map(|t| t.ch), Some('b'));
        // Off the canvas is clipped, and a blit marks what it wrote so `clear` finds it.
        let mut edge = Canvas::new(2, 1);
        edge.blit(&sprite, -2, -1);
        assert_eq!(edge.get(1, 2), Some(Color::Rgb(Rgb::hex(1))));
        assert_eq!(edge.get(2, 0), None);
        edge.clear();
        assert!(edge.cells().all(|c| c.bits == 0) && !edge.has_text());
        let mut far = Canvas::new(2, 1);
        far.blit(&sprite, 50, 50);
        assert_eq!(far.dirty, None);
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
