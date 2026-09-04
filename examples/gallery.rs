//! Every drawing primitive, one per panel, each in its own colour.
//!
//! ```text
//! cargo run --example gallery                    # detect and draw
//! cargo run --example gallery -- text            # plain braille
//! cargo run --example gallery -- svg out.svg     # transparent SVG (README image)
//! cargo run --example gallery -- png out.png     # transparent PNG, 2x scale
//! COBRA_COLORS=16 cargo run --example gallery    # the same on a 16-colour terminal
//! ```

use std::f32::consts::TAU;
use std::io::{self, Write};

use cobra::{Canvas, Font, Paint, Point, Renderer, Rgb, Terminal, export};

/// 4 x 3 panels of 36 x 32 dots each.
const COLS: u16 = 72;
const ROWS: u16 = 24;
const PANEL_W: i32 = 36;
const PANEL_H: i32 = 32;

fn main() -> io::Result<()> {
    let mut canvas = Canvas::new(COLS, ROWS);
    draw(&mut canvas);

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out = io::stdout().lock();
    match args.first().map(String::as_str) {
        Some("text") => out.write_all(canvas.to_text().as_bytes()),
        Some("svg") => write_or_print(args.get(1), export::svg(&canvas, &export::Style::default()).into_bytes()),
        Some("png") => write_or_print(args.get(1), export::png(&canvas, &export::Style::default().scale(2))),
        _ => {
            let term = Terminal::detect();
            eprintln!("{:?}, {:?} text colours", term.protocol, term.depth);
            Renderer::new(term).render(&canvas, &mut out)
        }
    }
}

fn draw(c: &mut Canvas) {
    let label_ink = Rgb::hex(0x8a94a6);

    /// Draws one panel's shapes, given its top-left dot and its colour.
    type Panel = fn(&mut Canvas, f32, f32, Rgb);

    // label, colour, body
    let panels: [(&str, u32, Panel); 12] = [
        ("set", 0xff3355, dots),
        ("line", 0xff8c1a, lines),
        ("disc", 0xffd21e, discs),
        ("rect", 0x5ec33a, rects),
        ("ellipse", 0x2ec4a6, ellipses),
        ("arc", 0x3aa0ff, arcs),
        ("polyline", 0x6c7bff, polyline),
        ("polygon", 0xa96cff, polygons),
        ("bezier", 0xe45cc4, bezier),
        ("spline", 0xff6f91, spline),
        ("text", 0xc9d1d9, text),
        ("dither", 0x4fd1c5, dither),
    ];

    for (i, (name, hex, body)) in panels.into_iter().enumerate() {
        let (px, py) = ((i as i32 % 4) * PANEL_W, (i as i32 / 4) * PANEL_H);
        let color = Rgb::hex(hex);
        c.text(px + 2, py + 2, name, Font::tiny(), label_ink);
        body(c, (px + 2) as f32, (py + 9) as f32, color);
    }
}

/// Individual dots: a scatter and a two-colour gradient row.
fn dots(c: &mut Canvas, x: f32, y: f32, color: Rgb) {
    let (x, y) = (x as i32, y as i32);
    for i in 0..16 {
        let t = i as f32 / 15.0;
        c.set(x + i * 2, y + 5 + ((t * TAU).sin() * 4.0) as i32, color);
    }
    let other = Rgb::hex(0x3aa0ff);
    for i in 0..32 {
        c.set(x + i, y + 14, color.lerp(other, i as f32 / 31.0));
    }
}

/// Bresenham lines through a shared origin.
fn lines(c: &mut Canvas, x: f32, y: f32, color: Rgb) {
    let (x, y) = (x as i32, y as i32);
    for i in 0..8 {
        let t = i as f32 / 7.0;
        c.line(x, y + 18, x + 4 + (t * 28.0) as i32, y + (t * 18.0) as i32, color.dim(0.5 + t * 0.5));
    }
}

/// Solid discs and the dithered variant.
fn discs(c: &mut Canvas, x: f32, y: f32, color: Rgb) {
    c.disc(x + 8.0, y + 9.0, 8.0, color);
    c.disc_dithered(x + 24.0, y + 9.0, 8.0, color, 0.45);
    c.clear_disc(x + 24.0, y + 9.0, 3.0);
}

