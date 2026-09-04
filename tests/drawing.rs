//! The drawing surface as a user meets it: coordinates, paints, primitives and text,
//! exercised through the public API only.

use cobra::{Canvas, Color, DOTS_X, DOTS_Y, Font, Paint, Point, Rgb, bayer, braille};

const WHITE: Rgb = Rgb::hex(0xffffff);

fn lit(c: &Canvas) -> usize {
    c.cells().map(|cell| cell.bits.count_ones() as usize).sum()
}

#[test]
fn a_canvas_is_cells_of_dots() {
    let c = Canvas::new(7, 3);
    assert_eq!((c.cols(), c.rows()), (7, 3));
    assert_eq!((c.width(), c.height()), (7 * i32::from(DOTS_X), 3 * i32::from(DOTS_Y)));
    assert_eq!(lit(&c), 0);
    assert_eq!(c.cells().count(), 21);
    assert_eq!(Canvas::new(0, 0).to_text(), "");
}

#[test]
fn clear_and_unset_free_dots_without_resizing() {
    let mut c = Canvas::new(2, 1);
    c.fill_rect(0.0, 0.0, 4.0, 4.0, WHITE);
    assert_eq!(lit(&c), 16);
    c.unset(0, 0);
    assert_eq!(c.get(0, 0), None);
    assert_eq!(lit(&c), 15);
    c.clear();
    assert_eq!(lit(&c), 0);
    assert_eq!((c.cols(), c.rows()), (2, 1));
}

#[test]
fn every_colour_kind_round_trips_through_a_dot() {
    let mut c = Canvas::new(2, 1);
    let colors = [Color::Rgb(Rgb::new(1, 2, 3)), Color::Indexed(0), Color::Indexed(255), Color::Foreground];
    for (x, color) in colors.iter().enumerate() {
        c.set(x as i32, 0, *color);
        assert_eq!(c.get(x as i32, 0), Some(*color));
    }
    // The convenience conversions land on the same dot value.
    c.set(0, 1, 0x010203u32);
    c.set(1, 1, (1u8, 2u8, 3u8));
    c.set(2, 1, Rgb::new(1, 2, 3));
    assert_eq!(c.get(0, 1), c.get(1, 1));
    assert_eq!(c.get(1, 1), c.get(2, 1));
}

#[test]
fn drawing_off_canvas_is_clipped_not_wrapped() {
    let mut c = Canvas::new(2, 1);
    c.line(-50, -50, 50, 50, WHITE);
    // The diagonal crosses (0,0)..(3,3); nothing wrapped into a neighbouring row.
    assert!(c.get(0, 0).is_some() && c.get(3, 3).is_some());
    let mut e = Canvas::new(2, 1);
    e.fill_rect(-100.0, -100.0, 10.0, 10.0, WHITE);
    assert_eq!(lit(&e), 0, "a rect entirely off-canvas draws nothing");
    let mut f = Canvas::new(2, 1);
    f.disc(-1.0, -1.0, 3.0, WHITE);
    assert!(f.get(0, 0).is_some() && f.get(3, 3).is_none());
}

#[test]
fn braille_bits_and_glyphs_line_up() {
    assert_eq!(braille(0), '\u{2800}');
    assert_eq!(braille(0xff), '\u{28ff}');
    let mut c = Canvas::new(1, 1);
    for y in 0..i32::from(DOTS_Y) {
        for x in 0..i32::from(DOTS_X) {
            c.clear();
            c.set(x, y, WHITE);
            let cell = c.cell(0, 0);
            assert_eq!(cell.bits.count_ones(), 1, "dot ({x}, {y})");
            assert_eq!(cell.glyph(), braille(cell.bits));
        }
    }
    c.fill_rect(0.0, 0.0, 2.0, 4.0, WHITE);
    assert_eq!(c.cell(0, 0).glyph(), '⣿');
}

