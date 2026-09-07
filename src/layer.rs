//! Stacked canvases with occlusion and effects between them.
//!
//! A [`Layers`] is a stack of [`Layer`]s, each a full [`Canvas`] of its own, drawn
//! with the same primitives. [`Layers::flatten`] composites them bottom to top into
//! one plain canvas: a set dot hides whatever is under it, a printed character owns
//! its cell, and each layer's [`Effect`]s decorate its silhouette on the way — a drop
//! shadow, an outline, a cleared gap that separates it from what is behind, a glow,
//! a shaded rim, or a shader of your own. Everything downstream (the renderer, the
//! ratatui widget, the exporters, [`Canvas::to_text`]) takes the flattened canvas as
//! it would any other.
//!
//! ```
//! use cobra::{Effect, Layers, Paint, Rgb};
//!
//! let mut layers = Layers::new(30, 6);
//! layers[0].fill_rect(0.0, 8.0, 60.0, 8.0, Paint::dithered(Rgb::hex(0x30363d), 0.5));
//! let card = layers.push();
//! card.fill_round_rect(6.0, 4.0, 30.0, 16.0, 4.0, Rgb::hex(0x3aa0ff));
//! card.effect(Effect::shadow(2, 2).paint(Paint::dithered(Rgb::hex(0), 0.6))).effect(Effect::gap(1.0));
//! let flat = layers.flatten(); // a `Canvas`: render, export or print it
//! # assert!(flat.get(20, 10).is_some());
//! ```
//!
//! # Effects
//!
//! An effect is a function of a dot's signed distance to the layer's silhouette:
//! the set dots and printed cells of the layer, with positive distances outside it
//! and negative ones inside. Effects run after the layer's own dots have been laid
//! down, each within the band of distances it declares, in the order they were
//! added, so a later effect paints over an earlier one where they overlap. Distances
//! are rounded to the nearest dot: an outline one dot wide is the ring of dots
//! touching the silhouette, diagonals included. Distance fields are computed only
//! for the layers, and the region, that need them.
//!
//! # The text fallback
//!
//! A flattened canvas remembers which layer each dot came from. Where a terminal
//! can only give a cell one colour, the cell takes the colour of the topmost layer
//! that has a dot in it (the most frequent among that layer's dots), so a shape in
//! front keeps its edges instead of losing every shared cell to a bigger shape
//! behind it. Effects count as part of their layer, which makes a dithered shadow
//! read as solid in that fallback where it falls on another shape.

use std::fmt;
use std::ops::{Deref, DerefMut, Index, IndexMut};
use std::sync::Arc;

use crate::{Canvas, Color, DOTS_X, DOTS_Y, Paint};

/// A user shader: paints a dot near a layer's silhouette, or leaves it alone.
type ShaderFn = dyn Fn(&Sample) -> Option<Paint> + Send + Sync;

/// One dot as an effect sees it; see [`Effect::shader`].
#[derive(Clone, Copy)]
pub struct Sample<'a> {
    /// Dot coordinates.
    pub x: i32,
    /// Dot coordinates.
    pub y: i32,
    /// Distance in dots to the nearest dot on the other side of the layer's
    /// silhouette: positive outside, negative inside (the edge is at `±1`).
    pub dist: f32,
    /// What the dot shows right now: the layer's own colour inside the silhouette,
    /// whatever the layers below (and earlier effects) left outside it.
    pub color: Option<Color>,
    layer: &'a Canvas,
}

impl Sample<'_> {
    /// Whether the dot is part of the silhouette.
    #[inline]
    pub fn inside(&self) -> bool {
        self.dist < 0.0
    }

    /// The layer being shaded.
    #[inline]
    pub fn layer(&self) -> &Canvas {
        self.layer
    }

    /// Whether dot `(x, y)` of the layer is in its silhouette: set, or in a cell
    /// that holds a character. This is how a shadow finds itself.
    #[inline]
    pub fn covered(&self, x: i32, y: i32) -> bool {
        covered(self.layer, x, y)
    }
}

impl fmt::Debug for Sample<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sample").field("x", &self.x).field("y", &self.y).field("dist", &self.dist).finish()
    }
}

