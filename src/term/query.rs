//! Raw tty access on unix: `isatty`, `TIOCGWINSZ`, and a single escape-sequence probe.

use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;

use super::CellSize;
use crate::{Palette, Rgb};

pub(super) struct WinSize {
    pub cols: u16,
    pub rows: u16,
    pub cell: CellSize,
}

pub(super) fn is_tty() -> bool {
    // SAFETY: `isatty` only inspects the descriptor.
    unsafe { libc::isatty(libc::STDOUT_FILENO) == 1 }
}

pub(super) fn winsize() -> WinSize {
    // SAFETY: `winsize` is plain-old-data; the ioctl writes into it only on success.
    let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
    let ok = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws) } == 0;
    let (cols, rows) = if ok { (ws.ws_col, ws.ws_row) } else { (0, 0) };
    let cell = if ok && cols > 0 && rows > 0 {
        CellSize { width: ws.ws_xpixel / cols, height: ws.ws_ypixel / rows }
    } else {
        CellSize::default()
    };
    WinSize { cols, rows, cell }
}

#[derive(Default)]
pub(super) struct Probe {
    pub kitty: bool,
    pub sixel: bool,
    pub cell: Option<CellSize>,
    pub palette: Palette,
    /// How many palette / foreground / background replies were parsed.
    pub palette_entries: u8,
}

