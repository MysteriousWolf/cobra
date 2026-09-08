//! Arbitrary outlines: straight lines, Bézier curves, arcs and splines in any
//! combination, filled or stroked with [`Canvas::fill_path`] and
//! [`Canvas::stroke_path`].

use crate::draw::{dist, steps};
use crate::{Point, Rect};

/// One drawing command; curves keep their control points until they are flattened.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Cmd {
    Move(Point),
    Line(Point),
    Quad(Point, Point),
    Cubic(Point, Point, Point),
    Close,
}

/// An outline built from segments: lines, quadratic and cubic Béziers, elliptical
/// arcs and smooth curves through points, in any order, in one or more subpaths.
///
/// A path is built with the `*_to` methods, each continuing from the current point;
/// [`move_to`](Self::move_to) starts a new subpath and [`close`](Self::close) joins
/// one back to its start. Filling treats every subpath as closed and uses the
/// even-odd rule, so a subpath inside another cuts a hole. Stroking follows each
/// subpath as drawn. Curves are flattened when drawn, about one segment per dot.
///
/// ```
/// use cobra::{Canvas, Path, Pen, Rgb};
///
/// let mut heart = Path::new();
/// heart.move_to((10.0, 6.0)).cubic_to((10.0, 0.0), (0.0, 0.0), (0.0, 7.0));
/// heart.cubic_to((0.0, 12.0), (10.0, 16.0), (10.0, 20.0));
/// heart.cubic_to((10.0, 16.0), (20.0, 12.0), (20.0, 7.0));
/// heart.cubic_to((20.0, 0.0), (10.0, 0.0), (10.0, 6.0)).close();
/// heart.ellipse(10.0, 9.0, 3.0, 2.0); // a hole
///
/// let mut canvas = Canvas::new(12, 6);
/// canvas.fill_path(&heart, Rgb::hex(0xff3355));
/// canvas.stroke_path(&heart, Pen::new(1.0).dash(2.0, 2.0), Rgb::hex(0xffffff));
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    cmds: Vec<Cmd>,
    /// Start of the current subpath, for [`close`](Self::close) and for `*_to`
    /// calls on a path with nothing to continue from.
    start: Point,
}

impl Path {
    /// An empty path.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether nothing has been drawn.
    pub fn is_empty(&self) -> bool {
        self.cmds.is_empty()
    }

    /// The point the next segment continues from.
    pub fn current(&self) -> Point {
        match self.cmds.last() {
            Some(Cmd::Move(p) | Cmd::Line(p) | Cmd::Quad(_, p) | Cmd::Cubic(_, _, p)) => *p,
            Some(Cmd::Close) | None => self.start,
        }
    }

    /// Starts a new subpath at `p`.
    pub fn move_to(&mut self, p: Point) -> &mut Self {
        self.start = p;
        self.cmds.push(Cmd::Move(p));
        self
    }

    /// A straight line to `p`.
    pub fn line_to(&mut self, p: Point) -> &mut Self {
        self.cmds.push(Cmd::Line(p));
        self
    }

    /// A quadratic Bézier to `p` steered by `c`.
    pub fn quad_to(&mut self, c: Point, p: Point) -> &mut Self {
        self.cmds.push(Cmd::Quad(c, p));
        self
    }

    /// A cubic Bézier to `p` steered by `c1` and `c2`.
    pub fn cubic_to(&mut self, c1: Point, c2: Point, p: Point) -> &mut Self {
        self.cmds.push(Cmd::Cubic(c1, c2, p));
        self
    }

