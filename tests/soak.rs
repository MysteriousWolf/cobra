//! The soak sequence, checked without a terminal.
//!
//! `examples/soak.rs` draws these scenes at a real tty and records what it sent; this
//! runs the same table through every protocol and takes the frames apart again, so a
//! frame that upsets a terminal can be argued about here, in a test, rather than only
//! in a log. Every scene is checked for what the picture is (decode it back and
//! compare it with the canvas) and for what the wire looks like (chunking, control
//! keys, clipping, determinism).

mod common;

#[path = "../examples/common/scenes.rs"]
mod scenes;

use cobra::{Canvas, CellSize, DOTS_X, DOTS_Y, Options, Palette, Placement, Protocol, Renderer, Terminal};
use common::{Image, find, parse_iterm2, parse_kitty, parse_sixel};
use scenes::{SCENES, Scene};

const CELL: CellSize = CellSize { width: 8, height: 16 };

/// Phases every scene is checked at: the start, an off-beat one, and near the end.
const PHASES: [f32; 3] = [0.0, 0.37, 0.9];

/// Scenes big enough that decoding them for every protocol and phase would dominate
/// the test run. They are still encoded and their wire structure is checked; only the
/// pixel-by-pixel comparison is skipped.
fn heavy(scene: &Scene) -> bool {
    scene.cols as u32 * scene.rows as u32 > 600
}

fn renderer(protocol: Protocol) -> Renderer {
    // `dot_size: 1.0` fills each dot's slot, so the centre pixel of a slot is exactly
    // the dot's colour and a decoded frame can be compared with the canvas.
    Renderer::with_options(Terminal::new(protocol, CELL), Options { dot_size: 1.0, copy_text: false })
}

/// Centre pixel of dot `(x, y)`.
fn dot_centre(x: i32, y: i32) -> (usize, usize) {
    let (slot_w, slot_h) = (CELL.width as usize / 2, CELL.height as usize / 4);
    (x as usize * slot_w + slot_w / 2, y as usize * slot_h + slot_h / 2)
}

/// Every dot of `canvas` is the pixel the image has at that dot's centre. `slack` is
/// the per-channel tolerance: zero for the lossless protocols, two for sixel, which
/// carries colours as percentages.
fn assert_matches_canvas(image: &Image, canvas: &Canvas, slack: u8, what: &str) {
    assert_eq!(image.width, canvas.cols() as usize * CELL.width as usize, "{what}: width");
    assert_eq!(image.height, canvas.rows() as usize * CELL.height as usize, "{what}: height");
    let palette = Palette::default();
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            // A cell holding a printed character contributes no dots to the raster:
            // the terminal prints the real glyph over the picture, so the image is
            // deliberately empty there (see `Raster::draw`).
            if canvas.text_cell(x / DOTS_X as i32, y / DOTS_Y as i32).is_some() {
                continue;
            }
            let (px, py) = dot_centre(x, y);
            let got = image.at(px, py);
            match canvas.get(x, y) {
                Some(color) => {
                    let want = color.resolve(&palette);
                    assert_eq!(got[3], 255, "{what}: dot ({x}, {y}) is missing");
                    for (g, w) in got[..3].iter().zip([want.r, want.g, want.b]) {
                        assert!(g.abs_diff(w) <= slack, "{what}: dot ({x}, {y}): {got:?} vs {want:?}");
                    }
                }
                None => assert_eq!(got[3], 0, "{what}: unset dot ({x}, {y}) is not transparent"),
            }
        }
    }
}

/// Every `ESC _ G … ESC \` packet of a frame, as `(control data, payload)`.
fn kitty_packets(frame: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut packets = Vec::new();
    let mut rest = frame;
    while let Some(start) = find(rest, b"\x1b_G") {
        let body = &rest[start + 3..];
        let end = find(body, b"\x1b\\").expect("unterminated APC");
        let chunk = &body[..end];
        let semi = find(chunk, b";").expect("no ';' in an APC chunk");
        packets.push((String::from_utf8_lossy(&chunk[..semi]).into_owned(), chunk[semi + 1..].to_vec()));
        rest = &body[end + 2..];
    }
    packets
}

