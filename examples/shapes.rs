//! Vector primitives and text: everything drawn through spans rather than dot loops.
//!
//! ```text
//! cargo run --example shapes                 # detect and draw
//! COBRA_COLORS=16 cargo run --example shapes # see the 16-colour fallback
//! cargo run --example shapes -- text         # plain braille
//! ```

use std::io;

use cobra::{Canvas, Font, Paint, Point, Renderer, Rgb, Terminal};

fn main() -> io::Result<()> {
    let mut c = Canvas::new(60, 12);
    let (w, h) = (c.width() as f32, c.height() as f32);
    let (ink, accent, grass, sky) = (Rgb::hex(0xe8e8e8), Rgb::hex(0xff3355), Rgb::hex(0x5ec33a), Rgb::hex(0x3aa0ff));

    // Ground: a dithered fill, then a solid stroked spline for the horizon.
    c.fill_rect(0.0, h * 0.7, w, h * 0.3, Paint::dithered(grass, 0.35));
    let ridge: Vec<Point> = (0..9).map(|i| (w * i as f32 / 8.0, h * (0.7 - 0.1 * (i as f32 * 1.3).sin()))).collect();
    c.spline(&ridge, false, 2.0, grass);

    // Sun: erase a halo so it stays readable over the ridge, then an outline and a disc.
    c.fill_ellipse(w * 0.82, h * 0.3, 12.0, 12.0, Paint::erase());
    c.disc(w * 0.82, h * 0.3, 9.0, accent);
    c.ellipse(w * 0.82, h * 0.3, 11.5, 11.5, 1.0, Paint::dithered(accent, 0.5));

    // A house: filled polygon roof, rectangle walls with a thick border, a door.
    let (hx, hy) = (w * 0.25, h * 0.68);
    c.fill_polygon(&[(hx - 16.0, hy - 14.0), (hx, hy - 26.0), (hx + 16.0, hy - 14.0)], accent);
    c.fill_rect(hx - 13.0, hy - 14.0, 26.0, 14.0, Paint::dithered(ink, 0.5));
    c.rect(hx - 13.0, hy - 14.0, 26.0, 14.0, 1.0, ink);
    c.fill_rect(hx - 3.0, hy - 8.0, 6.0, 8.0, Paint::erase());
    c.rect(hx - 3.0, hy - 8.0, 6.0, 8.0, 1.0, sky);

    // A path of smoke: one cubic Bézier, and a dashed arc from Bézier-free geometry.
    c.bezier(&[(hx + 9.0, hy - 24.0), (hx + 14.0, hy - 34.0), (hx - 4.0, hy - 40.0), (hx + 6.0, hy - 46.0)], 1.0, ink);
    for i in 0..6 {
        let a0 = 3.4 + i as f32 * 0.45;
        c.arc(w * 0.55, h * 0.75, 30.0, 14.0, a0, a0 + 0.25, 1.0, sky);
    }

    // Text: the built-in 3×5 font and a 2× scaled copy.
    let big = Font::tiny().scale(2);
    c.text(3, 3, "cobra", &big, ink);
    c.text(3, 3 + big.line_height() + 1, "shapes, splines & fonts", Font::tiny(), sky);
    let label = "tiny 3x5";
    let (lw, _) = Font::tiny().measure(label);
    c.text(c.width() - lw - 3, 3, label, Font::tiny(), Paint::dithered(ink, 0.75));

    if std::env::args().nth(1).as_deref() == Some("text") {
        print!("{}", c.to_text());
        return Ok(());
    }
    let term = Terminal::detect();
    eprintln!("{:?}, {:?} text colours", term.protocol, term.depth);
    Renderer::new(term).render(&c, &mut io::stdout().lock())
}
