//! Prints the cobra logo using the best protocol the terminal offers, or exports it.
//!
//! ```text
//! cargo run --example logo                  # detect and draw
//! cargo run --example logo -- text          # plain braille, copy-paste friendly
//! cargo run --example logo -- svg  [file]   # transparent SVG (README logo)
//! cargo run --example logo -- png  [file]   # transparent PNG, 3× scale
//! cargo run --example logo -- theme         # draw with the terminal's own colours
//! ```

#[path = "common/mod.rs"]
mod common;

use std::io::{self, Write};

use cobra::{export, Canvas, Options, Renderer, Terminal};

fn main() -> io::Result<()> {
    let mut canvas = Canvas::new(common::COLS, common::ROWS);
    common::draw(&mut canvas, 0.6);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out = io::stdout().lock();
    match args.first().map(String::as_str) {
        Some("text") => out.write_all(canvas.to_text().as_bytes()),
        Some("svg") => write_or_print(args.get(1), export::svg(&canvas, &export::Style::default()).into_bytes()),
        Some("png") => write_or_print(args.get(1), export::png(&canvas, &export::Style::default().scale(3))),
        mode => {
            if mode == Some("theme") {
                common::draw_themed(&mut canvas, 0.6);
            }
            let term = Terminal::detect();
            eprintln!(
                "{:?}, cell {}x{} px, palette {}",
                term.protocol,
                term.cell.width,
                term.cell.height,
                if term.palette_queried { "from terminal" } else { "xterm default" }
            );
            Renderer::with_options(term, Options { copy_text: true, ..Options::from_env() }).render(&canvas, &mut out)
        }
    }
}

fn write_or_print(path: Option<&String>, bytes: Vec<u8>) -> io::Result<()> {
    match path {
        Some(p) => std::fs::write(p, bytes),
        None => io::stdout().lock().write_all(&bytes),
    }
}
