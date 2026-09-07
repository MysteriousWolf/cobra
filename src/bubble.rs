//! Text boxes and speech bubbles.
//!
//! A [`Bubble`] is a body (rectangle, rounded box, ellipse, cloud or starburst) with
//! text in it and, optionally, a tail pointing at whoever is speaking. Without a tail
//! it is a text box; with one it is a chat bubble, and the tail can leave from any
//! side at any point along it.
//!
//! ```
//! use cobra::{Bubble, Canvas, Rgb};
//!
//! let mut c = Canvas::new(40, 10);
//! Bubble::speech("Hello!")
//!     .fill(Rgb::hex(0x1b2430))
//!     .border(1.0, Rgb::hex(0x3aa0ff))
//!     .ink(Rgb::hex(0xc9d1d9))
//!     .draw(&mut c, 4.0, 4.0);
//! ```
//!
//! The text is [real text](crate::text) by default, so it stays sharp and copyable;
//! [`Bubble::font`] switches it to dots in a bitmap [`Font`] when a bubble has to fit
//! somewhere too small for a character cell.
//!
//! # Choosing a place
//!
//! [`Bubble::speak`] takes the mouth to point at and any number of keep-out
//! rectangles, tries the bubble on all four sides of the mouth, and draws the one that
//! stays on the canvas and clear of the zones, with the tail leaning over to reach the
//! mouth:
//!
//! ```
//! # use cobra::{Bubble, Canvas, Rect, Rgb};
//! # let mut canvas = Canvas::new(60, 12);
//! let face = Rect::new(40.0, 20.0, 24.0, 20.0);
//! let mouth = (52.0, 30.0);
//! Bubble::speech("Watch out!").fill(Rgb::hex(0x203040)).speak(&mut canvas, mouth, &[face]);
//! ```

use crate::text::{Align, TextStyle};
use crate::{Canvas, Font, Paint, Point, Rect, text};

/// A cell in dots, as the `f32` the geometry works in.
const CELL_W: f32 = crate::DOTS_X as f32;
const CELL_H: f32 = crate::DOTS_Y as f32;

/// An edge of a bubble.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    /// Above the body.
    Top,
    /// Right of the body.
    Right,
    /// Below the body.
    Bottom,
    /// Left of the body.
    Left,
}

impl Side {
    /// Unit vector pointing out of the body.
    fn out(self) -> Point {
        match self {
            Side::Top => (0.0, -1.0),
            Side::Right => (1.0, 0.0),
            Side::Bottom => (0.0, 1.0),
            Side::Left => (-1.0, 0.0),
        }
    }

    /// Point `at` (`0..=1`) along this side of `body`.
    fn along(self, body: Rect, at: f32) -> Point {
        let t = at.clamp(0.0, 1.0);
        match self {
            Side::Top => (body.x + body.w * t, body.y),
            Side::Bottom => (body.x + body.w * t, body.bottom()),
            Side::Left => (body.x, body.y + body.h * t),
            Side::Right => (body.right(), body.y + body.h * t),
        }
    }

    /// Whether the side runs horizontally.
    fn horizontal(self) -> bool {
        matches!(self, Side::Top | Side::Bottom)
    }
}

/// The shape of a bubble's body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// Plain box.
    Rect,
    /// Box with corners rounded to a radius in dots.
    Round(f32),
    /// Ellipse through the body's box.
    Ellipse,
    /// Scalloped outline of overlapping lobes: a thought bubble.
    Cloud,
    /// Spiky starburst: a shout.
    Burst,
}

/// What a tail looks like.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TailKind {
    /// A straight triangle, the usual speech tail.
    Point,
    /// A curved, tapering comic tail.
    Curve,
    /// Shrinking discs, the usual thought-bubble trail.
    Bubbles,
}

/// Where and how a bubble's tail leaves the body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tail {
    /// Which side it leaves from.
    pub side: Side,
    /// Where along that side, `0..=1` (left to right, top to bottom).
    pub at: f32,
    /// How far it reaches, in dots.
    pub len: f32,
    /// Width of its base, in dots.
    pub width: f32,
    /// Its look.
    pub kind: TailKind,
    /// Where the point ends up. `None` puts it straight out from the base, which is
    /// what a hand-placed bubble wants; [`Bubble::speak`] sets it to the mouth.
    pub tip: Option<Point>,
}

impl Default for Tail {
    fn default() -> Self {
        Self { side: Side::Bottom, at: 0.25, len: 6.0, width: 5.0, kind: TailKind::Point, tip: None }
    }
}

impl Tail {
    /// A tail of `kind` leaving `side` at `at` (`0..=1` along it).
    pub fn new(side: Side, at: f32, kind: TailKind) -> Self {
        Self { side, at, kind, ..Self::default() }
    }

    /// Sets how far the tail reaches, in dots.
    pub fn len(mut self, len: f32) -> Self {
        self.len = len;
        self
    }

