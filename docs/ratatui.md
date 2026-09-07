# `ratatui`

[Index](README.md) · [canvas](canvas.md) · [draw](draw.md) · [path](path.md) · [layer](layer.md) · [bubble](bubble.md) · [font](font.md) · [text](text.md) · [color](color.md) · [render](render.md) · [term](term.md) · [export](export.md) · **ratatui**

[`ratatui`](ratatui.md) integration.

[`Braille`](ratatui.md#braille) is a `Widget` that draws a [`Canvas`](canvas.md#canvas) into the frame buffer. What it
writes depends on the renderer's protocol:

* **Text** – braille glyphs with a foreground colour per cell. Nothing else to do.
* **Any protocol** – cells holding [text](text.md) are written as that real
  character with its own style, since ratatui's buffer is exactly the place for it.
* **Kitty** – Unicode *placeholder* cells that reference the renderer's image id.
  The image itself has to be transmitted outside the buffer once per frame, which
  is what [`overlay`](ratatui.md#overlay) does. Placeholders never change between frames, so ratatui's
  diff never rewrites them and only the compressed image moves over the wire.
* **iTerm2 / sixel** – these protocols have no in-buffer placement, so the widget
  writes the text fallback and [`overlay`](ratatui.md#overlay) paints the image on top after the frame.

```rust
let mut terminal = ratatui::init();
let mut renderer = Renderer::new(Terminal::detect());
let canvas = Canvas::new(40, 10);
loop {
    let area = terminal.draw(|f| {
        let area = f.area();
        f.render_widget(Braille::new(&canvas, &renderer), area);
    })?.area;
    overlay(&mut renderer, &canvas, area, terminal.backend_mut())?;
}
```

## Contents

- [`Braille`](#braille)
- [`overlay`](#overlay)
- `Braille`: [`Braille::new`](ratatui.md#braillenew)

## `Braille`

```rust
pub struct Braille<'a>
```

Widget that draws a canvas. See the module docs.

## `overlay`

```rust
pub fn overlay(renderer: &mut Renderer, canvas: &Canvas, area: Rect, w: &mut impl Write) -> io::Result<()>
```

Transmits the image for a [`Braille`](ratatui.md#braille) widget rendered at `area`. Call it after
`Terminal::draw`, with the backend's writer. No-op for the text protocol.

## `Braille` methods

## `Braille::new`

```rust
pub fn new(canvas: &'a Canvas, renderer: &'a Renderer) -> Self
```

Draws `canvas` the way `renderer` will overlay it. Text cells are quantised to
the terminal's [`Depth`](color.md#depth), like the text protocol does.

[Index](README.md) · [canvas](canvas.md) · [draw](draw.md) · [path](path.md) · [layer](layer.md) · [bubble](bubble.md) · [font](font.md) · [text](text.md) · [color](color.md) · [render](render.md) · [term](term.md) · [export](export.md) · **ratatui**

