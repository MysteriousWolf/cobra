//! A tiny zlib (RFC 1950/1951) compressor for raster images.
//!
//! It emits one fixed-Huffman block, and finds matches two ways. The caller's hint
//! distances -- one pixel back, one scanline back -- are tried directly, because a
//! braille canvas is mostly transparent with flat-coloured dots and those two answer
//! it almost every time for the cost of two comparisons. Everything they miss goes to
//! a hash chain over the whole window.
//!
//! The hints alone used to be the whole search, which cost an order of magnitude on
//! any frame that was not flat: a colour-per-dot gradient compressed to 27KB where a
//! general LZ77 search reaches 4KB, and every byte of that went down the wire on
//! every frame. Keeping both is what makes the flat case cheap and the detailed case
//! small, still without pulling in a compression crate.

const LEN_BASE: [u16; 29] =
    [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEN_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145,
    8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] =
    [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;
const MAX_DIST: usize = 32768;

/// Slots in the hash table, and the ring of previous positions. The ring is the
/// window: two positions that far apart share a slot, and a match that far back is
/// out of range anyway.
const HASH_BITS: u32 = 15;
const HASH_SIZE: usize = 1 << HASH_BITS;

/// How many positions of a hash chain to walk before taking the best so far. Chains
/// run long on repetitive rasters, and the tail of one rarely improves the match.
const MAX_CHAIN: usize = 128;

const NIL: u32 = u32::MAX;

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
    let ((wa, ra), (wb, rb)) = (a.as_chunks::<8>(), b.as_chunks::<8>());
    for (x, y) in wa.iter().zip(wb) {
        let (x, y) = (u64::from_ne_bytes(*x), u64::from_ne_bytes(*y));
        if x != y {
            return n + ((x ^ y).to_le().trailing_zeros() / 8) as usize;
        }
        n += 8;
    }
    n + ra.iter().zip(rb).take_while(|(x, y)| x == y).count()
}

/// Three bytes at `pos`, scattered into a hash-table slot.
#[inline]
fn hash3(data: &[u8], pos: usize) -> usize {
    let v = (data[pos] as u32) << 16 | (data[pos + 1] as u32) << 8 | data[pos + 2] as u32;
    (v.wrapping_mul(0x9E37_79B1) >> (32 - HASH_BITS)) as usize
}

/// Records `pos` as the most recent position starting with its three bytes, pushing
/// whatever held that slot onto the chain behind it, and hands back that older
/// position: it is where a search from `pos` starts, since `pos` cannot match itself.
#[inline]
fn insert(data: &[u8], pos: usize, head: &mut [u32], prev: &mut [u32]) -> u32 {
    if pos + MIN_MATCH > data.len() {
        return NIL;
    }
    let h = hash3(data, pos);
    let older = head[h];
    prev[pos & (MAX_DIST - 1)] = older;
    head[h] = pos as u32;
    older
}

/// The longest match for `data[pos..]`, as (length, distance), or a length below
/// [`MIN_MATCH`] when there is nothing worth encoding as a pair.
fn find(data: &[u8], pos: usize, chain: u32, prev: &[u32], hints: &[usize], last: usize) -> (usize, usize) {
    let (mut best, mut best_dist) = (0, 0);
    // The distance that matched last time usually matches again: a run continues, a
    // row repeats. When it gives a maximal match there is nothing left to beat.
    if last > 0 && last <= pos {
        best = match_len(data, pos, last);
        best_dist = last;
    }
    if best < MAX_MATCH {
        for &d in hints {
            if d > 0 && d != last && d <= pos && d <= MAX_DIST {
                let l = match_len(data, pos, d);
                if l > best {
                    best = l;
                    best_dist = d;
                }
            }
        }
    }
    if best < MAX_MATCH {
        let mut p = chain;
        for _ in 0..MAX_CHAIN {
            if p == NIL {
                break;
            }
            let cand = p as usize;
            // A slot can hold a position from before the window wrapped; both tests
            // end the walk rather than trusting it.
            if cand >= pos || pos - cand > MAX_DIST {
                break;
            }
            let l = match_len(data, pos, pos - cand);
            if l > best {
                best = l;
                best_dist = pos - cand;
                if l >= MAX_MATCH {
                    break;
                }
            }
            p = prev[cand & (MAX_DIST - 1)];
        }
    }
    (best, best_dist)
}

