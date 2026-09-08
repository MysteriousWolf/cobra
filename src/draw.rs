//! Vector primitives: rectangles, polygons, ellipses, arcs, Bézier curves, splines
//! and [`Path`]s, filled or stroked, drawn straight into the dot buffer.
//!
//! Everything here reduces to horizontal spans ([`Canvas::span`]): a fill is one
//! clipped `slice::fill` per dot row rather than a bounds check per dot, and a stroke
//! is a chain of Bresenham lines (width ≤ 1) or convex quads plus round joins. Curves
//! are flattened on the fly, one segment at a time, so nothing here allocates
//! (except [`Canvas::fill_polygon`] on a polygon with more than 128 vertices, paths,
//! and the shape-relative paints described under [`Paint`]).
//!
//! Coordinates are in dots, `f32`, with dot `(x, y)` covering the unit square from
//! `(x, y)` to `(x + 1, y + 1)`. A fill includes every dot whose centre is inside the
//! shape, so `fill_rect(2.0, 3.0, 4.0, 2.0)` is exactly dots `2..6 × 3..5`.
//!
//! Every method takes a [`Paint`] (a colour, a dither, a pattern, a gradient, a
//! shader, or [`Paint::erase`]) and every stroke takes a [`Pen`] (a width, or a width
//! with a dash pattern; a bare `f32` is a solid pen).

use std::cell::RefCell;

use crate::canvas::bayer;
use crate::layer::{Edt, INF};
use crate::{Canvas, Color, Path, Silhouette, Transform};

/// A repeating dot pattern for [`Paint::pattern`], with its period in dots. Patterns
/// are anchored at the canvas origin unless the paint is [anchored](Paint::anchor)
/// elsewhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Pattern {
    /// Horizontal lines, one dot thick, every `n` dots.
    Rows(u8),
    /// Vertical lines, one dot thick, every `n` dots.
    Columns(u8),
    /// Lines running down-right (`\`), every `n` dots.
    Diagonal(u8),
    /// Lines running up-right (`/`), every `n` dots.
    Antidiagonal(u8),
    /// Both diagonals: cross-hatching.
    Cross(u8),
    /// Rows and columns: a grid.
    Grid(u8),
    /// A checkerboard of `n × n` squares.
    Checker(u8),
    /// One dot every `n` dots, each row staggered by half a period.
    Dots(u8),
}

impl Pattern {
    /// Whether the pattern has a dot at `(x, y)`.
    #[inline]
    pub fn on(self, x: i32, y: i32) -> bool {
        let n = |n: u8| n.max(1) as i32;
        match self {
            Pattern::Rows(p) => y.rem_euclid(n(p)) == 0,
            Pattern::Columns(p) => x.rem_euclid(n(p)) == 0,
            Pattern::Diagonal(p) => (x - y).rem_euclid(n(p)) == 0,
            Pattern::Antidiagonal(p) => (x + y).rem_euclid(n(p)) == 0,
            Pattern::Cross(p) => (x - y).rem_euclid(n(p)) == 0 || (x + y).rem_euclid(n(p)) == 0,
            Pattern::Grid(p) => x.rem_euclid(n(p)) == 0 || y.rem_euclid(n(p)) == 0,
            Pattern::Checker(p) => (x.div_euclid(n(p)) + y.div_euclid(n(p))).rem_euclid(2) == 0,
            Pattern::Dots(p) => {
                let n = n(p);
                y.rem_euclid(n) == 0 && (x + y.div_euclid(n) * (n / 2)).rem_euclid(n) == 0
            }
        }
    }
}

/// A shader for [`Paint::shader`]: the paint for one dot, or `None` to leave it.
pub type Shader = fn(&Probe) -> Option<Paint>;

/// One dot as a [`Paint::shader`] sees it: where it is, and where in the shape.
///
/// The shape-relative fields (`u`, `v`, `dist`, `normal`) come from the shape the
/// paint is filling: its bounding box and its distance field. With
/// [`Paint::per_cell`] they are those of one dot per cell, so what a shader decides
/// is decided per cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Probe {
    /// The dot, relative to the paint's [anchor](Paint::anchor).
    pub x: i32,
    /// The dot, relative to the paint's [anchor](Paint::anchor).
    pub y: i32,
    /// Where the dot sits across the shape's bounding box, `0..1` left to right.
    pub u: f32,
    /// Where the dot sits down the shape's bounding box, `0..1` top to bottom.
    pub v: f32,
    /// Signed distance in dots to the shape's edge: `-1` on the edge, more negative
    /// deeper inside. (A paint only ever fills the inside, so it is never positive.)
    pub dist: f32,
    /// The unit normal of the shape's surface at this dot, pointing outwards: the
    /// direction to the nearest edge, which for a rounded shape is the direction it
    /// faces. `(0, 0)` on a ridge equidistant from two edges. Dot it with a light
    /// direction and the shape is lit.
    pub normal: (f32, f32),
    /// The paint's first colour.
    pub a: Color,
    /// The paint's second colour.
    pub b: Color,
}

impl Probe {
    /// Solid `a` blended towards `b` by `t` (`0..=1`): a true blend for RGB colours,
    /// an ordered dither between the two for palette colours.
    #[inline]
    pub fn mix(&self, t: f32) -> Paint {
        Paint::new(Color::from_packed(mix(self.a.packed(), self.b.packed(), t, bayer(self.y, self.x))))
    }

    /// How much the surface faces `light`, a direction towards the light: `1`
    /// facing it, `0` side on, `-1` facing away. This is `normal · light` with
    /// `light` normalised, the number every kind of shading starts from.
    #[inline]
    pub fn lit(&self, light: Point) -> f32 {
        let (lx, ly) = unit(light);
        self.normal.0 * lx + self.normal.1 * ly
    }
}

/// `v` scaled to unit length, or `(0, 0)`.
#[inline]
fn unit(v: Point) -> Point {
    let len = (v.0 * v.0 + v.1 * v.1).sqrt();
    if len > 0.0 { (v.0 / len, v.1 / len) } else { (0.0, 0.0) }
}

/// Blends two packed colours, or picks one of them against a threshold when either
/// is a palette colour that cannot be blended.
#[inline]
fn mix(a: u32, b: u32, t: f32, threshold: f32) -> u32 {
    match (Color::from_packed(a), Color::from_packed(b)) {
        (Color::Rgb(p), Color::Rgb(q)) => Color::Rgb(p.lerp(q, t.clamp(0.0, 1.0))).packed(),
        _ if t > threshold => b,
        _ => a,
    }
}

/// A well-mixed hash of a dot position in `0..1`: a dither threshold with no
/// lattice in it.
#[inline]
fn hash(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^= h >> 15;
    (h >> 8) as f32 / 16_777_216.0
}

/// How a paint spreads partial coverage over dots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dither {
    /// The 4×4 Bayer lattice: even, and visibly a lattice.
    Ordered,
    /// A hash of the dot position: the same number of dots, with no grain.
    Hashed,
}

/// One band of a cel paint: everything at or above `above` gets its ink.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Band {
    above: f32,
    packed: u32,
    coverage: f32,
    pattern: Option<Pattern>,
}

/// Bands a cel paint can hold.
const MAX_BANDS: usize = 4;

/// How a paint decides the colour of a dot.
#[derive(Clone, Copy, Debug)]
enum Kind {
    /// One colour everywhere.
    Flat,
    /// The colour on the dots of a pattern.
    Pattern(Pattern),
    /// A blend from the paint's colour at one point to `b` at another.
    Linear { x0: f32, y0: f32, x1: f32, y1: f32, b: u32 },
    /// A blend from the paint's colour at a centre to `b` at a radius.
    Radial { cx: f32, cy: f32, r: f32, b: u32 },
    /// A blend from the paint's colour on the shape's edge to `b` `depth` dots in.
    Edge { depth: f32, b: u32 },
    /// Flat bands by how much the surface faces a light.
    Cel { lx: f32, ly: f32, bands: [Band; MAX_BANDS], n: u8, soft: f32 },
    /// A function of the dot.
    Shader { f: Shader, b: u32 },
}

impl PartialEq for Kind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Kind::Flat, Kind::Flat) => true,
            (Kind::Pattern(a), Kind::Pattern(b)) => a == b,
            (Kind::Linear { x0, y0, x1, y1, b }, Kind::Linear { x0: p, y0: q, x1: r, y1: s, b: t }) => {
                (x0, y0, x1, y1, b) == (p, q, r, s, t)
            }
            (Kind::Radial { cx, cy, r, b }, Kind::Radial { cx: p, cy: q, r: s, b: t }) => {
                (cx, cy, r, b) == (p, q, s, t)
            }
            (Kind::Edge { depth, b }, Kind::Edge { depth: d, b: c }) => (depth, b) == (d, c),
            (Kind::Cel { lx, ly, bands, n, soft }, Kind::Cel { lx: p, ly: q, bands: r, n: m, soft: t }) => {
                (lx, ly, n, soft) == (p, q, m, t) && bands[..*n as usize] == r[..*m as usize]
            }
            (Kind::Shader { f, b }, Kind::Shader { f: g, b: c }) => std::ptr::fn_addr_eq(*f, *g) && b == c,
            _ => false,
        }
    }
}