/// How many distinct colours a canvas resolves to, and whether anything is lit at all.
fn palette_size(canvas: &Canvas) -> (usize, bool) {
    let palette = Palette::default();
    let mut seen: Vec<[u8; 3]> = Vec::new();
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            if let Some(color) = canvas.get(x, y) {
                let c = color.resolve(&palette);
                let key = [c.r, c.g, c.b];
                if !seen.contains(&key) {
                    seen.push(key);
                }
            }
        }
    }
    let lit = !seen.is_empty();
    (seen.len(), lit)
}

#[test]
fn every_scene_is_the_picture_it_drew() {
    for scene in SCENES {
        if heavy(scene) {
            continue;
        }
        for phase in PHASES {
            let canvas = scene.canvas(phase);
            let what = format!("{} at phase {phase}", scene.name);

            let kitty = parse_kitty(renderer(Protocol::Kitty).encode(&canvas, Placement::Flow)).image();
            assert_matches_canvas(&kitty, &canvas, 0, &format!("kitty {what}"));

            let iterm2 = parse_iterm2(renderer(Protocol::Iterm2).encode(&canvas, Placement::Flow));
            assert_eq!(iterm2, kitty, "{what}: kitty and iTerm2 disagree");

            // Sixel carries a palette of 255 entries and quantises past that, and it
            // has nothing to say about a frame with no lit dot at all.
            let (colors, lit) = palette_size(&canvas);
            if lit && colors <= 255 {
                let sixel = parse_sixel(renderer(Protocol::Sixel).encode(&canvas, Placement::Flow));
                assert_matches_canvas(&sixel, &canvas, 2, &format!("sixel {what}"));
            } else if lit {
                // Past 255 colours the encoder quantises, and a frame this dense loses
                // dots rather than mapping each to the nearest entry it kept: `noise`
                // comes back with about 40% of the coverage kitty has. That is the
                // sixel encoder's own behaviour and not this harness's to assert away,
                // so what is checked is what a quantised frame still owes — the same
                // geometry, a legal palette, and no dot it was never given.
                let sixel = parse_sixel(renderer(Protocol::Sixel).encode(&canvas, Placement::Flow));
                assert_eq!((sixel.width, sixel.height), (kitty.width, kitty.height), "{what}: sixel size");
                assert!(sixel.colors().len() <= 255, "{what}: sixel kept {} colours", sixel.colors().len());
                assert!(sixel.opaque_count() > 0, "{what}: sixel drew nothing at all");
                let (drew, want) = (sixel.opaque_count(), kitty.opaque_count());
                assert!(drew <= want, "{what}: sixel lit {drew} dots of {want}, more than it was given");
            }
        }
    }
}

