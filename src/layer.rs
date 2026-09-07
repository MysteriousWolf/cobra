//! Stacked canvases with occlusion and effects between them.
//!
//! A [`Layers`] is a stack of [`Layer`]s, each a full [`Canvas`] of its own, drawn
//! with the same primitives. [`Layers::flatten`] composites them bottom to top into
//! one plain canvas: a set dot hides whatever is under it, a printed character owns
//! its cell, and each layer's [`Effect`]s decorate its silhouette on the way — a drop
//! shadow, an outline, a cleared gap that separates it from what is behind, a glow,
//! a shaded rim, or a shader of your own. A [matte](Layer::matte) layer hides what is
//! beneath it without painting anything itself. A layer can sit [offset](Layer::offset)
//! from the stack, and [wrap](Layer::wrap) around it, so a background that scrolls
//! slower than the foreground (parallax) is a matter of moving each layer by its own
//! amount. Everything downstream (the renderer, the ratatui widget, the exporters,
//! [`Canvas::to_text`]) takes the flattened canvas as it would any other, and
//! [`Canvas::effects`] runs the same effects around any mask on any canvas, without
//! a stack; a [`Field`] of your own keeps the scratch that takes between calls.
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
    sil: Silhouette<'a>,
}

impl Sample<'_> {
    /// Whether the dot is part of the silhouette.
    #[inline]
    pub fn inside(&self) -> bool {
        self.dist < 0.0
    }

    /// The layer being shaded. Its dots are in its own coordinates; the sample's
    /// `x` and `y` are in the flattened canvas's, which differ by the layer's
    /// [offset](Layer::offset).
    #[inline]
    pub fn layer(&self) -> &Canvas {
        self.sil.canvas
    }

    /// Whether dot `(x, y)` of the flattened canvas is in the layer's silhouette:
    /// set, or in a cell that holds a character, once the layer's offset is
    /// applied. This is how a shadow finds itself.
    #[inline]
    pub fn covered(&self, x: i32, y: i32) -> bool {
        self.sil.covered(x, y)
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
    /// A matte hides instead of shows: wherever it has a dot or a character, the
    /// layers beneath are erased and nothing is painted, so the flattened canvas is
    /// transparent there. Effects still run around its silhouette. A ring around a
    /// hole in the picture, or a hollow shape whose inside must stay clear, is a
    /// shape on a matte. Default `false`.
    pub matte: bool,
    /// Where the layer sits over the stack, in dots: everything on it is moved right
    /// by `.0` and down by `.1` when the stack is flattened, effects included, and
    /// what moves off the stack is lost unless the layer [wraps](Self::wrap).
    /// Printed characters move by whole cells, the offset rounded to the nearest.
    /// Layers moving by different amounts per frame are a parallax; see
    /// [`scroll`](Self::scroll). Default `(0, 0)`.
    pub offset: (i32, i32),
    /// Whether the layer repeats: what its offset moves off one edge of the stack
    /// comes back on the opposite edge, so a background drawn once scrolls forever.
    /// Effects see the wrapped silhouette, and so does [`Sample::covered`]. Default
    /// `false`.
    pub wrap: bool,
}

impl Layer {
    fn new(cols: u16, rows: u16) -> Self {
        Self {
            canvas: Canvas::new(cols, rows),
            effects: Vec::new(),
            visible: true,
            matte: false,
            offset: (0, 0),
            wrap: false,
        }
    }

    /// Adds an effect after the ones already there.
    pub fn effect(&mut self, effect: Effect) -> &mut Self {
        self.effects.push(effect);
        self
    }

