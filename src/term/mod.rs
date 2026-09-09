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
//!
//! # Multiplexers
//!
//! Inside tmux the terminal that draws is not the one the program talks to, and
//! tmux answers queries itself, so nothing is asked over the tty. The outer
//! terminal is instead recognised from the variables it leaves in tmux's
//! environment (`GHOSTTY_RESOURCES_DIR`, `KITTY_WINDOW_ID`, `WEZTERM_EXECUTABLE`,
//! `ITERM_SESSION_ID`), the cell size comes from `TIOCGWINSZ` (tmux ≥ 3.2 passes the
//! pixel size on), and every image is wrapped in tmux's passthrough sequence
//! ([`Terminal::passthrough`]), which reaches the outer terminal only when tmux has
//! `allow-passthrough on` (`set -g allow-passthrough on` in `tmux.conf`). Without
//! it the images are silently dropped; `COBRA_PROTOCOL=text` opts out. A terminal
//! started from inside tmux inherits `TMUX` without being a pane, and would take the
//! wrapped image as an unknown `DCS` (Ghostty crashed on one), so nothing about the
//! environment is believed on its own: tmux is asked over its socket whether the pane
//! it names draws on this process's tty, and only that answer makes a pane. `TERM`
//! never does, since a shell's start-up files may set it long after the terminal did.
//! Being wrong the other way only costs the images, so a tmux that cannot be run, or
//! answers about another tty, is not a pane; `COBRA_PASSTHROUGH=1` wraps them anyway
//! and `COBRA_PASSTHROUGH=0` never does. GNU screen passes nothing through and gets
//! text.

#[cfg(all(feature = "detect", unix))]
mod query;

use crate::{Depth, Palette};

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
    /// Returns `None` when nothing conclusive is set, and `Some(Text)` under GNU
    /// screen, which passes no graphics through. Under tmux the outer terminal is
    /// recognised by the variables it leaves in tmux's environment, since `TERM` and
    /// `TERM_PROGRAM` there are tmux's own; see [`Terminal::passthrough`].
    pub fn from_env() -> Option<Self> {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let term = var("TERM").unwrap_or_default();
        let tmux = in_tmux();
        if !tmux && term.starts_with("screen") {
            return Some(Self::Text);
        }
        // Set by the terminal for every process it starts, and kept by a tmux server
        // started from it, so they identify the outer terminal from inside.
        if var("KITTY_WINDOW_ID").is_some() || term == "xterm-kitty" {
            return Some(Self::Kitty);
        }
        if var("GHOSTTY_RESOURCES_DIR").is_some() || term == "xterm-ghostty" {
            return Some(Self::Kitty);
        }
        if var("WEZTERM_EXECUTABLE").is_some() || var("WEZTERM_PANE").is_some() {
            return Some(Self::Kitty);
        }
        if var("ITERM_SESSION_ID").is_some() || var("LC_TERMINAL").as_deref() == Some("iTerm2") {
            return Some(Self::Iterm2);
        }
        if term.starts_with("foot") {
            return Some(Self::Sixel);
        }
        match var("TERM_PROGRAM").as_deref() {
            Some("WezTerm" | "ghostty") => Some(Self::Kitty),
            Some("iTerm.app") => Some(Self::Iterm2),
            _ => None,
        }
    }
}

