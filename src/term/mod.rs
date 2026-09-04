//! Terminal capability detection.
//!
//! Two things matter for drawing an image that lines up with the character grid:
//! which graphics protocol the terminal speaks, and how many pixels one cell is.
//! Terminals do not expose the font name or point size; the cell box in pixels is
//! the only geometry they publish, and it is all that is needed for alignment.
//!
//! Detection order in [`Terminal::detect`]:
//!
//! 1. `COBRA_PROTOCOL` / `COBRA_CELL` environment overrides.
//! 2. Not a tty → [`Protocol::Text`].
//! 3. Cell size from `TIOCGWINSZ` (pixel fields), which costs one `ioctl`.
//! 4. Protocol from well-known environment variables ([`Protocol::from_env`]).
//! 5. One round trip on `/dev/tty`: a kitty graphics probe and `CSI 16 t` for the
//!    cell size when still unknown, `OSC 4` / `OSC 10` / `OSC 11` for the palette,
//!    and `DA1` for sixel (which also terminates the response).
//! 6. Anything without a usable cell size falls back to [`Protocol::Text`].

#[cfg(all(feature = "detect", unix))]
mod query;

use crate::Palette;

/// How the canvas gets onto the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Protocol {
    /// Braille glyphs, one foreground colour per cell. Works everywhere, copyable.
    Text,
    /// Kitty graphics protocol (kitty, WezTerm, Ghostty, Konsole ≥ 22.04, …).
    Kitty,
    /// iTerm2 inline images (iTerm2, WezTerm, mintty, Konsole, …).
    Iterm2,
    /// DEC sixel (foot, xterm, mlterm, Windows Terminal ≥ 1.22, …).
    Sixel,
}

impl Protocol {
    /// Parses `text` / `kitty` / `iterm2` / `sixel` (case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "text" | "braille" => Some(Self::Text),
            "kitty" => Some(Self::Kitty),
            "iterm2" | "iterm" => Some(Self::Iterm2),
            "sixel" => Some(Self::Sixel),
            _ => None,
        }
    }

    /// Guesses the protocol from environment variables alone, without touching the tty.
    ///
    /// Returns `None` when nothing conclusive is set; `Some(Text)` inside multiplexers
    /// that do not pass graphics through (tmux, screen).
    pub fn from_env() -> Option<Self> {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let term = var("TERM").unwrap_or_default();
        if var("TMUX").is_some() || term.starts_with("screen") || term.starts_with("tmux") {
            return Some(Self::Text);
        }
        if var("KITTY_WINDOW_ID").is_some() || term == "xterm-kitty" {
            return Some(Self::Kitty);
        }
        if var("GHOSTTY_RESOURCES_DIR").is_some() || term == "xterm-ghostty" {
            return Some(Self::Kitty);
        }
        if term.starts_with("foot") {
            return Some(Self::Sixel);
        }
        match var("TERM_PROGRAM").as_deref() {
            Some("WezTerm") | Some("ghostty") => Some(Self::Kitty),
            Some("iTerm.app") => Some(Self::Iterm2),
            _ => None,
        }
    }
}

/// Size of one character cell in pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CellSize {
    /// Cell width in pixels.
    pub width: u16,
    /// Cell height in pixels.
    pub height: u16,
}

impl CellSize {
    /// `true` when both dimensions are non-zero.
    #[inline]
    pub fn is_known(self) -> bool {
        self.width > 0 && self.height > 0
    }

    /// Parses `WxH` (e.g. `9x18`).
    pub fn parse(s: &str) -> Option<Self> {
        let (w, h) = s.split_once(['x', 'X'])?;
        Some(Self { width: w.trim().parse().ok()?, height: h.trim().parse().ok()? })
    }
}

/// What was learned about the terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Terminal {
    /// Best protocol available.
    pub protocol: Protocol,
    /// Pixel size of one cell (zero when unknown; then `protocol` is `Text`).
    pub cell: CellSize,
    /// Terminal width in cells, `0` when unknown.
    pub cols: u16,
    /// Terminal height in cells, `0` when unknown.
    pub rows: u16,
    /// The terminal's colour scheme, used to draw [`Color::Indexed`](crate::Color::Indexed)
    /// and [`Color::Foreground`](crate::Color::Foreground) dots in the image protocols.
    /// The xterm defaults until [`detect`](Self::detect) learns better.
    pub palette: Palette,
    /// Whether `palette` was reported by the terminal rather than assumed.
    pub palette_queried: bool,
}

impl Terminal {
    /// A terminal that only gets braille text. Always safe.
    pub fn text() -> Self {
        Self::new(Protocol::Text, CellSize::default())
    }

