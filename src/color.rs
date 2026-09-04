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

/// How many colours the terminal can show in text: what the braille fallback
/// quantises [`Color::Rgb`] dots to.
///
/// Image protocols always get full RGB; the depth only matters for
/// [`Protocol::Text`](crate::Protocol::Text) and the ratatui widget.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Depth {
    /// No colour: every dot is drawn in the default foreground.
    Mono,
    /// The 16 ANSI colours (`SGR 30–37`, `90–97`), matched against the terminal's
    /// real palette when it was queried.
    Ansi16,
    /// The xterm 256-colour palette: a 6×6×6 cube and a 24-step grey ramp.
    Ansi256,
    /// 24-bit `SGR 38;2;r;g;b`.
    TrueColor,
}

impl Depth {
    /// Parses `mono` / `16` / `256` / `true` (also `truecolor`, `24bit`, `ansi`).
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "mono" | "none" | "0" | "1" | "2" => Some(Self::Mono),
            "16" | "ansi" | "ansi16" => Some(Self::Ansi16),
            "256" | "ansi256" => Some(Self::Ansi256),
            "true" | "truecolor" | "24bit" | "rgb" | "16m" => Some(Self::TrueColor),
            _ => None,
        }
    }

    /// Guesses the depth from the environment, without touching the tty.
    ///
    /// `NO_COLOR` wins, then `COLORTERM`, then well-known terminal programs, then
    /// `TERM`. An unknown but non-empty `TERM` is assumed to be 256-colour capable,
    /// which every terminal of the last two decades is; `dumb`, `vt*` and `ansi`
    /// are not.
    pub fn from_env() -> Self {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        if var("NO_COLOR").is_some() {
            return Self::Mono;
        }
        let term = var("TERM").unwrap_or_default().to_ascii_lowercase();
        if term == "dumb" {
            return Self::Mono;
        }
        if matches!(var("COLORTERM").as_deref().map(str::to_ascii_lowercase).as_deref(), Some("truecolor" | "24bit")) {
            return Self::TrueColor;
        }
        if term.contains("direct") || term.contains("truecolor") {
            return Self::TrueColor;
        }
        match var("TERM_PROGRAM").as_deref() {
            Some("Apple_Terminal") => return Self::Ansi256,
            Some("iTerm.app" | "WezTerm" | "ghostty" | "vscode" | "Hyper" | "rio" | "Tabby") => return Self::TrueColor,
            _ => {}
        }
        if var("WT_SESSION").is_some() || var("KONSOLE_VERSION").is_some() || var("KITTY_WINDOW_ID").is_some() {
            return Self::TrueColor;
        }
        if var("VTE_VERSION").and_then(|v| v.parse::<u32>().ok()).is_some_and(|v| v >= 3600) {
            return Self::TrueColor;
        }
        for t in ["xterm-kitty", "xterm-ghostty", "wezterm", "foot", "alacritty", "contour", "st-256color", "rio"] {
            if term.starts_with(t) {
                return Self::TrueColor;
            }
        }
        if term.contains("256") {
            return Self::Ansi256;
        }
        if term.is_empty() || term.starts_with("vt") || term == "ansi" || term == "linux" {
            return Self::Ansi16;
        }
        Self::Ansi256
    }
}

/// Squared perceptual distance between two colours ("redmean": a cheap, well-tested
/// weighting of the RGB channels that tracks how the eye sees differences).
#[inline]
fn distance(a: Rgb, b: Rgb) -> u32 {
    let rm = (a.r as i32 + b.r as i32) / 2;
    let (dr, dg, db) = (a.r as i32 - b.r as i32, a.g as i32 - b.g as i32, a.b as i32 - b.b as i32);
    (((512 + rm) * dr * dr) >> 8) as u32 + (4 * dg * dg) as u32 + (((767 - rm) * db * db) >> 8) as u32
}

impl Color {
    /// The closest colour the terminal can show at `depth`; a no-op at
    /// [`Depth::TrueColor`]. Palette indices are kept where the depth has them and
    /// resolved through `palette` where it does not.
    pub fn quantize(self, depth: Depth, palette: &Palette) -> Color {
        match (depth, self) {
            (Depth::TrueColor, c) | (_, c @ Color::Foreground) => c,
            (Depth::Mono, _) => Color::Foreground,
            (Depth::Ansi256, c @ Color::Indexed(_)) | (Depth::Ansi16, c @ Color::Indexed(0..=15)) => c,
            (Depth::Ansi256, Color::Rgb(c)) => Color::Indexed(nearest_256(c)),
            (Depth::Ansi16, c) => Color::Indexed(palette.nearest_ansi(c.resolve(palette))),
        }
    }
}

