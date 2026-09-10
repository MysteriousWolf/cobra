//! Export a canvas as a transparent PNG or SVG with the same cell-aligned dot
//! geometry the image protocols draw, so a file looks like the terminal does.
//!
//! ```no_run
//! use cobra::{export, Canvas, Rgb};
//!
//! let mut canvas = Canvas::new(10, 3);
//! canvas.disc(10.0, 6.0, 4.0, Rgb::hex(0x5ec33a));
//! std::fs::write("dots.png", export::png(&canvas, &export::Style::default())).unwrap();
//! std::fs::write("dots.svg", export::svg(&canvas, &export::Style::default())).unwrap();
//! ```

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::raster::Raster;
use crate::{Canvas, CellSize, Color, Palette, Rgb};

/// Geometry and colours for an export.
#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    /// Pixel size of one cell. Default `10×20`.
    pub cell: CellSize,
    /// Dot diameter as a fraction of its slot, as in [`Options`](crate::Options). Default `0.7`.
    pub dot_size: f32,
    /// Background fill; `None` (the default) keeps it transparent.
    pub background: Option<Rgb>,
    /// Resolves [`Color::Indexed`] and [`Color::Foreground`] dots. Default: xterm.
    pub palette: Palette,
    /// Draw the [text layer](crate::text). A file has no terminal font, so [`svg`]
    /// writes `<text>` elements and [`png`] draws the characters with
    /// [`Font::mono`](crate::Font::mono), a 5×9 monospace face scaled to the cell,
    /// which covers printable ASCII. Default `true`.
    pub text: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            cell: CellSize { width: 10, height: 20 },
            dot_size: 0.7,
            background: None,
            palette: Palette::default(),
            text: true,
        }
    }
}

impl Style {
    /// Scales the cell (and thus the whole image) by an integer factor, e.g. for a
    /// crisp README logo.
    pub fn scale(mut self, k: u16) -> Self {
        self.cell = CellSize { width: self.cell.width * k, height: self.cell.height * k };
        self
    }
}

/// Encodes `canvas` as an RGBA PNG. Unset dots are transparent unless
/// `style.background` is set.
pub fn png(canvas: &Canvas, style: &Style) -> Vec<u8> {
    let mut raster = Raster::default();
    let font = style.text.then(crate::Font::mono);
    let (w, h) = raster.draw(canvas, style.cell, style.dot_size, &style.palette, canvas.cols(), canvas.rows(), font);
    if let Some(bg) = style.background {
        for px in raster.rgba.as_chunks_mut::<4>().0 {
            if px[3] == 0 {
                *px = [bg.r, bg.g, bg.b, 255];
            }
        }
    }
    let (mut scratch, mut out) = (Vec::new(), Vec::new());
    let dists = [1, style.cell.width as usize, w as usize, w as usize * (style.cell.height as usize / 4).max(1)];
    crate::encode::png::encode(&mut Default::default(), &raster.rgba, w, h, &dists, &mut scratch, &mut out);
    out
}