    /// Builds a terminal description by hand (for tests, or when you know better).
    /// Image protocols with an unknown cell size are demoted to text.
    pub fn new(protocol: Protocol, cell: CellSize) -> Self {
        let protocol = if cell.is_known() { protocol } else { Protocol::Text };
        Self { protocol, cell, cols: 0, rows: 0, palette: Palette::default(), palette_queried: false }
    }

    /// Replaces the palette (builder style).
    pub fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self.palette_queried = true;
        self
    }

    /// Whether frames are transmitted as images rather than glyphs.
    #[inline]
    pub fn is_graphical(&self) -> bool {
        self.protocol != Protocol::Text
    }

    /// Detects the terminal on stdout / `/dev/tty`.
    ///
    /// Order: `COBRA_PROTOCOL` / `COBRA_CELL` overrides, tty check, cell size from
    /// `TIOCGWINSZ`, protocol from the environment ([`Protocol::from_env`]), then one
    /// escape-sequence round trip that asks for whatever is still unknown (kitty probe,
    /// `CSI 16 t`, `DA1`) plus the colour scheme (`OSC 4`, `OSC 10`, `OSC 11`). Image
    /// protocols without a known cell size fall back to [`Protocol::Text`].
    ///
    /// Costs one `ioctl` plus one escape-sequence round trip, bounded by a short
    /// timeout and normally ending as soon as the terminal answers `DA1` (a few
    /// milliseconds). Call it once at start-up and keep the result. Set
    /// `COBRA_PALETTE=0` to skip the colour queries.
    #[cfg(feature = "detect")]
    pub fn detect() -> Self {
        let override_protocol = std::env::var("COBRA_PROTOCOL").ok().and_then(|s| Protocol::parse(&s));
        let override_cell = std::env::var("COBRA_CELL").ok().and_then(|s| CellSize::parse(&s));
        Self::detect_inner(override_protocol, override_cell)
    }

    #[cfg(all(feature = "detect", unix))]
    fn detect_inner(protocol: Option<Protocol>, cell: Option<CellSize>) -> Self {
        if protocol == Some(Protocol::Text) || (protocol.is_none() && !query::is_tty()) {
            return Self::text();
        }
        let ws = query::winsize();
        let mut t = Self { cell: cell.unwrap_or(ws.cell), cols: ws.cols, rows: ws.rows, ..Self::text() };

        let mut protocol = protocol.or_else(Protocol::from_env);
        let want_palette = std::env::var("COBRA_PALETTE").map_or(true, |v| v != "0");
        // Inside a multiplexer the queries would be swallowed or misrouted; skip them.
        let multiplexed = protocol == Some(Protocol::Text) && std::env::var("TMUX").is_ok();
        if (protocol.is_none() || !t.cell.is_known() || want_palette) && !multiplexed {
            let probe = query::probe(protocol.is_none(), !t.cell.is_known(), want_palette);
            if !t.cell.is_known() {
                if let Some(c) = probe.cell {
                    t.cell = c;
                }
            }
            if protocol.is_none() {
                protocol = Some(if probe.kitty {
                    Protocol::Kitty
                } else if probe.sixel {
                    Protocol::Sixel
                } else {
                    Protocol::Text
                });
            }
            if probe.palette_entries > 0 {
                t.palette = probe.palette;
                t.palette_queried = true;
            }
        }
        if t.cell.is_known() {
            t.protocol = protocol.unwrap_or(Protocol::Text);
        }
        t
    }

    #[cfg(all(feature = "detect", not(unix)))]
    fn detect_inner(protocol: Option<Protocol>, cell: Option<CellSize>) -> Self {
        // No tty queries on this platform: honour explicit overrides, otherwise text.
        match (protocol.or_else(Protocol::from_env), cell) {
            (Some(p), Some(c)) => Self::new(p, c),
            _ => Self::text(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing() {
        assert_eq!(Protocol::parse(" Kitty "), Some(Protocol::Kitty));
        assert_eq!(Protocol::parse("nope"), None);
        assert_eq!(CellSize::parse("9x18"), Some(CellSize { width: 9, height: 18 }));
        assert_eq!(CellSize::parse("9"), None);
    }

    #[test]
    fn unknown_cell_demotes_to_text() {
        assert_eq!(Terminal::new(Protocol::Kitty, CellSize::default()).protocol, Protocol::Text);
        assert_eq!(Terminal::new(Protocol::Kitty, CellSize { width: 8, height: 16 }).protocol, Protocol::Kitty);
    }
}