/// Nearest entry of the fixed part of the xterm palette (`16..=255`): the closest
/// cube corner and the closest grey are each found analytically and the better one wins.
fn nearest_256(c: Rgb) -> u8 {
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let level = |v: u8| -> usize {
        if v < 48 {
            0
        } else if v < 115 {
            1
        } else {
            (v as usize - 35) / 40
        }
    };
    let (ri, gi, bi) = (level(c.r), level(c.g), level(c.b));
    let cube = Rgb::new(LEVELS[ri], LEVELS[gi], LEVELS[bi]);
    let avg = (c.r as u32 + c.g as u32 + c.b as u32) / 3;
    let gi = if avg < 8 { 0 } else { ((avg - 8 + 5) / 10).min(23) } as u8;
    let grey = Rgb::new(8 + 10 * gi, 8 + 10 * gi, 8 + 10 * gi);
    if distance(c, grey) < distance(c, cube) {
        232 + gi
    } else {
        16 + (ri * 36 + level(c.g) * 6 + bi) as u8
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

    /// Index of the ANSI colour (`0..=15`) closest to `c` in this palette.
    pub fn nearest_ansi(&self, c: Rgb) -> u8 {
        let mut best = (u32::MAX, 0u8);
        for (i, &p) in self.colors[..16].iter().enumerate() {
            let d = distance(c, p);
            if d < best.0 {
                best = (d, i as u8);
            }
        }
        best.1
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
    fn quantise_to_256() {
        let p = Palette::default();
        let q = |rgb: u32| Color::Rgb(Rgb::hex(rgb)).quantize(Depth::Ansi256, &p);
        assert_eq!(q(0xff0000), Color::Indexed(196));
        assert_eq!(q(0x000000), Color::Indexed(16));
        assert_eq!(q(0xffffff), Color::Indexed(231));
        assert_eq!(q(0x808080), Color::Indexed(244));
        assert_eq!(q(0x5f87d7), Color::Indexed(68));
        // Every fixed entry maps onto itself.
        for i in 16..=255u8 {
            assert_eq!(Color::Rgb(p.colors[i as usize]).quantize(Depth::Ansi256, &p), Color::Indexed(i), "{i}");
        }
        assert_eq!(Color::Indexed(3).quantize(Depth::Ansi256, &p), Color::Indexed(3));
        assert_eq!(Color::Foreground.quantize(Depth::Ansi256, &p), Color::Foreground);
    }

    #[test]
    fn quantise_to_16_and_mono() {
        let p = Palette::default();
        assert_eq!(Color::Rgb(Rgb::hex(0xff2010)).quantize(Depth::Ansi16, &p), Color::Indexed(9));
        assert_eq!(Color::Rgb(Rgb::hex(0x0000aa)).quantize(Depth::Ansi16, &p), Color::Indexed(4));
        assert_eq!(Color::Indexed(196).quantize(Depth::Ansi16, &p), Color::Indexed(9));
        assert_eq!(Color::Indexed(12).quantize(Depth::Ansi16, &p), Color::Indexed(12));
        assert_eq!(Color::Rgb(Rgb::hex(0x123456)).quantize(Depth::Mono, &p), Color::Foreground);
        assert_eq!(Color::Rgb(Rgb::hex(0x123456)).quantize(Depth::TrueColor, &p), Color::Rgb(Rgb::hex(0x123456)));
        assert_eq!(Depth::parse("TRUECOLOR"), Some(Depth::TrueColor));
        assert_eq!(Depth::parse("16"), Some(Depth::Ansi16));
        assert_eq!(Depth::parse("x"), None);
    }

    #[test]
    fn contrast_ratio() {
        assert!((Rgb::hex(0xffffff).contrast(Rgb::hex(0)) - 21.0).abs() < 0.01);
        assert!((Rgb::hex(0x777777).contrast(Rgb::hex(0x777777)) - 1.0).abs() < 0.01);
    }
}