/// Whether the process runs in a tmux pane. Decided once per process.
///
/// `TMUX` alone does not say: a terminal started from inside tmux hands the
/// variable on to everything it runs, and an image wrapped for tmux that reaches
/// such a terminal directly is an unknown `DCS` to it (Ghostty crashed on one).
/// Nor does `TERM`: tmux sets a pane's to `tmux-*` or `screen-*` and a terminal
/// sets its own, but a shell's start-up files may set it again, and one that
/// exports `tmux-256color` whenever `TMUX` is set makes the new terminal look like
/// a pane. So tmux itself is asked, over the socket named in `TMUX`, whether the
/// pane it thinks this is draws on this process's tty, and only that answer makes
/// a pane: a socket that cannot be reached, a tty that differs, a tmux that cannot
/// be run at all are all "no". The two mistakes do not cost the same -- guessing
/// pane wrongly writes a `DCS` at a terminal that never asked for one, guessing
/// terminal wrongly only drops the images inside tmux, which `COBRA_PASSTHROUGH=1`
/// brings back -- so the doubtful cases go the cheap way.
fn in_tmux() -> bool {
    static IN_TMUX: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *IN_TMUX.get_or_init(|| {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        in_tmux_given(var("TMUX").as_deref(), |tmux| {
            let socket = tmux.split(',').next().unwrap_or(tmux);
            pane_tty(socket, var("TMUX_PANE").as_deref()).is_some_and(|pane| is_own_tty(&pane))
        })
    })
}

/// The decision behind [`in_tmux`], with the tmux round trip supplied: `ask` answers
/// whether the pane tmux names draws on this process's tty. Both halves are needed,
/// and nothing stands in for the answer.
fn in_tmux_given(tmux: Option<&str>, ask: impl FnOnce(&str) -> bool) -> bool {
    tmux.is_some_and(ask)
}

