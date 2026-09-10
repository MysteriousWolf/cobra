//! Minimal 8-bit RGBA PNG writer (filter type 0 on every scanline).

use super::deflate;

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// Appends a PNG for the `width × height` RGBA buffer `rgba` to `out`. `dists` are
/// the LZ77 distances in *pixels* worth trying (see [`deflate::zlib`]); rows are one
/// filter byte longer than the raster, which is accounted for here.
/// `scratch` is reused for the filtered scanlines and the zlib stream.
pub(crate) fn encode(
    window: &mut deflate::Window,
    rgba: &[u8],
    width: u32,
    height: u32,
    dists: &[usize],
    scratch: &mut Vec<u8>,
    out: &mut Vec<u8>,
) {
    let stride = width as usize * 4;
    scratch.clear();
    scratch.reserve(height as usize * (stride + 1));
    for row in rgba.chunks_exact(stride) {
        scratch.push(0); // filter: none
        scratch.extend_from_slice(row);
    }
    let filtered = std::mem::take(scratch);
    let mut byte_dists = [0usize; 8];
    for (slot, &d) in byte_dists.iter_mut().zip(dists) {
        // A distance of `d` pixels: within a row it is `4·d` bytes, and every row
        // boundary crossed adds a filter byte.
        *slot = d * 4 + d / width as usize;
    }
    deflate::zlib(window, &filtered, &byte_dists[..dists.len().min(8)], scratch);
    let idat = std::mem::replace(scratch, filtered);

    out.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    let mut ihdr = [0u8; 13];
    ihdr[..4].copy_from_slice(&width.to_be_bytes());
    ihdr[4..8].copy_from_slice(&height.to_be_bytes());
    ihdr[8..].copy_from_slice(&[8, 6, 0, 0, 0]); // 8-bit, RGBA, deflate, no filter, no interlace
    chunk(out, b"IHDR", &ihdr);
    chunk(out, b"IDAT", &idat);
    chunk(out, b"IEND", &[]);
    *scratch = idat;
}

#[cfg(test)]
mod tests {
    #[test]
    fn crc_known_value() {
        assert_eq!(super::crc32(b"IEND"), 0xAE42_6082);
    }

    #[test]
    fn structure() {
        let mut out = Vec::new();
        let mut scratch = Vec::new();
        super::encode(
            &mut Default::default(),
            &[255, 0, 0, 255, 0, 255, 0, 255],
            2,
            1,
            &[1, 2],
            &mut scratch,
            &mut out,
        );
        assert_eq!(&out[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&out[12..16], b"IHDR");
        assert_eq!(&out[out.len() - 8..out.len() - 4], b"IEND");
    }
}