    /// Moves the layer by `(dx, dy)` dots from where it is: adds to its
    /// [`offset`](Self::offset). A wrapping layer's offset is kept within the size
    /// of the stack, so scrolling it for hours never overflows.
    ///
    /// ```
    /// use cobra::Layers;
    ///
    /// let mut layers = Layers::new(40, 10);
    /// layers[0].wrap = true; // the far hills, drawn once
    /// let _near = layers.push(); // the near ones
    /// // Every frame the near layer moves a dot, the far one a dot every third frame.
    /// for frame in 0..30 {
    ///     layers[1].scroll(-1, 0);
    ///     if frame % 3 == 0 {
    ///         layers[0].scroll(-1, 0);
    ///     }
    ///     layers.flatten();
    /// }
    /// assert_eq!(layers[1].offset, (-30, 0));
    /// assert_eq!(layers[0].offset, (70, 0), "ten dots left, modulo the width");
    /// ```
    pub fn scroll(&mut self, dx: i32, dy: i32) -> &mut Self {
        self.offset = (self.offset.0 + dx, self.offset.1 + dy);
        if self.wrap {
            let (w, h) = (self.canvas.width().max(1), self.canvas.height().max(1));
            self.offset = (self.offset.0.rem_euclid(w), self.offset.1.rem_euclid(h));
        }
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
}

/// The widest reach of `effects`, and whether any wants a distance field outside
/// and inside the silhouette.
fn needs(effects: &[Effect]) -> Needs {
    let mut n = Needs::default();
    for e in effects {
        let (reach, depth) = e.band();
        n.reach = n.reach.max(reach);
        if e.needs_distance() {
            n.outside |= reach > 0.0;
            n.inside |= depth > 0.0;
        }
    }
    n
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
    field: Field,
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
            field: Field::default(),
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
        let (width, height) = (self.width() as usize, self.height() as usize);
        for i in 0..self.layers.len() {
            if !self.layers[i].visible {
                continue;
            }
            let prio = i.min(u8::MAX as usize) as u8;
            let layer = &self.layers[i];
            let sil = Silhouette::new(&layer.canvas, layer.offset, layer.wrap);
            // Every row of the stack takes the layer row the offset puts there, in one
            // run without wrapping and up to two with it.
            for y in 0..height {
                let Some(sy) = sil.row(y as i32) else { continue };
                let src = &layer.canvas.dots[sy as usize * width..][..width];
                for (x0, sx0, len) in sil.runs().into_iter().flatten() {
                    let at = y * width + x0;
                    let dots = &mut self.flat.dots[at..at + len];
                    let prios = &mut self.flat.prio[at..at + len];
                    for ((f, p), &d) in dots.iter_mut().zip(prios).zip(&src[sx0..sx0 + len]) {
                        if d != 0 {
                            (*f, *p) = if layer.matte { (0, 0) } else { (d, prio) };
                        }
                    }
                }
            }
            if layer.canvas.has_text() {
                for row in 0..self.rows as i32 {
                    for col in 0..self.cols as i32 {
                        let Some(cell) = layer.canvas.text_cell(col, row) else { continue };
                        let Some((col, row)) = sil.cell(col, row) else { continue };
                        if layer.matte {
                            self.flat.erase_text(col, row, 1);
                            let (x, y) = (col as f32 * DOTS_X as f32, row as f32 * DOTS_Y as f32);
                            self.flat.fill_rect(x, y, DOTS_X as f32, DOTS_Y as f32, Paint::erase());
                        } else {
                            self.flat.put(col, row, cell);
                        }
                    }
                }
            }
            if !layer.effects.is_empty() {
                self.field.apply(&mut self.flat, prio, sil, &layer.effects);
            }
        }
        &self.flat
    }
}

/// A layer's silhouette as the flattened canvas sees it: the layer's dots and printed
/// cells, moved by its offset and, when it wraps, repeated. Coordinates go in as the
/// stack's and come out as the layer's.
#[derive(Clone, Copy, Debug)]
struct Silhouette<'a> {
    canvas: &'a Canvas,
    dx: i32,
    dy: i32,
    wrap: bool,
}

impl<'a> Silhouette<'a> {
    fn new(canvas: &'a Canvas, (dx, dy): (i32, i32), wrap: bool) -> Self {
        let (w, h) = (canvas.width(), canvas.height());
        // A wrapping offset is a phase: take it modulo the layer once, here, so the
        // runs below are simple and an empty canvas cannot divide by zero.
        let wrap = wrap && w > 0 && h > 0;
        let (dx, dy) = if wrap { (dx.rem_euclid(w), dy.rem_euclid(h)) } else { (dx, dy) };
        Self { canvas, dx, dy, wrap }
    }