    /// An arc of the ellipse centred on `(cx, cy)` with radii `rx`, `ry`, from angle
    /// `a0` to `a1` (radians, clockwise on screen), joined to the current point by a
    /// line, or starting a subpath if there is none. Arcs are cubic Béziers, one per
    /// quarter turn.
    pub fn arc_to(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, a0: f32, a1: f32) -> &mut Self {
        let at = |a: f32| (cx + rx * a.cos(), cy + ry * a.sin());
        let p0 = at(a0);
        match self.cmds.last() {
            Some(Cmd::Close) | None => self.move_to(p0),
            _ => self.line_to(p0),
        };
        let sweep = a1 - a0;
        let n = (sweep.abs() / std::f32::consts::FRAC_PI_2).ceil().max(1.0) as u32;
        let step = sweep / n as f32;
        let k = 4.0 / 3.0 * (step / 4.0).tan();
        for i in 0..n {
            let (s, e) = (a0 + step * i as f32, a0 + step * (i + 1) as f32);
            let (p, q) = (at(s), at(e));
            let c1 = (p.0 - k * rx * s.sin(), p.1 + k * ry * s.cos());
            let c2 = (q.0 + k * rx * e.sin(), q.1 - k * ry * e.cos());
            self.cubic_to(c1, c2, q);
        }
        self
    }

    /// A smooth curve (Catmull–Rom) through every point of `pts`, starting with a
    /// line to the first, or a new subpath if there is none. Each span becomes the
    /// equivalent cubic Bézier.
    pub fn curve_through(&mut self, pts: &[Point]) -> &mut Self {
        let n = pts.len();
        if n == 0 {
            return self;
        }
        match self.cmds.last() {
            Some(Cmd::Close) | None => self.move_to(pts[0]),
            _ => self.line_to(pts[0]),
        };
        let at = |i: isize| pts[i.clamp(0, n as isize - 1) as usize];
        for s in 0..n.saturating_sub(1) {
            let (p0, p1, p2, p3) = (at(s as isize - 1), at(s as isize), at(s as isize + 1), at(s as isize + 2));
            let c1 = (p1.0 + (p2.0 - p0.0) / 6.0, p1.1 + (p2.1 - p0.1) / 6.0);
            let c2 = (p2.0 - (p3.0 - p1.0) / 6.0, p2.1 - (p3.1 - p1.1) / 6.0);
            self.cubic_to(c1, c2, p2);
        }
        self
    }

    /// Closes the current subpath with a line back to where it started.
    pub fn close(&mut self) -> &mut Self {
        if !matches!(self.cmds.last(), Some(Cmd::Close) | None) {
            self.cmds.push(Cmd::Close);
        }
        self
    }

