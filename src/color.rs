//! Colour types: truecolor [`Rgb`], the terminal-palette aware [`Color`], and the
//! [`Palette`] that maps one onto the other.

/// An opaque 24-bit RGB colour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rgb {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
}

impl Rgb {
    /// Creates a colour from its channels.
    #[inline]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Creates a colour from a `0xRRGGBB` literal.
    #[inline]
    pub const fn hex(rgb: u32) -> Self {
        Self::new((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
    }

    /// Linear blend towards `other`; `t` is clamped to `0..=1`.
    pub fn lerp(self, other: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Rgb::new(mix(self.r, other.r), mix(self.g, other.g), mix(self.b, other.b))
    }

    /// Scales every channel by `f` (clamped to `0..=1`); `0.5` halves the brightness.
    pub fn dim(self, f: f32) -> Rgb {
        self.lerp(Rgb::hex(0), 1.0 - f.clamp(0.0, 1.0))
    }

    /// Relative luminance in `0..=1` (sRGB, WCAG weights), for contrast checks.
    pub fn luminance(self) -> f32 {
        let lin = |c: u8| {
            let c = c as f32 / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(self.r) + 0.7152 * lin(self.g) + 0.0722 * lin(self.b)
    }

    /// WCAG contrast ratio between two colours, `1..=21`.
    pub fn contrast(self, other: Rgb) -> f32 {
        let (a, b) = (self.luminance() + 0.05, other.luminance() + 0.05);
        if a > b {
            a / b
        } else {
            b / a
        }
    }

    #[inline]
    const fn packed_rgb(self) -> u32 {
        (self.r as u32) << 16 | (self.g as u32) << 8 | self.b as u32
    }

    #[inline]
    const fn from_packed_rgb(p: u32) -> Self {
        Self::new((p >> 16) as u8, (p >> 8) as u8, p as u8)
    }
}

impl From<(u8, u8, u8)> for Rgb {
    fn from((r, g, b): (u8, u8, u8)) -> Self {
        Self::new(r, g, b)
    }
}

impl From<u32> for Rgb {
    fn from(rgb: u32) -> Self {
        Self::hex(rgb)
    }
}

/// A dot colour: either an explicit [`Rgb`] or a reference into the terminal's own
/// palette, so a canvas can follow the user's colour scheme.
///
/// In the text fallback, palette colours are emitted as ordinary SGR indices
/// (`38;5;n`, or the default foreground) and the terminal applies its theme. In the
/// image protocols they are resolved through the [`Palette`] that
/// [`Terminal::detect`](crate::Terminal::detect) reads from the terminal (`OSC 4`,
/// `OSC 10`), which costs one table lookup per dot and nothing on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Color {
    /// An explicit 24-bit colour.
    Rgb(Rgb),
    /// One of the terminal's 256 palette entries (`0..=15` are the themed ANSI colours).
    Indexed(u8),
    /// The terminal's default foreground colour.
    Foreground,
}

impl Color {
    /// Tag stored in the high byte of a packed dot.
    const TAG_RGB: u32 = 0xFF00_0000;
    const TAG_INDEXED: u32 = 0xFE00_0000;
    const TAG_FOREGROUND: u32 = 0xFD00_0000;

    /// Resolves the colour to RGB through `palette`.
    #[inline]
    pub fn resolve(self, palette: &Palette) -> Rgb {
        match self {
            Color::Rgb(c) => c,
            Color::Indexed(i) => palette.colors[i as usize],
            Color::Foreground => palette.foreground,
        }
    }

    /// Packed form stored in a [`Canvas`](crate::Canvas); the high byte is a non-zero tag so
    /// `0` can mean "unset".
    #[inline]
    pub(crate) const fn packed(self) -> u32 {
        match self {
            Color::Rgb(c) => Self::TAG_RGB | c.packed_rgb(),
            Color::Indexed(i) => Self::TAG_INDEXED | i as u32,
            Color::Foreground => Self::TAG_FOREGROUND,
        }
    }

    #[inline]
    pub(crate) const fn from_packed(p: u32) -> Self {
        match p & 0xFF00_0000 {
            Self::TAG_INDEXED => Color::Indexed(p as u8),
            Self::TAG_FOREGROUND => Color::Foreground,
            _ => Color::Rgb(Rgb::from_packed_rgb(p)),
        }
    }

    /// Resolves a packed dot (non-zero) to RGB.
    #[inline]
    pub(crate) fn resolve_packed(p: u32, palette: &Palette) -> Rgb {
        match p & 0xFF00_0000 {
            Self::TAG_RGB => Rgb::from_packed_rgb(p),
            Self::TAG_INDEXED => palette.colors[(p & 0xFF) as usize],
            _ => palette.foreground,
        }
    }
}

impl From<Rgb> for Color {
    fn from(c: Rgb) -> Self {
        Color::Rgb(c)
    }
}

impl From<(u8, u8, u8)> for Color {
    fn from(c: (u8, u8, u8)) -> Self {
        Color::Rgb(c.into())
    }
}

impl From<u32> for Color {
    fn from(rgb: u32) -> Self {
        Color::Rgb(Rgb::hex(rgb))
    }
}

/// The terminal's colour scheme: 256 palette entries plus default foreground and
/// background. Used to turn [`Color::Indexed`] and [`Color::Foreground`] dots into
/// pixels for the image protocols.
///
/// [`Palette::default`] is the xterm palette. [`Terminal::detect`](crate::Terminal::detect)
/// replaces entries `0..=15`, the foreground and the background with what the terminal
/// reports; the 6×6×6 cube and the grey ramp are the same on every terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// Entries `0..=255`.
    pub colors: [Rgb; 256],
    /// Default foreground.
    pub foreground: Rgb,
    /// Default background.
    pub background: Rgb,
}

impl Default for Palette {
    fn default() -> Self {
        const ANSI: [u32; 16] = [
            0x000000, 0xcd0000, 0x00cd00, 0xcdcd00, 0x0000ee, 0xcd00cd, 0x00cdcd, 0xe5e5e5, 0x7f7f7f, 0xff0000,
            0x00ff00, 0xffff00, 0x5c5cff, 0xff00ff, 0x00ffff, 0xffffff,
        ];
        const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
        let mut colors = [Rgb::default(); 256];
        for (i, &c) in ANSI.iter().enumerate() {
            colors[i] = Rgb::hex(c);
        }
        for i in 0..216 {
            colors[16 + i] = Rgb::new(LEVELS[i / 36], LEVELS[i / 6 % 6], LEVELS[i % 6]);
        }
        for i in 0..24 {
            let g = 8 + 10 * i as u8;
            colors[232 + i] = Rgb::new(g, g, g);
        }
        Self { colors, foreground: Rgb::hex(0xdddddd), background: Rgb::hex(0x000000) }
    }
}

impl Palette {
    /// `true` when the background is light, i.e. the terminal runs a light theme.
    pub fn is_light(&self) -> bool {
        self.background.luminance() > 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_every_kind() {
        for c in [Color::Rgb(Rgb::hex(0x102030)), Color::Indexed(9), Color::Indexed(0), Color::Foreground] {
            assert_ne!(c.packed(), 0);
            assert_eq!(Color::from_packed(c.packed()), c);
        }
        let p = Palette::default();
        assert_eq!(Color::resolve_packed(Color::Indexed(9).packed(), &p), Rgb::hex(0xff0000));
        assert_eq!(Color::resolve_packed(Color::Foreground.packed(), &p), p.foreground);
        assert_eq!(Color::resolve_packed(Color::Rgb(Rgb::hex(0xabcdef)).packed(), &p), Rgb::hex(0xabcdef));
    }

    #[test]
    fn xterm_palette() {
        let p = Palette::default();
        assert_eq!(p.colors[16], Rgb::hex(0x000000));
        assert_eq!(p.colors[21], Rgb::hex(0x0000ff));
        assert_eq!(p.colors[196], Rgb::hex(0xff0000));
        assert_eq!(p.colors[231], Rgb::hex(0xffffff));
        assert_eq!(p.colors[232], Rgb::hex(0x080808));
        assert_eq!(p.colors[255], Rgb::hex(0xeeeeee));
        assert!(!p.is_light());
    }

    #[test]
    fn contrast_ratio() {
        assert!((Rgb::hex(0xffffff).contrast(Rgb::hex(0)) - 21.0).abs() < 0.01);
        assert!((Rgb::hex(0x777777).contrast(Rgb::hex(0x777777)) - 1.0).abs() < 0.01);
    }
}
