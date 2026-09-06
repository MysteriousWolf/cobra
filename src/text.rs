//! Real terminal text on a canvas: characters, not dots.
//!
//! [`Canvas::text`](Canvas::text) draws with a bitmap [`Font`](crate::Font), so a
//! label is made of dots like everything else and can be any size or colour. That is
//! the right answer for a title inside a drawing and the wrong one for a paragraph:
//! at 3×5 dots a sentence is barely legible, and it is not text any more, so it
//! cannot be copied out of the terminal or read by a screen reader.
//!
//! So a canvas also has a *text layer*: one real character per terminal cell, with
//! its own colours and attributes, drawn by [`Canvas::print`]. A cell holding a
//! character shows that character instead of its eight dots — in the text protocol,
//! in the image protocols (which leave the cell transparent and print over it), in
//! [`Canvas::to_text`] and in the ratatui widget. Everything the terminal's own font
//! can draw works: ASCII, box drawing, CJK, emoji.
//!
//! ```
//! use cobra::{Canvas, Rgb, TextStyle};
//!
//! let mut c = Canvas::new(20, 3);
//! c.fill_rect(0.0, 0.0, 40.0, 12.0, Rgb::hex(0x1b2430));   // dots behind the text
//! c.print(1, 1, "Ready.", TextStyle::new(Rgb::hex(0xc9d1d9)).bold());
//! ```
//!
//! Cell coordinates here, dot coordinates everywhere else: one cell is
//! [`DOTS_X`](crate::DOTS_X) × [`DOTS_Y`](crate::DOTS_Y) dots.

use crate::{Canvas, Color};

/// Text attributes, as SGR bits. Combine with `|`.
///
/// ```
/// # use cobra::Attrs;
/// let a = Attrs::BOLD | Attrs::UNDERLINE;
/// assert!(a.has(Attrs::BOLD) && !a.has(Attrs::ITALIC));
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Attrs(pub u8);

impl Attrs {
    /// No attributes.
    pub const NONE: Attrs = Attrs(0);
    /// `SGR 1`.
    pub const BOLD: Attrs = Attrs(1);
    /// `SGR 2`.
    pub const DIM: Attrs = Attrs(2);
    /// `SGR 3`.
    pub const ITALIC: Attrs = Attrs(4);
    /// `SGR 4`.
    pub const UNDERLINE: Attrs = Attrs(8);
    /// `SGR 7`.
    pub const REVERSE: Attrs = Attrs(16);

    /// Whether every bit of `other` is set.
    #[inline]
    pub const fn has(self, other: Attrs) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for Attrs {
    type Output = Attrs;
    fn bitor(self, rhs: Attrs) -> Attrs {
        Attrs(self.0 | rhs.0)
    }
}

/// How a character is drawn: foreground, background and attributes, each optional so
/// the terminal's own defaults show through.
///
/// Anything that is a [`Color`] converts into a `TextStyle` foreground, so
/// `canvas.print(0, 0, "hi", Rgb::hex(0xff0055))` works.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextStyle {
    /// Ink; `None` is the terminal's default foreground.
    pub fg: Option<Color>,
    /// Cell background; `None` leaves whatever is behind it.
    pub bg: Option<Color>,
    /// Bold, italic and friends.
    pub attrs: Attrs,
}

impl TextStyle {
    /// Style with `fg` as its ink.
    pub fn new(fg: impl Into<Color>) -> Self {
        Self { fg: Some(fg.into()), ..Self::default() }
    }

    /// Sets the background.
    pub fn on(mut self, bg: impl Into<Color>) -> Self {
        self.bg = Some(bg.into());
        self
    }

    /// Adds attributes.
    pub fn with(mut self, attrs: Attrs) -> Self {
        self.attrs = self.attrs | attrs;
        self
    }

    /// Adds [`Attrs::BOLD`].
    pub fn bold(self) -> Self {
        self.with(Attrs::BOLD)
    }

    /// Adds [`Attrs::DIM`].
    pub fn dim(self) -> Self {
        self.with(Attrs::DIM)
    }

    /// Adds [`Attrs::ITALIC`].
    pub fn italic(self) -> Self {
        self.with(Attrs::ITALIC)
    }

    /// Adds [`Attrs::UNDERLINE`].
    pub fn underline(self) -> Self {
        self.with(Attrs::UNDERLINE)
    }
}