#[test]
fn bayer_spreads_coverage_evenly() {
    let mut seen: Vec<f32> = (0..4).flat_map(|y| (0..4).map(move |x| bayer(x, y))).collect();
    seen.sort_by(|a, b| a.partial_cmp(b).unwrap());
    seen.dedup();
    assert_eq!(seen.len(), 16, "the 4x4 matrix should have sixteen distinct levels");
    assert!(seen.iter().all(|v| (0.0..1.0).contains(v)));
    // The pattern tiles, so it is stable under a shift of four and under negatives.
    assert_eq!(bayer(1, 2), bayer(5, 6));
    assert_eq!(bayer(1, 2), bayer(-3, -2));
}

#[test]
fn dithered_coverage_is_proportional() {
    for (coverage, expect) in [(0.0, 0), (0.25, 16), (0.5, 32), (0.75, 48), (1.0, 64)] {
        let mut c = Canvas::new(4, 2);
        c.fill_rect(0.0, 0.0, 8.0, 8.0, Paint::dithered(WHITE, coverage));
        assert_eq!(lit(&c), expect, "coverage {coverage}");
    }
}

#[test]
fn erase_paint_cuts_holes() {
    let mut c = Canvas::new(4, 2);
    c.fill_rect(0.0, 0.0, 8.0, 8.0, WHITE);
    c.clear_disc(4.0, 4.0, 2.5);
    assert!(c.get(4, 4).is_none(), "the middle of the disc should be cleared");
    assert!(c.get(0, 0).is_some(), "the corner should survive");
    assert_eq!(Paint::erase().color(), None);
    assert_eq!(Paint::new(WHITE).color(), Some(Color::Rgb(WHITE)));
    assert_eq!(Paint::dithered(WHITE, 0.25).coverage(), 0.25);
    assert_eq!(Paint::new(WHITE).coverage(), 1.0);
}

#[test]
fn a_filled_shape_covers_its_own_outline() {
    let mut filled = Canvas::new(10, 5);
    filled.fill_ellipse(10.0, 10.0, 8.0, 6.0, WHITE);
    let mut outline = Canvas::new(10, 5);
    outline.ellipse(10.0, 10.0, 8.0, 6.0, 1.0, WHITE);
    for y in 0..outline.height() {
        for x in 0..outline.width() {
            if outline.get(x, y).is_some() {
                // Allow the stroke to sit one dot outside the fill's rounding.
                let near = (-1..=1).any(|dy| (-1..=1).any(|dx| filled.get(x + dx, y + dy).is_some()));
                assert!(near, "outline dot ({x}, {y}) is nowhere near the fill");
            }
        }
    }
    assert!(lit(&outline) < lit(&filled));
}

#[test]
fn degenerate_shapes_draw_nothing_instead_of_panicking() {
    let mut c = Canvas::new(4, 2);
    c.fill_ellipse(4.0, 4.0, 0.0, 5.0, WHITE);
    c.fill_ellipse(4.0, 4.0, -3.0, 5.0, WHITE);
    c.fill_polygon(&[(0.0, 0.0), (4.0, 4.0)], WHITE);
    c.fill_polygon(&[], WHITE);
    c.polyline(&[], 1.0, WHITE);
    c.bezier(&[(0.0, 0.0)], 1.0, WHITE);
    c.spline(&[(1.0, 1.0)], false, 1.0, WHITE);
    c.fill_rect(1.0, 1.0, 0.0, 0.0, WHITE);
    assert_eq!(lit(&c), 0);
}

#[test]
fn a_closed_spline_returns_to_its_start() {
    let square: [Point; 4] = [(4.0, 4.0), (16.0, 4.0), (16.0, 16.0), (4.0, 16.0)];
    let mut open = Canvas::new(10, 5);
    open.spline(&square, false, 1.0, WHITE);
    let mut closed = Canvas::new(10, 5);
    closed.spline(&square, true, 1.0, WHITE);
    assert!(lit(&closed) > lit(&open), "closing the loop adds the last segment");
    for p in square {
        assert!(closed.get(p.0 as i32, p.1 as i32).is_some(), "the curve misses {p:?}");
    }
}

