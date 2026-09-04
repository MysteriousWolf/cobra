//! The cobra mascot, drawn procedurally so it can wiggle.
//!
//! Depth on a dot matrix comes from three things: a dark outline that separates the
//! head from the hood on any background, ordered dithering on the hood flaps so
//! they read as lighter, further-back surfaces, and a solid, brighter head in front.

#![allow(dead_code)]

use cobra::{Canvas, Color, Rgb};

pub const COLS: u16 = 32;
pub const ROWS: u16 = 8;

/// Colours of the mascot; every entry is a [`Color`] so the same drawing works with
/// explicit RGB or with the terminal's palette.
#[derive(Clone, Copy)]
pub struct Theme {
    pub outline: Color,
    pub hood: Color,
    pub hood_edge: Color,
    pub dark: Color,
    pub body: Color,
    pub bright: Color,
    pub belly: Color,
    pub eye: Color,
    pub pupil: Color,
    pub tongue: Color,
}

impl Theme {
    /// Saturated greens with a near-black outline: reads on light and dark backgrounds.
    pub const RGB: Theme = Theme {
        outline: Color::Rgb(Rgb::hex(0x06210f)),
        hood: Color::Rgb(Rgb::hex(0x1b7a35)),
        hood_edge: Color::Rgb(Rgb::hex(0x0e4a20)),
        dark: Color::Rgb(Rgb::hex(0x14612a)),
        body: Color::Rgb(Rgb::hex(0x3fb544)),
        bright: Color::Rgb(Rgb::hex(0x8ae23c)),
        belly: Color::Rgb(Rgb::hex(0xe9f5a0)),
        eye: Color::Rgb(Rgb::hex(0xfff6d5)),
        pupil: Color::Rgb(Rgb::hex(0x000000)),
        tongue: Color::Rgb(Rgb::hex(0xff3355)),
    };

    /// The terminal's own colours: ANSI greens, default foreground for the highlights.
    pub const ANSI: Theme = Theme {
        outline: Color::Indexed(0),
        hood: Color::Indexed(2),
        hood_edge: Color::Indexed(8),
        dark: Color::Indexed(2),
        body: Color::Indexed(10),
        bright: Color::Indexed(11),
        belly: Color::Foreground,
        eye: Color::Foreground,
        pupil: Color::Indexed(0),
        tongue: Color::Indexed(9),
    };
}

/// Draws the snake into `canvas` (sized `COLS × ROWS`). `phase` animates it.
pub fn draw(canvas: &mut Canvas, phase: f32) {
    draw_with(canvas, phase, &Theme::RGB);
}

/// [`draw`] in the terminal's palette colours.
pub fn draw_themed(canvas: &mut Canvas, phase: f32) {
    draw_with(canvas, phase, &Theme::ANSI);
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    match (a, b) {
        (Color::Rgb(a), Color::Rgb(b)) => Color::Rgb(a.lerp(b, t)),
        _ if t < 0.5 => a,
        _ => b,
    }
}

pub fn draw_with(canvas: &mut Canvas, phase: f32, th: &Theme) {
    canvas.clear();
    let steps = 240;
    let sway = 1.4 * (phase * 0.7).sin();
    let (nx, y_bottom, y_top) = (45.0, 26.0, 12.5);
    let hx = nx + sway; // head x (top of the neck)
    let hy = 7.0;

    // Hood flaps, drawn first so everything else sits in front. Two lobes either
    // side of the neck, ordered-dithered: sparse at the rim, denser at the neck, so
    // they read as a surface further back than the solid head and neck.
    for side in [-1.0f32, 1.0] {
        let (cx, cy) = (hx + side * 6.2, 14.0);
        canvas.disc_dithered(cx, cy, 7.6, th.hood_edge, 0.35);
        canvas.disc_dithered(cx, cy, 6.4, th.hood_edge, 0.55);
        canvas.disc_dithered(cx, cy, 5.2, th.hood, 0.75);
        canvas.disc(cx, cy, 3.4, th.hood);
    }
    // Rim: a sparse dark edge around both flaps.
    for side in [-1.0f32, 1.0] {
        let (cx, cy) = (hx + side * 6.2, 14.0);
        for k in 0..64 {
            let a = k as f32 / 64.0 * std::f32::consts::TAU;
            canvas.set_dithered((cx + 8.2 * a.cos()) as i32, (cy + 8.2 * a.sin()) as i32, th.outline, 0.3);
        }
    }

    // Tail: a wave along the ground, thin at the tip, thickening towards the neck.
    let tail = |t: f32| {
        let x = 2.0 + (nx - 3.0) * t;
        let y = 26.5 + 2.2 * ((t * 7.0) + phase).sin() * (1.0 - t * 0.6);
        (x, y, 0.7 + 1.9 * t)
    };
    for i in 0..=steps {
        let (x, y, r) = tail(i as f32 / steps as f32);
        canvas.disc(x, y, r + 0.9, th.outline);
    }
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let (x, y, r) = tail(t);
        canvas.disc(x, y, r, mix(th.dark, th.body, t));
        if t > 0.2 {
            canvas.disc(x, y - r * 0.6, r * 0.28, mix(th.body, th.bright, 0.7));
        }
        if i % 20 == 10 && t > 0.3 {
            canvas.disc(x, y + r * 0.2, r * 0.28, th.bright);
        }
    }

    // Neck: solid column in front of the hood, cleared ring then outline then body.
    for pass in 0..3 {
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let y = y_bottom + (y_top - y_bottom) * t;
            let x = nx + sway * t * t;
            let w = 2.4 - 0.3 * t;
            match pass {
                0 => canvas.clear_disc(x, y, w + 1.8),
                1 => canvas.disc(x, y, w + 1.0, th.outline),
                _ => {
                    canvas.disc(x, y, w, mix(th.dark, th.body, 0.4 + 0.6 * t));
                    canvas.disc(x + 0.7, y, w * 0.4, th.belly);
                }
            }
        }
    }

    // Head: a cleared ring, a dark outline, then the brightest greens of the drawing.
    let snout = hx + 4.4;
    canvas.clear_disc(hx + 1.0, hy, 6.4);
    canvas.clear_disc(snout, hy + 0.8, 4.6);
    canvas.disc(hx + 1.0, hy, 5.3, th.outline);
    canvas.disc(snout, hy + 0.8, 3.5, th.outline);
    canvas.disc(hx + 1.0, hy, 4.3, th.body);
    canvas.disc(snout, hy + 0.8, 2.5, mix(th.body, th.bright, 0.5));
    canvas.disc(hx + 0.2, hy - 1.6, 2.2, mix(th.body, th.bright, 0.4));
    canvas.disc_dithered(hx + 1.6, hy + 2.8, 2.2, th.belly, 0.5);

    // Eye: dark ring, light sclera three dots wide, one-dot pupil.
    let (ex, ey) = (hx + 2.5, hy - 1.0);
    canvas.disc(ex, ey, 2.5, th.outline);
    canvas.disc(ex, ey, 1.6, th.eye);
    canvas.set((ex + 0.5) as i32, ey as i32, th.pupil);

    // Forked tongue.
    let flick = ((phase * 3.0).sin() > 0.3) as i32;
    let (tx, ty) = ((snout + 3.0) as i32, (hy + 1.5) as i32);
    canvas.line(tx, ty, tx + 3 + flick, ty, th.tongue);
    canvas.line(tx + 3 + flick, ty, tx + 5 + flick, ty - 1, th.tongue);
    canvas.line(tx + 3 + flick, ty, tx + 5 + flick, ty + 1, th.tongue);
}