/// What a shape is drawn with.
///
/// The simplest paint is a colour ([`Paint::new`], or any colour converted with
/// `into()`), and [`Paint::erase`] unsets dots instead. On top of that:
///
/// * [`dithered`](Self::dithered) draws a fraction of the dots in an ordered pattern
///   ([`hashed`](Self::hashed) spreads them without the pattern);
/// * [`pattern`](Self::pattern) draws hatching, grids, checkers or dots;
/// * [`linear`](Self::linear) and [`radial`](Self::radial) blend between two colours
///   across the canvas;
/// * [`edge`](Self::edge) blends from the edge of the shape inwards;
/// * [`cel`](Self::cel) lights the shape from a direction in flat bands;
/// * [`shader`](Self::shader) is a function of your own, given the dot's position,
///   where it lies in the shape and which way the shape faces there.
///
/// Dithers, patterns and gradients are laid out in canvas coordinates, so a shape
/// drawn in the same paint at another position shows another slice of the texture.
/// [`anchor`](Self::anchor) moves the paint's origin to a point of the shape, so the
/// texture moves with it.
///
/// Edge, cel and shader paints are *shape-relative*: the shape is first rasterised
/// into a scratch mask (one kept per thread, so it costs an allocation once), its
/// distance field is computed, and the paint is then evaluated per dot with its
/// position in the shape's bounding box, its distance to the edge and the surface
/// normal there. [`per_cell`](Self::per_cell) evaluates once per cell instead, so
/// that whatever the paint decides lands on cell boundaries and survives the text
/// fallback, where a cell has one colour.
///
/// ```
/// use cobra::{Canvas, Paint, Pattern, Rgb};
///
/// let (red, blue) = (Rgb::hex(0xff3355), Rgb::hex(0x3355ff));
/// let mut canvas = Canvas::new(20, 5);
/// canvas.fill_rect(0.0, 0.0, 20.0, 20.0, Paint::linear((0.0, 0.0), (20.0, 0.0), red, blue));
/// canvas.disc(30.0, 10.0, 8.0, Paint::edge(blue, red, 3.0));
/// canvas.fill_rect(0.0, 0.0, 40.0, 4.0, Paint::pattern(red, Pattern::Diagonal(3)).anchor(0, 0));
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paint {
    packed: u32,
    coverage: f32,
    anchor: (i32, i32),
    dither: Dither,
    per_cell: bool,
    kind: Kind,
}

impl Paint {
    /// Solid `color`.
    pub fn new(color: impl Into<Color>) -> Self {
        Self {
            packed: color.into().packed(),
            coverage: 1.0,
            anchor: (0, 0),
            dither: Dither::Ordered,
            per_cell: false,
            kind: Kind::Flat,
        }
    }

    /// `color` on `coverage` (`0..=1`) of the dots, in the ordered Bayer pattern of
    /// [`Canvas::set_dithered`].
    pub fn dithered(color: impl Into<Color>, coverage: f32) -> Self {
        Self::new(color).dither(coverage)
    }

    /// Unsets dots instead of colouring them.
    pub const fn erase() -> Self {
        Self { packed: 0, coverage: 1.0, anchor: (0, 0), dither: Dither::Ordered, per_cell: false, kind: Kind::Flat }
    }

    /// `color` on the dots of `pattern`.
    pub fn pattern(color: impl Into<Color>, pattern: Pattern) -> Self {
        Self { kind: Kind::Pattern(pattern), ..Self::new(color) }
    }

    /// A gradient from `a` at `from` to `b` at `to`, constant beyond either end and
    /// along lines perpendicular to the axis between them.
    pub fn linear(from: Point, to: Point, a: impl Into<Color>, b: impl Into<Color>) -> Self {
        let kind = Kind::Linear { x0: from.0, y0: from.1, x1: to.0, y1: to.1, b: b.into().packed() };
        Self { kind, ..Self::new(a) }
    }

    /// A gradient from `a` at `center` to `b` at radius `r`, and `b` beyond.
    pub fn radial(center: Point, r: f32, a: impl Into<Color>, b: impl Into<Color>) -> Self {
        Self { kind: Kind::Radial { cx: center.0, cy: center.1, r, b: b.into().packed() }, ..Self::new(a) }
    }

    /// A gradient from `a` on the edge of whatever shape it fills to `b` `depth` dots
    /// inside it, and `b` further in: shading that follows the shape. Shape-relative;
    /// see the [type docs](Self).
    pub fn edge(a: impl Into<Color>, b: impl Into<Color>, depth: f32) -> Self {
        Self { kind: Kind::Edge { depth: depth.max(f32::EPSILON), b: b.into().packed() }, ..Self::new(a) }
    }

    /// Cel shading: `base` where the shape faces away from `light` (a direction
    /// towards the light, such as `(-1.0, -1.0)` for light from the upper left), and
    /// each of `bands` where the surface faces it at least as much as the band's
    /// threshold, `-1..=1` as [`Probe::lit`] gives it. A band's paint lends its
    /// colour, coverage and pattern, so a band can be a flat ink, a dither or a
    /// hatch. Up to four bands, in any order. Shape-relative; see the [type
    /// docs](Self), and [`per_cell`](Self::per_cell) for bands that survive the text
    /// fallback, [`soften`](Self::soften) for band edges that dissolve instead of
    /// stepping.
    ///
    /// ```
    /// use cobra::{Canvas, Paint, Rgb};
    ///
    /// let (dark, mid, light) = (Rgb::hex(0x7a3e00), Rgb::hex(0xff8c1a), Rgb::hex(0xffe08a));
    /// let ball = Paint::cel(dark, (-1.0, -1.0), &[(-0.2, mid), (0.5, light)]).per_cell();
    /// let mut canvas = Canvas::new(10, 5);
    /// canvas.disc(10.0, 10.0, 9.0, ball);
    /// ```
    pub fn cel<P: Into<Paint> + Copy>(base: impl Into<Color>, light: Point, bands: &[(f32, P)]) -> Self {
        let (lx, ly) = unit(light);
        let mut sorted = [Band { above: 0.0, packed: 0, coverage: 1.0, pattern: None }; MAX_BANDS];
        let n = bands.len().min(MAX_BANDS);
        for (slot, &(above, p)) in sorted.iter_mut().zip(bands) {
            let p: Paint = p.into();
            let pattern = match p.kind {
                Kind::Pattern(pat) => Some(pat),
                _ => None,
            };
            *slot = Band { above, packed: p.packed, coverage: p.coverage, pattern };
        }
        sorted[..n].sort_by(|a, b| a.above.total_cmp(&b.above));
        Self { kind: Kind::Cel { lx, ly, bands: sorted, n: n as u8, soft: 0.0 }, ..Self::new(base) }
    }

    /// A paint computed per dot by `f`, given a [`Probe`] carrying the dot's
    /// position, its place in the shape, the surface normal there and the two
    /// colours `a` and `b`. `f` returns the paint for the dot (its colour, coverage
    /// and pattern are used) or `None` to leave the dot as it is. Shape-relative;
    /// see the [type docs](Self).
    ///
    /// ```
    /// use cobra::{Canvas, Paint, Rgb};
    ///
    /// // Horizontal bands of the two colours, four dots tall.
    /// let bands = Paint::shader(Rgb::hex(0xffffff), Rgb::hex(0x808080), |p| Some(p.mix((p.y / 4 % 2) as f32)));
    /// // Lit from the top left, smoothly: the surface normal against the light.
    /// let lit = Paint::shader(Rgb::hex(0x203040), Rgb::hex(0x80c0ff), |p| Some(p.mix(p.lit((-1.0, -1.0)))));
    /// let mut canvas = Canvas::new(10, 5);
    /// canvas.disc(10.0, 10.0, 8.0, lit);
    /// canvas.fill_rect(0.0, 0.0, 20.0, 4.0, bands);
    /// ```
    pub fn shader(a: impl Into<Color>, b: impl Into<Color>, f: Shader) -> Self {
        Self { kind: Kind::Shader { f, b: b.into().packed() }, ..Self::new(a) }
    }

    /// Draws only `coverage` (`0..=1`) of the dots this paint would, in the ordered
    /// pattern of [`Canvas::set_dithered`].
    pub fn dither(mut self, coverage: f32) -> Self {
        self.coverage = coverage;
        self
    }

    /// Spreads the paint's coverage by a hash of each dot's position instead of the
    /// Bayer lattice: the same share of dots, with no visible grid. Ordered dither
    /// is right for a gradient, where the eye wants regularity; this is right for a
    /// texture, where it reads regularity as a material.
    pub fn hashed(mut self) -> Self {
        self.dither = Dither::Hashed;
        self
    }

    /// Evaluates the paint once per cell, at the covered dot nearest the cell's
    /// centre, and gives every dot of the cell the answer: gradients, cel bands and
    /// shaders then change colour only on cell boundaries, so a band is never
    /// thinner than the one colour a cell has in the text fallback. The coverage
    /// dither stays per dot.
    pub fn per_cell(mut self) -> Self {
        self.per_cell = true;
        self
    }

    /// For a [`cel`](Self::cel) paint: within `width` (in units of [`Probe::lit`],
    /// so about `0.1`–`0.3`) of a band's threshold, dither between the two bands
    /// instead of stepping, so the terminator dissolves over a few dots. Ignored by
    /// other paints.
    pub fn soften(mut self, width: f32) -> Self {
        if let Kind::Cel { soft, .. } = &mut self.kind {
            *soft = width.max(0.0);
        }
        self
    }

    /// Moves the paint's origin to dot `(x, y)`: the dither, the pattern and the
    /// gradient geometry are evaluated relative to it, so a texture drawn at a
    /// shape's own corner follows the shape when it moves.
    pub fn anchor(mut self, x: i32, y: i32) -> Self {
        self.anchor = (x, y);
        self
    }

    /// The colour, `None` for [`erase`](Self::erase). For a gradient or a shader, the
    /// first of its two colours; for a cel paint, its base.
    pub fn color(&self) -> Option<Color> {
        (self.packed != 0).then(|| Color::from_packed(self.packed))
    }

    /// Dither coverage, `1.0` when solid.
    pub fn coverage(&self) -> f32 {
        self.coverage
    }

    /// Whether the paint is one colour everywhere, so a span of it is one `fill`.
    #[inline]
    fn is_solid(&self) -> bool {
        self.coverage >= 1.0 && matches!(self.kind, Kind::Flat)
    }

