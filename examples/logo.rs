//! Prints the cobra logo using the best protocol the terminal offers.
//!
//! ```text
//! cargo run --example logo          # detect and draw
//! cargo run --example logo -- text  # plain braille, copy-paste friendly
//! cargo run --example logo -- svg   # SVG on stdout (used for the README logo)
//! ```

#[path = "common/mod.rs"]
mod common;

use std::io::{self, Write};

use cobra::{Canvas, Options, Renderer, Terminal};

fn main() -> io::Result<()> {
    let mut canvas = Canvas::new(common::COLS, common::ROWS);
    common::draw(&mut canvas, 0.6);
    let mut out = io::stdout().lock();
    match std::env::args().nth(1).as_deref() {
        Some("text") => out.write_all(canvas.to_text().as_bytes()),
        Some("svg") => svg(&canvas, &mut out),
        _ => {
            let term = Terminal::detect();
            eprintln!("{term:?}");
            Renderer::with_options(term, Options { copy_text: true, ..Options::default() }).render(&canvas, &mut out)
        }
    }
}

/// Emits the canvas as an SVG of cell-aligned discs, the same geometry the image
/// protocols use.
fn svg(canvas: &Canvas, out: &mut impl Write) -> io::Result<()> {
    let (cw, ch, r) = (10.0, 20.0, 1.9);
    let (w, h) = (canvas.width() as f32 / 2.0 * cw, canvas.height() as f32 / 4.0 * ch);
    writeln!(out, r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}">"#)?;
    writeln!(out, r##"<rect width="100%" height="100%" rx="12" fill="#101418"/>"##)?;
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            if let Some(c) = canvas.get(x, y) {
                let (cx, cy) = ((x as f32 + 0.5) * cw / 2.0, (y as f32 + 0.5) * ch / 4.0);
                writeln!(out, r##"<circle cx="{cx}" cy="{cy}" r="{r}" fill="#{:02x}{:02x}{:02x}"/>"##, c.r, c.g, c.b)?;
            }
        }
    }
    writeln!(out, "</svg>")
}
