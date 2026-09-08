//! One small drawing per documented function, so the reference shows what each
//! one does. The generator renders each into an SVG and quotes the block below it
//! as the example, so keep them self-contained and short.

use std::f32::consts::{PI, TAU};

use cobra::{
    Align, Attrs, Bubble, Canvas, Color, DOTS_X, DOTS_Y, Depth, Effect, Field, Font, Layers, MAX_GLYPH_WIDTH, Mask,
    Paint, Palette, Path, Pattern, Pen, Point, Probe, Rect, Rgb, Shape, Side, Tail, TailKind, TextStyle, Transform,
    bayer, export, text,
};

/// The colours that depend on what is behind the picture.
#[derive(Clone, Copy)]
pub struct Theme {
    /// The page background.
    pub bg: Rgb,
    /// Text and outlines.
    pub ink: Rgb,
    /// A fill a little off the background.
    pub panel: Rgb,
}

pub const DARK: Theme = Theme { bg: Rgb::hex(0x0d1117), ink: Rgb::hex(0xe6edf3), panel: Rgb::hex(0x21262d) };
pub const LIGHT: Theme = Theme { bg: Rgb::hex(0xffffff), ink: Rgb::hex(0x1f2328), panel: Rgb::hex(0xd0d7de) };

const RED: Rgb = Rgb::hex(0xff5c7a);
const ORANGE: Rgb = Rgb::hex(0xffa657);
const YELLOW: Rgb = Rgb::hex(0xf2cc60);
const GREEN: Rgb = Rgb::hex(0x56d364);
const CYAN: Rgb = Rgb::hex(0x39d2c0);
const BLUE: Rgb = Rgb::hex(0x58a6ff);
const PURPLE: Rgb = Rgb::hex(0xbc8cff);

/// A drawing for one or more items of the reference.
pub struct Demo {
    /// The items it illustrates, as the parser names them (`Canvas::fill_rect`);
    /// the first names the demo and its files.
    pub items: &'static [&'static str],
    pub cols: u16,
    pub rows: u16,
    pub draw: fn(&mut Canvas, Theme),
}

impl Demo {
    /// The item the demo is named after.
    pub fn name(&self) -> &'static str {
        self.items[0]
    }
}

macro_rules! demos {
    ($($($item:literal)|+, $cols:literal x $rows:literal => |$c:ident, $t:ident| $body:block)*) => {
        /// Every demo, in the order they are written here.
        pub fn all() -> Vec<Demo> {
            vec![$(Demo { items: &[$($item),+], cols: $cols, rows: $rows, draw: |$c: &mut Canvas, $t: Theme| { let _ = &$t; $body } }),*]
        }
    };
}