    /// Sets the width of the tail's base, in dots.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// The tip for a tail leaving the body at `base`: straight out unless one was set.
    fn tip_from(&self, base: Point) -> Point {
        let (ox, oy) = self.side.out();
        self.tip.unwrap_or((base.0 + ox * self.len, base.1 + oy * self.len))
    }
}

/// A text box, or a chat bubble when it has a [`Tail`]. See the [module docs](self).
#[derive(Clone, Copy, Debug)]
pub struct Bubble<'a> {
    text: &'a str,
    shape: Shape,
    tail: Option<Tail>,
    fill: Option<Paint>,
    border: Option<(f32, Paint)>,
    ink: TextStyle,
    font: Option<&'a Font>,
    pad: (i32, i32),
    align: Align,
    wrap: i32,
    clear_behind: bool,
}

impl<'a> Bubble<'a> {
    /// A plain text box: a rectangle, no tail.
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            shape: Shape::Rect,
            tail: None,
            fill: None,
            border: None,
            ink: TextStyle::default(),
            font: None,
            pad: (1, 0),
            align: Align::Left,
            wrap: 0,
            clear_behind: false,
        }
    }

    /// A rounded box with a triangular tail: an ordinary speech bubble.
    pub fn speech(text: &'a str) -> Self {
        Self { shape: Shape::Round(4.0), tail: Some(Tail::default()), pad: (1, 1), ..Self::new(text) }
    }

    /// A cloud with a trail of discs: a thought bubble.
    pub fn thought(text: &'a str) -> Self {
        Self {
            shape: Shape::Cloud,
            tail: Some(Tail { kind: TailKind::Bubbles, len: 14.0, width: 6.0, ..Tail::default() }),
            align: Align::Center,
            pad: (1, 1),
            ..Self::new(text)
        }
    }

    /// A starburst with a straight tail: a shout.
    pub fn shout(text: &'a str) -> Self {
        Self {
            shape: Shape::Burst,
            tail: Some(Tail { width: 7.0, ..Tail::default() }),
            align: Align::Center,
            pad: (1, 1),
            ..Self::new(text)
        }
    }

    /// A rounded box with a curling comic tail: an aside.
    pub fn whisper(text: &'a str) -> Self {
        Self {
            shape: Shape::Round(3.0),
            tail: Some(Tail { kind: TailKind::Curve, len: 8.0, width: 4.0, ..Tail::default() }),
            pad: (1, 1),
            ..Self::new(text)
        }
    }

    /// Sets the body shape.
    pub fn shape(mut self, shape: Shape) -> Self {
        self.shape = shape;
        self
    }

    /// Attaches a tail.
    pub fn tail(mut self, tail: Tail) -> Self {
        self.tail = Some(tail);
        self
    }

    /// Removes the tail, leaving a text box.
    pub fn no_tail(mut self) -> Self {
        self.tail = None;
        self
    }

    /// Fills the body.
    pub fn fill(mut self, paint: impl Into<Paint>) -> Self {
        self.fill = Some(paint.into());
        self
    }

    /// Outlines the body with a border `width` dots thick.
    pub fn border(mut self, width: f32, paint: impl Into<Paint>) -> Self {
        self.border = Some((width, paint.into()));
        self
    }

    /// Sets the text style. Its background defaults to the bubble's fill, since a
    /// printed character replaces the dots in its cell.
    pub fn ink(mut self, ink: impl Into<TextStyle>) -> Self {
        self.ink = ink.into();
        self
    }

    /// Draws the text as dots in `font` instead of as real characters. Use it when a
    /// bubble is too small for a character cell, or in a drawing that is exported to
    /// an image.
    pub fn font(mut self, font: &'a Font) -> Self {
        self.font = Some(font);
        self
    }

    /// Padding between the text and the body edge, in cells. Default `(1, 0)`, and at
    /// least one row when the bubble has a border and prints real text: a character
    /// hides the dots in its cell, and the top and bottom of the border are in it.
    pub fn pad(mut self, cols: i32, rows: i32) -> Self {
        self.pad = (cols.max(0), rows.max(0));
        self
    }

    /// Aligns the lines inside the bubble.
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// Wraps the text to at most `cols` cells wide.
    pub fn wrap(mut self, cols: i32) -> Self {
        self.wrap = cols.max(0);
        self
    }

    /// Unsets the dots under the body before drawing, so the bubble reads on top of a
    /// busy background even when it is not filled.
    pub fn clear_behind(mut self) -> Self {
        self.clear_behind = true;
        self
    }

    /// Padding actually used: a bordered bubble of real text needs a row of its own
    /// for the border, since a printed character owns its whole cell.
    fn padding(&self) -> (i32, i32) {
        let rows = if self.border.is_some() && self.font.is_none() { self.pad.1.max(1) } else { self.pad.1 };
        (self.pad.0, rows)
    }

    /// Size of the body in dots, always a whole number of cells so the text inside
    /// lands on the character grid.
    pub fn size(&self) -> (f32, f32) {
        let (w, h) = self.content_cells();
        let pad = self.padding();
        let (mut cols, mut rows) = (w + 2 * pad.0, h + 2 * pad.1);
        // Round shapes lose the corners, so they need room the box does not, and a
        // burst has to be wide enough that its notches clear the text.
        match self.shape {
            Shape::Ellipse => {
                cols = (cols as f32 * 1.5).ceil() as i32;
                rows = (rows + 1).max((rows as f32 * 1.6).ceil() as i32);
            }
            Shape::Cloud => {
                // A cloud is wider than it is tall: one row of lobes above and below
                // the text, and enough width for the lobes to read as a row of them.
                rows += 1;
                cols = (cols as f32 * 1.6).ceil().max(rows as f32 * 2.0).ceil() as i32;
            }
            Shape::Burst => {
                cols = (cols as f32 * 1.8).ceil() as i32;
                rows = (rows + 1).max((rows as f32 * 1.8).ceil() as i32);
            }
            _ => {}
        }
        (cols.max(1) as f32 * CELL_W, rows.max(1) as f32 * CELL_H)
    }

    /// The body's box with its top-left at `(x, y)`, snapped to the cell grid.
    fn body_at(&self, x: f32, y: f32) -> Rect {
        let (w, h) = self.size();
        Rect::new(snap(x, CELL_W), snap(y, CELL_H), w, h)
    }

    /// Everything the bubble would cover if drawn at `(x, y)`: the body and its tail.
    /// Useful as a keep-out zone for the next bubble.
    pub fn bounds(&self, x: f32, y: f32) -> Rect {
        let body = self.body_at(x, y);
        let Some(tail) = self.tail else { return body };
        let tip = tail.tip_from(self.tail_anchor(body, &tail).base);
        let (x0, y0) = (body.x.min(tip.0), body.y.min(tip.1));
        Rect::new(x0, y0, body.right().max(tip.0) - x0, body.bottom().max(tip.1) - y0)
    }

    /// Draws the bubble with the top-left of its body at dot `(x, y)`, snapped to the
    /// cell grid, and returns the body's box. The tail may reach outside it.
    pub fn draw(&self, canvas: &mut Canvas, x: f32, y: f32) -> Rect {
        let body = self.body_at(x, y);
        if self.clear_behind {
            self.paint(canvas, body, self.border_width(), Paint::erase());
        }
        // Border first, then the fill inset by it: one path draws every shape, and an
        // unfilled body carves its inside back out.
        match (self.border, self.fill) {
            (Some((t, border)), fill) => {
                self.paint(canvas, body, 0.0, border);
                self.paint(canvas, body, -t, fill.unwrap_or(Paint::erase()));
            }
            (None, Some(fill)) => self.paint(canvas, body, 0.0, fill),
            (None, None) => {}
        }
        self.paint_text(canvas, body);
        body
    }

    /// Draws the bubble where it fits best: pointing its tail at `mouth`, inside the
    /// canvas and off every rectangle in `keep_out`. Returns the body's box.
    ///
    /// The four sides are tried in turn (above the mouth first, then beside it, then
    /// below); each candidate is pushed back onto the canvas, and the one that covers
    /// the least of the keep-out zones with the least tail lean wins.
    pub fn speak(&self, canvas: &mut Canvas, mouth: Point, keep_out: &[Rect]) -> Rect {
        let area = Rect::new(0.0, 0.0, canvas.width() as f32, canvas.height() as f32);
        let (placed, at) = self.place(area, mouth, keep_out);
        placed.draw(canvas, at.0, at.1)
    }

    /// The placement [`speak`](Self::speak) would use: a copy of the bubble with its
    /// tail aimed at `mouth`, and the top-left dot to draw it at.
    pub fn place(&self, area: Rect, mouth: Point, keep_out: &[Rect]) -> (Self, Point) {
        let (w, h) = self.size();
        let mut tail = self.tail.unwrap_or_default();
        let gap = tail.len.max(2.0);
        let mut best: Option<(f32, Point, Side)> = None;
        // Bias: a bubble above the speaker reads first, then beside, then below.
        for (bias, side) in [(0.0, Side::Bottom), (1.0, Side::Right), (1.0, Side::Left), (2.0, Side::Top)] {
            let corner = match side {
                Side::Bottom => (mouth.0 - w / 2.0, mouth.1 - gap - h),
                Side::Top => (mouth.0 - w / 2.0, mouth.1 + gap),
                Side::Right => (mouth.0 - gap - w, mouth.1 - h / 2.0),
                Side::Left => (mouth.0 + gap, mouth.1 - h / 2.0),
            };
            let x = snap(corner.0.clamp(area.x, (area.right() - w).max(area.x)), CELL_W);
            let y = snap(corner.1.clamp(area.y, (area.bottom() - h).max(area.y)), CELL_H);
            let body = Rect::new(x, y, w, h);
            // Everything is an area in dots², so the terms compare directly; the lean
            // is charged the strip of body it drags the tail across.
            let mut score = keep_out.iter().map(|k| body.overlap(k)).sum::<f32>();
            score += 2.0 * (w * h - body.overlap(&area));
            let base = tail_base(body, side, mouth);
            let lean = if side.horizontal() { (mouth.0 - base.0).abs() } else { (mouth.1 - base.1).abs() };
            score += lean * 4.0 + bias * w.min(h);
            if body.contains(mouth) {
                score += w * h * 8.0;
            }
            if best.as_ref().is_none_or(|(b, ..)| score < *b) {
                best = Some((score, (x, y), side));
            }
        }
        let (_, at, side) = best.expect("four candidates");
        let body = Rect::new(at.0, at.1, w, h);
        let base = tail_base(body, side, mouth);
        tail.side = side;
        tail.at = if side.horizontal() { (base.0 - body.x) / w } else { (base.1 - body.y) / h };
        tail.tip = Some(mouth);
        (Self { tail: Some(tail), ..*self }, at)
    }

    /// Border thickness in dots, `0.0` without one.
    fn border_width(&self) -> f32 {
        self.border.map_or(0.0, |(t, _)| t)
    }

    /// Text size in cells.
    fn content_cells(&self) -> (i32, i32) {
        match self.font {
            Some(f) => {
                // Dots, not cells: the border needs its own room here, since nothing
                // else keeps the glyphs off it.
                let (w, h) = self.measure_font(f);
                let t = 2 * self.border_width().ceil() as i32;
                ((w + t + CELL_W as i32 - 1) / CELL_W as i32, (h + t + CELL_H as i32 - 1) / CELL_H as i32)
            }
            None => {
                let (mut w, mut lines) = (0, 0);
                for line in text::wrap(self.text, self.wrap) {
                    w = w.max(text::width(line));
                    lines += 1;
                }
                (w, lines.max(1))
            }
        }
    }

    /// Characters per line when drawing in `font`: a cell is two dots wide, so a
    /// `wrap` in cells is that many dots of glyphs.
    fn font_wrap(&self, font: &Font) -> i32 {
        if self.wrap == 0 { 0 } else { (self.wrap * CELL_W as i32 / font.advance('n').max(1)).max(1) }
    }

    /// Size of the wrapped text in dots when drawn in `font`.
    fn measure_font(&self, font: &Font) -> (i32, i32) {
        let (mut w, mut lines) = (0, 0);
        for line in text::wrap(self.text, self.font_wrap(font)) {
            w = w.max(font.measure(line).0);
            lines += 1;
        }
        (w, lines.max(1) * font.line_height() - font.line_gap() as i32)
    }

    /// The text style to print with: a character hides the dots in its cell, so unless
    /// the caller picked a background the fill has to become one, or the text would
    /// punch holes in the bubble.
    fn text_style(&self) -> TextStyle {
        let mut ink = self.ink;
        if ink.bg.is_none()
            && let Some(fill) = self.fill
            && fill.coverage() >= 1.0
            && let Some(color) = fill.color()
        {
            ink.bg = Some(color);
        }
        ink
    }

    /// Prints or draws the text, centred in the body.
    fn paint_text(&self, canvas: &mut Canvas, body: Rect) {
        let (cw, ch) = self.content_cells();
        match self.font {
            Some(f) => {
                let (w, h) = self.measure_font(f);
                let x = body.x as i32 + ((body.w as i32 - w) / 2).max(0);
                let y = body.y as i32 + ((body.h as i32 - h) / 2).max(0);
                let ink = Paint::new(self.ink.fg.unwrap_or(crate::Color::Foreground));
                for (i, line) in text::wrap(self.text, self.font_wrap(f)).enumerate() {
                    let dx = match self.align {
                        Align::Left => 0,
                        Align::Center => (w - f.measure(line).0) / 2,
                        Align::Right => w - f.measure(line).0,
                    };
                    canvas.text(x + dx, y + i as i32 * f.line_height(), line, f, ink);
                }
            }
            None => {
                let col = body.x as i32 / CELL_W as i32 + ((body.w as i32 / CELL_W as i32) - cw) / 2;
                let row = body.y as i32 / CELL_H as i32 + ((body.h as i32 / CELL_H as i32) - ch) / 2;
                // Every line already fits `cw`, so wrapping to it only aligns.
                canvas.print_wrapped(col, row, cw, self.text, self.text_style(), self.align);
            }
        }
    }

    /// Fills the body and the tail grown by `grow` dots (negative shrinks them).
    fn paint(&self, canvas: &mut Canvas, body: Rect, grow: f32, paint: Paint) {
        self.paint_body(canvas, body, grow, paint);
        if let Some(tail) = self.tail {
            paint_tail(canvas, &tail, self.tail_anchor(body, &tail), grow, paint);
        }
    }

    /// Where a tail joins the body. Its `base` is on the side, slid along it if the
    /// body is not solid right to the ends there; its `root` is `depth` inside, where
    /// the body is solid across the whole width the tail needs, so the tail's sides
    /// run in through a rounded corner, a scallop or a notch and meet the outline
    /// wherever it is. A box is solid at its edge; a rounded one a radius in; an
    /// ellipse or burst at the rectangle inscribed in it; a cloud at its inner box,
    /// which the corner lobes complete.
    ///
    /// Every pass roots the tail on the same line, at least a border's width plus a
    /// dot inside the outline: the fill then overlaps the body's fill (no seam), and
    /// its inset tail always lies within the border pass's (no leak).
    fn tail_anchor(&self, body: Rect, tail: &Tail) -> Anchor {
        const CORE: f32 = 1.0 - std::f32::consts::FRAC_1_SQRT_2;
        let (len, across) = if tail.side.horizontal() { (body.w, body.h) } else { (body.h, body.w) };
        let (depth, margin) = match self.shape {
            Shape::Rect => (0.0, 0.0),
            Shape::Round(r) => (r.clamp(0.0, across / 2.0), 0.0),
            Shape::Ellipse => (across / 2.0 * CORE, len / 2.0 * CORE),
            Shape::Cloud => (cloud_lobe(body), 0.0),
            Shape::Burst => {
                let core = 1.0 - BURST_NOTCH * std::f32::consts::FRAC_1_SQRT_2;
                (across / 2.0 * core, len / 2.0 * core)
            }
        };
        // How far out the tail goes is the same wherever it leaves the side, so it
        // is known before the base is: the root is widened in proportion, so the
        // tail is still `width` where it crosses the outline.
        let depth = depth.max(self.border_width() + 1.0).min(across / 2.0);
        let (ox, oy) = tail.side.out();
        let mid = tail.side.along(body, 0.5);
        let tip = tail.tip_from(mid);
        let reach = ((tip.0 - mid.0) * ox + (tip.1 - mid.1) * oy).max(depth).max(1.0);
        let half = tail.width / 2.0 * (reach + depth) / reach;
        let (lo, hi) = (margin + half, len - margin - half);
        let at = if lo <= hi { (tail.at * len).clamp(lo, hi) / len } else { 0.5 };
        let base = tail.side.along(body, at);
        Anchor { base, root: (base.0 - ox * depth, base.1 - oy * depth), half }
    }

    /// Fills the body shape grown by `grow` dots (negative shrinks it).
    fn paint_body(&self, canvas: &mut Canvas, body: Rect, grow: f32, paint: Paint) {
        let r = body.inset(-grow);
        if r.w <= 0.0 || r.h <= 0.0 {
            return;
        }
        let (cx, cy) = r.center();
        match self.shape {
            Shape::Rect => canvas.fill_rect(r.x, r.y, r.w, r.h, paint),
            Shape::Round(rad) => canvas.fill_round_rect(r.x, r.y, r.w, r.h, rad + grow, paint),
            Shape::Ellipse => canvas.fill_ellipse(cx, cy, r.w / 2.0, r.h / 2.0, paint),
            Shape::Cloud => {
                // Lobes strung around an inner box, plus the box itself.
                let lobe = cloud_lobe(body);
                let inner = body.inset(lobe);
                canvas.fill_round_rect(
                    inner.x - grow,
                    inner.y - grow,
                    inner.w + 2.0 * grow,
                    inner.h + 2.0 * grow,
                    lobe,
                    paint,
                );
                for (px, py) in perimeter(inner, lobe * 1.3) {
                    canvas.fill_ellipse(px, py, lobe + grow, lobe + grow, paint);
                }
            }
            Shape::Burst => {
                // Few, deep spikes read as a shout; many shallow ones just look noisy.
                let spikes = ((body.w + body.h) / 16.0).clamp(5.0, 12.0) as u32;
                let mut pts = [(0.0f32, 0.0f32); 32];
                let n = (spikes * 2) as usize;
                let (rx, ry) = (body.w / 2.0, body.h / 2.0);
                for (i, p) in pts[..n].iter_mut().enumerate() {
                    let a = std::f32::consts::TAU * i as f32 / n as f32 - std::f32::consts::FRAC_PI_2;
                    let k = if i % 2 == 0 { 1.0 } else { BURST_NOTCH };
                    *p = (cx + rx * k * a.cos(), cy + ry * k * a.sin());
                }
                offset_polygon(&mut pts[..n], grow);
                canvas.fill_polygon(&pts[..n], paint);
            }
        }
    }
}