/// Something drawn around (or just inside) a layer's silhouette when the stack is
/// flattened. See the [module docs](self) for how effects are evaluated.
#[derive(Clone)]
#[non_exhaustive]
pub enum Effect {
    /// The silhouette again, moved by `(dx, dy)` dots and painted with `paint`,
    /// beneath the layer: a drop shadow. Dither the paint for a soft one.
    Shadow {
        /// Offset to the right.
        dx: i32,
        /// Offset downwards.
        dy: i32,
        /// What the shadow is painted with.
        paint: Paint,
    },
    /// A border `width` dots thick around the silhouette.
    Outline {
        /// Thickness in dots.
        width: f32,
        /// What the border is painted with.
        paint: Paint,
    },
    /// A halo `width` dots wide: an outer glow. It is painted in a darker shade of
    /// the paint's colour, falling from part of the paint's coverage at the edge to
    /// nothing at `width`, so a shape glowing in its own colour stays the brightest
    /// thing and reads as the source of the glow.
    Glow {
        /// Reach in dots.
        width: f32,
        /// Colour and peak coverage.
        paint: Paint,
    },
    /// Unsets every dot of the layers beneath within `width` dots of the silhouette,
    /// so the layer reads on any background: the cleared ring of
    /// [`clear_disc`](Canvas::clear_disc), for any shape, done automatically.
    Gap {
        /// Width of the cleared ring in dots.
        width: f32,
    },
    /// Paints the `depth` dots just inside the silhouette: a shaded rim. `None`
    /// paints each dot in a darker shade of its own colour; a light paint highlights
    /// the edge instead, a dark one deepens it.
    Rim {
        /// Depth in dots.
        depth: f32,
        /// What the rim is painted with; `None` for a shade of the layer's own colour.
        paint: Option<Paint>,
    },
    /// Your own: called for every dot within `reach` dots outside the silhouette and
    /// `depth` inside it, painting whatever it returns.
    Shader {
        /// How far outside the silhouette the shader is consulted.
        reach: f32,
        /// How far inside it.
        depth: f32,
        /// The shader.
        f: Arc<ShaderFn>,
    },
}

/// Coverage of a glow's innermost ring as a fraction of its paint's: below solid,
/// so the shape reads as the source of the glow rather than part of it.
const GLOW_PEAK: f32 = 0.7;
/// How much of the paint's brightness a glow keeps: a darker shade of the shape.
const GLOW_SHADE: f32 = 0.6;
/// How much of a dot's brightness a default rim keeps.
const RIM_SHADE: f32 = 0.55;

impl Effect {
    /// A drop shadow: the silhouette moved by `(dx, dy)` dots, in a dithered mid grey
    /// that reads on dark and light terminals alike. [`paint`](Self::paint) changes it.
    pub fn shadow(dx: i32, dy: i32) -> Self {
        Self::Shadow { dx, dy, paint: Paint::dithered(crate::Rgb::hex(0x6b7380), 0.6) }
    }

    /// A border `width` dots thick around the silhouette, in the terminal's
    /// foreground colour. [`paint`](Self::paint) changes it.
    pub fn outline(width: f32) -> Self {
        Self::Outline { width, paint: Paint::new(Color::Foreground) }
    }

    /// A glow fading out over `width` dots, in a darker shade of its paint so the shape
    /// stays the brightest thing; by default the terminal's foreground colour, but
    /// usually you want the shape's own colour: [`paint`](Self::paint) sets it.
    pub fn glow(width: f32) -> Self {
        Self::Glow { width, paint: Paint::new(Color::Foreground) }
    }

    /// A cleared ring `width` dots wide in the layers beneath.
    pub fn gap(width: f32) -> Self {
        Self::Gap { width }
    }

    /// A rim `depth` dots deep just inside the silhouette, in a darker shade of the
    /// layer's own colour: a shaded edge on any shape. [`paint`](Self::paint) paints
    /// it with something else instead.
    pub fn rim(depth: f32) -> Self {
        Self::Rim { depth, paint: None }
    }

    /// Replaces the effect's paint. Has no effect on a gap or a shader.
    pub fn paint(mut self, paint: impl Into<Paint>) -> Self {
        match &mut self {
            Effect::Shadow { paint: p, .. } | Effect::Outline { paint: p, .. } | Effect::Glow { paint: p, .. } => {
                *p = paint.into()
            }
            Effect::Rim { paint: p, .. } => *p = Some(paint.into()),
            Effect::Gap { .. } | Effect::Shader { .. } => {}
        }
        self
    }

    /// A shader of your own, consulted within `reach` dots outside the silhouette
    /// and `depth` inside. It returns the paint for the dot, or `None` to leave it.
    ///
    /// ```
    /// use cobra::{Color, Effect, Paint, Rgb};
    ///
    /// // A shadow that darkens what it falls on instead of painting over it.
    /// let shade = Effect::shader(4.0, 0.0, |s| match s.color {
    ///     Some(Color::Rgb(c)) if s.covered(s.x - 3, s.y - 2) => Some(Paint::new(c.dim(0.4))),
    ///     _ => None,
    /// });
    /// ```
    pub fn shader(reach: f32, depth: f32, f: impl Fn(&Sample) -> Option<Paint> + Send + Sync + 'static) -> Self {
        Self::Shader { reach, depth, f: Arc::new(f) }
    }

