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
//! 4. One round trip on `/dev/tty`: `XTVERSION` for the terminal's own name, a kitty
//!    graphics probe, `CSI 16 t` for the cell size when still unknown, `OSC 4` /
//!    `OSC 10` / `OSC 11` for the palette, and `DA1` for sixel (which also terminates
//!    the response).
//! 5. Only what the terminal itself answered decides the protocol. The environment
//!    ([`Protocol::from_env`]) is consulted for a terminal that answers nothing at
//!    all, and for iTerm2, whose inline images no query detects.
//! 6. Anything without a usable cell size falls back to [`Protocol::Text`].
//!
//! Environment variables are a guess of last resort because every terminal hands
//! them to everything it starts, terminals included: `GHOSTTY_RESOURCES_DIR` in an
//! Alacritty window started from Ghostty is Ghostty's, and taking it for the
//! terminal in front of the user sent Alacritty kitty images it cannot draw, with no
//! way back to braille. A terminal that answers `DA1` but no graphics query speaks
//! no protocol, whatever the environment says.
//!
//! # Multiplexers
//!
//! Inside tmux the terminal that draws is not the one the program talks to: tmux
//! answers queries itself, and every image has to be wrapped in tmux's passthrough
//! sequence ([`Terminal::passthrough`]) to reach the outer terminal, which needs
//! `allow-passthrough on` (`set -g allow-passthrough on` in `tmux.conf`).
//!
//! So detection asks the outer terminal through that same wrapper: the kitty query,
//! `XTVERSION` and `DA1` go out wrapped, and what comes back is proof of everything
//! at once -- that this really is tmux, that passthrough is allowed, and what the
//! terminal on the other side can draw. Silence means an image would be dropped just
//! as the query was, so the frames go out as braille, which tmux draws itself.
//!
//! Whether tmux is there at all is settled before any of that, by asking the tty
//! what it is (`XTVERSION`): tmux answers `tmux 3.4`, a terminal answers with its
//! own name. Nothing in the environment is believed, because a terminal started from
//! a pane inherits `TMUX`, `TMUX_PANE` and often a `tmux-256color` `TERM` from a
//! shell's start-up files, and would take the wrapped image as an unknown `DCS`
//! (Ghostty crashed on one). A terminal too old to answer `XTVERSION` is asked about
//! over tmux's own socket instead: only tmux's word that the pane it names draws on
//! this process's tty makes a pane, and a socket that cannot be reached, a tty that
//! differs and a tmux that cannot be run are all "no".
//!
//! Being wrong the other way only costs the images, so the doubtful cases go the
//! cheap way; `COBRA_PASSTHROUGH=1` wraps them anyway (though never at a terminal
//! that gave its own name) and `COBRA_PASSTHROUGH=0` never does. GNU screen passes
//! nothing through and gets text.

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
    /// screen, which passes no graphics through.
    ///
    /// A guess is all it is, and [`Terminal::detect`] uses it only where the terminal
    /// itself says nothing: every one of these variables is inherited by whatever the
    /// terminal starts, another terminal included, so `KITTY_WINDOW_ID` may well be
    /// set in an Alacritty window. What the tty answers outranks it.
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

/// Whether the process runs in a tmux pane, decided over tmux's own socket and once
/// per process. Used only for a terminal that does not answer `XTVERSION`, whose
/// name settles the question outright.
///
/// `TMUX` alone does not say: a terminal started from inside tmux hands the variable
/// on to everything it runs, and an image wrapped for tmux that reaches such a
/// terminal directly is an unknown `DCS` to it (Ghostty crashed on one). Nor does
/// `TERM`: tmux sets a pane's to `tmux-*` or `screen-*` and a terminal sets its own,
/// but a shell's start-up files may set it again, and one that exports
/// `tmux-256color` whenever `TMUX` is set makes the new terminal look like a pane.
/// So tmux itself is asked, over the socket named in `TMUX`, whether the pane it
/// thinks this is draws on this process's tty, and only that answer makes a pane: a
/// socket that cannot be reached, a tty that differs, a tmux that cannot be run at
/// all are all "no". The two mistakes do not cost the same -- guessing pane wrongly
/// writes a `DCS` at a terminal that never asked for one, guessing terminal wrongly
/// only drops the images inside tmux, which `COBRA_PASSTHROUGH=1` brings back -- so
/// the doubtful cases go the cheap way.
/// A cell edge the window can hold: `size` unless the window's `pixels` divided over
/// its `cells` is smaller. Zero for either means the window did not say, and `size`
/// stands.
#[cfg(all(feature = "detect", unix))]
fn fit(size: u16, pixels: u16, cells: u16) -> u16 {
    if pixels == 0 || cells == 0 { size } else { size.min((pixels / cells).max(1)) }
}

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

