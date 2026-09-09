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
4. Protocol from well-known environment variables ([`Protocol::from_env`](term.md#protocolfrom_env)).
5. One round trip on `/dev/tty`: a kitty graphics probe and `CSI 16 t` for the
   cell size when still unknown, `OSC 4` / `OSC 10` / `OSC 11` for the palette,
   and `DA1` for sixel (which also terminates the response).
6. Anything without a usable cell size falls back to [`Protocol::Text`](term.md#protocol).

### Multiplexers

Inside tmux the terminal that draws is not the one the program talks to, and
tmux answers queries itself, so nothing is asked over the tty. The outer
terminal is instead recognised from the variables it leaves in tmux's
environment (`GHOSTTY_RESOURCES_DIR`, `KITTY_WINDOW_ID`, `WEZTERM_EXECUTABLE`,
`ITERM_SESSION_ID`), the cell size comes from `TIOCGWINSZ` (tmux ≥ 3.2 passes the
pixel size on), and every image is wrapped in tmux's passthrough sequence
([`Terminal::passthrough`](term.md#terminal)), which reaches the outer terminal only when tmux has
`allow-passthrough on` (`set -g allow-passthrough on` in `tmux.conf`). Without
it the images are silently dropped; `COBRA_PROTOCOL=text` opts out. A terminal
started from inside tmux inherits `TMUX` without being a pane, and would take the
wrapped image as an unknown `DCS` (Ghostty crashed on one), so nothing about the
environment is believed on its own: tmux is asked over its socket whether the pane
it names draws on this process's tty, and only that answer makes a pane. `TERM`
never does, since a shell's start-up files may set it long after the terminal did.
Being wrong the other way only costs the images, so a tmux that cannot be run, or
answers about another tty, is not a pane; `COBRA_PASSTHROUGH=1` wraps them anyway
and `COBRA_PASSTHROUGH=0` never does. GNU screen passes nothing through and gets
text.

## Contents

- [`Protocol`](#protocol)
- [`CellSize`](#cellsize)
- [`Terminal`](#terminal)
- `Protocol`: [`Protocol::parse`](term.md#protocolparse), [`Protocol::from_env`](term.md#protocolfrom_env)
- `CellSize`: [`CellSize::is_known`](term.md#cellsizeis_known), [`CellSize::parse`](term.md#cellsizeparse)
- `Terminal`: [`Terminal::text`](term.md#terminaltext), [`Terminal::new`](term.md#terminalnew), [`Terminal::with_passthrough`](term.md#terminalwith_passthrough), [`Terminal::with_depth`](term.md#terminalwith_depth), [`Terminal::with_palette`](term.md#terminalwith_palette), [`Terminal::is_graphical`](term.md#terminalis_graphical), [`Terminal::detect`](term.md#terminaldetect)

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
- `pub cols: u16` — Terminal width in cells, `0` when unknown.
- `pub rows: u16` — Terminal height in cells, `0` when unknown.
- `pub palette: Palette` — The terminal's colour scheme, used to draw [`Color::Indexed`](color.md#color) and [`Color::Foreground`](color.md#color) dots in the image protocols. The xterm defaults until [`detect`](term.md#terminaldetect) learns better.
- `pub palette_queried: bool` — Whether `palette` was reported by the terminal rather than assumed.
- `pub depth: Depth` — Colour depth of the text fallback: what [`Color::Rgb`](color.md#color) dots are quantised to when the frame is braille glyphs. Ignored by image protocols.
- `pub passthrough: bool` — Wrap every image in tmux's passthrough sequence (`DCS tmux ; … ST`, with the escapes inside doubled), so it reaches the terminal tmux runs in. Set by [`detect`](term.md#terminaldetect) inside tmux; needs `allow-passthrough on` there. Text, cursor movement and printed characters are never wrapped, since tmux has to see those.  tmux hands the wrapped bytes on at wherever its own terminal's cursor is, so before each image the renderer erases the image's origin cell (`ECH`), which is the one thing that makes tmux put that cursor where the pane's is, and it clips the image to [`cols`](term.md#terminal) × [`rows`](term.md#terminal), the pane: an image hanging off the outer screen is drawn by a terminal that never saw the pane, and has crashed Ghostty.

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
screen, which passes no graphics through. Under tmux the outer terminal is
recognised by the variables it leaves in tmux's environment, since `TERM` and
`TERM_PROGRAM` there are tmux's own; see [`Terminal::passthrough`](term.md#terminal).

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
`TIOCGWINSZ`, protocol from the environment ([`Protocol::from_env`](term.md#protocolfrom_env)), then one
escape-sequence round trip that asks for whatever is still unknown (kitty probe,
`CSI 16 t`, `DA1`) plus the colour scheme (`OSC 4`, `OSC 10`, `OSC 11`). Image
protocols without a known cell size fall back to [`Protocol::Text`](term.md#protocol).

Costs one `ioctl` plus one escape-sequence round trip, bounded by a short
timeout and normally ending as soon as the terminal answers `DA1` (a few
milliseconds). Call it once at start-up and keep the result. Set
`COBRA_PALETTE=0` to skip the colour queries.

Inside tmux (a real pane, not a terminal started from one, which inherits
`TMUX`; tmux itself is asked which it is) there is no round trip to
the tty: the outer terminal is read from the
environment, the cell size from `TIOCGWINSZ`, and images are marked for
[passthrough](term.md#terminal): under tmux the queries would be answered by tmux
itself, and tmux needs `allow-passthrough on` for the images to reach its terminal.

The text colour depth comes from `COBRA_COLORS` (`mono|16|256|true`) or
[`Depth::from_env`](color.md#depthfrom_env); a terminal with a graphics protocol is assumed to have
true colour.

[Index](README.md) · [canvas](canvas.md) · [draw](draw.md) · [path](path.md) · [mask](mask.md) · [transform](transform.md) · [rig](rig.md) · [layer](layer.md) · [bubble](bubble.md) · [font](font.md) · [text](text.md) · [color](color.md) · [render](render.md) · **term** · [export](export.md) · [ratatui](ratatui.md)

