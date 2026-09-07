# `canvas`

[Index](README.md) · **canvas** · [draw](draw.md) · [path](path.md) · [mask](mask.md) · [transform](transform.md) · [layer](layer.md) · [bubble](bubble.md) · [font](font.md) · [text](text.md) · [color](color.md) · [render](render.md) · [term](term.md) · [export](export.md) · [ratatui](ratatui.md)

The dot grid.

## Contents

- [`DOTS_X`](#dots_x)
- [`DOTS_Y`](#dots_y)
- [`Cell`](#cell)
- [`Canvas`](#canvas)
- [`braille`](#braille)
- [`bayer`](#bayer)
- `Cell`: [`Cell::glyph`](canvas.md#cellglyph)
- `Canvas`: [`Canvas::new`](canvas.md#canvasnew), [`Canvas::with`](canvas.md#canvaswith), [`Canvas::transform`](canvas.md#canvastransform), [`Canvas::cols`](canvas.md#canvascols), [`Canvas::rows`](canvas.md#canvasrows), [`Canvas::width`](canvas.md#canvaswidth), [`Canvas::height`](canvas.md#canvasheight), [`Canvas::clear`](canvas.md#canvasclear), [`Canvas::set`](canvas.md#canvasset), [`Canvas::set_dithered`](canvas.md#canvasset_dithered), [`Canvas::unset`](canvas.md#canvasunset), [`Canvas::get`](canvas.md#canvasget), [`Canvas::line`](canvas.md#canvasline), [`Canvas::disc`](canvas.md#canvasdisc), [`Canvas::disc_dithered`](canvas.md#canvasdisc_dithered), [`Canvas::clear_disc`](canvas.md#canvasclear_disc), [`Canvas::cell`](canvas.md#canvascell), [`Canvas::cells`](canvas.md#canvascells), [`Canvas::to_text`](canvas.md#canvasto_text)

## `DOTS_X`

```rust
pub const DOTS_X: u16 = 2
```

Dots per cell horizontally.

## `DOTS_Y`

```rust
pub const DOTS_Y: u16 = 4
```

Dots per cell vertically.

## `Cell`

```rust
pub struct Cell
```

One terminal cell of a canvas: which of its eight dots are set, and the colour the
cell should take when it can only have one (text fallback).

- `pub bits: u8` — Braille dot bits (`U+2800 + bits` is the glyph).
- `pub color: Option<Color>` — The most frequent colour among the set dots, `None` when the cell is empty. On a canvas [flattened](layer.md#layersflatten) from layers only the dots of the topmost layer in the cell vote, so what is in front keeps its colour.

## `Canvas`

```rust
pub struct Canvas
```

A grid of individually coloured braille dots.

The canvas is sized in terminal cells; each cell holds a 2×4 block of dots, so a
`cols × rows` canvas has `2·cols × 4·rows` addressable dots with `(0, 0)` at the top
left. Coordinates are `i32` so callers can draw partially off-canvas shapes without
clamping; out-of-range dots are ignored.

Storage is one `u32` per dot (`0` = unset, otherwise a tagged [`Color`](color.md#color)), so a full
200×50-cell canvas is 320 KiB and never allocates after construction.

On top of the dots sits a [text layer](text.md): real characters, one per cell,
written with [`print`](text.md#canvasprint). It stays unallocated until something is
printed, and a cell that holds a character shows it instead of its dots.

Several canvases stacked, with occlusion and effects between them, are a
[`Layers`](layer.md#layers); flattening one gives back a plain canvas.

## `braille`

```rust
pub fn braille(bits: u8) -> char
```

Braille glyph for a dot bit pattern.

## `bayer`

```rust
pub fn bayer(x: i32, y: i32) -> f32
```

Ordered-dither threshold for dot `(x, y)`: a 4×4 Bayer matrix scaled to `0..1`
(sixteen evenly spaced levels). A dot is drawn when its coverage exceeds this.

## `Cell` methods

## `Cell::glyph`

```rust
pub fn glyph(&self) -> char
```

The braille glyph for this cell.

## `Canvas` methods

## `Canvas::new`

```rust
pub fn new(cols: u16, rows: u16) -> Self
```

Creates an empty canvas of `cols × rows` cells.

## `Canvas::with`

```rust
pub fn with(&mut self, t: Transform, f: impl FnOnce(&mut Canvas))
```

Draws everything in `f` through `t`: the coordinates every primitive takes
are local, and `t` says where they land. Nested calls compose, inner first,
so a hand drawn inside an arm drawn inside a body moves with all three.
Whole-cell operations ([`print`](text.md#canvasprint), [`span`](draw.md#canvasspan),
[`set`](canvas.md#canvasset) and the mask operations) are not transformed.

A translation or a flip is exact; under a rotation or a scale, boxes,
ellipses and rings are drawn as paths, and stroke widths scale with the
transform.

```rust
use cobra::{Canvas, Rgb, Transform};

let mut canvas = Canvas::new(20, 5);
let leg = |c: &mut Canvas| c.fill_rect(-1.0, 0.0, 2.0, 8.0, Rgb::hex(0xffa657));
for (x, angle) in [(10.0, -0.3), (14.0, 0.3)] {
    canvas.with(Transform::at(x, 8.0).rotate(angle), leg); // rotated, then placed
}
```

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/canvas-with.svg">
  <img src="img/canvas-with-light.svg" alt="Canvas::with" width="384">
</picture>

```rust
// A figure drawn about its own origin, facing either way.
let figure = |c: &mut Canvas| {
    c.fill_ellipse(0.0, 0.0, 5.0, 3.5, GREEN);      // body
    c.disc(5.0, -4.0, 2.2, GREEN);                  // head
    c.polyline(&[(7.0, -4.0), (10.0, -3.0)], 1.0, ORANGE); // beak
    c.polyline(&[(-1.0, 3.0), (-1.0, 7.0)], 1.0, ORANGE);  // leg
};
c.with(Transform::at(12.0, 8.0), figure);
c.with(Transform::at(36.0, 8.0).flip_x(), figure);
```

## `Canvas::transform`

```rust
pub fn transform(&self) -> Transform
```

The transform drawing currently lands through; the identity by default.

## `Canvas::cols`

```rust
pub fn cols(&self) -> u16
```

Width in cells.

## `Canvas::rows`

```rust
pub fn rows(&self) -> u16
```

Height in cells.

## `Canvas::width`

```rust
pub fn width(&self) -> i32
```

Width in dots (`2 · cols`).

## `Canvas::height`

```rust
pub fn height(&self) -> i32
```

Height in dots (`4 · rows`).

## `Canvas::clear`

```rust
pub fn clear(&mut self)
```

Unsets every dot and empties the text layer. Keeps the allocations.

## `Canvas::set`

```rust
pub fn set(&mut self, x: i32, y: i32, color: impl Into<Color>)
```

Sets dot `(x, y)` to `color`. Out-of-range dots are ignored.

Accepts an [`Rgb`](color.md#rgb), a `0xRRGGBB` literal, an `(r, g, b)` tuple or a
[`Color`](color.md#color) (for palette colours).

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/canvas-set.svg">
  <img src="img/canvas-set-light.svg" alt="Canvas::set" width="384">
</picture>

```rust
for x in 0..c.width() {
    let y = 8.0 + 6.0 * (x as f32 / 6.0).sin();
    c.set(x, y as i32, RED.lerp(BLUE, x as f32 / c.width() as f32));
}
```

## `Canvas::set_dithered`

```rust
pub fn set_dithered(&mut self, x: i32, y: i32, color: impl Into<Color>, coverage: f32)
```

Sets dot `(x, y)` to `color` with probability `coverage` (`0..=1`) using an
ordered 4×4 Bayer pattern, which is what dithering means on a dot matrix:
dots are on or off, so partial coverage is spread evenly across the area.
Dots that the pattern skips are left unchanged.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/canvas-set_dithered.svg">
  <img src="img/canvas-set_dithered-light.svg" alt="Canvas::set_dithered" width="384">
</picture>

```rust
for y in 0..c.height() {
    for x in 0..c.width() {
        c.set_dithered(x, y, GREEN, x as f32 / c.width() as f32);
    }
}
```

## `Canvas::unset`

```rust
pub fn unset(&mut self, x: i32, y: i32)
```

Unsets dot `(x, y)`.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/canvas-unset.svg">
  <img src="img/canvas-unset-light.svg" alt="Canvas::unset" width="384">
</picture>

```rust
c.fill_rect(0.0, 0.0, 48.0, 16.0, BLUE);
for x in (0..48).step_by(3) {
    c.unset(x, 8);
}
```

## `Canvas::get`

```rust
pub fn get(&self, x: i32, y: i32) -> Option<Color>
```

The colour of dot `(x, y)`, `None` if unset or out of range.

## `Canvas::line`

```rust
pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: impl Into<Color>)
```

Draws a one-dot-wide line with Bresenham's algorithm.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/canvas-line.svg">
  <img src="img/canvas-line-light.svg" alt="Canvas::line" width="384">
</picture>

```rust
c.line(0, 15, 47, 0, RED);
c.line(0, 0, 47, 15, BLUE);
c.line(0, 8, 47, 8, t.ink);
```

## `Canvas::disc`

```rust
pub fn disc(&mut self, cx: f32, cy: f32, r: f32, paint: impl Into<Paint>)
```

Fills a disc of radius `r` (in dots) centred on `(cx, cy)`.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/canvas-disc.svg">
  <img src="img/canvas-disc-light.svg" alt="Canvas::disc" width="384">
</picture>

```rust
c.disc(8.0, 8.0, 7.0, YELLOW);
c.disc(24.0, 8.0, 5.0, CYAN);
c.disc(40.0, 8.0, 3.0, RED);
```

## `Canvas::disc_dithered`

```rust
pub fn disc_dithered(&mut self, cx: f32, cy: f32, r: f32, color: impl Into<Color>, coverage: f32)
```

Fills a disc like [`disc`](canvas.md#canvasdisc) but only `coverage` (`0..=1`) of its
dots, in an ordered pattern; see [`set_dithered`](canvas.md#canvasset_dithered).

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/canvas-disc_dithered.svg">
  <img src="img/canvas-disc_dithered-light.svg" alt="Canvas::disc_dithered" width="384">
</picture>

```rust
c.disc_dithered(8.0, 8.0, 7.0, YELLOW, 0.25);
c.disc_dithered(24.0, 8.0, 7.0, YELLOW, 0.5);
c.disc_dithered(40.0, 8.0, 7.0, YELLOW, 0.75);
```

## `Canvas::clear_disc`

```rust
pub fn clear_disc(&mut self, cx: f32, cy: f32, r: f32)
```

Unsets every dot inside a disc of radius `r` centred on `(cx, cy)`; a cleared
ring around a shape separates it from what is behind it on any background.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/canvas-clear_disc.svg">
  <img src="img/canvas-clear_disc-light.svg" alt="Canvas::clear_disc" width="384">
</picture>

```rust
c.fill_rect(0.0, 0.0, 48.0, 16.0, Paint::dithered(BLUE, 0.5));
c.clear_disc(24.0, 8.0, 7.0);
c.disc(24.0, 8.0, 5.0, RED);
```

## `Canvas::cell`

```rust
pub fn cell(&self, col: u16, row: u16) -> Cell
```

Cell `(col, row)` as a glyph plus its dominant colour.

## `Canvas::cells`

```rust
pub fn cells(&self) -> impl Iterator<Item = Cell> + '_
```

Iterates over all cells, row-major.

## `Canvas::to_text`

```rust
pub fn to_text(&self) -> String
```

Plain text, one line per row, no colour: braille glyphs for the dots, and the
real character wherever one was [printed](text.md#canvasprint). This is what a user gets
when they copy the canvas out of a terminal running the text fallback.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/canvas-to_text.svg">
  <img src="img/canvas-to_text-light.svg" alt="Canvas::to_text" width="192">
</picture>

```rust
c.disc(12.0, 4.0, 3.5, GREEN);
c.print(0, 0, "hi", t.ink);
```

[Index](README.md) · **canvas** · [draw](draw.md) · [path](path.md) · [mask](mask.md) · [transform](transform.md) · [layer](layer.md) · [bubble](bubble.md) · [font](font.md) · [text](text.md) · [color](color.md) · [render](render.md) · [term](term.md) · [export](export.md) · [ratatui](ratatui.md)

