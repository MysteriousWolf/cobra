//! A tiny zlib (RFC 1950/1951) compressor tuned for flat raster images.
//!
//! It emits one fixed-Huffman block and only ever looks for matches at two distances:
//! one pixel back (horizontal runs) and one scanline back (vertical repetition). That
//! is all a braille canvas needs: frames are mostly transparent with flat-coloured
//! dots, so this shrinks them by two orders of magnitude at a fraction of the cost of
//! a general LZ77 search, and it makes the kitty (`o=z`) and PNG paths cheap enough
//! for animation without pulling in a compression crate.

const LEN_BASE: [u16; 29] =
    [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEN_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145,
    8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] =
    [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];
const MAX_MATCH: usize = 258;
const MAX_DIST: usize = 32768;

struct Bits<'a> {
    out: &'a mut Vec<u8>,
    acc: u64,
    n: u32,
}

impl Bits<'_> {
    /// LSB-first bit field (extra bits, headers).
    #[inline]
    fn put(&mut self, v: u32, bits: u32) {
        self.acc |= (v as u64) << self.n;
        self.n += bits;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }

    /// Huffman code: written MSB-first, so reverse it.
    #[inline]
    fn code(&mut self, code: u32, bits: u32) {
        self.put(code.reverse_bits() >> (32 - bits), bits);
    }

    fn literal(&mut self, sym: u32) {
        match sym {
            0..=143 => self.code(0x30 + sym, 8),
            144..=255 => self.code(0x190 + sym - 144, 9),
            256..=279 => self.code(sym - 256, 7),
            _ => self.code(0xC0 + sym - 280, 8),
        }
    }

    fn pair(&mut self, len: usize, dist: usize) {
        let li = LEN_BASE.iter().rposition(|&b| b as usize <= len).unwrap();
        self.literal(257 + li as u32);
        self.put((len - LEN_BASE[li] as usize) as u32, LEN_EXTRA[li] as u32);
        let di = DIST_BASE.iter().rposition(|&b| b as usize <= dist).unwrap();
        self.code(di as u32, 5);
        self.put((dist - DIST_BASE[di] as usize) as u32, DIST_EXTRA[di] as u32);
    }

    fn finish(&mut self) {
        if self.n > 0 {
            self.out.push(self.acc as u8);
        }
        self.acc = 0;
        self.n = 0;
    }
}

/// Length of the match between `data[pos..]` and `data[pos - dist..]`, eight bytes at
/// a time: flat rasters match in long runs and this is where the encoder spends
/// its time.
#[inline]
fn match_len(data: &[u8], pos: usize, dist: usize) -> usize {
    let max = MAX_MATCH.min(data.len() - pos);
    let (a, b) = (&data[pos - dist..pos - dist + max], &data[pos..pos + max]);
    let mut n = 0;
    let mut wa = a.chunks_exact(8);
    let mut wb = b.chunks_exact(8);
    for (x, y) in (&mut wa).zip(&mut wb) {
        let (x, y) = (u64::from_ne_bytes(x.try_into().unwrap()), u64::from_ne_bytes(y.try_into().unwrap()));
        if x != y {
            return n + ((x ^ y).to_le().trailing_zeros() / 8) as usize;
        }
        n += 8;
    }
    n + wa.remainder().iter().zip(wb.remainder()).take_while(|(x, y)| x == y).count()
}

