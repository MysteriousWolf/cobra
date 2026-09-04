<p align="center"><img src="assets/logo.svg" alt="cobra" width="420"></p>

# cobra

Per-dot coloured braille canvases for the terminal.

A braille glyph packs a 2×4 grid of dots into one character cell, but text can only
give the whole cell one colour. `cobra` keeps the braille model (a canvas of
individually coloured dots) and shows it the best way the current terminal can:
as an image aligned to the cell grid when the terminal has a graphics protocol, and
as plain coloured braille everywhere else.

```text
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⣤⣤⣄⡀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⠔⢄⢰⣿⣿⣿⣿⣷⡆⢄⢀⡀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢔⢕⢽⣵⣌⠻⣿⣿⡿⠿⢋⢽⢕⠇⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢝⢽⣿⣿⡟⣷⣶⣶⣶⣿⣿⣿⢝⠅⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠝⢝⢽⢽⠅⣿⣿⣿⢸⢽⢽⢝⠝⠅⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⣀⣀⣤⣤⣤⣀⣀⣁⡁⠅⠅⣿⣿⣿⠐⠅⠅⠁⠁⠀⠀⠀
⣠⣤⣀⣀⣠⣤⣴⣶⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡇⣿⣿⣿⠀⠀⠀⠀⠀⠀⠀⠀
⠈⠛⠛⠛⠛⠛⠛⠋⠉⠁⠀⠀⠀⠀⠀⠈⠉⠙⠛⠛⠿⠮⠭⠁⠀⠀⠀⠀⠀⠀⠀⠀
```

The mascot above is drawn with the library (`cargo run --example logo`) and exported
as a transparent SVG with the same cell-aligned geometry the image protocols use. Every
dot has its own colour; the hood flaps are ordered-dithered so they sit behind the
solid neck and head, and a dark outline keeps the shape readable on any background.

## How it works

| Protocol | Terminals | Frame on the wire | Copyable braille |
|---|---|---|---|
| Kitty  | kitty, WezTerm, Ghostty, Konsole ≥ 22.04 | zlib RGBA in chunked APC, one image id reused per canvas | with `copy_text` |
| iTerm2 | iTerm2, WezTerm, mintty, Konsole | PNG in OSC 1337 | with `copy_text` |
| Sixel  | foot, xterm, mlterm, Windows Terminal ≥ 1.22 | palettised DCS, transparent background | with `copy_text` |
| Text   | everything, tmux, pipes | braille glyphs, dominant colour per cell, quantised to the terminal's colour depth | yes |

1. **Detect once.** `Terminal::detect()` reads the `COBRA_PROTOCOL` and `COBRA_CELL`
   overrides, checks well-known environment variables, then spends one short
   escape-sequence round trip on `/dev/tty` for whatever is still unknown (kitty probe,
   `CSI 16 t` cell size, `DA1` for sixel) plus the colour scheme (`OSC 4/10/11`). The
   round trip ends as soon as the terminal answers `DA1`. The cell size comes from
   `TIOCGWINSZ` when the terminal fills in the pixel fields, which costs a single `ioctl`.
2. **Rasterise on the real grid.** Terminals do not publish font names or point
   sizes, only the pixel box of one cell. That is all alignment needs: each dot is
   drawn in its 2×4 slot of a cell of exactly that size, so the image lines up with
   surrounding text and with the braille glyphs the text fallback would print.
3. **Ship a small frame.** Frames are transparent apart from the dots (alpha for kitty
   and iTerm2, `P2=1` for sixel), so they never paint over the terminal's background.
   The built-in zlib encoder only looks for runs, cell-periodic patterns and repeated
   scanlines, which is cheap and turns the 288×144 RGBA logo frame (166 KB) into 7 KB.

## Usage

```rust
use cobra::{Canvas, Renderer, Rgb, Terminal};

let term = Terminal::detect();          // once, at start-up
let mut renderer = Renderer::new(term); // owns every scratch buffer
let mut canvas = Canvas::new(20, 5);    // 20×5 cells = 40×20 dots

for x in 0..canvas.width() {
    let t = x as f32 / canvas.width() as f32;
    canvas.set(x, 10, Rgb::hex(0xff0055).lerp(Rgb::hex(0x00ccff), t));
}
renderer.render(&canvas, &mut std::io::stdout())?;
```

`render` draws at the cursor and leaves it on the line below. `render_at(col, row)`
draws at an absolute position and preserves the cursor. `encode` returns the frame
bytes without writing, for callers that manage their own output.

`Canvas` offers `set`, `unset`, `get`, `line`, `disc`, `clear_disc`, `clear`, per-cell
inspection and `to_text()` for plain copyable braille. Coordinates are `i32`; off-canvas
dots are ignored, so shapes can be drawn partially outside without clamping.

