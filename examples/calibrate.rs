//! Shows the font's own braille glyphs next to image-rendered dots at several sizes
//! so you can pick the `COBRA_DOT` value that makes both look the same.
//!
//! Terminals publish the pixel size of a cell but not the font, so the *position* of
//! every dot is exact while the dot *diameter* is a style choice. The default
//! (`0.7` of a slot) is close to most monospace fonts; adjust it here if yours is
//! bolder or finer:
//!
//! ```text
//! cargo run --example calibrate
//! export COBRA_DOT=0.8
//! ```

use std::io::{self, Write};

use cobra::{Canvas, Color, Options, Renderer, Terminal};

const SIZES: [f32; 6] = [0.5, 0.6, 0.7, 0.8, 0.9, 1.0];

fn main() -> io::Result<()> {
    let term = Terminal::detect();
    let mut out = io::stdout().lock();
    if !term.is_graphical() {
        eprintln!("{:?}: no image protocol detected, nothing to calibrate (dots are the font's).", term.protocol);
        return Ok(());
    }

    // A pattern with full cells, half cells and single dots, in the terminal's
    // foreground colour so it matches the glyphs exactly.
    let mut canvas = Canvas::new(8, 1);
    for x in 0..16 {
        for y in 0..4 {
            let cell = x / 2;
            let on = match cell {
                0 | 1 => true,
                2 | 3 => y < 2,
                4 | 5 => (x + y) % 2 == 0,
                _ => x % 2 == 0 && y % 2 == 1,
            };
            if on {
                canvas.set(x, y, Color::Foreground);
            }
        }
    }

    writeln!(
        out,
        "{:?}, cell {}×{} px. Font glyphs first, then image dots at each size.",
        term.protocol, term.cell.width, term.cell.height
    )?;
    writeln!(out, "Pick the row that matches the glyphs and export COBRA_DOT=<size>.\r\n")?;
    write!(out, "  font  {}", canvas.to_text().trim_end())?;
    writeln!(out, "  ← drawn by the font\r")?;
    for size in SIZES {
        write!(out, "  {size:.2}  ")?;
        out.flush()?;
        let (col, row) = crossterm::cursor::position()?;
        let mut renderer = Renderer::with_options(term, Options { dot_size: size, ..Options::default() });
        renderer.render_at(&canvas, col, row, &mut out)?;
        let marker = if (size - Options::from_env().dot_size).abs() < 1e-3 { "  ← current" } else { "" };
        writeln!(out, "{}{marker}\r", " ".repeat(canvas.cols() as usize))?;
    }
    out.flush()
}
