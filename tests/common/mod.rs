//! Decoders for the wire formats cobra writes, so integration tests can check that a
//! frame really carries the picture rather than that it starts with the right bytes.
//!
//! Everything here is deliberately independent of `src/encode`: a round trip is only
//! evidence when the two directions do not share the code under test.

#![allow(dead_code)] // Each integration test binary uses a different subset.

/// One decoded RGBA pixel.
pub type Pixel = [u8; 4];

/// A decoded image: pixels row-major, plus its dimensions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<Pixel>,
}

impl Image {
    pub fn at(&self, x: usize, y: usize) -> Pixel {
        assert!(x < self.width && y < self.height, "({x}, {y}) outside {}x{}", self.width, self.height);
        self.pixels[y * self.width + x]
    }

    /// Every distinct opaque colour in the image, sorted.
    pub fn colors(&self) -> Vec<[u8; 3]> {
        let mut v: Vec<[u8; 3]> =
            self.pixels.iter().filter(|p| p[3] != 0).map(|p| [p[0], p[1], p[2]]).collect::<Vec<_>>();
        v.sort_unstable();
        v.dedup();
        v
    }

    pub fn opaque_count(&self) -> usize {
        self.pixels.iter().filter(|p| p[3] != 0).count()
    }

    pub fn from_rgba(rgba: &[u8], width: usize, height: usize) -> Self {
        assert_eq!(rgba.len(), width * height * 4, "raster is not {width}x{height} RGBA");
        let pixels = rgba.as_chunks::<4>().0.to_vec();
        Self { width, height, pixels }
    }
}

// ---------------------------------------------------------------- base64

pub fn base64_decode(input: &[u8]) -> Vec<u8> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for &b in input {
        if b == b'=' {
            break;
        }
        let v = TABLE.iter().position(|&t| t == b).unwrap_or_else(|| panic!("not base64: {:?}", b as char));
        acc = acc << 6 | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

// ---------------------------------------------------------------- inflate

/// Inflate for the fixed-Huffman zlib streams cobra emits (RFC 1950/1951).
///
/// Also verifies the zlib header and the trailing Adler-32, so a corrupt stream fails
/// here rather than silently decoding to the wrong bytes.
pub fn inflate_zlib(z: &[u8]) -> Vec<u8> {
    assert!(z.len() >= 6, "zlib stream too short: {} bytes", z.len());
    let cmf = z[0];
    assert_eq!(cmf & 0x0f, 8, "not deflate");
    assert_eq!((u16::from(cmf) << 8 | u16::from(z[1])) % 31, 0, "bad zlib header check");
    let out = inflate_raw(&z[2..z.len() - 4]);
    let want = u32::from_be_bytes(z[z.len() - 4..].try_into().unwrap());
    assert_eq!(adler32(&out), want, "Adler-32 mismatch");
    out
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &x in chunk {
            a += u32::from(x);
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    b << 16 | a
}

struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    bit: u32,
}

impl BitReader<'_> {
    fn bit(&mut self) -> u32 {
        let b = u32::from(self.data[self.pos] >> self.bit) & 1;
        self.bit += 1;
        if self.bit == 8 {
            self.bit = 0;
            self.pos += 1;
        }
        b
    }

    /// LSB-first field (headers, extra bits).
    fn bits(&mut self, n: u32) -> u32 {
        (0..n).fold(0, |acc, i| acc | self.bit() << i)
    }

    /// MSB-first field (Huffman codes).
    fn code(&mut self, n: u32) -> u32 {
        (0..n).fold(0, |acc, _| acc << 1 | self.bit())
    }
}