/// Moves every edge of the polygon `pts` out by `d` (in when negative), sliding each
/// vertex along the bisector of its two edges, so a border drawn as the difference
/// of two offsets is the same thickness at a point and in a notch. `pts` must wind
/// clockwise on screen.
fn offset_polygon(pts: &mut [Point], d: f32) {
    let n = pts.len();
    if n < 3 || d == 0.0 {
        return;
    }
    let mut src = [(0.0f32, 0.0f32); 32];
    src[..n].copy_from_slice(pts);
    let normal = |a: Point, b: Point| {
        let (ex, ey) = (b.0 - a.0, b.1 - a.1);
        let l = (ex * ex + ey * ey).sqrt().max(1e-6);
        (ey / l, -ex / l)
    };
    for i in 0..n {
        let (p, q, r) = (src[(i + n - 1) % n], src[i], src[(i + 1) % n]);
        let (n1, n2) = (normal(p, q), normal(q, r));
        // The bisector `n1 + n2` scaled so the edges move by exactly `d`; the floor
        // keeps a hairpin vertex from flying off.
        let k = d / (1.0 + n1.0 * n2.0 + n1.1 * n2.1).max(0.05);
        pts[i] = (q.0 + (n1.0 + n2.0) * k, q.1 + (n1.1 + n2.1) * k);
    }
}