impl From<Color> for TextStyle {
    fn from(fg: Color) -> Self {
        Self::new(fg)
    }
}

impl From<crate::Rgb> for TextStyle {
    fn from(fg: crate::Rgb) -> Self {
        Self::new(fg)
    }
}

impl From<u32> for TextStyle {
    fn from(fg: u32) -> Self {
        Self::new(fg)
    }
}

impl From<(u8, u8, u8)> for TextStyle {
    fn from(fg: (u8, u8, u8)) -> Self {
        Self::new(fg)
    }
}

/// One cell of the text layer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextCell {
    /// The character; `'\0'` for an empty cell and [`CONTINUATION`](Self::CONTINUATION)
    /// for the right half of a double-width one.
    pub ch: char,
    /// How to draw it.
    pub style: TextStyle,
}

impl TextCell {
    /// Placeholder in the cell that a double-width character spills into.
    pub const CONTINUATION: char = '\u{1}';

    /// Whether no character was printed here.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.ch == '\0'
    }

    /// Whether this is the right half of a double-width character.
    #[inline]
    pub const fn is_continuation(&self) -> bool {
        matches!(self.ch, Self::CONTINUATION)
    }
}

/// Horizontal alignment for [`Canvas::print_wrapped`] and text inside a
/// [`Bubble`](crate::Bubble).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Align {
    /// Against the left edge.
    #[default]
    Left,
    /// Centred, extra space split evenly.
    Center,
    /// Against the right edge.
    Right,
}

/// Cells a character occupies: `0` for a combining mark, `2` for the wide ranges of
/// East Asian scripts and emoji, `1` otherwise.
///
/// This is the common part of Unicode's East Asian Width, coarse enough to be a
/// handful of range checks and right for everything a terminal usually shows.
pub fn char_width(ch: char) -> i32 {
    let c = ch as u32;
    match c {
        0..=0x1f | 0x7f..=0x9f => 0,
        0x300..=0x36f | 0x200b..=0x200f | 0xfe00..=0xfe0f | 0x20d0..=0x20ff => 0,
        0x1100..=0x115f
        | 0x2e80..=0x303e
        | 0x3041..=0x33ff
        | 0x3400..=0x4dbf
        | 0x4e00..=0x9fff
        | 0xa000..=0xa4cf
        | 0xac00..=0xd7a3
        | 0xf900..=0xfaff
        | 0xfe30..=0xfe6f
        | 0xff00..=0xff60
        | 0xffe0..=0xffe6
        | 0x1f300..=0x1f64f
        | 0x1f680..=0x1f6ff
        | 0x1f900..=0x1f9ff
        | 0x20000..=0x3fffd => 2,
        _ => 1,
    }
}

/// Width of `text` in cells: the widest of its newline-separated lines.
pub fn width(text: &str) -> i32 {
    text.lines().map(|l| l.chars().map(char_width).sum::<i32>()).max().unwrap_or(0)
}

/// Size of `text` in cells, `(width, lines)`.
pub fn measure(text: &str) -> (i32, i32) {
    (width(text), text.lines().count().max(1) as i32)
}

/// Splits `text` into lines at most `width` cells wide, breaking at spaces where it
/// can and inside a word where it cannot. Existing newlines always break. A `width`
/// of `0` or less only splits on newlines.
///
/// The iterator borrows `text` and allocates nothing.
///
/// ```
/// # use cobra::text::wrap;
/// let lines: Vec<&str> = wrap("the quick brown fox", 9).collect();
/// assert_eq!(lines, ["the quick", "brown fox"]);
/// ```
pub fn wrap(text: &str, width: i32) -> Wrap<'_> {
    Wrap { rest: text, width, done: false }
}

/// Iterator of wrapped lines; see [`wrap`].
#[derive(Clone, Debug)]
pub struct Wrap<'a> {
    rest: &'a str,
    width: i32,
    done: bool,
}

