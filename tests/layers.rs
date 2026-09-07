//! Layers through the public API: what a stack flattens to has to come out of every
//! protocol, the exporters and the text fallback the way the layer order says.

mod common;

use cobra::{CellSize, Color, Effect, Layers, Paint, Placement, Protocol, Renderer, Rgb, Terminal, TextStyle, export};
use common::{Image, parse_iterm2, parse_kitty, parse_sixel};

const CELL: CellSize = CellSize { width: 8, height: 16 };
const BACK: Rgb = Rgb::hex(0x203040);
const FRONT: Rgb = Rgb::hex(0xff3355);
const INK: Rgb = Rgb::hex(0x111111);

/// A background box with a smaller box in front of it, overlapping cell `(1, 0)`
/// by two dots against six.
fn two_boxes() -> Layers {
    let mut l = Layers::new(6, 2);
    l[0].fill_rect(0.0, 0.0, 8.0, 8.0, BACK);
    l.push().fill_rect(2.0, 2.0, 8.0, 4.0, FRONT);
    l
}

fn frame(protocol: Protocol, layers: &mut Layers) -> Vec<u8> {
    let mut r = Renderer::new(Terminal::new(protocol, CELL));
    r.encode(layers.flatten(), Placement::Flow).to_vec()
}

/// The pixel at the centre of dot `(x, y)` of a decoded frame.
fn dot(image: &Image, x: usize, y: usize) -> [u8; 4] {
    image.at(x * 4 + 2, y * 4 + 2)
}

/// Whether two pixels are the same colour to within what sixel's percentage palette
/// can express.
fn close(a: [u8; 4], b: [u8; 4]) -> bool {
    a.iter().zip(b).all(|(&p, q)| p.abs_diff(q) <= 3)
}

#[test]
fn the_front_layer_wins_in_every_protocol() {
    let mut l = two_boxes();
    let back = [BACK.r, BACK.g, BACK.b, 255];
    let front = [FRONT.r, FRONT.g, FRONT.b, 255];
    let kitty = parse_kitty(&frame(Protocol::Kitty, &mut l)).image();
    let iterm2 = parse_iterm2(&frame(Protocol::Iterm2, &mut l));
    let sixel = parse_sixel(&frame(Protocol::Sixel, &mut l));
    for (name, image) in [("kitty", &kitty), ("iterm2", &iterm2), ("sixel", &sixel)] {
        assert!(close(dot(image, 0, 0), back), "{name}: back where only the back is");
        assert!(close(dot(image, 3, 3), front), "{name}: front where both are");
        assert!(close(dot(image, 9, 3), front), "{name}: front past the back");
        assert_eq!(dot(image, 9, 0)[3], 0, "{name}: nothing where neither is");
    }
}

#[test]
fn the_text_fallback_colours_a_shared_cell_by_its_front_layer() {
    let mut l = two_boxes();
    let s = String::from_utf8(frame(Protocol::Text, &mut l)).unwrap();
    // Cell (1, 0) holds two front dots (bottom row) and six back ones; without layers
    // the six would win. Cell (0, 0) has only back dots.
    let first_line = s.lines().next().unwrap();
    assert!(first_line.starts_with("\x1b[38;2;32;48;64m⣿\x1b[38;2;255;51;85m⣿"), "{first_line:?}");
    let flat = l.flatten();
    assert_eq!(flat.cell(1, 0).color, Some(Color::Rgb(FRONT)));
    assert_eq!(flat.cell(0, 0).color, Some(Color::Rgb(BACK)));
    assert_eq!(flat.to_text(), "⣿⣿⣿⣿⣤⠀\n⣿⣿⣿⣿⠛⠀\n");
    // A plain canvas with the same dots still votes by count.
    let mut plain = flat.clone();
    plain.clear();
    plain.fill_rect(0.0, 0.0, 8.0, 8.0, BACK);
    plain.fill_rect(2.0, 2.0, 8.0, 4.0, FRONT);
    assert_eq!(plain.cell(1, 0).color, Some(Color::Rgb(BACK)));
}