/// Sends the requested probes followed by DA1 and parses whatever comes back.
///
/// `DA1` is answered by every terminal, so its reply marks the end of the response
/// and avoids waiting for the full timeout on terminals that ignore the other
/// queries. Total wait is bounded by `TIMEOUT_MS`.
pub(super) fn probe(want_protocol: bool, want_cell: bool, want_palette: bool) -> Probe {
    const TIMEOUT_MS: i32 = 300;
    let mut seq = Vec::with_capacity(256);
    if want_protocol {
        // Kitty graphics query: a 1×1 RGB image, query action, id 31. Kitty-protocol
        // terminals reply `ESC _ G i=31;OK ESC \`; others ignore or (rarely) echo it.
        seq.extend_from_slice(b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\");
    }
    if want_cell {
        seq.extend_from_slice(b"\x1b[16t"); // reply: ESC [ 6 ; height ; width t
    }
    if want_palette {
        // ANSI colours 0..=15 (the themed ones), then default fg and bg.
        // Replies: ESC ] 4 ; n ; rgb:RRRR/GGGG/BBBB ST, ESC ] 10 ; rgb:... ST, ESC ] 11 ; rgb:... ST.
        for i in 0..16 {
            seq.extend_from_slice(format!("\x1b]4;{i};?\x1b\\").as_bytes());
        }
        seq.extend_from_slice(b"\x1b]10;?\x1b\\\x1b]11;?\x1b\\");
    }
    seq.extend_from_slice(b"\x1b[c"); // DA1, reply: ESC [ ? ... c

    let reply = roundtrip(&seq, TIMEOUT_MS).unwrap_or_default();
    parse(&reply)
}

/// Parses `rgb:RRRR/GGGG/BBBB` (any 1–4 hex digits per channel) or `#RRGGBB`.
fn parse_rgb(spec: &[u8]) -> Option<Rgb> {
    let spec = std::str::from_utf8(spec).ok()?.trim();
    let channel = |h: &str| -> Option<u8> {
        let v = u32::from_str_radix(h, 16).ok()?;
        // Scale to 8 bits by the number of digits given.
        let max = (1u32 << (4 * h.len().clamp(1, 4))) - 1;
        Some(((v * 255 + max / 2) / max) as u8)
    };
    if let Some(rest) = spec.strip_prefix("rgb:") {
        let mut it = rest.split('/');
        let (r, g, b) = (it.next()?, it.next()?, it.next()?);
        return Some(Rgb::new(channel(r)?, channel(g)?, channel(b)?));
    }
    if let Some(hex) = spec.strip_prefix('#').filter(|h| h.len() == 6) {
        return Some(Rgb::hex(u32::from_str_radix(hex, 16).ok()?));
    }
    None
}

/// Iterates over `ESC ] <body> (BEL | ESC \)` replies in `reply`.
fn osc_replies(reply: &[u8]) -> impl Iterator<Item = &[u8]> {
    let mut rest = reply;
    std::iter::from_fn(move || {
        let start = find(rest, b"\x1b]")? + 2;
        let body = &rest[start..];
        let end = body.iter().position(|&b| b == 0x07 || b == 0x1b).unwrap_or(body.len());
        rest = &body[end..];
        Some(&body[..end])
    })
}

fn parse(reply: &[u8]) -> Probe {
    let mut p = Probe { kitty: find(reply, b"_Gi=31;OK").is_some(), ..Default::default() };
    for body in osc_replies(reply) {
        let Some(semi) = body.iter().position(|&b| b == b';') else { continue };
        let (kind, rest) = (&body[..semi], &body[semi + 1..]);
        match kind {
            b"4" => {
                let Some(semi) = rest.iter().position(|&b| b == b';') else { continue };
                let index = std::str::from_utf8(&rest[..semi]).ok().and_then(|s| s.parse::<usize>().ok());
                if let (Some(i), Some(c)) = (index.filter(|&i| i < 256), parse_rgb(&rest[semi + 1..])) {
                    p.palette.colors[i] = c;
                    p.palette_entries += 1;
                }
            }
            b"10" => {
                if let Some(c) = parse_rgb(rest) {
                    p.palette.foreground = c;
                    p.palette_entries += 1;
                }
            }
            b"11" => {
                if let Some(c) = parse_rgb(rest) {
                    p.palette.background = c;
                    p.palette_entries += 1;
                }
            }
            _ => {}
        }
    }
    if let Some(i) = find(reply, b"\x1b[6;") {
        let body = &reply[i + 4..];
        if let Some(end) = body.iter().position(|&b| b == b't') {
            let mut it = std::str::from_utf8(&body[..end]).unwrap_or("").split(';');
            let h = it.next().and_then(|s| s.parse().ok());
            let w = it.next().and_then(|s| s.parse().ok());
            if let (Some(height), Some(width)) = (h, w) {
                let c = CellSize { width, height };
                if c.is_known() {
                    p.cell = Some(c);
                }
            }
        }
    }
    if let Some(i) = find(reply, b"\x1b[?") {
        let body = &reply[i + 3..];
        if let Some(end) = body.iter().position(|&b| b == b'c') {
            // First parameter is the device class; `4` among the rest means sixel.
            p.sixel = std::str::from_utf8(&body[..end]).unwrap_or("").split(';').skip(1).any(|s| s == "4");
        }
    }
    p
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Writes `seq` to `/dev/tty` in raw mode and reads until a DA1 reply or timeout.
fn roundtrip(seq: &[u8], timeout_ms: i32) -> Option<Vec<u8>> {
    let mut tty = OpenOptions::new().read(true).write(true).open("/dev/tty").ok()?;
    let fd = tty.as_raw_fd();

    // SAFETY: termios is plain-old-data and `fd` is a valid open descriptor for the
    // lifetime of `tty`; the original attributes are restored on every exit path.
    let saved = unsafe {
        let mut t: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(fd, &mut t) != 0 {
            return None;
        }
        let mut raw = t;
        libc::cfmakeraw(&mut raw);
        if libc::tcsetattr(fd, libc::TCSANOW, &raw) != 0 {
            return None;
        }
        t
    };
    struct Restore(i32, libc::termios);
    impl Drop for Restore {
        fn drop(&mut self) {
            // SAFETY: restoring attributes we previously read from the same descriptor.
            unsafe { libc::tcsetattr(self.0, libc::TCSANOW, &self.1) };
        }
    }
    let _restore = Restore(fd, saved);

    tty.write_all(seq).ok()?;
    tty.flush().ok()?;

    let mut buf = Vec::with_capacity(128);
    let mut chunk = [0u8; 256];
    let mut budget = timeout_ms;
    while budget > 0 {
        let mut pfd = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
        let started = std::time::Instant::now();
        // SAFETY: `pfd` is a valid single-element array for the duration of the call.
        let n = unsafe { libc::poll(&mut pfd, 1, budget) };
        budget -= started.elapsed().as_millis() as i32;
        if n <= 0 {
            break;
        }
        match tty.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
        if let Some(i) = find(&buf, b"\x1b[?") {
            if buf[i + 3..].contains(&b'c') {
                break;
            }
        }
    }
    Some(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_combined_reply() {
        let p = parse(b"\x1b_Gi=31;OK\x1b\\\x1b[6;20;9t\x1b[?62;4;22c");
        assert!(p.kitty && p.sixel);
        assert_eq!(p.cell, Some(CellSize { width: 9, height: 20 }));
    }

    #[test]
    fn parses_palette_replies() {
        let reply = b"\x1b]4;1;rgb:cccc/3333/4444\x1b\\\x1b]4;9;rgb:ff/00/00\x07\x1b]10;rgb:dddd/dddd/dddd\x1b\\\x1b]11;#1a1b26\x1b\\\x1b[?62;4c";
        let p = parse(reply);
        assert_eq!(p.palette_entries, 4);
        assert_eq!(p.palette.colors[1], Rgb::hex(0xcc3344));
        assert_eq!(p.palette.colors[9], Rgb::hex(0xff0000));
        assert_eq!(p.palette.foreground, Rgb::hex(0xdddddd));
        assert_eq!(p.palette.background, Rgb::hex(0x1a1b26));
        assert!(p.sixel && !p.kitty);
        assert_eq!(parse(b"\x1b[?1;2c").palette_entries, 0);
    }

    #[test]
    fn device_class_is_not_sixel() {
        assert!(!parse(b"\x1b[?4;6c").sixel);
        assert!(parse(b"\x1b[?1;4c").sixel);
        assert!(!parse(b"\x1b[?1;2c").sixel);
    }
}
