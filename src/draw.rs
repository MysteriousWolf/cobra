//! Vector primitives: rectangles, polygons, ellipses, arcs, Bézier curves and splines,
//! filled or stroked, drawn straight into the dot buffer.
//!
//! Everything here reduces to horizontal spans ([`Canvas::span`]): a fill is one
//! clipped `slice::fill` per dot row rather than a bounds check per dot, and a stroke
//! is a chain of Bresenham lines (width ≤ 1) or convex quads plus round joins. Curves
//! are flattened on the fly, one segment at a time, so nothing here allocates except
//! [`Canvas::fill_polygon`], which sorts its edge crossings.
//!
//! Coordinates are in dots, `f32`, with dot `(x, y)` covering the unit square from
//! `(x, y)` to `(x + 1, y + 1)`. A fill includes every dot whose centre is inside the
//! shape, so `fill_rect(2.0, 3.0, 4.0, 2.0)` is exactly dots `2..6 × 3..5`.
//!
//! Every method takes a [`Paint`]: a colour, optionally dithered to a coverage, or
//! [`Paint::erase`] to unset dots instead (a cleared margin around a shape keeps it
//! readable on top of whatever is behind it).

use crate::canvas::bayer;
use crate::{Canvas, Color};

/// What a shape is filled with: a colour at some dither coverage, or nothing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paint {
    packed: u32,
    coverage: f32,
}

impl Paint {
    /// Solid `color`.
    pub fn new(color: impl Into<Color>) -> Self {
        Self { packed: color.into().packed(), coverage: 1.0 }
    }

    /// `color` on `coverage` (`0..=1`) of the dots, in the ordered Bayer pattern of
    /// [`Canvas::set_dithered`].
    pub fn dithered(color: impl Into<Color>, coverage: f32) -> Self {
        Self { packed: color.into().packed(), coverage }
    }

    /// Unsets dots instead of colouring them.
    pub const fn erase() -> Self {
        Self { packed: 0, coverage: 1.0 }
    }

    /// The colour, `None` for [`erase`](Self::erase).
    pub fn color(&self) -> Option<Color> {
        (self.packed != 0).then(|| Color::from_packed(self.packed))
    }

    /// Dither coverage, `1.0` when solid.
    pub fn coverage(&self) -> f32 {
        self.coverage
    }
}

impl From<Color> for Paint {
    fn from(c: Color) -> Self {
        Self::new(c)
    }
}

impl From<crate::Rgb> for Paint {
    fn from(c: crate::Rgb) -> Self {
        Self::new(c)
    }
}

impl From<u32> for Paint {
    fn from(c: u32) -> Self {
        Self::new(c)
    }
}

impl From<(u8, u8, u8)> for Paint {
    fn from(c: (u8, u8, u8)) -> Self {
        Self::new(c)
    }
}

/// A point in dot coordinates.
pub type Point = (f32, f32);

/// First dot whose centre is at or after `v`.
#[inline]
fn first(v: f32) -> i32 {
    (v - 0.5).ceil() as i32
}

#[inline]
fn dist(a: Point, b: Point) -> f32 {
    ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()
}

/// Segments to flatten a curve of roughly `len` dots into: about one per dot.
#[inline]
fn steps(len: f32) -> u32 {
    (len.ceil() as u32).clamp(1, 4096)
}

impl Canvas {
    /// Paints dots `x0..x1` of row `y`, clipped to the canvas. This is the primitive
    /// every fill and stroke ends up in.
    pub fn span(&mut self, y: i32, x0: i32, x1: i32, paint: Paint) {
        if y < 0 || y >= self.height() {
            return;
        }
        let (x0, x1) = (x0.max(0), x1.min(self.width()));
        if x0 >= x1 {
            return;
        }
        let start = y as usize * self.width() as usize + x0 as usize;
        let row = &mut self.dots[start..start + (x1 - x0) as usize];
        if paint.coverage >= 1.0 {
            row.fill(paint.packed);
        } else if paint.coverage > 0.0 {
            for (i, d) in row.iter_mut().enumerate() {
                if paint.coverage > bayer(x0 + i as i32, y) {
                    *d = paint.packed;
                }
            }
        }
    }