    /// How far outside and inside the silhouette the effect can paint, in dots.
    fn band(&self) -> (f32, f32) {
        match *self {
            Effect::Shadow { dx, dy, .. } => (dx.abs().max(dy.abs()) as f32, 0.0),
            Effect::Outline { width, .. } | Effect::Glow { width, .. } | Effect::Gap { width } => (width, 0.0),
            Effect::Rim { depth, .. } => (0.0, depth),
            Effect::Shader { reach, depth, .. } => (reach, depth),
        }
    }

    /// Whether the effect reads [`Sample::dist`]; a shadow only needs to know which
    /// side of the edge a dot is on.
    fn needs_distance(&self) -> bool {
        !matches!(self, Effect::Shadow { .. })
    }

    /// The paint for a dot inside the effect's band, if any.
    #[inline]
    fn shade(&self, s: &Sample) -> Option<Paint> {
        match self {
            Effect::Shadow { dx, dy, paint } => (!s.inside() && s.covered(s.x - dx, s.y - dy)).then_some(*paint),
            Effect::Outline { paint, .. } => (!s.inside()).then_some(*paint),
            Effect::Glow { width, paint } => {
                if s.inside() {
                    return None;
                }
                let t = (1.0 - (s.dist - 0.5) / width.max(0.5)).clamp(0.0, 1.0);
                let color = match paint.color()? {
                    Color::Rgb(c) => Color::Rgb(c.dim(GLOW_SHADE)),
                    other => other, // a palette colour has no shade; the dither carries it
                };
                Some(Paint::dithered(color, paint.coverage() * GLOW_PEAK * t))
            }
            Effect::Gap { .. } => (!s.inside()).then_some(Paint::erase()),
            Effect::Rim { paint, .. } => {
                if !s.inside() {
                    return None;
                }
                paint.or_else(|| {
                    // A shade of the dot's own colour; a palette colour has none, so
                    // it is dithered darker instead.
                    Some(match s.color? {
                        Color::Rgb(c) => Paint::new(c.dim(RIM_SHADE)),
                        _ => Paint::dithered(crate::Rgb::hex(0), 0.5),
                    })
                })
            }
            Effect::Shader { f, .. } => f(s),
        }
    }
}

impl fmt::Debug for Effect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Effect::Shadow { dx, dy, paint } => {
                f.debug_struct("Shadow").field("dx", dx).field("dy", dy).field("paint", paint).finish()
            }
            Effect::Outline { width, paint } => {
                f.debug_struct("Outline").field("width", width).field("paint", paint).finish()
            }
            Effect::Glow { width, paint } => {
                f.debug_struct("Glow").field("width", width).field("paint", paint).finish()
            }
            Effect::Gap { width } => f.debug_struct("Gap").field("width", width).finish(),
            Effect::Rim { depth, paint } => f.debug_struct("Rim").field("depth", depth).field("paint", paint).finish(),
            Effect::Shader { reach, depth, .. } => {
                f.debug_struct("Shader").field("reach", reach).field("depth", depth).finish_non_exhaustive()
            }
        }
    }
}

/// One layer of a [`Layers`]: a [`Canvas`] it dereferences to, the effects applied
/// around it when the stack is flattened, and whether it is shown at all.
#[derive(Clone, Debug)]
pub struct Layer {
    canvas: Canvas,
    /// Effects, applied in order; see [`Effect`].
    pub effects: Vec<Effect>,
    /// Hidden layers are skipped by [`Layers::flatten`]. Default `true`.
    pub visible: bool,
}

impl Layer {
    fn new(cols: u16, rows: u16) -> Self {
        Self { canvas: Canvas::new(cols, rows), effects: Vec::new(), visible: true }
    }

    /// Adds an effect after the ones already there.
    pub fn effect(&mut self, effect: Effect) -> &mut Self {
        self.effects.push(effect);
        self
    }

    /// The layer's canvas.
    #[inline]
    pub fn canvas(&self) -> &Canvas {
        &self.canvas
    }

    /// The layer's canvas, to draw on.
    #[inline]
    pub fn canvas_mut(&mut self) -> &mut Canvas {
        &mut self.canvas
    }

    /// The widest reach of the layer's effects, and whether any wants a distance
    /// field outside and inside the silhouette.
    fn needs(&self) -> Needs {
        let mut n = Needs::default();
        for e in &self.effects {
            let (reach, depth) = e.band();
            n.reach = n.reach.max(reach);
            if e.needs_distance() {
                n.outside |= reach > 0.0;
                n.inside |= depth > 0.0;
            }
        }
        n
    }
}

impl Deref for Layer {
    type Target = Canvas;
    #[inline]
    fn deref(&self) -> &Canvas {
        &self.canvas
    }
}

impl DerefMut for Layer {
    #[inline]
    fn deref_mut(&mut self) -> &mut Canvas {
        &mut self.canvas
    }
}