`set_dithered` and `disc_dithered` take a coverage in `0..=1` and draw that fraction of
the dots in an ordered 4×4 Bayer pattern. Dots are on or off, so this is what shading
and depth look like on a dot matrix; `bayer(x, y)` exposes the threshold for custom
patterns.

### Shapes

Vector primitives draw straight into the dot buffer as horizontal spans, one clipped
`fill` per dot row instead of a bounds check per dot. Coordinates are `f32` dots; a
fill covers every dot whose centre is inside, so integer boxes are exact.

```rust
use cobra::{Paint, Point};

canvas.fill_rect(2.0, 3.0, 20.0, 8.0, 0x3aa0ff);             // dots 2..22 × 3..11
canvas.rect(2.0, 3.0, 20.0, 8.0, 1.0, Paint::dithered(ink, 0.5)); // 1-dot border, half the dots
canvas.fill_polygon(&[(0.0, 0.0), (30.0, 4.0), (10.0, 20.0)], green);
canvas.polygon(&tri, 2.0, ink);                              // closed stroke, 2 dots wide
canvas.fill_ellipse(cx, cy, 12.0, 6.0, Paint::erase());     // clear a margin, then outline it
canvas.ellipse(cx, cy, 12.0, 6.0, 1.0, red);
canvas.arc(cx, cy, r, r, 0.0, 3.14, 1.5, red);
canvas.bezier(&[p0, c0, c1, p1], 1.0, ink);                  // 3 points = quadratic, 4 = cubic
canvas.spline(&points, false, 2.0, ink);                     // Catmull–Rom through every point
canvas.polyline(&points, 1.0, ink);
```

Every shape takes a `Paint`: anything that converts to a `Color`, `Paint::dithered(color,
coverage)` for the Bayer pattern, or `Paint::erase()` to unset dots. Strokes of width
≤ 1 are Bresenham lines; wider ones are a convex quad per segment with round joins and
caps. Curves are flattened one segment at a time, so nothing allocates except
`fill_polygon` (it sorts its edge crossings). `span(y, x0, x1, paint)` is the primitive
underneath, for shapes of your own.

### Text and fonts

```rust
use cobra::Font;

canvas.text(2, 2, "cobra", &Font::tiny().scale(2), ink);
canvas.text(2, 14, "3×5 dots", Font::tiny(), Paint::dithered(ink, 0.75));
let (w, h) = Font::tiny().measure("right aligned");
```

`Font::tiny()` is a built-in proportional 3×5 font (printable ASCII); `scale(n)` makes an
`n`× copy once, so one small master gives every size. Fonts are a plain text format,
parsed at compile time with `include_str!` or at run time with `Font::parse`, so a script
in any language can generate one; `Font::add` defines glyphs from code:

```text
// comment
height 5      // line advance (defaults to the tallest glyph)
spacing 1     // dots between glyphs
line 1        // dots between lines

A             // one character, or U+0041; a blank line ends the glyph
.#.           // '#' is a dot, '.' is not
#.#
###
#.#
#.#
```

Glyphs may have any width and height (up to 64 dots wide), so a font can mix narrow
punctuation with wide capitals or hold a few large symbols.

### Terminal colours

Dot colours are `Color`s: an explicit `Rgb`, one of the terminal's 256 palette entries,
or its default foreground. Palette colours follow the user's theme:

```rust
use cobra::Color;

canvas.set(x, y, Color::Indexed(2));   // ANSI green, whatever the theme makes it
canvas.set(x, y, Color::Foreground);   // the text colour
canvas.set(x, y, 0x5ec33a);            // still fine: anything Into<Color>
```

The text fallback emits them as plain SGR indices, so the terminal applies its own
palette. For the image protocols `Terminal::detect()` asks the terminal for its ANSI
colours, foreground and background (`OSC 4`, `OSC 10`, `OSC 11`) in the same round
trip it already makes, and the renderer resolves each dot through that `Palette` with
one table lookup. No measurable cost either way; see the benchmarks below. Without a
reply the xterm defaults are used, and `Terminal::with_palette` sets one by hand.

### Fewer colours

Not every terminal that gets the text fallback can show 24-bit colour. `Terminal::detect`
also learns the colour `Depth` (`COBRA_COLORS`, `NO_COLOR`, `COLORTERM`, the terminal
program, `TERM`), and the text renderer and the ratatui widget quantise to it:

| Depth | Cells are | Nearest colour by |
|---|---|---|
| `TrueColor` | `38;2;r;g;b` | nothing to do |
| `Ansi256` | `38;5;n` | closest cube corner vs. closest grey, analytically |
| `Ansi16` | `30–37` / `90–97` | the terminal's own ANSI palette, when it was queried |
| `Mono` | default foreground | every dot |

