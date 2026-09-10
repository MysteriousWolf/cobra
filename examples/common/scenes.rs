//! The drawing sequence that the soak harness sends and the soak test checks.
//!
//! One table, two consumers: `examples/soak.rs` draws it at a real terminal and writes
//! a flight-recorder log next to every frame, and `tests/soak.rs` encodes the same
//! scenes for every protocol and decodes them back. A scene that breaks a terminal is
//! therefore always reproducible headlessly, under the same name and phase.
//!
//! Scenes are ordered cheap to expensive, and each leans on a different part of the
//! pipeline: an empty canvas, a single dot, flat fills, a colour per dot (which
//! defeats the deflate), the text layer (which puts the kitty image at `z=-1`),
//! degenerate geometry, and finally canvases far larger than any terminal, to be
//! clipped.

// The harness and the test use different halves of this module.
#![allow(dead_code)]

use std::f32::consts::TAU;

use cobra::{Bubble, Canvas, Color, Effect, Font, Layers, Mask, Paint, Pattern, Pen, Point, Rgb, TextStyle, Transform};

/// One drawing, at one size, as a function of an animation phase.
pub struct Scene {
    /// Stable name: what `--only` matches and what the log and dump files carry.
    pub name: &'static str,
    /// What this scene is meant to stress, for the log.
    pub about: &'static str,
    /// Canvas width in cells.
    pub cols: u16,
    /// Canvas height in cells.
    pub rows: u16,
    /// Draws the scene. `phase` runs `0.0..1.0` across the repeats of a scene.
    pub draw: fn(&mut Canvas, f32),
}

impl Scene {
    /// A canvas of this scene's size with the scene drawn on it.
    pub fn canvas(&self, phase: f32) -> Canvas {
        let mut canvas = Canvas::new(self.cols, self.rows);
        (self.draw)(&mut canvas, phase);
        canvas
    }
}

/// Every scene, in the order the harness sends them.
pub const SCENES: &[Scene] = &[
    Scene { name: "empty", about: "nothing drawn: a fully transparent frame", cols: 20, rows: 4, draw: empty },
    Scene { name: "one-dot", about: "a single lit dot in a transparent field", cols: 20, rows: 4, draw: one_dot },
    Scene { name: "corners", about: "the four extreme dots, so clipping shows", cols: 20, rows: 4, draw: corners },
    Scene { name: "flat-fill", about: "one colour everywhere: best case", cols: 40, rows: 10, draw: flat_fill },
    Scene { name: "gradient", about: "a colour per dot: few matches", cols: 40, rows: 10, draw: gradient },
    Scene { name: "noise", about: "pseudo-random dots: worst payload", cols: 40, rows: 10, draw: noise },
    Scene { name: "lines", about: "Bresenham lines in a fan", cols: 40, rows: 10, draw: lines },
    Scene { name: "fills", about: "rects, ellipses, stars, pies, rings", cols: 40, rows: 12, draw: fills },
    Scene { name: "strokes", about: "stroked shapes, wide pens and dashes", cols: 40, rows: 12, draw: strokes },
    Scene { name: "curves", about: "beziers and splines", cols: 40, rows: 12, draw: curves },
    Scene { name: "paints", about: "dither, pattern, linear and radial gradients", cols: 40, rows: 12, draw: paints },
    Scene { name: "palette", about: "indexed and default-foreground dots", cols: 40, rows: 8, draw: palette },
    Scene { name: "masks", about: "stencil, clip and cut through a mask", cols: 40, rows: 12, draw: masks },
    Scene { name: "layers", about: "stacked layers with shadow, outline and glow", cols: 40, rows: 12, draw: layers },
    Scene { name: "transform", about: "through a rotated, scaled frame", cols: 40, rows: 12, draw: transform },
    Scene { name: "font-text", about: "bitmap-font text as dots", cols: 40, rows: 8, draw: font_text },
    Scene { name: "cell-text", about: "a text layer: kitty sends z=-1", cols: 40, rows: 8, draw: cell_text },
    Scene { name: "bubble", about: "a speech bubble: fill, border, tail and text", cols: 40, rows: 12, draw: bubble },
    Scene { name: "one-cell", about: "a 1x1 canvas: the smallest frame there is", cols: 1, rows: 1, draw: flat_fill },
    Scene { name: "thin-row", about: "one cell tall: degenerate raster geometry", cols: 60, rows: 1, draw: gradient },
    Scene { name: "thin-col", about: "one cell wide and tall", cols: 1, rows: 20, draw: gradient },
    Scene { name: "odd-size", about: "prime-sized: nothing divides evenly", cols: 37, rows: 7, draw: fills },
    Scene { name: "big-flat", about: "past the screen, flat: clipped", cols: 240, rows: 60, draw: flat_fill },
    Scene { name: "big-noise", about: "past the screen, incompressible", cols: 240, rows: 60, draw: noise },
];

