//! Layers: one background, six layers on top of it, one effect each.
//!
//! ```text
//! cargo run --example layers                    # detect and draw
//! cargo run --example layers -- text            # the braille fallback, plain
//! cargo run --example layers -- svg out.svg     # transparent SVG (README image)
//! cargo run --example layers -- png out.png     # transparent PNG, 2x scale
//! cargo run --example layers -- png out.png light   # on a light background
//! ```

use std::io::{self, Write};

use cobra::{Bubble, Canvas, Color, Effect, Font, Layers, Paint, Renderer, Rgb, Terminal, TextStyle, export};

/// 3 x 2 panels of 38 x 28 dots.
const COLS: u16 = 57;
const ROWS: u16 = 14;
const PANEL_W: f32 = 38.0;
const PANEL_H: f32 = 28.0;

#[derive(Clone, Copy)]
struct Theme {
    bg: Rgb,
    ink: Rgb,
    stripe: Rgb,
    label: Rgb,
    /// What a shadow is cast in. Black vanishes on a dark terminal, so there it
    /// is a grey a step lighter than the background.
    shadow: Rgb,
    /// A panel fill a little off the background, for the bubble.
    panel: Rgb,
}

const DARK: Theme = Theme {
    bg: Rgb::hex(0x0b0e14),
    ink: Rgb::hex(0xf0f6fc),
    stripe: Rgb::hex(0x3d4451),
    label: Rgb::hex(0x8a94a6),
    shadow: Rgb::hex(0x6b7380),
    panel: Rgb::hex(0x161b22),
};
const LIGHT: Theme = Theme {
    bg: Rgb::hex(0xffffff),
    ink: Rgb::hex(0x0b0e14),
    stripe: Rgb::hex(0xb8c0cc),
    label: Rgb::hex(0x6b7380),
    shadow: Rgb::hex(0x000000),
    panel: Rgb::hex(0xf0f3f6),
};

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (theme, background) = match args.iter().any(|a| a == "light") {
        true => (LIGHT, Some(LIGHT.bg)),
        false => (DARK, args.iter().any(|a| a == "dark").then_some(DARK.bg)),
    };
    let style = export::Style { background, ..export::Style::default() };
    let mut layers = Layers::new(COLS, ROWS);
    draw(&mut layers, theme);
    let flat = layers.flatten();

    let mut out = io::stdout().lock();
    match args.first().map(String::as_str) {
        Some("text") => out.write_all(flat.to_text().as_bytes()),
        Some("svg") => write_or_print(args.get(1), export::svg(flat, &style).into_bytes()),
        Some("png") => write_or_print(args.get(1), export::png(flat, &style.scale(2))),
        _ => {
            let term = Terminal::detect();
            eprintln!("{:?}, {:?} text colours", term.protocol, term.depth);
            Renderer::new(term).render(flat, &mut out)
        }
    }
}

/// The background layer has stripes in every panel; each panel's shape sits on a
/// layer of its own, so every effect is seen against the same busy backdrop.
fn draw(layers: &mut Layers, theme: Theme) {
    let dark = Paint::dithered(theme.shadow, 0.6);
    let panels: [(&str, u32, Effect); 6] = [
        ("shadow", 0xff8c1a, Effect::shadow(2, 2, dark)),
        ("outline", 0x5ec33a, Effect::outline(1.0, theme.label)),
        ("gap", 0x3aa0ff, Effect::gap(1.5)),
        ("glow", 0xe45cc4, Effect::glow(4.0, Rgb::hex(0xe45cc4))),
        ("rim", 0xffd21e, Effect::rim(1.5, Paint::dithered(Rgb::hex(0x000000), 0.5))),
        // A shadow that darkens what it falls on instead of painting a colour.
        (
            "shader",
            0x2ec4a6,
            Effect::shader(4.0, 0.0, |s| match s.color {
                Some(Color::Rgb(c)) if s.covered(s.x - 3, s.y - 2) => Some(Paint::new(c.dim(0.35))),
                _ => None,
            }),
        ),
    ];

    for (i, (name, hex, effect)) in panels.into_iter().enumerate() {
        let (px, py) = ((i as f32 % 3.0) * PANEL_W, (i as f32 / 3.0).floor() * PANEL_H);
        let color = Rgb::hex(hex);
        stripes(&mut layers[0], px, py, theme.stripe);
        layers[0].text(px as i32 + 2, py as i32 + 2, name, Font::tiny(), theme.label);

        let layer = layers.push();
        layer.effect(effect);
        let (cx, cy) = (px + 13.0, py + 17.0);
        layer.disc(cx, cy, 7.0, color);
        layer.fill_star(px + 29.0, py + 15.0, 7.0, 3.0, 5, -1.57, color.dim(0.8));
        // A label in real text inside the disc: part of the same silhouette.
        layer.print((px / 2.0) as i32 + 6, (py / 4.0) as i32 + 4, "ab", TextStyle::new(theme.ink).on(color));
    }

    // A bubble on its own layer, across two panels, carrying two effects at once.
    let top = layers.push();
    top.effect(Effect::gap(1.0)).effect(Effect::shadow(2, 1, dark));
    Bubble::speech("layers").fill(theme.panel).border(1.0, theme.label).ink(theme.ink).draw(
        top,
        1.5 * PANEL_W,
        PANEL_H - 8.0,
    );
}

/// Diagonal stripes across a panel: the kind of background a plot has.
fn stripes(c: &mut Canvas, x: f32, y: f32, color: Rgb) {
    for i in 0..9 {
        let dy = 4.0 + i as f32 * 3.0;
        c.polyline(&[(x, y + dy), (x + PANEL_W - 2.0, y + dy - 3.0)], 1.0, Paint::dithered(color, 0.8));
    }
}

fn write_or_print(path: Option<&String>, bytes: Vec<u8>) -> io::Result<()> {
    match path {
        Some(p) => std::fs::write(p, bytes),
        None => io::stdout().lock().write_all(&bytes),
    }
}