    /// The layer row that lands on stack row `y`.
    #[inline]
    fn row(&self, y: i32) -> Option<i32> {
        let (sy, h) = (y - self.dy, self.canvas.height());
        if self.wrap { Some(sy.rem_euclid(h)) } else { (0..h).contains(&sy).then_some(sy) }
    }

    /// The runs of a row, `(stack x, layer x, length)`, that the layer lands on.
    fn runs(&self) -> [Option<(usize, usize, usize)>; 2] {
        let w = self.canvas.width();
        let run = |x0: i32, sx0: i32, len: i32| (len > 0).then_some((x0 as usize, sx0 as usize, len as usize));
        if self.wrap {
            // The offset is in `0..w`, so the row is the layer's tail, then its head.
            [run(self.dx, 0, w - self.dx), run(0, w - self.dx, self.dx)]
        } else {
            let (x0, x1) = (self.dx.max(0), (w + self.dx).min(w));
            [run(x0, x0 - self.dx, x1 - x0), None]
        }
    }

    /// The layer dot that lands on stack dot `(x, y)`, if any.
    #[inline]
    fn source(&self, x: i32, y: i32) -> Option<(i32, i32)> {
        let (sx, sy) = (x - self.dx, y - self.dy);
        let (w, h) = (self.canvas.width(), self.canvas.height());
        if self.wrap {
            Some((sx.rem_euclid(w), sy.rem_euclid(h)))
        } else {
            ((0..w).contains(&sx) && (0..h).contains(&sy)).then_some((sx, sy))
        }
    }

    /// Whether stack dot `(x, y)` is in the silhouette.
    #[inline]
    fn covered(&self, x: i32, y: i32) -> bool {
        self.source(x, y).is_some_and(|(sx, sy)| covered(self.canvas, sx, sy))
    }

    /// The stack cell that layer cell `(col, row)` lands on: the offset rounded to
    /// whole cells, since a character cannot straddle two.
    fn cell(&self, col: i32, row: i32) -> Option<(i32, i32)> {
        let (cols, rows) = (self.canvas.cols() as i32, self.canvas.rows() as i32);
        let (cw, ch) = (DOTS_X as i32, DOTS_Y as i32);
        let (col, row) = (col + (2 * self.dx + cw).div_euclid(2 * cw), row + (2 * self.dy + ch).div_euclid(2 * ch));
        if self.wrap {
            Some((col.rem_euclid(cols), row.rem_euclid(rows)))
        } else {
            ((0..cols).contains(&col) && (0..rows).contains(&row)).then_some((col, row))
        }
    }

    /// The box around the silhouette on a stack of `width × height` dots, `None`
    /// when it is empty or entirely off the stack.
    fn bounds(&self, width: i32, height: i32) -> Option<(i32, i32, i32, i32)> {
        let (x0, y0, x1, y1) = bounds(self.canvas)?;
        if self.wrap {
            return Some((0, 0, width, height));
        }
        let (x0, y0) = ((x0 + self.dx).max(0), (y0 + self.dy).max(0));
        let (x1, y1) = ((x1 + self.dx).min(width), (y1 + self.dy).min(height));
        (x0 < x1 && y0 < y1).then_some((x0, y0, x1, y1))
    }
}

