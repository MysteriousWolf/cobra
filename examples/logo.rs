//! The cobra banner: the mascot, the wordmark and a few of the things the crate
//! can do, drawn with the best protocol the terminal offers, or exported.
//!
//! ```text
//! cargo run --example logo                  # detect and draw
//! cargo run --example logo -- text          # plain braille, copy-paste friendly
//! cargo run --example logo -- svg  [file]   # transparent SVG (the README banner)
//! cargo run --example logo -- png  [file]   # transparent PNG, 3× scale
//! cargo run --example logo -- theme         # draw with the terminal's own colours
//! ```

#[path = "common/mod.rs"]
mod common;

use std::io::{self, Write};

use cobra::{Canvas, Color, Effect, Font, Layers, Options, Paint, Pen, Renderer, Rgb, Terminal, export};

const COLS: u16 = 68;
const ROWS: u16 = 9;

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.first().map(String::as_str);
    let mut layers = Layers::new(COLS, ROWS);
    banner(&mut layers, mode == Some("theme"));
    let flat = layers.flatten();
    let mut out = io::stdout().lock();
    match mode {
        Some("text") => out.write_all(flat.to_text().as_bytes()),
        Some("svg") => write_or_print(args.get(1), export::svg(flat, &style()).into_bytes()),
        Some("png") => write_or_print(args.get(1), export::png(flat, &style().scale(3))),
        _ => {
            let term = Terminal::detect();
            eprintln!(
                "{:?}, cell {}x{} px, palette {}",
                term.protocol,
                term.cell.width,
                term.cell.height,
                if term.palette_queried { "from terminal" } else { "xterm default" }
            );
            Renderer::with_options(term, Options { copy_text: true, ..Options::from_env() }).render(flat, &mut out)
        }
    }
}

/// Export geometry: slightly bigger dots than a terminal draws, so the banner still
/// reads when a README scales it down.
fn style() -> export::Style {
    export::Style { dot_size: 0.85, ..export::Style::default() }
}

/// The banner: the mascot with a soft glow on the left, the wordmark on the right
/// shaded from its edges with a shadow, a dashed baseline and a tagline under it.
fn banner(layers: &mut Layers, themed: bool) {
    // Mid greens read on dark and light backgrounds alike, which a banner in a
    // README needs; the shadow is the outline's near-black.
    let (green, bright, dark, ink): (Color, Color, Color, Color) = if themed {
        (Color::Indexed(2), Color::Indexed(10), Color::Indexed(0), Color::Indexed(2))
    } else {
        (Rgb::hex(0x3fb544).into(), Rgb::hex(0x8ae23c).into(), Rgb::hex(0x06210f).into(), Rgb::hex(0x3fb544).into())
    };

    // The mascot on its own layer, so its glow is computed from its silhouette.
    let mut mascot = Canvas::new(common::COLS, common::ROWS);
    if themed {
        common::draw_themed(&mut mascot, 0.6);
    } else {
        common::draw(&mut mascot, 0.6);
    }
    let snake = layers.push();
    blit(&mascot, snake, 0, 4);
    snake.effect(Effect::glow(2.5).paint(Paint::dithered(green, 0.45)));

    // The wordmark: a 3×5 font scaled 3×3, shaded from a bright edge into the body
    // green, with the outline and shadow every logo wants.
    let word = layers.push();
    let font = Font::tiny().scale_xy(3, 3);
    let x = 76;
    let (w, h) = word.text(x, 7, "cobra", &font, Paint::edge(bright, green, 1.5));
    word.effect(Effect::shadow(1, 1).paint(dark));

    // A dashed baseline and a tagline, dots too, so the banner is one picture.
    let base = layers.push();
    let y = (7 + h + 3) as f32;
    base.polyline(&[(x as f32, y), ((x + w) as f32, y)], Pen::new(1.0).dash(3.0, 2.0), green);
    base.text(x, 7 + h + 6, "dots, in colour", Font::tiny(), ink);
}

/// Copies every dot of `from` onto `to`, offset by `(dx, dy)` dots.
fn blit(from: &Canvas, to: &mut Canvas, dx: i32, dy: i32) {
    for y in 0..from.height() {
        for x in 0..from.width() {
            if let Some(c) = from.get(x, y) {
                to.set(x + dx, y + dy, c);
            }
        }
    }
}

fn write_or_print(path: Option<&String>, bytes: Vec<u8>) -> io::Result<()> {
    match path {
        Some(p) => std::fs::write(p, bytes),
        None => io::stdout().lock().write_all(&bytes),
    }
}
