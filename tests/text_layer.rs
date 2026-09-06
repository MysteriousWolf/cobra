//! The text layer and the bubbles built on it, through the public API only: what a
//! caller prints has to survive into every protocol, the exporters and `to_text`.

mod common;

use cobra::text::{self, Align};
use cobra::{
    Attrs, Bubble, Canvas, CellSize, Color, Depth, Palette, Placement, Protocol, Rect, Renderer, Rgb, Shape, Side,
    Tail, TailKind, Terminal, TextStyle, export,
};
use common::{Image, parse_kitty};

const CELL: CellSize = CellSize { width: 8, height: 16 };
const INK: Rgb = Rgb::hex(0xc9d1d9);

fn frame(protocol: Protocol, canvas: &Canvas) -> String {
    let mut r = Renderer::new(Terminal::new(protocol, CELL));
    String::from_utf8(r.encode(canvas, Placement::Flow).to_vec()).unwrap()
}

#[test]
fn printed_characters_survive_into_every_protocol() {
    let mut c = Canvas::new(6, 2);
    c.fill_rect(0.0, 0.0, 12.0, 8.0, Rgb::hex(0x203040));
    c.print(1, 1, "ok", TextStyle::new(INK));

    // Text protocol: the characters replace the braille of their own cells only.
    let text = frame(Protocol::Text, &c);
    assert!(text.contains("ok"), "{text:?}");
    assert_eq!(c.to_text().lines().nth(1).unwrap().chars().nth(1), Some('o'));
    assert_eq!(c.to_text().lines().next().unwrap(), "⣿⣿⣿⣿⣿⣿");

    // Image protocols: the cells are left transparent and the characters printed over.
    for protocol in [Protocol::Kitty, Protocol::Iterm2, Protocol::Sixel] {
        let f = frame(protocol, &c);
        assert!(f.contains("ok"), "{protocol:?} dropped the text");
    }
    let image: Image = parse_kitty(frame(Protocol::Kitty, &c).as_bytes()).image();
    // The centre of a cell falls between dots; a slot centre is where a dot lands.
    let cell = |col: usize, row: usize| image.at(col * 8 + 2, row * 16 + 2);
    assert_eq!(cell(0, 0)[3], 255, "a cell of dots is drawn");
    assert_eq!(cell(1, 1)[3], 0, "the cell holding 'o' is left for the terminal");
    assert_eq!(cell(2, 1)[3], 0, "and the one holding 'k'");
    assert_eq!(cell(3, 1)[3], 255, "the cell after the text is not");
}

#[test]
fn a_style_becomes_the_sgr_a_terminal_expects() {
    let mut c = Canvas::new(4, 1);
    c.print(0, 0, "x", TextStyle::new(Color::Indexed(4)).on(Color::Indexed(7)).bold().underline());
    let f = frame(Protocol::Text, &c);
    assert!(f.contains("\x1b[1m") && f.contains("\x1b[4m"), "{f:?}");
    assert!(f.contains("\x1b[34m") && f.contains("\x1b[47m"), "{f:?}");
    // Quantisation applies to printed text as it does to dots.
    let mut r = Renderer::new(Terminal::text().with_depth(Depth::Ansi16));
    let mut c = Canvas::new(2, 1);
    c.print(0, 0, "x", TextStyle::new(Rgb::hex(0x00ff00)));
    let f = String::from_utf8(r.encode(&c, Placement::Flow).to_vec()).unwrap();
    assert!(f.contains("\x1b[92m"), "{f:?}");
}

#[test]
fn wide_characters_take_two_cells_everywhere() {
    let mut c = Canvas::new(4, 1);
    c.fill_rect(0.0, 0.0, 8.0, 4.0, INK);
    assert_eq!(c.print(0, 0, "字x", TextStyle::default()), 3);
    assert_eq!(c.to_text(), "字x⣿\n");
    let f = frame(Protocol::Text, &c);
    assert_eq!(f.matches('字').count(), 1);
    let image: Image = parse_kitty(frame(Protocol::Kitty, &c).as_bytes()).image();
    assert_eq!(image.at(12, 8)[3], 0, "both halves of the wide character are cleared");
}

#[test]
fn text_measurement_and_wrapping_are_public() {
    assert_eq!(text::measure("ab\ncdef"), (4, 2));
    assert_eq!(text::char_width('字'), 2);
    assert_eq!(text::wrap("one two three", 7).collect::<Vec<_>>(), ["one two", "three"]);
    let mut c = Canvas::new(10, 3);
    assert_eq!(c.print_wrapped(0, 0, 7, "one two three", INK, Align::Right), (7, 2));
    assert_eq!(c.text_cell(2, 1).map(|t| t.ch), Some('t'), "the second line is pushed right");
    assert!(c.text_cell(0, 2).is_none());
    c.clear();
    assert!(!c.has_text(), "clearing the canvas clears the text too");
}