#[test]
fn an_arc_is_part_of_its_ellipse() {
    use std::f32::consts::{PI, TAU};
    let mut full = Canvas::new(10, 5);
    full.ellipse(10.0, 10.0, 8.0, 8.0, 1.0, WHITE);
    let mut half = Canvas::new(10, 5);
    half.arc(10.0, 10.0, 8.0, 8.0, 0.0, PI, 1.0, WHITE);
    assert!(lit(&half) * 2 >= lit(&full) - 8 && lit(&half) < lit(&full));
    let mut whole = Canvas::new(10, 5);
    whole.arc(10.0, 10.0, 8.0, 8.0, 0.0, TAU, 1.0, WHITE);
    assert_eq!(whole, full, "a full-turn arc is the ellipse");
}

#[test]
fn text_measures_what_it_draws() {
    let font = Font::tiny();
    let mut c = Canvas::new(20, 4);
    let size = c.text(1, 1, "Hello, world!", font, WHITE);
    assert_eq!(size, font.measure("Hello, world!"));
    assert!(size.0 > 0 && size.1 == i32::from(font.height()));
    assert!(lit(&c) > 0);
    // Newlines advance by a line height and reset x.
    let two = font.measure("ab\ncd");
    assert_eq!(two.1, i32::from(font.height()) * 2 + i32::from(font.line_gap()));
    assert_eq!(two.0, font.measure("ab").0);
}

#[test]
fn a_scaled_font_draws_the_same_shape_larger() {
    let big = Font::tiny().scale(2);
    assert_eq!(big.height(), Font::tiny().height() * 2);
    let small = Font::tiny().glyph('A').unwrap();
    let large = big.glyph('A').unwrap();
    assert_eq!((large.width, large.height), (small.width * 2, small.height * 2));
    for y in 0..small.height {
        for x in 0..small.width {
            assert_eq!(large.dot(x * 2, y * 2), small.dot(x, y), "({x}, {y})");
            assert_eq!(large.dot(x * 2 + 1, y * 2 + 1), small.dot(x, y));
        }
    }
}

#[test]
fn a_font_parsed_at_run_time_behaves_like_a_built_in_one() {
    let src = "height 3\nspacing 2\nline 1\n\nA\n###\n#.#\n###\n\nU+0042\n##\n#.\n";
    let font = Font::parse(src).expect("valid font");
    assert_eq!((font.height(), font.spacing(), font.line_gap()), (3, 2, 1));
    assert_eq!(font.len(), 2);
    assert_eq!(font.glyph('A').unwrap().row(1), 0b101);
    assert_eq!(font.glyph('B').unwrap().width, 2);
    assert_eq!(font.measure("AB"), (3 + 2 + 2, 3));
    let err = Font::parse("A\n#!#\n").unwrap_err();
    assert_eq!(err.line, 2);
    assert!(err.to_string().contains("font line 2"));
}

#[test]
fn to_text_is_a_rectangle_of_braille() {
    let mut c = Canvas::new(3, 2);
    c.set(0, 0, WHITE);
    c.set(5, 7, WHITE);
    let text = c.to_text();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines.iter().all(|l| l.chars().count() == 3));
    assert!(text.chars().all(|ch| ch == '\n' || ('\u{2800}'..='\u{28ff}').contains(&ch)));
    assert_eq!(lines[0].chars().next(), Some('⠁'));
    assert_eq!(lines[1].chars().nth(2), Some('⢀'));
}

#[test]
fn the_dominant_colour_of_a_cell_wins_the_text_fallback() {
    let mut c = Canvas::new(1, 1);
    let (major, minor) = (Rgb::hex(0x00ff00), Rgb::hex(0xff0000));
    c.set(0, 0, minor);
    for (x, y) in [(1, 0), (0, 1), (1, 1)] {
        c.set(x, y, major);
    }
    assert_eq!(c.cell(0, 0).color, Some(Color::Rgb(major)));
    assert_eq!(c.cell(0, 0).bits.count_ones(), 4);
}
