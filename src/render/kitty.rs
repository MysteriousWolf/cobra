//! Kitty graphics protocol: zlib-compressed RGBA, chunked, direct or virtual placement.

use std::fmt::Write as _;

use crate::encode::base64;

/// Base64 characters per chunk (the protocol's maximum is 4096).
const CHUNK: usize = 4096;

#[allow(clippy::too_many_arguments)]
pub(super) fn frame(
    zlib: &[u8],
    width: u32,
    height: u32,
    id: u32,
    virt: Option<(u16, u16)>,
    z: i32,
    scratch: &mut Vec<u8>,
    out: &mut Vec<u8>,
) {
    scratch.clear();
    base64::encode(zlib, scratch);
    // a=T transmit+display, f=32 RGBA, o=z zlib, q=2 no replies, C=1 keep cursor.
    // Reusing `i` replaces the previous image and its placements: one id per canvas
    // gives flicker-free updates.
    let mut control = format!("a=T,f=32,o=z,s={width},v={height},i={id},q=2,C=1");
    if let Some((cols, rows)) = virt {
        let _ = write!(control, ",U=1,c={cols},r={rows}");
    }
    if z != 0 {
        let _ = write!(control, ",z={z}");
    }
    let chunks = scratch.chunks(CHUNK);
    let n = chunks.len().max(1);
    for (k, chunk) in chunks.enumerate() {
        out.extend_from_slice(b"\x1b_G");
        if k == 0 {
            out.extend_from_slice(control.as_bytes());
        }
        out.extend_from_slice(if k + 1 < n { b",m=1;" } else { b",m=0;" });
        out.extend_from_slice(chunk);
        out.extend_from_slice(b"\x1b\\");
    }
}