/// How far in from the points a burst's notches sit, as a fraction of its radius.
const BURST_NOTCH: f32 = 0.75;

/// Radius of a cloud's lobes: a good fraction of the body, or the outline reads as a
/// rounded box.
fn cloud_lobe(body: Rect) -> f32 {
    (body.w.min(body.h) / 3.5).clamp(3.0, 10.0)
}

/// Where the tail should leave `side` to reach `mouth`, kept away from the corners.
fn tail_base(body: Rect, side: Side, mouth: Point) -> Point {
    let inset = 0.15;
    let t = if side.horizontal() { (mouth.0 - body.x) / body.w } else { (mouth.1 - body.y) / body.h };
    side.along(body, t.clamp(inset, 1.0 - inset))
}

/// Points spaced about `step` dots apart around the outline of `r`, corners
/// included, laid out the same on opposite sides so the shape stays symmetric.
fn perimeter(r: Rect, step: f32) -> impl Iterator<Item = Point> {
    let step = step.max(1.0);
    let (nx, ny) = ((r.w / step).round().max(1.0) as i32, (r.h / step).round().max(1.0) as i32);
    let top_bottom = (0..=nx).flat_map(move |i| {
        let x = r.x + r.w * i as f32 / nx as f32;
        [(x, r.y), (x, r.bottom())]
    });
    let sides = (1..ny).flat_map(move |i| {
        let y = r.y + r.h * i as f32 / ny as f32;
        [(r.x, y), (r.right(), y)]
    });
    top_bottom.chain(sides)
}

