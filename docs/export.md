# `export`

[Index](README.md) · [canvas](canvas.md) · [draw](draw.md) · [path](path.md) · [mask](mask.md) · [transform](transform.md) · [layer](layer.md) · [bubble](bubble.md) · [font](font.md) · [text](text.md) · [color](color.md) · [render](render.md) · [term](term.md) · **export** · [ratatui](ratatui.md)

Export a canvas as a transparent PNG or SVG with the same cell-aligned dot
geometry the image protocols draw, so a file looks like the terminal does.

```rust
use cobra::{export, Canvas, Rgb};

let mut canvas = Canvas::new(10, 3);
canvas.disc(10.0, 6.0, 4.0, Rgb::hex(0x5ec33a));
std::fs::write("dots.png", export::png(&canvas, &export::Style::default())).unwrap();
std::fs::write("dots.svg", export::svg(&canvas, &export::Style::default())).unwrap();
```

## Contents

- [`Style`](#style)
- [`png`](#png)
- [`svg`](#svg)
- [`resolve`](#resolve)
- `Style`: [`Style::scale`](export.md#stylescale)

## `Style`

```rust
pub struct Style
```

Geometry and colours for an export.

- `pub cell: CellSize` — Pixel size of one cell. Default `10×20`.
- `pub dot_size: f32` — Dot diameter as a fraction of its slot, as in [`Options`](render.md#options). Default `0.7`.
- `pub background: Option<Rgb>` — Background fill; `None` (the default) keeps it transparent.
- `pub palette: Palette` — Resolves [`Color::Indexed`](color.md#color) and [`Color::Foreground`](color.md#color) dots. Default: xterm.
- `pub text: bool` — Draw the [text layer](text.md). A file has no terminal font, so [`svg`](export.md#svg) writes `<text>` elements and [`png`](export.md#png) approximates the characters with [`Font::tiny`](font.md#fonttiny), which covers printable ASCII. Default `true`.

## `png`

```rust
pub fn png(canvas: &Canvas, style: &Style) -> Vec<u8>
```

Encodes `canvas` as an RGBA PNG. Unset dots are transparent unless
`style.background` is set.

## `svg`

```rust
pub fn svg(canvas: &Canvas, style: &Style) -> String
```

Renders `canvas` as an SVG document of one circle per set dot, positioned on the
same cell grid as [`png`](export.md#png). The `viewBox` is in pixels of `style.cell`.

## `resolve`

```rust
pub fn resolve(canvas: &Canvas, palette: &Palette) -> Canvas
```

Resolves every dot of `canvas` to RGB through `palette`; useful before exporting
a canvas that uses terminal colours to a file that has no terminal.

## `Style` methods

## `Style::scale`

```rust
pub fn scale(mut self, k: u16) -> Self
```

Scales the cell (and thus the whole image) by an integer factor, e.g. for a
crisp README logo.

[Index](README.md) · [canvas](canvas.md) · [draw](draw.md) · [path](path.md) · [mask](mask.md) · [transform](transform.md) · [layer](layer.md) · [bubble](bubble.md) · [font](font.md) · [text](text.md) · [color](color.md) · [render](render.md) · [term](term.md) · **export** · [ratatui](ratatui.md)

