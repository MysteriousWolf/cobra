//! DEC sixel encoder with a transparent background.
//!
//! Palette entries are assigned per distinct colour in the frame. Braille canvases use
//! few colours, so this is normally exact; past 255 colours the frame is quantised to
//! 3-3-2 bits.

/// Encodes `rgba` (`width × height`, alpha `0` = transparent) as a sixel sequence,
/// appending to `out`. `scratch` holds the per-pixel palette indices between calls.
pub(crate) fn encode(rgba: &[u8], width: usize, height: usize, scratch: &mut Vec<u8>, out: &mut Vec<u8>) {
    // Palette (index 0 is reserved for transparent).
    let mut palette: Vec<u32> = Vec::with_capacity(16);
    let mut quantise = false;
    scratch.clear();
    scratch.resize(width * height, 0);
    for pass in 0..2 {
        palette.clear();
        for (i, px) in rgba.chunks_exact(4).enumerate() {
            scratch[i] = if px[3] == 0 {
                0
            } else {
                let key = if quantise {
                    (px[0] as u32 & 0xE0) << 16 | (px[1] as u32 & 0xE0) << 8 | (px[2] as u32 & 0xC0)
                } else {
                    (px[0] as u32) << 16 | (px[1] as u32) << 8 | px[2] as u32
                };
                match palette.iter().position(|&c| c == key) {
                    Some(p) => p as u8 + 1,
                    None if palette.len() < 255 => {
                        palette.push(key);
                        palette.len() as u8
                    }
                    None => {
                        quantise = true;
                        break;
                    }
                }
            };
        }
        if !quantise || pass == 1 {
            break;
        }
    }

    // DCS P1=0 P2=1 (pixels not set stay transparent) P3=0 q, raster attributes.
    out.extend_from_slice(b"\x1bP0;1;0q");
    out.extend_from_slice(format!("\"1;1;{width};{height}").as_bytes());
    for (i, &c) in palette.iter().enumerate() {
        let pct = |v: u32| (v * 100 + 127) / 255;
        let line = format!("#{};2;{};{};{}", i + 1, pct(c >> 16 & 255), pct(c >> 8 & 255), pct(c & 255));
        out.extend_from_slice(line.as_bytes());
    }

    let mut present = [false; 256];
    for band in (0..height).step_by(6) {
        let rows = (height - band).min(6);
        present.fill(false);
        for y in band..band + rows {
            for &p in &scratch[y * width..(y + 1) * width] {
                present[p as usize] = true;
            }
        }
        for color in present.iter().enumerate().skip(1).filter_map(|(i, &p)| p.then_some(i)) {
            out.push(b'#');
            out.extend_from_slice(color.to_string().as_bytes());
            let (mut run_ch, mut run_n) = (0u8, 0usize);
            for x in 0..width {
                let mut bits = 0u8;
                for r in 0..rows {
                    if scratch[(band + r) * width + x] as usize == color {
                        bits |= 1 << r;
                    }
                }
                let ch = 63 + bits;
                if ch == run_ch {
                    run_n += 1;
                } else {
                    flush_run(out, run_ch, run_n);
                    run_ch = ch;
                    run_n = 1;
                }
            }
            flush_run(out, run_ch, run_n);
            out.push(b'$'); // carriage return within the band
        }
        out.push(b'-'); // next band
    }
    out.extend_from_slice(b"\x1b\\");
}

/// Emits `n` copies of `ch`, as a `!n` run when that is shorter. `n == 0` writes nothing.
fn flush_run(out: &mut Vec<u8>, ch: u8, n: usize) {
    if n > 3 {
        out.push(b'!');
        out.extend_from_slice(n.to_string().as_bytes());
        out.push(ch);
    } else {
        out.extend(std::iter::repeat_n(ch, n));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn encodes_two_colours() {
        let rgba = [255, 0, 0, 255, 0, 0, 0, 0, 0, 0, 255, 255, 255, 0, 0, 255];
        let (mut s, mut o) = (Vec::new(), Vec::new());
        super::encode(&rgba, 2, 2, &mut s, &mut o);
        let text = String::from_utf8(o).unwrap();
        assert!(text.starts_with("\x1bP0;1;0q\"1;1;2;2#1;2;100;0;0#2;2;0;0;100"));
        assert!(text.ends_with("-\x1b\\"));
        assert!(text.contains("#1@A$")); // red: (0,0) → bit 0 → '@', (1,1) → bit 1 → 'A'
        assert!(text.contains("#2A?$")); // blue: (0,1) → bit 1 → 'A', column 1 empty
    }
}