/// The scratch that running [`Effect`]s takes: the silhouette over the effect
/// window and the distance fields outside and inside it, sized for the canvas the
/// first time and kept. A [`Layers`] owns one; [`Canvas::effects`] makes one per
/// call. Keep your own to run effects against masks every frame without allocating:
///
/// ```
/// use cobra::{Canvas, Effect, Field, Rgb};
///
/// let mut field = Field::new();
/// let mut canvas = Canvas::new(20, 5);
/// let mut mask = Canvas::new(20, 5);
/// for frame in 0..3 {
///     canvas.clear();
///     mask.clear();
///     mask.disc(10.0 + frame as f32, 10.0, 5.0, Rgb::hex(0xffffff));
///     field.effects(&mut canvas, &mask, &[Effect::outline(1.0).paint(Rgb::hex(0x3aa0ff))]);
/// }
/// ```
#[derive(Clone, Debug, Default)]
pub struct Field {
    mask: Vec<u8>,
    outside: Vec<f32>,
    inside: Vec<f32>,
    edt: Edt,
}

impl Field {
    /// An empty scratch; the first call grows it to fit and later ones reuse it.
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs `effects` around the silhouette of `mask` on `target`, exactly as
    /// [`Canvas::effects`] does, with this scratch instead of a fresh one.
    pub fn effects(&mut self, target: &mut Canvas, mask: &Canvas, effects: &[Effect]) {
        self.apply(target, 0, Silhouette::new(mask, (0, 0), false), effects);
    }

    /// Runs `effects` around `sil` over `target`, tagging the dots they paint with
    /// `prio` where the target keeps layer tags.
    fn apply(&mut self, target: &mut Canvas, prio: u8, sil: Silhouette, effects: &[Effect]) {
        let needs = needs(effects);
        let (width, height) = (target.width(), target.height());
        let Some((x0, y0, x1, y1)) = sil.bounds(width, height) else { return };
        // The window: the silhouette's box grown by the widest reach, on the canvas,
        // plus a one-dot frame the distance fields use for what lies beyond it.
        let m = (needs.reach + 0.5).ceil() as i32;
        let (wx0, wy0) = ((x0 - m).max(0), (y0 - m).max(0));
        let (wx1, wy1) = ((x1 + m).min(width), (y1 + m).min(height));
        let (gw, gh) = ((wx1 - wx0 + 2) as usize, (wy1 - wy0 + 2) as usize);

        // The window follows the content, so the scratch is sized for the whole
        // canvas the first time round rather than growing frame by frame.
        let full = (width as usize + 2) * (height as usize + 2);
        self.mask.clear();
        self.mask.reserve(full);
        self.mask.resize(gw * gh, 0);
        for y in wy0..wy1 {
            let row = &mut self.mask[(y - wy0 + 1) as usize * gw + 1..][..(wx1 - wx0) as usize];
            for (k, m) in row.iter_mut().enumerate() {
                *m = sil.covered(wx0 + k as i32, y) as u8;
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

        let w = width as usize;
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
                    let current = target.dots[d];
                    let color = (current != 0).then(|| Color::from_packed(current));
                    let sample = Sample { x, y, dist, color, sil };
                    if let Some(v) = effect.shade(&sample).and_then(|paint| paint.sample(x, y)) {
                        target.dots[d] = v;
                        if let Some(p) = target.prio.get_mut(d) {
                            *p = if v == 0 { 0 } else { prio };
                        }
                    }
                }
            }
        }
    }
}