/// Where a tail meets its body; see [`Bubble::tail_anchor`].
#[derive(Clone, Copy, Debug)]
struct Anchor {
    /// Where the tail crosses the body's outline.
    base: Point,
    /// Where its base line really is, inside the body.
    root: Point,
    /// Half its width at the root.
    half: f32,
}

/// Fills a tail grown by `grow` dots, the same way [`Bubble::paint_body`] does: the
/// same tail drawn at `0.0` and then at `-t` leaves a border `t` thick all round.
fn paint_tail(canvas: &mut Canvas, tail: &Tail, anchor: Anchor, grow: f32, paint: Paint) {
    let Anchor { base, root, half } = anchor;
    let tip = tail.tip_from(base);
    let (ox, oy) = tail.side.out();
    let (sx, sy) = if tail.side.horizontal() { (1.0, 0.0) } else { (0.0, 1.0) };
    let hw = tail.width / 2.0;
    let (dx, dy) = (tip.0 - root.0, tip.1 - root.1);
    let len = (dx * dx + dy * dy).sqrt();
    if len <= 0.0 {
        return;
    }
    if tail.kind == TailKind::Bubbles {
        // Three shrinking discs on the line from base to tip, each clear of the
        // last and the first clear of the body, centred the same in every pass so
        // their borders stay even. Snapped to dot centres, so a disc a few dots
        // across is round, not lopsided.
        for (t, scale) in [(0.25, 0.8), (0.62, 0.5), (0.95, 0.3)] {
            let r = hw * scale + grow;
            if r <= 0.0 {
                continue; // too small to have an inside: the border pass is the whole disc
            }
            let (cx, cy) = (base.0 + (tip.0 - base.0) * t, base.1 + (tip.1 - base.1) * t);
            canvas.fill_ellipse(cx.floor() + 0.5, cy.floor() + 0.5, r, r, paint);
        }
        return;
    }
    // Offsetting the triangle's sides by `grow` moves its tip along the axis by
    // `grow · hyp / half`; the base line stays at the root, inside the body, and the
    // corners are where the shifted sides cross it. A thin tail's inset vanishes
    // well before the tip, which is what keeps its border even right to the point.
    let (ux, uy) = (dx / len, dy / len);
    let hyp = (half * half + len * len).sqrt();
    let apex = (tip.0 + ux * grow * hyp / half, tip.1 + uy * grow * hyp / half);
    if (apex.0 - root.0) * ox + (apex.1 - root.1) * oy <= 0.0 {
        return;
    }
    let corner = |sign: f32| {
        // The shifted side runs through the apex parallel to the original one.
        let (vx, vy) = (root.0 + sx * half * sign - tip.0, root.1 + sy * half * sign - tip.1);
        let along = vx * ox + vy * oy;
        let s = if along.abs() < 1e-3 { 0.0 } else { ((root.0 - apex.0) * ox + (root.1 - apex.1) * oy) / along };
        (apex.0 + vx * s, apex.1 + vy * s)
    };
    let (b0, b1) = (corner(-1.0), corner(1.0));
    match tail.kind {
        TailKind::Point | TailKind::Bubbles => canvas.fill_polygon(&[b0, b1, apex], paint),
        TailKind::Curve => {
            // Two quadratics from the base corners to the tip, bowed the same way: the
            // tail leaves the body at full width and curls to a point. The curl is
            // set by the tail's true width, so every pass follows the same curve.
            let c0 = (b0.0 + ox * len * 0.35, b0.1 + oy * len * 0.35);
            let c1 = (b1.0 + ox * len * 0.6 + sx * hw * 1.5, b1.1 + oy * len * 0.6 + sy * hw * 1.5);
            let q = |a: Point, c: Point, b: Point, t: f32| {
                let u = 1.0 - t;
                (u * u * a.0 + 2.0 * u * t * c.0 + t * t * b.0, u * u * a.1 + 2.0 * u * t * c.1 + t * t * b.1)
            };
            const N: usize = 12;
            let mut pts = [(0.0f32, 0.0f32); 2 * N + 2];
            for i in 0..=N {
                let t = i as f32 / N as f32;
                pts[i] = q(b0, c0, apex, t);
                pts[2 * N + 1 - i] = q(b1, c1, apex, t);
            }
            canvas.fill_polygon(&pts, paint);
        }
    }
}