/// What a layer's effects ask of the flattening pass.
#[derive(Clone, Copy, Debug, Default)]
struct Needs {
    reach: f32,
    outside: bool,
    inside: bool,
}

/// A stack of [`Layer`]s that flattens into one [`Canvas`]. See the
/// [module docs](self).
///
/// A new stack has one layer, index `0`, at the bottom; [`push`](Self::push) adds
/// one on top. Layers are indexed like a slice and dereference to their canvas, so
/// `layers[1].disc(..)` draws on the second one. The stack owns the scratch it
/// flattens with, so after the first frame flattening does not allocate.
#[derive(Clone, Debug)]
pub struct Layers {
    cols: u16,
    rows: u16,
    layers: Vec<Layer>,
    flat: Canvas,
    /// Something changed since the last flatten. Every `&mut` path sets it.
    dirty: bool,
    /// Silhouette of the layer being flattened, over the effect window (`1` = covered).
    mask: Vec<u8>,
    /// Squared distance fields over the window, outside and inside the silhouette.
    outside: Vec<f32>,
    inside: Vec<f32>,
    edt: Edt,
}

impl Layers {
    /// Creates a stack of `cols × rows` cells with one empty layer.
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            layers: vec![Layer::new(cols, rows)],
            flat: Canvas::new(cols, rows),
            dirty: true,
            mask: Vec::new(),
            outside: Vec::new(),
            inside: Vec::new(),
            edt: Edt::default(),
        }
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

    /// Number of layers.
    #[inline]
    pub fn len(&self) -> usize {
        self.layers.len()
    }

    /// Whether there are no layers at all (only after [`remove`](Self::remove)).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    /// Adds an empty layer on top and returns it, to draw on and give effects.
    pub fn push(&mut self) -> &mut Layer {
        self.insert(self.layers.len())
    }

    /// Adds an empty layer at `index` (`0` is the bottom; the length puts it on top)
    /// and returns it.
    pub fn insert(&mut self, index: usize) -> &mut Layer {
        self.dirty = true;
        let index = index.min(self.layers.len());
        self.layers.insert(index, Layer::new(self.cols, self.rows));
        &mut self.layers[index]
    }

    /// Removes and returns the layer at `index`, `None` if there is none.
    pub fn remove(&mut self, index: usize) -> Option<Layer> {
        self.dirty = true;
        (index < self.layers.len()).then(|| self.layers.remove(index))
    }

    /// Swaps two layers' places in the stack.
    pub fn swap(&mut self, a: usize, b: usize) {
        self.dirty = true;
        self.layers.swap(a, b);
    }

    /// The layer at `index`, if any.
    #[inline]
    pub fn get(&self, index: usize) -> Option<&Layer> {
        self.layers.get(index)
    }

    /// The layer at `index` to change, if any.
    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut Layer> {
        self.dirty = true;
        self.layers.get_mut(index)
    }

    /// The layers, bottom to top.
    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Layer> {
        self.layers.iter()
    }

    /// The layers to change, bottom to top.
    #[inline]
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Layer> {
        self.dirty = true;
        self.layers.iter_mut()
    }

    /// Clears every layer's dots and text. Keeps the layers, their effects and every
    /// allocation.
    pub fn clear(&mut self) {
        self.dirty = true;
        for layer in &mut self.layers {
            layer.canvas.clear();
        }
    }

    /// The flattened canvas as of the last [`flatten`](Self::flatten); empty before
    /// the first, stale after a change. For code that only has `&self`, such as a
    /// ratatui draw closure, after flattening outside it.
    #[inline]
    pub fn flat(&self) -> &Canvas {
        &self.flat
    }

    /// Composites the layers, bottom to top, into one canvas and returns it. Does
    /// nothing when nothing changed since the last call.
    pub fn flatten(&mut self) -> &Canvas {
        if !self.dirty {
            return &self.flat;
        }
        self.dirty = false;
        self.flat.clear();
        self.flat.prio.resize(self.flat.dots.len(), 0);
        for i in 0..self.layers.len() {
            if !self.layers[i].visible {
                continue;
            }
            let prio = i.min(u8::MAX as usize) as u8;
            let layer = &self.layers[i];
            for ((f, p), &d) in self.flat.dots.iter_mut().zip(&mut self.flat.prio).zip(&layer.canvas.dots) {
                if d != 0 {
                    (*f, *p) = (d, prio);
                }
            }
            if layer.canvas.has_text() {
                for row in 0..self.rows as i32 {
                    for col in 0..self.cols as i32 {
                        if let Some(cell) = layer.canvas.text_cell(col, row) {
                            self.flat.put(col, row, cell);
                        }
                    }
                }
            }
            if !layer.effects.is_empty() {
                self.apply(i, prio);
            }
        }
        &self.flat
    }

    /// Runs the effects of layer `i` over the flattened canvas.
    fn apply(&mut self, i: usize, prio: u8) {
        let needs = self.layers[i].needs();
        let Some((x0, y0, x1, y1)) = bounds(&self.layers[i].canvas) else { return };
        // The window: the silhouette's box grown by the widest reach, on the canvas,
        // plus a one-dot frame the distance fields use for what lies beyond it.
        let m = (needs.reach + 0.5).ceil() as i32;
        let (wx0, wy0) = ((x0 - m).max(0), (y0 - m).max(0));
        let (wx1, wy1) = ((x1 + m).min(self.width()), (y1 + m).min(self.height()));
        let (gw, gh) = ((wx1 - wx0 + 2) as usize, (wy1 - wy0 + 2) as usize);
        let layer = &self.layers[i].canvas;

        // The window follows the content, so the scratch is sized for the whole
        // canvas the first time round rather than growing frame by frame.
        let full = (self.width() as usize + 2) * (self.height() as usize + 2);
        self.mask.clear();
        self.mask.reserve(full);
        self.mask.resize(gw * gh, 0);
        for y in wy0..wy1 {
            let row = &mut self.mask[(y - wy0 + 1) as usize * gw + 1..][..(wx1 - wx0) as usize];
            for (k, m) in row.iter_mut().enumerate() {
                *m = covered(layer, wx0 + k as i32, y) as u8;
            }
        }
        if needs.outside {
            self.outside.clear();
            self.outside.reserve(full);
            self.outside.extend(self.mask.iter().map(|&m| if m != 0 { 0.0 } else { INF }));
            self.edt.run(&mut self.outside, gw, gh);
        }
        if needs.inside {
            // Off-canvas and beyond the window count as unset, so a shape that runs
            // off the edge still has a rim there.
            self.inside.clear();
            self.inside.reserve(full);
            self.inside.extend(self.mask.iter().map(|&m| if m != 0 { INF } else { 0.0 }));
            self.edt.run(&mut self.inside, gw, gh);
        }

        let effects = &self.layers[i].effects;
        let w = self.flat.width() as usize;
        for y in wy0..wy1 {
            for x in wx0..wx1 {
                let g = (y - wy0 + 1) as usize * gw + (x - wx0 + 1) as usize;
                let inside = self.mask[g] != 0;
                // Without a field the distance is only a side: far enough out or in
                // that no band admits it, which is what a shadow wants.
                let dist = match (inside, needs.outside, needs.inside) {
                    (false, true, _) => self.outside[g].sqrt(),
                    (false, false, _) => INF,
                    (true, _, true) => -self.inside[g].sqrt(),
                    (true, _, false) => -INF,
                };
                let d = y as usize * w + x as usize;
                for effect in effects {
                    let (reach, depth) = effect.band();
                    let admitted = if effect.needs_distance() {
                        if inside { -dist < depth + 0.5 } else { dist < reach + 0.5 }
                    } else {
                        !inside
                    };
                    if !admitted {
                        continue;
                    }
                    let current = self.flat.dots[d];
                    let color = (current != 0).then(|| Color::from_packed(current));
                    let sample = Sample { x, y, dist, color, layer };
                    if let Some(paint) = effect.shade(&sample)
                        && paint.covers(x, y)
                    {
                        self.flat.dots[d] = paint.packed();
                        self.flat.prio[d] = if paint.packed() == 0 { 0 } else { prio };
                    }
                }
            }
        }
    }
}