/// Filled rectangle, a 2-dot border, and an erased notch.
fn rects(c: &mut Canvas, x: f32, y: f32, color: Rgb) {
    c.fill_rect(x, y + 2.0, 14.0, 14.0, color);
    c.rect(x + 18.0, y + 2.0, 14.0, 14.0, 2.0, color);
    c.fill_rect(x + 4.0, y + 6.0, 6.0, 6.0, Paint::erase());
}

/// Filled ellipse next to a stroked one.
fn ellipses(c: &mut Canvas, x: f32, y: f32, color: Rgb) {
    c.fill_ellipse(x + 8.0, y + 9.0, 8.0, 6.0, color);
    c.ellipse(x + 24.0, y + 9.0, 8.0, 6.0, 1.0, color);
    c.ellipse(x + 24.0, y + 9.0, 4.0, 3.0, 1.0, Paint::dithered(color, 0.5));
}

/// Concentric arcs, each a different sweep.
fn arcs(c: &mut Canvas, x: f32, y: f32, color: Rgb) {
    let (cx, cy) = (x + 16.0, y + 20.0);
    for i in 0..4 {
        let r = 4.0 + i as f32 * 3.0;
        let a0 = 3.34 + i as f32 * 0.22;
        c.arc(cx, cy, r, r, a0, a0 + 2.0, 1.0, color.dim(0.55 + i as f32 * 0.15));
    }
}

/// An open stroked path.
fn polyline(c: &mut Canvas, x: f32, y: f32, color: Rgb) {
    let pts: Vec<Point> = (0..9).map(|i| (x + i as f32 * 4.0, y + 9.0 + (i as f32 * 0.9).sin() * 8.0)).collect();
    c.polyline(&pts, 2.0, color);
}

/// A filled triangle and a stroked closed polygon.
fn polygons(c: &mut Canvas, x: f32, y: f32, color: Rgb) {
    c.fill_polygon(&[(x, y + 17.0), (x + 8.0, y), (x + 16.0, y + 17.0)], color);
    let hex: Vec<Point> = (0..6)
        .map(|i| {
            let a = i as f32 * 1.047;
            (x + 25.0 + a.cos() * 8.0, y + 9.0 + a.sin() * 8.0)
        })
        .collect();
    c.polygon(&hex, 1.0, color);
}

/// Quadratic (3 points) and cubic (4 points) Beziers.
fn bezier(c: &mut Canvas, x: f32, y: f32, color: Rgb) {
    c.bezier(&[(x, y + 18.0), (x + 16.0, y + 1.0), (x + 32.0, y + 18.0)], 1.0, color);
    c.bezier(&[(x, y + 8.0), (x + 10.0, y + 21.0), (x + 22.0, y + 2.0), (x + 32.0, y + 15.0)], 2.0, color);
}

/// Catmull-Rom spline through every point, with the points marked.
fn spline(c: &mut Canvas, x: f32, y: f32, color: Rgb) {
    let pts: Vec<Point> = (0..6).map(|i| (x + i as f32 * 6.0, y + 9.0 + (i as f32 * 1.6).cos() * 8.0)).collect();
    c.spline(&pts, false, 2.0, color);
    for &(px, py) in &pts {
        c.disc(px, py, 1.5, Rgb::hex(0xffd21e));
    }
}

/// The built-in 3x5 font, plain and scaled 2x.
fn text(c: &mut Canvas, x: f32, y: f32, color: Rgb) {
    let (x, y) = (x as i32, y as i32);
    let big = Font::tiny().scale(2);
    c.text(x, y, "Aa1", &big, color);
    c.text(x, y + big.line_height() + 2, "3x5 dots", Font::tiny(), Paint::dithered(color, 0.75));
}

/// Ordered dithering: a coverage ramp, then raw spans at falling coverage.
fn dither(c: &mut Canvas, x: f32, y: f32, color: Rgb) {
    for i in 0..8 {
        c.fill_rect(x + i as f32 * 4.0, y, 4.0, 8.0, Paint::dithered(color, i as f32 / 7.0));
    }
    // `span` is the primitive every fill goes through, usable on its own.
    for i in 0..12 {
        let cov = 1.0 - i as f32 / 12.0;
        c.span(y as i32 + 10 + i, x as i32, x as i32 + 6 + i * 2, Paint::dithered(color, cov));
    }
}

fn write_or_print(path: Option<&String>, bytes: Vec<u8>) -> io::Result<()> {
    match path {
        Some(p) => std::fs::write(p, bytes),
        None => io::stdout().lock().write_all(&bytes),
    }
}