/// The tty of the tmux pane `pane` (`TMUX_PANE`, or tmux's current pane) on `socket`,
/// `None` when tmux could not be run or knows no such pane or socket.
#[cfg(all(feature = "detect", unix))]
fn pane_tty(socket: &str, pane: Option<&str>) -> Option<String> {
    let mut cmd = std::process::Command::new("tmux");
    cmd.args(["-S", socket, "display-message", "-p"]);
    if let Some(pane) = pane {
        cmd.args(["-t", pane]);
    }
    let out =
        cmd.arg("#{pane_tty}").stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

#[cfg(not(all(feature = "detect", unix)))]
fn pane_tty(_socket: &str, _pane: Option<&str>) -> Option<String> {
    None
}

/// Whether `path` names the terminal this process draws on. Compared as devices,
/// since tmux and `ttyname` need not spell the same terminal the same way, and by
/// path when either device cannot be read.
#[cfg(all(feature = "detect", unix))]
fn is_own_tty(path: &str) -> bool {
    match (query::tty_device(), query::device_at(path)) {
        (Some(own), Some(named)) => own == named,
        _ => query::tty_name().is_some_and(|own| own == path),
    }
}

#[cfg(not(all(feature = "detect", unix)))]
fn is_own_tty(_path: &str) -> bool {
    false
}

/// `1|true|yes|on` and `0|false|no|off`, for the environment overrides that switch
/// something on or off rather than set a value.
#[cfg(feature = "detect")]
fn parse_flag(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
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
    /// Colour depth of the text fallback: what [`Color::Rgb`](crate::Color::Rgb) dots
    /// are quantised to when the frame is braille glyphs. Ignored by image protocols.
    pub depth: Depth,
    /// Wrap every image in tmux's passthrough sequence (`DCS tmux ; … ST`, with the
    /// escapes inside doubled), so it reaches the terminal tmux runs in. Set by
    /// [`detect`](Self::detect) inside tmux; needs `allow-passthrough on` there.
    /// Text, cursor movement and printed characters are never wrapped, since tmux
    /// has to see those.
    ///
    /// tmux hands the wrapped bytes on at wherever its own terminal's cursor is, so
    /// before each image the renderer erases the image's origin cell (`ECH`), which
    /// is the one thing that makes tmux put that cursor where the pane's is, and it
    /// clips the image to [`cols`](Self::cols) × [`rows`](Self::rows), the pane: an
    /// image hanging off the outer screen is drawn by a terminal that never saw the
    /// pane, and has crashed Ghostty.
    pub passthrough: bool,
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
        Self {
            protocol,
            cell,
            cols: 0,
            rows: 0,
            palette: Palette::default(),
            palette_queried: false,
            depth: Depth::TrueColor,
            passthrough: false,
        }
    }

    /// Wraps images for tmux (builder style); see [`passthrough`](Self::passthrough).
    pub fn with_passthrough(mut self, passthrough: bool) -> Self {
        self.passthrough = passthrough;
        self
    }

    /// Sets the text colour depth (builder style).
    pub fn with_depth(mut self, depth: Depth) -> Self {
        self.depth = depth;
        self
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
    ///
    /// Inside tmux (a real pane, not a terminal started from one, which inherits
    /// `TMUX`; tmux itself is asked which it is) there is no round trip to
    /// the tty: the outer terminal is read from the
    /// environment, the cell size from `TIOCGWINSZ`, and images are marked for
    /// [passthrough](Self::passthrough): under tmux the queries would be answered by tmux
    /// itself, and tmux needs `allow-passthrough on` for the images to reach its terminal.
    ///
    /// The text colour depth comes from `COBRA_COLORS` (`mono|16|256|true`) or
    /// [`Depth::from_env`]; a terminal with a graphics protocol is assumed to have
    /// true colour.
    #[cfg(feature = "detect")]
    pub fn detect() -> Self {
        let override_protocol = std::env::var("COBRA_PROTOCOL").ok().and_then(|s| Protocol::parse(&s));
        let override_cell = std::env::var("COBRA_CELL").ok().and_then(|s| CellSize::parse(&s));
        let mut t = Self::detect_inner(override_protocol, override_cell);
        t.depth = match std::env::var("COBRA_COLORS").ok().and_then(|s| Depth::parse(&s)) {
            Some(d) => d,
            None if t.is_graphical() => Depth::TrueColor,
            None => Depth::from_env(),
        };
        // The last word on passthrough. Detection asks tmux and believes nothing else,
        // which is right for the terminal started inside tmux that must not be sent a
        // `DCS`, and wrong for the pane whose tmux is not on `PATH`.
        if let Some(on) = std::env::var("COBRA_PASSTHROUGH").ok().and_then(|s| parse_flag(&s)) {
            t.passthrough = on && t.is_graphical();
        }
        t
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
        // tmux answers queries itself (and would keep a kitty reply for half a
        // second as an unknown key), so inside it nothing is asked: the environment
        // and the ioctl are all there is, and images go through passthrough.
        let tmux = in_tmux();
        if (protocol.is_none() || !t.cell.is_known() || want_palette) && !tmux {
            let probe = query::probe(protocol.is_none(), !t.cell.is_known(), want_palette);
            if !t.cell.is_known()
                && let Some(c) = probe.cell
            {
                t.cell = c;
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
        t.passthrough = tmux && t.is_graphical();
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
        assert_eq!(Terminal::text().depth, Depth::TrueColor);
        assert_eq!(Terminal::text().with_depth(Depth::Ansi16).depth, Depth::Ansi16);
        assert!(!Terminal::text().passthrough && Terminal::text().with_passthrough(true).passthrough);
    }

    #[test]
    fn tmux_is_not_taken_on_its_word() {
        assert!(!in_tmux_given(None, |_| panic!("asked tmux")));
        // Only tmux's own answer, about the socket `TMUX` names, makes a pane.
        let mut asked = None;
        assert!(in_tmux_given(Some("/tmp/s,1,0"), |t| {
            asked = Some(t.to_owned());
            true
        }));
        assert_eq!(asked.as_deref(), Some("/tmp/s,1,0"));
        // A pane that draws on another tty is a terminal started from inside tmux,
        // and a tmux that cannot answer at all says nothing: neither is a pane, so
        // neither is sent a `DCS` it may not understand.
        assert!(!in_tmux_given(Some("/tmp/s,1,0"), |_| false));
    }

    #[test]
    #[cfg(feature = "detect")]
    fn flags_from_env() {
        assert_eq!(parse_flag(" ON "), Some(true));
        assert_eq!(parse_flag("1"), Some(true));
        assert_eq!(parse_flag("false"), Some(false));
        assert_eq!(parse_flag("0"), Some(false));
        assert_eq!(parse_flag("maybe"), None);
    }
}
