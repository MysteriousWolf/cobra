//! Shapes as things: a [`Mask`] is a set of dots with no colour, drawn with the same
//! primitives as a canvas, combined with other masks, moved, and then used to paint,
//! clip, cut or run effects. The [`Silhouette`] trait is what every mask-taking
//! operation accepts, so a plain [`Canvas`] serves as a mask too.

use crate::{Canvas, DOTS_X, DOTS_Y, Transform};

/// Something that covers some dots and not others: a [`Mask`], or a [`Canvas`] (its
/// set dots and printed cells). Everything that paints through, clips to, cuts out
/// or runs effects around a shape takes `&impl Silhouette`.
pub trait Silhouette {
    /// Whether dot `(x, y)` is covered. Out of range is never covered.
    fn covers(&self, x: i32, y: i32) -> bool;

    /// The box `(x0, y0, x1, y1)` (exclusive on the far side) around every covered
    /// dot, `None` when nothing is covered. It may be larger than the tightest box,
    /// never smaller.
    fn extent(&self) -> Option<(i32, i32, i32, i32)>;
}

impl<S: Silhouette + ?Sized> Silhouette for &S {
    fn covers(&self, x: i32, y: i32) -> bool {
        (**self).covers(x, y)
    }

    fn extent(&self) -> Option<(i32, i32, i32, i32)> {
        (**self).extent()
    }
}

impl Silhouette for Canvas {
    #[inline]
    fn covers(&self, x: i32, y: i32) -> bool {
        self.get(x, y).is_some()
            || (self.has_text() && self.text_cell(x.div_euclid(DOTS_X as i32), y.div_euclid(DOTS_Y as i32)).is_some())
    }

    fn extent(&self) -> Option<(i32, i32, i32, i32)> {
        let (w, h) = (self.width(), self.height());
        let (mut x0, mut y0, mut x1, mut y1) = self.dot_bounds().unwrap_or((w, h, 0, 0));
        if self.has_text() {
            for row in 0..self.rows() as i32 {
                for col in 0..self.cols() as i32 {
                    if self.text_cell(col, row).is_some() {
                        let (cx, cy) = (col * DOTS_X as i32, row * DOTS_Y as i32);
                        (x0, x1) = (x0.min(cx), x1.max(cx + DOTS_X as i32));
                        (y0, y1) = (y0.min(cy), y1.max(cy + DOTS_Y as i32));
                    }
                }
            }
        }
        (x0 < x1 && y0 < y1).then_some((x0, y0, x1, y1))
    }
}

/// A set of dots: a shape without a colour, one bit per dot.
///
/// A mask is sized like a canvas, in cells, and drawn like one: [`draw`](Self::draw)
/// runs any drawing code and keeps the dots it set, so a silhouette is the same
/// `fill_ellipse` and `fill_polygon` calls as the picture, without a colour. Masks
/// combine ([`union`](Self::union), [`subtract`](Self::subtract),
/// [`intersect`](Self::intersect), [`boundary_with`](Self::boundary_with)), move
/// ([`transform`](Self::transform)), and are kept between frames: a part built once
/// is placed anew each frame instead of being redrawn.
///
/// What a mask is *for* is the canvas: [`Canvas::stencil`] paints through it,
/// [`Canvas::clip`] and [`Canvas::cut`] keep or remove what it covers,
/// [`Canvas::clipped`] draws through it, and [`Canvas::effects`] runs outlines,
/// glows, rims and shadows around it.
///
/// ```
/// use cobra::{Canvas, Effect, Mask, Paint, Rgb, Transform};
///
/// let mut head = Mask::new(20, 5);
/// head.draw(|c| c.fill_ellipse(0.0, 0.0, 5.0, 4.0, Rgb::hex(0)));   // about its own origin
/// let mut eye = Mask::new(20, 5);
/// eye.draw(|c| c.disc(2.0, -1.0, 1.0, Rgb::hex(0)));
/// head.subtract(&eye);
/// head.transform(&Transform::at(20.0, 10.0));                        // placed
///
/// let mut canvas = Canvas::new(20, 5);
/// canvas.stencil(&head, Paint::edge(Rgb::hex(0xffe08a), Rgb::hex(0xffa657), 2.0));
/// canvas.effects(&head, &[Effect::outline(1.0).paint(Rgb::hex(0x3d2200))]);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mask {
    cols: u16,
    rows: u16,
    /// Words per row.
    stride: usize,
    bits: Vec<u64>,
    /// The box around the set bits, `None` when empty; a bound, kept tight by
    /// everything that only adds or moves, loose after a subtraction.
    bounds: Option<(i32, i32, i32, i32)>,
}