    /// A closed rectangle as its own subpath.
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32) -> &mut Self {
        self.move_to((x, y)).line_to((x + w, y)).line_to((x + w, y + h)).line_to((x, y + h)).close()
    }

    /// A closed box with corners rounded to radius `r`, as its own subpath.
    pub fn round_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32) -> &mut Self {
        use std::f32::consts::{FRAC_PI_2, PI};
        let r = r.clamp(0.0, w.min(h) / 2.0);
        if r <= 0.0 {
            return self.rect(x, y, w, h);
        }
        let (right, bottom) = (x + w, y + h);
        self.move_to((x + r, y));
        self.arc_to(right - r, y + r, r, r, -FRAC_PI_2, 0.0);
        self.arc_to(right - r, bottom - r, r, r, 0.0, FRAC_PI_2);
        self.arc_to(x + r, bottom - r, r, r, FRAC_PI_2, PI);
        self.arc_to(x + r, y + r, r, r, PI, 1.5 * PI);
        self.close()
    }

    /// Applies `t` to every point (control points included).
    pub fn apply(&mut self, t: &crate::Transform) -> &mut Self {
        self.transform(|p| t.apply(p))
    }

    /// A closed ellipse as its own subpath.
    pub fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32) -> &mut Self {
        self.move_to((cx + rx, cy)).arc_to(cx, cy, rx, ry, 0.0, std::f32::consts::TAU).close()
    }

    /// A closed polygon as its own subpath.
    pub fn polygon(&mut self, pts: &[Point]) -> &mut Self {
        if let Some((&p, rest)) = pts.split_first() {
            self.move_to(p);
            for &q in rest {
                self.line_to(q);
            }
            self.close();
        }
        self
    }

    /// Maps every point (control points included) through `f`.
    pub fn transform(&mut self, f: impl Fn(Point) -> Point) -> &mut Self {
        for cmd in &mut self.cmds {
            match cmd {
                Cmd::Move(p) | Cmd::Line(p) => *p = f(*p),
                Cmd::Quad(c, p) => (*c, *p) = (f(*c), f(*p)),
                Cmd::Cubic(c1, c2, p) => (*c1, *c2, *p) = (f(*c1), f(*c2), f(*p)),
                Cmd::Close => {}
            }
        }
        self.start = f(self.start);
        self
    }

    /// Moves the path by `(dx, dy)`.
    pub fn translate(&mut self, dx: f32, dy: f32) -> &mut Self {
        self.transform(|(x, y)| (x + dx, y + dy))
    }

    /// Scales the path about the origin.
    pub fn scale(&mut self, sx: f32, sy: f32) -> &mut Self {
        self.transform(|(x, y)| (x * sx, y * sy))
    }

    /// Rotates the path by `angle` radians (clockwise on screen) about `center`.
    pub fn rotate(&mut self, angle: f32, center: Point) -> &mut Self {
        let (s, c) = angle.sin_cos();
        self.transform(|(x, y)| {
            let (dx, dy) = (x - center.0, y - center.1);
            (center.0 + dx * c - dy * s, center.1 + dx * s + dy * c)
        })
    }

    /// The box around every point of the path, control points included; `None`
    /// when empty.
    pub fn bounds(&self) -> Option<Rect> {
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        let mut grow = |p: &Point| (x0, y0, x1, y1) = (x0.min(p.0), y0.min(p.1), x1.max(p.0), y1.max(p.1));
        for cmd in &self.cmds {
            match cmd {
                Cmd::Move(p) | Cmd::Line(p) => grow(p),
                Cmd::Quad(c, p) => [c, p].into_iter().for_each(&mut grow),
                Cmd::Cubic(c1, c2, p) => [c1, c2, p].into_iter().for_each(&mut grow),
                Cmd::Close => {}
            }
        }
        (x0 <= x1).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
    }

    /// Flattens the path and hands each subpath to `f` as a polyline, with whether
    /// it was closed.
    pub fn subpaths(&self, mut f: impl FnMut(&[Point], bool)) {
        let mut pts: Vec<Point> = Vec::new();
        let mut flush = |pts: &mut Vec<Point>, closed: bool| {
            if pts.len() > 1 || (closed && !pts.is_empty()) {
                f(pts, closed);
            }
            pts.clear();
        };
        for cmd in &self.cmds {
            let last = pts.last().copied().unwrap_or(self.start);
            match *cmd {
                Cmd::Move(p) => {
                    flush(&mut pts, false);
                    pts.push(p);
                }
                Cmd::Line(p) => pts.push(p),
                Cmd::Quad(c, p) => {
                    let n = steps(dist(last, c) + dist(c, p));
                    pts.extend((1..=n).map(|i| {
                        let t = i as f32 / n as f32;
                        let (a, b) = (lerp(last, c, t), lerp(c, p, t));
                        lerp(a, b, t)
                    }));
                }
                Cmd::Cubic(c1, c2, p) => {
                    let n = steps(dist(last, c1) + dist(c1, c2) + dist(c2, p));
                    pts.extend((1..=n).map(|i| {
                        let t = i as f32 / n as f32;
                        let (a, b, c) = (lerp(last, c1, t), lerp(c1, c2, t), lerp(c2, p, t));
                        lerp(lerp(a, b, t), lerp(b, c, t), t)
                    }));
                }
                Cmd::Close => flush(&mut pts, true),
            }
        }
        flush(&mut pts, false);
    }
}