/// What answered `XTVERSION` (`CSI > q`) on this tty, as far as drawing cares.
///
/// Only detection asks, so without it nothing reads this.
///
/// The one question is whether images may be wrapped for tmux, and a name answers
/// it where the environment cannot: `TMUX` is inherited by every process a pane
/// starts, a terminal emulator included, but the emulator answers with its own name
/// and never unwraps a `DCS` meant for tmux.
#[cfg_attr(not(feature = "detect"), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Speaker {
    /// tmux, the one thing images may be wrapped for.
    Tmux,
    /// A terminal, with the protocol its name alone settles -- only iTerm2's, since
    /// no query detects it and every other protocol answers for itself.
    Terminal(Option<Protocol>),
}

#[cfg_attr(not(feature = "detect"), allow(dead_code))]
impl Speaker {
    /// Reads the name out of an `XTVERSION` payload, already lowercased:
    /// `tmux 3.4`, `ghostty 1.3.1`, `kitty(0.32.2)`, `xterm(390)`.
    fn of(name: &str) -> Self {
        let word = name.trim().split(|c: char| !c.is_ascii_alphanumeric()).next().unwrap_or("");
        match word {
            "tmux" => Self::Tmux,
            // Its inline images have no query to ask; the rest are read off the wire,
            // so a terminal that gains a protocol is not held to this list.
            "iterm2" => Self::Terminal(Some(Protocol::Iterm2)),
            _ => Self::Terminal(None),
        }
    }
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
    /// Terminal width in cells, `0` when unknown. Images are clipped to it.
    pub cols: u16,
    /// Terminal height in cells, `0` when unknown. Images are clipped to it.
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
    /// [`detect`](Self::detect) when the tty answers that it is tmux and a wrapped
    /// query comes back from the terminal behind it, which is what `allow-passthrough
    /// on` there buys.
    /// Text, cursor movement and printed characters are never wrapped, since tmux
    /// has to see those.
    ///
    /// tmux hands the wrapped bytes on at wherever its own terminal's cursor is, so
    /// before each image the renderer erases the image's origin cell (`ECH`), which
    /// is the one thing that makes tmux put that cursor where the pane's is. The
    /// image itself is kept inside [`cols`](Self::cols) × [`rows`](Self::rows) here
    /// as everywhere else, since a picture drawn partly has crashed Ghostty.
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
    /// `TIOCGWINSZ`, then one escape-sequence round trip that asks the terminal what
    /// it is (`XTVERSION`) and what it can draw (kitty probe, `DA1`), plus whatever
    /// else is still unknown (`CSI 16 t` for the cell, `OSC 4`, `OSC 10`, `OSC 11`
    /// for the colour scheme). What the terminal answers decides; the environment
    /// ([`Protocol::from_env`]) only fills in for a terminal that answers nothing.
    /// Image protocols without a known cell size fall back to [`Protocol::Text`].
    ///
    /// Costs one `ioctl` plus one escape-sequence round trip, bounded by a short
    /// timeout and normally ending as soon as the terminal answers `DA1` (a few
    /// milliseconds). Call it once at start-up and keep the result. Set
    /// `COBRA_PALETTE=0` to skip the colour queries.
    ///
    /// A tty that answers `tmux` costs a second round trip, wrapped in tmux's
    /// passthrough so that the terminal tmux draws on answers it: what comes back
    /// says whether images can reach that terminal at all and what it can draw, and
    /// silence means braille. A terminal that inherited `TMUX` from a pane without
    /// being one answers with its own name and is never sent a wrapped byte.
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
        t
    }

    /// What the terminal calls itself, lowercased: `tmux 3.4`, `ghostty 1.3.1`,
    /// `kitty(0.32.2)`. `None` when it does not answer, as terminals older than
    /// `XTVERSION` (`CSI > q`) do not.
    ///
    /// [`detect`](Self::detect) asks this first, since it is the one answer that says
    /// whether tmux or a terminal is on the tty, and so whether an image may be
    /// wrapped for tmux. This asks it again, for a diagnostic to print; it costs one
    /// round trip and is bounded by the same short timeout.
    #[cfg(feature = "detect")]
    pub fn name() -> Option<String> {
        #[cfg(unix)]
        {
            query::probe(query::Ask { name: true, ..Default::default() }).name
        }
        #[cfg(not(unix))]
        {
            None
        }
    }

    #[cfg(all(feature = "detect", unix))]
    fn detect_inner(protocol: Option<Protocol>, cell: Option<CellSize>) -> Self {
        if protocol == Some(Protocol::Text) || (protocol.is_none() && !query::is_tty()) {
            return Self::text();
        }
        let ws = query::winsize();
        let mut t = Self { cell: cell.unwrap_or(ws.cell), cols: ws.cols, rows: ws.rows, ..Self::text() };
        let guess = protocol.or_else(Protocol::from_env);
        let want_protocol = protocol.is_none();

        // First round trip: whatever is still unknown, plus the question of who is
        // listening. The kitty query is held back while tmux may be the one reading
        // it, since tmux keeps its reply as an unknown key for half a second.
        let suspect_tmux = std::env::var("TMUX").is_ok_and(|v| !v.is_empty());
        let mut p = query::probe(query::Ask {
            kitty: want_protocol && !suspect_tmux,
            cell: !t.cell.is_known(),
            palette: std::env::var("COBRA_PALETTE").map_or(true, |v| v != "0"),
            name: true,
            wrap: false,
        });
        if !t.cell.is_known()
            && let Some(c) = p.cell
        {
            // A cell the terminal reports in physical pixels, for a window it sizes in
            // logical ones, is larger than the window can hold. Believe the window: an
            // image raised to the larger cell would reach past the last row, and a
            // picture drawn partly has crashed Ghostty.
            t.cell =
                CellSize { width: fit(c.width, ws.pixels.0, ws.cols), height: fit(c.height, ws.pixels.1, ws.rows) };
        }
        if p.palette_entries > 0 {
            t.palette = p.palette;
            t.palette_queried = true;
        }

        // Who is on this tty, which is the only thing that may be sent a passthrough
        // `DCS`. A name settles it: `TMUX` is inherited by everything a pane starts,
        // terminals included, and one of those crashed on a `DCS` meant for tmux.
        let named = p.name.as_deref().map(Speaker::of);
        let tmux = match named {
            Some(Speaker::Tmux) => true,
            Some(Speaker::Terminal(_)) => false,
            None => suspect_tmux && in_tmux(),
        };

        let mut seen = None;
        if tmux {
            // Second round trip, wrapped: tmux hands the queries to the terminal it
            // draws on, whose answers come back in the order they were asked. A reply
            // is proof of everything the images need -- that this is tmux, that
            // `allow-passthrough` is on, and what the outer terminal can draw. Nothing
            // coming back means an image would not arrive either, and only the
            // environment is left to say what the outer terminal is.
            let outer = query::probe(query::Ask { kitty: want_protocol, name: true, wrap: true, ..Default::default() });
            t.passthrough = true;
            // Silence means an image would be dropped just as the query was, so the
            // frames go out as braille, which tmux draws itself and always shows.
            seen = Some(if outer.answered {
                Self::seen(&outer, outer.name.as_deref().map(Speaker::of), guess)
            } else {
                Protocol::Text
            });
        } else {
            if want_protocol && suspect_tmux {
                // The kitty query was held back for a tmux that turned out not to be
                // one. Nothing unwraps a `DCS` here, so it is safe to ask now.
                p = query::probe(query::Ask { kitty: true, ..Default::default() });
            }
            if p.answered {
                // What the terminal answers outranks the environment, which is only a
                // guess and is inherited by every terminal started from this one: a
                // stale `GHOSTTY_RESOURCES_DIR` had Alacritty sent kitty images it
                // cannot draw, with no way back to braille.
                seen = Some(Self::seen(&p, named, guess));
            }
        }

        if t.cell.is_known() {
            t.protocol = protocol.or(seen).or(guess).unwrap_or(Protocol::Text);
        }
        t.passthrough &= t.is_graphical();
        // The last word, for the pane whose tmux could not be reached. It cannot put a
        // `DCS` at a terminal that named itself, which is the mistake that crashes one.
        if let Some(on) = std::env::var("COBRA_PASSTHROUGH").ok().and_then(|s| parse_flag(&s)) {
            t.passthrough = on && t.is_graphical() && !matches!(named, Some(Speaker::Terminal(_)));
        }
        t
    }

    /// The protocol one probe's answers show: the name the terminal gave, then what
    /// it replied to the graphics queries, then `guess` where nothing on the wire can
    /// tell. [`Protocol::Text`] when none of the three says anything.
    #[cfg(all(feature = "detect", unix))]
    fn seen(p: &query::Probe, named: Option<Speaker>, guess: Option<Protocol>) -> Protocol {
        if let Some(Speaker::Terminal(Some(known))) = named {
            return known;
        }
        if p.kitty {
            Protocol::Kitty
        } else if p.sixel {
            Protocol::Sixel
        } else if guess == Some(Protocol::Iterm2) {
            // iTerm2's inline images answer no query and its versions before 3.4 give
            // no name either, so `ITERM_SESSION_ID` / `LC_TERMINAL` is all there is.
            Protocol::Iterm2
        } else {
            Protocol::Text
        }
    }

    #[cfg(all(feature = "detect", not(unix)))]
    fn detect_inner(protocol: Option<Protocol>, cell: Option<CellSize>) -> Self {
        // No tty queries on this platform: honour explicit overrides, otherwise text.
        let mut t = match (protocol.or_else(Protocol::from_env), cell) {
            (Some(p), Some(c)) => Self::new(p, c),
            _ => Self::text(),
        };
        // Nothing here can tell a pane from a terminal started in one, so wrapping is
        // asked for by hand or not at all.
        if let Some(on) = std::env::var("COBRA_PASSTHROUGH").ok().and_then(|s| parse_flag(&s)) {
            t.passthrough = on && t.is_graphical();
        }
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(all(feature = "detect", unix))]
    #[test]
    fn a_cell_larger_than_the_window_is_cut_down_to_it() {
        assert_eq!(fit(20, 800, 80), 10, "the window holds 10 px per cell, not 20");
        assert_eq!(fit(8, 800, 80), 8, "a cell that fits is left alone");
        assert_eq!(fit(20, 0, 80), 20, "a window without pixels says nothing");
        assert_eq!(fit(20, 800, 0), 20);
        assert_eq!(fit(20, 40, 80), 1, "never zero, which would mean no protocol at all");
    }

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
    fn a_terminal_that_gives_its_name_is_never_taken_for_tmux() {
        assert_eq!(Speaker::of("tmux 3.4"), Speaker::Tmux);
        // Every other name is a terminal, and a terminal is never sent a wrapped
        // image however much of tmux's environment it inherited.
        for name in ["ghostty 1.3.1", "kitty(0.32.2)", "alacritty 0.15.1", "xterm(390)", "wezterm 20240203"] {
            assert_eq!(Speaker::of(name), Speaker::Terminal(None), "{name}");
        }
        // The one protocol no query detects, so the name is what there is.
        assert_eq!(Speaker::of("iterm2 3.5.0"), Speaker::Terminal(Some(Protocol::Iterm2)));
    }

    #[cfg(all(feature = "detect", unix))]
    #[test]
    fn what_the_terminal_answers_outranks_the_environment() {
        use query::Probe;
        let answered = |kitty, sixel| Probe { kitty, sixel, answered: true, ..Default::default() };
        // A terminal that answers the queries but not the graphics ones draws no
        // images, whatever `KITTY_WINDOW_ID` a terminal above it left behind.
        assert_eq!(Terminal::seen(&answered(false, false), None, Some(Protocol::Kitty)), Protocol::Text);
        assert_eq!(Terminal::seen(&answered(true, false), None, None), Protocol::Kitty);
        assert_eq!(Terminal::seen(&answered(false, true), None, None), Protocol::Sixel);
        // Except iTerm2, which has no query to answer and no name before 3.4.
        assert_eq!(Terminal::seen(&answered(false, false), None, Some(Protocol::Iterm2)), Protocol::Iterm2);
        let named = Some(Speaker::Terminal(Some(Protocol::Iterm2)));
        assert_eq!(Terminal::seen(&answered(false, false), named, None), Protocol::Iterm2);
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