/// Renders `canvas` as an SVG document of one circle per set dot, positioned on the
/// same cell grid as [`png`]. The `viewBox` is in pixels of `style.cell`.
pub fn svg(canvas: &Canvas, style: &Style) -> String {
    let (cw, ch) = (style.cell.width as f32, style.cell.height as f32);
    let (slot_w, slot_h) = (cw / 2.0, ch / 4.0);
    let r = style.dot_size.clamp(0.0, 1.0) * slot_w.min(slot_h) / 2.0;
    let (w, h) = (canvas.cols() as f32 * cw, canvas.rows() as f32 * ch);
    let mut s = String::with_capacity(4096);
    let _ = writeln!(s, r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}">"#);
    if let Some(bg) = style.background {
        let _ = writeln!(s, r##"<rect width="100%" height="100%" fill="#{:02x}{:02x}{:02x}"/>"##, bg.r, bg.g, bg.b);
    }
    // Group dots by colour so the file stays small and stylable. A map keyed by the
    // packed colour keeps the output deterministic however many colours a canvas uses.
    let mut groups: BTreeMap<u32, String> = BTreeMap::new();
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            let Some(color) = canvas.get(x, y) else { continue };
            let c = color.resolve(&style.palette);
            let key = (c.r as u32) << 16 | (c.g as u32) << 8 | c.b as u32;
            let (cx, cy) = ((x as f32 + 0.5) * slot_w, (y as f32 + 0.5) * slot_h);
            let group = groups.entry(key).or_default();
            let _ = write!(group, r#"<circle cx="{cx}" cy="{cy}" r="{r}"/>"#);
        }
    }
    for (key, circles) in groups {
        let _ = writeln!(s, r##"<g fill="#{key:06x}">{circles}</g>"##);
    }
    if style.text {
        svg_text(canvas, style, &mut s);
    }
    s.push_str("</svg>\n");
    s
}

/// Writes the text layer as real `<text>` elements: a file keeps text as text.
fn svg_text(canvas: &Canvas, style: &Style, s: &mut String) {
    let (cw, ch) = (style.cell.width as f32, style.cell.height as f32);
    // A monospace advance is about 0.6 em, so this is the size whose glyphs fill a cell.
    let size = (cw / 0.6).min(ch * 0.8);
    let hex = |c: Color| {
        let c = c.resolve(&style.palette);
        format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
    };
    for row in 0..canvas.rows() {
        for col in 0..canvas.cols() {
            let Some(cell) = canvas.text_cell(col as i32, row as i32) else { continue };
            let attrs = cell.style.attrs;
            // Reverse video swaps the two colours, the terminal's defaults standing in
            // for the ones that were not set.
            let (fg, bg) = if attrs.has(crate::Attrs::REVERSE) {
                (
                    Some(cell.style.bg.unwrap_or(Color::Rgb(style.palette.background))),
                    Some(cell.style.fg.unwrap_or(Color::Foreground)),
                )
            } else {
                (cell.style.fg, cell.style.bg)
            };
            if let Some(bg) = bg {
                let _ = writeln!(
                    s,
                    r#"<rect x="{}" y="{}" width="{cw}" height="{ch}" fill="{}"/>"#,
                    col as f32 * cw,
                    row as f32 * ch,
                    hex(bg)
                );
            }
            if cell.is_continuation() {
                continue;
            }
            let escaped = match cell.ch {
                '&' => "&amp;".to_string(),
                '<' => "&lt;".to_string(),
                '>' => "&gt;".to_string(),
                ch => ch.to_string(),
            };
            let weight = if attrs.has(crate::Attrs::BOLD) { r#" font-weight="bold""# } else { "" };
            let italic = if attrs.has(crate::Attrs::ITALIC) { r#" font-style="italic""# } else { "" };
            let under = if attrs.has(crate::Attrs::UNDERLINE) { r#" text-decoration="underline""# } else { "" };
            let dim = if attrs.has(crate::Attrs::DIM) { r#" opacity="0.6""# } else { "" };
            let _ = writeln!(
                s,
                r#"<text x="{}" text-anchor="middle" y="{}" font-family="monospace" font-size="{size}" fill="{}"{weight}{italic}{under}{dim}>{escaped}</text>"#,
                col as f32 * cw + cw / 2.0,
                (row + 1) as f32 * ch - ch * 0.25,
                hex(fg.unwrap_or(Color::Foreground)),
            );
        }
    }
}

/// Resolves every dot of `canvas` to RGB through `palette`; useful before exporting
/// a canvas that uses terminal colours to a file that has no terminal.
pub fn resolve(canvas: &Canvas, palette: &Palette) -> Canvas {
    let mut out = canvas.clone();
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            if let Some(c) = canvas.get(x, y) {
                out.set(x, y, Color::Rgb(c.resolve(palette)));
            }
        }
    }
    for row in 0..canvas.rows() as i32 {
        for col in 0..canvas.cols() as i32 {
            let Some(cell) = canvas.text_cell(col, row) else { continue };
            let baked = |c: Option<Color>| c.map(|c| Color::Rgb(c.resolve(palette)));
            let style = crate::TextStyle { fg: baked(cell.style.fg), bg: baked(cell.style.bg), ..cell.style };
            out.print(col, row, &cell.ch.to_string(), style);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_is_transparent_unless_asked() {
        let mut c = Canvas::new(1, 1);
        c.set(0, 0, Rgb::hex(0xff0000));
        let p = png(&c, &Style::default());
        assert_eq!(&p[..8], b"\x89PNG\r\n\x1a\n");
        // IHDR: width 10, height 20, 8-bit RGBA.
        assert_eq!(&p[16..24], &[0, 0, 0, 10, 0, 0, 0, 20]);
        assert_eq!(p[25], 6);
        let q = png(&c, &Style { background: Some(Rgb::hex(0x101418)), ..Style::default() });
        assert_ne!(p, q);
    }

    #[test]
    fn svg_groups_by_colour() {
        let mut c = Canvas::new(2, 1);
        c.set(0, 0, Rgb::hex(0xff0000));
        c.set(1, 0, Rgb::hex(0xff0000));
        c.set(2, 3, Color::Indexed(4));
        let s = svg(&c, &Style::default());
        assert!(s.starts_with("<svg") && s.ends_with("</svg>\n"));
        assert!(!s.contains("<rect"));
        assert_eq!(s.matches("<g fill=").count(), 2);
        assert!(s.contains(r##"<g fill="#0000ee">"##));
        assert!(s.contains(r#"<circle cx="2.5" cy="2.5" r="1.75"/><circle cx="7.5" cy="2.5" r="1.75"/>"#));
    }

    #[test]
    fn svg_text_carries_every_attribute() {
        let mut c = Canvas::new(4, 1);
        let style = crate::TextStyle::new(Rgb::hex(0x102030)).underline().dim();
        c.print(0, 0, "a", style);
        c.print(1, 0, "b", crate::TextStyle::new(Rgb::hex(0x102030)).with(crate::Attrs::REVERSE));
        let s = svg(&c, &Style::default());
        assert!(s.contains(r#"text-decoration="underline""#) && s.contains(r#"opacity="0.6""#), "{s}");
        // Reverse video: the ink becomes the cell's background and the default background the ink.
        assert!(s.contains(r##"<rect x="10" y="0" width="10" height="20" fill="#102030"/>"##), "{s}");
        assert!(s.contains(r##"fill="#000000">b</text>"##), "{s}");
    }

    #[test]
    fn resolve_bakes_palette() {
        let mut c = Canvas::new(1, 1);
        c.set(0, 0, Color::Foreground);
        let r = resolve(&c, &Palette::default());
        assert_eq!(r.get(0, 0), Some(Color::Rgb(Palette::default().foreground)));
    }
}