/// Rounds `v` to the nearest multiple of `step`, so text lands on the cell grid.
fn snap(v: f32, step: f32) -> f32 {
    (v / step).round() * step
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rgb;

    const INK: Rgb = Rgb::hex(0xffffff);

    fn lit(c: &Canvas) -> usize {
        c.cells().map(|cell| cell.bits.count_ones() as usize).sum()
    }

    #[test]
    fn a_text_box_is_a_bubble_without_a_tail() {
        let mut c = Canvas::new(20, 4);
        let b = Bubble::new("hi").fill(Rgb::hex(0x202020)).ink(INK);
        assert_eq!(b.size(), (8.0, 4.0), "2 cells of text plus a cell of padding either side");
        assert_eq!(b.bounds(2.0, 4.0), Rect::new(2.0, 4.0, 8.0, 4.0), "no tail, no reach past the body");
        let body = b.draw(&mut c, 2.0, 4.0);
        assert_eq!(body, Rect::new(2.0, 4.0, 8.0, 4.0));
        assert_eq!(c.text_cell(2, 1).map(|t| t.ch), Some('h'));
        assert_eq!(c.text_cell(3, 1).map(|t| t.ch), Some('i'));
        assert!(c.get(2, 4).is_some() && c.get(0, 4).is_none());
    }

    #[test]
    fn drawing_snaps_to_the_cell_grid() {
        let mut c = Canvas::new(20, 4);
        let body = Bubble::new("x").ink(INK).draw(&mut c, 3.0, 5.0);
        assert_eq!((body.x % 2.0, body.y % 4.0), (0.0, 0.0));
    }

    #[test]
    fn every_shape_fills_and_borders() {
        for shape in [Shape::Rect, Shape::Round(3.0), Shape::Ellipse, Shape::Cloud, Shape::Burst] {
            let mut c = Canvas::new(24, 6);
            let b = Bubble::new("ab cd").wrap(4).shape(shape).fill(Rgb::hex(0x102030)).border(1.0, INK).ink(INK);
            let body = b.draw(&mut c, 4.0, 4.0);
            assert!(body.w >= 8.0 && body.h >= 8.0, "{shape:?} body {body:?}");
            assert!(lit(&c) > 20, "{shape:?} drew nothing");
            let printed = (0..24)
                .flat_map(|col| (0..6).map(move |row| (col, row)))
                .filter(|&(c2, r)| c.text_cell(c2, r).is_some_and(|t| t.ch == 'a' || t.ch == 'c'));
            assert_eq!(printed.count(), 2, "{shape:?} lost its text");
        }
    }

    #[test]
    fn tails_leave_from_every_side() {
        for side in [Side::Top, Side::Right, Side::Bottom, Side::Left] {
            let mut c = Canvas::new(20, 6);
            let b = Bubble::new("hi").tail(Tail::new(side, 0.5, TailKind::Point).len(6.0)).fill(INK).ink(0u32);
            let body = b.draw(&mut c, 8.0, 8.0);
            let (ox, oy) = side.out();
            let (px, py) = (body.center().0 + ox * (body.w / 2.0 + 2.0), body.center().1 + oy * (body.h / 2.0 + 2.0));
            assert!(c.get(px as i32, py as i32).is_some(), "{side:?} tail missing at {px},{py}");
        }
    }

    #[test]
    fn tails_join_the_body_without_a_seam() {
        let fill = Rgb::hex(0x102030);
        let mut c = Canvas::new(20, 8);
        let tail = Tail::new(Side::Bottom, 0.5, TailKind::Point).len(6.0).width(6.0);
        let body = Bubble::new("hi").tail(tail).fill(fill).border(1.0, INK).ink(INK).draw(&mut c, 4.0, 4.0);
        let x = body.center().0 as i32;
        // The border row of the body and the first row of the tail are both fill
        // where the tail leaves: no border line across its base.
        for y in [body.bottom() as i32 - 1, body.bottom() as i32] {
            assert_eq!(c.get(x, y), Some(crate::Color::Rgb(fill)), "seam at {x},{y}");
        }
        // The tail's own edges are border.
        assert_eq!(c.get(x - 3, body.bottom() as i32), Some(crate::Color::Rgb(INK)));
    }

    #[test]
    fn dot_font_text_keeps_off_the_border() {
        let red = Rgb::hex(0xff0000);
        let mut c = Canvas::new(20, 6);
        let body = Bubble::new("no cells").font(Font::tiny()).wrap(6).border(1.0, INK).ink(red).draw(&mut c, 0.0, 0.0);
        let (top, bottom) = (body.y as i32, body.bottom() as i32 - 1);
        for x in body.x as i32..body.right() as i32 {
            assert_ne!(c.get(x, top), Some(crate::Color::Rgb(red)), "text on the top border at {x}");
            assert_ne!(c.get(x, bottom), Some(crate::Color::Rgb(red)), "text on the bottom border at {x}");
        }
        assert!(c.cells().any(|cell| cell.bits != 0), "and the text is drawn");
    }

    #[test]
    fn placement_avoids_keep_out_zones_and_the_edges() {
        let c = Canvas::new(30, 8);
        let area = Rect::new(0.0, 0.0, c.width() as f32, c.height() as f32);
        let b = Bubble::speech("hello there");
        // Speaking from the top of the canvas: the bubble cannot go above, so it does
        // not, and it still points at the mouth.
        let (placed, at) = b.place(area, (30.0, 2.0), &[]);
        let body = Rect::new(at.0, at.1, placed.size().0, placed.size().1);
        assert!(body.y >= 0.0 && body.bottom() <= area.bottom() && body.right() <= area.right());
        assert_eq!(placed.tail.unwrap().tip, Some((30.0, 2.0)));
        // With the whole left half blocked, it goes right.
        let block = Rect::new(0.0, 0.0, 30.0, 32.0);
        let (_, at) = b.place(area, (30.0, 16.0), &[block]);
        assert!(at.0 >= 28.0, "moved off the blocked half: {at:?}");
    }

    #[test]
    fn speak_draws_and_reports_where() {
        let mut c = Canvas::new(40, 10);
        let face = Rect::new(40.0, 20.0, 24.0, 20.0);
        let body = Bubble::speech("watch out").fill(Rgb::hex(0x203040)).ink(INK).speak(&mut c, (52.0, 24.0), &[face]);
        assert_eq!(body.overlap(&face), 0.0);
        assert!(c.has_text());
        assert!(lit(&c) > 50);
    }

    #[test]
    fn text_takes_the_fill_as_its_background() {
        let mut c = Canvas::new(10, 2);
        Bubble::new("hi").fill(Rgb::hex(0x203040)).ink(INK).draw(&mut c, 0.0, 0.0);
        assert_eq!(c.text_cell(1, 0).unwrap().style.bg, Some(crate::Color::Rgb(Rgb::hex(0x203040))));
        // An explicit background wins, and a dithered fill is not one.
        let mut c = Canvas::new(10, 2);
        Bubble::new("hi").fill(Paint::dithered(Rgb::hex(0x203040), 0.5)).ink(INK).draw(&mut c, 0.0, 0.0);
        assert_eq!(c.text_cell(1, 0).unwrap().style.bg, None);
    }

    #[test]
    fn font_mode_draws_dots_not_cells() {
        let mut c = Canvas::new(20, 6);
        Bubble::new("hi").font(Font::tiny()).ink(INK).fill(Rgb::hex(0x101010)).draw(&mut c, 0.0, 0.0);
        assert!(!c.has_text(), "font mode never touches the text layer");
        assert!(lit(&c) > 8);
    }

    #[test]
    fn wrapping_grows_the_box_downwards() {
        let one = Bubble::new("a b c d e f").size();
        let many = Bubble::new("a b c d e f").wrap(3).size();
        assert!(many.0 < one.0 && many.1 > one.1);
    }
}