fn inflate_raw(data: &[u8]) -> Vec<u8> {
    const LEN_BASE: [u16; 29] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227,
        258,
    ];
    const LEN_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
    const DIST_BASE: [u16; 30] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097,
        6145, 8193, 12289, 16385, 24577,
    ];
    const DIST_EXTRA: [u8; 30] =
        [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

    let mut r = BitReader { data, pos: 0, bit: 0 };
    let mut out = Vec::new();
    loop {
        let final_block = r.bits(1) == 1;
        assert_eq!(r.bits(2), 1, "expected a fixed-Huffman block");
        loop {
            // Fixed literal/length alphabet: 7-bit codes first, then 8, then 9.
            let mut c = r.code(7);
            let sym = if c <= 0b001_0111 {
                c + 256
            } else {
                c = c << 1 | r.bit();
                if c <= 0b1011_1111 {
                    c - 0x30
                } else if c <= 0b1100_0111 {
                    c - 0xC0 + 280
                } else {
                    (c << 1 | r.bit()) - 0x190 + 144
                }
            };
            match sym {
                256 => break,
                0..=255 => out.push(sym as u8),
                _ => {
                    let li = sym as usize - 257;
                    let len = LEN_BASE[li] as usize + r.bits(u32::from(LEN_EXTRA[li])) as usize;
                    let di = r.code(5) as usize;
                    let dist = DIST_BASE[di] as usize + r.bits(u32::from(DIST_EXTRA[di])) as usize;
                    assert!(dist <= out.len(), "back-reference before the start of the stream");
                    for _ in 0..len {
                        out.push(out[out.len() - dist]);
                    }
                }
            }
        }
        if final_block {
            return out;
        }
    }
}

// ---------------------------------------------------------------- PNG

/// Decodes an 8-bit RGBA PNG, checking every chunk CRC.
pub fn decode_png(png: &[u8]) -> Image {
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "not a PNG");
    let (mut rest, mut idat, mut dims) = (&png[8..], Vec::new(), None);
    let mut saw_iend = false;
    while rest.len() >= 12 {
        let len = u32::from_be_bytes(rest[..4].try_into().unwrap()) as usize;
        let kind = &rest[4..8];
        let body = &rest[8..8 + len];
        let crc = u32::from_be_bytes(rest[8 + len..12 + len].try_into().unwrap());
        assert_eq!(crc32(&rest[4..8 + len]), crc, "bad CRC on {:?}", std::str::from_utf8(kind));
        match kind {
            b"IHDR" => {
                let w = u32::from_be_bytes(body[..4].try_into().unwrap()) as usize;
                let h = u32::from_be_bytes(body[4..8].try_into().unwrap()) as usize;
                assert_eq!(&body[8..], &[8, 6, 0, 0, 0], "expected 8-bit RGBA, no interlace");
                dims = Some((w, h));
            }
            b"IDAT" => idat.extend_from_slice(body),
            b"IEND" => saw_iend = true,
            _ => {}
        }
        rest = &rest[12 + len..];
    }
    assert!(saw_iend, "no IEND chunk");
    let (width, height) = dims.expect("no IHDR chunk");
    let raw = inflate_zlib(&idat);
    let stride = width * 4;
    assert_eq!(raw.len(), height * (stride + 1), "unexpected scanline length");
    let mut pixels = Vec::with_capacity(width * height);
    for row in raw.chunks_exact(stride + 1) {
        assert_eq!(row[0], 0, "only filter type 0 is expected");
        pixels.extend_from_slice(row[1..].as_chunks::<4>().0);
    }
    Image { width, height, pixels }
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

// ---------------------------------------------------------------- kitty

/// A kitty graphics frame split into its control data and reassembled payload.
pub struct KittyFrame {
    /// The `key=value` control pairs of the first chunk.
    pub control: Vec<(String, String)>,
    /// The base64-decoded, zlib-inflated payload.
    pub payload: Vec<u8>,
}

impl KittyFrame {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.control.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    /// The payload as an image, using `s` and `v` from the control data.
    pub fn image(&self) -> Image {
        let w: usize = self.get("s").expect("no width").parse().unwrap();
        let h: usize = self.get("v").expect("no height").parse().unwrap();
        Image::from_rgba(&self.payload, w, h)
    }
}