    /// Whether the paint needs the shape it fills rasterised first.
    #[inline]
    pub(crate) fn needs_shape(&self) -> bool {
        matches!(self.kind, Kind::Edge { .. } | Kind::Shader { .. } | Kind::Cel { .. }) || self.per_cell
    }

    /// The dither threshold for a dot, in the paint's own coordinates.
    #[inline]
    fn threshold(&self, x: i32, y: i32) -> f32 {
        match self.dither {
            Dither::Ordered => bayer(x, y),
            Dither::Hashed => hash(x, y),
        }
    }

    /// What the paint writes at dot `(x, y)` when nothing is known about the shape:
    /// `None` to leave the dot alone, `Some(0)` to erase it.
    #[inline]
    pub(crate) fn sample(&self, x: i32, y: i32) -> Option<u32> {
        self.sample_in(x, y, &Shaped { x, y, u: 0.0, v: 0.0, dist: -INF, normal: (0.0, 0.0) })
    }

    /// [`sample`](Self::sample) for dot `(x, y)` with what is known of the shape at
    /// `at` (the dot itself, or the cell's representative under `per_cell`).
    #[inline]
    fn sample_in(&self, x: i32, y: i32, at: &Shaped) -> Option<u32> {
        let (x, y) = (x - self.anchor.0, y - self.anchor.1);
        let thr = self.threshold(x, y);
        if self.coverage < 1.0 && self.coverage <= thr {
            return None;
        }
        let (px, py) = (at.x - self.anchor.0, at.y - self.anchor.1);
        match self.kind {
            Kind::Flat => Some(self.packed),
            Kind::Pattern(p) => p.on(x, y).then_some(self.packed),
            Kind::Linear { x0, y0, x1, y1, b } => {
                let (dx, dy) = (x1 - x0, y1 - y0);
                let len2 = dx * dx + dy * dy;
                let t =
                    if len2 > 0.0 { ((px as f32 + 0.5 - x0) * dx + (py as f32 + 0.5 - y0) * dy) / len2 } else { 1.0 };
                Some(mix(self.packed, b, t, bayer(y, x)))
            }
            Kind::Radial { cx, cy, r, b } => {
                let d = ((px as f32 + 0.5 - cx).powi(2) + (py as f32 + 0.5 - cy).powi(2)).sqrt();
                Some(mix(self.packed, b, if r > 0.0 { d / r } else { 1.0 }, bayer(y, x)))
            }
            Kind::Edge { depth, b } => Some(mix(self.packed, b, (-at.dist - 1.0) / depth, bayer(y, x))),
            Kind::Cel { lx, ly, bands, n, soft } => {
                let n = n as usize;
                let t = at.normal.0 * lx + at.normal.1 * ly;
                let mut k = bands[..n].iter().take_while(|band| band.above <= t).count();
                if soft > 0.0 {
                    // Near a threshold, the two bands share the dots.
                    if k < n && t > bands[k].above - soft {
                        if (t - bands[k].above + soft) / (2.0 * soft) > thr {
                            k += 1;
                        }
                    } else if k > 0
                        && t < bands[k - 1].above + soft
                        && (t - bands[k - 1].above + soft) / (2.0 * soft) <= thr
                    {
                        k -= 1;
                    }
                }
                // A band's dither or pattern leaves its off dots to the band below.
                while k > 0 {
                    let band = bands[k - 1];
                    if (band.coverage >= 1.0 || band.coverage > thr) && band.pattern.is_none_or(|p| p.on(x, y)) {
                        return Some(band.packed);
                    }
                    k -= 1;
                }
                Some(self.packed)
            }
            Kind::Shader { f, b } => {
                let probe = Probe {
                    x: px,
                    y: py,
                    u: at.u,
                    v: at.v,
                    dist: at.dist,
                    normal: at.normal,
                    a: Color::from_packed(self.packed),
                    b: Color::from_packed(b),
                };
                let p = f(&probe)?;
                // The returned paint is read at the dot in this paint's frame.
                p.sample(x + p.anchor.0, y + p.anchor.1)
            }
        }
    }
}

/// Where an axis-aligned box lands under a canvas's transform.
enum Boxed {
    Same,
    Moved(f32, f32, f32, f32),
    Turned,
}

/// What a shape-relative paint knows about one dot of its shape.
#[derive(Clone, Copy)]
struct Shaped {
    x: i32,
    y: i32,
    u: f32,
    v: f32,
    dist: f32,
    normal: (f32, f32),
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

/// How a stroke is drawn: its width, and optionally a dash pattern.
///
/// Every stroking method takes `impl Into<Pen>`, and a bare `f32` is a solid pen of
/// that width, so `canvas.polyline(&pts, 2.0, paint)` and
/// `canvas.polyline(&pts, Pen::new(2.0).dash(4.0, 2.0), paint)` both work.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pen {
    /// Width in dots. Up to `1.0` the stroke is a one-dot Bresenham line; wider ones
    /// are quads with round joins and caps.
    pub width: f32,
    /// Dash pattern as `(on, off)` lengths in dots, `None` for a solid stroke.
    pub dash: Option<(f32, f32)>,
    /// Where in the dash pattern the stroke starts, in dots; animate it for a
    /// marching-ants effect.
    pub phase: f32,
}

impl Pen {
    /// A solid pen `width` dots wide.
    pub const fn new(width: f32) -> Self {
        Self { width, dash: None, phase: 0.0 }
    }

    /// Dashes `on` dots long with `off` dots between them. Each dash has the pen's
    /// round caps, so a dash shorter than the width is a dot.
    pub const fn dash(mut self, on: f32, off: f32) -> Self {
        self.dash = Some((on, off));
        self
    }

    /// A dotted pen: dashes as long as the pen is wide, spaced the same again.
    pub const fn dotted(self) -> Self {
        let w = if self.width > 1.0 { self.width } else { 1.0 };
        self.dash(w, w)
    }

    /// Starts the dash pattern `phase` dots in.
    pub const fn phase(mut self, phase: f32) -> Self {
        self.phase = phase;
        self
    }
}