impl Mask {
    /// An empty mask of `cols × rows` cells.
    pub fn new(cols: u16, rows: u16) -> Self {
        let stride = (cols as usize * DOTS_X as usize).div_ceil(64);
        Self { cols, rows, stride, bits: vec![0; stride * rows as usize * DOTS_Y as usize], bounds: None }
    }

    /// The silhouette of `shape` as a mask of the same size: a canvas's set dots and
    /// printed cells.
    pub fn of(shape: &Canvas) -> Self {
        let mut m = Self::new(shape.cols(), shape.rows());
        if let Some((x0, y0, x1, y1)) = shape.extent() {
            for y in y0..y1 {
                for x in x0..x1 {
                    if shape.covers(x, y) {
                        m.set(x, y);
                    }
                }
            }
        }
        m
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

    /// Width in dots.
    #[inline]
    pub fn width(&self) -> i32 {
        self.cols as i32 * DOTS_X as i32
    }

    /// Height in dots.
    #[inline]
    pub fn height(&self) -> i32 {
        self.rows as i32 * DOTS_Y as i32
    }

    #[inline]
    fn slot(&self, x: i32, y: i32) -> Option<(usize, u64)> {
        (x >= 0 && y >= 0 && x < self.width() && y < self.height())
            .then(|| (y as usize * self.stride + (x as usize >> 6), 1u64 << (x & 63)))
    }

    /// Whether dot `(x, y)` is set.
    #[inline]
    pub fn contains(&self, x: i32, y: i32) -> bool {
        self.slot(x, y).is_some_and(|(i, bit)| self.bits[i] & bit != 0)
    }

    /// Sets dot `(x, y)`; out of range is ignored.
    #[inline]
    pub fn set(&mut self, x: i32, y: i32) {
        if let Some((i, bit)) = self.slot(x, y) {
            self.bits[i] |= bit;
            self.grow(x, y, x + 1, y + 1);
        }
    }

    /// Unsets dot `(x, y)`.
    #[inline]
    pub fn unset(&mut self, x: i32, y: i32) {
        if let Some((i, bit)) = self.slot(x, y) {
            self.bits[i] &= !bit;
        }
    }

    /// Unsets everything. Keeps the allocation.
    pub fn clear(&mut self) {
        self.bits.fill(0);
        self.bounds = None;
    }

    /// Whether no dot is set.
    pub fn is_empty(&self) -> bool {
        self.bounds.is_none() || self.bits.iter().all(|&w| w == 0)
    }

    /// How many dots are set.
    pub fn len(&self) -> usize {
        self.bits.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// The box `(x0, y0, x1, y1)` around the set dots, exact; `None` when empty.
    pub fn bounds(&self) -> Option<(i32, i32, i32, i32)> {
        let (bx0, by0, bx1, by1) = self.bounds?;
        let (mut x0, mut y0, mut x1, mut y1) = (bx1, by1, bx0, by0);
        for y in by0..by1 {
            let row = &self.bits[y as usize * self.stride..][..self.stride];
            let Some(first) = row.iter().position(|&w| w != 0) else { continue };
            let last = row.iter().rposition(|&w| w != 0).unwrap_or(first);
            let lo = first as i32 * 64 + row[first].trailing_zeros() as i32;
            let hi = last as i32 * 64 + 64 - row[last].leading_zeros() as i32;
            (x0, x1) = (x0.min(lo), x1.max(hi));
            (y0, y1) = (y0.min(y), y + 1);
        }
        (x0 < x1).then_some((x0, y0, x1, y1))
    }

    #[inline]
    fn grow(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        self.bounds = Some(match self.bounds {
            Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
            None => (x0, y0, x1, y1),
        });
    }

    /// The set dots, row-major.
    pub fn dots(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        let (x0, y0, x1, y1) = self.bounds.unwrap_or((0, 0, 0, 0));
        (y0..y1).flat_map(move |y| (x0..x1).map(move |x| (x, y))).filter(move |&(x, y)| self.contains(x, y))
    }

    /// Runs `f` with a scratch canvas of the mask's size and adds every dot it set:
    /// drawing a shape *is* drawing it on a canvas, with any primitive and any paint
    /// (a dithered paint sets only the dots its dither lands on, and
    /// [`Paint::erase`](crate::Paint::erase) unsets, within this call). The scratch
    /// is kept per thread, so this allocates once.
    pub fn draw(&mut self, f: impl FnOnce(&mut Canvas)) {
        crate::draw::with_scratch(self.cols, self.rows, |c| {
            f(c);
            if let Some((x0, y0, x1, y1)) = c.dot_bounds() {
                for y in y0..y1 {
                    for x in x0..x1 {
                        if c.get(x, y).is_some() {
                            self.set(x, y);
                        }
                    }
                }
            }
        });
    }

    /// Runs `f` as [`draw`](Self::draw) does and removes every dot it set.
    pub fn erase(&mut self, f: impl FnOnce(&mut Canvas)) {
        let mut cut = Mask::new(self.cols, self.rows);
        cut.draw(f);
        self.subtract(&cut);
    }

    /// Combines with `other` word by word; `bounds` says how the box changes.
    fn combine(&mut self, other: &Mask, op: impl Fn(u64, u64) -> u64) {
        if (self.cols, self.rows) == (other.cols, other.rows) {
            for (a, &b) in self.bits.iter_mut().zip(&other.bits) {
                *a = op(*a, b);
            }
        } else {
            // Different sizes: dot by dot, with the other mask's edge as its edge.
            for y in 0..self.height() {
                for x in 0..self.width() {
                    let (i, bit) = self.slot(x, y).unwrap();
                    let b = if other.contains(x, y) { bit } else { 0 };
                    self.bits[i] = (self.bits[i] & !bit) | (op(self.bits[i] & bit, b) & bit);
                }
            }
        }
    }

    /// Adds every dot of `other`.
    pub fn union(&mut self, other: &Mask) {
        self.combine(other, |a, b| a | b);
        if let Some((x0, y0, x1, y1)) = other.bounds {
            self.grow(x0, y0, x1, y1);
        }
    }

    /// Removes every dot of `other`.
    pub fn subtract(&mut self, other: &Mask) {
        self.combine(other, |a, b| a & !b);
    }

    /// Keeps only the dots also in `other`.
    pub fn intersect(&mut self, other: &Mask) {
        self.combine(other, |a, b| a & b);
        self.bounds = self.bounds();
    }

    /// The dots of this mask that touch `other` (eight neighbours): the seam where
    /// two shapes meet, such as a collar between a head and a body.
    pub fn boundary_with(&self, other: &Mask) -> Mask {
        let mut out = Mask::new(self.cols, self.rows);
        for (x, y) in self.dots() {
            let touches = (-1..=1).any(|dy| (-1..=1).any(|dx| (dx, dy) != (0, 0) && other.contains(x + dx, y + dy)));
            if touches {
                out.set(x, y);
            }
        }
        out
    }

    /// Moves the mask by whole dots. Dots moved off the mask are lost.
    pub fn translate(&mut self, dx: i32, dy: i32) {
        self.transform(&Transform::at(dx as f32, dy as f32));
    }

    /// Mirrors left-to-right about the column `x` dots from the left edge (`x` is
    /// the mirror line, so a mask spanning `2..8` flipped about `5.0` spans `2..8`).
    pub fn flip_x(&mut self, x: f32) {
        self.transform(&Transform::at(x, 0.0).flip_x().translate(-x, 0.0));
    }

    /// Mirrors top-to-bottom about the row `y` dots from the top.
    pub fn flip_y(&mut self, y: f32) {
        self.transform(&Transform::at(0.0, y).flip_y().translate(0.0, -y));
    }

    /// Applies `t` to every dot: an integer move or flip is exact, anything else
    /// (a rotation, a scale) resamples with the nearest dot, which on a dot matrix
    /// is the honest answer. Dots that land off the mask are lost.
    pub fn transform(&mut self, t: &Transform) {
        let Some((x0, y0, x1, y1)) = self.bounds() else { return };
        let mut out = Mask::new(self.cols, self.rows);
        let exact = t.is_axis_aligned()
            && t.m[0].abs() == 1.0
            && t.m[3].abs() == 1.0
            && t.m[4].fract() == 0.0
            && t.m[5].fract() == 0.0;
        if exact {
            // Each dot goes to exactly one dot: map forwards.
            for (x, y) in self.dots() {
                let (nx, ny) = t.apply((x as f32 + 0.5, y as f32 + 0.5));
                out.set(nx.floor() as i32, ny.floor() as i32);
            }
        } else if let Some(inv) = t.inverse() {
            // Map every dot of the destination box back and see if it lands on a
            // set dot, so nothing tears.
            let corners = [(x0, y0), (x1, y0), (x0, y1), (x1, y1)].map(|(x, y)| t.apply((x as f32, y as f32)));
            let (mut dx0, mut dy0, mut dx1, mut dy1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for (x, y) in corners {
                (dx0, dy0, dx1, dy1) = (dx0.min(x), dy0.min(y), dx1.max(x), dy1.max(y));
            }
            let (dx0, dy0) = ((dx0.floor() as i32).max(0), (dy0.floor() as i32).max(0));
            let (dx1, dy1) = ((dx1.ceil() as i32).min(self.width()), (dy1.ceil() as i32).min(self.height()));
            for y in dy0..dy1 {
                for x in dx0..dx1 {
                    let (sx, sy) = inv.apply((x as f32 + 0.5, y as f32 + 0.5));
                    if self.contains(sx.floor() as i32, sy.floor() as i32) {
                        out.set(x, y);
                    }
                }
            }
        }
        *self = out;
    }
}

impl Silhouette for Mask {
    #[inline]
    fn covers(&self, x: i32, y: i32) -> bool {
        self.contains(x, y)
    }

    fn extent(&self) -> Option<(i32, i32, i32, i32)> {
        self.bounds
    }
}

impl From<&Canvas> for Mask {
    fn from(canvas: &Canvas) -> Self {
        Self::of(canvas)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rgb;

    #[test]
    fn drawing_booleans_and_bounds() {
        let mut a = Mask::new(4, 2);
        a.draw(|c| c.fill_rect(1.0, 1.0, 4.0, 4.0, Rgb::hex(1)));
        assert_eq!(a.len(), 16);
        assert_eq!(a.bounds(), Some((1, 1, 5, 5)));
        assert!(a.contains(1, 1) && !a.contains(5, 5) && !a.contains(-1, 0));
        let mut b = Mask::new(4, 2);
        b.draw(|c| c.fill_rect(3.0, 3.0, 4.0, 4.0, Rgb::hex(1)));
        let mut u = a.clone();
        u.union(&b);
        assert_eq!(u.len(), 16 + 16 - 4);
        let mut i = a.clone();
        i.intersect(&b);
        assert_eq!((i.len(), i.bounds()), (4, Some((3, 3, 5, 5))));
        let mut s = a.clone();
        s.subtract(&b);
        assert_eq!(s.len(), 12);
        assert!(!s.contains(4, 4) && s.contains(1, 1));
        let seam = a.boundary_with(&b);
        assert!(seam.contains(2, 2) && seam.contains(4, 4) && !seam.contains(1, 1));
        s.erase(|c| c.fill_rect(0.0, 0.0, 8.0, 8.0, Rgb::hex(1)));
        assert!(s.is_empty() && s.bounds().is_none());
        a.clear();
        assert!(a.is_empty() && a.dots().next().is_none());
    }

    #[test]
    fn draw_sees_paints_and_erase() {
        let mut m = Mask::new(2, 1);
        m.draw(|c| {
            c.fill_rect(0.0, 0.0, 4.0, 4.0, crate::Paint::dithered(Rgb::hex(1), 0.5));
        });
        assert_eq!(m.len(), 8);
        m.draw(|c| {
            c.fill_rect(0.0, 0.0, 4.0, 4.0, Rgb::hex(1));
            c.fill_rect(0.0, 0.0, 2.0, 4.0, crate::Paint::erase());
        });
        assert_eq!(m.len(), 8 + 4, "erase within a draw only affects that draw");
    }

    #[test]
    fn transforms_move_flip_and_rotate() {
        let mut m = Mask::new(6, 3);
        m.draw(|c| c.fill_rect(2.0, 2.0, 4.0, 2.0, Rgb::hex(1)));
        m.translate(3, 1);
        assert_eq!(m.bounds(), Some((5, 3, 9, 5)));
        m.flip_x(6.0);
        assert_eq!(m.bounds(), Some((3, 3, 7, 5)), "mirrored about x = 6");
        m.flip_y(4.0);
        assert_eq!(m.bounds(), Some((3, 3, 7, 5)));
        assert_eq!(m.len(), 8, "exact moves keep every dot");
        m.transform(&Transform::IDENTITY.rotate_about(std::f32::consts::FRAC_PI_2, (5.0, 4.0)));
        assert_eq!(m.bounds(), Some((4, 2, 6, 6)), "a quarter turn: 4×2 becomes 2×4");
        assert_eq!(m.len(), 8);
        m.transform(&Transform::at(-100.0, 0.0));
        assert!(m.is_empty(), "off the mask is gone");
    }

    #[test]
    fn a_canvas_is_a_silhouette_and_becomes_a_mask() {
        let mut c = Canvas::new(4, 2);
        c.disc(2.0, 2.0, 1.5, Rgb::hex(1));
        c.print(3, 1, "x", Rgb::hex(1));
        assert!(c.covers(2, 2) && c.covers(6, 4) && !c.covers(0, 7));
        assert_eq!(c.extent(), Some((1, 1, 8, 8)));
        let m = Mask::of(&c);
        assert_eq!(m.len(), c.cells().map(|cell| cell.bits.count_ones() as usize).sum::<usize>() + 8);
        assert_eq!(m.extent(), m.bounds());
        let mut odd = Mask::new(2, 1);
        odd.draw(|c| c.fill_rect(0.0, 0.0, 4.0, 4.0, Rgb::hex(1)));
        let mut big = Mask::new(4, 2);
        big.draw(|c| c.fill_rect(0.0, 0.0, 8.0, 8.0, Rgb::hex(1)));
        big.intersect(&odd);
        assert_eq!(big.len(), 16, "masks of different sizes combine over the overlap");
    }
}
