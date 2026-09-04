//! [`ratatui`] integration.
//!
//! [`Braille`] is a [`Widget`] that draws a [`Canvas`] into the frame buffer. What it
//! writes depends on the renderer's protocol:
//!
//! * **Text** – braille glyphs with a foreground colour per cell. Nothing else to do.
//! * **Kitty** – Unicode *placeholder* cells that reference the renderer's image id.
//!   The image itself has to be transmitted outside the buffer once per frame, which
//!   is what [`overlay`] does. Placeholders never change between frames, so ratatui's
//!   diff never rewrites them and only the compressed image moves over the wire.
//! * **iTerm2 / sixel** – these protocols have no in-buffer placement, so the widget
//!   writes the text fallback and [`overlay`] paints the image on top after the frame.
//!
//! ```no_run
//! # use cobra::{Canvas, Renderer, Terminal};
//! # use cobra::ratatui::{Braille, overlay};
//! # fn main() -> std::io::Result<()> {
//! let mut terminal = ratatui::init();
//! let mut renderer = Renderer::new(Terminal::detect());
//! let canvas = Canvas::new(40, 10);
//! loop {
//!     let area = terminal.draw(|f| {
//!         let area = f.area();
//!         f.render_widget(Braille::new(&canvas, &renderer), area);
//!     })?.area;
//!     overlay(&mut renderer, &canvas, area, terminal.backend_mut())?;
//! #   break;
//! }
//! # Ok(()) }
//! ```

use std::io::{self, Write};

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Color;
use ratatui::widgets::Widget;

use crate::render::Placement;
use crate::{Canvas, Protocol, Renderer};

/// Kitty placeholder character.
const PLACEHOLDER: char = '\u{10EEEE}';