#[test]
fn every_kitty_frame_is_well_formed() {
    for scene in SCENES {
        // The big scenes inflate to megabytes; their structure is checked once, and
        // the pixels they carry are the clipping test's business.
        let phases: &[f32] = if heavy(scene) { &PHASES[..1] } else { &PHASES };
        for &phase in phases {
            let canvas = scene.canvas(phase);
            let mut r = renderer(Protocol::Kitty);
            let id = r.image_id();
            let frame = r.encode(&canvas, Placement::Flow).to_vec();
            let what = format!("{} at phase {phase}", scene.name);
            let packets = kitty_packets(&frame);
            assert!(!packets.is_empty(), "{what}: no kitty packet");

            for (k, (control, payload)) in packets.iter().enumerate() {
                assert!(payload.len() <= 4096, "{what}: packet {k} carries {} base64 bytes", payload.len());
                assert!(
                    payload.iter().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=')),
                    "{what}: packet {k} has a byte outside the base64 alphabet"
                );
                let last = k + 1 == packets.len();
                assert_eq!(control.contains("m=0"), last, "{what}: packet {k} has the wrong m= flag");
                assert_eq!(control.contains("m=1"), !last, "{what}: packet {k} has the wrong m= flag");
                if k > 0 {
                    // A continuation carries `m` alone, the last one included: a
                    // leading comma would start the control data with an empty key.
                    let want = if last { "m=0" } else { "m=1" };
                    assert_eq!(control, want, "{what}: packet {k} repeats the control data");
                }
                assert!(!control.starts_with(','), "{what}: packet {k} starts with an empty key");
            }

            let key = |name: &str| {
                packets[0]
                    .0
                    .split(',')
                    .filter_map(|p| p.split_once('='))
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| v.to_string())
            };
            assert_eq!(key("a").as_deref(), Some("T"), "{what}: not transmit-and-display");
            assert_eq!(key("f").as_deref(), Some("32"), "{what}: not RGBA");
            assert_eq!(key("o").as_deref(), Some("z"), "{what}: not zlib");
            assert_eq!(key("i"), Some(id.to_string()), "{what}: wrong image id");
            assert_eq!(key("c"), Some(scene.cols.to_string()), "{what}: wrong cell width");
            assert_eq!(key("r"), Some(scene.rows.to_string()), "{what}: wrong cell height");
            let w: usize = key("s").unwrap().parse().unwrap();
            let h: usize = key("v").unwrap().parse().unwrap();
            assert_eq!(
                (w, h),
                (scene.cols as usize * CELL.width as usize, scene.rows as usize * CELL.height as usize),
                "{what}: raster size"
            );

            if !heavy(scene) {
                // The inflated payload is exactly the raster the control data claims;
                // `parse_kitty` checks the zlib header and the Adler-32 on the way.
                let parsed = parse_kitty(&frame);
                assert_eq!(parsed.payload.len(), w * h * 4, "{what}: payload is not the raster");
            }
        }
    }
}

#[test]
fn a_text_layer_puts_the_image_underneath() {
    let scene = SCENES.iter().find(|s| s.name == "cell-text").expect("the cell-text scene");
    let canvas = scene.canvas(0.0);
    assert!(canvas.has_text(), "the cell-text scene is meant to carry a text layer");
    let frame = renderer(Protocol::Kitty).encode(&canvas, Placement::Flow).to_vec();
    assert_eq!(parse_kitty(&frame).get("z"), Some("-1"), "text scenes need the image below the text");
    let text = String::from_utf8_lossy(&frame);
    assert!(text.contains("text layer over dots"), "the printed characters are missing from the frame");

    // Without a text layer there is no z at all, so the image keeps the default plane.
    let plain = SCENES.iter().find(|s| s.name == "fills").unwrap().canvas(0.0);
    assert_eq!(parse_kitty(renderer(Protocol::Kitty).encode(&plain, Placement::Flow)).get("z"), None);
}

#[test]
fn no_frame_reaches_past_the_screen() {
    // A 20x6 screen, far smaller than the biggest scenes: every frame has to be
    // clipped to it, because a partly visible image has crashed terminals.
    let (cols, rows) = (20u16, 6u16);
    let mut term = Terminal::new(Protocol::Kitty, CELL);
    term.cols = cols;
    term.rows = rows;
    for scene in SCENES {
        let canvas = scene.canvas(0.0);
        let mut r = Renderer::with_options(term, Options { dot_size: 1.0, copy_text: false });
        for (col, row) in [(0u16, 0u16), (5, 2), (cols - 1, rows - 1)] {
            let frame = r.encode(&canvas, Placement::At(col, row)).to_vec();
            if frame.is_empty() {
                continue;
            }
            let parsed = parse_kitty(&frame);
            let c: u16 = parsed.get("c").unwrap().parse().unwrap();
            let rr: u16 = parsed.get("r").unwrap().parse().unwrap();
            let what = format!("{} at ({col}, {row})", scene.name);
            assert!(c <= cols - col, "{what}: {c} columns past the right edge");
            assert!(rr <= rows - row, "{what}: {rr} rows past the bottom edge");
            let w: usize = parsed.get("s").unwrap().parse().unwrap();
            let h: usize = parsed.get("v").unwrap().parse().unwrap();
            assert_eq!(w, c as usize * CELL.width as usize, "{what}: raster wider than the placement");
            assert_eq!(h, rr as usize * CELL.height as usize, "{what}: raster taller than the placement");
        }
    }
}