impl Index<usize> for Layers {
    type Output = Layer;
    #[inline]
    fn index(&self, index: usize) -> &Layer {
        &self.layers[index]
    }
}

impl IndexMut<usize> for Layers {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Layer {
        self.dirty = true;
        &mut self.layers[index]
    }
}

/// Whether dot `(x, y)` is in `canvas`'s silhouette: set, or in a printed cell.
#[inline]
fn covered(canvas: &Canvas, x: i32, y: i32) -> bool {
    canvas.get(x, y).is_some()
        || (canvas.has_text() && canvas.text_cell(x.div_euclid(DOTS_X as i32), y.div_euclid(DOTS_Y as i32)).is_some())
}

/// The box `(x0, y0, x1, y1)` (exclusive on the far side) around the silhouette of
/// `canvas`, `None` when it is empty.
fn bounds(canvas: &Canvas) -> Option<(i32, i32, i32, i32)> {
    let (w, h) = (canvas.width(), canvas.height());
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for (y, row) in canvas.dots.chunks_exact(w as usize).enumerate() {
        let Some(first) = row.iter().position(|&d| d != 0) else { continue };
        let last = row.iter().rposition(|&d| d != 0).unwrap_or(first);
        (x0, x1) = (x0.min(first as i32), x1.max(last as i32 + 1));
        (y0, y1) = (y0.min(y as i32), y as i32 + 1);
    }
    if canvas.has_text() {
        for row in 0..canvas.rows() as i32 {
            for col in 0..canvas.cols() as i32 {
                if canvas.text_cell(col, row).is_some() {
                    let (cx, cy) = (col * DOTS_X as i32, row * DOTS_Y as i32);
                    (x0, x1) = (x0.min(cx), x1.max(cx + DOTS_X as i32));
                    (y0, y1) = (y0.min(cy), y1.max(cy + DOTS_Y as i32));
                }
            }
        }
    }
    (x0 < x1 && y0 < y1).then_some((x0, y0, x1, y1))
}

