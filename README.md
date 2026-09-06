<p align="center"><img src="assets/logo.svg" alt="cobra" width="420"></p>

# cobra

[![CI](https://github.com/MysteriousWolf/cobra/actions/workflows/ci.yml/badge.svg)](https://github.com/MysteriousWolf/cobra/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/cobra.svg)](https://crates.io/crates/cobra)
[![docs.rs](https://docs.rs/cobra/badge.svg)](https://docs.rs/cobra)

Draw with individually coloured dots in the terminal.

A braille character packs a 2×4 grid of dots into one cell, which gives you a cheap
pixel grid four times taller and twice as wide as the text grid. The catch is that a
character can only have one colour, so every dot in a cell has to share it.

cobra keeps the braille model but drops that limit. You draw on a canvas where every
dot has its own colour — plus a layer of real characters where dots are too coarse — and
it picks the best way to show it:

* On kitty, WezTerm, Ghostty, iTerm2, foot, xterm and Windows Terminal it sends a small
  image aligned to the character grid, so each dot keeps its colour.
* Everywhere else, including tmux and pipes, it prints real braille glyphs with one
  colour per cell, quantised to however many colours the terminal has.

Same code either way. Nothing to configure.

```sh
cargo add cobra
```

```rust
use cobra::{Canvas, Renderer, Rgb, Terminal};

let mut renderer = Renderer::new(Terminal::detect());  // detect once, at start-up
let mut canvas = Canvas::new(20, 5);                   // 20×5 cells = 40×20 dots

for x in 0..canvas.width() {
    let t = x as f32 / canvas.width() as f32;
    canvas.set(x, 10, Rgb::hex(0xff0055).lerp(Rgb::hex(0x00ccff), t));
}

renderer.render(&canvas, &mut std::io::stdout())?;
```

`render` draws at the cursor and leaves it on the next line. `render_at(col, row)` draws
at an absolute position and puts the cursor back. `encode` returns the bytes if you want
to write them yourself.

## Shapes

<p align="center"><img src="assets/gallery.svg" alt="every primitive, one per panel" width="720"></p>

That image is `cargo run --example gallery`, one panel per primitive. Coordinates are
`f32` dots, `(0, 0)` is the top left, and anything off-canvas is clipped, so you can draw
partly outside without checking bounds.

```rust
use cobra::{Canvas, Font, Paint, Rgb};

let mut c = Canvas::new(72, 24);

// Dots and lines take integer coordinates.
c.set(4, 6, Rgb::hex(0xff3355));                          // one dot
c.line(0, 20, 30, 4, Rgb::hex(0xff8c1a));                 // Bresenham
c.disc(10.0, 12.0, 8.0, Rgb::hex(0xffd21e));              // filled circle

// Rectangles: fill, then a 2-dot border.
c.fill_rect(2.0, 2.0, 14.0, 14.0, Rgb::hex(0x5ec33a));
c.rect(20.0, 2.0, 14.0, 14.0, 2.0, Rgb::hex(0x5ec33a));

// Ellipses and arcs. `arc` takes start and end angles in radians.
c.fill_ellipse(8.0, 9.0, 8.0, 6.0, Rgb::hex(0x2ec4a6));
c.ellipse(24.0, 9.0, 8.0, 6.0, 1.0, Rgb::hex(0x2ec4a6));
c.arc(16.0, 20.0, 10.0, 10.0, 3.34, 5.34, 1.0, Rgb::hex(0x3aa0ff));

// Paths. Points are plain (f32, f32) tuples.
let pts = [(0.0, 8.0), (8.0, 0.0), (16.0, 12.0), (24.0, 4.0)];
c.polyline(&pts, 2.0, Rgb::hex(0x6c7bff));                // open
c.polygon(&pts, 1.0, Rgb::hex(0xa96cff));                 // closed
c.fill_polygon(&pts, Rgb::hex(0xa96cff));

// Curves. 3 control points is quadratic, 4 is cubic.
c.bezier(&[(0.0, 18.0), (16.0, 1.0), (32.0, 18.0)], 1.0, Rgb::hex(0xe45cc4));
c.spline(&pts, false, 2.0, Rgb::hex(0xff6f91));           // Catmull-Rom through every point

// Rounded boxes, regular polygons, stars, wedges, rings and arrows.
c.fill_round_rect(2.0, 2.0, 20.0, 12.0, 4.0, Rgb::hex(0x5ec33a));
c.round_rect(2.0, 2.0, 20.0, 12.0, 4.0, 2.0, Rgb::hex(0x5ec33a));  // 2-dot border
c.fill_ngon(10.0, 10.0, 8.0, 6, 0.0, Rgb::hex(0xffd21e));          // hexagon
c.fill_star(30.0, 10.0, 9.0, 4.0, 5, 0.0, Rgb::hex(0xffd21e));     // 5 spikes, notches at 4
c.fill_pie(10.0, 10.0, 8.0, 8.0, 0.0, 2.1, Rgb::hex(0x2ec4a6));    // wedge, radians
c.ring(30.0, 10.0, 8.0, 5.0, Rgb::hex(0x2ec4a6));                  // outer, inner
c.arrow((0.0, 8.0), (30.0, 8.0), 2.0, 6.0, Rgb::hex(0xff8c1a));    // tip lands on the point

// Text, in a built-in 3×5 font.
c.text(2, 2, "cobra", &Font::tiny().scale(2), Rgb::hex(0xc9d1d9));
```

`Rect { x, y, w, h }` is the box type the bubbles below use, with `center`, `contains`,
`inset`, `offset` and `overlap` on it.

### Paint

Every shape takes a `Paint`. A bare colour works, and two constructors cover the rest:

```rust
let teal = Rgb::hex(0x4fd1c5);

c.fill_rect(0.0, 0.0, 20.0, 8.0, teal);                             // solid
c.fill_rect(0.0, 0.0, 20.0, 8.0, Paint::dithered(teal, 0.35));      // 35% of the dots
c.fill_ellipse(10.0, 4.0, 6.0, 4.0, Paint::erase());                // unset dots
```

Dots are on or off, so `Paint::dithered` is how you get shading. It fills a fraction of
the dots in an ordered 4×4 Bayer pattern. Erasing first is the usual trick for making
something readable on top of a busy background: clear a margin, then draw.

Strokes narrower than one dot are Bresenham lines. Wider ones are a quad per segment
with round joins and caps. Everything fills through `span(y, x0, x1, paint)`, one clipped
horizontal run per dot row, which is public if you want to write your own primitive.
Only `fill_polygon` allocates, and only to sort edge crossings.

## Colours

A dot colour is an `Rgb`, one of the terminal's 256 palette entries, or the default
foreground:

```rust
use cobra::Color;

canvas.set(x, y, 0x5ec33a);            // anything Into<Color>
canvas.set(x, y, Color::Indexed(2));   // ANSI green, whatever the user's theme makes it
canvas.set(x, y, Color::Foreground);   // the text colour
```

Palette colours follow the user's theme. In the text fallback they become plain SGR
indices and the terminal resolves them. For the image protocols, detection asks the
terminal for its actual ANSI colours (`OSC 4`, `OSC 10`, `OSC 11`) and the renderer looks
each one up. If the terminal does not answer, xterm's defaults are used.

Not every terminal has 24-bit colour, so detection also works out the colour depth from
`NO_COLOR`, `COLORTERM`, `TERM` and the terminal program, and the text renderer quantises
to it:

| Depth | Cells become | Nearest colour picked by |
|---|---|---|
| `TrueColor` | `38;2;r;g;b` | nothing to do |
| `Ansi256` | `38;5;n` | closest cube corner vs. closest grey |
| `Ansi16` | `30-37` / `90-97` | the terminal's own ANSI palette |
| `Mono` | default foreground | everything |

Quantisation happens per dot, before the cell picks its dominant colour, so two shades
that land on the same palette entry vote together instead of splitting the cell. Set
`COBRA_COLORS=16` to see it without hunting for an old terminal.

## Text

Two kinds, because a dot grid and a character grid are different resolutions.

`canvas.text` draws with a bitmap font, so a label is dots like every other shape: any
size, any colour, anywhere. `canvas.print` puts a **real character** in a terminal cell —
whatever the user's font can draw, still copyable, still readable by a screen reader, and
sharp at any font size.

```rust
use cobra::{Attrs, Align, TextStyle, text};

canvas.print(2, 1, "Ready.", TextStyle::new(ink).bold());     // (col, row) in cells
canvas.print(2, 2, "字 ± λ ✓", ink);                          // anything the font has
canvas.print(2, 3, " selected ", TextStyle::new(bg).on(ink)); // colours and attributes
canvas.print_wrapped(2, 4, 20, paragraph, ink, Align::Center);

text::measure("two\nlines");        // (width, lines) in cells
text::wrap(paragraph, 20);          // an iterator of lines, allocating nothing
text::char_width('字');             // 2
```

A cell holding a character shows that character instead of its eight dots, in every
protocol: the text fallback prints it in place of the braille glyph, the image protocols
leave the cell transparent and print over the picture (on kitty the image is placed below
the text layer), the ratatui widget writes it into the buffer, `to_text()` includes it,
and both exporters draw it. The layer costs nothing until you print: it is not allocated
until the first character.

Since a character owns its whole cell, whatever was drawn under it is hidden — use
`TextStyle::on` (or a `Bubble`, which does it for you) to keep the background colour.

### Fonts

```rust
canvas.text(2, 2, "cobra", &Font::tiny().scale(2), ink);
canvas.text(2, 14, "3×5 dots", Font::tiny(), Paint::dithered(ink, 0.75));
let (w, h) = Font::tiny().measure("right aligned");
```

`Font::tiny()` is a proportional 3×5 font covering printable ASCII. `scale(n)` builds an
`n`× copy once, so one master gives you every size.

Fonts are a plain text format. Parse one at compile time with `include_str!`, at run time
with `Font::parse`, or build glyphs from code with `Font::add`:

```text
// comment
height 5      // line advance, defaults to the tallest glyph
spacing 1     // dots between glyphs
line 1        // dots between lines

A             // one character, or U+0041; a blank line ends the glyph
.#.           // '#' is a dot, '.' is not
#.#
###
#.#
#.#
```

Glyphs can be any size up to 64 dots wide, so one font can mix narrow punctuation with
wide capitals, or hold a handful of large symbols. Any script can generate one.

## Text boxes and speech bubbles

A `Bubble` is a body with text in it. Without a tail it is a text box; with one it is a
chat bubble.

```rust
use cobra::{Align, Bubble, Shape, Side, Tail, TailKind};

Bubble::new("Text boxes wrap, pad and align themselves.")
    .wrap(20)                          // cells
    .align(Align::Center)
    .fill(Rgb::hex(0x161b22))
    .border(1.0, Rgb::hex(0x6c7bff))
    .ink(Rgb::hex(0xc9d1d9))
    .draw(&mut canvas, 4.0, 4.0);      // returns the body's Rect
```

Four presets cover the usual voices, and every part of them can still be changed:

| | Body | Tail |
|---|---|---|
| `Bubble::speech` | rounded box | triangle |
| `Bubble::thought` | cloud of lobes | trail of discs |
| `Bubble::shout` | starburst | triangle |
| `Bubble::whisper` | rounded box | curling comic tail |
| `Bubble::new` | box | none |

`shape` takes `Rect`, `Round(radius)`, `Ellipse`, `Cloud` or `Burst`; `tail` takes a
`Tail`, which is a `Side`, a position `0..=1` along it, a length, a base width and one of
`TailKind::Point`, `Curve` or `Bubbles`:

```rust
Bubble::speech("psst")
    .tail(Tail::new(Side::Left, 0.7, TailKind::Curve).len(8.0).width(4.0))
    .clear_behind()                    // unset the dots underneath, for busy backgrounds
    .draw(&mut canvas, 4.0, 4.0);
```

The text is real text, so a bubble is copyable and stays sharp. `font(&Font::tiny())`
switches it to dots instead, for bubbles smaller than a character cell or drawings headed
for a file.

### Letting it choose a place

Give `speak` the mouth to point at and the rectangles to stay off, and it tries the bubble
on all four sides, pushes each candidate back onto the canvas, and draws the one that
covers the least of what you wanted kept clear — with the tail leaning over to reach the
mouth wherever it ends up:

```rust
let face = Rect::new(40.0, 20.0, 24.0, 20.0);
let body = Bubble::speech("Watch out!").wrap(10).fill(ink).speak(&mut canvas, mouth, &[face]);
```

`place(area, mouth, keep_out)` does the same without drawing, returning the bubble with
its tail aimed and the position to draw it at, so you can confine it to part of the canvas
or feed `bounds()` back in as the keep-out zone for the next one.

## Terminals

| Protocol | Terminals | What goes over the wire |
|---|---|---|
| Kitty  | kitty, WezTerm, Ghostty, Konsole ≥ 22.04 | zlib RGBA in chunked APC, one image id per canvas |
| iTerm2 | iTerm2, WezTerm, mintty, Konsole | PNG in OSC 1337 |
| Sixel  | foot, xterm, mlterm, Windows Terminal ≥ 1.22 | palettised DCS, transparent background |
| Text   | everything, tmux, pipes | braille glyphs, one colour per cell |

`Terminal::detect()` runs once. It reads the `COBRA_*` overrides, checks the usual
environment variables, then spends a single escape-sequence round trip on `/dev/tty` for
whatever is left: the kitty probe, `CSI 16 t` for cell size, `DA1` for sixel, and the
colour scheme. The round trip ends as soon as `DA1` comes back. Cell size comes from one
`ioctl` when the terminal fills in the pixel fields.

Frames are transparent apart from the dots, so they never paint over the background.

Terminals publish the pixel size of a cell but not the font, and no protocol exposes glyph
outlines. So the *position* of every dot is exact, while its *diameter* is a guess. The
default of 0.7 of a slot matches what most monospace fonts draw. To match yours exactly:

```sh
cargo run --example calibrate
```

It prints your font's braille above image dots at six sizes. Export `COBRA_DOT` to
whichever row lines up and every renderer picks it up. Kitty and Ghostty draw braille
themselves rather than from the font, so on those the two always agree.

## ratatui

```toml
cobra = { version = "26.1", features = ["ratatui"] }
```

```rust
use cobra::ratatui::{Braille, overlay};

let area = terminal.draw(|f| f.render_widget(Braille::new(&canvas, &renderer), f.area()))?.area;
overlay(&mut renderer, &canvas, area, terminal.backend_mut())?;
```

On kitty the widget writes Unicode placeholder cells that never change between frames, so
ratatui's diff leaves them alone and `overlay` only sends the compressed image. On iTerm2
and sixel, which cannot place an image in the buffer, the widget writes the text fallback
and `overlay` paints over it. On text terminals `overlay` does nothing. Cells you printed
into are written as those characters with their own style, whatever the protocol.

## Export

```rust
use cobra::export::{png, svg, Style};

std::fs::write("plot.png", png(&canvas, &Style::default().scale(2)))?;
std::fs::write("plot.svg", svg(&canvas, &Style::default()))?;
```

Both reuse the renderer's geometry, so a file looks like the terminal did. Printed text
comes along: SVG writes real `<text>` elements, and PNG, which has no terminal font,
approximates the characters with the built-in one (`Style::text` turns that off). The
background is transparent unless you set `Style::background`. The logo and the shape gallery at the
top of this file were made this way.

## Performance

`cargo bench` measures a whole frame: drawing a fresh animated canvas, encoding it and
writing it to a sink. Medians on one core, 9×18 px cells.

| Canvas | Protocol | Encode + write | Whole frame | FPS | Bytes/frame |
|---|---|---|---|---|---|
| logo 32×8    | Text   | 15 µs   | 269 µs  | 3700 | 1.9 KB  |
| logo 32×8    | Kitty  | 193 µs  | 446 µs  | 2200 | 7.0 KB  |
| logo 32×8    | iTerm2 | 255 µs  | 508 µs  | 2000 | 7.2 KB  |
| logo 32×8    | Sixel  | 654 µs  | 908 µs  | 1100 | 8.1 KB  |
| plot 80×24   | Text   | 52 µs   | 70 µs   | 14000 | 7.2 KB |
| plot 80×24   | Kitty  | 902 µs  | 919 µs  | 1100 | 22 KB   |
| plot 80×24   | iTerm2 | 1.14 ms | 1.16 ms | 860  | 22 KB   |
| plot 80×24   | Sixel  | 1.31 ms | 1.32 ms | 760  | 21 KB   |
| plot 200×50  | Text   | 237 µs  | 301 µs  | 3300 | 32 KB   |
| plot 200×50  | Kitty  | 4.1 ms  | 4.2 ms  | 240  | 93 KB   |
| plot 200×50  | iTerm2 | 5.4 ms  | 5.5 ms  | 180  | 94 KB   |
| plot 200×50  | Sixel  | 6.5 ms  | 6.5 ms  | 150  | 94 KB   |

In practice this is not what limits you. A full-screen 80×24 plot on kitty costs about a
millisecond of your process, and the terminal's own image decoder sets the frame rate from
there. Through the ratatui widget the same plot runs at roughly 980 fps on kitty, 790 on
iTerm2, 700 on sixel and 7500 as text. The whole shapes example, a dithered fill plus a
spline, a Bézier, a filled polygon, an ellipse and text, draws in under 100 µs. Palette
colours instead of RGB cost nothing measurable and make frames slightly smaller.

Why it is cheap:

* A canvas is one flat `u32` per dot. 200×50 cells is 320 KB, allocated once.
* `Renderer` reuses every buffer, so steady-state rendering does not allocate.
* Rasterising is one table lookup per pixel, from a mask built once per cell size.
* The built-in zlib encoder tries four match distances (one pixel, one cell, one row, one
  dot row), which is what flat runs, dither patterns and vertical repetition actually need.
  It compares eight bytes at a time. The 288×144 logo frame goes from 166 KB to 7 KB.
* One kitty image id per renderer, so updates replace in place and never flicker.
* No dependencies beyond optional `libc` for detection and `ratatui` for the widget.

## Examples

```sh
cargo run --example gallery                 # every primitive, one per panel
cargo run --example shapes                  # a scene built from them
cargo run --example logo                    # the mascot (add `theme`, `text`, `svg`, `png`)
cargo run --example calibrate               # match dot size to your font
cargo run --release --example snake         # animation, with timing
cargo run --release --features ratatui --example tui
COBRA_COLORS=16 cargo run --example gallery # the 16-colour fallback
cargo bench                                 # the table above
```

## Environment

| Variable | Effect |
|---|---|
| `COBRA_PROTOCOL` | `text`, `kitty`, `iterm2` or `sixel`; skips detection |
| `COBRA_CELL` | cell size in pixels, e.g. `9x18` |
| `COBRA_DOT` | dot diameter as a fraction of its slot, e.g. `0.8` |
| `COBRA_PALETTE` | `0` skips the colour-scheme queries |
| `COBRA_COLORS` | text colour depth: `mono`, `16`, `256` or `true` |

## Limits

* Inside tmux or screen you get the text protocol. Set `COBRA_PROTOCOL` if your
  multiplexer passes graphics through.
* Off unix there are no tty queries. Overrides are honoured, otherwise text.
* `copy_text` prints the glyphs under the image so selecting the region copies braille. On
  terminals that draw text above images it shows through, so it is off by default.
* A printed character takes its whole cell, so the dots in that cell are not drawn. Text
  and dots share a canvas, not a cell.

## Versioning

Releases are named for the year they ship in: `YY.N.P` is the `N`th release of 20`YY`,
patch `P`. `26.1.0` is the first release of 2026, `26.1.1` a patch on it, `26.2.0` the
next release of the year, and `27.1.0` the first of the next.

The three fields are still a valid semver triple, so Cargo treats them the way you would
want: a patch is a drop-in, a new release within a year is a compatible upgrade, and a new
year bumps the major — which is the one place a yearly line should be free to break things.
Depending on `"26.1"` gets you every release of 2026 from `26.1.0` onwards.

`tests/version.rs` checks the shape of the version in `Cargo.toml`, and `ci/version.fish`
checks the rest against the repository's tags: main always carries a version strictly newer
than the newest release, so a release can never be cut twice or go backwards. Both run in
CI on every pull request.

## Contributing

```sh
cargo test --all-features           # unit, integration and doc tests
cargo clippy --all-features --all-targets -- -D warnings
cargo fmt --all
ci/version.fish check               # the version gate CI runs
```

Integration tests under `tests/` decode frames back to pixels with their own PNG, zlib,
sixel and kitty readers, deliberately not sharing code with `src/encode`, so a round trip
is real evidence rather than a restatement.

To cut a release, bump the version in `Cargo.toml` and merge it to main; the release
workflow tags it, publishes it and opens the next patch version. A patch can also be cut
straight from the Actions tab: run **Release** with a `bump` of `patch`, `release` or
`year`, and it does the bump, the checks and the tag in one go. Every release runs the
full test suite first, so a red build cannot ship.

## License

MIT or Apache-2.0, at your option.
