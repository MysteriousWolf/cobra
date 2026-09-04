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
//! | `Text`   | braille glyphs, dominant colour per cell                | yes |
//!
//! Image frames are rasterised on the terminal's real cell grid (queried once from the
//! terminal), so every dot lands exactly where the font would draw it and the picture
//! lines up with surrounding text.
//!
//! ```no_run
//! use cobra::{Canvas, Renderer, Rgb, Terminal};
//!
//! let term = Terminal::detect();                // once, at start-up
//! let mut renderer = Renderer::new(term);       // owns all scratch buffers
//! let mut canvas = Canvas::new(20, 5);          // 20×5 cells = 40×20 dots
//!
//! for x in 0..canvas.width() {
//!     let hue = x as f32 / canvas.width() as f32;
//!     canvas.set(x, 10, Rgb::hex(0xff0055).lerp(Rgb::hex(0x00ccff), hue));
//! }
//! renderer.render(&canvas, &mut std::io::stdout()).unwrap();
//! ```
//!
//! ## Detection
//!
//! [`Terminal::detect`] reads `COBRA_PROTOCOL` (`text|kitty|iterm2|sixel`) and
//! `COBRA_CELL` (`WxH` pixels) overrides, checks environment variables, and only then
//! spends one short escape-sequence round trip on `/dev/tty`. Terminals do not expose
//! font names or point sizes; the cell box in pixels is what they publish and what
//! alignment needs. Inside tmux/screen, or when the cell size cannot be learned, the
//! text protocol is used.
//!
//! ## Performance
//!
//! * [`Canvas`] is a flat `u32` per dot; [`Renderer`] reuses its buffers, so steady-state
//!   rendering does not allocate.
//! * Rasterisation is a table lookup per pixel using a mask built once per cell size.
//! * Frames are mostly transparent flat colour; the built-in zlib encoder exploits that
//!   (runs and repeated scanlines) so a kitty/iTerm2 frame is usually a few kilobytes.
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

mod canvas;
mod color;
mod encode;
mod raster;
mod render;
mod term;

#[cfg(feature = "ratatui")]
#[cfg_attr(docsrs, doc(cfg(feature = "ratatui")))]
pub mod ratatui;

pub use canvas::{braille, Canvas, Cell, DOTS_X, DOTS_Y};
pub use color::Rgb;
pub use render::{Options, Placement, Renderer};
pub use term::{CellSize, Protocol, Terminal};
