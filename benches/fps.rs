//! Frames per second for every protocol, canvas size and colour mode.
//!
//! ```text
//! cargo bench                       # encode only, the cost the library adds
//! cargo bench --features ratatui    # also the ratatui widget path
//! ```
//!
//! Each row draws a fresh animated frame, encodes it for the protocol and writes it
//! to a sink (`/dev/null`-like), which is everything a program does per frame apart
//! from the terminal's own decoding. Numbers are the median of repeated runs, so a
//! busy machine only adds noise to the tail.

#[path = "../examples/common/mod.rs"]
mod common;

use std::io::Write;
use std::time::{Duration, Instant};

use cobra::{Canvas, CellSize, Color, Depth, Font, Options, Paint, Placement, Protocol, Renderer, Rgb, Terminal};

const CELL: CellSize = CellSize { width: 9, height: 18 };

/// Every protocol, plus the text protocol quantised to 16 colours.
fn terminals() -> [(&'static str, Terminal); 5] {
    [
        ("Text", Terminal::new(Protocol::Text, CELL)),
        ("Text/16", Terminal::new(Protocol::Text, CELL).with_depth(Depth::Ansi16)),
        ("Kitty", Terminal::new(Protocol::Kitty, CELL)),
        ("Iterm2", Terminal::new(Protocol::Iterm2, CELL)),
        ("Sixel", Terminal::new(Protocol::Sixel, CELL)),
    ]
}

struct Case {
    name: &'static str,
    cols: u16,
    rows: u16,
    draw: fn(&mut Canvas, f32, bool),
}

/// A dense plot: a few sine traces plus a filled area, the kind of content a
/// dashboard shows.
fn plot(c: &mut Canvas, phase: f32, themed: bool) {
    c.clear();
    let (w, h) = (c.width(), c.height());
    let colours: [Color; 3] = if themed {
        [Color::Indexed(2), Color::Indexed(4), Color::Indexed(1)]
    } else {
        [Rgb::hex(0x5ec33a).into(), Rgb::hex(0x3aa0ff).into(), Rgb::hex(0xff3355).into()]
    };
    for x in 0..w {
        let t = x as f32 / w as f32;
        let base = h as f32 * (0.7 + 0.2 * (t * 6.0 + phase).sin());
        for y in base as i32..h {
            c.set_dithered(x, y, colours[0], 0.5);
        }
        for (k, colour) in colours.iter().enumerate() {
            let y = h as f32 * (0.5 + 0.35 * (t * (4.0 + k as f32) + phase * (k as f32 + 1.0)).sin());
            c.set(x, y as i32, *colour);
        }
    }
}

/// Vector content: filled and stroked shapes, a spline, a curve and labels, all
/// through the span-based primitives rather than per-dot loops.
fn shapes(c: &mut Canvas, phase: f32, themed: bool) {
    c.clear();
    let (w, h) = (c.width() as f32, c.height() as f32);
    let colours: [Color; 3] = if themed {
        [Color::Indexed(2), Color::Indexed(4), Color::Indexed(1)]
    } else {
        [Rgb::hex(0x5ec33a).into(), Rgb::hex(0x3aa0ff).into(), Rgb::hex(0xff3355).into()]
    };
    c.fill_rect(0.0, h * 0.6, w, h * 0.4, Paint::dithered(colours[0], 0.4));
    c.rect(1.0, 1.0, w - 2.0, h - 2.0, 1.0, colours[1]);
    let pts: Vec<(f32, f32)> =
        (0..8).map(|i| (w * (0.1 + 0.8 * i as f32 / 7.0), h * (0.3 + 0.25 * (i as f32 + phase).sin()))).collect();
    c.spline(&pts, false, 2.0, colours[2]);
    c.bezier(&[(0.0, h), (w * 0.3, 0.0), (w * 0.7, h), (w, 0.0)], 1.0, colours[1]);
    let (cx, cy) = (w * 0.5 + w * 0.2 * phase.cos(), h * 0.5);
    c.fill_polygon(&[(cx, cy - 8.0), (cx + 7.0, cy + 5.0), (cx - 7.0, cy + 5.0)], colours[0]);
    c.fill_ellipse(w * 0.8, h * 0.5, 9.0, 5.0, Paint::erase());
    c.ellipse(w * 0.8, h * 0.5, 9.0, 5.0, 1.5, colours[2]);
    c.text(2, 2, "shapes", Font::tiny(), colours[1]);
}

fn snake(c: &mut Canvas, phase: f32, themed: bool) {
    common::draw_with(c, phase, if themed { &common::Theme::ANSI } else { &common::Theme::RGB });
}

fn measure(mut f: impl FnMut(), runs: usize, iters: usize) -> Duration {
    let mut samples: Vec<Duration> = (0..runs)
        .map(|_| {
            let t = Instant::now();
            for _ in 0..iters {
                f();
            }
            t.elapsed() / iters as u32
        })
        .collect();
    samples.sort();
    samples[samples.len() / 2]
}

fn main() {
    let cases = [
        Case { name: "logo 32×8", cols: common::COLS, rows: common::ROWS, draw: snake },
        Case { name: "plot 80×24", cols: 80, rows: 24, draw: plot },
        Case { name: "plot 200×50", cols: 200, rows: 50, draw: plot },
        Case { name: "shapes 80×24", cols: 80, rows: 24, draw: shapes },
    ];
    let quick = std::env::args().any(|a| a == "--quick");
    let (runs, iters) = if quick { (3, 5) } else { (9, 20) };

    println!("cell {}×{} px, median of {runs} runs × {iters} frames, one core\n", CELL.width, CELL.height);
    println!("| Canvas | Colours | Protocol | Draw | Encode + write | Frame | FPS | Bytes/frame |");
    println!("|---|---|---|---|---|---|---|---|");
    for case in &cases {
        for themed in [false, true] {
            let mut canvas = Canvas::new(case.cols, case.rows);
            let mut phase = 0.0f32;
            let draw = measure(
                || {
                    phase += 0.1;
                    (case.draw)(&mut canvas, phase, themed);
                },
                runs,
                iters,
            );
            for (protocol, term) in terminals() {
                let mut renderer = Renderer::with_options(term, Options::default());
                let mut sink = std::io::sink();
                let mut bytes = 0usize;
                (case.draw)(&mut canvas, 0.3, themed);
                renderer.encode(&canvas, Placement::Flow); // warm-up: allocations happen here
                let encode = measure(
                    || {
                        phase += 0.1;
                        (case.draw)(&mut canvas, phase, themed);
                        let frame = renderer.encode(&canvas, Placement::Flow);
                        bytes = frame.len();
                        sink.write_all(frame).unwrap();
                    },
                    runs,
                    iters,
                );
                let encode = encode.saturating_sub(draw);
                let frame = draw + encode;
                println!(
                    "| {} | {} | {} | {} | {} | {} | {:.0} | {} |",
                    case.name,
                    if themed { "palette" } else { "rgb" },
                    protocol,
                    us(draw),
                    us(encode),
                    us(frame),
                    1.0 / frame.as_secs_f64(),
                    kb(bytes)
                );
            }
        }
    }

    #[cfg(feature = "ratatui")]
    ratatui_path(runs, iters);
}

fn us(d: Duration) -> String {
    let u = d.as_secs_f64() * 1e6;
    if u < 1000.0 { format!("{u:.0} µs") } else { format!("{:.2} ms", u / 1000.0) }
}

fn kb(b: usize) -> String {
    if b < 1024 { format!("{b} B") } else { format!("{:.1} KB", b as f64 / 1024.0) }
}

/// The full ratatui frame path: widget into a `Buffer`, the buffer diff against the
/// previous frame written out as a `TestBackend` would, then the overlay.
#[cfg(feature = "ratatui")]
fn ratatui_path(runs: usize, iters: usize) {
    use cobra::ratatui::{Braille, overlay};
    use ratatui::backend::{Backend, TestBackend};
    use ratatui::layout::Rect;
    use ratatui::widgets::Widget;

    println!("\nratatui: widget render + buffer diff + overlay, 80×24 terminal\n");
    println!("| Canvas | Protocol | Frame | FPS |");
    println!("|---|---|---|---|");
    for (name, cols, rows, draw) in
        [("logo 32×8", common::COLS, common::ROWS, snake as fn(&mut Canvas, f32, bool)), ("plot 80×24", 80, 24, plot)]
    {
        for protocol in [Protocol::Text, Protocol::Kitty, Protocol::Iterm2, Protocol::Sixel] {
            let term = Terminal::new(protocol, CELL);
            let mut renderer = Renderer::with_options(term, Options::default());
            let mut canvas = Canvas::new(cols, rows);
            let mut backend = TestBackend::new(80, 24);
            let mut prev = ratatui::buffer::Buffer::empty(Rect::new(0, 0, 80, 24));
            let mut sink = std::io::sink();
            let mut phase = 0.0f32;
            let frame = measure(
                || {
                    phase += 0.1;
                    draw(&mut canvas, phase, false);
                    let mut buf = ratatui::buffer::Buffer::empty(prev.area);
                    let area = Rect::new(0, 0, cols.min(80), rows.min(24));
                    Braille::new(&canvas, &renderer).render(area, &mut buf);
                    let updates = prev.diff(&buf);
                    backend.draw(updates.into_iter()).unwrap();
                    prev = buf;
                    overlay(&mut renderer, &canvas, area, &mut sink).unwrap();
                },
                runs,
                iters,
            );
            println!("| {name} | {protocol:?} | {} | {:.0} |", us(frame), 1.0 / frame.as_secs_f64());
        }
    }
}
