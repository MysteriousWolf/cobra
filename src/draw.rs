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

/// An axis-aligned box in dot coordinates: what a [`Bubble`](crate::Bubble) occupies
/// and what a keep-out zone is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width in dots.
    pub w: f32,
    /// Height in dots.
    pub h: f32,
}

impl Rect {
    /// A box from its corner and size.
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// A box around `center`.
    pub fn around(center: Point, w: f32, h: f32) -> Self {
        Self::new(center.0 - w / 2.0, center.1 - h / 2.0, w, h)
    }

    /// Right edge (`x + w`).
    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    /// Bottom edge (`y + h`).
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    /// Centre point.
    pub fn center(&self) -> Point {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    /// Whether `p` is inside.
    pub fn contains(&self, p: Point) -> bool {
        p.0 >= self.x && p.0 < self.right() && p.1 >= self.y && p.1 < self.bottom()
    }

    /// A copy shrunk by `d` on every side (grown when `d` is negative).
    pub fn inset(&self, d: f32) -> Rect {
        Rect::new(self.x + d, self.y + d, (self.w - 2.0 * d).max(0.0), (self.h - 2.0 * d).max(0.0))
    }

    /// A copy moved by `(dx, dy)`.
    pub fn offset(&self, dx: f32, dy: f32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }

    /// Area the two boxes share, `0.0` when they do not touch.
    pub fn overlap(&self, other: &Rect) -> f32 {
        let w = (self.right().min(other.right()) - self.x.max(other.x)).max(0.0);
        let h = (self.bottom().min(other.bottom()) - self.y.max(other.y)).max(0.0);
        w * h
    }
}

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

/// Vertices a generated shape may have, so they fit a stack buffer.
const MAX_POINTS: usize = 128;

/// Horizontal extent of a rounded box at scan line `sy`, `None` outside it.
fn round_rect_span(sy: f32, x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<(f32, f32)> {
    if sy < y || sy >= y + h || w <= 0.0 || h <= 0.0 {
        return None;
    }
    let r = r.clamp(0.0, w.min(h) / 2.0);
    let dy = (y + r - sy).max(sy - (y + h - r)).max(0.0);
    let inset = if dy > 0.0 { r - (r * r - dy * dy).max(0.0).sqrt() } else { 0.0 };
    Some((x + inset, x + w - inset))
}

/// Vertices of a regular polygon, alternating `r1` and `r2` so one call also makes a
/// star. Returns the buffer and how much of it is used.
fn ngon_points(cx: f32, cy: f32, r1: f32, r2: f32, n: u32, rot: f32) -> ([Point; MAX_POINTS], usize) {
    let n = n.clamp(3, MAX_POINTS as u32) as usize;
    let mut pts = [(0.0f32, 0.0f32); MAX_POINTS];
    for (i, p) in pts[..n].iter_mut().enumerate() {
        let a = rot + std::f32::consts::TAU * i as f32 / n as f32;
        let r = if i % 2 == 0 { r1 } else { r2 };
        *p = (cx + r * a.cos(), cy + r * a.sin());
    }
    (pts, n)
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
            for pair in xs.as_chunks::<2>().0 {
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
            if closed { pts[i.rem_euclid(n as isize) as usize] } else { pts[i.clamp(0, n as isize - 1) as usize] }
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

    /// Fills the box from `(x, y)` of size `w × h` with corners rounded to radius `r`.
    pub fn fill_round_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, paint: impl Into<Paint>) {
        let paint = paint.into();
        for row in first(y)..first(y + h) {
            if let Some((a, b)) = round_rect_span(row as f32 + 0.5, x, y, w, h, r) {
                self.span(row, first(a), first(b), paint);
            }
        }
    }

    /// Outlines a rounded box with a border `width` dots thick, drawn inside it.
    #[allow(clippy::too_many_arguments)]
    pub fn round_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, width: f32, paint: impl Into<Paint>) {
        let paint = paint.into();
        let t = width.max(1.0).min(w / 2.0).min(h / 2.0);
        for row in first(y)..first(y + h) {
            let sy = row as f32 + 0.5;
            let Some((a, b)) = round_rect_span(sy, x, y, w, h, r) else { continue };
            // The hole is the same shape inset by the border; rows above and below it
            // are solid.
            match round_rect_span(sy, x + t, y + t, w - 2.0 * t, h - 2.0 * t, r - t) {
                Some((ia, ib)) => {
                    self.span(row, first(a), first(ia), paint);
                    self.span(row, first(ib), first(b), paint);
                }
                None => self.span(row, first(a), first(b), paint),
            }
        }
    }

    /// Fills the ring between radii `inner` and `outer`, centred on `(cx, cy)`.
    pub fn ring(&mut self, cx: f32, cy: f32, outer: f32, inner: f32, paint: impl Into<Paint>) {
        let paint = paint.into();
        let (outer, inner) = (outer.max(0.0), inner.clamp(0.0, outer));
        for y in first(cy - outer)..first(cy + outer) {
            let dy = (y as f32 + 0.5 - cy).abs();
            let ho = (outer * outer - dy * dy).max(0.0).sqrt();
            let hi = (inner * inner - dy * dy).max(0.0).sqrt();
            if hi > 0.0 {
                self.span(y, first(cx - ho), first(cx - hi), paint);
                self.span(y, first(cx + hi), first(cx + ho), paint);
            } else {
                self.span(y, first(cx - ho), first(cx + ho), paint);
            }
        }
    }

    /// Fills the pie slice of the ellipse `(cx, cy, rx, ry)` from angle `a0` to `a1`
    /// (radians, clockwise on screen).
    #[allow(clippy::too_many_arguments)]
    pub fn fill_pie(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, a0: f32, a1: f32, paint: impl Into<Paint>) {
        let n = steps((a1 - a0).abs() * rx.max(ry)).min(MAX_POINTS as u32 - 1);
        let mut pts = [(0.0f32, 0.0f32); MAX_POINTS];
        pts[0] = (cx, cy);
        for i in 0..=n {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            pts[1 + i as usize] = (cx + rx * a.cos(), cy + ry * a.sin());
        }
        self.fill_polygon(&pts[..2 + n as usize], paint);
    }

    /// Fills the regular `sides`-gon inscribed in radius `r` around `(cx, cy)`,
    /// rotated by `rot` radians (`0` puts a vertex to the right).
    #[allow(clippy::too_many_arguments)]
    pub fn fill_ngon(&mut self, cx: f32, cy: f32, r: f32, sides: u32, rot: f32, paint: impl Into<Paint>) {
        let (pts, n) = ngon_points(cx, cy, r, r, sides, rot);
        self.fill_polygon(&pts[..n], paint);
    }

    /// Strokes the outline of [`fill_ngon`](Self::fill_ngon).
    #[allow(clippy::too_many_arguments)]
    pub fn ngon(&mut self, cx: f32, cy: f32, r: f32, sides: u32, rot: f32, width: f32, paint: impl Into<Paint>) {
        let (pts, n) = ngon_points(cx, cy, r, r, sides, rot);
        self.polygon(&pts[..n], width, paint);
    }

    /// Fills a star with `points` spikes reaching `outer`, its notches at `inner`.
    #[allow(clippy::too_many_arguments)]
    pub fn fill_star(
        &mut self,
        cx: f32,
        cy: f32,
        outer: f32,
        inner: f32,
        points: u32,
        rot: f32,
        paint: impl Into<Paint>,
    ) {
        let (pts, n) = ngon_points(cx, cy, outer, inner, points.clamp(2, 32) * 2, rot);
        self.fill_polygon(&pts[..n], paint);
    }

    /// Strokes the outline of [`fill_star`](Self::fill_star).
    #[allow(clippy::too_many_arguments)]
    pub fn star(
        &mut self,
        cx: f32,
        cy: f32,
        outer: f32,
        inner: f32,
        points: u32,
        rot: f32,
        width: f32,
        paint: impl Into<Paint>,
    ) {
        let (pts, n) = ngon_points(cx, cy, outer, inner, points.clamp(2, 32) * 2, rot);
        self.polygon(&pts[..n], width, paint);
    }

    /// Draws an arrow from `from` to `to`: a shaft `width` dots wide and a filled head
    /// `head` dots long, whose tip is exactly `to`.
    pub fn arrow(&mut self, from: Point, to: Point, width: f32, head: f32, paint: impl Into<Paint>) {
        let paint = paint.into();
        let len = dist(from, to);
        if len <= 0.0 {
            return;
        }
        let head = head.max(width).min(len);
        let (ux, uy) = ((to.0 - from.0) / len, (to.1 - from.1) / len);
        // The shaft stops where the head starts, so a dithered arrow does not paint
        // the overlap twice.
        let base = (to.0 - ux * head, to.1 - uy * head);
        self.polyline(&[from, base], width, paint);
        let (nx, ny) = (-uy * head * 0.4, ux * head * 0.4);
        self.fill_polygon(&[to, (base.0 + nx, base.1 + ny), (base.0 - nx, base.1 - ny)], paint);
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
    fn round_boxes_lose_their_corners_only() {
        let mut c = Canvas::new(10, 5);
        c.fill_round_rect(0.0, 0.0, 20.0, 20.0, 6.0, C);
        assert!(c.get(10, 0).is_some() && c.get(0, 10).is_some(), "flat sides stay flat");
        assert!(c.get(0, 0).is_none() && c.get(19, 19).is_none(), "corners are cut");
        let mut o = Canvas::new(10, 5);
        o.round_rect(0.0, 0.0, 20.0, 20.0, 6.0, 2.0, C);
        assert!(o.get(10, 0).is_some() && o.get(10, 1).is_some() && o.get(10, 3).is_none(), "2 dots of border");
        assert!(count(&o) < count(&c));
        // A radius of zero is the plain box.
        let (mut a, mut b) = (Canvas::new(4, 2), Canvas::new(4, 2));
        a.fill_round_rect(1.0, 1.0, 6.0, 6.0, 0.0, C);
        b.fill_rect(1.0, 1.0, 6.0, 6.0, C);
        assert_eq!(a, b);
    }

    #[test]
    fn rings_are_hollow_and_pies_are_wedges() {
        let mut c = Canvas::new(10, 5);
        c.ring(10.0, 10.0, 8.0, 4.0, C);
        assert!(c.get(10, 10).is_none() && c.get(10, 3).is_some() && c.get(10, 16).is_some());
        let n = count(&c) as f32;
        assert!((n - std::f32::consts::PI * (64.0 - 16.0)).abs() < 20.0, "{n}");
        let mut p = Canvas::new(10, 5);
        p.fill_pie(10.0, 10.0, 8.0, 8.0, 0.0, std::f32::consts::FRAC_PI_2, C);
        assert!(p.get(14, 12).is_some(), "inside the quarter");
        assert!(p.get(6, 8).is_none() && p.get(14, 8).is_none(), "outside it");
        assert!((count(&p) as f32 - std::f32::consts::PI * 64.0 / 4.0).abs() < 12.0);
    }

    #[test]
    fn ngons_stars_and_arrows() {
        let mut h = Canvas::new(10, 5);
        h.fill_ngon(10.0, 10.0, 8.0, 6, 0.0, C);
        assert!(h.get(10, 10).is_some() && h.get(17, 10).is_some() && h.get(3, 16).is_none());
        let mut s = Canvas::new(10, 5);
        s.fill_star(10.0, 10.0, 9.0, 3.5, 5, 0.0, C);
        assert!(s.get(16, 10).is_some(), "the spike reaches out past the notch radius");
        assert!(s.get(16, 16).is_none() && s.get(3, 3).is_none(), "the notches between spikes are empty");
        let mut o = Canvas::new(10, 5);
        o.star(10.0, 10.0, 9.0, 3.5, 5, 0.0, 1.0, C);
        assert!(o.get(10, 10).is_none() && count(&o) < count(&s));
        let mut a = Canvas::new(12, 4);
        a.arrow((2.0, 8.0), (20.0, 8.0), 2.0, 6.0, C);
        assert!(a.get(18, 8).is_some(), "the head runs up to the point asked for");
        assert!(a.get(15, 7).is_some() && a.get(15, 9).is_some(), "the head is wider than the shaft");
        assert!(a.get(10, 6).is_none(), "the shaft is only as wide as asked");
    }

    #[test]
    fn rect_geometry_helpers() {
        let r = Rect::new(2.0, 4.0, 10.0, 6.0);
        assert_eq!((r.right(), r.bottom(), r.center()), (12.0, 10.0, (7.0, 7.0)));
        assert!(r.contains((2.0, 4.0)) && !r.contains((12.0, 7.0)));
        assert_eq!(r.inset(1.0), Rect::new(3.0, 5.0, 8.0, 4.0));
        assert_eq!(r.offset(1.0, -1.0), Rect::new(3.0, 3.0, 10.0, 6.0));
        assert_eq!(r.overlap(&Rect::new(7.0, 4.0, 10.0, 6.0)), 30.0);
        assert_eq!(r.overlap(&Rect::new(90.0, 0.0, 1.0, 1.0)), 0.0);
        assert_eq!(Rect::around((0.0, 0.0), 4.0, 2.0), Rect::new(-2.0, -1.0, 4.0, 2.0));
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
