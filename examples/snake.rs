//! Animates the cobra in place and reports frame cost.
//!
//! ```text
//! cargo run --release --example snake            # RGB colours
//! cargo run --release --example snake -- theme   # the terminal's own palette
//! ```

#[path = "common/mod.rs"]
mod common;

use std::io::{self, Write};
use std::time::{Duration, Instant};

use cobra::{Canvas, Placement, Renderer, Terminal};

fn main() -> io::Result<()> {
    let themed = std::env::args().nth(1).as_deref() == Some("theme");
    let term = Terminal::detect();
    let mut renderer = Renderer::new(term);
    let mut canvas = Canvas::new(common::COLS, common::ROWS);
    let mut out = io::stdout().lock();

    // Reserve the rows once, then redraw at that absolute position each frame.
    for _ in 0..common::ROWS {
        out.write_all(b"\r\n")?;
    }
    write!(out, "\x1b[{}A", common::ROWS)?;
    out.flush()?;
    let start = Instant::now();
    let frame_time = Duration::from_millis(33);
    let mut encoded = 0usize;
    let mut cpu = Duration::ZERO;
    let frames = 90;
    write!(out, "\x1b[?25l")?;
    for i in 0..frames {
        let t = Instant::now();
        if themed {
            common::draw_themed(&mut canvas, i as f32 * 0.15);
        } else {
            common::draw(&mut canvas, i as f32 * 0.15);
        }
        let bytes = renderer.encode(&canvas, Placement::Flow);
        encoded += bytes.len();
        out.write_all(bytes)?;
        write!(out, "\x1b[{}A", common::ROWS)?;
        out.flush()?;
        cpu += t.elapsed();
        let next = start + frame_time * (i + 1);
        if let Some(sleep) = next.checked_duration_since(Instant::now()) {
            std::thread::sleep(sleep);
        }
    }
    write!(out, "\x1b[{}B\x1b[?25h", common::ROWS)?;
    eprintln!(
        "{:?}: {frames} frames, {:.0} µs/frame encode+write, {} bytes/frame",
        term.protocol,
        cpu.as_micros() as f64 / frames as f64,
        encoded / frames as usize
    );
    Ok(())
}
