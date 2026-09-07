# `render`

[Index](README.md) · [canvas](canvas.md) · [draw](draw.md) · [path](path.md) · [layer](layer.md) · [bubble](bubble.md) · [font](font.md) · [text](text.md) · [color](color.md) · **render** · [term](term.md) · [export](export.md) · [ratatui](ratatui.md)

Frame encoding for each protocol, and the [`Renderer`](render.md#renderer) that drives it.

## Contents

- [`Options`](#options)
- [`Placement`](#placement)
- [`Renderer`](#renderer)
- `Options`: [`Options::from_env`](render.md#optionsfrom_env)
- `Renderer`: [`Renderer::new`](render.md#renderernew), [`Renderer::with_options`](render.md#rendererwith_options), [`Renderer::terminal`](render.md#rendererterminal), [`Renderer::options`](render.md#rendereroptions), [`Renderer::set_options`](render.md#rendererset_options), [`Renderer::image_id`](render.md#rendererimage_id), [`Renderer::invalidate`](render.md#rendererinvalidate), [`Renderer::render`](render.md#rendererrender), [`Renderer::render_at`](render.md#rendererrender_at), [`Renderer::encode`](render.md#rendererencode), [`Renderer::encode_view`](render.md#rendererencode_view)

## `Options`

```rust
pub struct Options
```

Rendering options.

- `pub dot_size: f32` — Dot diameter as a fraction of its 2×4 slot, `0..=1`. Default `0.7`, close to what monospace fonts draw for braille; `COBRA_DOT` overrides it in [`Options::from_env`](render.md#optionsfrom_env) so the image dots can be matched to the font's glyphs (`cargo run --example calibrate` shows them side by side).
- `pub copy_text: bool` — Print the braille text underneath the image so the canvas can still be copied as text. Only meaningful for image protocols. Default `false`.

## `Placement`

```rust
pub enum Placement
```

Where a frame goes.

- `Flow` — At the cursor; scrolls if needed and leaves the cursor on the line below.
- `At(u16, u16)` — At an absolute 0-based `(col, row)`; cursor position is preserved.
- `Virtual` — Kitty *virtual* placement: transmits the image for placeholder cells written elsewhere (see [`crate::ratatui`](ratatui.md)). Draws nothing on other protocols.

## `Renderer`

```rust
pub struct Renderer
```

Turns canvases into frames for one terminal.

Owns every scratch buffer it needs, so after the first frame rendering does not
allocate. One renderer per canvas (or per widget) is the intended granularity: it
carries the kitty image id that identifies "this picture" across frames.

## `Options` methods

## `Options::from_env`

```rust
pub fn from_env() -> Self
```

Defaults, with `COBRA_DOT` (a fraction such as `0.8`) applied to `dot_size`.

## `Renderer` methods

## `Renderer::new`

```rust
pub fn new(term: Terminal) -> Self
```

Creates a renderer for `term` with [`Options::from_env`](render.md#optionsfrom_env).

## `Renderer::with_options`

```rust
pub fn with_options(term: Terminal, opts: Options) -> Self
```

Creates a renderer with explicit options.

## `Renderer::terminal`

```rust
pub fn terminal(&self) -> &Terminal
```

The terminal this renderer targets.

## `Renderer::options`

```rust
pub fn options(&self) -> Options
```

Current options.

## `Renderer::set_options`

```rust
pub fn set_options(&mut self, opts: Options)
```

Replaces the options.

## `Renderer::image_id`

```rust
pub fn image_id(&self) -> u32
```

Kitty image id used for this renderer's frames (24-bit).

## `Renderer::invalidate`

```rust
pub fn invalidate(&mut self)
```

Forgets the last frame drawn with [`render_at`](render.md#rendererrender_at), so the next
one is sent in full. Text-protocol frames at a fixed position send only the
cells that changed since the previous frame at the same position; call this
after the screen was cleared or drawn over by something else.

## `Renderer::render`

```rust
pub fn render(&mut self, canvas: &Canvas, w: &mut impl Write) -> io::Result<()>
```

Renders `canvas` at the cursor and leaves the cursor at the start of the line
below it.

## `Renderer::render_at`

```rust
pub fn render_at(&mut self, canvas: &Canvas, col: u16, row: u16, w: &mut impl Write) -> io::Result<()>
```

Renders `canvas` at an absolute cell position, preserving the cursor.

## `Renderer::encode`

```rust
pub fn encode(&mut self, canvas: &Canvas, placement: Placement) -> &[u8]
```

Encodes a frame without writing it. The returned slice is valid until the next
call and is exactly what [`render`](render.md#rendererrender) would write.

## `Renderer::encode_view`

```rust
pub fn encode_view(&mut self, canvas: &Canvas, placement: Placement, cols: u16, rows: u16) -> &[u8]
```

Like [`encode`](render.md#rendererencode) but only the top-left `cols × rows` cells.

[Index](README.md) · [canvas](canvas.md) · [draw](draw.md) · [path](path.md) · [layer](layer.md) · [bubble](bubble.md) · [font](font.md) · [text](text.md) · [color](color.md) · **render** · [term](term.md) · [export](export.md) · [ratatui](ratatui.md)