impl From<f32> for Pen {
    fn from(width: f32) -> Self {
        Self::new(width)
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

/// One past the last dot whose centre is at or before `v`. A span `first(a)..last(b)`
/// takes the dots on both its edges, so a shape rounds the same way on its left and
/// its right (an exclusive `first(b)` would drop a centre that lands exactly on `b`).
#[inline]
fn last(v: f32) -> i32 {
    (v - 0.5).floor() as i32 + 1
}

#[inline]
pub(crate) fn dist(a: Point, b: Point) -> f32 {
    ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()
}

/// Segments to flatten a curve of roughly `len` dots into: about one per dot.
#[inline]
pub(crate) fn steps(len: f32) -> u32 {
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

thread_local! {
    /// A canvas kept per thread for drawing a shape before painting through it.
    static SCRATCH: RefCell<Option<Canvas>> = const { RefCell::new(None) };
}

/// Runs `f` with an empty scratch canvas of `cols × rows` cells and clears it after.
/// The canvas is kept per thread, so this allocates once per size change.
pub(crate) fn with_scratch<R>(cols: u16, rows: u16, f: impl FnOnce(&mut Canvas) -> R) -> R {
    let mut scratch = SCRATCH.with(|s| s.borrow_mut().take()).unwrap_or_else(|| Canvas::new(0, 0));
    if (scratch.cols(), scratch.rows()) != (cols, rows) {
        scratch = Canvas::new(cols, rows);
    }
    let out = f(&mut scratch);
    scratch.clear();
    scratch.transform = Transform::IDENTITY;
    SCRATCH.with(|s| *s.borrow_mut() = Some(scratch));
    out
}

impl Canvas {
    /// Paints dots `x0..x1` of row `y`, clipped to the canvas. This is the primitive
    /// every fill and stroke ends up in. It is not transformed by [`with`](Self::with).
    pub fn span(&mut self, y: i32, x0: i32, x1: i32, paint: Paint) {
        if y < 0 || y >= self.height() {
            return;
        }
        let (x0, x1) = (x0.max(0), x1.min(self.width()));
        if x0 >= x1 {
            return;
        }
        self.mark(x0, y, x1, y + 1);
        let start = y as usize * self.width() as usize + x0 as usize;
        let row = &mut self.dots[start..start + (x1 - x0) as usize];
        if paint.is_solid() {
            row.fill(paint.packed);
        } else if paint.coverage > 0.0 {
            for (i, d) in row.iter_mut().enumerate() {
                if let Some(v) = paint.sample(x0 + i as i32, y) {
                    *d = v;
                }
            }
        }
    }

    /// Runs `draw` with `paint`, or, for a shape-relative paint, runs it into a
    /// scratch mask and paints through that instead; see [`Paint`].
    pub(crate) fn shaped(&mut self, paint: Paint, draw: impl FnOnce(&mut Canvas, Paint)) {
        if !paint.needs_shape() {
            return draw(self, paint);
        }
        let transform = self.transform;
        with_scratch(self.cols(), self.rows(), |mask| {
            mask.transform = transform;
            draw(mask, Paint::new(Color::Foreground));
            if let Some((x0, y0, x1, y1)) = mask.dot_bounds() {
                self.stencil_box(&*mask, x0, y0, x1, y1, paint);
            }
        });
    }

    /// Paints every dot that `mask` covers: a [`Mask`](crate::Mask), or a canvas of
    /// the same size (its set dots and printed cells). This is how a shape-relative
    /// [`Paint`] is applied, and it works for any paint: the mask is the shape,
    /// whatever drew it. [`stencil_in`](Self::stencil_in) limits it to a box.
    pub fn stencil(&mut self, mask: &impl Silhouette, paint: impl Into<Paint>) {
        let Some((x0, y0, x1, y1)) = mask.extent() else { return };
        self.stencil_box(mask, x0, y0, x1, y1, paint.into());
    }

    /// [`stencil`](Self::stencil) within `area` only. The dots of `mask` outside it
    /// are neither painted nor looked at. The area is also the frame a
    /// shape-relative paint works in: a gradient runs across it, and an edge paint
    /// measures to its border as to the mask's own. It covers the dots
    /// [`fill_rect`](Self::fill_rect) would with the same box.
    ///
    /// ```
    /// use cobra::{Canvas, Paint, Rect, Rgb};
    ///
    /// let mut mask = Canvas::new(20, 5);
    /// mask.disc(10.0, 10.0, 6.0, Rgb::hex(0xffffff));
    /// let mut canvas = Canvas::new(20, 5);
    /// canvas.stencil_in(&mask, Rect::new(4.0, 4.0, 12.0, 12.0), Paint::edge(Rgb::hex(0x3aa0ff), Rgb::hex(0x0b2a4a), 3.0));
    /// ```
    pub fn stencil_in(&mut self, mask: &impl Silhouette, area: Rect, paint: impl Into<Paint>) {
        let (x0, y0) = (first(area.x), first(area.y));
        let (x1, y1) = (first(area.right()), first(area.bottom()));
        self.stencil_box(mask, x0, y0, x1, y1, paint.into());
    }

    /// Paints the dots of `mask` within `x0..x1 × y0..y1`, clipped to the canvas.
    fn stencil_box(&mut self, mask: &impl Silhouette, x0: i32, y0: i32, x1: i32, y1: i32, paint: Paint) {
        let (x0, y0) = (x0.max(0), y0.max(0));
        let (x1, y1) = (x1.min(self.width()), y1.min(self.height()));
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        self.mark(x0, y0, x1, y1);
        let stride = self.width() as usize;
        if !paint.needs_shape() {
            for y in y0..y1 {
                for x in x0..x1 {
                    if mask.covers(x, y)
                        && let Some(d) = paint.sample(x, y)
                    {
                        self.dots[y as usize * stride + x as usize] = d;
                    }
                }
            }
            return;
        }
        let (w, h) = ((x1 - x0) as f32, (y1 - y0) as f32);
        // Distance to the nearest uncovered dot, with a one-dot frame of uncovered
        // dots so the mask's edge (and the box's) counts as an edge; its gradient is
        // the surface normal.
        let gw = (x1 - x0 + 2) as usize;
        let field = matches!(paint.kind, Kind::Edge { .. } | Kind::Shader { .. } | Kind::Cel { .. }).then(|| {
            let mut grid = vec![0.0f32; gw * (y1 - y0 + 2) as usize];
            for y in y0..y1 {
                for x in x0..x1 {
                    grid[(y - y0 + 1) as usize * gw + (x - x0 + 1) as usize] =
                        if mask.covers(x, y) { INF } else { 0.0 };
                }
            }
            Edt::default().run(&mut grid, gw, (y1 - y0 + 2) as usize);
            for d in &mut grid {
                *d = d.sqrt();
            }
            grid
        });
        let shaped = |x: i32, y: i32| -> Shaped {
            let (u, v) = ((x - x0) as f32 / w, (y - y0) as f32 / h);
            let Some(f) = &field else { return Shaped { x, y, u, v, dist: -INF, normal: (0.0, 0.0) } };
            let at = |dx: i32, dy: i32| f[(y - y0 + 1 + dy) as usize * gw + (x - x0 + 1 + dx) as usize];
            let dist = -at(0, 0);
            // Distance grows inwards, so the outward normal is minus its gradient.
            let normal = unit((at(-1, 0) - at(1, 0), at(0, -1) - at(0, 1)));
            Shaped { x, y, u, v, dist, normal }
        };
        // Under `per_cell`, the covered dot nearest the cell's centre speaks for the
        // cell; these are the dots of a cell in that order.
        const NEAR_CENTRE: [(i32, i32); 8] = [(0, 1), (1, 1), (0, 2), (1, 2), (0, 0), (1, 0), (0, 3), (1, 3)];
        for y in y0..y1 {
            for x in x0..x1 {
                if !mask.covers(x, y) {
                    continue;
                }
                let at = if paint.per_cell {
                    let (cx, cy) = (x & !1, y & !3);
                    let mut rep = NEAR_CENTRE.iter().map(|&(dx, dy)| (cx + dx, cy + dy));
                    let (rx, ry) = rep
                        .find(|&(rx, ry)| (x0..x1).contains(&rx) && (y0..y1).contains(&ry) && mask.covers(rx, ry))
                        .unwrap_or((x, y));
                    shaped(rx, ry)
                } else {
                    shaped(x, y)
                };
                if let Some(d) = paint.sample_in(x, y, &at) {
                    self.dots[y as usize * stride + x as usize] = d;
                }
            }
        }
    }

    /// Keeps only the dots that `mask` covers: clips the canvas to a shape.
    pub fn clip(&mut self, mask: &impl Silhouette) {
        // Only the box that was written can hold a dot to unset.
        let Some((x0, y0, x1, y1)) = self.dirty else { return };
        let stride = self.width() as usize;
        for y in y0..y1 {
            let row = &mut self.dots[y as usize * stride..][..stride];
            for (x, d) in row[x0 as usize..x1 as usize].iter_mut().enumerate() {
                if *d != 0 && !mask.covers(x0 + x as i32, y) {
                    *d = 0;
                }
            }
        }
    }

    /// Unsets every dot that `mask` covers: cuts a shape out of the canvas.
    pub fn cut(&mut self, mask: &impl Silhouette) {
        let Some((x0, y0, x1, y1)) = mask.extent() else { return };
        let stride = self.width() as usize;
        for y in y0.max(0)..y1.min(self.height()) {
            for x in x0.max(0)..x1.min(self.width()) {
                if mask.covers(x, y) {
                    self.dots[y as usize * stride + x as usize] = 0;
                }
            }
        }
    }

    /// Draws everything in `f`, then keeps only what landed inside `mask`: a line
    /// clipped to a body, a texture that stops at a silhouette. `f` draws on a
    /// scratch canvas under the current transform, with any primitive and any paint;
    /// what it erases there does not erase here.
    ///
    /// ```
    /// use cobra::{Canvas, Mask, Rgb};
    ///
    /// let mut body = Mask::new(20, 5);
    /// body.draw(|c| c.fill_ellipse(20.0, 10.0, 14.0, 8.0, Rgb::hex(0)));
    /// let mut canvas = Canvas::new(20, 5);
    /// canvas.stencil(&body, Rgb::hex(0xffa657));
    /// // A crease that cannot escape the body however the points move.
    /// canvas.clipped(&body, |c| c.spline(&[(2.0, 4.0), (20.0, 14.0), (38.0, 6.0)], false, 1.0, Rgb::hex(0x7a3e00)));
    /// ```
    pub fn clipped(&mut self, mask: &impl Silhouette, f: impl FnOnce(&mut Canvas)) {
        let transform = self.transform;
        with_scratch(self.cols(), self.rows(), |scratch| {
            scratch.transform = transform;
            f(scratch);
            let (Some((x0, y0, x1, y1)), Some((mx0, my0, mx1, my1))) = (scratch.dot_bounds(), mask.extent()) else {
                return;
            };
            let (x0, y0, x1, y1) = (x0.max(mx0), y0.max(my0), x1.min(mx1), y1.min(my1));
            if x0 >= x1 || y0 >= y1 {
                return;
            }
            self.mark(x0, y0, x1, y1);
            let stride = self.width() as usize;
            for y in y0..y1 {
                for x in x0..x1 {
                    if let Some(c) = scratch.get(x, y)
                        && mask.covers(x, y)
                    {
                        self.dots[y as usize * stride + x as usize] = c.packed();
                    }
                }
            }
        });
    }

    /// Fills the axis-aligned box from `(x, y)` of size `w × h`.
    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, paint: impl Into<Paint>) {
        let paint = paint.into();
        match self.boxed(x, y, w, h) {
            Boxed::Same => {}
            Boxed::Moved(x, y, w, h) => return self.untransformed(|c| c.fill_rect(x, y, w, h, paint)),
            Boxed::Turned => {
                let mut p = Path::new();
                p.rect(x, y, w, h);
                return self.fill_path(&p, paint);
            }
        }
        self.shaped(paint, |c, paint| {
            let (x0, x1) = (first(x), first(x + w));
            for row in first(y)..first(y + h) {
                c.span(row, x0, x1, paint);
            }
        })
    }

    /// Where the box `(x, y, w, h)` lands under the transform: unchanged, another
    /// axis-aligned box, or somewhere a box cannot describe.
    fn boxed(&self, x: f32, y: f32, w: f32, h: f32) -> Boxed {
        let t = self.transform;
        if t.is_identity() {
            Boxed::Same
        } else if t.is_axis_aligned() {
            let (a, b) = (t.apply((x, y)), t.apply((x + w, y + h)));
            Boxed::Moved(a.0.min(b.0), a.1.min(b.1), (b.0 - a.0).abs(), (b.1 - a.1).abs())
        } else {
            Boxed::Turned
        }
    }

    /// Runs `draw` with the transform switched off, for a primitive that has already
    /// mapped its geometry.
    fn untransformed(&mut self, draw: impl FnOnce(&mut Canvas)) {
        let t = std::mem::replace(&mut self.transform, Transform::IDENTITY);
        draw(self);
        self.transform = t;
    }

    /// Outlines the box from `(x, y)` of size `w × h` with a border `width` dots
    /// thick, drawn inside the box so it covers the same dots as
    /// [`fill_rect`](Self::fill_rect) would.
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, width: f32, paint: impl Into<Paint>) {
        let paint = paint.into();
        match self.boxed(x, y, w, h) {
            Boxed::Same => {}
            Boxed::Moved(mx, my, mw, mh) => {
                let width = width * self.transform.scale_factor();
                return self.untransformed(|c| c.rect(mx, my, mw, mh, width, paint));
            }
            Boxed::Turned => {
                // The box minus the box inset by the border, even-odd.
                let t = width.max(1.0).min(w / 2.0).min(h / 2.0);
                let mut p = Path::new();
                p.rect(x, y, w, h).rect(x + t, y + t, w - 2.0 * t, h - 2.0 * t);
                return self.fill_path(&p, paint);
            }
        }
        self.shaped(paint, |c, paint| {
            let t = width.max(1.0).min(w / 2.0).min(h / 2.0);
            c.fill_rect(x, y, w, t, paint);
            c.fill_rect(x, y + h - t, w, t, paint);
            c.fill_rect(x, y + t, t, h - 2.0 * t, paint);
            c.fill_rect(x + w - t, y + t, t, h - 2.0 * t, paint);
        })
    }

    /// Fills the ellipse centred on `(cx, cy)` with radii `rx`, `ry`.
    pub fn fill_ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, paint: impl Into<Paint>) {
        let paint = paint.into();
        match self.boxed(cx - rx, cy - ry, 2.0 * rx, 2.0 * ry) {
            Boxed::Same => {}
            Boxed::Moved(x, y, w, h) => {
                return self.untransformed(|c| c.fill_ellipse(x + w / 2.0, y + h / 2.0, w / 2.0, h / 2.0, paint));
            }
            Boxed::Turned => {
                let mut p = Path::new();
                p.ellipse(cx, cy, rx, ry);
                return self.fill_path(&p, paint);
            }
        }
        self.shaped(paint, |c, paint| c.ellipse_spans(cx, cy, rx, ry, paint))
    }