/// Appends a zlib stream for `data` to `out`, trying only the match distances in
/// `dists` (in bytes). For a raster that is one pixel back, one cell back, one row
/// back and one dot row back: flat runs, dither patterns and vertical repetition.
pub(crate) fn zlib(data: &[u8], dists: &[usize], out: &mut Vec<u8>) {
    out.extend_from_slice(&[0x78, 0x01]);
    let mut w = Bits { out, acc: 0, n: 0 };
    w.put(1, 1); // BFINAL
    w.put(1, 2); // BTYPE = fixed Huffman
    let mut i = 0;
    let mut last = dists.first().copied().unwrap_or(0);
    while i < data.len() {
        // The distance that matched last time usually matches again (a run continues,
        // a pattern repeats); when it gives a maximal match there is nothing to beat.
        let (mut best, mut best_dist) = (0, 0);
        if last > 0 && last <= i {
            best = match_len(data, i, last);
            best_dist = last;
        }
        if best < MAX_MATCH {
            for &d in dists {
                if d != last && d > 0 && d <= i && d <= MAX_DIST {
                    let l = match_len(data, i, d);
                    if l > best {
                        best = l;
                        best_dist = d;
                    }
                }
            }
        }
        if best >= 3 {
            last = best_dist;
        }
        if best >= 3 {
            w.pair(best, best_dist);
            i += best;
        } else {
            w.literal(data[i] as u32);
            i += 1;
        }
    }
    w.literal(256);
    w.finish();
    let out = w.out;
    out.extend_from_slice(&adler32(data).to_be_bytes());
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    b << 16 | a
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal inflate for fixed-Huffman blocks, enough to verify round trips in tests.
    fn inflate_fixed(z: &[u8]) -> Vec<u8> {
        struct R<'a> {
            d: &'a [u8],
            pos: usize,
            bit: u32,
        }
        impl R<'_> {
            fn bit(&mut self) -> u32 {
                let b = (self.d[self.pos] >> self.bit) as u32 & 1;
                self.bit += 1;
                if self.bit == 8 {
                    self.bit = 0;
                    self.pos += 1;
                }
                b
            }
            fn bits(&mut self, n: u32) -> u32 {
                (0..n).fold(0, |acc, i| acc | self.bit() << i)
            }
            fn code(&mut self, n: u32) -> u32 {
                (0..n).fold(0, |acc, _| acc << 1 | self.bit())
            }
        }
        let mut r = R { d: &z[2..], pos: 0, bit: 0 };
        assert_eq!(r.bits(1), 1);
        assert_eq!(r.bits(2), 1);
        let mut out = Vec::new();
        loop {
            let mut c = r.code(7);
            let sym = if c <= 0b0010111 {
                c + 256
            } else {
                c = c << 1 | r.bit();
                if c <= 0b10111111 {
                    c - 0x30
                } else if c <= 0b11000111 {
                    c - 0xC0 + 280
                } else {
                    (c << 1 | r.bit()) - 0x190 + 144
                }
            };
            match sym {
                256 => break,
                0..=255 => out.push(sym as u8),
                _ => {
                    let li = (sym - 257) as usize;
                    let len = LEN_BASE[li] as usize + r.bits(LEN_EXTRA[li] as u32) as usize;
                    let di = r.code(5) as usize;
                    let dist = DIST_BASE[di] as usize + r.bits(DIST_EXTRA[di] as u32) as usize;
                    for _ in 0..len {
                        out.push(out[out.len() - dist]);
                    }
                }
            }
        }
        out
    }

    #[test]
    fn round_trips() {
        let mut img = vec![0u8; 40 * 10 * 4];
        for (i, b) in img.iter_mut().enumerate() {
            if (i / 4) % 7 == 0 {
                *b = (i % 4 * 60) as u8 + 1;
            }
        }
        let cases: Vec<Vec<u8>> =
            vec![vec![], b"a".to_vec(), b"abcabcabcabcabc".to_vec(), img, (0..=255u8).cycle().take(1000).collect()];
        for data in cases {
            let mut z = Vec::new();
            zlib(&data, &[4, 160], &mut z);
            assert_eq!(inflate_fixed(&z), data);
            assert_eq!(&z[z.len() - 4..], adler32(&data).to_be_bytes());
        }
    }

    #[test]
    fn compresses_flat_images() {
        let data = vec![0u8; 100_000];
        let mut z = Vec::new();
        zlib(&data, &[4, 400], &mut z);
        assert!(z.len() < 1000, "{}", z.len());
    }
}