#[test]
fn effects_show_up_in_frames_and_exports() {
    let mut l = Layers::new(8, 4);
    l[0].fill_rect(0.0, 0.0, 16.0, 16.0, Paint::dithered(BACK, 0.5));
    let card = l.push();
    card.fill_rect(4.0, 4.0, 6.0, 6.0, FRONT);
    card.effect(Effect::gap(1.0)).effect(Effect::shadow(2, 2).paint(INK));
    let image = parse_kitty(&frame(Protocol::Kitty, &mut l)).image();
    let ink = [INK.r, INK.g, INK.b, 255];
    assert_eq!(dot(&image, 11, 11), ink, "the shadow shows past the corner");
    assert_eq!(dot(&image, 7, 7), [FRONT.r, FRONT.g, FRONT.b, 255], "the card covers its own shadow");
    // The gap clears the dithered background next to the card; the shadow, which
    // the same layer paints after it, fills the gap again where it falls.
    for y in 4..10 {
        assert_eq!(dot(&image, 3, y)[3], 0, "gap at 3,{y}");
    }
    assert_eq!(dot(&image, 10, 5)[3], 0, "gap above where the shadow starts");
    assert_eq!(dot(&image, 10, 8), ink, "the shadow lands in the gap");
    let flat = l.flatten();
    let svg = export::svg(flat, &export::Style::default());
    assert!(svg.contains(r##"<g fill="#111111">"##), "the shadow colour is in the SVG");
    let png = common::decode_png(&export::png(flat, &export::Style { cell: CELL, ..export::Style::default() }));
    assert_eq!(dot(&png, 11, 11), ink);
}

#[test]
fn printed_text_on_layers_reaches_the_terminal() {
    let mut l = Layers::new(6, 2);
    l[0].fill_rect(0.0, 0.0, 12.0, 8.0, BACK);
    l[0].print(0, 1, "under", TextStyle::new(INK));
    let label = l.push();
    label.print(1, 0, "hi", TextStyle::new(FRONT).on(INK));
    label.effect(Effect::gap(1.0));
    for protocol in [Protocol::Text, Protocol::Kitty, Protocol::Iterm2, Protocol::Sixel] {
        let s = String::from_utf8(frame(protocol, &mut l)).unwrap();
        assert!(s.contains("hi") && s.contains("under"), "{protocol:?}: {s:?}");
    }
    let flat = l.flatten();
    assert_eq!(flat.to_text(), "⡇hi⢸⣿⣿\nunder⣿\n", "one dot of gap either side of the label");
    assert_eq!(flat.text_cell(1, 0).unwrap().style.bg, Some(Color::Rgb(INK)));
    // The gap around the label's cells reaches into the row below and to the sides.
    assert_eq!(flat.get(6, 0), None, "right of `hi`");
    assert_eq!(flat.get(1, 0), None, "left of it");
    assert_eq!(flat.get(0, 0), Some(Color::Rgb(BACK)), "two dots away it stops");
}

#[test]
fn a_hidden_layer_and_a_custom_shader() {
    let mut l = Layers::new(4, 2);
    l[0].fill_rect(0.0, 0.0, 8.0, 8.0, BACK);
    let top = l.push();
    top.fill_rect(2.0, 2.0, 2.0, 2.0, FRONT);
    // Darken whatever the shifted silhouette lands on, painting nothing on its own.
    top.effect(Effect::shader(3.0, 0.0, |s| match s.color {
        Some(Color::Rgb(c)) if s.covered(s.x - 2, s.y - 2) => Some(Paint::new(c.dim(0.5))),
        _ => None,
    }));
    let flat = l.flatten();
    assert_eq!(flat.get(4, 4), Some(Color::Rgb(BACK.dim(0.5))));
    assert_eq!(flat.get(2, 2), Some(Color::Rgb(FRONT)), "inside the silhouette the shape stays");
    assert_eq!(flat.get(0, 0), Some(Color::Rgb(BACK)));
    l[1].visible = false;
    assert_eq!(l.flatten().get(4, 4), Some(Color::Rgb(BACK)));
    assert_eq!(l.flatten().get(2, 2), Some(Color::Rgb(BACK)));
}

#[cfg(feature = "ratatui")]
#[test]
fn the_ratatui_widget_takes_the_flattened_canvas() {
    use cobra::ratatui::Braille;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::widgets::Widget;
    let mut l = two_boxes();
    let r = Renderer::new(Terminal::text());
    let mut buf = Buffer::empty(Rect::new(0, 0, 6, 2));
    Braille::new(l.flatten(), &r).render(buf.area, &mut buf);
    assert_eq!(buf[(1, 0)].fg, ratatui::style::Color::Rgb(FRONT.r, FRONT.g, FRONT.b));
    assert_eq!(buf[(0, 0)].fg, ratatui::style::Color::Rgb(BACK.r, BACK.g, BACK.b));
}