impl<'a> Iterator for Wrap<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if self.done {
            return None;
        }
        let (mut end, mut next, mut soft) = (self.rest.len(), self.rest.len(), false);
        let (mut w, mut space) = (0, None);
        for (i, ch) in self.rest.char_indices() {
            if ch == '\n' {
                (end, next) = (i, i + 1);
                break;
            }
            let cw = char_width(ch);
            if self.width > 0 && w + cw > self.width && i > 0 {
                // The space that overflows is itself the break; otherwise fall back to
                // the last one on the line, and failing that split the word.
                (end, next, soft) = match (ch, space) {
                    (' ', _) => (i, i + 1, true),
                    (_, Some((s, e))) => (s, e, true),
                    (_, None) => (i, i, true),
                };
                break;
            }
            w += cw;
            if ch == ' ' {
                space = Some((i, i + 1));
            }
        }
        let line = &self.rest[..end];
        if next >= self.rest.len() {
            self.done = true;
            self.rest = "";
        } else {
            self.rest = &self.rest[next..];
            if soft {
                self.rest = self.rest.trim_start_matches(' ');
            }
        }
        Some(line)
    }
}

impl Canvas {
    /// Prints `text` with its first character in cell `(col, row)`, returning the
    /// number of cells the widest line took.
    ///
    /// Newlines start a new line at `col`. Double-width characters take two cells;
    /// zero-width ones are dropped. Characters landing outside the canvas are
    /// skipped. A printed cell hides the dots underneath it, so `style.bg` is how a
    /// label keeps the colour of the shape it sits on.
    pub fn print(&mut self, col: i32, row: i32, text: &str, style: impl Into<TextStyle>) -> i32 {
        let style = style.into();
        let (mut x, mut y, mut max) = (col, row, 0);
        for ch in text.chars() {
            if ch == '\n' {
                (x, y) = (col, y + 1);
                continue;
            }
            let w = char_width(ch);
            if w == 0 {
                continue;
            }
            self.put(x, y, TextCell { ch, style });
            if w == 2 {
                self.put(x + 1, y, TextCell { ch: TextCell::CONTINUATION, style });
            }
            x += w;
            max = max.max(x - col);
        }
        max
    }

    /// [`print`](Self::print) with the text wrapped to `width` cells and each line
    /// aligned inside it. Returns the size drawn, in cells.
    pub fn print_wrapped(
        &mut self,
        col: i32,
        row: i32,
        width: i32,
        text: &str,
        style: impl Into<TextStyle>,
        align: Align,
    ) -> (i32, i32) {
        let style = style.into();
        let (mut used, mut lines) = (0, 0);
        for line in wrap(text, width) {
            let w = crate::text::width(line);
            let x = col
                + match align {
                    Align::Left => 0,
                    Align::Center => (width - w) / 2,
                    Align::Right => width - w,
                };
            self.print(x, row + lines, line, style);
            used = used.max(w);
            lines += 1;
        }
        (used, lines)
    }

    /// The text-layer cell at `(col, row)`, `None` when nothing was printed there or
    /// the cell is off-canvas. Continuation cells (see [`TextCell`]) come back too.
    pub fn text_cell(&self, col: i32, row: i32) -> Option<TextCell> {
        let cell = self.text_at(col, row);
        (!cell.is_empty()).then_some(cell)
    }

    /// Removes `cells` characters of the text layer from `(col, row)` rightwards, so
    /// the dots underneath show again.
    pub fn erase_text(&mut self, col: i32, row: i32, cells: i32) {
        for x in col..col + cells {
            self.put(x, row, TextCell::default());
        }
    }

    /// Empties the text layer, keeping the dots and the allocation.
    pub fn clear_text(&mut self) {
        self.text.clear();
    }

    /// Whether anything has been printed on the text layer.
    #[inline]
    pub fn has_text(&self) -> bool {
        !self.text.is_empty()
    }

    /// Text cell at `(col, row)`, empty when unset or out of range.
    #[inline]
    pub(crate) fn text_at(&self, col: i32, row: i32) -> TextCell {
        if self.text.is_empty() || col < 0 || row < 0 || col >= self.cols() as i32 || row >= self.rows() as i32 {
            return TextCell::default();
        }
        self.text[row as usize * self.cols() as usize + col as usize]
    }