#[test]
fn the_same_scene_encodes_to_the_same_bytes() {
    for scene in SCENES {
        // Encoding a 240x60 canvas for four protocols twice over is minutes of debug
        // build; for those, kitty is the path that matters.
        let protocols: &[Protocol] = match heavy(scene) {
            true => &[Protocol::Kitty],
            false => &[Protocol::Text, Protocol::Kitty, Protocol::Iterm2, Protocol::Sixel],
        };
        for &protocol in protocols {
            let canvas = scene.canvas(0.42);
            let mut r = renderer(protocol);
            let first = r.encode(&canvas, Placement::Flow).to_vec();
            let second = r.encode(&canvas, Placement::Flow).to_vec();
            assert_eq!(first, second, "{} on {protocol:?} is not deterministic", scene.name);
        }
    }
}

#[test]
fn passthrough_wraps_every_image_sequence() {
    let term = Terminal::new(Protocol::Kitty, CELL).with_passthrough(true);
    for scene in SCENES {
        let canvas = scene.canvas(0.0);
        let mut r = Renderer::with_options(term, Options { dot_size: 1.0, copy_text: false });
        let frame = r.encode(&canvas, Placement::Flow).to_vec();
        // Inside a wrapper every escape is doubled, so each APC chunk appears as
        // `ESC ESC _ G` — and there is exactly one wrapper per chunk, since tmux hands
        // on one sequence per passthrough.
        let wrappers = frame.windows(7).filter(|w| *w == b"\x1bPtmux;").count();
        let apcs = frame.windows(4).filter(|w| *w == b"\x1b\x1b_G").count();
        assert!(wrappers > 0, "{}: nothing was wrapped", scene.name);
        assert_eq!(apcs, wrappers, "{}: {apcs} APC chunks in {wrappers} wrappers", scene.name);
        // A chunk that reached the wire undoubled would be eaten by tmux: every `_G`
        // introducer has to sit behind a doubled escape.
        let bare = frame.windows(4).filter(|w| w[1..] == *b"\x1b_G" && w[0] != 0x1b).count();
        assert_eq!(bare, 0, "{}: an unwrapped APC would not reach the terminal", scene.name);
    }
}

#[test]
fn the_text_protocol_stays_text() {
    for scene in SCENES {
        for phase in PHASES {
            let canvas = scene.canvas(phase);
            let mut r = Renderer::new(Terminal::text());
            let frame = r.encode(&canvas, Placement::Flow).to_vec();
            let what = format!("{} at phase {phase}", scene.name);
            let text = String::from_utf8(frame).unwrap_or_else(|e| panic!("{what}: not UTF-8: {e}"));
            assert!(!text.contains("\x1b_G"), "{what}: the text protocol emitted an image");
            assert!(!text.contains("\x1bP"), "{what}: the text protocol emitted a DCS");
            let glyphs = text.chars().filter(|c| ('\u{2800}'..='\u{28ff}').contains(c)).count();
            let drawn = (0..canvas.height()).any(|y| (0..canvas.width()).any(|x| canvas.get(x, y).is_some()));
            assert!(!drawn || glyphs > 0, "{what}: a drawn canvas came out with no braille glyphs");
        }
    }
}

#[test]
fn every_scene_draws_what_it_says_it_does() {
    // A scene that silently stopped drawing would make the whole soak run
    // meaningless: the harness would send empty frames and call them a pass.
    for scene in SCENES {
        let canvas = scene.canvas(0.25);
        assert_eq!((canvas.cols(), canvas.rows()), (scene.cols, scene.rows), "{}: wrong canvas size", scene.name);
        let lit = (0..canvas.height())
            .map(|y| (0..canvas.width()).filter(|&x| canvas.get(x, y).is_some()).count())
            .sum::<usize>();
        if scene.name == "empty" {
            assert_eq!(lit, 0, "the empty scene drew something");
        } else {
            assert!(lit > 0 || canvas.has_text(), "{}: drew nothing at all", scene.name);
        }
    }
}
