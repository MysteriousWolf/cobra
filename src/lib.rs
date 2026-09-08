//! # cobra
//!
//! Per-dot coloured braille canvases for the terminal.
//!
//! A braille glyph packs a 2×4 grid of dots into one character cell, but text can only
//! give the whole cell one colour. `cobra` keeps the braille *model* (a [`Canvas`] of
//! individually coloured dots) and picks the best way the current terminal can show it:
//!
//! | [`Protocol`] | How the frame travels | Copyable as braille |
//! |---|---|---|
//! | `Kitty`  | zlib RGBA, chunked APC, one image id reused per canvas | with `copy_text` |
//! | `Iterm2` | PNG in OSC 1337                                         | with `copy_text` |
//! | `Sixel`  | palettised DCS with transparent background              | with `copy_text` |
//! | `Text`   | braille glyphs, dominant colour per cell, quantised to the terminal's [`Depth`] | yes |
//!
//! Image frames are rasterised on the terminal's real cell grid (queried once from the
//! terminal), so every dot lands exactly where the font would draw it and the picture
//! lines up with surrounding text.
//!
//! ```no_run
//! use cobra::{Canvas, Renderer, Rgb, Terminal};
//!
//! # #[cfg(feature = "detect")] {
//! let term = Terminal::detect();                // once, at start-up
//! let mut renderer = Renderer::new(term);       // owns all scratch buffers
//! let mut canvas = Canvas::new(20, 5);          // 20×5 cells = 40×20 dots
//!
//! for x in 0..canvas.width() {
//!     let hue = x as f32 / canvas.width() as f32;
//!     canvas.set(x, 10, Rgb::hex(0xff0055).lerp(Rgb::hex(0x00ccff), hue));
//! }
//! renderer.render(&canvas, &mut std::io::stdout()).unwrap();
//! # }
//! ```
//!
//! ## Detection
//!
//! [`Terminal::detect`] reads `COBRA_PROTOCOL` (`text|kitty|iterm2|sixel`),
//! `COBRA_CELL` (`WxH` pixels) and `COBRA_COLORS` (`mono|16|256|true`) overrides, checks environment variables, and then
//! spends one short escape-sequence round trip on `/dev/tty` for what is still unknown
//! and for the colour scheme. Terminals do not expose font names or point sizes; the
//! cell box in pixels is what they publish and what alignment needs. The dot diameter
//! is a style choice, `COBRA_DOT`, that `cargo run --example calibrate` helps pick.
//! Inside tmux the outer terminal is read from the environment and images go through
//! tmux's passthrough (`allow-passthrough on`); under screen, or when the cell size
//! cannot be learned, the text protocol is used. See [`Terminal::detect`].
//!
//! ## Drawing
//!
//! Besides single dots and Bresenham lines, [`Canvas`] has span-based vector
//! primitives (`fill_rect`, `fill_round_rect`, `fill_polygon`, `fill_ellipse`,
//! `fill_ngon`, `fill_star`, `fill_pie`, `ring`, `arrow`, stroked `rect`,
//! `round_rect`, `polygon`, `polyline`, `ellipse`, `ngon`, `star`, `arc`, `bezier`,
//! `spline`) and a [`Path`] of lines, curves and arcs. Every shape takes a
//! [`Paint`]: a colour, a dither, a [`Pattern`], a gradient, an edge gradient, cel
//! bands, a shader or [`Paint::erase`]; every stroke takes a [`Pen`], optionally
//! dashed. A [`Mask`] is a shape on its own, to combine, move with a [`Transform`]
//! and paint through ([`Canvas::stencil`], [`Canvas::clip`], [`Canvas::cut`],
//! [`Canvas::clipped`], [`Canvas::effects`]); [`Canvas::with`] draws in local
//! coordinates. See the [`mask`] module. A [`Rig`] is a figure of [`Part`]s to pose;
//! see the [`rig`] module.
//!
//! ## Text
//!
//! Two kinds, for two jobs. [`Canvas::text`] draws with a [`Font`], a tiny text-format
//! bitmap font that scales ([`Font::tiny`] is built in), so a label is dots like
//! everything else. [`Canvas::print`] puts a *real character* in a cell — any glyph the
//! terminal's font has, still copyable, sharp at any size — on a [text layer](text)
//! that every protocol and the exporters understand.
//!
//! [`Bubble`] puts the two together: a text box, or a chat bubble with a tail that
//! [`Bubble::speak`] aims at a speaker while dodging the rectangles you want kept
//! clear.
//!
//! ## Layers
//!
//! [`Layers`] stacks canvases and flattens them into one, bottom to top: what is in
//! front hides what is behind, and each [`Layer`] can carry [`Effect`]s around its
//! silhouette — a drop shadow, an outline, a cleared gap, a glow, a shaded rim, or a
//! shader of your own. A layer can be offset from the stack and wrap around it, so a
//! parallax is layers scrolling by different amounts. The result is a plain
//! [`Canvas`], which every protocol and exporter takes as usual; in the text
//! fallback a cell takes the colour of the topmost layer in it. See the [`layer`]
//! module.
//!
//! ## Colours
//!
//! Dots take a [`Color`]: an explicit [`Rgb`], a terminal palette index or the default
//! foreground. Palette colours follow the user's theme in every protocol: the text
//! fallback emits SGR indices, and the image protocols resolve them through the
//! [`Palette`] that detection reads from the terminal. On terminals without true
//! colour the text fallback quantises each dot to the nearest colour of the detected
//! [`Depth`] (256, 16 or none) before choosing a cell's dominant colour.
//!
//! ## Export
//!
//! [`export::png`] and [`export::svg`] write a canvas to a file with the same
//! cell-aligned geometry and a transparent background.
//!
//! ## Performance
//!
//! * [`Canvas`] is a flat `u32` per dot; [`Renderer`] and [`Layers`] reuse their
//!   buffers, so steady-state rendering does not allocate.
//! * Rasterisation is a table lookup per pixel using a mask built once per cell size.
//! * Frames are mostly transparent flat colour; the built-in zlib encoder exploits that
//!   (runs, cell-periodic patterns, repeated scanlines) so a kitty/iTerm2 frame is
//!   usually a few kilobytes. `cargo bench` prints frames per second per protocol.
//! * One kitty image id per renderer means updates replace in place, without flicker
//!   or accumulating placements.
//!
//! ## Features
//!
//! * `detect` (default) – terminal probing via `libc` on unix.
//! * `ratatui` – [`ratatui::Braille`] widget and [`ratatui::overlay`].