/// A deterministic 32-bit hash, so `noise` is the same picture on every machine and
/// every run: a frame that kills a terminal has to be reproducible.
fn hash(x: u32) -> u32 {
    let mut h = x.wrapping_mul(0x9E37_79B9);
    h ^= h >> 15;
    h = h.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    h
}

fn ink(phase: f32, k: u32) -> Rgb {
    let t = (phase * TAU + k as f32 * 1.7).sin() * 0.5 + 0.5;
    Rgb::hex(0x3aa0ff).lerp(Rgb::hex(0xff3355), t)
}

fn empty(_c: &mut Canvas, _phase: f32) {}

fn one_dot(c: &mut Canvas, phase: f32) {
    let x = ((phase * c.width() as f32) as i32).rem_euclid(c.width().max(1));
    c.set(x, c.height() / 2, ink(phase, 0));
}

fn corners(c: &mut Canvas, phase: f32) {
    let (w, h) = (c.width() - 1, c.height() - 1);
    for (k, (x, y)) in [(0, 0), (w, 0), (0, h), (w, h)].into_iter().enumerate() {
        c.set(x, y, ink(phase, k as u32));
    }
}

fn flat_fill(c: &mut Canvas, phase: f32) {
    let color = ink(phase, 0);
    for y in 0..c.height() {
        for x in 0..c.width() {
            c.set(x, y, color);
        }
    }
}

fn gradient(c: &mut Canvas, phase: f32) {
    let (w, h) = (c.width().max(1) as f32, c.height().max(1) as f32);
    for y in 0..c.height() {
        for x in 0..c.width() {
            let t = (x as f32 / w + y as f32 / h + phase) % 1.0;
            c.set(x, y, Rgb::hex(0x5ec33a).lerp(Rgb::hex(0xff00aa), t));
        }
    }
}

fn noise(c: &mut Canvas, phase: f32) {
    let seed = (phase * 1000.0) as u32;
    for y in 0..c.height() {
        for x in 0..c.width() {
            let h = hash(seed ^ hash(x as u32).wrapping_add(y as u32));
            if h & 3 != 0 {
                c.set(x, y, Rgb::new((h >> 8) as u8, (h >> 16) as u8, (h >> 24) as u8));
            }
        }
    }
}

fn lines(c: &mut Canvas, phase: f32) {
    let (w, h) = (c.width(), c.height());
    for k in 0..12 {
        let a = phase * TAU + k as f32 * TAU / 12.0;
        let (x, y) = ((w / 2) as f32 + a.cos() * w as f32, (h / 2) as f32 + a.sin() * h as f32);
        c.line(w / 2, h / 2, x as i32, y as i32, ink(phase, k));
    }
}

fn fills(c: &mut Canvas, phase: f32) {
    let (w, h) = (c.width() as f32, c.height() as f32);
    let t = phase * TAU;
    c.fill_rect(1.0, 1.0, w * 0.2, h * 0.4, ink(phase, 0));
    c.fill_round_rect(w * 0.25, 1.0, w * 0.2, h * 0.4, 3.0, ink(phase, 1));
    c.fill_ellipse(w * 0.6, h * 0.25, w * 0.1, h * 0.2, ink(phase, 2));
    c.fill_ngon(w * 0.85, h * 0.25, h * 0.2, 6, t, ink(phase, 3));
    c.fill_star(w * 0.15, h * 0.72, h * 0.22, h * 0.1, 5, t, ink(phase, 4));
    c.fill_pie(w * 0.45, h * 0.72, w * 0.12, h * 0.22, t, t + 2.0, ink(phase, 5));
    c.ring(w * 0.75, h * 0.72, h * 0.24, h * 0.14, ink(phase, 6));
    c.fill_polygon(&[(w * 0.9, h * 0.55), (w * 0.98, h * 0.9), (w * 0.82, h * 0.9)], ink(phase, 7));
}

