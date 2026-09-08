# `text`

[Index](README.md) · [canvas](canvas.md) · [draw](draw.md) · [path](path.md) · [mask](mask.md) · [transform](transform.md) · [rig](rig.md) · [layer](layer.md) · [bubble](bubble.md) · [font](font.md) · **text** · [color](color.md) · [render](render.md) · [term](term.md) · [export](export.md) · [ratatui](ratatui.md)

Real terminal text on a canvas: characters, not dots.

[`Canvas::text`](font.md#canvastext) draws with a bitmap [`Font`](font.md#font), so a
label is made of dots like everything else and can be any size or colour. That is
the right answer for a title inside a drawing and the wrong one for a paragraph:
at 3×5 dots a sentence is barely legible, and it is not text any more, so it
cannot be copied out of the terminal or read by a screen reader.

So a canvas also has a *text layer*: one real character per terminal cell, with
its own colours and attributes, drawn by [`Canvas::print`](text.md#canvasprint). A cell holding a
character shows that character instead of its eight dots — in the text protocol,
in the image protocols (which leave the cell transparent and print over it), in
[`Canvas::to_text`](canvas.md#canvasto_text) and in the ratatui widget. Everything the terminal's own font
can draw works: ASCII, box drawing, CJK, emoji.

```rust
use cobra::{Canvas, Rgb, TextStyle};

let mut c = Canvas::new(20, 3);
c.fill_rect(0.0, 0.0, 40.0, 12.0, Rgb::hex(0x1b2430));   // dots behind the text
c.print(1, 1, "Ready.", TextStyle::new(Rgb::hex(0xc9d1d9)).bold());
```

Cell coordinates here, dot coordinates everywhere else: one cell is
[`DOTS_X`](canvas.md#dots_x) × [`DOTS_Y`](canvas.md#dots_y) dots.

## Contents

- [`Attrs`](#attrs)
- [`TextStyle`](#textstyle)
- [`TextCell`](#textcell)
- [`Align`](#align)
- [`Wrap`](#wrap)
- [`char_width`](#char_width)
- [`width`](#width)
- [`measure`](#measure)
- [`wrap`](#wrap)
- `Attrs`: [`Attrs::NONE`](text.md#attrsnone), [`Attrs::BOLD`](text.md#attrsbold), [`Attrs::DIM`](text.md#attrsdim), [`Attrs::ITALIC`](text.md#attrsitalic), [`Attrs::UNDERLINE`](text.md#attrsunderline), [`Attrs::REVERSE`](text.md#attrsreverse), [`Attrs::has`](text.md#attrshas)
- `TextStyle`: [`TextStyle::new`](text.md#textstylenew), [`TextStyle::on`](text.md#textstyleon), [`TextStyle::with`](text.md#textstylewith), [`TextStyle::bold`](text.md#textstylebold), [`TextStyle::dim`](text.md#textstyledim), [`TextStyle::italic`](text.md#textstyleitalic), [`TextStyle::underline`](text.md#textstyleunderline)
- `TextCell`: [`TextCell::CONTINUATION`](text.md#textcellcontinuation), [`TextCell::is_empty`](text.md#textcellis_empty), [`TextCell::is_continuation`](text.md#textcellis_continuation)
- `Canvas`: [`Canvas::print`](text.md#canvasprint), [`Canvas::print_wrapped`](text.md#canvasprint_wrapped), [`Canvas::text_cell`](text.md#canvastext_cell), [`Canvas::erase_text`](text.md#canvaserase_text), [`Canvas::clear_text`](text.md#canvasclear_text), [`Canvas::has_text`](text.md#canvashas_text)

## `Attrs`

```rust
pub struct Attrs(pub u8)
```

Text attributes, as SGR bits. Combine with `|`.

```rust
let a = Attrs::BOLD | Attrs::UNDERLINE;
assert!(a.has(Attrs::BOLD) && !a.has(Attrs::ITALIC));
```

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/attrs.svg">
  <img src="img/attrs-light.svg" alt="Attrs, Attrs::NONE, Attrs::BOLD, Attrs::DIM, Attrs::ITALIC, Attrs::UNDERLINE, Attrs::REVERSE, Attrs::has, TextStyle::with" width="512">
</picture>

```rust
let all = [
    ("none", Attrs::NONE),
    ("bold", Attrs::BOLD),
    ("dim", Attrs::DIM),
    ("italic", Attrs::ITALIC),
    ("under", Attrs::UNDERLINE),
    ("rev", Attrs::REVERSE),
];
let mut col = 1;
for (name, attrs) in all {
    let ink = if attrs.has(Attrs::REVERSE) { RED } else { t.ink };
    col += c.print(col, 1, name, TextStyle::new(ink).with(attrs)) + 1;
}
```

## `TextStyle`

```rust
pub struct TextStyle
```

How a character is drawn: foreground, background and attributes, each optional so
the terminal's own defaults show through.

Anything that is a [`Color`](color.md#color) converts into a `TextStyle` foreground, so
`canvas.print(0, 0, "hi", Rgb::hex(0xff0055))` works.

- `pub fg: Option<Color>` — Ink; `None` is the terminal's default foreground.
- `pub bg: Option<Color>` — Cell background; `None` leaves whatever is behind it.
- `pub attrs: Attrs` — Bold, italic and friends.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/textstyle.svg">
  <img src="img/textstyle-light.svg" alt="TextStyle, TextStyle::new, TextStyle::on, TextStyle::bold, TextStyle::dim, TextStyle::italic, TextStyle::underline" width="480">
</picture>

```rust
c.print(1, 1, "bold", TextStyle::new(t.ink).bold());
c.print(6, 1, "dim", TextStyle::new(t.ink).dim());
c.print(10, 1, "italic", TextStyle::new(BLUE).italic());
c.print(17, 1, "under", TextStyle::new(GREEN).underline());
c.print(23, 1, "on bg", TextStyle::new(t.bg).on(RED).with(Attrs::BOLD));
```

## `TextCell`

```rust
pub struct TextCell
```

One cell of the text layer.

- `pub ch: char` — The character; `'\0'` for an empty cell and [`CONTINUATION`](text.md#textcellcontinuation) for the right half of a double-width one.
- `pub style: TextStyle` — How to draw it.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/textcell.svg">
  <img src="img/textcell-light.svg" alt="TextCell, TextCell::CONTINUATION, TextCell::is_empty, TextCell::is_continuation, Canvas::text_cell" width="192">
</picture>

```rust
// A wide character takes two cells: its own and a continuation. Under each
// cell, what `text_cell` reports about it: empty, a character, or the
// right half of one.
c.print(1, 0, "a日b", t.ink);
for col in 0..6 {
    let color = match c.text_cell(col, 0) {
        None => t.panel,
        Some(cell) if cell.is_continuation() => ORANGE,
        Some(_) => GREEN,
    };
    c.fill_rect(col as f32 * 2.0, 5.0, 2.0, 3.0, color);
}
```

## `Align`

```rust
pub enum Align
```

Horizontal alignment for [`Canvas::print_wrapped`](text.md#canvasprint_wrapped) and text inside a
[`Bubble`](bubble.md#bubble).

- `Left` — Against the left edge.
- `Center` — Centred, extra space split evenly.
- `Right` — Against the right edge.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/align.svg">
  <img src="img/align-light.svg" alt="Align" width="384">
</picture>

```rust
c.print_wrapped(0, 0, 24, "left", t.ink, Align::Left);
c.print_wrapped(0, 1, 24, "centre", t.ink, Align::Center);
c.print_wrapped(0, 2, 24, "right", t.ink, Align::Right);
```

## `Wrap`

```rust
pub struct Wrap<'a>
```

Iterator of wrapped lines; see [`wrap`](text.md#wrap).

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/wrap.svg">
  <img src="img/wrap-light.svg" alt="wrap, Wrap, width, measure, char_width" width="384">
</picture>

```rust
// Lines wrapped at spaces to a width in cells, each with a bar as wide as
// `width` says it is: the 日本 counts double.
let paragraph = "Lines wrap at spaces; 日本 is wide.";
for (i, line) in text::wrap(paragraph, 14).enumerate() {
    c.print(1, i as i32, line, t.ink);
    c.fill_rect(32.0, i as f32 * 4.0 + 1.0, text::width(line) as f32, 2.0, BLUE);
}
let (w, lines) = text::measure(paragraph); // unwrapped
c.text(32, 13, &format!("{w}x{lines}"), Font::tiny(), t.panel);
```

## `char_width`

```rust
pub fn char_width(ch: char) -> i32
```

Cells a character occupies: `0` for a combining mark, `2` for the wide ranges of
East Asian scripts and emoji, `1` otherwise.

This is the common part of Unicode's East Asian Width, coarse enough to be a
handful of range checks and right for everything a terminal usually shows.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/wrap.svg">
  <img src="img/wrap-light.svg" alt="wrap, Wrap, width, measure, char_width" width="384">
</picture>

```rust
// Lines wrapped at spaces to a width in cells, each with a bar as wide as
// `width` says it is: the 日本 counts double.
let paragraph = "Lines wrap at spaces; 日本 is wide.";
for (i, line) in text::wrap(paragraph, 14).enumerate() {
    c.print(1, i as i32, line, t.ink);
    c.fill_rect(32.0, i as f32 * 4.0 + 1.0, text::width(line) as f32, 2.0, BLUE);
}
let (w, lines) = text::measure(paragraph); // unwrapped
c.text(32, 13, &format!("{w}x{lines}"), Font::tiny(), t.panel);
```

## `width`

```rust
pub fn width(text: &str) -> i32
```

Width of `text` in cells: the widest of its newline-separated lines.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/wrap.svg">
  <img src="img/wrap-light.svg" alt="wrap, Wrap, width, measure, char_width" width="384">
</picture>

```rust
// Lines wrapped at spaces to a width in cells, each with a bar as wide as
// `width` says it is: the 日本 counts double.
let paragraph = "Lines wrap at spaces; 日本 is wide.";
for (i, line) in text::wrap(paragraph, 14).enumerate() {
    c.print(1, i as i32, line, t.ink);
    c.fill_rect(32.0, i as f32 * 4.0 + 1.0, text::width(line) as f32, 2.0, BLUE);
}
let (w, lines) = text::measure(paragraph); // unwrapped
c.text(32, 13, &format!("{w}x{lines}"), Font::tiny(), t.panel);
```

## `measure`

```rust
pub fn measure(text: &str) -> (i32, i32)
```

Size of `text` in cells, `(width, lines)`.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/wrap.svg">
  <img src="img/wrap-light.svg" alt="wrap, Wrap, width, measure, char_width" width="384">
</picture>

```rust
// Lines wrapped at spaces to a width in cells, each with a bar as wide as
// `width` says it is: the 日本 counts double.
let paragraph = "Lines wrap at spaces; 日本 is wide.";
for (i, line) in text::wrap(paragraph, 14).enumerate() {
    c.print(1, i as i32, line, t.ink);
    c.fill_rect(32.0, i as f32 * 4.0 + 1.0, text::width(line) as f32, 2.0, BLUE);
}
let (w, lines) = text::measure(paragraph); // unwrapped
c.text(32, 13, &format!("{w}x{lines}"), Font::tiny(), t.panel);
```

## `wrap`

```rust
pub fn wrap(text: &str, width: i32) -> Wrap<'_>
```

Splits `text` into lines at most `width` cells wide, breaking at spaces where it
can and inside a word where it cannot. Existing newlines always break. A `width`
of `0` or less only splits on newlines.

The iterator borrows `text` and allocates nothing.

```rust
let lines: Vec<&str> = wrap("the quick brown fox", 9).collect();
assert_eq!(lines, ["the quick", "brown fox"]);
```

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/wrap.svg">
  <img src="img/wrap-light.svg" alt="wrap, Wrap, width, measure, char_width" width="384">
</picture>

```rust
// Lines wrapped at spaces to a width in cells, each with a bar as wide as
// `width` says it is: the 日本 counts double.
let paragraph = "Lines wrap at spaces; 日本 is wide.";
for (i, line) in text::wrap(paragraph, 14).enumerate() {
    c.print(1, i as i32, line, t.ink);
    c.fill_rect(32.0, i as f32 * 4.0 + 1.0, text::width(line) as f32, 2.0, BLUE);
}
let (w, lines) = text::measure(paragraph); // unwrapped
c.text(32, 13, &format!("{w}x{lines}"), Font::tiny(), t.panel);
```

## `Attrs` methods

## `Attrs::NONE`

```rust
pub const NONE: Attrs = Attrs(0)
```

No attributes.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/attrs.svg">
  <img src="img/attrs-light.svg" alt="Attrs, Attrs::NONE, Attrs::BOLD, Attrs::DIM, Attrs::ITALIC, Attrs::UNDERLINE, Attrs::REVERSE, Attrs::has, TextStyle::with" width="512">
</picture>

```rust
let all = [
    ("none", Attrs::NONE),
    ("bold", Attrs::BOLD),
    ("dim", Attrs::DIM),
    ("italic", Attrs::ITALIC),
    ("under", Attrs::UNDERLINE),
    ("rev", Attrs::REVERSE),
];
let mut col = 1;
for (name, attrs) in all {
    let ink = if attrs.has(Attrs::REVERSE) { RED } else { t.ink };
    col += c.print(col, 1, name, TextStyle::new(ink).with(attrs)) + 1;
}
```

## `Attrs::BOLD`

```rust
pub const BOLD: Attrs = Attrs(1)
```

`SGR 1`.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/attrs.svg">
  <img src="img/attrs-light.svg" alt="Attrs, Attrs::NONE, Attrs::BOLD, Attrs::DIM, Attrs::ITALIC, Attrs::UNDERLINE, Attrs::REVERSE, Attrs::has, TextStyle::with" width="512">
</picture>

```rust
let all = [
    ("none", Attrs::NONE),
    ("bold", Attrs::BOLD),
    ("dim", Attrs::DIM),
    ("italic", Attrs::ITALIC),
    ("under", Attrs::UNDERLINE),
    ("rev", Attrs::REVERSE),
];
let mut col = 1;
for (name, attrs) in all {
    let ink = if attrs.has(Attrs::REVERSE) { RED } else { t.ink };
    col += c.print(col, 1, name, TextStyle::new(ink).with(attrs)) + 1;
}
```

## `Attrs::DIM`

```rust
pub const DIM: Attrs = Attrs(2)
```

`SGR 2`.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/attrs.svg">
  <img src="img/attrs-light.svg" alt="Attrs, Attrs::NONE, Attrs::BOLD, Attrs::DIM, Attrs::ITALIC, Attrs::UNDERLINE, Attrs::REVERSE, Attrs::has, TextStyle::with" width="512">
</picture>

```rust
let all = [
    ("none", Attrs::NONE),
    ("bold", Attrs::BOLD),
    ("dim", Attrs::DIM),
    ("italic", Attrs::ITALIC),
    ("under", Attrs::UNDERLINE),
    ("rev", Attrs::REVERSE),
];
let mut col = 1;
for (name, attrs) in all {
    let ink = if attrs.has(Attrs::REVERSE) { RED } else { t.ink };
    col += c.print(col, 1, name, TextStyle::new(ink).with(attrs)) + 1;
}
```

## `Attrs::ITALIC`

```rust
pub const ITALIC: Attrs = Attrs(4)
```

`SGR 3`.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/attrs.svg">
  <img src="img/attrs-light.svg" alt="Attrs, Attrs::NONE, Attrs::BOLD, Attrs::DIM, Attrs::ITALIC, Attrs::UNDERLINE, Attrs::REVERSE, Attrs::has, TextStyle::with" width="512">
</picture>

```rust
let all = [
    ("none", Attrs::NONE),
    ("bold", Attrs::BOLD),
    ("dim", Attrs::DIM),
    ("italic", Attrs::ITALIC),
    ("under", Attrs::UNDERLINE),
    ("rev", Attrs::REVERSE),
];
let mut col = 1;
for (name, attrs) in all {
    let ink = if attrs.has(Attrs::REVERSE) { RED } else { t.ink };
    col += c.print(col, 1, name, TextStyle::new(ink).with(attrs)) + 1;
}
```

## `Attrs::UNDERLINE`

```rust
pub const UNDERLINE: Attrs = Attrs(8)
```

`SGR 4`.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/attrs.svg">
  <img src="img/attrs-light.svg" alt="Attrs, Attrs::NONE, Attrs::BOLD, Attrs::DIM, Attrs::ITALIC, Attrs::UNDERLINE, Attrs::REVERSE, Attrs::has, TextStyle::with" width="512">
</picture>

```rust
let all = [
    ("none", Attrs::NONE),
    ("bold", Attrs::BOLD),
    ("dim", Attrs::DIM),
    ("italic", Attrs::ITALIC),
    ("under", Attrs::UNDERLINE),
    ("rev", Attrs::REVERSE),
];
let mut col = 1;
for (name, attrs) in all {
    let ink = if attrs.has(Attrs::REVERSE) { RED } else { t.ink };
    col += c.print(col, 1, name, TextStyle::new(ink).with(attrs)) + 1;
}
```

## `Attrs::REVERSE`

```rust
pub const REVERSE: Attrs = Attrs(16)
```

`SGR 7`.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/attrs.svg">
  <img src="img/attrs-light.svg" alt="Attrs, Attrs::NONE, Attrs::BOLD, Attrs::DIM, Attrs::ITALIC, Attrs::UNDERLINE, Attrs::REVERSE, Attrs::has, TextStyle::with" width="512">
</picture>

```rust
let all = [
    ("none", Attrs::NONE),
    ("bold", Attrs::BOLD),
    ("dim", Attrs::DIM),
    ("italic", Attrs::ITALIC),
    ("under", Attrs::UNDERLINE),
    ("rev", Attrs::REVERSE),
];
let mut col = 1;
for (name, attrs) in all {
    let ink = if attrs.has(Attrs::REVERSE) { RED } else { t.ink };
    col += c.print(col, 1, name, TextStyle::new(ink).with(attrs)) + 1;
}
```

## `Attrs::has`

```rust
pub const fn has(self, other: Attrs) -> bool
```

Whether every bit of `other` is set.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/attrs.svg">
  <img src="img/attrs-light.svg" alt="Attrs, Attrs::NONE, Attrs::BOLD, Attrs::DIM, Attrs::ITALIC, Attrs::UNDERLINE, Attrs::REVERSE, Attrs::has, TextStyle::with" width="512">
</picture>

```rust
let all = [
    ("none", Attrs::NONE),
    ("bold", Attrs::BOLD),
    ("dim", Attrs::DIM),
    ("italic", Attrs::ITALIC),
    ("under", Attrs::UNDERLINE),
    ("rev", Attrs::REVERSE),
];
let mut col = 1;
for (name, attrs) in all {
    let ink = if attrs.has(Attrs::REVERSE) { RED } else { t.ink };
    col += c.print(col, 1, name, TextStyle::new(ink).with(attrs)) + 1;
}
```

## `TextStyle` methods

## `TextStyle::new`

```rust
pub fn new(fg: impl Into<Color>) -> Self
```

Style with `fg` as its ink.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/textstyle.svg">
  <img src="img/textstyle-light.svg" alt="TextStyle, TextStyle::new, TextStyle::on, TextStyle::bold, TextStyle::dim, TextStyle::italic, TextStyle::underline" width="480">
</picture>

```rust
c.print(1, 1, "bold", TextStyle::new(t.ink).bold());
c.print(6, 1, "dim", TextStyle::new(t.ink).dim());
c.print(10, 1, "italic", TextStyle::new(BLUE).italic());
c.print(17, 1, "under", TextStyle::new(GREEN).underline());
c.print(23, 1, "on bg", TextStyle::new(t.bg).on(RED).with(Attrs::BOLD));
```

## `TextStyle::on`

```rust
pub fn on(mut self, bg: impl Into<Color>) -> Self
```

Sets the background.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/textstyle.svg">
  <img src="img/textstyle-light.svg" alt="TextStyle, TextStyle::new, TextStyle::on, TextStyle::bold, TextStyle::dim, TextStyle::italic, TextStyle::underline" width="480">
</picture>

```rust
c.print(1, 1, "bold", TextStyle::new(t.ink).bold());
c.print(6, 1, "dim", TextStyle::new(t.ink).dim());
c.print(10, 1, "italic", TextStyle::new(BLUE).italic());
c.print(17, 1, "under", TextStyle::new(GREEN).underline());
c.print(23, 1, "on bg", TextStyle::new(t.bg).on(RED).with(Attrs::BOLD));
```

## `TextStyle::with`

```rust
pub fn with(mut self, attrs: Attrs) -> Self
```

Adds attributes.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/attrs.svg">
  <img src="img/attrs-light.svg" alt="Attrs, Attrs::NONE, Attrs::BOLD, Attrs::DIM, Attrs::ITALIC, Attrs::UNDERLINE, Attrs::REVERSE, Attrs::has, TextStyle::with" width="512">
</picture>

```rust
let all = [
    ("none", Attrs::NONE),
    ("bold", Attrs::BOLD),
    ("dim", Attrs::DIM),
    ("italic", Attrs::ITALIC),
    ("under", Attrs::UNDERLINE),
    ("rev", Attrs::REVERSE),
];
let mut col = 1;
for (name, attrs) in all {
    let ink = if attrs.has(Attrs::REVERSE) { RED } else { t.ink };
    col += c.print(col, 1, name, TextStyle::new(ink).with(attrs)) + 1;
}
```

## `TextStyle::bold`

```rust
pub fn bold(self) -> Self
```

Adds [`Attrs::BOLD`](text.md#attrsbold).

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/textstyle.svg">
  <img src="img/textstyle-light.svg" alt="TextStyle, TextStyle::new, TextStyle::on, TextStyle::bold, TextStyle::dim, TextStyle::italic, TextStyle::underline" width="480">
</picture>

```rust
c.print(1, 1, "bold", TextStyle::new(t.ink).bold());
c.print(6, 1, "dim", TextStyle::new(t.ink).dim());
c.print(10, 1, "italic", TextStyle::new(BLUE).italic());
c.print(17, 1, "under", TextStyle::new(GREEN).underline());
c.print(23, 1, "on bg", TextStyle::new(t.bg).on(RED).with(Attrs::BOLD));
```

## `TextStyle::dim`

```rust
pub fn dim(self) -> Self
```

Adds [`Attrs::DIM`](text.md#attrsdim).

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/textstyle.svg">
  <img src="img/textstyle-light.svg" alt="TextStyle, TextStyle::new, TextStyle::on, TextStyle::bold, TextStyle::dim, TextStyle::italic, TextStyle::underline" width="480">
</picture>

```rust
c.print(1, 1, "bold", TextStyle::new(t.ink).bold());
c.print(6, 1, "dim", TextStyle::new(t.ink).dim());
c.print(10, 1, "italic", TextStyle::new(BLUE).italic());
c.print(17, 1, "under", TextStyle::new(GREEN).underline());
c.print(23, 1, "on bg", TextStyle::new(t.bg).on(RED).with(Attrs::BOLD));
```

## `TextStyle::italic`

```rust
pub fn italic(self) -> Self
```

Adds [`Attrs::ITALIC`](text.md#attrsitalic).

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/textstyle.svg">
  <img src="img/textstyle-light.svg" alt="TextStyle, TextStyle::new, TextStyle::on, TextStyle::bold, TextStyle::dim, TextStyle::italic, TextStyle::underline" width="480">
</picture>

```rust
c.print(1, 1, "bold", TextStyle::new(t.ink).bold());
c.print(6, 1, "dim", TextStyle::new(t.ink).dim());
c.print(10, 1, "italic", TextStyle::new(BLUE).italic());
c.print(17, 1, "under", TextStyle::new(GREEN).underline());
c.print(23, 1, "on bg", TextStyle::new(t.bg).on(RED).with(Attrs::BOLD));
```

## `TextStyle::underline`

```rust
pub fn underline(self) -> Self
```

Adds [`Attrs::UNDERLINE`](text.md#attrsunderline).

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/textstyle.svg">
  <img src="img/textstyle-light.svg" alt="TextStyle, TextStyle::new, TextStyle::on, TextStyle::bold, TextStyle::dim, TextStyle::italic, TextStyle::underline" width="480">
</picture>

```rust
c.print(1, 1, "bold", TextStyle::new(t.ink).bold());
c.print(6, 1, "dim", TextStyle::new(t.ink).dim());
c.print(10, 1, "italic", TextStyle::new(BLUE).italic());
c.print(17, 1, "under", TextStyle::new(GREEN).underline());
c.print(23, 1, "on bg", TextStyle::new(t.bg).on(RED).with(Attrs::BOLD));
```

## `TextCell` methods

## `TextCell::CONTINUATION`

```rust
pub const CONTINUATION: char = '\u{1}'
```

Placeholder in the cell that a double-width character spills into.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/textcell.svg">
  <img src="img/textcell-light.svg" alt="TextCell, TextCell::CONTINUATION, TextCell::is_empty, TextCell::is_continuation, Canvas::text_cell" width="192">
</picture>

```rust
// A wide character takes two cells: its own and a continuation. Under each
// cell, what `text_cell` reports about it: empty, a character, or the
// right half of one.
c.print(1, 0, "a日b", t.ink);
for col in 0..6 {
    let color = match c.text_cell(col, 0) {
        None => t.panel,
        Some(cell) if cell.is_continuation() => ORANGE,
        Some(_) => GREEN,
    };
    c.fill_rect(col as f32 * 2.0, 5.0, 2.0, 3.0, color);
}
```

## `TextCell::is_empty`

```rust
pub const fn is_empty(&self) -> bool
```

Whether no character was printed here.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/textcell.svg">
  <img src="img/textcell-light.svg" alt="TextCell, TextCell::CONTINUATION, TextCell::is_empty, TextCell::is_continuation, Canvas::text_cell" width="192">
</picture>

```rust
// A wide character takes two cells: its own and a continuation. Under each
// cell, what `text_cell` reports about it: empty, a character, or the
// right half of one.
c.print(1, 0, "a日b", t.ink);
for col in 0..6 {
    let color = match c.text_cell(col, 0) {
        None => t.panel,
        Some(cell) if cell.is_continuation() => ORANGE,
        Some(_) => GREEN,
    };
    c.fill_rect(col as f32 * 2.0, 5.0, 2.0, 3.0, color);
}
```

## `TextCell::is_continuation`

```rust
pub const fn is_continuation(&self) -> bool
```

Whether this is the right half of a double-width character.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/textcell.svg">
  <img src="img/textcell-light.svg" alt="TextCell, TextCell::CONTINUATION, TextCell::is_empty, TextCell::is_continuation, Canvas::text_cell" width="192">
</picture>

```rust
// A wide character takes two cells: its own and a continuation. Under each
// cell, what `text_cell` reports about it: empty, a character, or the
// right half of one.
c.print(1, 0, "a日b", t.ink);
for col in 0..6 {
    let color = match c.text_cell(col, 0) {
        None => t.panel,
        Some(cell) if cell.is_continuation() => ORANGE,
        Some(_) => GREEN,
    };
    c.fill_rect(col as f32 * 2.0, 5.0, 2.0, 3.0, color);
}
```

## `Canvas` methods

## `Canvas::print`

```rust
pub fn print(&mut self, col: i32, row: i32, text: &str, style: impl Into<TextStyle>) -> i32
```

Prints `text` with its first character in cell `(col, row)`, returning the
number of cells the widest line took.

Newlines start a new line at `col`. Double-width characters take two cells;
zero-width ones are dropped. Characters landing outside the canvas are
skipped. A printed cell hides the dots underneath it, so `style.bg` is how a
label keeps the colour of the shape it sits on.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/canvas-print.svg">
  <img src="img/canvas-print-light.svg" alt="Canvas::print" width="384">
</picture>

```rust
c.fill_rect(0.0, 0.0, 48.0, 12.0, Paint::pattern(t.panel, Pattern::Checker(2)));
c.print(1, 1, "Real text: 日本語 ✓", TextStyle::new(t.ink).bold());
```

## `Canvas::print_wrapped`

```rust
pub fn print_wrapped(&mut self, col: i32, row: i32, width: i32, text: &str, style: impl Into<TextStyle>, align: Align) -> (i32, i32)
```

[`print`](text.md#canvasprint) with the text wrapped to `width` cells and each line
aligned inside it. Returns the size drawn, in cells.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/canvas-print_wrapped.svg">
  <img src="img/canvas-print_wrapped-light.svg" alt="Canvas::print_wrapped" width="384">
</picture>

```rust
let text = "Wrapped at a width, aligned as asked.";
c.print_wrapped(1, 0, 22, text, TextStyle::new(t.ink), Align::Right);
```

## `Canvas::text_cell`

```rust
pub fn text_cell(&self, col: i32, row: i32) -> Option<TextCell>
```

The text-layer cell at `(col, row)`, `None` when nothing was printed there or
the cell is off-canvas. Continuation cells (see [`TextCell`](text.md#textcell)) come back too.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/textcell.svg">
  <img src="img/textcell-light.svg" alt="TextCell, TextCell::CONTINUATION, TextCell::is_empty, TextCell::is_continuation, Canvas::text_cell" width="192">
</picture>

```rust
// A wide character takes two cells: its own and a continuation. Under each
// cell, what `text_cell` reports about it: empty, a character, or the
// right half of one.
c.print(1, 0, "a日b", t.ink);
for col in 0..6 {
    let color = match c.text_cell(col, 0) {
        None => t.panel,
        Some(cell) if cell.is_continuation() => ORANGE,
        Some(_) => GREEN,
    };
    c.fill_rect(col as f32 * 2.0, 5.0, 2.0, 3.0, color);
}
```

## `Canvas::erase_text`

```rust
pub fn erase_text(&mut self, col: i32, row: i32, cells: i32)
```

Removes `cells` characters of the text layer from `(col, row)` rightwards, so
the dots underneath show again.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/canvas-erase_text.svg">
  <img src="img/canvas-erase_text-light.svg" alt="Canvas::erase_text, Canvas::clear_text, Canvas::has_text" width="384">
</picture>

```rust
c.fill_rect(0.0, 0.0, 48.0, 12.0, Paint::pattern(t.panel, Pattern::Checker(2)));
c.print(1, 1, "everything here goes", t.ink);
c.clear_text();
c.print(1, 0, "erase the middle", t.ink);
c.erase_text(7, 0, 4); // the dots show again there
c.print(1, 2, if c.has_text() { "has text" } else { "no text" }, t.ink);
```

## `Canvas::clear_text`

```rust
pub fn clear_text(&mut self)
```

Empties the text layer, keeping the dots and the allocation.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/canvas-erase_text.svg">
  <img src="img/canvas-erase_text-light.svg" alt="Canvas::erase_text, Canvas::clear_text, Canvas::has_text" width="384">
</picture>

```rust
c.fill_rect(0.0, 0.0, 48.0, 12.0, Paint::pattern(t.panel, Pattern::Checker(2)));
c.print(1, 1, "everything here goes", t.ink);
c.clear_text();
c.print(1, 0, "erase the middle", t.ink);
c.erase_text(7, 0, 4); // the dots show again there
c.print(1, 2, if c.has_text() { "has text" } else { "no text" }, t.ink);
```

## `Canvas::has_text`

```rust
pub fn has_text(&self) -> bool
```

Whether anything has been printed on the text layer.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/canvas-erase_text.svg">
  <img src="img/canvas-erase_text-light.svg" alt="Canvas::erase_text, Canvas::clear_text, Canvas::has_text" width="384">
</picture>

```rust
c.fill_rect(0.0, 0.0, 48.0, 12.0, Paint::pattern(t.panel, Pattern::Checker(2)));
c.print(1, 1, "everything here goes", t.ink);
c.clear_text();
c.print(1, 0, "erase the middle", t.ink);
c.erase_text(7, 0, 4); // the dots show again there
c.print(1, 2, if c.has_text() { "has text" } else { "no text" }, t.ink);
```

[Index](README.md) · [canvas](canvas.md) · [draw](draw.md) · [path](path.md) · [mask](mask.md) · [transform](transform.md) · [rig](rig.md) · [layer](layer.md) · [bubble](bubble.md) · [font](font.md) · **text** · [color](color.md) · [render](render.md) · [term](term.md) · [export](export.md) · [ratatui](ratatui.md)