/// Appends a zlib stream for `data` to `out`. `dists` are match distances worth
/// trying first (in bytes): for a raster that is one pixel back, one cell back, one
/// row back and one dot row back -- flat runs, dither patterns, vertical repetition.
/// Anything they do not catch is found by searching the window.
pub(crate) fn zlib(data: &[u8], dists: &[usize], out: &mut Vec<u8>) {
    out.extend_from_slice(&[0x78, 0x01]);
    let mut w = Bits { out, acc: 0, n: 0 };
    w.put(1, 1); // BFINAL
    w.put(1, 2); // BTYPE = fixed Huffman

    let mut head = vec![NIL; HASH_SIZE];
    let mut prev = vec![NIL; MAX_DIST];
    let mut last = dists.first().copied().unwrap_or(0);

    // Lazy matching: a match found here is held back one byte to see whether the next
    // position starts a longer one, which is worth more than the literal it costs.
    let mut held: Option<(usize, usize)> = None;
    let mut i = 0;
    while i < data.len() {
        let chain = insert(data, i, &mut head, &mut prev);
        let (len, dist) = find(data, i, chain, &prev, dists, last);
        match held {
            // The next position beat it: let the held match go and keep the better one.
            Some((hl, _)) if len > hl => {
                w.literal(data[i - 1] as u32);
                held = Some((len, dist));
                i += 1;
            }
            Some((hl, hd)) => {
                w.pair(hl, hd);
                last = hd;
                // The match started one byte back, so it ends here; every position it
                // covers still belongs in the chain for later matches to find.
                let end = i - 1 + hl;
                for k in i + 1..end {
                    insert(data, k, &mut head, &mut prev);
                }
                i = end;
                held = None;
            }
            None if len >= MIN_MATCH => {
                held = Some((len, dist));
                i += 1;
            }
            None => {
                w.literal(data[i] as u32);
                i += 1;
            }
        }
    }
    if let Some((hl, hd)) = held {
        // Nothing followed it, so the held match stands as it is.
        w.pair(hl, hd);
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

    /// A repeat at a distance the caller never hints at is exactly what the old
    /// search could not see: it only ever probed the distances it was given.
    #[test]
    fn finds_matches_the_hints_miss() {
        let mut data = Vec::new();
        for i in 0..3000u32 {
            data.extend_from_slice(&(i % 251).to_le_bytes());
        }
        data.extend_from_within(..); // the whole thing again, 12000 bytes back
        let mut z = Vec::new();
        zlib(&data, &[4, 160], &mut z);
        assert_eq!(inflate_fixed(&z), data);
        assert!(z.len() < data.len() / 4, "{} of {}", z.len(), data.len());
    }

    /// A colour per dot is cobra's own hard case and the one that used to blow up:
    /// the picture repeats by dot and by row, but never at a single fixed distance.
    #[test]
    fn compresses_detailed_images() {
        const W: usize = 400;
        const DOT: usize = 5;
        let mut img = vec![0u8; W * 200 * 4];
        for y in 0..200 {
            for x in 0..W {
                let (dx, dy) = (x / DOT, y / DOT);
                let p = (y * W + x) * 4;
                img[p] = (dx * 3) as u8;
                img[p + 1] = (dy * 5) as u8;
                img[p + 2] = ((dx + dy) * 2) as u8;
                img[p + 3] = if (dx + dy) % 3 == 0 { 255 } else { 0 };
            }
        }
        let mut z = Vec::new();
        zlib(&img, &[4, 40, W * 4, W * 4 * 5], &mut z);
        assert_eq!(inflate_fixed(&z), img);
        // zlib itself needs ~20200 bytes for this at fixed Huffman, so this is the
        // search working, not the coding; what is left to win is a dynamic table.
        assert!(z.len() < 22_000, "{}", z.len());
    }

    #[test]
    fn compresses_flat_images() {
        let data = vec![0u8; 100_000];
        let mut z = Vec::new();
        zlib(&data, &[4, 400], &mut z);
        assert!(z.len() < 1000, "{}", z.len());
    }
}