/// Every image in a frame. An oversized frame is sent as several, each one a
/// horizontal band placed under the one before it.
pub fn parse_kitty_all(frame: &[u8]) -> Vec<KittyFrame> {
    // A new image starts at a chunk whose control data carries the action; the chunks
    // after it carry `m=` alone.
    let mut starts: Vec<usize> = Vec::new();
    let mut at = 0;
    while let Some(i) = find(&frame[at..], b"\x1b_Ga=") {
        starts.push(at + i);
        at += i + 3;
    }
    assert!(!starts.is_empty(), "no kitty images in frame");
    starts
        .iter()
        .enumerate()
        .map(|(k, &from)| {
            let to = starts.get(k + 1).copied().unwrap_or(frame.len());
            parse_kitty(&frame[from..to])
        })
        .collect()
}

/// The bands of an oversized frame stacked back into the single picture they tile.
/// The raster is row-major, so that is their payloads end to end.
pub fn stack(tiles: &[KittyFrame]) -> Image {
    let w: usize = tiles[0].get("s").expect("no width").parse().unwrap();
    let h: usize = tiles.iter().map(|t| t.get("v").unwrap().parse::<usize>().unwrap()).sum();
    let rgba: Vec<u8> = tiles.iter().flat_map(|t| t.payload.iter().copied()).collect();
    Image::from_rgba(&rgba, w, h)
}

/// Parses the `ESC _ G ... ESC \` chunks of one kitty image, checking the `m=`
/// chunking flags line up with the number of chunks.
pub fn parse_kitty(frame: &[u8]) -> KittyFrame {
    let mut control = Vec::new();
    let mut b64 = Vec::new();
    let mut rest = frame;
    let mut chunks = 0usize;
    let mut saw_last = false;
    while let Some(start) = find(rest, b"\x1b_G") {
        let body = &rest[start + 3..];
        let end = find(body, b"\x1b\\").expect("unterminated APC");
        let (chunk, tail) = (&body[..end], &body[end + 2..]);
        let semi = find(chunk, b";").expect("no ';' in chunk");
        let (head, data) = (&chunk[..semi], &chunk[semi + 1..]);
        let head = std::str::from_utf8(head).unwrap();
        if chunks == 0 {
            for pair in head.split(',') {
                let (k, v) = pair.split_once('=').unwrap_or_else(|| panic!("bad control pair {pair:?}"));
                control.push((k.to_string(), v.to_string()));
            }
        }
        let more = control_flag(head);
        assert!(!saw_last, "a chunk followed the one marked m=0");
        saw_last = !more;
        b64.extend_from_slice(data);
        chunks += 1;
        rest = tail;
    }
    assert!(chunks > 0, "no kitty chunks in frame");
    assert!(saw_last, "last chunk is not marked m=0");
    let compressed = matches!(control.iter().find(|(k, _)| k == "o"), Some((_, v)) if v == "z");
    let raw = base64_decode(&b64);
    let payload = if compressed { inflate_zlib(&raw) } else { raw };
    KittyFrame { control, payload }
}

/// `m=1` (more chunks follow) anywhere in a chunk's control data.
fn control_flag(head: &str) -> bool {
    head.split(',').any(|p| p == "m=1")
}

// ---------------------------------------------------------------- iTerm2

