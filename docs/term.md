# `term`

[Index](README.md) · [canvas](canvas.md) · [draw](draw.md) · [path](path.md) · [mask](mask.md) · [transform](transform.md) · [rig](rig.md) · [layer](layer.md) · [bubble](bubble.md) · [font](font.md) · [text](text.md) · [color](color.md) · [render](render.md) · **term** · [export](export.md) · [ratatui](ratatui.md)

Terminal capability detection.

Two things matter for drawing an image that lines up with the character grid:
which graphics protocol the terminal speaks, and how many pixels one cell is.
Terminals do not expose the font name or point size; the cell box in pixels is
the only geometry they publish, and it is all that is needed for alignment.

Detection order in [`Terminal::detect`](term.md#terminaldetect):

1. `COBRA_PROTOCOL` / `COBRA_CELL` environment overrides.
2. Not a tty → [`Protocol::Text`](term.md#protocol).
3. Cell size from `TIOCGWINSZ` (pixel fields), which costs one `ioctl`.
4. One round trip on `/dev/tty`: `XTVERSION` for the terminal's own name, a kitty
   graphics probe, `CSI 16 t` for the cell size when still unknown, `OSC 4` /
   `OSC 10` / `OSC 11` for the palette, and `DA1` for sixel (which also terminates
   the response).
5. Only what the terminal itself answered decides the protocol. The environment
   ([`Protocol::from_env`](term.md#protocolfrom_env)) is consulted for a terminal that answers nothing at
   all, and for iTerm2, whose inline images no query detects.
6. Anything without a usable cell size falls back to [`Protocol::Text`](term.md#protocol).

Environment variables are a guess of last resort because every terminal hands
them to everything it starts, terminals included: `GHOSTTY_RESOURCES_DIR` in an
Alacritty window started from Ghostty is Ghostty's, and taking it for the
terminal in front of the user sent Alacritty kitty images it cannot draw, with no
way back to braille. A terminal that answers `DA1` but no graphics query speaks
no protocol, whatever the environment says.

### Multiplexers

Inside tmux the terminal that draws is not the one the program talks to: tmux
answers queries itself, and every image has to be wrapped in tmux's passthrough
sequence ([`Terminal::passthrough`](term.md#terminal)) to reach the outer terminal, which needs
`allow-passthrough on` (`set -g allow-passthrough on` in `tmux.conf`).

So detection asks the outer terminal through that same wrapper: the kitty query,
`XTVERSION` and `DA1` go out wrapped, and what comes back is proof of everything
at once -- that this really is tmux, that passthrough is allowed, and what the
terminal on the other side can draw. Silence means an image would be dropped just
as the query was, so the frames go out as braille, which tmux draws itself.

Whether tmux is there at all is settled before any of that, by asking the tty
what it is (`XTVERSION`): tmux answers `tmux 3.4`, a terminal answers with its
own name. Nothing in the environment is believed, because a terminal started from
a pane inherits `TMUX`, `TMUX_PANE` and often a `tmux-256color` `TERM` from a
shell's start-up files, and would take the wrapped image as an unknown `DCS`
(Ghostty crashed on one). A terminal too old to answer `XTVERSION` is asked about
over tmux's own socket instead: only tmux's word that the pane it names draws on
this process's tty makes a pane, and a socket that cannot be reached, a tty that
differs and a tmux that cannot be run are all "no".

Being wrong the other way only costs the images, so the doubtful cases go the
cheap way; `COBRA_PASSTHROUGH=1` wraps them anyway (though never at a terminal
that gave its own name) and `COBRA_PASSTHROUGH=0` never does. GNU screen passes
nothing through and gets text.

## Contents

- [`Protocol`](#protocol)
- [`CellSize`](#cellsize)
- [`Terminal`](#terminal)
- `Protocol`: [`Protocol::parse`](term.md#protocolparse), [`Protocol::from_env`](term.md#protocolfrom_env)
- `CellSize`: [`CellSize::is_known`](term.md#cellsizeis_known), [`CellSize::parse`](term.md#cellsizeparse)
- `Terminal`: [`Terminal::text`](term.md#terminaltext), [`Terminal::new`](term.md#terminalnew), [`Terminal::with_passthrough`](term.md#terminalwith_passthrough), [`Terminal::with_depth`](term.md#terminalwith_depth), [`Terminal::with_palette`](term.md#terminalwith_palette), [`Terminal::is_graphical`](term.md#terminalis_graphical), [`Terminal::detect`](term.md#terminaldetect), [`Terminal::name`](term.md#terminalname)

## `Protocol`

```rust
pub enum Protocol
```

How the canvas gets onto the screen.

- `Text` — Braille glyphs, one foreground colour per cell. Works everywhere, copyable.
- `Kitty` — Kitty graphics protocol (kitty, WezTerm, Ghostty, Konsole ≥ 22.04, …).
- `Iterm2` — iTerm2 inline images (iTerm2, WezTerm, mintty, Konsole, …).
- `Sixel` — DEC sixel (foot, xterm, mlterm, Windows Terminal ≥ 1.22, …).

## `CellSize`

```rust
pub struct CellSize
```

Size of one character cell in pixels.

- `pub width: u16` — Cell width in pixels.
- `pub height: u16` — Cell height in pixels.

## `Terminal`

```rust
pub struct Terminal
```

What was learned about the terminal.

- `pub protocol: Protocol` — Best protocol available.
- `pub cell: CellSize` — Pixel size of one cell (zero when unknown; then `protocol` is `Text`).
- `pub cols: u16` — Terminal width in cells, `0` when unknown. Images are clipped to it.
- `pub rows: u16` — Terminal height in cells, `0` when unknown. Images are clipped to it.
- `pub palette: Palette` — The terminal's colour scheme, used to draw [`Color::Indexed`](color.md#color) and [`Color::Foreground`](color.md#color) dots in the image protocols. The xterm defaults until [`detect`](term.md#terminaldetect) learns better.
- `pub palette_queried: bool` — Whether `palette` was reported by the terminal rather than assumed.
- `pub depth: Depth` — Colour depth of the text fallback: what [`Color::Rgb`](color.md#color) dots are quantised to when the frame is braille glyphs. Ignored by image protocols.
- `pub passthrough: bool` — Wrap every image in tmux's passthrough sequence (`DCS tmux ; … ST`, with the escapes inside doubled), so it reaches the terminal tmux runs in. Set by [`detect`](term.md#terminaldetect) when the tty answers that it is tmux and a wrapped query comes back from the terminal behind it, which is what `allow-passthrough on` there buys. Text, cursor movement and printed characters are never wrapped, since tmux has to see those.  tmux hands the wrapped bytes on at wherever its own terminal's cursor is, so before each image the renderer erases the image's origin cell (`ECH`), which is the one thing that makes tmux put that cursor where the pane's is. The image itself is kept inside [`cols`](term.md#terminal) × [`rows`](term.md#terminal) here as everywhere else, since a picture drawn partly has crashed Ghostty.

## `Protocol` methods

## `Protocol::parse`

```rust
pub fn parse(s: &str) -> Option<Self>
```

Parses `text` / `kitty` / `iterm2` / `sixel` (case-insensitive).

## `Protocol::from_env`

```rust
pub fn from_env() -> Option<Self>
```

Guesses the protocol from environment variables alone, without touching the tty.

Returns `None` when nothing conclusive is set, and `Some(Text)` under GNU
screen, which passes no graphics through.

A guess is all it is, and [`Terminal::detect`](term.md#terminaldetect) uses it only where the terminal
itself says nothing: every one of these variables is inherited by whatever the
terminal starts, another terminal included, so `KITTY_WINDOW_ID` may well be
set in an Alacritty window. What the tty answers outranks it.

## `CellSize` methods

## `CellSize::is_known`

```rust
pub fn is_known(self) -> bool
```

`true` when both dimensions are non-zero.

## `CellSize::parse`

```rust
pub fn parse(s: &str) -> Option<Self>
```

Parses `WxH` (e.g. `9x18`).

## `Terminal` methods

## `Terminal::text`

```rust
pub fn text() -> Self
```

A terminal that only gets braille text. Always safe.

## `Terminal::new`

```rust
pub fn new(protocol: Protocol, cell: CellSize) -> Self
```

Builds a terminal description by hand (for tests, or when you know better).
Image protocols with an unknown cell size are demoted to text.

## `Terminal::with_passthrough`

```rust
pub fn with_passthrough(mut self, passthrough: bool) -> Self
```

Wraps images for tmux (builder style); see [`passthrough`](term.md#terminal).

## `Terminal::with_depth`

```rust
pub fn with_depth(mut self, depth: Depth) -> Self
```

Sets the text colour depth (builder style).

## `Terminal::with_palette`

```rust
pub fn with_palette(mut self, palette: Palette) -> Self
```

Replaces the palette (builder style).

## `Terminal::is_graphical`

```rust
pub fn is_graphical(&self) -> bool
```

Whether frames are transmitted as images rather than glyphs.

## `Terminal::detect`

```rust
pub fn detect() -> Self
```

Detects the terminal on stdout / `/dev/tty`.

Order: `COBRA_PROTOCOL` / `COBRA_CELL` overrides, tty check, cell size from
`TIOCGWINSZ`, then one escape-sequence round trip that asks the terminal what
it is (`XTVERSION`) and what it can draw (kitty probe, `DA1`), plus whatever
else is still unknown (`CSI 16 t` for the cell, `OSC 4`, `OSC 10`, `OSC 11`
for the colour scheme). What the terminal answers decides; the environment
([`Protocol::from_env`](term.md#protocolfrom_env)) only fills in for a terminal that answers nothing.
Image protocols without a known cell size fall back to [`Protocol::Text`](term.md#protocol).

Costs one `ioctl` plus one escape-sequence round trip, bounded by a short
timeout and normally ending as soon as the terminal answers `DA1` (a few
milliseconds). Call it once at start-up and keep the result. Set
`COBRA_PALETTE=0` to skip the colour queries.

A tty that answers `tmux` costs a second round trip, wrapped in tmux's
passthrough so that the terminal tmux draws on answers it: what comes back
says whether images can reach that terminal at all and what it can draw, and
silence means braille. A terminal that inherited `TMUX` from a pane without
being one answers with its own name and is never sent a wrapped byte.

The text colour depth comes from `COBRA_COLORS` (`mono|16|256|true`) or
[`Depth::from_env`](color.md#depthfrom_env); a terminal with a graphics protocol is assumed to have
true colour.

## `Terminal::name`

```rust
pub fn name() -> Option<String>
```

What the terminal calls itself, lowercased: `tmux 3.4`, `ghostty 1.3.1`,
`kitty(0.32.2)`. `None` when it does not answer, as terminals older than
`XTVERSION` (`CSI > q`) do not.

[`detect`](term.md#terminaldetect) asks this first, since it is the one answer that says
whether tmux or a terminal is on the tty, and so whether an image may be
wrapped for tmux. This asks it again, for a diagnostic to print; it costs one
round trip and is bounded by the same short timeout.

[Index](README.md) · [canvas](canvas.md) · [draw](draw.md) · [path](path.md) · [mask](mask.md) · [transform](transform.md) · [rig](rig.md) · [layer](layer.md) · [bubble](bubble.md) · [font](font.md) · [text](text.md) · [color](color.md) · [render](render.md) · **term** · [export](export.md) · [ratatui](ratatui.md)

