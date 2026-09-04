//! Raw tty access on unix: `isatty`, `TIOCGWINSZ`, and a single escape-sequence probe.

use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;

use super::CellSize;

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
}

/// Sends the requested probes followed by DA1 and parses whatever comes back.
///
/// `DA1` is answered by every terminal, so its reply marks the end of the response
/// and avoids waiting for the full timeout on terminals that ignore the other
/// queries. Total wait is bounded by `TIMEOUT_MS`.
pub(super) fn probe(want_protocol: bool, want_cell: bool) -> Probe {
    const TIMEOUT_MS: i32 = 300;
    let mut seq = Vec::with_capacity(64);
    if want_protocol {
        // Kitty graphics query: a 1×1 RGB image, query action, id 31. Kitty-protocol
        // terminals reply `ESC _ G i=31;OK ESC \`; others ignore or (rarely) echo it.
        seq.extend_from_slice(b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\");
    }
    if want_cell {
        seq.extend_from_slice(b"\x1b[16t"); // reply: ESC [ 6 ; height ; width t
    }
    seq.extend_from_slice(b"\x1b[c"); // DA1, reply: ESC [ ? ... c

    let reply = roundtrip(&seq, TIMEOUT_MS).unwrap_or_default();
    parse(&reply)
}

fn parse(reply: &[u8]) -> Probe {
    let mut p = Probe { kitty: find(reply, b"_Gi=31;OK").is_some(), ..Default::default() };
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
    fn device_class_is_not_sixel() {
        assert!(!parse(b"\x1b[?4;6c").sixel);
        assert!(parse(b"\x1b[?1;4c").sixel);
        assert!(!parse(b"\x1b[?1;2c").sixel);
    }
}
