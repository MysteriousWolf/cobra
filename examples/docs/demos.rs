//! One small drawing per documented function, so the reference shows what each
//! one does. The generator renders each into an SVG and quotes the block below it
//! as the example, so keep them self-contained and short.

use std::f32::consts::{PI, TAU};

use cobra::{
    Align, Attrs, Bubble, Canvas, Color, Effect, Field, Font, Layers, Mask, Paint, Path, Pattern, Pen, Rect, Rgb,
    Shape, Side, Tail, TailKind, TextStyle, Transform,
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

/// A drawing for one item of the reference.
pub struct Demo {
    /// The item it illustrates, as the parser names it (`Canvas::fill_rect`).
    pub item: &'static str,
    pub cols: u16,
    pub rows: u16,
    pub draw: fn(&mut Canvas, Theme),
}

macro_rules! demos {
    ($($item:literal, $cols:literal x $rows:literal => |$c:ident, $t:ident| $body:block)*) => {
        /// Every demo, in the order they are written here.
        pub fn all() -> Vec<Demo> {
            vec![$(Demo { item: $item, cols: $cols, rows: $rows, draw: |$c: &mut Canvas, $t: Theme| { let _ = &$t; $body } }),*]
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
    "Mask::subtract", 24 x 4 => |c, t| {
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
    "Field", 24 x 4 => |c, t| {
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
    "Shape", 44 x 6 => |c, t| {
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
    "Font::add", 24 x 3 => |c, t| {
        let mut font = Font::tiny().clone();
        font.add('♥', &[".#.#.", "#####", "#####", ".###.", "..#.."]);
        c.text(1, 1, "I ♥ dots", &font, RED);
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
    "TextStyle", 26 x 3 => |c, t| {
        c.print(1, 1, "bold", TextStyle::new(t.ink).bold());
        c.print(6, 1, "italic", TextStyle::new(BLUE).italic());
        c.print(13, 1, "under", TextStyle::new(GREEN).underline());
        c.print(19, 1, "on bg", TextStyle::new(t.bg).on(RED).with(Attrs::BOLD));
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
}
