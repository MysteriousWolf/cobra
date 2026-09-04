//! The cobra mascot, drawn procedurally so it can wiggle.

#![allow(dead_code)]

use cobra::{Canvas, Rgb};

pub const COLS: u16 = 28;
pub const ROWS: u16 = 7;

const DARK: Rgb = Rgb::hex(0x1f6b2f);
const LEAF: Rgb = Rgb::hex(0x5ec33a);
const LIME: Rgb = Rgb::hex(0xc8f542);
const BELLY: Rgb = Rgb::hex(0xf0e68c);
const RED: Rgb = Rgb::hex(0xff3355);
const EYE: Rgb = Rgb::hex(0xfff2a8);

/// Draws the snake into `canvas` (sized `COLS × ROWS`). `phase` animates it.
pub fn draw(canvas: &mut Canvas, phase: f32) {
    canvas.clear();
    let steps = 200;
    // Tail: a wave along the ground, thin at the tip, thickening towards the neck.
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = 2.0 + 37.0 * t;
        let y = 23.5 + 2.8 * ((t * 8.0) + phase).sin() * (1.0 - t * 0.7);
        let r = 0.7 + 1.7 * t;
        canvas.disc(x, y, r, DARK.lerp(LEAF, t));
        if i % 16 == 8 {
            canvas.disc(x, y - r * 0.45, r * 0.35, LIME);
        }
    }
    // Neck and hood: rises from the tail, flares just below the head, narrows at the top.
    let sway = 1.2 * (phase * 0.7).sin();
    let (nx, y_bottom, y_top) = (42.0, 22.0, 6.5);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let y = y_bottom + (y_top - y_bottom) * t;
        let x = nx + sway * t * t;
        let neck = 2.2;
        let flare = ((t - 0.72) / 0.28).clamp(-1.0, 1.0);
        let hood = neck + 4.8 * (1.0 - flare * flare) * ((t - 0.35) / 0.3).clamp(0.0, 1.0);
        canvas.disc(x, y, hood, DARK.lerp(LEAF, t));
        if t > 0.5 {
            canvas.disc(x, y, hood * 0.4, BELLY.lerp(LEAF, 0.55));
        }
    }
    // Head, eye, tongue.
    let hx = nx + sway;
    canvas.disc(hx + 2.0, 3.8, 2.9, LEAF);
    canvas.disc(hx + 3.2, 3.6, 2.2, LEAF.lerp(LIME, 0.35));
    canvas.disc(hx + 4.0, 2.9, 1.1, EYE);
    canvas.set((hx + 4.3) as i32, 2, Rgb::hex(0x101010));
    let flick = ((phase * 3.0).sin() > 0.3) as i32;
    let (tx, ty) = ((hx + 5.5) as i32, 5);
    canvas.line(tx, ty, tx + 3 + flick, ty, RED);
    canvas.line(tx + 3 + flick, ty, tx + 5 + flick, ty - 1, RED);
    canvas.line(tx + 3 + flick, ty, tx + 5 + flick, ty + 1, RED);
}