Distances are "redmean" weighted RGB, a cheap approximation of perceptual difference.
Each dot is quantised *before* the cell's dominant colour is picked, so two shades that
land on the same palette entry vote together instead of splitting; a per-frame cache
makes that one multiply and a compare per dot. `Color::quantize` and
`Palette::nearest_ansi` are public for callers that pick colours themselves.

### Export

```rust
use cobra::export::{png, svg, Style};

std::fs::write("plot.png", png(&canvas, &Style::default().scale(2)))?;
std::fs::write("plot.svg", svg(&canvas, &Style::default()))?;
```

Both use the renderer's geometry (cell box, dot diameter) so a file looks like the
terminal did. The background is transparent unless `Style::background` is set;
`Style::palette` resolves terminal colours for the file.

### Dot style and fonts

Terminals publish the pixel size of a cell (`TIOCGWINSZ`, `CSI 16 t`) but not the
font, and no protocol exposes glyph outlines, so the *position* of every dot is exact
while its *diameter* is a style choice. The default (`0.7` of a 2×4 slot) is close to
what most monospace fonts draw for braille. To match yours exactly run

```sh
cargo run --example calibrate
```

which prints the font's glyphs above image dots at six sizes; export `COBRA_DOT=0.8`
(or whichever row matches) and every renderer picks it up through `Options::from_env`.
Kitty and Ghostty draw braille themselves rather than from the font, so their glyphs
and cobra's dots are consistent across fonts on those terminals.

### ratatui

```toml
cobra = { version = "0.1", features = ["ratatui"] }
```

```rust
use cobra::ratatui::{Braille, overlay};

let area = terminal.draw(|f| {
    let area = f.area();
    f.render_widget(Braille::new(&canvas, &renderer), area);
})?.area;
overlay(&mut renderer, &canvas, area, terminal.backend_mut())?;
```

On kitty the widget writes Unicode placeholder cells that never change between frames,
so ratatui's diff leaves them alone and `overlay` only transmits the compressed image.
On iTerm2 and sixel, which have no in-buffer placement, the widget writes the text
fallback and `overlay` paints the image over it. On text terminals `overlay` is a no-op.

## Performance

`cargo bench` measures, per frame, drawing a fresh animated canvas, encoding it and
writing it to a sink: everything a program pays apart from the terminal's own decoding.
Medians on one core, 9×18 px cells (`cargo bench --features ratatui` adds the widget path):

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

The logo's "whole frame" is dominated by drawing it (240 discs); the plot is three sine
traces over a dithered fill. `cargo bench` also has a `Text/16` row (quantising every
dot to the ANSI palette adds about 5 % to the text encode) and a `shapes 80×24` case
(dithered fill, stroked spline, Bézier, filled polygon, ellipse and text) that draws in
under 100 µs through the span primitives. Palette colours instead of RGB change nothing measurable
(within 5 %, frames a little smaller). Through the ratatui widget (render into a
`Buffer`, diff, overlay) an 80×24 plot runs at about 980 fps on kitty, 790 on iTerm2,
700 on sixel and 7500 as text. In practice the terminal's decoder, not this crate, sets
the frame rate for the image protocols.

* `Canvas` is a flat `u32` per dot; a 200×50-cell canvas is 320 KB and never reallocates.
* `Renderer` reuses all its buffers; steady-state rendering does not allocate.
* Rasterisation is one table lookup per pixel using a mask built once per cell size.
* The built-in zlib encoder only tries four match distances (one pixel, one cell, one
  row, one dot row), which covers flat runs, dither patterns and vertical repetition;
  it compares eight bytes at a time and sticks with the last distance while it keeps
  matching.
* One kitty image id per renderer, so updates replace in place without flicker.
* No dependencies beyond optional `libc` (detection) and `ratatui` (widget).

## Examples

```sh
cargo run --example logo                    # detect and draw the mascot
cargo run --example logo -- theme           # in the terminal's palette colours
cargo run --example logo -- text            # plain braille
cargo run --example logo -- svg logo.svg    # transparent SVG (or png)
cargo run --example calibrate               # match dot size to your font
cargo run --example shapes                  # polygons, splines, arcs, fonts
COBRA_COLORS=16 cargo run --example shapes  # the same on a 16-colour terminal
cargo run --release --example snake         # animation with timing (add `theme`)
cargo run --release --features ratatui --example tui   # `t` toggles palette colours
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

* Inside tmux or screen the text protocol is used; set `COBRA_PROTOCOL` if your
  multiplexer passes graphics through.
* On non-unix platforms there are no tty queries; overrides are honoured, otherwise text.
* `copy_text` prints the glyphs under the image so selecting the region copies braille;
  on terminals that draw text above images it shows through and is off by default.

## License

MIT or Apache-2.0, at your option.
