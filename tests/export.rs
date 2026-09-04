//! `export::png` and `export::svg`: the files are decoded back and checked against the
//! canvas, so the exported geometry has to match what the image protocols draw.

mod common;

use cobra::export::{self, Style};
use cobra::{Canvas, CellSize, Color, Palette, Rgb};
use common::decode_png;

fn sample() -> Canvas {
    let mut c = Canvas::new(2, 2);
    c.set(0, 0, Rgb::hex(0xff0000));
    c.set(3, 5, Color::Indexed(2));
    c.set(2, 7, Color::Foreground);
    c
}

fn style() -> Style {
    Style { cell: CellSize { width: 8, height: 16 }, dot_size: 1.0, ..Style::default() }
}

#[test]
fn png_defaults_are_documented_geometry() {
    let image = decode_png(&export::png(&sample(), &Style::default()));
    assert_eq!((image.width, image.height), (2 * 10, 2 * 20), "default cell is 10x20");
    assert_eq!(Style::default().dot_size, 0.7);
    assert!(Style::default().background.is_none());
}

#[test]
fn png_pixels_match_the_canvas() {
    let canvas = sample();
    let style = style();
    let image = decode_png(&export::png(&canvas, &style));
    assert_eq!((image.width, image.height), (16, 32));
    let (slot_w, slot_h) = (style.cell.width as usize / 2, style.cell.height as usize / 4);
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            let (px, py) = (x as usize * slot_w + slot_w / 2, y as usize * slot_h + slot_h / 2);
            match canvas.get(x, y) {
                Some(c) => {
                    let want = c.resolve(&style.palette);
                    assert_eq!(image.at(px, py), [want.r, want.g, want.b, 255], "dot ({x}, {y})");
                }
                None => assert_eq!(image.at(px, py)[3], 0, "dot ({x}, {y}) should be transparent"),
            }
        }
    }
}

#[test]
fn png_background_replaces_only_transparent_pixels() {
    let canvas = sample();
    let bg = Rgb::hex(0x101418);
    let opaque = decode_png(&export::png(&canvas, &Style { background: Some(bg), ..style() }));
    let transparent = decode_png(&export::png(&canvas, &style()));
    assert!(opaque.pixels.iter().all(|p| p[3] == 255), "background left holes");
    for (o, t) in opaque.pixels.iter().zip(&transparent.pixels) {
        let want = if t[3] == 0 { [bg.r, bg.g, bg.b, 255] } else { *t };
        assert_eq!(*o, want);
    }
}

#[test]
fn scale_multiplies_the_cell() {
    let canvas = sample();
    let one = decode_png(&export::png(&canvas, &style()));
    let three = decode_png(&export::png(&canvas, &style().scale(3)));
    assert_eq!((three.width, three.height), (one.width * 3, one.height * 3));
    assert_eq!(style().scale(1).cell, style().cell);
}

#[test]
fn png_of_an_empty_canvas_is_fully_transparent() {
    let image = decode_png(&export::png(&Canvas::new(3, 2), &style()));
    assert_eq!(image.opaque_count(), 0);
    assert_eq!(image.pixels.len(), 24 * 32);
}

#[test]
fn svg_has_one_group_per_colour_and_a_circle_per_dot() {
    let canvas = sample();
    let svg = export::svg(&canvas, &style());
    assert!(svg.starts_with("<svg") && svg.ends_with("</svg>\n"));
    assert!(svg.contains(r#"viewBox="0 0 16 32""#));
    assert_eq!(svg.matches("<circle").count(), 3, "one circle per set dot");
    assert_eq!(svg.matches("<g fill=").count(), 3, "three distinct colours");
    assert!(!svg.contains("<rect"), "no background was asked for");
    // Palette dots are resolved to concrete colours in the file.
    let fg = Palette::default().foreground;
    assert!(svg.contains(&format!("#{:02x}{:02x}{:02x}", fg.r, fg.g, fg.b)));
}

#[test]
fn svg_groups_are_ordered_deterministically() {
    let canvas = sample();
    let svg = export::svg(&canvas, &style());
    assert_eq!(svg, export::svg(&canvas, &style()), "the same canvas should export byte for byte");
    let keys: Vec<&str> = svg.split("<g fill=\"#").skip(1).map(|s| &s[..6]).collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(keys, sorted, "colour groups should be sorted by packed colour");
}

#[test]
fn svg_background_is_a_full_bleed_rect() {
    let svg = export::svg(&sample(), &Style { background: Some(Rgb::hex(0x1a1b26)), ..style() });
    assert!(svg.contains(r##"<rect width="100%" height="100%" fill="#1a1b26"/>"##));
}

#[test]
fn resolve_bakes_the_palette_into_rgb_dots() {
    let canvas = sample();
    let mut palette = Palette::default();
    palette.colors[2] = Rgb::hex(0x00ff88);
    palette.foreground = Rgb::hex(0xeeeeee);
    let baked = export::resolve(&canvas, &palette);
    assert_eq!(baked.cols(), canvas.cols());
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            assert_eq!(baked.get(x, y), canvas.get(x, y).map(|c| Color::Rgb(c.resolve(&palette))), "({x}, {y})");
        }
    }
    // Baked dots no longer depend on the palette they are drawn with.
    assert_eq!(export::png(&baked, &style()), export::png(&baked, &Style { palette, ..style() }));
}

#[test]
fn export_and_the_image_protocols_agree() {
    use cobra::{Options, Placement, Protocol, Renderer, Terminal};
    let canvas = sample();
    let cell = CellSize { width: 8, height: 16 };
    let mut r =
        Renderer::with_options(Terminal::new(Protocol::Iterm2, cell), Options { dot_size: 0.7, copy_text: false });
    let from_terminal = common::parse_iterm2(r.encode(&canvas, Placement::Flow));
    let from_file = decode_png(&export::png(&canvas, &Style { cell, dot_size: 0.7, ..Style::default() }));
    assert_eq!(from_terminal, from_file, "a file should look like the terminal does");
}