fn strokes(c: &mut Canvas, phase: f32) {
    let (w, h) = (c.width() as f32, c.height() as f32);
    let dashed = Pen::new(1.5).dash(3.0, 2.0).phase(phase * 5.0);
    c.rect(1.0, 1.0, w * 0.25, h * 0.4, 1.0, ink(phase, 0));
    c.round_rect(w * 0.35, 1.0, w * 0.25, h * 0.4, 3.0, 2.0, ink(phase, 1));
    c.ellipse(w * 0.8, h * 0.25, w * 0.15, h * 0.2, 3.0, ink(phase, 2));
    c.arc(w * 0.2, h * 0.75, w * 0.12, h * 0.2, phase * TAU, phase * TAU + 2.5, 2.0, ink(phase, 3));
    c.polyline(&[(w * 0.4, h * 0.6), (w * 0.5, h * 0.9), (w * 0.6, h * 0.6)], dashed, ink(phase, 4));
    c.star(w * 0.85, h * 0.75, h * 0.2, h * 0.1, 5, phase * TAU, 1.0, ink(phase, 5));
    c.arrow((w * 0.65, h * 0.9), (w * 0.78, h * 0.6), 2.0, 4.0, ink(phase, 6));
}

fn curves(c: &mut Canvas, phase: f32) {
    let (w, h) = (c.width() as f32, c.height() as f32);
    let wobble = (phase * TAU).sin() * h * 0.3;
    let ctrl: [Point; 4] =
        [(1.0, h * 0.8), (w * 0.3, h * 0.1 + wobble), (w * 0.7, h * 0.9 - wobble), (w - 2.0, h * 0.2)];
    c.bezier(&ctrl, 2.0, ink(phase, 0));
    let pts: Vec<Point> =
        (0..8).map(|k| (k as f32 * w / 7.0, h * 0.5 + (k as f32 + phase * 4.0).sin() * h * 0.3)).collect();
    c.spline(&pts, false, 1.0, ink(phase, 1));
}

fn paints(c: &mut Canvas, phase: f32) {
    let (w, h) = (c.width() as f32, c.height() as f32);
    c.fill_rect(0.0, 0.0, w * 0.24, h, Paint::dithered(ink(phase, 0), 0.35));
    c.fill_rect(w * 0.26, 0.0, w * 0.24, h, Paint::pattern(ink(phase, 1), Pattern::Checker(2)));
    let grad = Paint::linear((w * 0.52, 0.0), (w * 0.74, h), Rgb::hex(0x00ffcc), Rgb::hex(0x3300ff));
    c.fill_rect(w * 0.52, 0.0, w * 0.22, h, grad);
    let radial = Paint::radial((w * 0.88, h * 0.5), h * 0.5, Rgb::hex(0xffee00), Rgb::hex(0x220000));
    c.fill_ellipse(w * 0.88, h * 0.5, w * 0.1, h * 0.45, radial);
}

fn palette(c: &mut Canvas, phase: f32) {
    let (w, h) = (c.width(), c.height());
    let band = (h / 3).max(1);
    for x in 0..w {
        let idx = ((x as usize * 16) / w.max(1) as usize) as u8;
        for y in 0..band {
            c.set(x, y, Color::Indexed(idx));
        }
        for y in band..band * 2 {
            c.set(x, y, Color::Foreground);
        }
        for y in band * 2..h {
            c.set_dithered(x, y, Color::Indexed(idx), 0.25 + 0.5 * phase);
        }
    }
}