/// Row/column diacritics from kitty's `rowcolumn-diacritics.txt` (index = row or column).
const DIACRITICS: [char; 297] = [
    '\u{305}',
    '\u{30D}',
    '\u{30E}',
    '\u{310}',
    '\u{312}',
    '\u{33D}',
    '\u{33E}',
    '\u{33F}',
    '\u{346}',
    '\u{34A}',
    '\u{34B}',
    '\u{34C}',
    '\u{350}',
    '\u{351}',
    '\u{352}',
    '\u{357}',
    '\u{35B}',
    '\u{363}',
    '\u{364}',
    '\u{365}',
    '\u{366}',
    '\u{367}',
    '\u{368}',
    '\u{369}',
    '\u{36A}',
    '\u{36B}',
    '\u{36C}',
    '\u{36D}',
    '\u{36E}',
    '\u{36F}',
    '\u{483}',
    '\u{484}',
    '\u{485}',
    '\u{486}',
    '\u{487}',
    '\u{592}',
    '\u{593}',
    '\u{594}',
    '\u{595}',
    '\u{597}',
    '\u{598}',
    '\u{599}',
    '\u{59C}',
    '\u{59D}',
    '\u{59E}',
    '\u{59F}',
    '\u{5A0}',
    '\u{5A1}',
    '\u{5A8}',
    '\u{5A9}',
    '\u{5AB}',
    '\u{5AC}',
    '\u{5AF}',
    '\u{5C4}',
    '\u{610}',
    '\u{611}',
    '\u{612}',
    '\u{613}',
    '\u{614}',
    '\u{615}',
    '\u{616}',
    '\u{617}',
    '\u{657}',
    '\u{658}',
    '\u{659}',
    '\u{65A}',
    '\u{65B}',
    '\u{65D}',
    '\u{65E}',
    '\u{6D6}',
    '\u{6D7}',
    '\u{6D8}',
    '\u{6D9}',
    '\u{6DA}',
    '\u{6DB}',
    '\u{6DC}',
    '\u{6DF}',
    '\u{6E0}',
    '\u{6E1}',
    '\u{6E2}',
    '\u{6E4}',
    '\u{6E7}',
    '\u{6E8}',
    '\u{6EB}',
    '\u{6EC}',
    '\u{730}',
    '\u{732}',
    '\u{733}',
    '\u{735}',
    '\u{736}',
    '\u{73A}',
    '\u{73D}',
    '\u{73F}',
    '\u{740}',
    '\u{741}',
    '\u{743}',
    '\u{745}',
    '\u{747}',
    '\u{749}',
    '\u{74A}',
    '\u{7EB}',
    '\u{7EC}',
    '\u{7ED}',
    '\u{7EE}',
    '\u{7EF}',
    '\u{7F0}',
    '\u{7F1}',
    '\u{7F3}',
    '\u{816}',
    '\u{817}',
    '\u{818}',
    '\u{819}',
    '\u{81B}',
    '\u{81C}',
    '\u{81D}',
    '\u{81E}',
    '\u{81F}',
    '\u{820}',
    '\u{821}',
    '\u{822}',
    '\u{823}',
    '\u{825}',
    '\u{826}',
    '\u{827}',
    '\u{829}',
    '\u{82A}',
    '\u{82B}',
    '\u{82C}',
    '\u{82D}',
    '\u{951}',
    '\u{953}',
    '\u{954}',
    '\u{F82}',
    '\u{F83}',
    '\u{F86}',
    '\u{F87}',
    '\u{135D}',
    '\u{135E}',
    '\u{135F}',
    '\u{17DD}',
    '\u{193A}',
    '\u{1A17}',
    '\u{1A75}',
    '\u{1A76}',
    '\u{1A77}',
    '\u{1A78}',
    '\u{1A79}',
    '\u{1A7A}',
    '\u{1A7B}',
    '\u{1A7C}',
    '\u{1B6B}',
    '\u{1B6D}',
    '\u{1B6E}',
    '\u{1B6F}',
    '\u{1B70}',
    '\u{1B71}',
    '\u{1B72}',
    '\u{1B73}',
    '\u{1CD0}',
    '\u{1CD1}',
    '\u{1CD2}',
    '\u{1CDA}',
    '\u{1CDB}',
    '\u{1CE0}',
    '\u{1DC0}',
    '\u{1DC1}',
    '\u{1DC3}',
    '\u{1DC4}',
    '\u{1DC5}',
    '\u{1DC6}',
    '\u{1DC7}',
    '\u{1DC8}',
    '\u{1DC9}',
    '\u{1DCB}',
    '\u{1DCC}',
    '\u{1DD1}',
    '\u{1DD2}',
    '\u{1DD3}',
    '\u{1DD4}',
    '\u{1DD5}',
    '\u{1DD6}',
    '\u{1DD7}',
    '\u{1DD8}',
    '\u{1DD9}',
    '\u{1DDA}',
    '\u{1DDB}',
    '\u{1DDC}',
    '\u{1DDD}',
    '\u{1DDE}',
    '\u{1DDF}',
    '\u{1DE0}',
    '\u{1DE1}',
    '\u{1DE2}',
    '\u{1DE3}',
    '\u{1DE4}',
    '\u{1DE5}',
    '\u{1DE6}',
    '\u{1DFE}',
    '\u{20D0}',
    '\u{20D1}',
    '\u{20D4}',
    '\u{20D5}',
    '\u{20D6}',
    '\u{20D7}',
    '\u{20DB}',
    '\u{20DC}',
    '\u{20E1}',
    '\u{20E7}',
    '\u{20E9}',
    '\u{20F0}',
    '\u{2CEF}',
    '\u{2CF0}',
    '\u{2CF1}',
    '\u{2DE0}',
    '\u{2DE1}',
    '\u{2DE2}',
    '\u{2DE3}',
    '\u{2DE4}',
    '\u{2DE5}',
    '\u{2DE6}',
    '\u{2DE7}',
    '\u{2DE8}',
    '\u{2DE9}',
    '\u{2DEA}',
    '\u{2DEB}',
    '\u{2DEC}',
    '\u{2DED}',
    '\u{2DEE}',
    '\u{2DEF}',
    '\u{2DF0}',
    '\u{2DF1}',
    '\u{2DF2}',
    '\u{2DF3}',
    '\u{2DF4}',
    '\u{2DF5}',
    '\u{2DF6}',
    '\u{2DF7}',
    '\u{2DF8}',
    '\u{2DF9}',
    '\u{2DFA}',
    '\u{2DFB}',
    '\u{2DFC}',
    '\u{2DFD}',
    '\u{2DFE}',
    '\u{2DFF}',
    '\u{A66F}',
    '\u{A67C}',
    '\u{A67D}',
    '\u{A6F0}',
    '\u{A6F1}',
    '\u{A8E0}',
    '\u{A8E1}',
    '\u{A8E2}',
    '\u{A8E3}',
    '\u{A8E4}',
    '\u{A8E5}',
    '\u{A8E6}',
    '\u{A8E7}',
    '\u{A8E8}',
    '\u{A8E9}',
    '\u{A8EA}',
    '\u{A8EB}',
    '\u{A8EC}',
    '\u{A8ED}',
    '\u{A8EE}',
    '\u{A8EF}',
    '\u{A8F0}',
    '\u{A8F1}',
    '\u{AAB0}',
    '\u{AAB2}',
    '\u{AAB3}',
    '\u{AAB7}',
    '\u{AAB8}',
    '\u{AABE}',
    '\u{AABF}',
    '\u{AAC1}',
    '\u{FE20}',
    '\u{FE21}',
    '\u{FE22}',
    '\u{FE23}',
    '\u{FE24}',
    '\u{FE25}',
    '\u{FE26}',
    '\u{10A0F}',
    '\u{10A38}',
    '\u{1D185}',
    '\u{1D186}',
    '\u{1D187}',
    '\u{1D188}',
    '\u{1D189}',
    '\u{1D1AA}',
    '\u{1D1AB}',
    '\u{1D1AC}',
    '\u{1D1AD}',
    '\u{1D242}',
    '\u{1D243}',
    '\u{1D244}',
];