#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(all(feature = "detect", unix)), forbid(unsafe_code))]
#![warn(missing_docs)]

pub mod bubble;
mod canvas;
mod color;
mod draw;
mod encode;
pub mod export;
mod font;
pub mod layer;
pub mod mask;
mod path;
mod raster;
mod render;
pub mod rig;
mod term;
pub mod text;
mod transform;

#[cfg(feature = "ratatui")]
#[cfg_attr(docsrs, doc(cfg(feature = "ratatui")))]
pub mod ratatui;

pub use bubble::{Bubble, Shape, Side, Tail, TailKind};
pub use canvas::{Canvas, Cell, DOTS_X, DOTS_Y, bayer, braille};
pub use color::{Color, Depth, Palette, Rgb};
pub use draw::{Paint, Pattern, Pen, Point, Probe, Rect, Shader};
pub use font::{Font, FontError, Glyph, MAX_GLYPH_WIDTH};
pub use layer::{Effect, Field, Layer, Layers, Sample};
pub use mask::{Mask, Silhouette};
pub use path::Path;
pub use render::{Options, Placement, Renderer};
pub use rig::{Part, Rig};
pub use term::{CellSize, Protocol, Terminal};
pub use text::{Align, Attrs, TextCell, TextStyle};
pub use transform::Transform;
