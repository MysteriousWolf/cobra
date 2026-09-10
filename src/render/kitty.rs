//! Kitty graphics protocol: zlib-compressed RGBA, chunked, direct or virtual placement.

use std::fmt::Write as _;

use crate::encode::base64;

/// Base64 characters per chunk (the protocol's maximum is 4096).
const CHUNK: usize = 4096;

/// The chunk size to use, `COBRA_CHUNK` if it names a usable one.
///
/// Terminals have been known to die on a picture that arrives in many chunks rather
/// than on anything wrong with the picture, and telling the two apart means sending
/// the same bytes split differently. This override does that without a rebuild; the
/// protocol's own maximum is the default and the ceiling.
fn chunk() -> usize {
    static CHOSEN: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *CHOSEN.get_or_init(|| {
        std::env::var("COBRA_CHUNK")
            .ok()
            .and_then(|s| s.trim().parse::<usize>().ok())
            .filter(|n| *n > 0)
            .map_or(CHUNK, |n| n.min(CHUNK))
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn frame(
    zlib: &[u8],
    width: u32,
    height: u32,
    id: u32,
    cells: (u16, u16),
    virt: bool,
    z: i32,
    scratch: &mut Vec<u8>,
    out: &mut Vec<u8>,
) {
    scratch.clear();
    base64::encode(zlib, scratch);
    // a=T transmit+display, f=32 RGBA, o=z zlib, q=2 no replies, C=1 keep cursor.
    // Reusing `i` replaces the previous image and its placements: one id per canvas
    // gives flicker-free updates.
    let (cols, rows) = cells;
    // `c`,`r` give the placement its size in cells, so the picture covers exactly the
    // room the renderer made for it. Without them the terminal derives that from the
    // image's pixels and its own cell, and a cell size we read too large would put
    // part of the image past the last row: partly visible images have crashed Ghostty.
    let mut control = format!("a=T,f=32,o=z,s={width},v={height},i={id},q=2,C=1,c={cols},r={rows}");
    if virt {
        control.push_str(",U=1");
    }
    if z != 0 {
        let _ = write!(control, ",z={z}");
    }
    let chunks = scratch.chunks(chunk());
    let n = chunks.len().max(1);
    for (k, chunk) in chunks.enumerate() {
        // A continuation chunk carries `m` alone: a leading comma would make the
        // control data start with an empty key, which a terminal is free to reject.
        out.extend_from_slice(b"\x1b_G");
        if k == 0 {
            out.extend_from_slice(control.as_bytes());
            out.push(b',');
        }
        out.extend_from_slice(if k + 1 < n { b"m=1;" } else { b"m=0;" });
        out.extend_from_slice(chunk);
        out.extend_from_slice(b"\x1b\\");
    }
}