    fn put(&mut self, col: i32, row: i32, cell: TextCell) {
        if col < 0 || row < 0 || col >= self.cols() as i32 || row >= self.rows() as i32 {
            return;
        }
        if self.text.is_empty() {
            if cell.is_empty() {
                return;
            }
            self.text.resize(self.cols() as usize * self.rows() as usize, TextCell::default());
        }
        let i = row as usize * self.cols() as usize + col as usize;
        // Overwriting half of a double-width character orphans the other half.
        if self.text[i].is_continuation() {
            self.text[i - 1] = TextCell::default();
        } else if char_width(self.text[i].ch) == 2 {
            self.text[i + 1] = TextCell::default();
        }
        self.text[i] = cell;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rgb;

    #[test]
    fn prints_characters_into_cells() {
        let mut c = Canvas::new(6, 2);
        assert!(!c.has_text());
        assert_eq!(c.print(1, 0, "hi\nyo", Rgb::hex(0xff0000)), 2);
        assert!(c.has_text());
        assert_eq!(c.text_cell(1, 0).map(|t| t.ch), Some('h'));
        assert_eq!(c.text_cell(2, 1).map(|t| t.ch), Some('o'));
        assert_eq!(c.text_cell(0, 0), None);
        assert_eq!(c.text_cell(1, 0).unwrap().style.fg, Some(Color::Rgb(Rgb::hex(0xff0000))));
        c.erase_text(1, 0, 2);
        assert_eq!(c.text_cell(1, 0), None);
        c.clear_text();
        assert!(!c.has_text());
    }

    #[test]
    fn wide_characters_take_two_cells() {
        let mut c = Canvas::new(6, 1);
        assert_eq!(c.print(0, 0, "字a", TextStyle::default()), 3);
        assert_eq!(c.text_cell(0, 0).unwrap().ch, '字');
        assert!(c.text_cell(1, 0).unwrap().is_continuation());
        assert_eq!(c.text_cell(2, 0).unwrap().ch, 'a');
        // Overwriting either half clears both.
        c.print(1, 0, "x", TextStyle::default());
        assert_eq!(c.text_cell(0, 0), None);
        assert_eq!(c.text_cell(1, 0).unwrap().ch, 'x');
        assert_eq!(char_width('\u{300}'), 0);
        assert_eq!(width("字a\nabcd"), 4);
    }

    #[test]
    fn printing_off_canvas_is_clipped() {
        let mut c = Canvas::new(2, 1);
        c.print(-4, 0, "abcdef", TextStyle::default());
        c.print(0, 9, "z", TextStyle::default());
        assert_eq!(c.text_cell(0, 0).unwrap().ch, 'e');
        assert_eq!(c.text_cell(1, 0).unwrap().ch, 'f');
        let mut empty = Canvas::new(2, 1);
        empty.erase_text(0, 0, 4);
        assert!(!empty.has_text(), "erasing nothing does not allocate the layer");
    }

    #[test]
    fn wrapping_breaks_on_spaces_then_anywhere() {
        let lines: Vec<&str> = wrap("the quick brown fox", 9).collect();
        assert_eq!(lines, ["the quick", "brown fox"]);
        assert_eq!(wrap("supercalifragilistic", 6).collect::<Vec<_>>(), ["superc", "alifra", "gilist", "ic"]);
        assert_eq!(wrap("a\n\nb", 10).collect::<Vec<_>>(), ["a", "", "b"]);
        assert_eq!(wrap("no limit here", 0).collect::<Vec<_>>(), ["no limit here"]);
        assert_eq!(wrap("", 4).collect::<Vec<_>>(), [""]);
        assert_eq!(measure("ab\ncd\ne"), (2, 3));
    }

    #[test]
    fn wrapped_text_aligns() {
        let mut c = Canvas::new(8, 3);
        let size = c.print_wrapped(0, 0, 4, "ab cdef", TextStyle::default(), Align::Right);
        assert_eq!(size, (4, 2));
        assert_eq!(c.text_cell(2, 0).unwrap().ch, 'a');
        assert_eq!(c.text_cell(0, 1).unwrap().ch, 'c');
        c.clear_text();
        c.print_wrapped(0, 0, 6, "ab cdef", TextStyle::default(), Align::Center);
        assert_eq!(c.text_cell(2, 0).unwrap().ch, 'a');
        assert_eq!(c.text_cell(1, 1).unwrap().ch, 'c');
    }

    #[test]
    fn styles_carry_colours_and_attributes() {
        let s = TextStyle::new(Rgb::hex(0x112233)).on(Color::Indexed(4)).bold().italic();
        assert_eq!(s.fg, Some(Color::Rgb(Rgb::hex(0x112233))));
        assert_eq!(s.bg, Some(Color::Indexed(4)));
        assert!(s.attrs.has(Attrs::BOLD) && s.attrs.has(Attrs::ITALIC) && !s.attrs.has(Attrs::DIM));
        assert_eq!(TextStyle::from(0x00ff00u32).fg, Some(Color::Rgb(Rgb::hex(0x00ff00))));
    }
}
