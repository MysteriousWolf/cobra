# `mask`

[Index](README.md) · [canvas](canvas.md) · [draw](draw.md) · [path](path.md) · **mask** · [transform](transform.md) · [rig](rig.md) · [layer](layer.md) · [bubble](bubble.md) · [font](font.md) · [text](text.md) · [color](color.md) · [render](render.md) · [term](term.md) · [export](export.md) · [ratatui](ratatui.md)

Shapes as things: a [`Mask`](mask.md#mask) is a set of dots with no colour, drawn with the same
primitives as a canvas, combined with other masks, moved, and then used to paint,
clip, cut or run effects. The [`Silhouette`](mask.md#silhouette) trait is what every mask-taking
operation accepts, so a plain [`Canvas`](canvas.md#canvas) serves as a mask too.

## Contents

- [`Silhouette`](#silhouette)
- [`Mask`](#mask)
- `Mask`: [`Mask::new`](mask.md#masknew), [`Mask::of`](mask.md#maskof), [`Mask::path`](mask.md#maskpath), [`Mask::cols`](mask.md#maskcols), [`Mask::rows`](mask.md#maskrows), [`Mask::width`](mask.md#maskwidth), [`Mask::height`](mask.md#maskheight), [`Mask::contains`](mask.md#maskcontains), [`Mask::set`](mask.md#maskset), [`Mask::unset`](mask.md#maskunset), [`Mask::clear`](mask.md#maskclear), [`Mask::is_empty`](mask.md#maskis_empty), [`Mask::len`](mask.md#masklen), [`Mask::bounds`](mask.md#maskbounds), [`Mask::dots`](mask.md#maskdots), [`Mask::draw`](mask.md#maskdraw), [`Mask::erase`](mask.md#maskerase), [`Mask::union`](mask.md#maskunion), [`Mask::subtract`](mask.md#masksubtract), [`Mask::intersect`](mask.md#maskintersect), [`Mask::boundary_with`](mask.md#maskboundary_with), [`Mask::translate`](mask.md#masktranslate), [`Mask::flip_x`](mask.md#maskflip_x), [`Mask::flip_y`](mask.md#maskflip_y), [`Mask::transform`](mask.md#masktransform)

## `Silhouette`

```rust
pub trait Silhouette
```

Something that covers some dots and not others: a [`Mask`](mask.md#mask), or a [`Canvas`](canvas.md#canvas) (its
set dots and printed cells). Everything that paints through, clips to, cuts out
or runs effects around a shape takes `&impl Silhouette`.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/silhouette.svg">
  <img src="img/silhouette-light.svg" alt="Silhouette" width="384">
</picture>

```rust
// Anything that covers dots: a `Mask`, or a `Canvas` (its dots and its
// printed cells) serve equally as the shape to paint through.
let mut mask = Mask::new(24, 4);
mask.draw(|c| c.disc(10.0, 8.0, 7.0, t.ink));
let mut canvas = Canvas::new(24, 4);
canvas.print(10, 1, "TEXT", t.ink);
canvas.fill_rect(34.0, 2.0, 12.0, 12.0, t.ink);
c.stencil(&mask, ORANGE);
c.stencil(&canvas, BLUE);
```

## `Mask`

```rust
pub struct Mask
```

A set of dots: a shape without a colour, one bit per dot.

A mask is sized like a canvas, in cells, and drawn like one: [`draw`](mask.md#maskdraw)
runs any drawing code and keeps the dots it set, so a silhouette is the same
`fill_ellipse` and `fill_polygon` calls as the picture, without a colour. Masks
combine ([`union`](mask.md#maskunion), [`subtract`](mask.md#masksubtract),
[`intersect`](mask.md#maskintersect), [`boundary_with`](mask.md#maskboundary_with)), move
([`transform`](mask.md#masktransform)), and are kept between frames: a part built once
is placed anew each frame instead of being redrawn.

What a mask is *for* is the canvas: [`Canvas::stencil`](draw.md#canvasstencil) paints through it,
[`Canvas::clip`](draw.md#canvasclip) and [`Canvas::cut`](draw.md#canvascut) keep or remove what it covers,
[`Canvas::clipped`](draw.md#canvasclipped) draws through it, and [`Canvas::effects`](layer.md#canvaseffects) runs outlines,
glows, rims and shadows around it.

```rust
use cobra::{Canvas, Effect, Mask, Paint, Rgb, Transform};

let mut head = Mask::new(20, 5);
head.draw(|c| c.fill_ellipse(0.0, 0.0, 5.0, 4.0, Rgb::hex(0)));   // about its own origin
let mut eye = Mask::new(20, 5);
eye.draw(|c| c.disc(2.0, -1.0, 1.0, Rgb::hex(0)));
head.subtract(&eye);
head.transform(&Transform::at(20.0, 10.0));                        // placed

let mut canvas = Canvas::new(20, 5);
canvas.stencil(&head, Paint::edge(Rgb::hex(0xffe08a), Rgb::hex(0xffa657), 2.0));
canvas.effects(&head, &[Effect::outline(1.0).paint(Rgb::hex(0x3d2200))]);
```

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask.svg">
  <img src="img/mask-light.svg" alt="Mask" width="384">
</picture>

```rust
// A silhouette built from parts, then painted, shaded and outlined as one.
let mut duck = Mask::new(24, 4);
duck.draw(|c| {
    c.fill_ellipse(20.0, 10.0, 11.0, 5.0, t.ink);
    c.fill_ellipse(30.0, 4.0, 4.0, 3.5, t.ink);
    c.fill_polygon(&[(33.0, 4.0), (40.0, 3.0), (33.0, 6.0)], t.ink);
});
let mut eye = Mask::new(24, 4);
eye.draw(|c| c.disc(31.0, 3.0, 0.8, t.ink));
duck.subtract(&eye);
c.stencil(&duck, Paint::cel(Rgb::hex(0x7a3e00), (-1.0, -1.0), &[(-0.1, ORANGE), (0.5, YELLOW)]).per_cell());
c.effects(&duck, &[Effect::outline(1.0).paint(t.ink)]);
```

## `Mask` methods

## `Mask::new`

```rust
pub fn new(cols: u16, rows: u16) -> Self
```

An empty mask of `cols × rows` cells.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-new.svg">
  <img src="img/mask-new-light.svg" alt="Mask::new, Mask::set, Mask::unset, Mask::contains" width="384">
</picture>

```rust
// Dot by dot: a mask is a set of dots, written and read like a canvas
// without colours.
let mut m = Mask::new(24, 4);
for y in 0..16 {
    for x in 0..48 {
        if (x / 4 + y / 4) % 2 == 0 {
            m.set(x, y);
        }
    }
}
for x in 0..48 {
    m.unset(x, 8);
}
for y in 0..16 {
    for x in 0..48 {
        c.set(x, y, if m.contains(x, y) { CYAN } else { t.panel });
    }
}
```

## `Mask::of`

```rust
pub fn of(shape: &Canvas) -> Self
```

The silhouette of `shape` as a mask of the same size: a canvas's set dots and
printed cells.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-of.svg">
  <img src="img/mask-of-light.svg" alt="Mask::of" width="384">
</picture>

```rust
// The silhouette of a canvas, set dots and printed cells alike, as a mask.
let mut drawing = Canvas::new(24, 4);
drawing.disc(9.0, 8.0, 7.0, Paint::linear((2.0, 0.0), (16.0, 0.0), RED, YELLOW));
drawing.print(10, 1, "text", t.ink);
let m = Mask::of(&drawing);
c.stencil(&m, GREEN);
c.effects(&m, &[Effect::outline(1.0).paint(t.ink)]);
```

## `Mask::path`

```rust
pub fn path(cols: u16, rows: u16, path: &crate::Path) -> Self
```

A `cols × rows` mask of the dots `path` fills (every subpath closed, even-odd,
as [`Canvas::fill_path`](draw.md#canvasfill_path) does): a shape posed as control points, rasterised
once.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-path.svg">
  <img src="img/mask-path-light.svg" alt="Mask::path" width="384">
</picture>

```rust
// Pose the control points, rasterise once.
let mut leaf = Path::new();
leaf.move_to((0.0, 0.0)).quad_to((10.0, -8.0), (20.0, 0.0)).quad_to((10.0, 8.0), (0.0, 0.0)).close();
for i in 0..3 {
    let mut posed = leaf.clone();
    posed.apply(&Transform::at(4.0 + 14.0 * i as f32, 8.0).rotate(0.4 * i as f32 - 0.4));
    c.stencil(&Mask::path(24, 4, &posed), GREEN.lerp(YELLOW, i as f32 / 2.0));
}
```

## `Mask::cols`

```rust
pub fn cols(&self) -> u16
```

Width in cells.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-cols.svg">
  <img src="img/mask-cols-light.svg" alt="Mask::cols, Mask::rows, Mask::width, Mask::height, Mask::len, Mask::is_empty, Mask::clear" width="448">
</picture>

```rust
let mut m = Mask::new(28, 4); // 56 × 16 dots
m.draw(|c| c.disc(8.0, 8.0, 7.0, t.ink));
c.stencil(&m, BLUE);
c.text(20, 1, &format!("{}/{}", m.len(), m.width() * m.height()), Font::tiny(), t.ink);
m.clear();
c.text(20, 9, if m.is_empty() { "cleared" } else { "not empty" }, Font::tiny(), t.panel);
```

## `Mask::rows`

```rust
pub fn rows(&self) -> u16
```

Height in cells.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-cols.svg">
  <img src="img/mask-cols-light.svg" alt="Mask::cols, Mask::rows, Mask::width, Mask::height, Mask::len, Mask::is_empty, Mask::clear" width="448">
</picture>

```rust
let mut m = Mask::new(28, 4); // 56 × 16 dots
m.draw(|c| c.disc(8.0, 8.0, 7.0, t.ink));
c.stencil(&m, BLUE);
c.text(20, 1, &format!("{}/{}", m.len(), m.width() * m.height()), Font::tiny(), t.ink);
m.clear();
c.text(20, 9, if m.is_empty() { "cleared" } else { "not empty" }, Font::tiny(), t.panel);
```

## `Mask::width`

```rust
pub fn width(&self) -> i32
```

Width in dots.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-cols.svg">
  <img src="img/mask-cols-light.svg" alt="Mask::cols, Mask::rows, Mask::width, Mask::height, Mask::len, Mask::is_empty, Mask::clear" width="448">
</picture>

```rust
let mut m = Mask::new(28, 4); // 56 × 16 dots
m.draw(|c| c.disc(8.0, 8.0, 7.0, t.ink));
c.stencil(&m, BLUE);
c.text(20, 1, &format!("{}/{}", m.len(), m.width() * m.height()), Font::tiny(), t.ink);
m.clear();
c.text(20, 9, if m.is_empty() { "cleared" } else { "not empty" }, Font::tiny(), t.panel);
```

## `Mask::height`

```rust
pub fn height(&self) -> i32
```

Height in dots.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-cols.svg">
  <img src="img/mask-cols-light.svg" alt="Mask::cols, Mask::rows, Mask::width, Mask::height, Mask::len, Mask::is_empty, Mask::clear" width="448">
</picture>

```rust
let mut m = Mask::new(28, 4); // 56 × 16 dots
m.draw(|c| c.disc(8.0, 8.0, 7.0, t.ink));
c.stencil(&m, BLUE);
c.text(20, 1, &format!("{}/{}", m.len(), m.width() * m.height()), Font::tiny(), t.ink);
m.clear();
c.text(20, 9, if m.is_empty() { "cleared" } else { "not empty" }, Font::tiny(), t.panel);
```

## `Mask::contains`

```rust
pub fn contains(&self, x: i32, y: i32) -> bool
```

Whether dot `(x, y)` is set.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-new.svg">
  <img src="img/mask-new-light.svg" alt="Mask::new, Mask::set, Mask::unset, Mask::contains" width="384">
</picture>

```rust
// Dot by dot: a mask is a set of dots, written and read like a canvas
// without colours.
let mut m = Mask::new(24, 4);
for y in 0..16 {
    for x in 0..48 {
        if (x / 4 + y / 4) % 2 == 0 {
            m.set(x, y);
        }
    }
}
for x in 0..48 {
    m.unset(x, 8);
}
for y in 0..16 {
    for x in 0..48 {
        c.set(x, y, if m.contains(x, y) { CYAN } else { t.panel });
    }
}
```

## `Mask::set`

```rust
pub fn set(&mut self, x: i32, y: i32)
```

Sets dot `(x, y)`; out of range is ignored.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-new.svg">
  <img src="img/mask-new-light.svg" alt="Mask::new, Mask::set, Mask::unset, Mask::contains" width="384">
</picture>

```rust
// Dot by dot: a mask is a set of dots, written and read like a canvas
// without colours.
let mut m = Mask::new(24, 4);
for y in 0..16 {
    for x in 0..48 {
        if (x / 4 + y / 4) % 2 == 0 {
            m.set(x, y);
        }
    }
}
for x in 0..48 {
    m.unset(x, 8);
}
for y in 0..16 {
    for x in 0..48 {
        c.set(x, y, if m.contains(x, y) { CYAN } else { t.panel });
    }
}
```

## `Mask::unset`

```rust
pub fn unset(&mut self, x: i32, y: i32)
```

Unsets dot `(x, y)`.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-new.svg">
  <img src="img/mask-new-light.svg" alt="Mask::new, Mask::set, Mask::unset, Mask::contains" width="384">
</picture>

```rust
// Dot by dot: a mask is a set of dots, written and read like a canvas
// without colours.
let mut m = Mask::new(24, 4);
for y in 0..16 {
    for x in 0..48 {
        if (x / 4 + y / 4) % 2 == 0 {
            m.set(x, y);
        }
    }
}
for x in 0..48 {
    m.unset(x, 8);
}
for y in 0..16 {
    for x in 0..48 {
        c.set(x, y, if m.contains(x, y) { CYAN } else { t.panel });
    }
}
```

## `Mask::clear`

```rust
pub fn clear(&mut self)
```

Unsets everything. Keeps the allocation.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-cols.svg">
  <img src="img/mask-cols-light.svg" alt="Mask::cols, Mask::rows, Mask::width, Mask::height, Mask::len, Mask::is_empty, Mask::clear" width="448">
</picture>

```rust
let mut m = Mask::new(28, 4); // 56 × 16 dots
m.draw(|c| c.disc(8.0, 8.0, 7.0, t.ink));
c.stencil(&m, BLUE);
c.text(20, 1, &format!("{}/{}", m.len(), m.width() * m.height()), Font::tiny(), t.ink);
m.clear();
c.text(20, 9, if m.is_empty() { "cleared" } else { "not empty" }, Font::tiny(), t.panel);
```

## `Mask::is_empty`

```rust
pub fn is_empty(&self) -> bool
```

Whether no dot is set.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-cols.svg">
  <img src="img/mask-cols-light.svg" alt="Mask::cols, Mask::rows, Mask::width, Mask::height, Mask::len, Mask::is_empty, Mask::clear" width="448">
</picture>

```rust
let mut m = Mask::new(28, 4); // 56 × 16 dots
m.draw(|c| c.disc(8.0, 8.0, 7.0, t.ink));
c.stencil(&m, BLUE);
c.text(20, 1, &format!("{}/{}", m.len(), m.width() * m.height()), Font::tiny(), t.ink);
m.clear();
c.text(20, 9, if m.is_empty() { "cleared" } else { "not empty" }, Font::tiny(), t.panel);
```

## `Mask::len`

```rust
pub fn len(&self) -> usize
```

How many dots are set.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-cols.svg">
  <img src="img/mask-cols-light.svg" alt="Mask::cols, Mask::rows, Mask::width, Mask::height, Mask::len, Mask::is_empty, Mask::clear" width="448">
</picture>

```rust
let mut m = Mask::new(28, 4); // 56 × 16 dots
m.draw(|c| c.disc(8.0, 8.0, 7.0, t.ink));
c.stencil(&m, BLUE);
c.text(20, 1, &format!("{}/{}", m.len(), m.width() * m.height()), Font::tiny(), t.ink);
m.clear();
c.text(20, 9, if m.is_empty() { "cleared" } else { "not empty" }, Font::tiny(), t.panel);
```

## `Mask::bounds`

```rust
pub fn bounds(&self) -> Option<(i32, i32, i32, i32)>
```

The box `(x0, y0, x1, y1)` around the set dots, exact; `None` when empty.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-bounds.svg">
  <img src="img/mask-bounds-light.svg" alt="Mask::bounds, Mask::dots" width="384">
</picture>

```rust
let mut m = Mask::new(24, 4);
m.draw(|c| c.fill_star(22.0, 8.0, 7.5, 3.0, 5, -PI / 2.0, t.ink));
let (x0, y0, x1, y1) = m.bounds().unwrap(); // exact, exclusive on the far side
c.rect(x0 as f32, y0 as f32, (x1 - x0) as f32, (y1 - y0) as f32, 1.0, t.panel);
for (x, y) in m.dots() {
    // row-major
    c.set(x, y, YELLOW.lerp(RED, (y - y0) as f32 / (y1 - y0) as f32));
}
```

## `Mask::dots`

```rust
pub fn dots(&self) -> impl Iterator<Item = (i32, i32)> + '_
```

The set dots, row-major.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-bounds.svg">
  <img src="img/mask-bounds-light.svg" alt="Mask::bounds, Mask::dots" width="384">
</picture>

```rust
let mut m = Mask::new(24, 4);
m.draw(|c| c.fill_star(22.0, 8.0, 7.5, 3.0, 5, -PI / 2.0, t.ink));
let (x0, y0, x1, y1) = m.bounds().unwrap(); // exact, exclusive on the far side
c.rect(x0 as f32, y0 as f32, (x1 - x0) as f32, (y1 - y0) as f32, 1.0, t.panel);
for (x, y) in m.dots() {
    // row-major
    c.set(x, y, YELLOW.lerp(RED, (y - y0) as f32 / (y1 - y0) as f32));
}
```

## `Mask::draw`

```rust
pub fn draw(&mut self, f: impl FnOnce(&mut Canvas))
```

Runs `f` with a scratch canvas of the mask's size and adds every dot it set:
drawing a shape *is* drawing it on a canvas, with any primitive and any paint
(a dithered paint sets only the dots its dither lands on, and
[`Paint::erase`](draw.md#painterase) unsets, within this call). The scratch
is kept per thread, so this allocates once.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-draw.svg">
  <img src="img/mask-draw-light.svg" alt="Mask::draw" width="384">
</picture>

```rust
let mut m = Mask::new(24, 4);
m.draw(|c| {
    c.fill_star(10.0, 8.0, 7.5, 3.5, 5, -PI / 2.0, t.ink);          // any primitive
    c.fill_rect(20.0, 2.0, 26.0, 12.0, Paint::dithered(t.ink, 0.5)); // any paint
});
c.stencil(&m, GREEN);
```

## `Mask::erase`

```rust
pub fn erase(&mut self, f: impl FnOnce(&mut Canvas))
```

Runs `f` as [`draw`](mask.md#maskdraw) does and removes every dot it set.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-erase.svg">
  <img src="img/mask-erase-light.svg" alt="Mask::erase" width="384">
</picture>

```rust
let mut m = Mask::new(24, 4);
m.draw(|c| c.fill_round_rect(2.0, 1.0, 44.0, 14.0, 4.0, t.ink));
m.erase(|c| {
    for i in 0..5 {
        c.disc(8.0 + 8.0 * i as f32, 8.0, 3.0, t.ink); // any drawing, removed
    }
});
c.stencil(&m, PURPLE);
```

## `Mask::union`

```rust
pub fn union(&mut self, other: &Mask)
```

Adds every dot of `other`.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-subtract.svg">
  <img src="img/mask-subtract-light.svg" alt="Mask::subtract, Mask::union, Mask::intersect" width="384">
</picture>

```rust
let mut a = Mask::new(24, 4);
a.draw(|c| c.disc(10.0, 8.0, 7.0, t.ink));
let mut b = Mask::new(24, 4);
b.draw(|c| c.disc(15.0, 8.0, 7.0, t.ink));
let (mut union, mut cut, mut both) = (a.clone(), a.clone(), a.clone());
union.union(&b);
cut.subtract(&b);
both.intersect(&b);
union.translate(0, 0);
cut.translate(20, 0);
both.translate(34, 0);
c.stencil(&union, RED);
c.stencil(&cut, YELLOW);
c.stencil(&both, BLUE);
```

## `Mask::subtract`

```rust
pub fn subtract(&mut self, other: &Mask)
```

Removes every dot of `other`.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-subtract.svg">
  <img src="img/mask-subtract-light.svg" alt="Mask::subtract, Mask::union, Mask::intersect" width="384">
</picture>

```rust
let mut a = Mask::new(24, 4);
a.draw(|c| c.disc(10.0, 8.0, 7.0, t.ink));
let mut b = Mask::new(24, 4);
b.draw(|c| c.disc(15.0, 8.0, 7.0, t.ink));
let (mut union, mut cut, mut both) = (a.clone(), a.clone(), a.clone());
union.union(&b);
cut.subtract(&b);
both.intersect(&b);
union.translate(0, 0);
cut.translate(20, 0);
both.translate(34, 0);
c.stencil(&union, RED);
c.stencil(&cut, YELLOW);
c.stencil(&both, BLUE);
```

## `Mask::intersect`

```rust
pub fn intersect(&mut self, other: &Mask)
```

Keeps only the dots also in `other`.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-subtract.svg">
  <img src="img/mask-subtract-light.svg" alt="Mask::subtract, Mask::union, Mask::intersect" width="384">
</picture>

```rust
let mut a = Mask::new(24, 4);
a.draw(|c| c.disc(10.0, 8.0, 7.0, t.ink));
let mut b = Mask::new(24, 4);
b.draw(|c| c.disc(15.0, 8.0, 7.0, t.ink));
let (mut union, mut cut, mut both) = (a.clone(), a.clone(), a.clone());
union.union(&b);
cut.subtract(&b);
both.intersect(&b);
union.translate(0, 0);
cut.translate(20, 0);
both.translate(34, 0);
c.stencil(&union, RED);
c.stencil(&cut, YELLOW);
c.stencil(&both, BLUE);
```

## `Mask::boundary_with`

```rust
pub fn boundary_with(&self, other: &Mask) -> Mask
```

The dots of this mask that touch `other` (eight neighbours): the seam where
two shapes meet, such as a collar between a head and a body.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-boundary_with.svg">
  <img src="img/mask-boundary_with-light.svg" alt="Mask::boundary_with" width="384">
</picture>

```rust
let mut body = Mask::new(24, 4);
body.draw(|c| c.fill_ellipse(22.0, 10.0, 12.0, 5.5, t.ink));
let mut head = Mask::new(24, 4);
head.draw(|c| c.fill_ellipse(32.0, 5.0, 5.0, 4.5, t.ink));
body.subtract(&head);
c.stencil(&body, ORANGE);
c.stencil(&head, GREEN);
// The seam where the two meet: a collar, drawn on the body's side.
c.stencil(&body.boundary_with(&head), t.ink);
```

## `Mask::translate`

```rust
pub fn translate(&mut self, dx: i32, dy: i32)
```

Moves the mask by whole dots. Dots moved off the mask are lost.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-translate.svg">
  <img src="img/mask-translate-light.svg" alt="Mask::translate, Mask::flip_y" width="384">
</picture>

```rust
let mut fish = Mask::new(24, 4);
fish.draw(|c| {
    c.fill_ellipse(9.0, 5.0, 6.0, 3.0, t.ink);
    c.fill_polygon(&[(9.0, 5.0), (2.0, 1.0), (2.0, 9.0)], t.ink);
});
c.stencil(&fish, BLUE);
fish.translate(16, 0); // moved right by whole dots
c.stencil(&fish, CYAN);
fish.translate(16, 0);
fish.flip_y(8.0); // mirrored about the middle row
c.stencil(&fish, GREEN);
```

## `Mask::flip_x`

```rust
pub fn flip_x(&mut self, x: f32)
```

Mirrors left-to-right about the column `x` dots from the left edge (`x` is
the mirror line, so a mask spanning `2..8` flipped about `5.0` spans `2..8`).

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-flip_x.svg">
  <img src="img/mask-flip_x-light.svg" alt="Mask::flip_x" width="384">
</picture>

```rust
let mut fish = Mask::new(24, 4);
fish.draw(|c| {
    c.fill_ellipse(12.0, 8.0, 8.0, 4.0, t.ink);
    c.fill_polygon(&[(4.0, 8.0), (0.0, 3.0), (0.0, 13.0)], t.ink);
});
c.stencil(&fish, BLUE);
fish.flip_x(24.0); // mirrored about the middle of the canvas: facing left
c.stencil(&fish, GREEN);
```

## `Mask::flip_y`

```rust
pub fn flip_y(&mut self, y: f32)
```

Mirrors top-to-bottom about the row `y` dots from the top.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-translate.svg">
  <img src="img/mask-translate-light.svg" alt="Mask::translate, Mask::flip_y" width="384">
</picture>

```rust
let mut fish = Mask::new(24, 4);
fish.draw(|c| {
    c.fill_ellipse(9.0, 5.0, 6.0, 3.0, t.ink);
    c.fill_polygon(&[(9.0, 5.0), (2.0, 1.0), (2.0, 9.0)], t.ink);
});
c.stencil(&fish, BLUE);
fish.translate(16, 0); // moved right by whole dots
c.stencil(&fish, CYAN);
fish.translate(16, 0);
fish.flip_y(8.0); // mirrored about the middle row
c.stencil(&fish, GREEN);
```

## `Mask::transform`

```rust
pub fn transform(&mut self, t: &Transform)
```

Applies `t` to every dot: an integer move or flip is exact, anything else
(a rotation, a scale) resamples with the nearest dot, which on a dot matrix
is the honest answer. Dots that land off the mask are lost.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/mask-transform.svg">
  <img src="img/mask-transform-light.svg" alt="Mask::transform" width="384">
</picture>

```rust
// One limb, built once, hung from its pivot at three angles.
let mut limb = Mask::new(24, 4);
limb.draw(|c| {
    c.fill_round_rect(2.5, 1.0, 3.0, 10.0, 1.5, t.ink);
    c.disc(4.0, 12.0, 2.5, t.ink);
});
for (i, angle) in [-0.5, 0.0, 0.5].into_iter().enumerate() {
    let mut placed = limb.clone();
    placed.transform(&Transform::at(6.0 + 14.0 * i as f32, 0.0).rotate_about(angle, (4.0, 1.0)));
    c.stencil(&placed, CYAN.lerp(PURPLE, i as f32 / 2.0));
}
```

[Index](README.md) · [canvas](canvas.md) · [draw](draw.md) · [path](path.md) · **mask** · [transform](transform.md) · [rig](rig.md) · [layer](layer.md) · [bubble](bubble.md) · [font](font.md) · [text](text.md) · [color](color.md) · [render](render.md) · [term](term.md) · [export](export.md) · [ratatui](ratatui.md)