    /// Fills the axis-aligned box from `(x, y)` of size `w × h`.
    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, paint: impl Into<Paint>) {
        let paint = paint.into();
        let (x0, x1) = (first(x), first(x + w));
        for row in first(y)..first(y + h) {
            self.span(row, x0, x1, paint);
        }
    }

    /// Outlines the box from `(x, y)` of size `w × h` with a border `width` dots
    /// thick, drawn inside the box so it covers the same dots as
    /// [`fill_rect`](Self::fill_rect) would.
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, width: f32, paint: impl Into<Paint>) {
        let paint = paint.into();
        let t = width.max(1.0).min(w / 2.0).min(h / 2.0);
        self.fill_rect(x, y, w, t, paint);
        self.fill_rect(x, y + h - t, w, t, paint);
        self.fill_rect(x, y + t, t, h - 2.0 * t, paint);
        self.fill_rect(x + w - t, y + t, t, h - 2.0 * t, paint);
    }

    /// Fills the ellipse centred on `(cx, cy)` with radii `rx`, `ry`.
    pub fn fill_ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, paint: impl Into<Paint>) {
        let paint = paint.into();
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        for y in first(cy - ry)..first(cy + ry) {
            let ey = (y as f32 + 0.5 - cy) / ry;
            let half = rx * (1.0 - ey * ey).max(0.0).sqrt();
            self.span(y, first(cx - half), (cx + half - 0.5).floor() as i32 + 1, paint);
        }
    }

    /// Strokes the ellipse centred on `(cx, cy)` with radii `rx`, `ry`.
    pub fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, width: f32, paint: impl Into<Paint>) {
        self.arc(cx, cy, rx, ry, 0.0, std::f32::consts::TAU, width, paint);
    }

    /// Strokes the elliptical arc from angle `a0` to `a1` (radians, clockwise on
    /// screen since `y` grows downwards).
    #[allow(clippy::too_many_arguments)]
    pub fn arc(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, a0: f32, a1: f32, width: f32, paint: impl Into<Paint>) {
        let n = steps((a1 - a0).abs() * rx.max(ry));
        let pts = (0..=n).map(move |i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            (cx + rx * a.cos(), cy + ry * a.sin())
        });
        self.stroke(pts, false, width, paint.into());
    }

    /// Strokes straight segments through `pts`.
    pub fn polyline(&mut self, pts: &[Point], width: f32, paint: impl Into<Paint>) {
        self.stroke(pts.iter().copied(), false, width, paint.into());
    }

    /// Strokes the closed outline through `pts`.
    pub fn polygon(&mut self, pts: &[Point], width: f32, paint: impl Into<Paint>) {
        self.stroke(pts.iter().copied(), true, width, paint.into());
    }

    /// Fills the polygon with vertices `pts` (any shape, even-odd rule).
    pub fn fill_polygon(&mut self, pts: &[Point], paint: impl Into<Paint>) {
        let paint = paint.into();
        if pts.len() < 3 {
            return;
        }
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for p in pts {
            lo = lo.min(p.1);
            hi = hi.max(p.1);
        }
        let mut xs: Vec<f32> = Vec::with_capacity(pts.len());
        for y in first(lo).max(0)..first(hi).min(self.height()) {
            let sy = y as f32 + 0.5;
            xs.clear();
            for (i, &a) in pts.iter().enumerate() {
                let b = pts[(i + 1) % pts.len()];
                if (a.1 <= sy) != (b.1 <= sy) {
                    xs.push(a.0 + (sy - a.1) * (b.0 - a.0) / (b.1 - a.1));
                }
            }
            xs.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            for pair in xs.chunks_exact(2) {
                self.span(y, first(pair[0]), first(pair[1]), paint);
            }
        }
    }

    /// Strokes a Bézier curve with control points `ctrl` (three for quadratic, four
    /// for cubic, up to sixteen).
    pub fn bezier(&mut self, ctrl: &[Point], width: f32, paint: impl Into<Paint>) {
        let n = ctrl.len().min(16);
        if n < 2 {
            return;
        }
        let len: f32 = ctrl.windows(2).map(|w| dist(w[0], w[1])).sum();
        let m = steps(len);
        let pts = (0..=m).map(move |i| {
            // De Casteljau in place; degree is tiny so the quadratic cost is nothing.
            let t = i as f32 / m as f32;
            let mut p = [(0.0f32, 0.0f32); 16];
            p[..n].copy_from_slice(&ctrl[..n]);
            for k in (1..n).rev() {
                for j in 0..k {
                    p[j] = (p[j].0 + (p[j + 1].0 - p[j].0) * t, p[j].1 + (p[j + 1].1 - p[j].1) * t);
                }
            }
            p[0]
        });
        self.stroke(pts, false, width, paint.into());
    }

    /// Strokes a smooth curve through every point of `pts` (Catmull–Rom), open or
    /// closed into a loop.
    pub fn spline(&mut self, pts: &[Point], closed: bool, width: f32, paint: impl Into<Paint>) {
        let n = pts.len();
        if n < 2 {
            return;
        }
        let at = |i: isize| -> Point {
            if closed {
                pts[i.rem_euclid(n as isize) as usize]
            } else {
                pts[i.clamp(0, n as isize - 1) as usize]
            }
        };
        let segments = if closed { n } else { n - 1 };
        let pts = (0..segments).flat_map(move |s| {
            let (p0, p1, p2, p3) = (at(s as isize - 1), at(s as isize), at(s as isize + 1), at(s as isize + 2));
            let m = steps(dist(p1, p2));
            // The first point of a segment is the last of the previous one.
            (if s == 0 { 0 } else { 1 }..=m).map(move |i| {
                let t = i as f32 / m as f32;
                let (t2, t3) = (t * t, t * t * t);
                let f = |a: f32, b: f32, c: f32, d: f32| {
                    0.5 * (2.0 * b
                        + (c - a) * t
                        + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                        + (3.0 * b - a - 3.0 * c + d) * t3)
                };
                (f(p0.0, p1.0, p2.0, p3.0), f(p0.1, p1.1, p2.1, p3.1))
            })
        });
        self.stroke(pts, closed, width, paint.into());
    }

    /// Strokes the path through `pts`: Bresenham lines for `width ≤ 1`, otherwise a
    /// quad per segment with round joins and caps.
    fn stroke(&mut self, pts: impl Iterator<Item = Point>, closed: bool, width: f32, paint: Paint) {
        let mut first_pt = None;
        let mut prev: Option<Point> = None;
        for p in pts {
            if let Some(q) = prev {
                self.segment(q, p, width, paint);
            } else {
                first_pt = Some(p);
                if width > 1.0 {
                    self.fill_ellipse(p.0, p.1, width / 2.0, width / 2.0, paint);
                }
            }
            prev = Some(p);
        }
        if let (true, Some(a), Some(b)) = (closed, prev, first_pt) {
            self.segment(a, b, width, paint);
        }
    }

    fn segment(&mut self, a: Point, b: Point, width: f32, paint: Paint) {
        if width <= 1.0 {
            self.line_paint(a.0.floor() as i32, a.1.floor() as i32, b.0.floor() as i32, b.1.floor() as i32, paint);
            return;
        }
        let hw = width / 2.0;
        let len = dist(a, b);
        if len > 0.0 {
            let (nx, ny) = (-(b.1 - a.1) / len * hw, (b.0 - a.0) / len * hw);
            let quad = [(a.0 + nx, a.1 + ny), (b.0 + nx, b.1 + ny), (b.0 - nx, b.1 - ny), (a.0 - nx, a.1 - ny)];
            self.fill_convex(&quad, paint);
        }
        self.fill_ellipse(b.0, b.1, hw, hw, paint);
    }

    /// Scanline fill for a convex polygon: no sorting, no allocation.
    fn fill_convex(&mut self, pts: &[Point], paint: Paint) {
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for p in pts {
            lo = lo.min(p.1);
            hi = hi.max(p.1);
        }
        for y in first(lo).max(0)..first(hi).min(self.height()) {
            let sy = y as f32 + 0.5;
            let (mut xmin, mut xmax) = (f32::MAX, f32::MIN);
            for (i, &a) in pts.iter().enumerate() {
                let b = pts[(i + 1) % pts.len()];
                if (a.1 <= sy) != (b.1 <= sy) {
                    let x = a.0 + (sy - a.1) * (b.0 - a.0) / (b.1 - a.1);
                    xmin = xmin.min(x);
                    xmax = xmax.max(x);
                }
            }
            if xmin <= xmax {
                self.span(y, first(xmin), first(xmax), paint);
            }
        }
    }

    /// Bresenham line with a [`Paint`] (so it can dither or erase).
    pub(crate) fn line_paint(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, paint: Paint) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = ((x1 - x0).signum(), (y1 - y0).signum());
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        loop {
            self.span(y, x, x + 1, paint);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rgb;

    const C: Rgb = Rgb::hex(0xffffff);

    fn count(c: &Canvas) -> usize {
        c.cells().map(|cell| cell.bits.count_ones() as usize).sum()
    }

    #[test]
    fn rect_is_exact() {
        let mut c = Canvas::new(4, 2);
        c.fill_rect(2.0, 3.0, 4.0, 2.0, C);
        assert_eq!(count(&c), 8);
        assert!(c.get(2, 3).is_some() && c.get(5, 4).is_some());
        assert!(c.get(1, 3).is_none() && c.get(6, 4).is_none() && c.get(2, 5).is_none());
        let mut o = Canvas::new(4, 2);
        o.rect(2.0, 3.0, 4.0, 2.0, 1.0, C);
        assert_eq!(o, c, "a 1-wide border of a 4×2 box is the whole box");
        let mut o = Canvas::new(4, 2);
        o.rect(0.0, 0.0, 8.0, 8.0, 1.0, C);
        assert_eq!(count(&o), 28);
        assert!(o.get(1, 1).is_none());
    }

    #[test]
    fn spans_clip() {
        let mut c = Canvas::new(2, 1);
        c.span(0, -5, 100, Paint::new(C));
        c.span(-1, 0, 4, Paint::new(C));
        c.span(9, 0, 4, Paint::new(C));
        c.span(1, 3, 1, Paint::new(C));
        assert_eq!(count(&c), 4);
        c.fill_rect(-10.0, -10.0, 100.0, 100.0, Paint::erase());
        assert_eq!(count(&c), 0);
    }

    #[test]
    fn dithered_paint_matches_set_dithered() {
        let (mut a, mut b) = (Canvas::new(2, 1), Canvas::new(2, 1));
        a.fill_rect(0.0, 0.0, 4.0, 4.0, Paint::dithered(C, 0.5));
        for y in 0..4 {
            for x in 0..4 {
                b.set_dithered(x, y, C, 0.5);
            }
        }
        assert_eq!(a, b);
        assert_eq!(count(&a), 8);
    }

    #[test]
    fn disc_area() {
        let mut c = Canvas::new(10, 5);
        c.disc(10.0, 10.0, 6.0, C);
        let n = count(&c) as f32;
        assert!((n - std::f32::consts::PI * 36.0).abs() < 8.0, "{n}");
        // Symmetric about the centre.
        for y in 4..16 {
            for x in 4..16 {
                assert_eq!(c.get(x, y).is_some(), c.get(19 - x, 19 - y).is_some(), "{x},{y}");
            }
        }
    }

    #[test]
    fn polygon_fill_and_outline() {
        let tri = [(0.0, 0.0), (8.0, 0.0), (0.0, 8.0)];
        let mut c = Canvas::new(4, 2);
        c.fill_polygon(&tri, C);
        assert!((count(&c) as i32 - 32).abs() <= 4);
        assert!(c.get(0, 0).is_some() && c.get(7, 7).is_none());
        let mut o = Canvas::new(4, 2);
        o.polygon(&tri, 1.0, C);
        assert!(o.get(0, 0).is_some() && o.get(8, 0).is_none() && o.get(3, 3).is_none());
        // Concave, even-odd: a bow-tie leaves the centre column crossing.
        let mut b = Canvas::new(4, 2);
        b.fill_polygon(&[(0.0, 0.0), (8.0, 8.0), (8.0, 0.0), (0.0, 8.0)], C);
        assert!(b.get(1, 4).is_some() && b.get(4, 1).is_none());
    }

    #[test]
    fn thick_strokes_have_width() {
        let mut c = Canvas::new(10, 3);
        c.polyline(&[(2.0, 6.0), (18.0, 6.0)], 4.0, C);
        assert!(c.get(10, 4).is_some() && c.get(10, 7).is_some() && c.get(10, 8).is_none());
        assert!(c.get(0, 6).is_some(), "round cap extends past the end point");
        let mut e = Canvas::new(10, 3);
        e.ellipse(10.0, 6.0, 8.0, 4.0, 1.0, C);
        assert!(e.get(10, 6).is_none() && count(&e) > 20);
    }

    #[test]
    fn curves_pass_through_end_points() {
        let mut c = Canvas::new(10, 5);
        c.bezier(&[(0.0, 0.0), (10.0, 19.0), (19.0, 0.0)], 1.0, C);
        assert!(c.get(0, 0).is_some() && c.get(19, 0).is_some());
        let mut s = Canvas::new(10, 5);
        s.spline(&[(0.0, 10.0), (6.0, 2.0), (12.0, 18.0), (19.0, 10.0)], false, 1.0, C);
        assert!(s.get(0, 10).is_some() && s.get(6, 2).is_some() && s.get(12, 18).is_some() && s.get(19, 10).is_some());
        let mut l = Canvas::new(10, 5);
        l.spline(&[(4.0, 4.0), (16.0, 4.0), (16.0, 16.0), (4.0, 16.0)], true, 1.0, C);
        assert!(l.get(4, 4).is_some() && l.get(16, 16).is_some());
        assert!(count(&l) > 40);
    }
}