/// Squared distance standing in for "no source anywhere": far beyond any canvas,
/// finite so the transform's arithmetic stays finite.
const INF: f32 = 1e10;

/// Scratch for the exact Euclidean distance transform (Felzenszwalb–Huttenlocher):
/// a lower envelope of parabolas per line, in two separable passes.
#[derive(Clone, Debug, Default)]
struct Edt {
    f: Vec<f32>,
    d: Vec<f32>,
    v: Vec<usize>,
    z: Vec<f32>,
}

impl Edt {
    /// Replaces `grid` (`0` at sources, [`INF`] elsewhere, `w × h` row-major) with
    /// the squared distance of every entry to its nearest source.
    fn run(&mut self, grid: &mut [f32], w: usize, h: usize) {
        let n = w.max(h);
        self.f.resize(n, 0.0);
        self.d.resize(n, 0.0);
        self.v.resize(n + 1, 0);
        self.z.resize(n + 2, 0.0);
        for x in 0..w {
            // A column without a source stays at INF, which the row pass reads as
            // "nothing here", so it can be skipped.
            if !(0..h).any(|y| grid[y * w + x] == 0.0) {
                continue;
            }
            for y in 0..h {
                self.f[y] = grid[y * w + x];
            }
            self.line(h);
            for y in 0..h {
                grid[y * w + x] = self.d[y];
            }
        }
        for row in grid.chunks_exact_mut(w) {
            self.f[..w].copy_from_slice(row);
            self.line(w);
            row.copy_from_slice(&self.d[..w]);
        }
    }