demos! {
    // ----- canvas ---------------------------------------------------------------
    "Canvas::set", 24 x 4 => |c, t| {
        for x in 0..c.width() {
            let y = 8.0 + 6.0 * (x as f32 / 6.0).sin();
            c.set(x, y as i32, RED.lerp(BLUE, x as f32 / c.width() as f32));
        }
    }
    "Canvas::set_dithered", 24 x 4 => |c, t| {
        for y in 0..c.height() {
            for x in 0..c.width() {
                c.set_dithered(x, y, GREEN, x as f32 / c.width() as f32);
            }
        }
    }
    "Canvas::unset", 24 x 4 => |c, t| {
        c.fill_rect(0.0, 0.0, 48.0, 16.0, BLUE);
        for x in (0..48).step_by(3) {
            c.unset(x, 8);
        }
    }
    "Canvas::line", 24 x 4 => |c, t| {
        c.line(0, 15, 47, 0, RED);
        c.line(0, 0, 47, 15, BLUE);
        c.line(0, 8, 47, 8, t.ink);
    }
    "Canvas::disc", 24 x 4 => |c, t| {
        c.disc(8.0, 8.0, 7.0, YELLOW);
        c.disc(24.0, 8.0, 5.0, CYAN);
        c.disc(40.0, 8.0, 3.0, RED);
    }
    "Canvas::disc_dithered", 24 x 4 => |c, t| {
        c.disc_dithered(8.0, 8.0, 7.0, YELLOW, 0.25);
        c.disc_dithered(24.0, 8.0, 7.0, YELLOW, 0.5);
        c.disc_dithered(40.0, 8.0, 7.0, YELLOW, 0.75);
    }
    "Canvas::clear_disc", 24 x 4 => |c, t| {
        c.fill_rect(0.0, 0.0, 48.0, 16.0, Paint::dithered(BLUE, 0.5));
        c.clear_disc(24.0, 8.0, 7.0);
        c.disc(24.0, 8.0, 5.0, RED);
    }
    "Canvas::to_text", 12 x 2 => |c, t| {
        c.disc(12.0, 4.0, 3.5, GREEN);
        c.print(0, 0, "hi", t.ink);
    }
    // ----- draw -----------------------------------------------------------------
    "Canvas::span", 24 x 4 => |c, t| {
        for y in 0..16 {
            c.span(y, 24 - y, 24 + y, Paint::new(PURPLE));
        }
    }
    "Canvas::fill_rect", 24 x 4 => |c, t| {
        c.fill_rect(2.0, 2.0, 20.0, 12.0, BLUE);
        c.fill_rect(26.0, 4.0, 20.0, 8.0, Paint::dithered(BLUE, 0.5));
    }
    "Canvas::rect", 24 x 4 => |c, t| {
        c.rect(2.0, 2.0, 20.0, 12.0, 1.0, GREEN);
        c.rect(26.0, 2.0, 20.0, 12.0, 3.0, GREEN);
    }
    "Canvas::fill_ellipse", 24 x 4 => |c, t| {
        c.fill_ellipse(12.0, 8.0, 10.0, 6.0, ORANGE);
        c.fill_ellipse(36.0, 8.0, 4.0, 7.0, CYAN);
    }
    "Canvas::ellipse", 24 x 4 => |c, t| {
        c.ellipse(12.0, 8.0, 10.0, 6.0, 1.0, ORANGE);
        c.ellipse(36.0, 8.0, 10.0, 6.0, 2.5, CYAN);
    }
    "Canvas::arc", 24 x 4 => |c, t| {
        c.arc(12.0, 9.0, 9.0, 7.0, PI, TAU, 1.0, RED);
        c.arc(36.0, 8.0, 7.0, 7.0, 0.0, 1.5 * PI, 2.0, BLUE);
    }
    "Canvas::polyline", 24 x 4 => |c, t| {
        c.polyline(&[(1.0, 14.0), (10.0, 2.0), (18.0, 12.0), (24.0, 4.0)], 1.0, GREEN);
        c.polyline(&[(28.0, 14.0), (36.0, 3.0), (46.0, 13.0)], 3.0, PURPLE);
    }
    "Canvas::polygon", 24 x 4 => |c, t| {
        c.polygon(&[(2.0, 14.0), (12.0, 1.0), (22.0, 14.0)], 1.0, YELLOW);
        c.polygon(&[(27.0, 4.0), (45.0, 2.0), (43.0, 13.0), (30.0, 12.0)], 2.0, CYAN);
    }
    "Canvas::fill_polygon", 24 x 4 => |c, t| {
        c.fill_polygon(&[(2.0, 14.0), (12.0, 1.0), (22.0, 14.0)], YELLOW);
        // Even-odd: a self-crossing outline leaves its crossing empty.
        c.fill_polygon(&[(26.0, 1.0), (46.0, 14.0), (46.0, 1.0), (26.0, 14.0)], CYAN);
    }
    "Canvas::bezier", 24 x 4 => |c, t| {
        c.bezier(&[(1.0, 14.0), (12.0, -6.0), (22.0, 14.0)], 1.0, RED);
        c.bezier(&[(26.0, 14.0), (30.0, -4.0), (42.0, 20.0), (46.0, 2.0)], 2.0, BLUE);
    }
    "Canvas::spline", 24 x 4 => |c, t| {
        c.spline(&[(1.0, 8.0), (8.0, 2.0), (15.0, 14.0), (22.0, 8.0)], false, 1.0, GREEN);
        c.spline(&[(30.0, 3.0), (44.0, 4.0), (42.0, 13.0), (28.0, 12.0)], true, 1.0, ORANGE);
    }
    "Canvas::fill_round_rect", 24 x 4 => |c, t| {
        c.fill_round_rect(1.0, 1.0, 22.0, 14.0, 4.0, PURPLE);
        c.fill_round_rect(26.0, 1.0, 20.0, 14.0, 7.0, CYAN);
    }
    "Canvas::round_rect", 24 x 4 => |c, t| {
        c.round_rect(1.0, 1.0, 22.0, 14.0, 4.0, 1.0, PURPLE);
        c.round_rect(26.0, 1.0, 20.0, 14.0, 7.0, 3.0, CYAN);
    }
    "Canvas::ring", 24 x 4 => |c, t| {
        c.ring(8.0, 8.0, 7.5, 5.0, YELLOW);
        c.ring(24.0, 8.0, 7.5, 2.0, GREEN);
        c.ring(40.0, 8.0, 7.5, 0.0, BLUE);
    }
    "Canvas::fill_pie", 24 x 4 => |c, t| {
        c.fill_pie(12.0, 8.0, 7.5, 7.5, 0.0, 0.3 * TAU, RED);
        c.fill_pie(12.0, 8.0, 7.5, 7.5, 0.3 * TAU, 0.75 * TAU, BLUE);
        c.fill_pie(12.0, 8.0, 7.5, 7.5, 0.75 * TAU, TAU, YELLOW);
        c.fill_pie(36.0, 8.0, 10.0, 6.0, -0.4 * PI, 0.4 * PI, GREEN);
    }
    "Canvas::fill_ngon", 24 x 4 => |c, t| {
        c.fill_ngon(8.0, 8.0, 7.5, 3, -PI / 2.0, RED);
        c.fill_ngon(24.0, 8.0, 7.5, 5, -PI / 2.0, ORANGE);
        c.fill_ngon(40.0, 8.0, 7.5, 6, 0.0, YELLOW);
    }
    "Canvas::ngon", 24 x 4 => |c, t| {
        c.ngon(8.0, 8.0, 7.5, 3, -PI / 2.0, 1.0, RED);
        c.ngon(24.0, 8.0, 7.5, 5, -PI / 2.0, 1.0, ORANGE);
        c.ngon(40.0, 8.0, 7.5, 8, 0.0, 2.0, YELLOW);
    }
    "Canvas::fill_star", 24 x 4 => |c, t| {
        c.fill_star(8.0, 8.0, 7.5, 3.0, 5, -PI / 2.0, YELLOW);
        c.fill_star(24.0, 8.0, 7.5, 5.0, 8, 0.0, CYAN);
        c.fill_star(40.0, 8.0, 7.5, 1.5, 4, 0.0, PURPLE);
    }
    "Canvas::star", 24 x 4 => |c, t| {
        c.star(8.0, 8.0, 7.5, 3.0, 5, -PI / 2.0, 1.0, YELLOW);
        c.star(24.0, 8.0, 7.5, 5.0, 8, 0.0, 1.0, CYAN);
        c.star(40.0, 8.0, 7.5, 1.5, 4, 0.0, 2.0, PURPLE);
    }
    "Canvas::arrow", 24 x 4 => |c, t| {
        c.arrow((2.0, 13.0), (22.0, 3.0), 1.0, 5.0, GREEN);
        c.arrow((26.0, 8.0), (46.0, 8.0), 3.0, 8.0, BLUE);
    }
    "Canvas::stencil", 24 x 4 => |c, t| {
        let mut mask = Canvas::new(24, 4);
        mask.text(1, 1, "MASK", &Font::tiny().scale_xy(3, 3), t.ink);
        c.stencil(&mask, Paint::linear((0.0, 0.0), (48.0, 0.0), RED, BLUE));
    }
    "Canvas::stencil_in", 24 x 4 => |c, t| {
        let mut mask = Canvas::new(24, 4);
        mask.fill_rect(0.0, 0.0, 48.0, 16.0, t.ink);
        // Only the box is painted, and it is the gradient's frame: the mask's
        // extent is never looked at.
        c.stencil_in(&mask, Rect::new(4.0, 2.0, 40.0, 12.0), Paint::edge(BLUE, t.panel, 4.0));
    }
    "Canvas::clip", 24 x 4 => |c, t| {
        c.fill_rect(0.0, 0.0, 48.0, 16.0, Paint::pattern(GREEN, Pattern::Diagonal(3)));
        let mut mask = Canvas::new(24, 4);
        mask.disc(24.0, 8.0, 7.5, t.ink);
        c.clip(&mask);
    }
    "Canvas::cut", 24 x 4 => |c, t| {
        c.fill_rect(0.0, 0.0, 48.0, 16.0, Paint::pattern(GREEN, Pattern::Diagonal(3)));
        let mut mask = Canvas::new(24, 4);
        mask.disc(24.0, 8.0, 7.5, t.ink);
        c.cut(&mask);
    }
    "Canvas::clipped", 24 x 4 => |c, t| {
        let mut body = Mask::new(24, 4);
        body.draw(|c| c.fill_ellipse(24.0, 8.0, 18.0, 7.0, t.ink));
        c.stencil(&body, ORANGE);
        // Creases that stay inside the body wherever the points go.
        c.clipped(&body, |c| {
            c.spline(&[(0.0, 2.0), (16.0, 12.0), (30.0, 3.0), (48.0, 14.0)], false, 1.5, Rgb::hex(0x7a3e00));
            c.polyline(&[(20.0, -4.0), (28.0, 20.0)], 1.0, Rgb::hex(0x7a3e00));
        });
    }
    "Canvas::fill_path", 24 x 4 => |c, t| {
        let mut p = Path::new();
        p.move_to((2.0, 14.0)).quad_to((12.0, -8.0), (22.0, 14.0)).close();
        p.ellipse(12.0, 9.0, 3.0, 2.5); // inside the first subpath: a hole
        p.rect(28.0, 2.0, 16.0, 12.0).rect(32.0, 5.0, 8.0, 6.0);
        c.fill_path(&p, ORANGE);
    }
    "Canvas::stroke_path", 24 x 4 => |c, t| {
        let mut p = Path::new();
        p.move_to((2.0, 14.0)).quad_to((12.0, -8.0), (22.0, 14.0)).close();
        p.move_to((26.0, 12.0)).curve_through(&[(32.0, 3.0), (38.0, 13.0), (46.0, 3.0)]);
        c.stroke_path(&p, Pen::new(1.0).dash(4.0, 2.0), CYAN);
    }
    "Paint::new", 24 x 4 => |c, t| {
        c.disc(12.0, 8.0, 7.0, Paint::new(RED));
        c.disc(36.0, 8.0, 7.0, Paint::new(Color::Foreground)); // the terminal's own
    }
    "Paint::dithered", 24 x 4 => |c, t| {
        for (i, coverage) in [0.15, 0.35, 0.6, 0.85].into_iter().enumerate() {
            c.fill_rect(i as f32 * 12.0, 0.0, 12.0, 16.0, Paint::dithered(BLUE, coverage));
        }
    }
    "Paint::erase", 24 x 4 => |c, t| {
        c.fill_rect(0.0, 0.0, 48.0, 16.0, PURPLE);
        c.text(4, 3, "GONE", &Font::tiny().scale(2), Paint::erase());
    }
    "Paint::pattern", 24 x 4 => |c, t| {
        c.fill_rect(0.0, 0.0, 15.0, 16.0, Paint::pattern(GREEN, Pattern::Diagonal(4)));
        c.fill_rect(16.0, 0.0, 15.0, 16.0, Paint::pattern(GREEN, Pattern::Checker(2)));
        c.disc(40.0, 8.0, 7.5, Paint::pattern(GREEN, Pattern::Dots(3)));
    }
    "Pattern", 32 x 5 => |c, t| {
        let all = [
            Pattern::Rows(3), Pattern::Columns(3), Pattern::Diagonal(4), Pattern::Antidiagonal(4),
            Pattern::Cross(6), Pattern::Grid(4), Pattern::Checker(3), Pattern::Dots(4),
        ];
        for (i, p) in all.into_iter().enumerate() {
            c.fill_rect(i as f32 * 8.0 + 0.5, 0.0, 7.0, 20.0, Paint::pattern(CYAN, p));
        }
    }
    "Paint::linear", 24 x 4 => |c, t| {
        c.fill_rect(0.0, 0.0, 23.0, 16.0, Paint::linear((0.0, 0.0), (23.0, 0.0), RED, YELLOW));
        c.disc(36.0, 8.0, 7.5, Paint::linear((30.0, 2.0), (42.0, 14.0), BLUE, GREEN));
    }
    "Paint::radial", 24 x 4 => |c, t| {
        c.fill_rect(0.0, 0.0, 23.0, 16.0, Paint::radial((11.5, 8.0), 12.0, YELLOW, RED));
        c.disc(36.0, 8.0, 7.5, Paint::radial((33.0, 5.0), 10.0, t.ink, BLUE));
    }
    "Paint::edge", 24 x 4 => |c, t| {
        c.fill_star(9.0, 8.0, 8.0, 3.5, 5, -PI / 2.0, Paint::edge(YELLOW, RED, 3.0));
        c.fill_round_rect(20.0, 1.0, 26.0, 14.0, 5.0, Paint::edge(t.ink, BLUE, 4.0));
    }
    "Paint::shader", 24 x 4 => |c, t| {
        // Lit from the top left, using the dot's place in the shape.
        let lit = Paint::shader(BLUE, t.ink, |p| Some(p.mix(1.0 - (p.u + p.v) / 2.0)));
        c.disc(9.0, 8.0, 7.5, lit);
        // Rings, from the distance to the edge.
        let rings = Paint::shader(GREEN, YELLOW, |p| Some(p.mix(((-p.dist / 2.0) % 2.0 < 1.0) as i32 as f32)));
        c.fill_rect(20.0, 0.0, 28.0, 16.0, rings);
    }
    "Paint::dither", 24 x 4 => |c, t| {
        let grad = Paint::linear((0.0, 0.0), (48.0, 0.0), PURPLE, CYAN);
        c.fill_rect(0.0, 0.0, 48.0, 7.0, grad);
        c.fill_rect(0.0, 9.0, 48.0, 7.0, grad.dither(0.5));
    }
    "Paint::anchor", 24 x 4 => |c, t| {
        // The same hatch, anchored at each box's corner: it lines up with the box.
        for i in 0..4 {
            let x = 1.0 + i as f32 * 12.0;
            c.fill_rect(x, 2.0, 10.0, 12.0, Paint::pattern(ORANGE, Pattern::Grid(4)).anchor(x as i32, 2));
        }
    }
    "Paint::cel", 24 x 4 => |c, t| {
        // Three tones by how much the surface faces the light, from the upper left.
        let ball = Paint::cel(Rgb::hex(0x7a3e00), (-1.0, -1.0), &[(-0.2, ORANGE), (0.5, YELLOW)]);
        c.disc(9.0, 8.0, 7.5, ball);
        c.fill_round_rect(20.0, 1.0, 26.0, 14.0, 6.0, ball);
    }
    "Paint::per_cell", 24 x 4 => |c, t| {
        // Left: bands wherever the normal says. Right: the same, decided once per
        // cell, so every band is at least a cell and survives the text fallback.
        let ball = Paint::cel(BLUE.dim(0.4), (-1.0, -1.0), &[(-0.2, BLUE), (0.5, t.ink)]);
        c.disc(10.0, 8.0, 7.5, ball);
        c.disc(36.0, 8.0, 7.5, ball.per_cell());
    }
    "Paint::soften", 24 x 4 => |c, t| {
        // A hard terminator, and one that dissolves into a dither over the band edge.
        let ball = Paint::cel(PURPLE.dim(0.4), (-1.0, -0.5), &[(0.1, PURPLE)]);
        c.disc(10.0, 8.0, 7.5, ball);
        c.disc(36.0, 8.0, 7.5, ball.soften(0.35));
    }
    "Paint::hashed", 24 x 4 => |c, t| {
        // The same coverage: ordered on the left reads as a lattice, hashed on the
        // right reads as grain.
        c.fill_rect(0.0, 0.0, 23.0, 16.0, Paint::dithered(GREEN, 0.15));
        c.fill_rect(25.0, 0.0, 23.0, 16.0, Paint::dithered(GREEN, 0.15).hashed());
    }
    "Probe::lit", 24 x 4 => |c, t| {
        // Smooth shading from the normal: `lit` is -1..1, mixed into a colour.
        let lit = Paint::shader(CYAN.dim(0.3), CYAN, |p| Some(p.mix((p.lit((-1.0, -1.0)) + 1.0) / 2.0)));
        c.disc(9.0, 8.0, 7.5, lit);
        c.fill_round_rect(20.0, 1.0, 26.0, 14.0, 5.0, lit);
    }
    "Pen::dash", 24 x 4 => |c, t| {
        c.polyline(&[(1.0, 3.0), (47.0, 3.0)], Pen::new(1.0).dash(3.0, 2.0), GREEN);
        c.polyline(&[(1.0, 8.0), (47.0, 8.0)], Pen::new(2.0).dash(6.0, 3.0), GREEN);
        c.ellipse(24.0, 12.0, 20.0, 3.0, Pen::new(1.0).dash(4.0, 4.0), BLUE);
    }
    "Pen::dotted", 24 x 4 => |c, t| {
        c.polyline(&[(1.0, 4.0), (47.0, 4.0)], Pen::new(1.0).dotted(), CYAN);
        c.polyline(&[(2.0, 11.0), (46.0, 11.0)], Pen::new(3.0).dotted(), CYAN);
    }
    "Pen::phase", 24 x 4 => |c, t| {
        for i in 0..4 {
            let y = 2.0 + i as f32 * 4.0;
            c.polyline(&[(1.0, y), (47.0, y)], Pen::new(1.0).dash(4.0, 4.0).phase(i as f32 * 2.0), YELLOW);
        }
    }
    // ----- path -----------------------------------------------------------------
    "Path", 24 x 4 => |c, t| {
        let mut p = Path::new();
        p.move_to((2.0, 14.0)).line_to((8.0, 2.0)).quad_to((14.0, 14.0), (20.0, 2.0));
        p.cubic_to((26.0, -4.0), (30.0, 20.0), (34.0, 6.0)).arc_to(40.0, 8.0, 6.0, 6.0, PI, 2.5 * PI);
        c.stroke_path(&p, 1.0, PURPLE);
    }
    "Path::arc_to", 24 x 4 => |c, t| {
        let mut p = Path::new();
        p.move_to((2.0, 14.0)).arc_to(12.0, 8.0, 8.0, 6.0, PI, TAU).line_to((24.0, 14.0));
        p.arc_to(36.0, 8.0, 7.0, 7.0, 0.5 * PI, 2.0 * PI);
        c.stroke_path(&p, 1.0, RED);
    }
    "Path::curve_through", 24 x 4 => |c, t| {
        let pts = [(2.0, 12.0), (12.0, 2.0), (22.0, 13.0), (32.0, 3.0), (46.0, 12.0)];
        let mut p = Path::new();
        p.curve_through(&pts);
        c.stroke_path(&p, 1.0, GREEN);
        for q in pts {
            c.disc(q.0, q.1, 1.2, t.ink);
        }
    }
    "Path::rotate", 24 x 4 => |c, t| {
        for i in 0..5 {
            let mut p = Path::new();
            p.rect(-6.0, -2.0, 12.0, 4.0).rotate(i as f32 * PI / 5.0, (0.0, 0.0)).translate(24.0, 8.0);
            c.stroke_path(&p, 1.0, BLUE.lerp(CYAN, i as f32 / 4.0));
        }
    }
    "Path::transform", 24 x 4 => |c, t| {
        let mut p = Path::new();
        p.polygon(&[(0.0, 0.0), (6.0, 0.0), (3.0, 6.0)]);
        for i in 0..6 {
            let mut copy = p.clone();
            copy.scale(1.0 + i as f32 * 0.3, 1.0 + i as f32 * 0.3).translate(2.0 + i as f32 * 8.0, 2.0);
            c.fill_path(&copy, ORANGE);
        }
    }
    // ----- mask -----------------------------------------------------------------
    "Mask", 24 x 4 => |c, t| {
        // A silhouette built from parts, then painted, shaded and outlined as one.
        let mut duck = Mask::new(24, 4);
        duck.draw(|c| {
            c.fill_ellipse(20.0, 10.0, 11.0, 5.0, t.ink);
            c.fill_ellipse(30.0, 4.0, 4.0, 3.5, t.ink);
            c.fill_polygon(&[(33.0, 4.0), (40.0, 3.0), (33.0, 6.0)], t.ink);
        });
        let mut eye = Mask::new(24, 4);
        eye.draw(|c| c.disc(31.0, 3.0, 0.8, t.ink));
        duck.subtract(&eye);
        c.stencil(&duck, Paint::cel(Rgb::hex(0x7a3e00), (-1.0, -1.0), &[(-0.1, ORANGE), (0.5, YELLOW)]).per_cell());
        c.effects(&duck, &[Effect::outline(1.0).paint(t.ink)]);
    }
    "Mask::draw", 24 x 4 => |c, t| {
        let mut m = Mask::new(24, 4);
        m.draw(|c| {
            c.fill_star(10.0, 8.0, 7.5, 3.5, 5, -PI / 2.0, t.ink);          // any primitive
            c.fill_rect(20.0, 2.0, 26.0, 12.0, Paint::dithered(t.ink, 0.5)); // any paint
        });
        c.stencil(&m, GREEN);
    }
    "Mask::subtract" | "Mask::union" | "Mask::intersect", 24 x 4 => |c, t| {
        let mut a = Mask::new(24, 4);
        a.draw(|c| c.disc(10.0, 8.0, 7.0, t.ink));
        let mut b = Mask::new(24, 4);
        b.draw(|c| c.disc(15.0, 8.0, 7.0, t.ink));
        let (mut union, mut cut, mut both) = (a.clone(), a.clone(), a.clone());
        union.union(&b);
        cut.subtract(&b);
        both.intersect(&b);
        union.translate(0, 0);
        cut.translate(20, 0);
        both.translate(34, 0);
        c.stencil(&union, RED);
        c.stencil(&cut, YELLOW);
        c.stencil(&both, BLUE);
    }
    "Mask::boundary_with", 24 x 4 => |c, t| {
        let mut body = Mask::new(24, 4);
        body.draw(|c| c.fill_ellipse(22.0, 10.0, 12.0, 5.5, t.ink));
        let mut head = Mask::new(24, 4);
        head.draw(|c| c.fill_ellipse(32.0, 5.0, 5.0, 4.5, t.ink));
        body.subtract(&head);
        c.stencil(&body, ORANGE);
        c.stencil(&head, GREEN);
        // The seam where the two meet: a collar, drawn on the body's side.
        c.stencil(&body.boundary_with(&head), t.ink);
    }
    "Mask::transform", 24 x 4 => |c, t| {
        // One limb, built once, hung from its pivot at three angles.
        let mut limb = Mask::new(24, 4);
        limb.draw(|c| {
            c.fill_round_rect(2.5, 1.0, 3.0, 10.0, 1.5, t.ink);
            c.disc(4.0, 12.0, 2.5, t.ink);
        });
        for (i, angle) in [-0.5, 0.0, 0.5].into_iter().enumerate() {
            let mut placed = limb.clone();
            placed.transform(&Transform::at(6.0 + 14.0 * i as f32, 0.0).rotate_about(angle, (4.0, 1.0)));
            c.stencil(&placed, CYAN.lerp(PURPLE, i as f32 / 2.0));
        }
    }
    "Mask::flip_x", 24 x 4 => |c, t| {
        let mut fish = Mask::new(24, 4);
        fish.draw(|c| {
            c.fill_ellipse(12.0, 8.0, 8.0, 4.0, t.ink);
            c.fill_polygon(&[(4.0, 8.0), (0.0, 3.0), (0.0, 13.0)], t.ink);
        });
        c.stencil(&fish, BLUE);
        fish.flip_x(24.0); // mirrored about the middle of the canvas: facing left
        c.stencil(&fish, GREEN);
    }
    // ----- transform ------------------------------------------------------------
    "Transform", 24 x 4 => |c, t| {
        // A chain reads outer to inner: rotated, then scaled, then placed.
        for i in 0..5 {
            let at = Transform::at(5.0 + 9.5 * i as f32, 8.0).scale(1.0 + 0.2 * i as f32, 1.0).rotate(0.3 * i as f32);
            c.with(at, |c| c.fill_rect(-3.0, -3.0, 6.0, 6.0, RED.lerp(YELLOW, i as f32 / 4.0)));
        }
    }
    "Canvas::with", 24 x 4 => |c, t| {
        // A figure drawn about its own origin, facing either way.
        let figure = |c: &mut Canvas| {
            c.fill_ellipse(0.0, 0.0, 5.0, 3.5, GREEN);      // body
            c.disc(5.0, -4.0, 2.2, GREEN);                  // head
            c.polyline(&[(7.0, -4.0), (10.0, -3.0)], 1.0, ORANGE); // beak
            c.polyline(&[(-1.0, 3.0), (-1.0, 7.0)], 1.0, ORANGE);  // leg
        };
        c.with(Transform::at(12.0, 8.0), figure);
        c.with(Transform::at(36.0, 8.0).flip_x(), figure);
    }
    // ----- layer ----------------------------------------------------------------
    "Layers", 24 x 4 => |c, t| {
        let mut layers = Layers::new(24, 4);
        layers[0].fill_rect(0.0, 0.0, 48.0, 16.0, Paint::pattern(t.panel, Pattern::Rows(2)));
        layers.push().disc(18.0, 8.0, 7.0, BLUE);
        layers.push().disc(28.0, 8.0, 7.0, RED);
        *c = layers.flatten().clone();
    }
    "Effect::shadow", 24 x 4 => |c, t| {
        let mut layers = Layers::new(24, 4);
        layers[0].fill_rect(0.0, 0.0, 48.0, 16.0, Paint::dithered(t.panel, 0.7));
        let card = layers.push();
        card.fill_round_rect(6.0, 2.0, 30.0, 10.0, 3.0, YELLOW);
        card.effect(Effect::shadow(2, 2));
        *c = layers.flatten().clone();
    }
    "Effect::outline", 24 x 4 => |c, t| {
        let mut layers = Layers::new(24, 4);
        layers[0].fill_rect(0.0, 0.0, 48.0, 16.0, Paint::dithered(t.panel, 0.7));
        let shape = layers.push();
        shape.fill_star(12.0, 8.0, 7.0, 3.0, 5, -PI / 2.0, RED);
        shape.print(11, 1, "text", RED);
        shape.effect(Effect::outline(1.0).paint(t.ink));
        *c = layers.flatten().clone();
    }
    "Effect::glow", 24 x 4 => |c, t| {
        let mut layers = Layers::new(24, 4);
        layers[0].fill_rect(0.0, 0.0, 48.0, 16.0, Paint::dithered(t.panel, 0.7));
        let shape = layers.push();
        shape.disc(12.0, 8.0, 4.0, CYAN);
        shape.effect(Effect::glow(5.0).paint(CYAN));
        let shape = layers.push();
        shape.disc(36.0, 8.0, 4.0, YELLOW);
        shape.effect(Effect::glow(5.0).paint(YELLOW));
        *c = layers.flatten().clone();
    }
    "Effect::gap", 24 x 4 => |c, t| {
        let mut layers = Layers::new(24, 4);
        layers[0].fill_rect(0.0, 0.0, 48.0, 16.0, Paint::pattern(BLUE, Pattern::Cross(4)));
        let shape = layers.push();
        shape.fill_round_rect(10.0, 3.0, 28.0, 10.0, 4.0, GREEN);
        shape.effect(Effect::gap(2.0));
        *c = layers.flatten().clone();
    }
    "Effect::rim", 24 x 4 => |c, t| {
        let mut layers = Layers::new(24, 4);
        let shape = layers.push();
        shape.disc(12.0, 8.0, 7.5, ORANGE);
        shape.effect(Effect::rim(2.0));
        let shape = layers.push();
        shape.fill_round_rect(24.0, 1.0, 22.0, 14.0, 4.0, PURPLE);
        shape.effect(Effect::rim(1.0).paint(t.ink));
        *c = layers.flatten().clone();
    }
    "Effect::shader", 24 x 4 => |c, t| {
        let mut layers = Layers::new(24, 4);
        layers[0].fill_rect(0.0, 0.0, 48.0, 16.0, Paint::dithered(t.panel, 0.7));
        let shape = layers.push();
        shape.disc(24.0, 8.0, 5.0, RED);
        // Concentric rings: every other dot of distance, fading out.
        shape.effect(Effect::shader(8.0, 0.0, |s| {
            (s.dist.round() as i32 % 3 == 0).then(|| Paint::dithered(RED, 1.0 - s.dist / 9.0))
        }));
        *c = layers.flatten().clone();
    }
    "Layer::matte", 24 x 4 => |c, t| {
        let mut layers = Layers::new(24, 4);
        layers[0].fill_rect(0.0, 0.0, 48.0, 16.0, Paint::pattern(BLUE, Pattern::Diagonal(3)));
        let hole = layers.push();
        hole.disc(24.0, 8.0, 6.0, t.ink); // the colour does not matter
        hole.matte = true;
        hole.effect(Effect::outline(1.0).paint(RED));
        *c = layers.flatten().clone();
    }
    "Layer::offset", 24 x 4 => |c, t| {
        // One ridge, drawn once on each of three layers, each moved by its own
        // amount: the further back, the less it moves. Scroll them every frame and
        // the scene has depth.
        let mut layers = Layers::new(24, 4);
        let ridge = [(0.0, 12.0), (8.0, 4.0), (14.0, 9.0), (22.0, 2.0), (30.0, 10.0), (38.0, 5.0), (48.0, 12.0)];
        let shades = [t.panel, BLUE.dim(0.5), BLUE];
        for (i, shade) in shades.into_iter().enumerate() {
            let layer = if i == 0 { &mut layers[0] } else { layers.push() };
            let mut hill: Vec<(f32, f32)> = ridge.iter().map(|&(x, y)| (x, y + 2.0 * i as f32)).collect();
            hill.extend([(48.0, 16.0), (0.0, 16.0)]);
            layer.fill_polygon(&hill, shade);
            layer.wrap = true;
            layer.offset = (-6 * i as i32, 0);
        }
        *c = layers.flatten().clone();
    }
    "Layer::wrap", 24 x 4 => |c, t| {
        let mut layers = Layers::new(24, 4);
        layers[0].fill_rect(0.0, 0.0, 48.0, 16.0, Paint::dithered(t.panel, 0.7));
        let train = layers.push();
        for i in 0..4 {
            train.fill_round_rect(2.0 + 12.0 * i as f32, 4.0, 9.0, 8.0, 2.0, [RED, ORANGE, YELLOW, GREEN][i]);
        }
        train.wrap = true;
        train.offset = (7, 0); // the last car comes back on the left
        *c = layers.flatten().clone();
    }
    "Layer::scroll", 24 x 4 => |c, t| {
        let mut layers = Layers::new(24, 4);
        layers[0].fill_rect(0.0, 0.0, 48.0, 16.0, Paint::dithered(t.panel, 0.7));
        let comet = layers.push();
        comet.disc(6.0, 8.0, 3.0, CYAN);
        comet.effect(Effect::glow(3.0).paint(CYAN));
        // Ten frames on, the comet is further right; each frame moved it by four dots.
        for _ in 0..10 {
            comet.scroll(4, 0);
        }
        *c = layers.flatten().clone();
    }
    "Canvas::effects", 24 x 4 => |c, t| {
        c.fill_rect(0.0, 0.0, 48.0, 16.0, Paint::dithered(t.panel, 0.7));
        let mut mask = Canvas::new(24, 4);
        mask.text(3, 3, "MASK", &Font::tiny().scale(2), t.ink);
        c.effects(&mask, &[Effect::gap(1.0), Effect::outline(1.0).paint(GREEN)]);
    }
    "Canvas::effects_in", 24 x 4 => |c, t| {
        let mut ground = Mask::new(24, 4);
        ground.draw(|c| c.fill_rect(0.0, 13.0, 48.0, 3.0, t.ink));
        let mut figure = Mask::new(24, 4);
        figure.draw(|c| c.fill_ellipse(24.0, 7.0, 9.0, 6.5, t.ink));
        c.stencil(&ground, t.panel);
        c.stencil(&figure, YELLOW);
        // Contact shadow: the figure darkens within four dots of the ground, and
        // nothing else does.
        c.effects_in(&figure, &ground, &[Effect::shader(4.0, 0.0, |s| match s.color {
            Some(Color::Rgb(c)) => Some(Paint::new(c.dim(0.45 + 0.5 * s.dist / 4.0))),
            _ => None,
        })]);
    }
    "Field" | "Field::new" | "Field::effects", 24 x 4 => |c, t| {
        c.fill_rect(0.0, 0.0, 48.0, 16.0, Paint::dithered(t.panel, 0.7));
        // One scratch for every ring: nothing is allocated after the first.
        let mut field = Field::new();
        let mut mask = Canvas::new(24, 4);
        for (i, color) in [RED, YELLOW, GREEN, CYAN, PURPLE].into_iter().enumerate() {
            mask.clear();
            mask.disc(6.0 + 9.0 * i as f32, 8.0, 3.0, color);
            field.effects(c, &mask, &[Effect::gap(1.0), Effect::outline(1.0).paint(color)]);
        }
    }
    // ----- bubble ---------------------------------------------------------------
    "Bubble::new", 24 x 5 => |c, t| {
        Bubble::new("A text box.").ink(t.ink).border(1.0, t.ink).draw(c, 4.0, 4.0);
    }
    "Bubble::speech", 28 x 6 => |c, t| {
        c.disc(8.0, 20.0, 3.0, GREEN);
        Bubble::speech("Hello!").ink(t.bg).fill(BLUE).speak(c, (8.0, 18.0), &[]);
    }
    "Bubble::thought", 32 x 7 => |c, t| {
        c.disc(10.0, 24.0, 3.0, GREEN);
        Bubble::thought("hmm").ink(t.ink).border(1.0, t.ink).speak(c, (10.0, 22.0), &[]);
    }
    "Bubble::shout", 32 x 7 => |c, t| {
        c.disc(12.0, 24.0, 3.0, GREEN);
        Bubble::shout("HEY!").ink(t.bg).fill(YELLOW).speak(c, (12.0, 22.0), &[]);
    }
    "Bubble::whisper", 28 x 6 => |c, t| {
        c.disc(8.0, 20.0, 3.0, GREEN);
        Bubble::whisper("psst").ink(t.ink).border(1.0, t.ink).speak(c, (8.0, 18.0), &[]);
    }
    "Bubble::speak", 40 x 8 => |c, t| {
        let keep_out = cobra::Rect::new(0.0, 0.0, 80.0, 12.0);
        c.fill_rect(0.0, 0.0, 80.0, 12.0, Paint::pattern(t.panel, Pattern::Rows(2)));
        c.disc(60.0, 28.0, 3.0, GREEN);
        Bubble::speech("Mind the bar.").ink(t.bg).fill(BLUE).speak(c, (60.0, 26.0), &[keep_out]);
    }
    "Shape" | "Bubble::shape", 44 x 6 => |c, t| {
        let shapes = [Shape::Rect, Shape::Round(3.0), Shape::Ellipse, Shape::Cloud, Shape::Burst];
        let mut x = 0.0;
        for shape in shapes {
            let b = Bubble::new("hi").shape(shape).ink(t.ink).border(1.0, t.ink).no_tail();
            let r = b.draw(c, x, 4.0);
            x = r.right() + 2.0;
        }
    }
    "TailKind", 48 x 8 => |c, t| {
        let kinds = [TailKind::Point, TailKind::Curve, TailKind::Bubbles, TailKind::Line];
        for (i, kind) in kinds.into_iter().enumerate() {
            let x = i as f32 * 24.0 + 4.0;
            let b = Bubble::new("tail").ink(t.ink).border(1.0, t.ink).tail(Tail::new(Side::Bottom, 0.5, kind).len(8.0));
            b.draw(c, x, 4.0);
        }
    }
    "Bubble::fill", 24 x 5 => |c, t| {
        Bubble::new("filled").ink(t.bg).fill(PURPLE).no_tail().draw(c, 4.0, 4.0);
    }
    "Bubble::border", 24 x 5 => |c, t| {
        Bubble::new("thick").ink(t.ink).border(2.0, ORANGE).shape(Shape::Round(4.0)).no_tail().draw(c, 4.0, 4.0);
    }
    "Bubble::wrap", 24 x 8 => |c, t| {
        let b = Bubble::new("Long text wraps to the width you set.").wrap(14).ink(t.ink).border(1.0, t.ink);
        b.no_tail().draw(c, 4.0, 4.0);
    }
    "Bubble::align", 24 x 8 => |c, t| {
        let b = Bubble::new("one\ncentred\nline").align(Align::Center).ink(t.ink).border(1.0, t.ink);
        b.no_tail().draw(c, 4.0, 4.0);
    }
    "Bubble::clear_behind", 24 x 6 => |c, t| {
        c.fill_rect(0.0, 0.0, 48.0, 24.0, Paint::pattern(BLUE, Pattern::Diagonal(3)));
        Bubble::new("clear").ink(t.ink).border(1.0, t.ink).clear_behind().no_tail().draw(c, 8.0, 4.0);
    }
    // ----- font -----------------------------------------------------------------
    "Font::tiny", 24 x 3 => |c, t| {
        c.text(1, 1, "Hello, 3x5!", Font::tiny(), t.ink);
        c.text(1, 7, "abc XYZ 0123", Font::tiny(), BLUE);
    }
    "Font::scale", 24 x 4 => |c, t| {
        c.text(1, 1, "x1", Font::tiny(), t.ink);
        c.text(10, 1, "x2", &Font::tiny().scale(2), t.ink);
        c.text(26, 1, "x3", &Font::tiny().scale(3), t.ink);
    }
    "Font::scale_xy", 24 x 4 => |c, t| {
        c.text(1, 3, "1x2", &Font::tiny().scale_xy(1, 2), GREEN);
        c.text(18, 5, "2x1", &Font::tiny().scale_xy(2, 1), GREEN);
        c.text(34, 1, "1x3", &Font::tiny().scale_xy(1, 3), GREEN);
    }
    "Font::add" | "Font::empty" | "Font::len" | "Font::is_empty", 24 x 3 => |c, t| {
        let mut font = Font::tiny().clone();          // 95 glyphs; `Font::empty()` has none
        font.add('♥', &[".#.#.", "#####", "#####", ".###.", "..#.."]);
        c.text(1, 1, "I ♥ dots", &font, RED);
        c.text(28, 1, &format!("{} glyphs", font.len()), Font::tiny(), t.ink);
    }
    "Canvas::text", 28 x 4 => |c, t| {
        c.text(1, 1, "Dots, any\ncolour", Font::tiny(), Paint::linear((0.0, 0.0), (40.0, 0.0), CYAN, PURPLE));
        c.text(32, 3, "BIG", &Font::tiny().scale(2), YELLOW);
    }
    // ----- text -----------------------------------------------------------------
    "Canvas::print", 24 x 3 => |c, t| {
        c.fill_rect(0.0, 0.0, 48.0, 12.0, Paint::pattern(t.panel, Pattern::Checker(2)));
        c.print(1, 1, "Real text: 日本語 ✓", TextStyle::new(t.ink).bold());
    }
    "Canvas::print_wrapped", 24 x 4 => |c, t| {
        let text = "Wrapped at a width, aligned as asked.";
        c.print_wrapped(1, 0, 22, text, TextStyle::new(t.ink), Align::Right);
    }
    "TextStyle" | "TextStyle::new" | "TextStyle::on" | "TextStyle::bold" | "TextStyle::dim" | "TextStyle::italic" | "TextStyle::underline", 30 x 3 => |c, t| {
        c.print(1, 1, "bold", TextStyle::new(t.ink).bold());
        c.print(6, 1, "dim", TextStyle::new(t.ink).dim());
        c.print(10, 1, "italic", TextStyle::new(BLUE).italic());
        c.print(17, 1, "under", TextStyle::new(GREEN).underline());
        c.print(23, 1, "on bg", TextStyle::new(t.bg).on(RED).with(Attrs::BOLD));
    }
    "Align", 24 x 3 => |c, t| {
        c.print_wrapped(0, 0, 24, "left", t.ink, Align::Left);
        c.print_wrapped(0, 1, 24, "centre", t.ink, Align::Center);
        c.print_wrapped(0, 2, 24, "right", t.ink, Align::Right);
    }
    // ----- color ----------------------------------------------------------------
    "Rgb::lerp", 24 x 3 => |c, t| {
        for i in 0..12 {
            c.fill_rect(i as f32 * 4.0, 0.0, 4.0, 12.0, RED.lerp(BLUE, i as f32 / 11.0));
        }
    }
    "Rgb::dim", 24 x 3 => |c, t| {
        for i in 0..6 {
            c.fill_rect(i as f32 * 8.0, 0.0, 8.0, 12.0, GREEN.dim(1.0 - i as f32 / 6.0));
        }
    }
    "Color", 24 x 3 => |c, t| {
        // Palette colours follow the terminal's theme; the file shows the default palette.
        for i in 0..8u8 {
            c.fill_rect(i as f32 * 5.0, 0.0, 5.0, 6.0, Color::Indexed(i));
            c.fill_rect(i as f32 * 5.0, 6.0, 5.0, 6.0, Color::Indexed(i + 8));
        }
        c.fill_rect(42.0, 0.0, 6.0, 12.0, Color::Foreground);
    }
    // ----- canvas, the rest ------------------------------------------------------
    "Canvas", 24 x 4 => |c, t| {
        // Cells of dots, each dot its own colour: a scene, drawn with the shapes and
        // paints below and a printed label.
        c.fill_rect(0.0, 11.0, 48.0, 5.0, Paint::linear((0.0, 11.0), (0.0, 16.0), GREEN, GREEN.dim(0.4)));
        c.disc(38.0, 5.0, 4.0, Paint::radial((37.0, 4.0), 5.0, YELLOW, ORANGE));
        c.fill_polygon(&[(4.0, 11.0), (14.0, 2.0), (24.0, 11.0)], Paint::edge(t.ink, BLUE, 3.0));
        c.print(1, 0, "hill", t.ink);
    }
    "Canvas::new" | "Canvas::blit", 24 x 4 => |c, t| {
        // A sprite drawn once on a canvas of its own, copied wherever it is wanted.
        let mut star = Canvas::new(4, 2);
        star.fill_star(4.0, 4.0, 3.8, 1.6, 5, -PI / 2.0, YELLOW);
        for (i, y) in [6, 2, 8, 1, 5].into_iter().enumerate() {
            c.blit(&star, 1 + i as i32 * 9, y);
        }
    }
    "DOTS_X" | "DOTS_Y" | "Canvas::cols" | "Canvas::rows" | "Canvas::width" | "Canvas::height", 12 x 3 => |c, t| {
        // A 12 × 3-cell canvas is 24 × 12 dots: every cell holds DOTS_X × DOTS_Y of them.
        let (cw, ch) = (DOTS_X as i32, DOTS_Y as i32);
        for row in 0..c.rows() as i32 {
            for col in 0..c.cols() as i32 {
                let shade = if (col + row) % 2 == 0 { BLUE } else { t.panel };
                c.fill_rect((col * cw) as f32, (row * ch) as f32, cw as f32, ch as f32, shade);
            }
        }
        c.set(0, 0, RED);                                // the first dot
        c.set(c.width() - 1, c.height() - 1, RED);       // the last: (width - 1, height - 1)
    }
    "Cell" | "Cell::glyph" | "braille" | "Canvas::cell" | "Canvas::cells", 24 x 4 => |c, t| {
        // Left: the dots. Right: each cell as the text protocol sends it, the braille
        // glyph of its dot pattern in the cell's one dominant colour.
        c.fill_ellipse(12.0, 8.0, 10.0, 6.0, Paint::linear((2.0, 0.0), (22.0, 0.0), RED, BLUE));
        c.line(2, 14, 22, 2, YELLOW);
        for row in 0..c.rows() {
            for col in 0..12 {
                let cell = c.cell(col, row);
                if let Some(color) = cell.color {
                    c.print(col as i32 + 12, row as i32, &cell.glyph().to_string(), color);
                }
            }
        }
    }
    "Cell::dot", 24 x 4 => |c, t| {
        // A cell's dots read back one by one: the left half, redrawn on the right
        // with every dot in its cell's colour, which is what a glyph can show.
        c.disc(12.0, 8.0, 7.0, Paint::radial((9.0, 5.0), 9.0, YELLOW, RED));
        for row in 0..c.rows() {
            for col in 0..12 {
                let cell = c.cell(col, row);
                for dy in 0..DOTS_Y {
                    for dx in 0..DOTS_X {
                        if cell.dot(dx, dy) {
                            c.set(((col + 12) * DOTS_X + dx) as i32, (row * DOTS_Y + dy) as i32, cell.color.unwrap());
                        }
                    }
                }
            }
        }
    }
    "bayer", 24 x 3 => |c, t| {
        // The 4 × 4 threshold matrix, each entry as a shade, and the ramp it dithers:
        // a dot is drawn where the coverage beats its threshold.
        for y in 0..4 {
            for x in 0..4 {
                c.fill_rect(x as f32 * 3.0, y as f32 * 3.0, 3.0, 3.0, t.panel.lerp(BLUE, bayer(x, y)));
            }
        }
        for y in 0..12 {
            for x in 14..48 {
                if (x - 14) as f32 / 34.0 > bayer(x, y) {
                    c.set(x, y, GREEN);
                }
            }
        }
    }
    "Canvas::fallback", 24 x 4 => |c, t| {
        // What a plain terminal makes of the left half: a cell's dots keep their
        // pattern and take its one dominant colour, quantised to 256 colours here.
        c.disc(12.0, 8.0, 7.0, Paint::linear((5.0, 1.0), (19.0, 15.0), CYAN, PURPLE));
        c.polyline(&[(2.0, 14.0), (22.0, 2.0)], 1.0, YELLOW);
        c.blit(&c.clone().fallback(Depth::Ansi256, &Palette::default()), 24, 0);
    }
    "Canvas::transform", 24 x 4 => |c, t| {
        c.with(Transform::at(6.0, 8.0).scale(1.5, 1.5), |c| {
            c.with(Transform::at(4.0, 0.0).rotate(0.3), |c| {
                // The composed transform, inner first: a one-dot stroke comes out
                // one and a half wide, which is its scale factor.
                let k = c.transform().scale_factor();
                c.polyline(&[(0.0, 0.0), (16.0, 0.0)], 1.0, GREEN);
                c.text(0, 2, &format!("k={k:.1}"), Font::tiny(), YELLOW);
            });
        });
    }
    "Canvas::clear", 24 x 4 => |c, t| {
        c.fill_rect(0.0, 0.0, 48.0, 16.0, RED);
        c.print(1, 1, "gone", t.ink);
        c.clear(); // dots and text, the allocation kept
        c.disc(24.0, 8.0, 6.0, GREEN);
    }
    "Canvas::get", 24 x 4 => |c, t| {
        // The left half read back dot by dot and mirrored on the right, upside down.
        c.fill_pie(12.0, 8.0, 10.0, 7.0, -0.8 * PI, 0.3 * PI, ORANGE);
        c.disc(6.0, 4.0, 2.5, BLUE);
        for y in 0..16 {
            for x in 0..24 {
                if let Some(color) = c.get(x, y) {
                    c.set(47 - x, 15 - y, color);
                }
            }
        }
    }
    // ----- draw, the rest --------------------------------------------------------
    "Paint", 24 x 4 => |c, t| {
        // One shape in six paints: a colour, a dither, a pattern, a gradient, an
        // edge gradient and cel bands.
        let paints = [
            Paint::new(BLUE),
            Paint::dithered(BLUE, 0.4),
            Paint::pattern(BLUE, Pattern::Diagonal(3)),
            Paint::linear((0.0, 0.0), (0.0, 16.0), BLUE, GREEN),
            Paint::edge(t.ink, BLUE, 2.5),
            Paint::cel(BLUE.dim(0.4), (-1.0, -1.0), &[(-0.2, BLUE), (0.5, CYAN)]),
        ];
        for (i, paint) in paints.into_iter().enumerate() {
            c.fill_round_rect(i as f32 * 8.0 + 0.5, 1.0, 7.0, 14.0, 2.5, paint);
        }
    }
    "Pattern::on", 24 x 4 => |c, t| {
        // The pattern as a predicate, for when a paint is not what is wanted.
        for y in 0..16 {
            for x in 0..48 {
                if Pattern::Dots(4).on(x, y) {
                    c.disc(x as f32 + 0.5, y as f32 + 0.5, 1.2, CYAN);
                }
            }
        }
    }
    "Shader" | "Probe::mix", 24 x 4 => |c, t| {
        // A shader is a plain function. `mix` blends `a` towards `b` for RGB colours
        // and dithers between them for palette colours, which cannot blend.
        fn across(p: &Probe) -> Option<Paint> {
            Some(p.mix(p.u))
        }
        c.fill_round_rect(1.0, 1.0, 22.0, 14.0, 4.0, Paint::shader(RED, BLUE, across));
        c.fill_round_rect(25.0, 1.0, 22.0, 14.0, 4.0, Paint::shader(Color::Indexed(1), Color::Indexed(4), across));
    }
    "Probe", 24 x 4 => |c, t| {
        // What a shader is told about a dot: where it sits across the shape (`u`,
        // `v`), how deep inside it is (`dist`), and which way the surface faces there.
        c.fill_round_rect(1.0, 1.0, 14.0, 14.0, 4.0, Paint::shader(RED, BLUE, |p| Some(p.mix(p.v))));
        c.fill_round_rect(17.0, 1.0, 14.0, 14.0, 4.0, Paint::shader(YELLOW, PURPLE, |p| Some(p.mix(-p.dist / 6.0))));
        c.fill_round_rect(33.0, 1.0, 14.0, 14.0, 4.0, Paint::shader(GREEN, CYAN, |p| Some(p.mix((p.normal.0 + 1.0) / 2.0))));
    }
    "Paint::color" | "Paint::coverage", 24 x 4 => |c, t| {
        // What a paint says about itself: the dithered swatch, then its colour solid
        // and its coverage as a number.
        let paint = Paint::dithered(GREEN, 0.35);
        c.fill_rect(1.0, 1.0, 20.0, 14.0, paint);
        c.fill_rect(24.0, 1.0, 6.0, 14.0, paint.color().unwrap());
        c.text(33, 5, &format!("{:.0}%", paint.coverage() * 100.0), Font::tiny(), t.ink);
    }
    "Pen" | "Pen::new", 24 x 4 => |c, t| {
        for (y, width) in [(1.0, 1.0), (4.0, 2.0), (8.0, 3.0), (13.0, 5.0)] {
            c.polyline(&[(2.0, y), (46.0, y)], Pen::new(width), BLUE);
        }
    }
    "Point", 24 x 4 => |c, t| {
        // Dot coordinates as `f32`: dot `(x, y)` spans `x..x+1`, so its centre is
        // `(x + 0.5, y + 0.5)`.
        let pts: [Point; 4] = [(4.0, 12.0), (16.0, 3.0), (30.0, 13.0), (44.0, 4.0)];
        c.polyline(&pts, 1.0, t.panel);
        for p in pts {
            c.disc(p.0, p.1, 1.5, RED);
        }
    }
    "Rect" | "Rect::new" | "Rect::around" | "Rect::right" | "Rect::bottom" | "Rect::center" | "Rect::contains", 24 x 4 => |c, t| {
        let r = Rect::new(3.0, 2.0, 20.0, 12.0);
        c.rect(r.x, r.y, r.w, r.h, 1.0, BLUE);
        let (cx, cy) = r.center();
        c.disc(cx, cy, 1.5, YELLOW);
        c.disc(r.right(), r.bottom(), 1.5, YELLOW);
        let a = Rect::around((36.0, 8.0), 14.0, 8.0); // by its centre
        c.rect(a.x, a.y, a.w, a.h, 1.0, GREEN);
        for p in [(31.0, 6.0), (36.0, 8.0), (45.0, 3.0), (40.0, 13.0)] {
            c.disc(p.0, p.1, 1.2, if a.contains(p) { GREEN } else { RED });
        }
    }
    "Rect::inset" | "Rect::offset" | "Rect::overlap", 24 x 4 => |c, t| {
        let a = Rect::new(2.0, 1.0, 20.0, 12.0);
        let b = a.offset(14.0, 3.0); // moved
        c.fill_rect(a.x, a.y, a.w, a.h, Paint::dithered(BLUE, 0.5));
        c.fill_rect(b.x, b.y, b.w, b.h, Paint::dithered(RED, 0.5));
        let i = a.inset(3.0); // shrunk on every side
        c.rect(i.x, i.y, i.w, i.h, 1.0, t.ink);
        c.text(38, 1, &format!("{}", a.overlap(&b) as i32), Font::tiny(), t.ink); // the area they share
    }
    // ----- path, the rest --------------------------------------------------------
    "Path::new" | "Path::move_to" | "Path::line_to" | "Path::close" | "Path::current" | "Path::is_empty", 24 x 4 => |c, t| {
        let mut p = Path::new();
        p.move_to((2.0, 14.0)).line_to((8.0, 2.0)).line_to((16.0, 8.0)).close(); // a closed subpath
        p.move_to((22.0, 14.0)).line_to((30.0, 2.0)).line_to((38.0, 10.0)).line_to((46.0, 3.0)); // an open one
        c.stroke_path(&p, 1.0, PURPLE);
        let (x, y) = p.current(); // where the next segment would start
        c.disc(x, y, 1.5, YELLOW);
    }
    "Path::quad_to" | "Path::cubic_to", 24 x 4 => |c, t| {
        let mut p = Path::new();
        p.move_to((1.0, 15.0)).quad_to((12.0, 1.0), (22.0, 15.0)); // one control point
        p.move_to((26.0, 15.0)).cubic_to((30.0, 1.0), (43.0, 15.0), (47.0, 1.0)); // two
        c.stroke_path(&p, 1.0, CYAN);
        // The control polygons, dotted.
        c.polyline(&[(1.0, 15.0), (12.0, 1.0), (22.0, 15.0)], Pen::new(1.0).dotted(), t.panel);
        c.polyline(&[(26.0, 15.0), (30.0, 1.0), (43.0, 15.0), (47.0, 1.0)], Pen::new(1.0).dotted(), t.panel);
    }
    "Path::rect" | "Path::round_rect" | "Path::ellipse" | "Path::polygon", 24 x 4 => |c, t| {
        // Closed shapes as subpaths of one path, filled together; even-odd, so the
        // ellipse inside the box is a hole.
        let mut p = Path::new();
        p.rect(1.0, 1.0, 14.0, 14.0).ellipse(8.0, 8.0, 4.0, 3.0);
        p.round_rect(17.0, 1.0, 14.0, 14.0, 4.0);
        p.polygon(&[(33.0, 15.0), (40.0, 1.0), (47.0, 15.0)]);
        c.fill_path(&p, ORANGE);
    }
    "Path::apply" | "Path::translate" | "Path::scale", 24 x 4 => |c, t| {
        let mut arrow = Path::new();
        arrow.polygon(&[(0.0, -2.0), (4.0, -2.0), (4.0, -4.0), (8.0, 0.0), (4.0, 4.0), (4.0, 2.0), (0.0, 2.0)]);
        let mut a = arrow.clone();
        a.translate(2.0, 8.0);
        let mut b = arrow.clone();
        b.scale(1.5, 1.5).translate(14.0, 8.0);
        let mut d = arrow.clone();
        d.apply(&Transform::at(38.0, 4.0).rotate(PI / 2.0).scale(1.5, 1.5)); // any transform at once
        c.fill_path(&a, GREEN);
        c.fill_path(&b, GREEN);
        c.fill_path(&d, GREEN);
    }
    "Path::bounds", 24 x 4 => |c, t| {
        let mut p = Path::new();
        p.move_to((4.0, 12.0)).curve_through(&[(14.0, 3.0), (26.0, 13.0), (40.0, 4.0)]);
        c.stroke_path(&p, 1.5, GREEN);
        let b = p.bounds().unwrap(); // control points included
        c.rect(b.x, b.y, b.w, b.h, 1.0, Paint::dithered(t.ink, 0.5));
    }
    "Path::subpaths", 24 x 4 => |c, t| {
        // The flattened polylines a path is drawn as: about one vertex per dot.
        let mut p = Path::new();
        p.move_to((2.0, 12.0)).quad_to((14.0, -4.0), (24.0, 12.0));
        p.move_to((28.0, 4.0)).arc_to(38.0, 8.0, 8.0, 6.0, PI, 2.0 * PI);
        p.subpaths(|pts, _closed| {
            for (i, q) in pts.iter().enumerate() {
                c.disc(q.0, q.1, 0.8, if i % 2 == 0 { CYAN } else { BLUE });
            }
        });
    }
    // ----- mask, the rest --------------------------------------------------------
    "Silhouette", 24 x 4 => |c, t| {
        // Anything that covers dots: a `Mask`, or a `Canvas` (its dots and its
        // printed cells) serve equally as the shape to paint through.
        let mut mask = Mask::new(24, 4);
        mask.draw(|c| c.disc(10.0, 8.0, 7.0, t.ink));
        let mut canvas = Canvas::new(24, 4);
        canvas.print(10, 1, "TEXT", t.ink);
        canvas.fill_rect(34.0, 2.0, 12.0, 12.0, t.ink);
        c.stencil(&mask, ORANGE);
        c.stencil(&canvas, BLUE);
    }
    "Mask::new" | "Mask::set" | "Mask::unset" | "Mask::contains", 24 x 4 => |c, t| {
        // Dot by dot: a mask is a set of dots, written and read like a canvas
        // without colours.
        let mut m = Mask::new(24, 4);
        for y in 0..16 {
            for x in 0..48 {
                if (x / 4 + y / 4) % 2 == 0 {
                    m.set(x, y);
                }
            }
        }
        for x in 0..48 {
            m.unset(x, 8);
        }
        for y in 0..16 {
            for x in 0..48 {
                c.set(x, y, if m.contains(x, y) { CYAN } else { t.panel });
            }
        }
    }
    "Mask::of", 24 x 4 => |c, t| {
        // The silhouette of a canvas, set dots and printed cells alike, as a mask.
        let mut drawing = Canvas::new(24, 4);
        drawing.disc(9.0, 8.0, 7.0, Paint::linear((2.0, 0.0), (16.0, 0.0), RED, YELLOW));
        drawing.print(10, 1, "text", t.ink);
        let m = Mask::of(&drawing);
        c.stencil(&m, GREEN);
        c.effects(&m, &[Effect::outline(1.0).paint(t.ink)]);
    }
    "Mask::cols" | "Mask::rows" | "Mask::width" | "Mask::height" | "Mask::len" | "Mask::is_empty" | "Mask::clear", 28 x 4 => |c, t| {
        let mut m = Mask::new(28, 4); // 56 × 16 dots
        m.draw(|c| c.disc(8.0, 8.0, 7.0, t.ink));
        c.stencil(&m, BLUE);
        c.text(20, 1, &format!("{}/{}", m.len(), m.width() * m.height()), Font::tiny(), t.ink);
        m.clear();
        c.text(20, 9, if m.is_empty() { "cleared" } else { "not empty" }, Font::tiny(), t.panel);
    }
    "Mask::bounds" | "Mask::dots", 24 x 4 => |c, t| {
        let mut m = Mask::new(24, 4);
        m.draw(|c| c.fill_star(22.0, 8.0, 7.5, 3.0, 5, -PI / 2.0, t.ink));
        let (x0, y0, x1, y1) = m.bounds().unwrap(); // exact, exclusive on the far side
        c.rect(x0 as f32, y0 as f32, (x1 - x0) as f32, (y1 - y0) as f32, 1.0, t.panel);
        for (x, y) in m.dots() {
            // row-major
            c.set(x, y, YELLOW.lerp(RED, (y - y0) as f32 / (y1 - y0) as f32));
        }
    }
    "Mask::erase", 24 x 4 => |c, t| {
        let mut m = Mask::new(24, 4);
        m.draw(|c| c.fill_round_rect(2.0, 1.0, 44.0, 14.0, 4.0, t.ink));
        m.erase(|c| {
            for i in 0..5 {
                c.disc(8.0 + 8.0 * i as f32, 8.0, 3.0, t.ink); // any drawing, removed
            }
        });
        c.stencil(&m, PURPLE);
    }
    "Mask::translate" | "Mask::flip_y", 24 x 4 => |c, t| {
        let mut fish = Mask::new(24, 4);
        fish.draw(|c| {
            c.fill_ellipse(9.0, 5.0, 6.0, 3.0, t.ink);
            c.fill_polygon(&[(9.0, 5.0), (2.0, 1.0), (2.0, 9.0)], t.ink);
        });
        c.stencil(&fish, BLUE);
        fish.translate(16, 0); // moved right by whole dots
        c.stencil(&fish, CYAN);
        fish.translate(16, 0);
        fish.flip_y(8.0); // mirrored about the middle row
        c.stencil(&fish, GREEN);
    }
    // ----- transform, the rest ---------------------------------------------------
    "Transform::IDENTITY" | "Transform::is_identity" | "Transform::at" | "Transform::translate", 24 x 4 => |c, t| {
        let shape = |c: &mut Canvas| c.fill_ngon(0.0, 0.0, 6.0, 6, 0.0, PURPLE);
        c.with(Transform::IDENTITY, shape); // drawn where it says: around (0, 0)
        c.with(Transform::at(20.0, 8.0), shape); // the local origin moved to (20, 8)
        c.with(Transform::at(20.0, 8.0).translate(18.0, 0.0), shape); // moved, then placed
    }
    "Transform::scale" | "Transform::flip_x" | "Transform::flip_y", 24 x 4 => |c, t| {
        // A flag about the top of its pole: as drawn, stretched, mirrored, upside down.
        let flag = |c: &mut Canvas| {
            c.polyline(&[(0.0, 0.0), (0.0, 8.0)], 1.0, ORANGE);
            c.fill_rect(1.0, 0.0, 6.0, 4.0, YELLOW);
            c.fill_polygon(&[(7.0, 0.0), (10.0, 2.0), (7.0, 4.0)], RED);
        };
        c.with(Transform::at(2.0, 4.0), flag);
        c.with(Transform::at(14.0, 2.0).scale(1.0, 1.5), flag);
        c.with(Transform::at(36.0, 4.0).flip_x(), flag);
        c.with(Transform::at(38.0, 12.0).flip_y(), flag);
    }
    "Transform::rotate" | "Transform::rotate_about", 24 x 4 => |c, t| {
        // A hand pointing up from its pivot, turned about its own origin; then the
        // same hand turned about a point that is not its origin, so it swings.
        let hand = |c: &mut Canvas| c.fill_round_rect(-1.0, -7.0, 2.0, 8.0, 1.0, CYAN);
        for i in 0..5 {
            c.with(Transform::at(12.0, 9.0).rotate(i as f32 * 0.4), hand);
            c.with(Transform::at(36.0, 9.0).rotate_about(i as f32 * 0.4, (0.0, -7.0)), hand);
        }
        c.disc(12.0, 9.0, 1.2, t.ink);
        c.disc(36.0, 2.0, 1.2, t.ink);
    }
    "Transform::then" | "Transform::apply" | "Transform::inverse", 24 x 4 => |c, t| {
        let place = Transform::at(24.0, 8.0).rotate(0.5).scale(2.0, 1.0);
        c.with(place, |c| c.fill_rect(-6.0, -3.0, 12.0, 6.0, PURPLE));
        // Where the box's corners landed, from the same transform.
        for corner in [(-6.0, -3.0), (6.0, -3.0), (6.0, 3.0), (-6.0, 3.0)] {
            let (x, y) = place.apply(corner);
            c.disc(x, y, 1.2, YELLOW);
        }
        // `a.then(&b)` applies `a` first: here a nudge in local coordinates, then the placing.
        c.with(Transform::at(0.0, 2.0).then(&place), |c| c.rect(-6.0, -3.0, 12.0, 6.0, 1.0, GREEN));
        // The inverse maps back: the canvas's top-left corner in the box's own coordinates.
        let (u, v) = place.inverse().unwrap().apply((0.0, 0.0));
        c.text(1, 11, &format!("{u:.0},{v:.0}"), Font::tiny(), t.ink);
    }
    "Transform::is_axis_aligned" | "Transform::scale_factor", 24 x 4 => |c, t| {
        // A box stays a box under moves, scales and flips (axis-aligned, green); a
        // rotation makes it a path (orange). The scale factor is what a stroke's
        // width grows by.
        let placed = [Transform::at(8.0, 6.0), Transform::at(24.0, 6.0).scale(2.0, 1.0), Transform::at(40.0, 6.0).rotate(0.6)];
        for (i, tr) in placed.into_iter().enumerate() {
            let color = if tr.is_axis_aligned() { GREEN } else { ORANGE };
            c.with(tr, |c| c.rect(-4.0, -4.0, 8.0, 8.0, 1.0, color));
            c.text(5 + i as i32 * 16, 12, &format!("x{:.1}", tr.scale_factor()), Font::tiny(), t.ink);
        }
    }
    // ----- layer, the rest -------------------------------------------------------
    "Effect", 24 x 4 => |c, t| {
        // One shape, every effect: a shadow, an outline, a gap, a glow, a rim.
        let mut layers = Layers::new(24, 4);
        layers[0].fill_rect(0.0, 0.0, 48.0, 16.0, Paint::pattern(t.panel, Pattern::Cross(4)));
        let effects =
            [Effect::shadow(2, 2), Effect::outline(1.0).paint(t.ink), Effect::gap(2.0), Effect::glow(4.0).paint(CYAN), Effect::rim(2.0)];
        for (i, effect) in effects.into_iter().enumerate() {
            let shape = layers.push();
            shape.disc(5.0 + i as f32 * 9.5, 8.0, 3.5, CYAN);
            shape.effect(effect);
        }
        *c = layers.flatten().clone();
    }
    "Effect::paint", 24 x 4 => |c, t| {
        // The same shadow in the default grey, in a colour, and dithered thin.
        let mut layers = Layers::new(24, 4);
        layers[0].fill_rect(0.0, 0.0, 48.0, 16.0, Paint::dithered(t.panel, 0.7));
        let shadows = [Effect::shadow(2, 2), Effect::shadow(2, 2).paint(PURPLE), Effect::shadow(2, 2).paint(Paint::dithered(RED, 0.3))];
        for (i, shadow) in shadows.into_iter().enumerate() {
            let card = layers.push();
            card.fill_round_rect(2.0 + i as f32 * 16.0, 2.0, 11.0, 10.0, 2.0, YELLOW);
            card.effect(shadow);
        }
        *c = layers.flatten().clone();
    }
    "Sample" | "Sample::inside" | "Sample::lit" | "Sample::covered" | "Sample::layer", 24 x 4 => |c, t| {
        let mut layers = Layers::new(24, 4);
        layers[0].fill_rect(0.0, 0.0, 48.0, 16.0, Paint::dithered(t.panel, 0.7));
        let ball = layers.push();
        ball.disc(14.0, 8.0, 6.5, ORANGE);
        ball.disc(32.0, 8.0, 6.5, CYAN);
        // Inside: shaded by how much the edge faces the light. Outside: a shadow
        // wherever the layer's own dots are three right and two up, in their colour.
        ball.effect(Effect::shader(3.0, 3.0, |s| {
            if s.inside() {
                return s.color.map(|c| Paint::new(c.resolve(&Palette::default()).dim(0.6 + 0.4 * s.lit((-1.0, -1.0)))));
            }
            let (x, y) = (s.x - 3, s.y - 2);
            let shade = s.layer()?.get(x, y)?.resolve(&Palette::default()).dim(0.5);
            s.covered(x, y).then(|| Paint::dithered(shade, 0.6))
        }));
        *c = layers.flatten().clone();
    }
    "Layer" | "Layer::effect" | "Layer::canvas" | "Layer::canvas_mut", 24 x 4 => |c, t| {
        let mut layers = Layers::new(24, 4);
        layers[0].fill_rect(0.0, 0.0, 48.0, 16.0, Paint::dithered(t.panel, 0.7));
        let card = layers.push();
        card.canvas_mut().fill_round_rect(4.0, 3.0, 26.0, 10.0, 3.0, GREEN); // the same as drawing on `card`
        card.effect(Effect::gap(1.0)).effect(Effect::outline(1.0).paint(t.ink));
        let hidden = layers.push();
        hidden.disc(40.0, 8.0, 6.0, RED);
        hidden.visible = false; // skipped when flattening
        c.blit(layers.flatten(), 0, 0);
        let lit = layers[2].canvas().cells().filter(|cell| cell.bits != 0).count(); // read, not drawn
        c.text(34, 5, &format!("{lit}"), Font::tiny(), t.ink);
    }
    "Layers::new" | "Layers::push" | "Layers::insert" | "Layers::len", 24 x 4 => |c, t| {
        // Push goes on top; insert at 0 goes underneath everything.
        let mut layers = Layers::new(24, 4); // one empty layer, index 0
        layers[0].disc(18.0, 8.0, 7.0, BLUE);
        layers.push().disc(26.0, 8.0, 7.0, RED); // on top
        layers.insert(0).disc(34.0, 8.0, 7.0, YELLOW); // at the bottom
        c.blit(layers.flatten(), 0, 0);
        c.text(1, 1, &format!("{}", layers.len()), Font::tiny(), t.ink);
    }
    "Layers::remove" | "Layers::swap", 24 x 4 => |c, t| {
        let mut layers = Layers::new(24, 4);
        layers[0].disc(8.0, 8.0, 5.0, BLUE);
        layers.push().disc(12.0, 8.0, 5.0, RED);
        layers.push().disc(16.0, 8.0, 5.0, YELLOW);
        layers.swap(0, 2); // blue in front of yellow
        c.blit(layers.flatten(), 0, 0);
        layers.remove(1); // no more red
        c.blit(layers.flatten(), 24, 0);
    }
    "Layers::get" | "Layers::get_mut" | "Layers::iter" | "Layers::iter_mut" | "Layers::is_empty", 24 x 4 => |c, t| {
        let mut layers = Layers::new(24, 4);
        for (i, color) in [RED, YELLOW, GREEN, CYAN].into_iter().enumerate() {
            layers.push().disc(8.0 + i as f32 * 10.0, 10.0, 4.5, color);
        }
        for layer in layers.iter_mut() {
            layer.effect(Effect::outline(1.0).paint(t.ink));
        }
        if let Some(third) = layers.get_mut(3) {
            third.visible = false;
        }
        let shown = layers.iter().filter(|l| l.visible).count();
        c.blit(layers.flatten(), 0, 0);
        c.text(1, 0, &format!("{shown} of {} shown", layers.len()), Font::tiny(), t.ink);
    }
    "Layers::flatten" | "Layers::flat" | "Layers::clear" | "Layers::cols" | "Layers::rows" | "Layers::width" | "Layers::height", 24 x 4 => |c, t| {
        let mut layers = Layers::new(24, 4);
        let (w, h) = (layers.width() as f32, layers.height() as f32); // 48 × 16 dots
        layers[0].fill_rect(0.0, 0.0, w / 2.0, h, Paint::dithered(t.panel, 0.5));
        layers.push().disc(12.0, 8.0, 6.0, GREEN);
        layers.flatten(); // composited once; `flat` is that result until something changes
        c.blit(layers.flat(), 0, 0);
        layers.clear(); // every layer emptied, the stack and its effects kept
        layers[1].disc(12.0, 8.0, 6.0, RED);
        c.blit(layers.flatten(), 24, 0);
    }
    "Field::effects_in", 24 x 4 => |c, t| {
        // A kept scratch, running a contact shadow for several figures against one
        // ground: no allocation after the first.
        let mut ground = Mask::new(24, 4);
        ground.draw(|c| c.fill_rect(0.0, 13.0, 48.0, 3.0, t.ink));
        c.stencil(&ground, t.panel);
        let mut field = Field::new();
        let mut figure = Mask::new(24, 4);
        for (i, color) in [RED, YELLOW, GREEN].into_iter().enumerate() {
            figure.clear();
            figure.draw(|c| c.fill_ellipse(8.0 + 16.0 * i as f32, 7.0, 6.0, 6.5, t.ink));
            c.stencil(&figure, color);
            field.effects_in(c, &figure, &ground, &[Effect::shader(4.0, 0.0, |s| match s.color {
                Some(Color::Rgb(c)) => Some(Paint::new(c.dim(0.45 + 0.5 * s.dist / 4.0))),
                _ => None,
            })]);
        }
    }
    // ----- bubble, the rest ------------------------------------------------------
    "Bubble", 40 x 8 => |c, t| {
        // Two speakers; each bubble is placed by `speak` where it fits, the second
        // told to keep off the first.
        c.disc(10.0, 26.0, 4.0, GREEN);
        c.disc(70.0, 26.0, 4.0, PURPLE);
        let first = Bubble::speech("Hi there!").ink(t.bg).fill(GREEN).speak(c, (10.0, 22.0), &[]);
        Bubble::thought("Who?").ink(t.ink).border(1.0, t.ink).speak(c, (70.0, 22.0), &[first]);
    }
    "Side" | "Tail" | "Tail::new" | "Tail::len" | "Tail::width" | "Bubble::tail", 50 x 8 => |c, t| {
        // A tail from each side, at a point along it, of a length and a base width.
        let tails = [
            Tail::new(Side::Top, 0.2, TailKind::Point).len(6.0),
            Tail::new(Side::Right, 0.5, TailKind::Point).len(8.0).width(8.0),
            Tail::new(Side::Bottom, 0.8, TailKind::Curve).len(8.0),
            Tail::new(Side::Left, 0.5, TailKind::Line).len(10.0),
        ];
        for (i, tail) in tails.into_iter().enumerate() {
            Bubble::new("side").ink(t.ink).border(1.0, t.ink).tail(tail).draw(c, 12.0 + i as f32 * 22.0, 10.0);
        }
    }
    "Bubble::no_tail", 28 x 5 => |c, t| {
        let b = Bubble::speech("same").ink(t.bg).fill(BLUE);
        b.draw(c, 4.0, 2.0);
        b.no_tail().draw(c, 32.0, 2.0); // a text box again
    }
    "Bubble::ink", 30 x 5 => |c, t| {
        // Any text style; the background defaults to the fill.
        Bubble::new("bold on red").ink(TextStyle::new(t.bg).on(RED).bold()).draw(c, 2.0, 4.0);
        Bubble::new("plain").ink(BLUE).border(1.0, BLUE).draw(c, 34.0, 4.0);
    }
    "Bubble::font", 24 x 4 => |c, t| {
        // Dots instead of characters: a bubble small enough to go anywhere.
        Bubble::speech("tiny").font(Font::tiny()).ink(t.bg).fill(ORANGE).draw(c, 2.0, 1.0);
        Bubble::new("big").font(&Font::tiny().scale(2)).ink(t.ink).border(1.0, GREEN).draw(c, 26.0, 1.0);
    }
    "Bubble::pad", 28 x 5 => |c, t| {
        // Padding in cells around the text, none and plenty.
        Bubble::new("0,0").font(Font::tiny()).pad(0, 0).ink(t.ink).border(1.0, t.ink).draw(c, 2.0, 4.0);
        Bubble::new("3,1").font(Font::tiny()).pad(3, 1).ink(t.ink).border(1.0, t.ink).draw(c, 24.0, 4.0);
    }
    "Bubble::size" | "Bubble::bounds" | "Bubble::draw", 28 x 6 => |c, t| {
        let b = Bubble::speech("Drawn at (4, 4)").ink(t.bg).fill(PURPLE);
        let body = b.draw(c, 4.0, 4.0); // the body's box, a whole number of cells
        let all = b.bounds(4.0, 4.0); // body and tail: a keep-out zone for the next bubble
        c.rect(all.x, all.y, all.w, all.h, 1.0, Paint::dithered(t.ink, 0.5));
        let (w, h) = b.size(); // the same as the body's, known before drawing
        c.text(42, 18, &format!("{w}x{h}"), Font::tiny(), t.ink);
        c.disc(body.right(), body.y, 1.2, YELLOW);
    }
    "Bubble::place", 40 x 8 => |c, t| {
        // `place` decides, `draw` draws: the tail is aimed and the position chosen
        // within an area, here the left half of the canvas.
        let area = Rect::new(0.0, 0.0, 40.0, 32.0);
        c.rect(area.x, area.y, area.w, area.h, 1.0, Paint::dithered(t.panel, 0.7));
        c.disc(30.0, 26.0, 3.0, GREEN);
        let (placed, at) = Bubble::speech("Placed").ink(t.bg).fill(BLUE).place(area, (30.0, 24.0), &[]);
        placed.draw(c, at.0, at.1);
    }
    // ----- font, the rest --------------------------------------------------------
    "Font" | "Font::parse" | "FontError", 44 x 4 => |c, t| {
        // A font is text: a keyword line or two and a bitmap per character.
        let font = Font::parse("spacing 1\n\n♥\n.#.#.\n#####\n.###.\n..#..\n\n→\n..#.\n####\n..#.\n").unwrap();
        c.text(2, 2, "♥→♥", &font.scale(2), RED);
        // A bad line is reported by number.
        let err = Font::parse("A\n#?#\n").unwrap_err();
        c.print(0, 3, &format!("line {}: {}", err.line, err.message), t.ink);
    }
    "Glyph" | "Glyph::dot" | "Glyph::row" | "Font::glyph", 24 x 4 => |c, t| {
        // A glyph read dot by dot and drawn three times larger; then row by row, as
        // bit masks, twice as large.
        let g = Font::tiny().glyph('R').unwrap();
        for y in 0..g.height {
            for x in 0..g.width {
                if g.dot(x, y) {
                    c.fill_rect(1.0 + x as f32 * 3.0, y as f32 * 3.0, 3.0, 3.0, GREEN);
                }
            }
            let bits = g.row(y);
            for x in 0..g.width {
                if bits >> x & 1 == 1 {
                    c.fill_rect(20.0 + x as f32 * 2.0, y as f32 * 2.0, 2.0, 2.0, CYAN);
                }
            }
        }
    }
    "Font::height" | "Font::spacing" | "Font::line_gap" | "Font::line_height" | "Font::with_spacing" | "Font::with_line_gap", 24 x 4 => |c, t| {
        let tight = Font::tiny(); // 5 high, 1 between glyphs, 0 between lines
        let loose = Font::tiny().clone().with_spacing(3).with_line_gap(3);
        c.text(1, 1, "ab\ncd", tight, t.ink);
        c.text(14, 1, "ab\ncd", &loose, BLUE);
        c.text(30, 1, &format!("{}+{}", loose.height(), loose.line_gap()), Font::tiny(), t.panel);
        c.text(30, 8, &format!("={}", loose.line_height()), Font::tiny(), t.panel);
    }
    "Font::advance" | "Font::measure", 24 x 4 => |c, t| {
        let font = Font::tiny();
        let (w, h) = font.measure("Hello"); // the box the text takes
        c.text(2, 3, "Hello", font, t.ink);
        c.rect(1.0, 2.0, w as f32 + 2.0, h as f32 + 2.0, 1.0, Paint::dithered(BLUE, 0.5));
        // Each character's advance: its width plus the spacing.
        let mut x = 2;
        for ch in "Hello".chars() {
            c.line(x, 11, x + font.advance(ch) - 2, 11, ORANGE);
            x += font.advance(ch);
        }
    }
    "MAX_GLYPH_WIDTH", 40 x 3 => |c, t| {
        // A glyph can be up to 64 dots wide: one character that is a whole ruler.
        let (ticks, rule) = ("#...".repeat(MAX_GLYPH_WIDTH / 4), "#".repeat(MAX_GLYPH_WIDTH));
        let mut font = Font::empty();
        font.add('=', &[&ticks, &ticks, &rule]);
        c.text(8, 4, "=", &font, YELLOW);
        c.text(8, 8, &format!("{MAX_GLYPH_WIDTH} dots"), Font::tiny(), t.ink);
    }
    // ----- text, the rest --------------------------------------------------------
    "Attrs" | "Attrs::NONE" | "Attrs::BOLD" | "Attrs::DIM" | "Attrs::ITALIC" | "Attrs::UNDERLINE" | "Attrs::REVERSE" | "Attrs::has" | "TextStyle::with", 32 x 3 => |c, t| {
        let all = [
            ("none", Attrs::NONE),
            ("bold", Attrs::BOLD),
            ("dim", Attrs::DIM),
            ("italic", Attrs::ITALIC),
            ("under", Attrs::UNDERLINE),
            ("rev", Attrs::REVERSE),
        ];
        let mut col = 1;
        for (name, attrs) in all {
            let ink = if attrs.has(Attrs::REVERSE) { RED } else { t.ink };
            col += c.print(col, 1, name, TextStyle::new(ink).with(attrs)) + 1;
        }
    }
    "TextCell" | "TextCell::CONTINUATION" | "TextCell::is_empty" | "TextCell::is_continuation" | "Canvas::text_cell", 12 x 2 => |c, t| {
        // A wide character takes two cells: its own and a continuation. Under each
        // cell, what `text_cell` reports about it: empty, a character, or the
        // right half of one.
        c.print(1, 0, "a日b", t.ink);
        for col in 0..6 {
            let color = match c.text_cell(col, 0) {
                None => t.panel,
                Some(cell) if cell.is_continuation() => ORANGE,
                Some(_) => GREEN,
            };
            c.fill_rect(col as f32 * 2.0, 5.0, 2.0, 3.0, color);
        }
    }
    "wrap" | "Wrap" | "width" | "measure" | "char_width", 24 x 5 => |c, t| {
        // Lines wrapped at spaces to a width in cells, each with a bar as wide as
        // `width` says it is: the 日本 counts double.
        let paragraph = "Lines wrap at spaces; 日本 is wide.";
        for (i, line) in text::wrap(paragraph, 14).enumerate() {
            c.print(1, i as i32, line, t.ink);
            c.fill_rect(32.0, i as f32 * 4.0 + 1.0, text::width(line) as f32, 2.0, BLUE);
        }
        let (w, lines) = text::measure(paragraph); // unwrapped
        c.text(32, 13, &format!("{w}x{lines}"), Font::tiny(), t.panel);
    }
    "Canvas::erase_text" | "Canvas::clear_text" | "Canvas::has_text", 24 x 3 => |c, t| {
        c.fill_rect(0.0, 0.0, 48.0, 12.0, Paint::pattern(t.panel, Pattern::Checker(2)));
        c.print(1, 1, "everything here goes", t.ink);
        c.clear_text();
        c.print(1, 0, "erase the middle", t.ink);
        c.erase_text(7, 0, 4); // the dots show again there
        c.print(1, 2, if c.has_text() { "has text" } else { "no text" }, t.ink);
    }
    // ----- color, the rest -------------------------------------------------------
    "Rgb" | "Rgb::new" | "Rgb::hex", 24 x 3 => |c, t| {
        // The same colour three ways.
        c.fill_rect(0.0, 0.0, 16.0, 12.0, Rgb::new(0x58, 0xa6, 0xff));
        c.fill_rect(16.0, 0.0, 16.0, 12.0, Rgb::hex(0x58a6ff));
        c.fill_rect(32.0, 0.0, 16.0, 12.0, 0x58a6ffu32); // a literal converts too
    }
    "Rgb::luminance" | "Rgb::contrast", 24 x 4 => |c, t| {
        // Ink chosen by the swatch's luminance, and under each its contrast ratio
        // against the page.
        for (i, color) in [Rgb::hex(0x0d1117), BLUE, YELLOW, Rgb::hex(0xffffff)].into_iter().enumerate() {
            let x = i as f32 * 12.0;
            c.fill_rect(x, 0.0, 12.0, 9.0, color);
            let ink = if color.luminance() > 0.4 { Rgb::hex(0x000000) } else { Rgb::hex(0xffffff) };
            c.text(x as i32 + 2, 2, "Aa", Font::tiny(), ink);
            c.text(x as i32 + 1, 10, &format!("{:.0}", color.contrast(t.bg)), Font::tiny(), t.ink);
        }
    }
    "Palette" | "Color::resolve" | "Palette::is_light" | "Palette::nearest_ansi", 24 x 4 => |c, t| {
        // The sixteen ANSI colours of the xterm palette, resolved to RGB; then a
        // shade of orange and the ANSI colour nearest to it.
        let palette = Palette::default();
        for i in 0..16u8 {
            c.fill_rect(i as f32 * 3.0, 0.0, 3.0, 6.0, Color::Indexed(i).resolve(&palette));
        }
        c.fill_rect(0.0, 8.0, 20.0, 8.0, ORANGE);
        c.fill_rect(20.0, 8.0, 20.0, 8.0, Color::Indexed(palette.nearest_ansi(ORANGE)));
        c.text(41, 10, if palette.is_light() { "light" } else { "dark" }, Font::tiny(), t.ink);
    }
    "Depth" | "Color::quantize" | "Depth::parse" | "Depth::from_env", 32 x 4 => |c, t| {
        // One gradient at every depth: what the text fallback quantises to on a
        // terminal with true colour, 256 colours, 16, or none.
        let palette = Palette::default();
        let depths = ["true", "256", "16", "mono"].map(|name| Depth::parse(name).unwrap());
        for (i, depth) in depths.into_iter().enumerate() {
            for y in 0..16 {
                for x in 0..15 {
                    let color = Color::Rgb(RED.lerp(BLUE, x as f32 / 14.0)).quantize(depth, &palette);
                    c.set(i as i32 * 16 + x, y, color);
                }
            }
        }
    }
    // ----- render ----------------------------------------------------------------
    "Renderer", 36 x 5 => |c, t| {
        // The same canvas three ways: as the image protocols draw it, every dot its
        // own colour; as the text protocol shows it in true colour, one colour per
        // cell (the dominant one); and as a 16-colour terminal shows it.
        let mut scene = Canvas::new(12, 5);
        scene.fill_rect(0.0, 13.0, 24.0, 7.0, Paint::linear((0.0, 13.0), (0.0, 20.0), GREEN, GREEN.dim(0.3)));
        scene.disc(9.0, 8.0, 6.0, Paint::radial((7.0, 6.0), 7.0, YELLOW, ORANGE));
        scene.polyline(&[(2.0, 12.0), (22.0, 2.0)], 1.0, RED);
        c.blit(&scene, 0, 0);
        c.blit(&scene.fallback(Depth::TrueColor, &Palette::default()), 24, 0);
        c.blit(&scene.fallback(Depth::Ansi16, &Palette::default()), 48, 0);
    }
    // ----- export ----------------------------------------------------------------
    "svg" | "png" | "Style" | "Style::scale", 30 x 4 => |c, t| {
        // Every picture in this reference is `export::svg` of a canvas like this one,
        // with a style carrying the page background.
        c.fill_round_rect(1.0, 1.0, 58.0, 14.0, 4.0, Paint::edge(BLUE, t.panel, 3.0));
        let style = export::Style { background: Some(t.bg), ..export::Style::default() }.scale(2);
        let png = export::png(c, &style); // the same picture as a PNG, twice the size
        c.text(4, 5, &format!("svg / png: {} bytes", png.len()), Font::tiny(), t.ink);
    }
    "resolve", 24 x 4 => |c, t| {
        // Palette dots baked to RGB through a palette of your own, for a file that
        // has no terminal to ask: the default palette on the left, ours on the right.
        let mut themed = Canvas::new(12, 4);
        for i in 0..8u8 {
            themed.fill_rect(i as f32 * 3.0, 0.0, 3.0, 16.0, Color::Indexed(i + 8));
        }
        let mut palette = Palette::default();
        let ours = [RED, ORANGE, YELLOW, GREEN, CYAN, BLUE, PURPLE, Rgb::hex(0xffffff)];
        palette.colors[8..16].copy_from_slice(&ours);
        c.blit(&themed, 0, 0);
        c.blit(&export::resolve(&themed, &palette), 24, 0);
    }
}