impl Canvas {
    /// Runs `effects` around the silhouette of `mask` (its set dots and printed
    /// cells) on this canvas, exactly as [`Layers::flatten`] would around a layer:
    /// an outline, a glow, a rim or a shadow against any mask you hand in, with no
    /// stack involved. The mask is usually a scratch canvas of the same size that a
    /// shape was drawn on. This allocates the scratch the effects run in; a kept
    /// [`Field`] does not.
    ///
    /// ```
    /// use cobra::{Canvas, Effect, Rgb};
    ///
    /// let mut canvas = Canvas::new(10, 5);
    /// canvas.fill_rect(0.0, 0.0, 20.0, 20.0, Rgb::hex(0x203040));
    /// let mut mask = Canvas::new(10, 5);
    /// mask.disc(10.0, 10.0, 5.0, Rgb::hex(0xffffff));
    /// canvas.effects(&mask, &[Effect::gap(1.0), Effect::outline(1.0).paint(Rgb::hex(0xffffff))]);
    /// ```
    pub fn effects(&mut self, mask: &Canvas, effects: &[Effect]) {
        Field::default().effects(self, mask, effects);
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
    let (mut x0, mut y0, mut x1, mut y1) = canvas.dot_bounds().unwrap_or((w, h, 0, 0));
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
pub(crate) const INF: f32 = 1e10;

/// Scratch for the exact Euclidean distance transform (Felzenszwalb–Huttenlocher):
/// a lower envelope of parabolas per line, in two separable passes.
#[derive(Clone, Debug, Default)]
pub(crate) struct Edt {
    f: Vec<f32>,
    d: Vec<f32>,
    v: Vec<usize>,
    z: Vec<f32>,
}

impl Edt {
    /// Replaces `grid` (`0` at sources, [`INF`] elsewhere, `w × h` row-major) with
    /// the squared distance of every entry to its nearest source.
    pub(crate) fn run(&mut self, grid: &mut [f32], w: usize, h: usize) {
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
                l.field.mask.capacity(),
                l.field.outside.capacity(),
                l.field.inside.capacity(),
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

    #[test]
    fn mattes_hide_without_painting() {
        let mut l = Layers::new(4, 2);
        l[0].fill_rect(0.0, 0.0, 8.0, 8.0, A);
        l[0].print(3, 1, "x", B);
        let matte = l.push();
        matte.disc(4.0, 4.0, 2.5, B);
        matte.print(3, 1, " ", B);
        matte.matte = true;
        matte.effect(Effect::outline(1.0).paint(B));
        let flat = l.flatten();
        assert_eq!(flat.get(4, 4), None, "the matte's dots are holes");
        assert_eq!(flat.get(0, 0), Some(Color::Rgb(A)));
        assert_eq!(flat.get(4, 1), Some(Color::Rgb(B)), "the outline runs around the hole");
        assert!(flat.text_cell(3, 1).is_none(), "a printed cell on a matte clears the cell");
        assert_eq!(flat.get(6, 4), None);
    }

    #[test]
    fn an_offset_layer_moves_with_its_text_and_effects() {
        let mut l = Layers::new(6, 3);
        l[0].fill_rect(0.0, 0.0, 12.0, 12.0, A);
        let top = l.push();
        top.fill_rect(0.0, 0.0, 4.0, 4.0, B);
        top.print(0, 0, "x", A);
        top.effect(Effect::shadow(1, 1).paint(Rgb::hex(0x111111)));
        top.offset = (3, 5);
        let flat = l.flatten().clone();
        assert_eq!(flat.get(0, 0), Some(Color::Rgb(A)), "nothing of the moved layer stays behind");
        assert_eq!(flat.get(3, 5), Some(Color::Rgb(B)));
        assert_eq!(flat.get(6, 8), Some(Color::Rgb(B)));
        assert_eq!(flat.get(7, 9), Some(Color::Rgb(Rgb::hex(0x111111))), "the shadow follows");
        assert_eq!(flat.get(2, 4), Some(Color::Rgb(A)), "no shadow up-left of the moved box");
        assert!(flat.text_cell(0, 0).is_none());
        assert_eq!(flat.text_cell(2, 1).map(|t| t.ch), Some('x'), "3 dots is two cells, 5 dots one row");
        // Off the stack is gone; text on a cell that would be off the stack too.
        l[1].offset = (-3, 0);
        let flat = l.flatten().clone();
        assert_eq!(flat.get(0, 0), Some(Color::Rgb(B)));
        assert_eq!(flat.get(1, 0), Some(Color::Rgb(A)), "only one column of the box remains");
        assert_eq!(flat.get(1, 1), Some(Color::Rgb(Rgb::hex(0x111111))), "and the shadow of its neighbour");
        assert!(!flat.has_text() || flat.text_cell(0, 0).is_none(), "the cell went off the left");
        l[1].offset = (100, 0);
        assert_eq!(lit(l.flatten()), 144, "a layer moved off entirely draws nothing");
        // A shader sees stack coordinates and covered() in them too.
        l[1].offset = (4, 4);
        l[1].effects = vec![Effect::shader(1.0, 1.0, |s| {
            assert_eq!(s.inside(), s.covered(s.x, s.y));
            assert_eq!(s.layer().get(s.x - 4, s.y - 4).is_some(), s.inside());
            None
        })];
        l.flatten();
    }

    #[test]
    fn a_wrapping_layer_comes_back_on_the_other_side() {
        let mut l = Layers::new(4, 2);
        let top = l.push();
        top.fill_rect(0.0, 0.0, 3.0, 3.0, B);
        top.print(0, 0, "w", A);
        top.wrap = true;
        top.offset = (-1, -1);
        let flat = l.flatten().clone();
        assert_eq!(flat.get(0, 0), Some(Color::Rgb(B)));
        assert_eq!(flat.get(1, 1), Some(Color::Rgb(B)));
        assert_eq!(flat.get(2, 2), None, "the box is 3 wide, moved up-left by one");
        assert_eq!(flat.get(7, 7), Some(Color::Rgb(B)), "its first row and column wrap to the far edges");
        assert_eq!(flat.get(7, 0), Some(Color::Rgb(B)));
        assert_eq!(flat.get(0, 7), Some(Color::Rgb(B)));
        assert_eq!(lit(&flat), 9);
        assert_eq!(flat.text_cell(0, 0).map(|t| t.ch), Some('w'), "one dot rounds to no cell");
        l[1].offset = (-1, -3);
        assert_eq!(l.flatten().text_cell(0, 1).map(|t| t.ch), Some('w'), "three dots round to a row: the last");
        // Any offset is the same modulo the stack, and scroll keeps it there.
        l[1].offset = (7, 7);
        assert_eq!(l.flatten().clone(), flat);
        l[1].scroll(-8, 16);
        assert_eq!(l[1].offset, (7, 7));
        assert_eq!(l.flatten().clone(), flat);
        // Effects wrap with the silhouette: an outline of the box at the far corner.
        l[1].effect(Effect::outline(1.0).paint(A));
        let flat = l.flatten().clone();
        assert_eq!(flat.get(6, 6), Some(Color::Rgb(A)));
        assert_eq!(flat.get(2, 2), Some(Color::Rgb(A)));
        assert_eq!(flat.get(3, 3), None);
    }

    #[test]
    fn a_kept_field_matches_a_fresh_one_and_stops_allocating() {
        let mut mask = Canvas::new(10, 4);
        let effects = [Effect::glow(3.0).paint(A), Effect::rim(1.0)];
        let mut field = Field::new();
        let mut caps = Vec::new();
        for t in 0..3 {
            mask.clear();
            mask.disc(6.0 + 3.0 * t as f32, 8.0, 4.0, B);
            let mut kept = Canvas::new(10, 4);
            field.effects(&mut kept, &mask, &effects);
            let mut fresh = Canvas::new(10, 4);
            fresh.effects(&mask, &effects);
            assert_eq!(kept, fresh, "frame {t}");
            assert!(lit(&kept) > 0);
            caps.push((field.mask.capacity(), field.outside.capacity(), field.inside.capacity()));
        }
        assert_eq!(caps[0], caps[1]);
        assert_eq!(caps[1], caps[2]);
    }

    #[test]
    fn effects_run_against_any_mask() {
        let mut c = Canvas::new(8, 4);
        c.fill_rect(0.0, 0.0, 16.0, 16.0, A);
        let mut mask = Canvas::new(8, 4);
        mask.fill_rect(6.0, 6.0, 4.0, 4.0, B);
        c.effects(&mask, &[Effect::gap(1.0), Effect::rim(1.0).paint(B)]);
        assert_eq!(c.get(5, 5), None, "gap around the mask");
        assert_eq!(c.get(6, 6), Some(Color::Rgb(B)), "rim inside it");
        assert_eq!(c.get(4, 4), Some(Color::Rgb(A)));
        assert_eq!(c.get(8, 8), Some(Color::Rgb(A)), "the mask itself is not painted");
    }
}
