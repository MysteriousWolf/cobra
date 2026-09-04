<p align="center"><img src="assets/logo.svg" alt="cobra" width="420"></p>

# cobra

Per-dot coloured braille canvases for the terminal.

A braille glyph packs a 2×4 grid of dots into one character cell, but text can only
give the whole cell one colour. `cobra` keeps the braille model (a canvas of
individually coloured dots) and shows it the best way the current terminal can:
as an image aligned to the cell grid when the terminal has a graphics protocol, and
as plain coloured braille everywhere else.

```text
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣀⣴⣶⣤⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣴⣿⣿⣿⣿⣿⡒⠒⠭⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢰⣿⣿⣿⣿⣿⣿⡇⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⢿⣿⣿⣿⣿⣿⠃⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠛⣿⣿⠛⠁⠀⠀⠀⠀
⠀⠀⠀⠀⠀⢀⣠⣴⣶⠿⠿⣿⣶⣶⣤⣤⣤⣤⣤⣤⣿⣿⠀⠀⠀⠀⠀⠀
⠐⠒⠶⠶⠾⠛⠉⠁⠀⠀⠀⠀⠈⠉⠛⠛⠿⠿⠿⠟⠃⠀⠀⠀⠀⠀⠀⠀```

The mascot above is drawn with the library (`cargo run --example logo`), rendered
as SVG with the same cell-aligned geometry the image protocols use. Every dot in it
has its own colour.

## How it works

| Protocol | Terminals | Frame on the wire | Copyable braille |
|---|---|---|---|
| Kitty  | kitty, WezTerm, Ghostty, Konsole ≥ 22.04 | zlib RGBA in chunked APC, one image id reused per canvas | with `copy_text` |
| iTerm2 | iTerm2, WezTerm, mintty, Konsole | PNG in OSC 1337 | with `copy_text` |
| Sixel  | foot, xterm, mlterm, Windows Terminal ≥ 1.22 | palettised DCS, transparent background | with `copy_text` |
| Text   | everything, tmux, pipes | braille glyphs, dominant colour per cell | yes |

1. **Detect once.** `Terminal::detect()` reads the `COBRA_PROTOCOL` and `COBRA_CELL`
   overrides, checks well-known environment variables, and only then spends one short
   escape-sequence round trip on `/dev/tty` (kitty probe, `CSI 16 t` cell size, `DA1`
   for sixel). The cell size comes from `TIOCGWINSZ` when the terminal fills in the
   pixel fields, which costs a single `ioctl`.
2. **Rasterise on the real grid.** Terminals do not publish font names or point
   sizes, only the pixel box of one cell. That is all alignment needs: each dot is
   drawn in its 2×4 slot of a cell of exactly that size, so the image lines up with
   surrounding text and with the braille glyphs the text fallback would print.
3. **Ship a small frame.** Frames are mostly transparent flat colour. The built-in
   zlib encoder only looks for runs and repeated scanlines, which is cheap and turns a
   250×126 RGBA frame (127 KB) into about 7 KB for kitty and iTerm2.

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

`Canvas` offers `set`, `unset`, `get`, `line`, `disc`, `clear`, per-cell
inspection and `to_text()` for plain copyable braille. Coordinates are `i32`; off-canvas
dots are ignored, so shapes can be drawn partially outside without clamping.

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

Measured with `cargo run --release --example snake` (28×7 cells, 9×18 px cells, 90 frames),
encode plus write per frame on one core:

| Protocol | Time per frame | Bytes per frame |
|---|---|---|
| Text   | 0.14 ms | 1.7 KB |
| Kitty  | 0.42 ms | 7.1 KB |
| iTerm2 | 0.44 ms | 7.2 KB |
| Sixel  | 0.82 ms | 7.6 KB |

* `Canvas` is a flat `u32` per dot; a 200×50-cell canvas is 320 KB and never reallocates.
* `Renderer` reuses all its buffers; steady-state rendering does not allocate.
* Rasterisation is one table lookup per pixel using a mask built once per cell size.
* One kitty image id per renderer, so updates replace in place without flicker.
* No dependencies beyond optional `libc` (detection) and `ratatui` (widget).

## Examples

```sh
cargo run --example logo            # detect and draw the mascot
cargo run --example logo -- text    # plain braille
cargo run --example logo -- svg     # SVG on stdout
cargo run --release --example snake # animation with timing
cargo run --release --features ratatui --example tui
```

## Environment

| Variable | Effect |
|---|---|
| `COBRA_PROTOCOL` | `text`, `kitty`, `iterm2` or `sixel`; skips detection |
| `COBRA_CELL` | cell size in pixels, e.g. `9x18` |

## Limits

* Inside tmux or screen the text protocol is used; set `COBRA_PROTOCOL` if your
  multiplexer passes graphics through.
* On non-unix platforms there are no tty queries; overrides are honoured, otherwise text.
* `copy_text` prints the glyphs under the image so selecting the region copies braille;
  on terminals that draw text above images it shows through and is off by default.

## License

MIT or Apache-2.0, at your option.
