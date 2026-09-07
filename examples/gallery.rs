//! Every drawing primitive, one per panel, each in its own colour.
//!
//! ```text
//! cargo run --example gallery                    # detect and draw
//! cargo run --example gallery -- text            # plain braille
//! cargo run --example gallery -- svg out.svg     # transparent SVG (README image)
//! cargo run --example gallery -- png out.png     # transparent PNG, 2x scale
//! cargo run --example gallery -- png out.png light   # on a light background, light inks
//! cargo run --example gallery -- png out.png dark    # on a dark one
//! COBRA_COLORS=16 cargo run --example gallery    # the same on a 16-colour terminal
//! ```

use std::f32::consts::TAU;
use std::io::{self, Write};

use cobra::{
    Align, Bubble, Canvas, Font, Paint, Point, Rect, Renderer, Rgb, Shape, Side, Tail, TailKind, Terminal, TextStyle,
    export,
};

/// 4 x 6 panels of 36 x 36 dots each, with two rows spare for the tails at the bottom.
const COLS: u16 = 72;
const ROWS: u16 = 56;
const PANEL_W: i32 = 36;
const PANEL_H: i32 = 36;

/// The few colours that depend on what is behind the canvas: a dark terminal by
/// default, or a light one for `light`.
#[derive(Clone, Copy)]
struct Theme {
    /// The terminal background, for cut-outs and text on a bright fill.
    bg: Rgb,
    /// A panel fill a little off the background.
    panel: Rgb,
    /// Text on a coloured fill.
    on_fill: Rgb,
    /// Plain text and the text-panel colour.
    text: Rgb,
}

const DARK: Theme =
    Theme { bg: Rgb::hex(0x0b0e14), panel: Rgb::hex(0x161b22), on_fill: Rgb::hex(0xf0f6fc), text: Rgb::hex(0xc9d1d9) };
const LIGHT: Theme =
    Theme { bg: Rgb::hex(0xffffff), panel: Rgb::hex(0xf0f3f6), on_fill: Rgb::hex(0x0b0e14), text: Rgb::hex(0x2f3640) };