#[inline]
fn lerp(a: Point, b: Point, t: f32) -> Point {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Canvas, Pen, Rgb};

    const C: Rgb = Rgb::hex(0xffffff);

    fn count(c: &Canvas) -> usize {
        c.cells().map(|cell| cell.bits.count_ones() as usize).sum()
    }

    #[test]
    fn building_and_current_point() {
        let mut p = Path::new();
        assert!(p.is_empty());
        p.move_to((1.0, 2.0)).line_to((3.0, 4.0));
        assert_eq!(p.current(), (3.0, 4.0));
        p.close();
        assert_eq!(p.current(), (1.0, 2.0), "closing returns to the start");
        p.close();
        assert_eq!(p.cmds.iter().filter(|c| matches!(c, Cmd::Close)).count(), 1, "closing twice is once");
        p.arc_to(10.0, 10.0, 5.0, 5.0, 0.0, std::f32::consts::PI);
        assert!(matches!(p.cmds[3], Cmd::Move(_)), "an arc after a close starts a subpath");
        assert_eq!(p.bounds().map(|r| r.x), Some(1.0));
        assert!(Path::new().bounds().is_none());
    }

    #[test]
    fn fills_with_holes_and_strokes_dashed() {
        let mut ring = Path::new();
        ring.ellipse(10.0, 10.0, 9.0, 9.0).ellipse(10.0, 10.0, 4.0, 4.0);
        let mut c = Canvas::new(10, 5);
        c.fill_path(&ring, C);
        assert!(c.get(10, 10).is_none(), "the inner ellipse is a hole");
        assert!(c.get(10, 3).is_some() && c.get(10, 16).is_some());
        let mut d = Canvas::new(10, 5);
        d.ring(10.0, 10.0, 9.0, 4.0, C);
        assert!((count(&c) as i32 - count(&d) as i32).abs() < 20, "a path ring is about a ring");

        let mut line = Path::new();
        line.move_to((0.0, 2.0)).line_to((19.0, 2.0));
        let mut s = Canvas::new(10, 1);
        s.stroke_path(&line, Pen::new(1.0).dash(3.0, 3.0), C);
        let lit: Vec<bool> = (0..20).map(|x| s.get(x, 2).is_some()).collect();
        assert!(lit[0] && lit[2] && !lit[3] && !lit[5] && lit[6], "{lit:?}");
        let mut s2 = Canvas::new(10, 1);
        s2.stroke_path(&line, Pen::new(1.0).dash(3.0, 3.0).phase(3.0), C);
        assert!(s2.get(0, 2).is_none() && s2.get(3, 2).is_some(), "the phase shifts the pattern");
    }

    #[test]
    fn curves_transform_and_flatten() {
        let mut p = Path::new();
        p.move_to((0.0, 0.0)).quad_to((10.0, 20.0), (20.0, 0.0));
        p.curve_through(&[(30.0, 10.0), (40.0, 0.0)]);
        let mut n = 0;
        p.subpaths(|pts, closed| {
            n += 1;
            assert!(!closed);
            assert_eq!(pts[0], (0.0, 0.0));
            assert_eq!(*pts.last().unwrap(), (40.0, 0.0));
            assert!(pts.iter().any(|q| q.1 > 8.0), "the quadratic bows out");
        });
        assert_eq!(n, 1);
        p.translate(1.0, 1.0).scale(2.0, 2.0);
        assert_eq!(p.current(), (82.0, 2.0));
        let mut sq = Path::new();
        sq.rect(0.0, 0.0, 10.0, 10.0).rotate(std::f32::consts::FRAC_PI_2, (5.0, 5.0));
        let b = sq.bounds().unwrap();
        assert!((b.x).abs() < 1e-4 && (b.w - 10.0).abs() < 1e-4);
        let mut c = Canvas::new(5, 3);
        c.fill_path(&sq, C);
        assert_eq!(count(&c), 100);
    }
}