/// Widget that draws a canvas. See the [module docs](self).
#[derive(Clone, Copy, Debug)]
pub struct Braille<'a> {
    canvas: &'a Canvas,
    protocol: Protocol,
    id: u32,
}

impl<'a> Braille<'a> {
    /// Draws `canvas` the way `renderer` will overlay it.
    pub fn new(canvas: &'a Canvas, renderer: &Renderer) -> Self {
        Self { canvas, protocol: renderer.terminal().protocol, id: renderer.image_id() }
    }
}

impl Widget for Braille<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let cols = area.width.min(self.canvas.cols());
        let rows = area.height.min(self.canvas.rows()).min(DIACRITICS.len() as u16);
        let kitty = self.protocol == Protocol::Kitty;
        let id = Color::Rgb((self.id >> 16) as u8, (self.id >> 8) as u8, self.id as u8);
        let mut first = String::with_capacity(12);
        for row in 0..rows {
            for col in 0..cols {
                let Some(cell) = buf.cell_mut(Position::new(area.x + col, area.y + row)) else { continue };
                if kitty {
                    // Only the first cell of a row carries diacritics; the terminal
                    // infers the rest from their neighbours.
                    if col == 0 {
                        first.clear();
                        first.push(PLACEHOLDER);
                        first.push(DIACRITICS[row as usize]);
                        first.push(DIACRITICS[0]);
                        cell.set_symbol(&first);
                    } else {
                        cell.set_char(PLACEHOLDER);
                    }
                    cell.set_fg(id);
                } else {
                    let c = self.canvas.cell(col, row);
                    cell.set_char(c.glyph());
                    cell.set_fg(match c.color {
                        Some(crate::Color::Rgb(c)) => Color::Rgb(c.r, c.g, c.b),
                        Some(crate::Color::Indexed(i)) => Color::Indexed(i),
                        Some(crate::Color::Foreground) | None => Color::Reset,
                    });
                }
            }
        }
    }
}

/// Transmits the image for a [`Braille`] widget rendered at `area`. Call it after
/// `Terminal::draw`, with the backend's writer. No-op for the text protocol.
pub fn overlay(renderer: &mut Renderer, canvas: &Canvas, area: Rect, w: &mut impl Write) -> io::Result<()> {
    let placement = match renderer.terminal().protocol {
        Protocol::Text => return Ok(()),
        Protocol::Kitty => Placement::Virtual,
        Protocol::Iterm2 | Protocol::Sixel => Placement::At(area.x, area.y),
    };
    let rows = area.height.min(DIACRITICS.len() as u16);
    let bytes = renderer.encode_view(canvas, placement, area.width, rows);
    w.write_all(bytes)?;
    w.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CellSize, Rgb, Terminal};

    #[test]
    fn text_widget_writes_glyphs() {
        let mut c = Canvas::new(2, 1);
        c.set(0, 0, Rgb::hex(0x102030));
        let r = Renderer::new(Terminal::text());
        let mut buf = Buffer::empty(Rect::new(0, 0, 2, 1));
        Braille::new(&c, &r).render(buf.area, &mut buf);
        assert_eq!(buf[(0, 0)].symbol(), "⠁");
        assert_eq!(buf[(0, 0)].fg, Color::Rgb(0x10, 0x20, 0x30));
        assert_eq!(buf[(1, 0)].symbol(), "⠀");
        let mut c = Canvas::new(1, 1);
        c.set(0, 0, crate::Color::Indexed(3));
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        Braille::new(&c, &r).render(buf.area, &mut buf);
        assert_eq!(buf[(0, 0)].fg, Color::Indexed(3));
    }

    #[test]
    fn kitty_widget_writes_placeholders() {
        let c = Canvas::new(3, 2);
        let r = Renderer::new(Terminal::new(Protocol::Kitty, CellSize { width: 8, height: 16 }));
        let mut buf = Buffer::empty(Rect::new(1, 1, 3, 2));
        Braille::new(&c, &r).render(buf.area, &mut buf);
        let id = r.image_id();
        assert_eq!(buf[(1, 2)].symbol(), "\u{10EEEE}\u{30D}\u{305}");
        assert_eq!(buf[(2, 2)].symbol(), "\u{10EEEE}");
        assert_eq!(buf[(3, 1)].fg, Color::Rgb((id >> 16) as u8, (id >> 8) as u8, id as u8));
    }
}