/// Extracts the PNG from an `OSC 1337 ; File=...` frame, checking the declared size.
pub fn parse_iterm2(frame: &[u8]) -> Image {
    let start = find(frame, b"\x1b]1337;File=").expect("no OSC 1337 introducer") + 2;
    let colon = start + find(&frame[start..], b":").expect("no ':' before the payload");
    let header = std::str::from_utf8(&frame[start..colon]).unwrap();
    let end = colon + 1 + frame[colon + 1..].iter().position(|&b| b == 0x07).expect("no BEL terminator");
    let png = base64_decode(&frame[colon + 1..end]);

    let field = |name: &str| {
        header.split(';').find_map(|f| f.strip_prefix(name).map(str::to_string)).unwrap_or_else(|| panic!("no {name}"))
    };
    assert_eq!(field("size=").parse::<usize>().unwrap(), png.len(), "declared size does not match the payload");
    let image = decode_png(&png);
    // The box is given in cells, so it only has to divide the image evenly.
    for (name, px) in [("width=", image.width), ("height=", image.height)] {
        let cells: usize = field(name).parse().unwrap_or_else(|_| panic!("{name} is not a cell count"));
        assert!(cells > 0 && px % cells == 0, "{name}{cells} cells does not divide {px} px");
    }
    image
}

// ---------------------------------------------------------------- sixel

/// Decodes a DEC sixel sequence back to pixels. Unset pixels stay fully transparent,
/// matching the `P2=1` mode cobra writes.
pub fn parse_sixel(frame: &[u8]) -> Image {
    let start = find(frame, b"\x1bP").expect("no DCS introducer");
    let q = start + find(&frame[start..], b"q").expect("no 'q' terminating the DCS parameters");
    let end = find(frame, b"\x1b\\").expect("no ST terminator");
    let body = std::str::from_utf8(&frame[q + 1..end]).expect("sixel body is not UTF-8");

    let mut palette = std::collections::HashMap::<usize, [u8; 3]>::new();
    let (mut width, mut height) = (0usize, 0usize);
    let mut pixels: Vec<Pixel> = Vec::new();
    let (mut band, mut x, mut color) = (0usize, 0usize, 0usize);
    let mut repeat = 1usize;

    let mut chars = body.chars().peekable();
    let number = |chars: &mut std::iter::Peekable<std::str::Chars>| -> usize {
        let mut n = 0usize;
        while let Some(d) = chars.peek().and_then(|c| c.to_digit(10)) {
            n = n * 10 + d as usize;
            chars.next();
        }
        n
    };
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                // Raster attributes: pan;pad;width;height.
                let mut fields = Vec::new();
                for _ in 0..4 {
                    fields.push(number(&mut chars));
                    if chars.peek() == Some(&';') {
                        chars.next();
                    }
                }
                (width, height) = (fields[2], fields[3]);
                pixels = vec![[0, 0, 0, 0]; width * height];
            }
            '#' => {
                let index = number(&mut chars);
                if chars.peek() == Some(&';') {
                    chars.next();
                    let space = number(&mut chars);
                    assert_eq!(space, 2, "expected RGB colour space");
                    let mut rgb = [0u8; 3];
                    for channel in &mut rgb {
                        assert_eq!(chars.next(), Some(';'));
                        // Sixel colour components are percentages of 100.
                        *channel = ((number(&mut chars) * 255 + 50) / 100) as u8;
                    }
                    palette.insert(index, rgb);
                }
                color = index;
            }
            '!' => repeat = number(&mut chars),
            '$' => {
                x = 0;
                repeat = 1;
            }
            '-' => {
                band += 6;
                x = 0;
                repeat = 1;
            }
            '?'..='~' => {
                let bits = c as usize - 63;
                let rgb = *palette.get(&color).unwrap_or_else(|| panic!("colour {color} was never defined"));
                for _ in 0..repeat {
                    for row in 0..6 {
                        if bits >> row & 1 == 1 {
                            let (px, py) = (x, band + row);
                            assert!(px < width && py < height, "sixel writes ({px}, {py}) outside the raster");
                            pixels[py * width + px] = [rgb[0], rgb[1], rgb[2], 255];
                        }
                    }
                    x += 1;
                }
                repeat = 1;
            }
            _ => panic!("unexpected sixel byte {c:?}"),
        }
    }
    assert!(width > 0 && height > 0, "no raster attributes");
    Image { width, height, pixels }
}

// ---------------------------------------------------------------- misc

pub fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}