impl Theme {
    /// A quieter version of `color` for a fill behind text: pulled towards the
    /// background, so `on_fill` stays readable on it in either theme.
    fn shade(self, color: Rgb, f: f32) -> Rgb {
        color.lerp(self.bg, 1.0 - f)
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (theme, background) = match args.iter().any(|a| a == "light") {
        true => (LIGHT, Some(LIGHT.bg)),
        false => (DARK, args.iter().any(|a| a == "dark").then_some(DARK.bg)),
    };
    let style = export::Style { background, ..export::Style::default() };
    let mut canvas = Canvas::new(COLS, ROWS);
    draw(&mut canvas, theme);

    let mut out = io::stdout().lock();
    match args.first().map(String::as_str) {
        Some("text") => out.write_all(canvas.to_text().as_bytes()),
        Some("svg") => write_or_print(args.get(1), export::svg(&canvas, &style).into_bytes()),
        Some("png") => write_or_print(args.get(1), export::png(&canvas, &style.scale(2))),
        _ => {
            let term = Terminal::detect();
            eprintln!("{:?}, {:?} text colours", term.protocol, term.depth);
            Renderer::new(term).render(&canvas, &mut out)
        }
    }
}

fn draw(c: &mut Canvas, theme: Theme) {
    let label_ink = Rgb::hex(0x8a94a6);

    /// Draws one panel's shapes, given its top-left dot, its colour and the theme.
    type Panel = fn(&mut Canvas, f32, f32, Rgb, Theme);

    // label, colour, body
    let panels: [(&str, u32, Panel); 24] = [
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
        ("round rect", 0x5ec33a, round_rects),
        ("ngon + star", 0xffd21e, ngons),
        ("pie + ring", 0x2ec4a6, pies),
        ("arrow", 0xff8c1a, arrows),
        ("print", 0xc9d1d9, print),
        ("text box", 0x6c7bff, text_box),
        ("speech", 0xff6f91, speech),
        ("thought", 0xe45cc4, thought),
        ("tails", 0xffd21e, tails),
        ("speak", 0x3aa0ff, speak),
        ("clear", 0x5ec33a, clear_behind),
        ("dot text", 0x2ec4a6, font_bubble),
    ];

    for (i, (name, hex, body)) in panels.into_iter().enumerate() {
        let (px, py) = ((i as i32 % 4) * PANEL_W, (i as i32 / 4) * PANEL_H);
        let color = if hex == 0xc9d1d9 { theme.text } else { Rgb::hex(hex) };
        c.text(px + 2, py + 2, name, Font::tiny(), label_ink);
        body(c, (px + 2) as f32, (py + 9) as f32, color, theme);
    }
}

/// Individual dots: a scatter and a two-colour gradient row.
fn dots(c: &mut Canvas, x: f32, y: f32, color: Rgb, _: Theme) {
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
fn lines(c: &mut Canvas, x: f32, y: f32, color: Rgb, _: Theme) {
    let (x, y) = (x as i32, y as i32);
    for i in 0..8 {
        let t = i as f32 / 7.0;
        c.line(x, y + 18, x + 4 + (t * 28.0) as i32, y + (t * 18.0) as i32, color.dim(0.5 + t * 0.5));
    }
}

/// Solid discs and the dithered variant.
fn discs(c: &mut Canvas, x: f32, y: f32, color: Rgb, _: Theme) {
    c.disc(x + 8.0, y + 9.0, 8.0, color);
    c.disc_dithered(x + 24.0, y + 9.0, 8.0, color, 0.45);
    c.clear_disc(x + 24.0, y + 9.0, 3.0);
}

/// Filled rectangle, a 2-dot border, and an erased notch.
fn rects(c: &mut Canvas, x: f32, y: f32, color: Rgb, _: Theme) {
    c.fill_rect(x, y + 2.0, 14.0, 14.0, color);
    c.rect(x + 18.0, y + 2.0, 14.0, 14.0, 2.0, color);
    c.fill_rect(x + 4.0, y + 6.0, 6.0, 6.0, Paint::erase());
}

/// Filled ellipse next to a stroked one.
fn ellipses(c: &mut Canvas, x: f32, y: f32, color: Rgb, _: Theme) {
    c.fill_ellipse(x + 8.0, y + 9.0, 8.0, 6.0, color);
    c.ellipse(x + 24.0, y + 9.0, 8.0, 6.0, 1.0, color);
    c.ellipse(x + 24.0, y + 9.0, 4.0, 3.0, 1.0, Paint::dithered(color, 0.5));
}

/// Concentric arcs, each a different sweep.
fn arcs(c: &mut Canvas, x: f32, y: f32, color: Rgb, _: Theme) {
    let (cx, cy) = (x + 16.0, y + 20.0);
    for i in 0..4 {
        let r = 4.0 + i as f32 * 3.0;
        let a0 = 3.34 + i as f32 * 0.22;
        c.arc(cx, cy, r, r, a0, a0 + 2.0, 1.0, color.dim(0.55 + i as f32 * 0.15));
    }
}

/// An open stroked path.
fn polyline(c: &mut Canvas, x: f32, y: f32, color: Rgb, _: Theme) {
    let pts: Vec<Point> = (0..9).map(|i| (x + i as f32 * 4.0, y + 9.0 + (i as f32 * 0.9).sin() * 8.0)).collect();
    c.polyline(&pts, 2.0, color);
}

/// A filled triangle and a stroked closed polygon.
fn polygons(c: &mut Canvas, x: f32, y: f32, color: Rgb, _: Theme) {
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
fn bezier(c: &mut Canvas, x: f32, y: f32, color: Rgb, _: Theme) {
    c.bezier(&[(x, y + 18.0), (x + 16.0, y + 1.0), (x + 32.0, y + 18.0)], 1.0, color);
    c.bezier(&[(x, y + 8.0), (x + 10.0, y + 21.0), (x + 22.0, y + 2.0), (x + 32.0, y + 15.0)], 2.0, color);
}

/// Catmull-Rom spline through every point, with the points marked.
fn spline(c: &mut Canvas, x: f32, y: f32, color: Rgb, _: Theme) {
    let pts: Vec<Point> = (0..6).map(|i| (x + i as f32 * 6.0, y + 9.0 + (i as f32 * 1.6).cos() * 8.0)).collect();
    c.spline(&pts, false, 2.0, color);
    for &(px, py) in &pts {
        c.disc(px, py, 1.5, Rgb::hex(0xffd21e));
    }
}

/// The built-in 3x5 font, plain and scaled 2x.
fn text(c: &mut Canvas, x: f32, y: f32, color: Rgb, _: Theme) {
    let (x, y) = (x as i32, y as i32);
    let big = Font::tiny().scale(2);
    c.text(x, y, "Aa1", &big, color);
    c.text(x, y + big.line_height() + 2, "3x5 dots", Font::tiny(), Paint::dithered(color, 0.75));
}

/// Rounded boxes: filled, and a 2-dot border.
fn round_rects(c: &mut Canvas, x: f32, y: f32, color: Rgb, _: Theme) {
    c.fill_round_rect(x, y + 2.0, 15.0, 14.0, 5.0, color);
    c.round_rect(x + 18.0, y + 2.0, 15.0, 14.0, 5.0, 2.0, color);
}

/// A regular polygon and a star, filled and stroked.
fn ngons(c: &mut Canvas, x: f32, y: f32, color: Rgb, theme: Theme) {
    c.fill_ngon(x + 8.0, y + 9.0, 8.0, 6, 0.0, color);
    c.ngon(x + 8.0, y + 9.0, 4.0, 6, 0.5, 1.0, theme.bg);
    c.fill_star(x + 25.0, y + 9.0, 9.0, 4.0, 5, -1.57, color);
    c.star(x + 25.0, y + 9.0, 9.0, 4.0, 5, -1.57, 1.0, color.dim(0.6));
}

/// A pie chart of three slices, and a ring next to it.
fn pies(c: &mut Canvas, x: f32, y: f32, color: Rgb, _: Theme) {
    let mut a = 0.0;
    for (share, dim) in [(0.45, 1.0), (0.3, 0.7), (0.25, 0.45)] {
        let next = a + share * TAU;
        c.fill_pie(x + 9.0, y + 9.0, 8.0, 8.0, a, next, color.dim(dim));
        a = next;
    }
    c.ring(x + 26.0, y + 9.0, 8.0, 4.5, color);
    c.ring(x + 26.0, y + 9.0, 3.5, 2.0, Paint::dithered(color, 0.5));
}

/// Arrows: the tip lands exactly on the point given, the head scales with the shaft.
fn arrows(c: &mut Canvas, x: f32, y: f32, color: Rgb, _: Theme) {
    c.arrow((x, y + 2.0), (x + 32.0, y + 2.0), 1.0, 5.0, color.dim(0.55));
    c.arrow((x, y + 9.0), (x + 32.0, y + 9.0), 3.0, 8.0, color.dim(0.75));
    c.arrow((x + 2.0, y + 18.0), (x + 30.0, y + 15.0), 5.0, 11.0, color);
}

/// The text layer: real characters in the terminal's own font, one per cell.
fn print(c: &mut Canvas, x: f32, y: f32, color: Rgb, theme: Theme) {
    let (col, row) = ((x / 2.0) as i32, (y / 4.0) as i32);
    c.print(col, row, "Real text, bold", TextStyle::new(color).bold());
    c.print(col, row + 1, "any glyph: ± λ ✓", TextStyle::new(color.dim(0.75)));
    c.print(col, row + 2, " on a background ", TextStyle::new(theme.bg).on(Rgb::hex(0x4fd1c5)));
    c.print(col, row + 3, "and underlined", TextStyle::new(color).underline());
}

/// A bubble with no tail is a text box: wrapped, padded, bordered.
fn text_box(c: &mut Canvas, x: f32, y: f32, color: Rgb, theme: Theme) {
    Bubble::new("Text boxes wrap, pad and align themselves.")
        .wrap(15)
        .border(1.0, color)
        .fill(theme.panel)
        .ink(color)
        .align(Align::Center)
        .draw(c, x, y);
}

/// A speech bubble, and the whisper preset with its curling tail.
fn speech(c: &mut Canvas, x: f32, y: f32, color: Rgb, theme: Theme) {
    Bubble::speech("hi").fill(theme.shade(color, 0.45)).border(1.0, color).ink(theme.on_fill).draw(c, x, y);
    Bubble::whisper("psst").border(1.0, color.dim(0.6)).ink(color).draw(c, x + 16.0, y + 4.0);
}

/// The thought and shout presets: a cloud of lobes and a starburst.
fn thought(c: &mut Canvas, x: f32, y: f32, color: Rgb, theme: Theme) {
    Bubble::thought("hm").fill(theme.panel).border(1.0, color.dim(0.8)).ink(color).draw(c, x, y);
    Bubble::shout("HEY").fill(color).ink(theme.bg).draw(c, x + 17.0, y);
}

/// A tail can leave any side, at any point along it, in any of three kinds.
fn tails(c: &mut Canvas, x: f32, y: f32, color: Rgb, theme: Theme) {
    let corners = [
        (0.0, 2.0, Side::Left, TailKind::Point),
        (20.0, 2.0, Side::Top, TailKind::Curve),
        (0.0, 16.0, Side::Bottom, TailKind::Bubbles),
        (20.0, 16.0, Side::Right, TailKind::Point),
    ];
    for (dx, dy, side, kind) in corners {
        Bubble::new("hi")
            .shape(Shape::Round(3.0))
            .tail(Tail::new(side, 0.5, kind).len(6.0))
            .fill(theme.shade(color, 0.5))
            .ink(theme.on_fill)
            .draw(c, x + dx, y + dy);
    }
}

/// `clear_behind` unsets the dots under a bubble, so it reads on a busy background.
fn clear_behind(c: &mut Canvas, x: f32, y: f32, color: Rgb, _: Theme) {
    for i in 0..7 {
        c.polyline(&[(x, y + i as f32 * 4.0), (x + 34.0, y + 2.0 + i as f32 * 4.0)], 1.0, Paint::dithered(color, 0.8));
    }
    Bubble::new("on top of it").wrap(12).shape(Shape::Round(4.0)).border(1.0, color).ink(color).clear_behind().draw(
        c,
        x + 3.0,
        y + 9.0,
    );
}

/// A bubble can draw its text in a bitmap font instead of the text layer, for when a
/// character cell is too coarse, or for an export that has no terminal font.
fn font_bubble(c: &mut Canvas, x: f32, y: f32, color: Rgb, theme: Theme) {
    Bubble::new("dots")
        .font(Font::tiny())
        .shape(Shape::Round(3.0))
        .fill(theme.shade(color, 0.45))
        .ink(theme.on_fill)
        .draw(c, x, y + 2.0);
    Bubble::new("2x")
        .font(&Font::tiny().scale(2))
        .pad(0, 0)
        .shape(Shape::Round(3.0))
        .fill(theme.shade(color, 0.45))
        .ink(theme.on_fill)
        .draw(c, x + 20.0, y);
    Bubble::new("no cells")
        .font(Font::tiny())
        .wrap(14)
        .align(Align::Center)
        .border(1.0, Paint::dithered(color, 0.7))
        .ink(color)
        .draw(c, x + 5.0, y + 14.0);
}

/// `speak` puts the bubble where it fits: pointing at the mouth, on the canvas, and
/// clear of the zones it was told to avoid.
fn speak(c: &mut Canvas, x: f32, y: f32, color: Rgb, theme: Theme) {
    let area = Rect::new(x, y, 34.0, 26.0);
    let face = Rect::new(x + 19.0, y + 8.0, 14.0, 14.0);
    c.fill_ellipse(face.center().0, face.center().1, 7.0, 7.0, color.dim(0.5));
    c.fill_ellipse(face.center().0 + 2.0, face.center().1 - 2.0, 1.5, 1.5, theme.bg);
    let mouth = (face.center().0 - 2.0, face.center().1 + 4.0);
    // `speak` is this over the whole canvas; here the bubble is kept in its panel.
    let (placed, at) =
        Bubble::speech("over here").wrap(5).align(Align::Center).fill(theme.panel).border(1.0, color).ink(color).place(
            area,
            mouth,
            &[face],
        );
    placed.draw(c, at.0, at.1);
}

/// Ordered dithering: a coverage ramp, then raw spans at falling coverage.
fn dither(c: &mut Canvas, x: f32, y: f32, color: Rgb, _: Theme) {
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
