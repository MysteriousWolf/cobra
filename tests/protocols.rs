//! End-to-end checks that a frame carries the actual picture: every image protocol is
//! decoded back to pixels and compared against the canvas it was drawn from, and
//! against each other.

mod common;

use cobra::{Canvas, CellSize, Color, Depth, Options, Palette, Placement, Protocol, Renderer, Rgb, Terminal};
use common::{Image, parse_iterm2, parse_kitty, parse_sixel};

const CELL: CellSize = CellSize { width: 8, height: 16 };

/// A canvas using all three colour kinds, with dots in known slots.
fn sample() -> Canvas {
    let mut c = Canvas::new(3, 2);
    c.set(0, 0, Rgb::hex(0xff0000)); // cell (0,0), top-left dot
    c.set(1, 3, Rgb::hex(0x00ff00)); // cell (0,0), bottom-right dot
    c.set(5, 7, Color::Indexed(4)); // cell (2,1), bottom-right dot
    c.set(4, 4, Color::Foreground); // cell (2,1), top-left dot
    c
}

fn renderer(protocol: Protocol) -> Renderer {
    Renderer::with_options(Terminal::new(protocol, CELL), Options { dot_size: 1.0, copy_text: false })
}

/// Centre pixel of dot `(x, y)` for a canvas rendered on `CELL`.
fn dot_centre(x: i32, y: i32) -> (usize, usize) {
    let (slot_w, slot_h) = (CELL.width as usize / 2, CELL.height as usize / 4);
    (x as usize * slot_w + slot_w / 2, y as usize * slot_h + slot_h / 2)
}

fn assert_matches_canvas(image: &Image, canvas: &Canvas, palette: &Palette) {
    assert_eq!(image.width, canvas.cols() as usize * CELL.width as usize);
    assert_eq!(image.height, canvas.rows() as usize * CELL.height as usize);
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            let (px, py) = dot_centre(x, y);
            let got = image.at(px, py);
            match canvas.get(x, y) {
                Some(color) => {
                    let want = color.resolve(palette);
                    assert_eq!(got, [want.r, want.g, want.b, 255], "dot ({x}, {y}) at pixel ({px}, {py})");
                }
                None => assert_eq!(got[3], 0, "unset dot ({x}, {y}) is not transparent"),
            }
        }
    }
}

#[test]
fn kitty_payload_is_the_raster() {
    let canvas = sample();
    let mut r = renderer(Protocol::Kitty);
    let frame = parse_kitty(r.encode(&canvas, Placement::Flow));
    assert_eq!(frame.get("f"), Some("32"), "RGBA");
    assert_eq!(frame.get("o"), Some("z"), "zlib");
    assert_eq!(frame.get("a"), Some("T"), "transmit and display");
    assert_matches_canvas(&frame.image(), &canvas, &Palette::default());
}

#[test]
fn kitty_chunks_reassemble() {
    // A canvas large enough that the payload needs several 4096-byte chunks.
    let mut canvas = Canvas::new(60, 20);
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            canvas.set(x, y, Rgb::new((x * 3) as u8, (y * 5) as u8, (x ^ y) as u8));
        }
    }
    let mut r = renderer(Protocol::Kitty);
    let bytes = r.encode(&canvas, Placement::Flow).to_vec();
    assert!(bytes.windows(3).filter(|w| *w == b"\x1b_G").count() > 1, "expected a chunked frame");
    // `parse_kitty` asserts the m= flags and reassembles; decoding proves the split
    // happened on byte boundaries of the base64 stream.
    assert_matches_canvas(&parse_kitty(&bytes).image(), &canvas, &Palette::default());
}

#[test]
fn kitty_virtual_placement_declares_the_cell_span() {
    let canvas = sample();
    let mut r = renderer(Protocol::Kitty);
    let frame = parse_kitty(r.encode(&canvas, Placement::Virtual));
    assert_eq!(frame.get("U"), Some("1"));
    assert_eq!(frame.get("c"), Some("3"));
    assert_eq!(frame.get("r"), Some("2"));
    assert_matches_canvas(&frame.image(), &canvas, &Palette::default());
}