#[test]
fn exports_carry_the_text_layer() {
    let mut c = Canvas::new(4, 1);
    c.print(0, 0, "Hi", TextStyle::new(Rgb::hex(0xff0055)).on(Rgb::hex(0x102030)).bold());
    let svg = export::svg(&c, &export::Style::default());
    assert!(svg.contains(">H</text>") && svg.contains(">i</text>"), "{svg}");
    assert!(svg.contains(r##"fill="#ff0055""##) && svg.contains(r##"fill="#102030""##));
    assert!(svg.contains(r#"font-weight="bold""#));
    let plain = export::svg(&c, &export::Style { text: false, ..export::Style::default() });
    assert!(!plain.contains("<text"));
    // PNG has no terminal font, so it approximates with the built-in one.
    let image = common::decode_png(&export::png(&c, &export::Style::default()));
    assert_eq!(image.at(2, 10), [0xff, 0x00, 0x55, 255], "a glyph dot");
    assert_eq!(image.at(9, 19), [0x10, 0x20, 0x30, 255], "on its background");
}

#[test]
fn a_bubble_is_a_box_with_text_in_it() {
    let mut c = Canvas::new(20, 4);
    let body = Bubble::new("hello there")
        .wrap(6)
        .align(Align::Center)
        .fill(Rgb::hex(0x161b22))
        .border(1.0, INK)
        .ink(INK)
        .draw(&mut c, 2.0, 0.0);
    assert_eq!(body, Rect::new(2.0, 0.0, 14.0, 16.0), "5 cells of text plus padding, 2 lines plus border rows");
    assert!(c.get(2, 0).is_some() && c.get(3, 0).is_some(), "the border is drawn");
    assert_eq!(c.text_cell(2, 1).map(|t| t.ch), Some('h'));
    assert_eq!(c.text_cell(2, 2).map(|t| t.ch), Some('t'), "the second line sits under the first");
    assert_eq!(c.text_cell(2, 1).unwrap().style.bg, Some(Color::Rgb(Rgb::hex(0x161b22))), "fill shows behind text");
    assert!(c.to_text().contains("hello") && c.to_text().contains("there"), "both lines are copyable text");
}

#[test]
fn tails_point_where_they_are_told() {
    let mut c = Canvas::new(24, 6);
    let tail = Tail::new(Side::Right, 0.5, TailKind::Point).len(8.0).width(6.0);
    let b = Bubble::new("hi").shape(Shape::Round(3.0)).tail(tail).fill(INK).ink(0u32);
    let body = b.draw(&mut c, 4.0, 4.0);
    assert!(c.get(body.right() as i32 + 3, body.center().1 as i32).is_some(), "the tail reaches out");
    assert_eq!(b.bounds(4.0, 4.0).right(), body.right() + 8.0, "bounds include the tail");
}

#[test]
fn speaking_dodges_the_zones_it_is_given() {
    let mut c = Canvas::new(40, 10);
    let face = Rect::new(30.0, 10.0, 20.0, 20.0);
    let bubble = Bubble::speech("look out").wrap(8).fill(Rgb::hex(0x161b22)).border(1.0, INK).ink(INK);
    let body = bubble.speak(&mut c, (36.0, 26.0), &[face]);
    assert_eq!(body.overlap(&face), 0.0, "the speaker is not covered");
    let canvas = Rect::new(0.0, 0.0, c.width() as f32, c.height() as f32);
    assert_eq!(body.overlap(&canvas), body.w * body.h, "and the bubble stays on the canvas");
    assert!(c.has_text() && c.to_text().contains("look"));
    // Every corner of the canvas gets a placement that fits.
    for mouth in [(1.0, 1.0), (78.0, 1.0), (1.0, 38.0), (78.0, 38.0), (40.0, 20.0)] {
        let body = bubble.speak(&mut c, mouth, &[]);
        assert_eq!(body.overlap(&canvas), body.w * body.h, "{mouth:?} pushed the bubble off the canvas");
    }
}

#[test]
fn a_bubble_can_draw_its_text_in_dots_instead() {
    let mut c = Canvas::new(20, 4);
    Bubble::new("hi").font(cobra::Font::tiny()).fill(Rgb::hex(0x161b22)).ink(INK).draw(&mut c, 0.0, 0.0);
    assert!(!c.has_text(), "nothing reaches the text layer");
    let dots = (0..c.width()).flat_map(|x| (0..c.height()).map(move |y| (x, y)));
    assert!(dots.clone().any(|(x, y)| c.get(x, y) == Some(Color::Rgb(INK))), "the glyphs are dots of the ink colour");
    let f = frame(Protocol::Text, &c);
    assert!(!f.contains("hi"));
}

#[test]
fn attributes_combine_and_report() {
    let s = TextStyle::new(INK).with(Attrs::BOLD | Attrs::REVERSE);
    assert!(s.attrs.has(Attrs::BOLD) && s.attrs.has(Attrs::REVERSE) && !s.attrs.has(Attrs::DIM));
    assert_eq!(TextStyle::default(), TextStyle { fg: None, bg: None, attrs: Attrs::NONE });
    let _: Palette = Palette::default();
}