    /// One-dimensional squared distance transform of `f[..n]` into `d[..n]`.
    fn line(&mut self, n: usize) {
        let (f, d, v, z) = (&self.f[..n], &mut self.d[..n], &mut self.v, &mut self.z);
        let mut k = 0;
        v[0] = 0;
        z[0] = f32::NEG_INFINITY;
        z[1] = f32::INFINITY;
        for q in 1..n {
            let fq = f[q] + (q * q) as f32;
            let s = loop {
                let p = v[k];
                let s = (fq - f[p] - (p * p) as f32) / (2 * (q - p)) as f32;
                if s > z[k] {
                    break s;
                }
                k -= 1;
            };
            k += 1;
            v[k] = q;
            z[k] = s;
            z[k + 1] = f32::INFINITY;
        }
        k = 0;
        for (q, out) in d.iter_mut().enumerate() {
            while z[k + 1] < q as f32 {
                k += 1;
            }
            let p = v[k];
            *out = (q as f32 - p as f32).powi(2) + f[p];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rgb;

    const A: Rgb = Rgb::hex(0xaa0000);
    const B: Rgb = Rgb::hex(0x0000bb);

    fn lit(c: &Canvas) -> usize {
        c.cells().map(|cell| cell.bits.count_ones() as usize).sum()
    }

    #[test]
    fn upper_layers_hide_lower_ones() {
        let mut l = Layers::new(4, 2);
        l[0].fill_rect(0.0, 0.0, 8.0, 8.0, A);
        l.push().fill_rect(2.0, 2.0, 4.0, 4.0, B);
        let flat = l.flatten();
        assert_eq!(flat.get(0, 0), Some(Color::Rgb(A)));
        assert_eq!(flat.get(3, 3), Some(Color::Rgb(B)));
        assert_eq!(lit(flat), 64);
        // The cell shared by both takes the upper colour, whatever the count.
        assert_eq!(flat.cell(1, 0).color, Some(Color::Rgb(B)), "two of B in front of six of A");
    }

    #[test]
    fn flatten_is_lazy_and_hidden_layers_are_skipped() {
        let mut l = Layers::new(2, 1);
        l[0].set(0, 0, A);
        assert_eq!(l.flat().get(0, 0), None, "nothing flattened yet");
        assert_eq!(l.flatten().get(0, 0), Some(Color::Rgb(A)));
        let ptr = l.flatten().dots.as_ptr();
        assert_eq!(l.flatten().dots.as_ptr(), ptr);
        l[0].visible = false;
        assert_eq!(l.flatten().get(0, 0), None);
        l.get_mut(0).unwrap().visible = true;
        assert_eq!(l.flatten().get(0, 0), Some(Color::Rgb(A)));
        l.clear();
        assert_eq!(l.flatten().get(0, 0), None);
    }

    #[test]
    fn stack_edits() {
        let mut l = Layers::new(2, 1);
        assert_eq!(l.len(), 1);
        l.push().set(0, 0, A);
        l.insert(0).set(0, 0, B);
        assert_eq!(l.len(), 3);
        assert_eq!(l.flatten().get(0, 0), Some(Color::Rgb(A)), "the pushed layer is on top");
        l.swap(0, 2);
        assert_eq!(l.flatten().get(0, 0), Some(Color::Rgb(B)));
        assert!(l.remove(2).is_some() && l.remove(9).is_none());
        assert_eq!(l.iter().count(), 2);
        assert_eq!(l.flatten().get(0, 0), Some(Color::Rgb(A)));
        assert!(l.get(5).is_none());
    }

    #[test]
    fn text_is_merged_and_counts_as_silhouette() {
        let mut l = Layers::new(6, 2);
        l[0].fill_rect(0.0, 0.0, 12.0, 8.0, A);
        l[0].print(0, 0, "ab", B);
        let top = l.push();
        top.print(1, 0, "X", A);
        top.effect(Effect::gap(1.0));
        let flat = l.flatten();
        assert_eq!(flat.text_cell(0, 0).map(|t| t.ch), Some('a'));
        assert_eq!(flat.text_cell(1, 0).map(|t| t.ch), Some('X'), "the upper character wins");
        // The gap around the printed cell clears the dots of the layer beneath.
        assert_eq!(flat.get(4, 0), None, "right of the cell");
        assert_eq!(flat.get(2, 4), None, "below it");
        assert_eq!(flat.get(6, 0), Some(Color::Rgb(A)), "two dots away is untouched");
    }

    #[test]
    fn shadow_outline_gap_glow_and_rim() {
        let mut l = Layers::new(8, 4);
        l[0].fill_rect(0.0, 0.0, 16.0, 16.0, A);
        let top = l.push();
        top.fill_rect(6.0, 6.0, 4.0, 4.0, B);
        top.effect(Effect::shadow(2, 2).paint(Rgb::hex(0x111111)));
        let flat = l.flatten().clone();
        assert_eq!(flat.get(11, 11), Some(Color::Rgb(Rgb::hex(0x111111))), "shadow past the corner");
        assert_eq!(flat.get(9, 9), Some(Color::Rgb(B)), "the shape stays on top of its shadow");
        assert_eq!(flat.get(5, 5), Some(Color::Rgb(A)), "no shadow up-left");

        l[1].effects = vec![Effect::outline(1.0).paint(Rgb::hex(0x222222))];
        let flat = l.flatten().clone();
        let ring = Some(Color::Rgb(Rgb::hex(0x222222)));
        assert_eq!(flat.get(5, 5), ring, "diagonal neighbour is in a 1-dot outline");
        assert_eq!(flat.get(5, 7), ring);
        assert_eq!(flat.get(4, 7), Some(Color::Rgb(A)), "two dots out is not");

        l[1].effects = vec![Effect::gap(2.0)];
        let flat = l.flatten().clone();
        assert_eq!(flat.get(4, 7), None);
        assert_eq!(flat.get(3, 7), Some(Color::Rgb(A)));
        assert_eq!(flat.get(7, 7), Some(Color::Rgb(B)), "the gap never touches the layer itself");

        l[1].effects = vec![Effect::glow(4.0).paint(Rgb::hex(0x333333))];
        let flat = l.flatten().clone();
        let halo = Some(Color::Rgb(Rgb::hex(0x333333).dim(GLOW_SHADE)));
        let glow = |d: i32| (0..16).filter(|&y| flat.get(6 - d, y) == halo).count();
        assert!(glow(1) > 0, "the glow starts at the shape, in a darker shade");
        assert!(glow(1) > glow(3), "the glow thins out: {} vs {}", glow(1), glow(3));
        assert!(glow(1) < 16, "the glow never reaches solid");
        assert_eq!(glow(6), 0);
        assert_eq!(flat.get(7, 7), Some(Color::Rgb(B)), "the shape keeps its own colour");

        l[1].effects = vec![Effect::rim(1.0).paint(Rgb::hex(0x444444))];
        let flat = l.flatten().clone();
        assert_eq!(flat.get(6, 6), Some(Color::Rgb(Rgb::hex(0x444444))), "the edge dot is rim");
        assert_eq!(flat.get(5, 5), Some(Color::Rgb(A)), "outside is untouched");
        // A 4×4 box is all rim at depth 1 (every dot touches the outside), and the
        // layer's own colour is what the rim replaces.
        let mut l = Layers::new(8, 4);
        l.push().fill_rect(2.0, 2.0, 8.0, 8.0, B);
        l[1].effect(Effect::rim(1.0).paint(Rgb::hex(0x444444)));
        let flat = l.flatten();
        assert_eq!(flat.get(5, 5), Some(Color::Rgb(B)), "the middle of an 8×8 box is not");
        assert_eq!(flat.get(2, 5), Some(Color::Rgb(Rgb::hex(0x444444))));
    }

    #[test]
    fn effects_stack_in_order_and_shaders_see_distances() {
        let mut l = Layers::new(8, 4);
        l[0].fill_rect(0.0, 0.0, 16.0, 16.0, A);
        let top = l.push();
        top.fill_rect(6.0, 6.0, 4.0, 4.0, B);
        top.effect(Effect::shader(3.0, 2.0, |s| {
            assert!(s.dist.abs() < 3.5, "consulted outside its band: {s:?}");
            assert_eq!(s.inside(), s.covered(s.x, s.y));
            assert_eq!(s.layer().get(s.x, s.y).is_some(), s.inside());
            let grey = 0x10 * s.dist.round() as i32;
            Some(Paint::new(Rgb::hex((0x404040 + grey) as u32)))
        }));
        top.effect(Effect::gap(1.0));
        let flat = l.flatten();
        assert_eq!(flat.get(6, 6), Some(Color::Rgb(Rgb::hex(0x404040 - 0x10))), "one dot inside");
        assert_eq!(flat.get(7, 7), Some(Color::Rgb(Rgb::hex(0x404040 - 0x20))), "two dots inside");
        assert_eq!(flat.get(5, 7), None, "the later gap erased the shader's ring");
        assert_eq!(flat.get(4, 7), Some(Color::Rgb(Rgb::hex(0x404040 + 0x20))));
        assert_eq!(flat.get(3, 7), Some(Color::Rgb(Rgb::hex(0x404040 + 0x30))));
        assert_eq!(flat.get(2, 7), Some(Color::Rgb(A)), "four out is past the reach");
        assert_eq!(flat.get(4, 4), Some(Color::Rgb(Rgb::hex(0x404040 + 0x30))), "diagonal: 2.83 rounds to 3");
        assert_eq!(flat.get(3, 3), Some(Color::Rgb(A)), "diagonal: 4.24 is past the reach");
    }

    #[test]
    fn a_shape_off_the_edge_keeps_its_rim_there() {
        let mut l = Layers::new(4, 2);
        l[0].fill_rect(-4.0, -4.0, 8.0, 8.0, B);
        l[0].effect(Effect::rim(1.0).paint(A));
        let flat = l.flatten();
        assert_eq!(flat.get(0, 0), Some(Color::Rgb(A)), "the canvas edge is an edge");
        assert_eq!(flat.get(1, 1), Some(Color::Rgb(B)));
        assert_eq!(flat.get(3, 1), Some(Color::Rgb(A)));
    }

    #[test]
    fn flatten_does_not_allocate_after_warmup() {
        let mut l = Layers::new(20, 5);
        l.push()
            .effect(Effect::outline(2.0).paint(A))
            .effect(Effect::rim(1.0).paint(B))
            .effect(Effect::shadow(1, 1).paint(A));
        let frame = |l: &mut Layers, t: f32| {
            l.clear();
            l[0].fill_rect(0.0, 0.0, 40.0, 20.0, Paint::dithered(A, 0.5));
            l[1].disc(20.0 + t, 10.0, 6.0, B);
            l[1].print(2, 1, "hi", A);
            l.flatten();
            (
                l.flat.dots.capacity(),
                l.flat.text.capacity(),
                l.mask.capacity(),
                l.outside.capacity(),
                l.inside.capacity(),
            )
        };
        let a = frame(&mut l, -8.0);
        let b = frame(&mut l, 12.0);
        assert_eq!(a, b);
    }

    #[test]
    fn distance_transform_is_exact() {
        let (w, h) = (6, 5);
        let mut grid = vec![INF; w * h];
        grid[w + 1] = 0.0;
        grid[3 * w + 5] = 0.0;
        Edt::default().run(&mut grid, w, h);
        for y in 0..h {
            for x in 0..w {
                let d = |sx: usize, sy: usize| (x as f32 - sx as f32).powi(2) + (y as f32 - sy as f32).powi(2);
                assert_eq!(grid[y * w + x], d(1, 1).min(d(5, 3)), "at {x},{y}");
            }
        }
        let mut none = vec![INF; 4];
        Edt::default().run(&mut none, 2, 2);
        assert!(none.iter().all(|&d| d >= INF - 8.0));
    }
}