#[test]
fn iterm2_png_is_the_raster() {
    let canvas = sample();
    let mut r = renderer(Protocol::Iterm2);
    let image = parse_iterm2(r.encode(&canvas, Placement::Flow));
    assert_matches_canvas(&image, &canvas, &Palette::default());
}

#[test]
fn sixel_is_the_raster() {
    let canvas = sample();
    let mut r = renderer(Protocol::Sixel);
    let image = parse_sixel(r.encode(&canvas, Placement::Flow));
    assert_eq!((image.width, image.height), (24, 32));
    // Sixel carries colours as percentages of 100, so channels come back rounded.
    let palette = Palette::default();
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            let (px, py) = dot_centre(x, y);
            let got = image.at(px, py);
            match canvas.get(x, y) {
                Some(color) => {
                    let want = color.resolve(&palette);
                    assert_eq!(got[3], 255, "dot ({x}, {y}) is missing");
                    for (g, w) in got[..3].iter().zip([want.r, want.g, want.b]) {
                        assert!(g.abs_diff(w) <= 2, "dot ({x}, {y}): {got:?} vs {want:?}");
                    }
                }
                None => assert_eq!(got[3], 0, "unset dot ({x}, {y}) is not transparent"),
            }
        }
    }
}

#[test]
fn sixel_falls_back_to_quantising_past_255_colours() {
    // 12 x 4 cells = 24 x 16 dots = 384 distinct colours, well past the 255 the
    // palette holds, so the encoder must switch to its 3-3-2 bucket key.
    let mut canvas = Canvas::new(12, 4);
    let mut n = 0u32;
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            canvas.set(x, y, Rgb::new((n >> 3) as u8, (n * 7) as u8, (n * 13) as u8));
            n += 1;
        }
    }
    let mut r = renderer(Protocol::Sixel);
    let image = parse_sixel(r.encode(&canvas, Placement::Flow));
    assert!(image.colors().len() <= 255, "quantised frame still has {} colours", image.colors().len());
    assert!(image.opaque_count() > 0);
}

#[test]
fn every_protocol_draws_the_same_pixels() {
    let canvas = sample();
    let kitty = parse_kitty(renderer(Protocol::Kitty).encode(&canvas, Placement::Flow)).image();
    let iterm2 = parse_iterm2(renderer(Protocol::Iterm2).encode(&canvas, Placement::Flow));
    assert_eq!(kitty, iterm2, "kitty and iTerm2 disagree");
    let sixel = parse_sixel(renderer(Protocol::Sixel).encode(&canvas, Placement::Flow));
    assert_eq!((sixel.width, sixel.height), (kitty.width, kitty.height));
    assert_eq!(sixel.opaque_count(), kitty.opaque_count(), "sixel covers a different area");
}

#[test]
fn palette_dots_follow_the_terminal_theme() {
    let mut canvas = Canvas::new(1, 1);
    canvas.set(0, 0, Color::Indexed(1));
    canvas.set(1, 0, Color::Foreground);
    let mut palette = Palette::default();
    palette.colors[1] = Rgb::hex(0x1a2b3c);
    palette.foreground = Rgb::hex(0xfedcba);
    let term = Terminal::new(Protocol::Kitty, CELL).with_palette(palette);
    assert!(term.palette_queried);
    let mut r = Renderer::with_options(term, Options { dot_size: 1.0, copy_text: false });
    assert_matches_canvas(&parse_kitty(r.encode(&canvas, Placement::Flow)).image(), &canvas, &palette);
}

#[test]
fn a_view_renders_only_the_requested_cells() {
    let mut canvas = Canvas::new(4, 3);
    canvas.fill_rect(0.0, 0.0, canvas.width() as f32, canvas.height() as f32, Rgb::hex(0x00ff00));
    let mut r = renderer(Protocol::Kitty);
    let image = parse_kitty(r.encode_view(&canvas, Placement::Flow, 2, 1)).image();
    assert_eq!((image.width, image.height), (2 * CELL.width as usize, CELL.height as usize));
    // Clamped to the canvas rather than padded out.
    let image = parse_kitty(r.encode_view(&canvas, Placement::Flow, 99, 99)).image();
    assert_eq!((image.width, image.height), (4 * CELL.width as usize, 3 * CELL.height as usize));
    assert!(r.encode_view(&canvas, Placement::Flow, 0, 5).is_empty());
}