    fn ellipse_spans(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, paint: Paint) {
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        for y in first(cy - ry)..first(cy + ry) {
            let ey = (y as f32 + 0.5 - cy) / ry;
            let half = rx * (1.0 - ey * ey).max(0.0).sqrt();
            self.span(y, first(cx - half), last(cx + half), paint);
        }
    }

    /// Strokes the ellipse centred on `(cx, cy)` with radii `rx`, `ry`.
    pub fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, pen: impl Into<Pen>, paint: impl Into<Paint>) {
        self.arc(cx, cy, rx, ry, 0.0, std::f32::consts::TAU, pen, paint);
    }

    /// Strokes the elliptical arc from angle `a0` to `a1` (radians, clockwise on
    /// screen since `y` grows downwards).
    #[allow(clippy::too_many_arguments)]
    pub fn arc(
        &mut self,
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        a0: f32,
        a1: f32,
        pen: impl Into<Pen>,
        paint: impl Into<Paint>,
    ) {
        let pen = pen.into();
        self.shaped(paint.into(), |c, paint| {
            let n = steps((a1 - a0).abs() * rx.max(ry));
            let pts = (0..=n).map(move |i| {
                let a = a0 + (a1 - a0) * i as f32 / n as f32;
                (cx + rx * a.cos(), cy + ry * a.sin())
            });
            c.stroke(pts, false, pen, paint);
        })
    }

    /// Strokes straight segments through `pts`.
    pub fn polyline(&mut self, pts: &[Point], pen: impl Into<Pen>, paint: impl Into<Paint>) {
        let pen = pen.into();
        self.shaped(paint.into(), |c, paint| c.stroke(pts.iter().copied(), false, pen, paint))
    }

    /// Strokes the closed outline through `pts`.
    pub fn polygon(&mut self, pts: &[Point], pen: impl Into<Pen>, paint: impl Into<Paint>) {
        let pen = pen.into();
        self.shaped(paint.into(), |c, paint| c.stroke(pts.iter().copied(), true, pen, paint))
    }

    /// Fills the polygon with vertices `pts` (any shape, even-odd rule).
    pub fn fill_polygon(&mut self, pts: &[Point], paint: impl Into<Paint>) {
        self.shaped(paint.into(), |c, paint| {
            if pts.len() < 3 {
                return;
            }
            // A scan line crosses at most one edge per vertex, so the crossings of any
            // generated shape fit on the stack; only a caller's huge polygon allocates.
            let mut stack = [0.0f32; MAX_POINTS];
            let mut heap = Vec::new();
            let xs: &mut [f32] = if pts.len() <= MAX_POINTS {
                &mut stack[..pts.len()]
            } else {
                heap.resize(pts.len(), 0.0);
                &mut heap
            };
            let edges = pts.iter().enumerate().map(|(i, &a)| (a, pts[(i + 1) % pts.len()]));
            c.fill_edges(edges, xs, paint);
        })
    }

    /// Even-odd scanline fill of the shape bounded by `edges`, with `xs` as room for
    /// one crossing per edge.
    fn fill_edges(&mut self, edges: impl Iterator<Item = (Point, Point)> + Clone, xs: &mut [f32], paint: Paint) {
        let t = self.transform;
        let edges = edges.map(move |(a, b)| if t.is_identity() { (a, b) } else { (t.apply(a), t.apply(b)) });
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for (a, _) in edges.clone() {
            lo = lo.min(a.1);
            hi = hi.max(a.1);
        }
        for y in first(lo).max(0)..first(hi).min(self.height()) {
            let sy = y as f32 + 0.5;
            let mut n = 0;
            for (a, b) in edges.clone() {
                if (a.1 <= sy) != (b.1 <= sy) {
                    xs[n] = a.0 + (sy - a.1) * (b.0 - a.0) / (b.1 - a.1);
                    n += 1;
                }
            }
            xs[..n].sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            for pair in xs[..n].as_chunks::<2>().0 {
                self.span(y, first(pair[0]), last(pair[1]), paint);
            }
        }
    }

    /// Fills a [`Path`]: every subpath is closed and the even-odd rule decides what
    /// is inside, so a subpath drawn inside another is a hole.
    pub fn fill_path(&mut self, path: &Path, paint: impl Into<Paint>) {
        self.shaped(paint.into(), |c, paint| {
            let mut edges = Vec::new();
            path.subpaths(|pts, _| {
                edges.extend(pts.iter().enumerate().map(|(i, &a)| (a, pts[(i + 1) % pts.len()])));
            });
            let mut xs = vec![0.0f32; edges.len()];
            c.fill_edges(edges.iter().copied(), &mut xs, paint);
        })
    }

    /// Strokes a [`Path`]: each subpath as a polyline, closed where the path closed it.
    pub fn stroke_path(&mut self, path: &Path, pen: impl Into<Pen>, paint: impl Into<Paint>) {
        let pen = pen.into();
        self.shaped(paint.into(), |c, paint| {
            path.subpaths(|pts, closed| c.stroke(pts.iter().copied(), closed, pen, paint))
        })
    }

    /// Strokes a Bézier curve with control points `ctrl` (three for quadratic, four
    /// for cubic, up to sixteen).
    pub fn bezier(&mut self, ctrl: &[Point], pen: impl Into<Pen>, paint: impl Into<Paint>) {
        let pen = pen.into();
        self.shaped(paint.into(), |c, paint| {
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
            c.stroke(pts, false, pen, paint);
        })
    }

    /// Strokes a smooth curve through every point of `pts` (Catmull–Rom), open or
    /// closed into a loop.
    pub fn spline(&mut self, pts: &[Point], closed: bool, pen: impl Into<Pen>, paint: impl Into<Paint>) {
        let pen = pen.into();
        self.shaped(paint.into(), |c, paint| {
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
                (if s == 0 { 0 } else { 1 }..=m).map(move |i| catmull_rom(p0, p1, p2, p3, i as f32 / m as f32))
            });
            c.stroke(pts, closed, pen, paint);
        })
    }

    /// Fills the box from `(x, y)` of size `w × h` with corners rounded to radius `r`.
    pub fn fill_round_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, paint: impl Into<Paint>) {
        let paint = paint.into();
        match self.boxed(x, y, w, h) {
            Boxed::Same => {}
            Boxed::Moved(mx, my, mw, mh) => {
                let r = r * self.transform.scale_factor();
                return self.untransformed(|c| c.fill_round_rect(mx, my, mw, mh, r, paint));
            }
            Boxed::Turned => {
                let mut p = Path::new();
                p.round_rect(x, y, w, h, r);
                return self.fill_path(&p, paint);
            }
        }
        self.shaped(paint, |c, paint| {
            for row in first(y)..first(y + h) {
                if let Some((a, b)) = round_rect_span(row as f32 + 0.5, x, y, w, h, r) {
                    c.span(row, first(a), first(b), paint);
                }
            }
        })
    }

    /// Outlines a rounded box with a border `width` dots thick, drawn inside it.
    #[allow(clippy::too_many_arguments)]
    pub fn round_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, width: f32, paint: impl Into<Paint>) {
        let paint = paint.into();
        match self.boxed(x, y, w, h) {
            Boxed::Same => {}
            Boxed::Moved(mx, my, mw, mh) => {
                let k = self.transform.scale_factor();
                return self.untransformed(|c| c.round_rect(mx, my, mw, mh, r * k, width * k, paint));
            }
            Boxed::Turned => {
                let t = width.max(1.0).min(w / 2.0).min(h / 2.0);
                let mut p = Path::new();
                p.round_rect(x, y, w, h, r).round_rect(x + t, y + t, w - 2.0 * t, h - 2.0 * t, r - t);
                return self.fill_path(&p, paint);
            }
        }
        self.shaped(paint, |c, paint| {
            let t = width.max(1.0).min(w / 2.0).min(h / 2.0);
            for row in first(y)..first(y + h) {
                let sy = row as f32 + 0.5;
                let Some((a, b)) = round_rect_span(sy, x, y, w, h, r) else { continue };
                // The hole is the same shape inset by the border; rows above and below it
                // are solid.
                match round_rect_span(sy, x + t, y + t, w - 2.0 * t, h - 2.0 * t, r - t) {
                    Some((ia, ib)) => {
                        c.span(row, first(a), first(ia), paint);
                        c.span(row, first(ib), first(b), paint);
                    }
                    None => c.span(row, first(a), first(b), paint),
                }
            }
        })
    }

    /// Fills the ring between radii `inner` and `outer`, centred on `(cx, cy)`.
    pub fn ring(&mut self, cx: f32, cy: f32, outer: f32, inner: f32, paint: impl Into<Paint>) {
        let paint = paint.into();
        if !self.transform.is_identity() {
            let mut p = Path::new();
            p.ellipse(cx, cy, outer, outer).ellipse(cx, cy, inner.min(outer), inner.min(outer));
            return self.fill_path(&p, paint);
        }
        self.shaped(paint, |c, paint| {
            let (outer, inner) = (outer.max(0.0), inner.clamp(0.0, outer));
            for y in first(cy - outer)..first(cy + outer) {
                let dy = (y as f32 + 0.5 - cy).abs();
                let ho = (outer * outer - dy * dy).max(0.0).sqrt();
                let hi = (inner * inner - dy * dy).max(0.0).sqrt();
                if hi > 0.0 {
                    c.span(y, first(cx - ho), last(cx - hi), paint);
                    c.span(y, first(cx + hi), last(cx + ho), paint);
                } else {
                    c.span(y, first(cx - ho), last(cx + ho), paint);
                }
            }
        })
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
    pub fn fill_ngon(&mut self, cx: f32, cy: f32, r: f32, sides: u32, rot: f32, paint: impl Into<Paint>) {
        let (pts, n) = ngon_points(cx, cy, r, r, sides, rot);
        self.fill_polygon(&pts[..n], paint);
    }

    /// Strokes the outline of [`fill_ngon`](Self::fill_ngon).
    #[allow(clippy::too_many_arguments)]
    pub fn ngon(
        &mut self,
        cx: f32,
        cy: f32,
        r: f32,
        sides: u32,
        rot: f32,
        pen: impl Into<Pen>,
        paint: impl Into<Paint>,
    ) {
        let (pts, n) = ngon_points(cx, cy, r, r, sides, rot);
        self.polygon(&pts[..n], pen, paint);
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
        pen: impl Into<Pen>,
        paint: impl Into<Paint>,
    ) {
        let (pts, n) = ngon_points(cx, cy, outer, inner, points.clamp(2, 32) * 2, rot);
        self.polygon(&pts[..n], pen, paint);
    }

    /// Draws an arrow from `from` to `to`: a shaft `pen` wide and a filled head `head`
    /// dots long, whose tip is exactly `to`.
    pub fn arrow(&mut self, from: Point, to: Point, pen: impl Into<Pen>, head: f32, paint: impl Into<Paint>) {
        let pen = pen.into();
        self.shaped(paint.into(), |c, paint| {
            let len = dist(from, to);
            if len <= 0.0 {
                return;
            }
            let head = head.max(pen.width).min(len);
            let (ux, uy) = ((to.0 - from.0) / len, (to.1 - from.1) / len);
            // The shaft stops where the head starts, so a dithered arrow does not paint
            // the overlap twice.
            let base = (to.0 - ux * head, to.1 - uy * head);
            c.stroke([from, base].into_iter(), false, pen, paint);
            let (nx, ny) = (-uy * head * 0.4, ux * head * 0.4);
            c.fill_polygon(&[to, (base.0 + nx, base.1 + ny), (base.0 - nx, base.1 - ny)], paint);
        })
    }

    /// Strokes the path through `pts`: Bresenham lines for `width ≤ 1`, otherwise a
    /// quad per segment with round joins and caps; dashed if the pen says so.
    pub(crate) fn stroke(&mut self, pts: impl Iterator<Item = Point>, closed: bool, pen: Pen, paint: Paint) {
        let t = self.transform;
        let (pen, dash) = if t.is_identity() {
            (pen, pen.dash)
        } else {
            let k = t.scale_factor();
            let dash = pen.dash.map(|(on, off)| (on * k, off * k));
            (Pen { width: pen.width * k, dash, phase: pen.phase * k }, dash)
        };
        let pts = pts.map(move |p| if t.is_identity() { p } else { t.apply(p) });
        let dash = dash.filter(|&(on, off)| on > 0.0 && off > 0.0);
        let mut along = pen.phase;
        let mut first_pt = None;
        let mut prev: Option<Point> = None;
        for p in pts {
            match (prev, dash) {
                (Some(q), Some(d)) => self.dashes(q, p, pen.width, d, &mut along, paint),
                (Some(q), None) => self.segment(q, p, pen.width, paint),
                (None, None) => {
                    first_pt = Some(p);
                    if pen.width > 1.0 {
                        self.ellipse_spans(p.0, p.1, pen.width / 2.0, pen.width / 2.0, paint);
                    }
                }
                (None, Some(_)) => first_pt = Some(p),
            }
            prev = Some(p);
        }
        if let (true, Some(a), Some(b)) = (closed, prev, first_pt) {
            match dash {
                Some(d) => self.dashes(a, b, pen.width, d, &mut along, paint),
                None => self.segment(a, b, pen.width, paint),
            }
        }
    }

    /// The dashes of the segment `a → b`, continuing the pattern from `along` dots
    /// into it (and leaving it there for the next segment).
    fn dashes(&mut self, a: Point, b: Point, width: f32, (on, off): (f32, f32), along: &mut f32, paint: Paint) {
        let len = dist(a, b);
        if len <= 0.0 {
            return;
        }
        let (ux, uy) = ((b.0 - a.0) / len, (b.1 - a.1) / len);
        let period = on + off;
        let mut pos = 0.0;
        while pos < len {
            let phase = along.rem_euclid(period);
            let run = if phase < on { (on - phase).min(len - pos) } else { (period - phase).min(len - pos) };
            if phase < on {
                // A thin dash stops short of its end so it lights `on` dots, not one more.
                let end = pos + run - if width <= 1.0 { 1e-3 } else { 0.0 };
                let (p, q) = ((a.0 + ux * pos, a.1 + uy * pos), (a.0 + ux * end, a.1 + uy * end));
                if width > 1.0 {
                    self.ellipse_spans(p.0, p.1, width / 2.0, width / 2.0, paint);
                }
                self.segment(p, q, width, paint);
            }
            pos += run;
            *along += run;
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
        self.ellipse_spans(b.0, b.1, hw, hw, paint);
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
                self.span(y, first(xmin), last(xmax), paint);
            }
        }
    }

    /// Bresenham line with a [`Paint`] (so it can dither or erase).
    pub(crate) fn line_paint(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, paint: Paint) {
        self.bresenham(x0, y0, x1, y1, |c, x, y| c.span(y, x, x + 1, paint));
    }
}

