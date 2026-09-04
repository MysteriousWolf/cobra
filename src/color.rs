//! Colour type.

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

    /// Packed `0xFF_RR_GG_BB`; the high byte marks "dot is set" inside a [`Canvas`](crate::Canvas).
    #[inline]
    pub(crate) const fn packed(self) -> u32 {
        0xFF00_0000 | (self.r as u32) << 16 | (self.g as u32) << 8 | self.b as u32
    }

    #[inline]
    pub(crate) const fn from_packed(p: u32) -> Self {
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