#[test]
fn dot_size_changes_coverage_but_not_geometry() {
    let mut canvas = Canvas::new(1, 1);
    canvas.set(0, 0, Rgb::hex(0xffffff));
    let coverage = |dot_size: f32| {
        let term = Terminal::new(Protocol::Kitty, CELL);
        let mut r = Renderer::with_options(term, Options { dot_size, copy_text: false });
        let image = parse_kitty(r.encode(&canvas, Placement::Flow)).image();
        assert_eq!((image.width, image.height), (8, 16));
        image.opaque_count()
    };
    let (small, large) = (coverage(0.4), coverage(1.0));
    assert!(small > 0 && small < large, "{small} vs {large}");
}

#[test]
fn copy_text_prepends_the_braille_glyphs() {
    let canvas = sample();
    let term = Terminal::new(Protocol::Kitty, CELL);
    let mut plain = Renderer::with_options(term, Options { dot_size: 1.0, copy_text: false });
    let plain = String::from_utf8_lossy(plain.encode(&canvas, Placement::Flow)).into_owned();
    let mut copyable = Renderer::with_options(term, Options { dot_size: 1.0, copy_text: true });
    let copyable = String::from_utf8_lossy(copyable.encode(&canvas, Placement::Flow)).into_owned();
    let braille = |s: &str| s.chars().filter(|c| ('\u{2800}'..='\u{28ff}').contains(c)).collect::<String>();
    assert_eq!(braille(&plain), "", "an image frame should carry no glyphs");
    assert_eq!(braille(&copyable), trimmed(&canvas));
}

/// The canvas as text, minus the trailing blanks of each row: a frame ends a row
/// with an erase instead of sending them.
fn trimmed(canvas: &Canvas) -> String {
    canvas.to_text().lines().map(|l| l.trim_end_matches('\u{2800}')).collect()
}

#[test]
fn text_protocol_survives_a_round_trip_through_braille() {
    let canvas = sample();
    let mut r = Renderer::new(Terminal::text());
    let frame = String::from_utf8(r.encode(&canvas, Placement::Flow).to_vec()).unwrap();
    let glyphs: String = frame.chars().filter(|c| ('\u{2800}'..='\u{28ff}').contains(c)).collect();
    assert_eq!(glyphs, trimmed(&canvas));
    assert!(frame.contains("\x1b[K"), "trailing blanks are one erase: {frame:?}");
}

#[test]
fn depth_never_emits_colours_the_terminal_lacks() {
    let mut canvas = Canvas::new(4, 1);
    for x in 0..canvas.width() {
        canvas.set(x, 0, Rgb::new((x * 30) as u8, 200 - (x * 20) as u8, (x * 7) as u8));
    }
    for depth in [Depth::Mono, Depth::Ansi16, Depth::Ansi256] {
        let mut r = Renderer::new(Terminal::text().with_depth(depth));
        let frame = String::from_utf8(r.encode(&canvas, Placement::Flow).to_vec()).unwrap();
        assert!(!frame.contains("38;2;"), "{depth:?} emitted a truecolor SGR: {frame:?}");
        if depth != Depth::Ansi256 {
            assert!(!frame.contains("38;5;"), "{depth:?} emitted a 256-colour SGR: {frame:?}");
        }
    }
}

#[test]
fn renderers_get_distinct_kitty_image_ids() {
    let term = Terminal::new(Protocol::Kitty, CELL);
    let ids: Vec<u32> = (0..4).map(|_| Renderer::new(term).image_id()).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "image ids collide: {ids:?}");
    assert!(ids.iter().all(|&id| id <= 0x00FF_FFFF), "image id is not 24-bit: {ids:?}");
}

#[test]
fn an_image_terminal_without_a_cell_size_is_demoted_to_text() {
    for protocol in [Protocol::Kitty, Protocol::Iterm2, Protocol::Sixel] {
        let term = Terminal::new(protocol, CellSize::default());
        assert_eq!(term.protocol, Protocol::Text);
        assert!(!term.is_graphical());
        let mut r = Renderer::new(term);
        let frame = r.encode(&sample(), Placement::Flow);
        assert!(common::find(frame, b"\x1b_G").is_none() && common::find(frame, b"\x1bP").is_none());
    }
}