/// The point at `t` of the Catmull–Rom segment from `p1` to `p2`, with `p0` and `p3`
/// steering its ends.
#[inline]
pub(crate) fn catmull_rom(p0: Point, p1: Point, p2: Point, p3: Point, t: f32) -> Point {
    let (t2, t3) = (t * t, t * t * t);
    let f = |a: f32, b: f32, c: f32, d: f32| {
        0.5 * (2.0 * b + (c - a) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (3.0 * b - a - 3.0 * c + d) * t3)
    };
    (f(p0.0, p1.0, p2.0, p3.0), f(p0.1, p1.1, p2.1, p3.1))
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

    #[test]
    fn patterns_and_anchors() {
        let mut c = Canvas::new(4, 2);
        c.fill_rect(0.0, 0.0, 8.0, 8.0, Paint::pattern(C, Pattern::Rows(2)));
        assert!(c.get(3, 0).is_some() && c.get(3, 1).is_none() && c.get(3, 2).is_some());
        let mut d = Canvas::new(4, 2);
        d.fill_rect(0.0, 0.0, 8.0, 8.0, Paint::pattern(C, Pattern::Rows(2)).anchor(0, 1));
        assert!(d.get(3, 0).is_none() && d.get(3, 1).is_some(), "the anchor shifts the pattern");
        assert!(Pattern::Checker(2).on(0, 0) && !Pattern::Checker(2).on(2, 0) && Pattern::Checker(2).on(2, 2));
        assert!(Pattern::Diagonal(4).on(5, 1) && Pattern::Antidiagonal(4).on(3, 1) && Pattern::Cross(4).on(5, 1));
        assert!(Pattern::Grid(3).on(3, 1) && Pattern::Grid(3).on(1, 3) && !Pattern::Grid(3).on(1, 1));
        assert!(Pattern::Dots(4).on(0, 0) && Pattern::Dots(4).on(2, 4) && !Pattern::Dots(4).on(0, 4));
        assert!(Pattern::Columns(0).on(7, 7), "a period of zero is one");
        // A dither anchored at the shape moves with it.
        let (mut a, mut b) = (Canvas::new(4, 2), Canvas::new(4, 2));
        a.fill_rect(0.0, 0.0, 4.0, 4.0, Paint::dithered(C, 0.5).anchor(0, 0));
        b.fill_rect(1.0, 1.0, 4.0, 4.0, Paint::dithered(C, 0.5).anchor(1, 1));
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(a.get(x, y).is_some(), b.get(x + 1, y + 1).is_some(), "{x},{y}");
            }
        }
        assert_eq!(count(&a), 8);
    }

    #[test]
    fn gradients_blend_and_dither() {
        let (black, white) = (Rgb::hex(0), Rgb::hex(0xffffff));
        let mut c = Canvas::new(8, 1);
        c.fill_rect(0.0, 0.0, 16.0, 1.0, Paint::linear((0.0, 0.0), (16.0, 0.0), black, white));
        let grey = |x: i32| match c.get(x, 0) {
            Some(Color::Rgb(g)) => g.r,
            other => panic!("{other:?}"),
        };
        assert!(grey(0) < 20 && grey(15) > 235 && grey(4) < grey(8) && grey(8) < grey(12));
        let mut r = Canvas::new(8, 4);
        r.disc(8.0, 8.0, 8.0, Paint::radial((8.0, 8.0), 8.0, white, black));
        let at = |x: i32, y: i32| match r.get(x, y) {
            Some(Color::Rgb(g)) => g.r,
            other => panic!("{other:?}"),
        };
        assert!(at(8, 8) > 200 && at(8, 1) < 60 && at(8, 4) > at(8, 2));
        // Palette colours cannot blend, so they dither between the two.
        let mut p = Canvas::new(8, 1);
        p.fill_rect(0.0, 0.0, 16.0, 4.0, Paint::linear((0.0, 0.0), (16.0, 0.0), Color::Indexed(1), Color::Indexed(2)));
        let ones =
            (0..16).flat_map(|x| (0..4).map(move |y| (x, y))).filter(|&(x, y)| p.get(x, y) == Some(Color::Indexed(1)));
        let ones: Vec<(i32, i32)> = ones.collect();
        assert!(!ones.is_empty() && ones.len() < 64 && ones.iter().all(|&(x, _)| x < 16));
        assert!(ones.iter().filter(|&&(x, _)| x < 8).count() > ones.iter().filter(|&&(x, _)| x >= 8).count());
        assert_eq!(count(&p), 64, "every dot is one of the two");
        // Coverage still thins any paint.
        let mut d = Canvas::new(8, 1);
        d.fill_rect(0.0, 0.0, 16.0, 4.0, Paint::linear((0.0, 0.0), (16.0, 0.0), black, white).dither(0.5));
        assert_eq!(count(&d), 32);
    }

    #[test]
    fn edge_and_shader_paints_know_the_shape() {
        let (a, b) = (Rgb::hex(0xff0000), Rgb::hex(0x0000ff));
        let mut c = Canvas::new(8, 4);
        c.disc(8.0, 8.0, 7.0, Paint::edge(a, b, 3.0));
        assert_eq!(c.get(8, 1), Some(Color::Rgb(a)), "the edge is `a`");
        assert_eq!(c.get(8, 8), Some(Color::Rgb(b)), "deep inside is `b`");
        let mid = match c.get(8, 3) {
            Some(Color::Rgb(g)) => g,
            other => panic!("{other:?}"),
        };
        assert!(mid.r > 0 && mid.b > 0, "between them it blends: {mid:?}");
        assert!((count(&c) as f32 - std::f32::consts::PI * 49.0).abs() < 10.0, "the shape is the same disc");

        let mut s = Canvas::new(8, 4);
        s.fill_rect(2.0, 2.0, 12.0, 12.0, Paint::shader(a, b, |p| (p.u < 0.5).then(|| p.mix(p.v))));
        assert_eq!(s.get(2, 2), Some(Color::Rgb(a)), "top left: u=0, v=0");
        assert!(s.get(13, 13).is_none(), "the right half was left alone");
        let low = match s.get(2, 13) {
            Some(Color::Rgb(g)) => g,
            other => panic!("{other:?}"),
        };
        assert!(low.r < 30 && low.b > 225, "v runs down: {low:?}");
        // A shader on a stroke sees the stroke as the shape.
        let mut t = Canvas::new(8, 4);
        t.polyline(
            &[(0.0, 8.0), (16.0, 8.0)],
            4.0,
            Paint::shader(a, b, |p| Some(p.mix((-p.dist > 1.5) as i32 as f32))),
        );
        assert_eq!(t.get(8, 6), Some(Color::Rgb(a)), "the rim of the stroke");
        assert_eq!(t.get(8, 8), Some(Color::Rgb(b)), "its core");
    }

    #[test]
    fn stencils_clip_and_cut() {
        let mut mask = Canvas::new(4, 2);
        mask.fill_rect(2.0, 2.0, 4.0, 4.0, C);
        let mut c = Canvas::new(4, 2);
        c.stencil(&mask, Rgb::hex(0xff0000));
        assert_eq!(count(&c), 16);
        assert_eq!(c.get(2, 2), Some(Color::Rgb(Rgb::hex(0xff0000))));
        let mut full = Canvas::new(4, 2);
        full.fill_rect(0.0, 0.0, 8.0, 8.0, C);
        let mut clipped = full.clone();
        clipped.clip(&mask);
        c.stencil(&mask, C);
        assert_eq!(clipped, c, "clipping keeps only the mask");
        let mut cut = full.clone();
        cut.cut(&mask);
        assert_eq!(count(&cut), 64 - 16);
        assert!(cut.get(2, 2).is_none() && cut.get(0, 0).is_some());
        // A mask of another size is clipped to this canvas.
        let mut big = Canvas::new(10, 10);
        big.fill_rect(0.0, 0.0, 40.0, 40.0, C);
        let mut small = Canvas::new(2, 1);
        small.stencil(&big, C);
        assert_eq!(count(&small), 16);
    }

    #[test]
    fn a_stencil_within_an_area_ignores_the_rest_of_the_mask() {
        let mut mask = Canvas::new(8, 2);
        mask.fill_rect(0.0, 0.0, 16.0, 8.0, C);
        let mut c = Canvas::new(8, 2);
        c.stencil_in(&mask, Rect::new(2.0, 0.0, 4.0, 8.0), Rgb::hex(0xff0000));
        assert_eq!(count(&c), 32, "four columns of eight");
        assert!(c.get(1, 0).is_none() && c.get(2, 0).is_some() && c.get(5, 0).is_some() && c.get(6, 0).is_none());
        let mut whole = Canvas::new(8, 2);
        whole.fill_rect(2.0, 0.0, 4.0, 8.0, Rgb::hex(0xff0000));
        assert_eq!(c, whole, "the area rounds to dots as fill_rect does");
        // A shape-relative paint takes the area as its frame: a gradient across the
        // area, an edge paint measuring to the area's border.
        let mut g = Canvas::new(8, 2);
        g.stencil_in(
            &mask,
            Rect::new(4.0, 0.0, 8.0, 8.0),
            Paint::linear((4.5, 0.0), (11.5, 0.0), Rgb::hex(0), Rgb::hex(0xff)),
        );
        assert_eq!(g.get(4, 0), Some(Color::Rgb(Rgb::hex(0))));
        assert_eq!(g.get(11, 0), Some(Color::Rgb(Rgb::hex(0xff))));
        let mut e = Canvas::new(8, 2);
        e.stencil_in(&mask, Rect::new(4.0, 0.0, 8.0, 8.0), Paint::edge(Rgb::hex(0xff), Rgb::hex(0), 2.0));
        assert_eq!(e.get(4, 3), Some(Color::Rgb(Rgb::hex(0xff))), "the area's left edge is an edge");
        assert_eq!(e.get(7, 3), Some(Color::Rgb(Rgb::hex(0))), "and its middle is deep inside");
        // Off both canvases is clipped, and an empty area paints nothing.
        let mut off = Canvas::new(8, 2);
        off.stencil_in(&mask, Rect::new(-10.0, -10.0, 100.0, 100.0), C);
        assert_eq!(count(&off), 128, "the whole 16×8 mask");
        off.clear();
        off.stencil_in(&mask, Rect::new(3.0, 3.0, 0.0, 5.0), C);
        assert_eq!(count(&off), 0);
    }

    #[test]
    fn dashed_pens() {
        let mut c = Canvas::new(10, 1);
        c.polyline(&[(0.0, 1.0), (19.0, 1.0)], Pen::new(1.0).dash(2.0, 2.0), C);
        let lit: Vec<bool> = (0..20).map(|x| c.get(x, 1).is_some()).collect();
        assert_eq!(&lit[..8], &[true, true, false, false, true, true, false, false], "{lit:?}");
        let mut w = Canvas::new(10, 3);
        w.polyline(&[(0.0, 6.0), (40.0, 6.0)], Pen::new(3.0).dash(6.0, 6.0), C);
        assert!(w.get(3, 4).is_some() && w.get(3, 6).is_some() && w.get(3, 8).is_none(), "wide dashes have width");
        assert!(w.get(9, 6).is_none() && w.get(14, 6).is_some(), "and gaps");
        // The pattern carries on around corners and along closed outlines.
        let mut sq = Canvas::new(10, 5);
        sq.polygon(&[(2.0, 2.0), (17.0, 2.0), (17.0, 17.0), (2.0, 17.0)], Pen::new(1.0).dash(3.0, 3.0), C);
        let top = (2..17).filter(|&x| sq.get(x, 2).is_some()).count();
        let right = (2..17).filter(|&y| sq.get(17, y).is_some()).count();
        assert!((6..=9).contains(&top) && (6..=9).contains(&right), "{top} {right}");
        assert_eq!(Pen::new(2.0).dotted().dash, Some((2.0, 2.0)));
        assert_eq!(Pen::from(1.5).width, 1.5);
        let mut off = Canvas::new(10, 1);
        off.polyline(&[(0.0, 1.0), (19.0, 1.0)], Pen::new(1.0).dash(2.0, 0.0), C);
        assert_eq!(count(&off), 20, "a dash without a gap is a solid line");
    }

    #[test]
    fn normals_face_outwards_and_cel_bands_follow_them() {
        let (dark, mid, light) = (Rgb::hex(0x100000), Rgb::hex(0x800000), Rgb::hex(0xff0000));
        let mut c = Canvas::new(10, 5);
        c.disc(10.0, 10.0, 9.0, Paint::cel(dark, (-1.0, -1.0), &[(-0.3, mid), (0.4, light)]));
        let at = |x: i32, y: i32| c.get(x, y);
        assert_eq!(at(4, 4), Some(Color::Rgb(light)), "the upper left faces the light");
        assert_eq!(at(15, 15), Some(Color::Rgb(dark)), "the lower right faces away");
        assert_eq!(at(15, 4), Some(Color::Rgb(mid)), "side on is the middle band");
        assert_eq!(at(4, 15), Some(Color::Rgb(mid)));
        // A shader sees the same normal and can light the shape itself.
        let mut n = Canvas::new(10, 5);
        n.disc(10.0, 10.0, 9.0, Paint::shader(dark, light, |p| Some(p.mix((p.lit((0.0, -1.0)) + 1.0) / 2.0))));
        let red = |x: i32, y: i32| match n.get(x, y) {
            Some(Color::Rgb(g)) => g.r,
            other => panic!("{other:?}"),
        };
        assert!(red(10, 2) > red(10, 8) && red(10, 8) > red(10, 17), "lit from above: brighter at the top");
        // Bands and their paints carry coverage and patterns.
        let mut h = Canvas::new(10, 5);
        let hatch = Paint::pattern(light, Pattern::Rows(2));
        h.disc(10.0, 10.0, 9.0, Paint::cel(dark, (-1.0, -1.0), &[(0.3, hatch)]));
        assert_eq!(h.get(4, 4), Some(Color::Rgb(light)));
        assert_eq!(h.get(4, 5), Some(Color::Rgb(dark)), "the hatched band leaves its off rows to the base");
        assert_eq!(
            Paint::cel(dark, (1.0, 0.0), &[(0.0, mid); 9]).color(),
            Some(Color::Rgb(dark)),
            "extra bands are dropped"
        );
    }

    #[test]
    fn per_cell_paints_change_only_on_cell_boundaries() {
        let (a, b) = (Rgb::hex(0x000000), Rgb::hex(0xffffff));
        let mut c = Canvas::new(10, 5);
        c.fill_rect(0.0, 0.0, 20.0, 20.0, Paint::linear((0.0, 0.0), (20.0, 0.0), a, b).per_cell());
        for y in 0..20 {
            for x in (0..20).step_by(2) {
                assert_eq!(c.get(x, y), c.get(x + 1, y), "{x},{y}: both columns of a cell agree");
                assert_eq!(c.get(x, y & !3), c.get(x, y), "{x},{y}: all four rows of a cell agree");
            }
        }
        assert_ne!(c.get(0, 0), c.get(18, 0), "the gradient still runs");
        // The coverage dither stays per dot.
        let mut d = Canvas::new(10, 5);
        d.fill_rect(0.0, 0.0, 20.0, 20.0, Paint::edge(a, b, 4.0).per_cell().dither(0.5));
        assert_eq!(count(&d), 200);
    }

    #[test]
    fn hashed_dither_keeps_the_count_and_loses_the_lattice() {
        let mut ordered = Canvas::new(20, 10);
        ordered.fill_rect(0.0, 0.0, 40.0, 40.0, Paint::dithered(C, 0.25));
        let mut hashed = Canvas::new(20, 10);
        hashed.fill_rect(0.0, 0.0, 40.0, 40.0, Paint::dithered(C, 0.25).hashed());
        let (o, h) = (count(&ordered) as i32, count(&hashed) as i32);
        assert_eq!(o, 400);
        assert!((h - 400).abs() < 60, "about a quarter of 1600: {h}");
        // Every 4×4 tile of an ordered dither is the same; hashed ones differ.
        let tile = |c: &Canvas, tx: i32, ty: i32| {
            (0..16).map(|i| c.get(tx * 4 + i % 4, ty * 4 + i / 4).is_some() as u32).sum::<u32>()
        };
        assert!((0..10).all(|t| tile(&ordered, t, t) == 4));
        assert!((0..10).any(|t| tile(&hashed, t, t) != 4));
        // The anchor moves the hash with the shape.
        let (mut p, mut q) = (Canvas::new(20, 10), Canvas::new(20, 10));
        p.fill_rect(0.0, 0.0, 8.0, 8.0, Paint::dithered(C, 0.3).hashed().anchor(0, 0));
        q.fill_rect(5.0, 3.0, 8.0, 8.0, Paint::dithered(C, 0.3).hashed().anchor(5, 3));
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(p.get(x, y).is_some(), q.get(x + 5, y + 3).is_some());
            }
        }
    }

    #[test]
    fn soft_cel_edges_dither_between_bands() {
        let (a, b) = (Rgb::hex(0x000000), Rgb::hex(0xffffff));
        let mut hard = Canvas::new(12, 6);
        hard.disc(12.0, 12.0, 11.0, Paint::cel(a, (1.0, 0.0), &[(0.0, b)]));
        let mut soft = Canvas::new(12, 6);
        soft.disc(12.0, 12.0, 11.0, Paint::cel(a, (1.0, 0.0), &[(0.0, b)]).soften(0.4));
        let white = |c: &Canvas, x: i32| (0..24).filter(|&y| c.get(x, y) == Some(Color::Rgb(b))).count();
        assert_eq!(white(&hard, 9), 0, "left of the terminator nothing is lit");
        assert!(white(&hard, 11) <= 4, "only the ridge at the centre is undecided: {}", white(&hard, 11));
        assert!(white(&hard, 13) > 18);
        // Softened, the columns near it hold both colours.
        assert!(white(&soft, 10) > 0 && white(&soft, 10) < 20, "{}", white(&soft, 10));
        assert!(white(&soft, 14) > white(&soft, 10));
        assert_eq!(white(&soft, 2), 0, "far from it, the bands are as before");
        assert!(white(&soft, 20) > 10, "{}", white(&soft, 20));
    }

    #[test]
    fn clipped_drawing_stays_inside_the_mask() {
        let mut mask = Canvas::new(10, 5);
        mask.fill_rect(4.0, 4.0, 8.0, 8.0, C);
        let mut c = Canvas::new(10, 5);
        c.clipped(&mask, |c| {
            c.polyline(&[(0.0, 8.0), (19.0, 8.0)], 1.0, Rgb::hex(0xff0000));
            c.fill_rect(0.0, 0.0, 20.0, 2.0, C);
            c.fill_rect(4.0, 4.0, 2.0, 2.0, Paint::erase()); // only the scratch is erased
        });
        assert_eq!(count(&c), 8, "the line alone; the erase never touched it");
        assert_eq!(c.get(4, 8), Some(Color::Rgb(Rgb::hex(0xff0000))));
        assert!(c.get(3, 8).is_none() && c.get(12, 8).is_none() && c.get(5, 0).is_none());
        let mut d = Canvas::new(10, 5);
        d.fill_rect(0.0, 0.0, 20.0, 20.0, C);
        d.clipped(&mask, |c| c.fill_rect(0.0, 0.0, 20.0, 20.0, Paint::erase()));
        assert_eq!(count(&d), 400, "erasing inside a clip erases nothing here");
    }

    #[test]
    fn transformed_drawing() {
        use crate::Transform;
        // A translation and a flip land every primitive where a hand-moved one would.
        let (mut moved, mut byhand) = (Canvas::new(12, 6), Canvas::new(12, 6));
        moved.with(Transform::at(10.0, 8.0).flip_x(), |c| {
            c.fill_rect(0.0, 0.0, 4.0, 2.0, C);
            c.disc(-6.0, 3.0, 2.0, C);
            c.polyline(&[(0.0, 5.0), (8.0, 9.0)], 1.0, C);
            c.fill_polygon(&[(-2.0, 10.0), (-8.0, 10.0), (-5.0, 14.0)], C);
            c.set(3, 12, C);
            c.line(0, 15, 6, 15, C);
        });
        byhand.fill_rect(6.0, 8.0, 4.0, 2.0, C);
        byhand.disc(16.0, 11.0, 2.0, C);
        byhand.polyline(&[(10.0, 13.0), (2.0, 17.0)], 1.0, C);
        byhand.fill_polygon(&[(12.0, 18.0), (18.0, 18.0), (15.0, 22.0)], C);
        byhand.set(6, 20, C);
        byhand.line(9, 23, 3, 23, C);
        assert_eq!(moved, byhand);
        assert!(moved.transform().is_identity(), "the transform is restored after `with`");
        // A rotation turns a box into a diamond, with the same area.
        let mut turned = Canvas::new(12, 6);
        turned.with(Transform::at(12.0, 12.0).rotate(std::f32::consts::FRAC_PI_4), |c| {
            c.fill_rect(-6.0, -6.0, 12.0, 12.0, C)
        });
        assert!(turned.get(12, 4).is_some() && turned.get(12, 19).is_some() && turned.get(5, 5).is_none());
        assert!((count(&turned) as i32 - 144).abs() < 16, "{}", count(&turned));
        // A scale widens strokes, and nested transforms compose inner first.
        let mut thick = Canvas::new(12, 6);
        thick.with(Transform::IDENTITY.scale(3.0, 3.0), |c| {
            c.polyline(&[(1.0, 2.0), (7.0, 2.0)], 1.0, C);
            c.with(Transform::at(1.0, 5.0), |c| c.fill_rect(0.0, 0.0, 1.0, 1.0, Rgb::hex(0xff0000)));
        });
        assert!(
            thick.get(12, 4).is_some() && thick.get(12, 6).is_some() && thick.get(12, 7).is_none(),
            "three dots wide"
        );
        assert_eq!(thick.get(4, 16), Some(Color::Rgb(Rgb::hex(0xff0000))), "(1, 5) scaled by three");
        // Shape-relative paints and text follow the transform too.
        let mut shaded = Canvas::new(12, 6);
        shaded.with(Transform::at(12.0, 12.0).rotate(0.5), |c| {
            c.fill_rect(-6.0, -6.0, 12.0, 12.0, Paint::edge(Rgb::hex(0xff0000), Rgb::hex(0x0000ff), 3.0));
            c.text(-4, -2, "A", crate::Font::tiny(), C);
        });
        let glyph: Vec<(i32, i32)> = (0..24)
            .flat_map(|y| (0..24).map(move |x| (x, y)))
            .filter(|&(x, y)| shaded.get(x, y) == Some(Color::Rgb(C)))
            .collect();
        assert!(glyph.len() >= 9, "the A is there, give or take a dot of resampling: {glyph:?}");
        assert!(
            glyph.iter().all(|&(x, y)| (x - 10).abs() <= 4 && (y - 11).abs() <= 4),
            "and where the transform put it"
        );
        assert!(matches!(shaded.get(12, 8), Some(Color::Rgb(g)) if g.b > 0));
    }
}