fn masks(c: &mut Canvas, phase: f32) {
    let (w, h) = (c.width(), c.height());
    let mut mask = Mask::new(c.cols(), c.rows());
    mask.draw(|m| {
        m.fill_ellipse(w as f32 * 0.5, h as f32 * 0.5, w as f32 * 0.35, h as f32 * 0.4, Rgb::hex(0xffffff));
    });
    c.stencil(&mask, Paint::linear((0.0, 0.0), (w as f32, h as f32), Rgb::hex(0xff8800), Rgb::hex(0x0088ff)));
    c.clipped(&mask, |inner| {
        for k in 0..10 {
            let y = (k as f32 + phase) * h as f32 / 10.0;
            inner.line(0, y as i32, w, y as i32, Rgb::hex(0x000000));
        }
    });
    let mut hole = Mask::new(c.cols(), c.rows());
    hole.draw(|m| {
        m.fill_ngon(w as f32 * 0.5, h as f32 * 0.5, h as f32 * 0.18, 6, phase * TAU, Rgb::hex(0xffffff));
    });
    c.cut(&hole);
}

fn layers(c: &mut Canvas, phase: f32) {
    let mut stack = Layers::new(c.cols(), c.rows());
    let (w, h) = (c.width() as f32, c.height() as f32);
    let back = stack.push();
    back.canvas_mut().fill_rect(0.0, h * 0.6, w, h * 0.4, Paint::dithered(Rgb::hex(0x224466), 0.5));
    back.effect(Effect::outline(1.0).paint(Rgb::hex(0x99ccff)));
    let front = stack.push();
    let cx = w * (0.2 + 0.6 * phase);
    front.canvas_mut().fill_ellipse(cx, h * 0.45, h * 0.22, h * 0.22, Rgb::hex(0xffcc33));
    front.effect(Effect::shadow(2, 1).paint(Rgb::hex(0x000000)));
    front.effect(Effect::glow(2.0).paint(Rgb::hex(0xff6600)));
    c.blit(stack.flatten(), 0, 0);
}

fn transform(c: &mut Canvas, phase: f32) {
    let (w, h) = (c.width() as f32, c.height() as f32);
    let t = Transform::at(w * 0.5, h * 0.5).rotate(phase * TAU).scale(1.0 + 0.3 * phase, 1.0);
    c.with(t, |inner| {
        inner.rect(-w * 0.25, -h * 0.3, w * 0.5, h * 0.6, 1.0, Rgb::hex(0x66ff99));
        inner.fill_ngon(0.0, 0.0, h * 0.2, 3, 0.0, Rgb::hex(0xff3366));
    });
}

fn font_text(c: &mut Canvas, phase: f32) {
    let font = Font::tiny();
    c.text(1, 1, "COBRA SOAK", font, ink(phase, 0));
    c.text(1, font.line_height() + 2, "0123456789 !?", font, ink(phase, 1));
    let big = font.scale(2);
    c.text(1, font.line_height() * 2 + 4, "BIG", &big, ink(phase, 2));
}

fn cell_text(c: &mut Canvas, phase: f32) {
    let (w, h) = (c.width() as f32, c.height() as f32);
    c.fill_round_rect(1.0, 1.0, w - 2.0, h - 2.0, 4.0, Paint::dithered(Rgb::hex(0x203040), 0.6));
    c.print(1, 1, "text layer over dots", TextStyle::new(Rgb::hex(0xffffff)).bold());
    c.print(1, 2, "kitty sends this frame at z=-1", TextStyle::new(Rgb::hex(0x9ad1ff)));
    c.print(1, 3, &format!("phase {phase:.2}"), TextStyle::new(Color::Foreground).italic());
}

fn bubble(c: &mut Canvas, phase: f32) {
    let (w, h) = (c.width() as f32, c.height() as f32);
    c.fill_ellipse(w * 0.15, h * 0.75, h * 0.15, h * 0.15, Rgb::hex(0x44cc66));
    let speaker = (w * 0.15, h * 0.62);
    let text = if phase < 0.5 { "a soak frame\nwith a tail" } else { "still here\nafter a repeat" };
    let bubble = Bubble::speech(text).fill(Rgb::hex(0x101820)).border(1.0, Rgb::hex(0x88ddff));
    let _ = bubble.speak(c, speaker, &[]);
}
