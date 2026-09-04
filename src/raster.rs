//! Canvas → RGBA raster, aligned to the terminal's cell grid.

use crate::{Canvas, CellSize, Color, Palette};

/// Reusable rasteriser. Holds a per-cell lookup mask so a frame is one table lookup
/// per pixel and no per-dot geometry.
#[derive(Default)]
pub(crate) struct Raster {
    cell: CellSize,
    radius: f32,
    /// `cell.width × cell.height` entries: dot index `0..8`, or `8` for background.
    mask: Vec<u8>,
    pub rgba: Vec<u8>,
}

impl Raster {
    /// Builds the mask for a cell size and dot radius (fraction of the largest disc
    /// that fits a dot's slot).
    fn prepare(&mut self, cell: CellSize, radius: f32) {
        if self.cell == cell && self.radius == radius && !self.mask.is_empty() {
            return;
        }
        self.cell = cell;
        self.radius = radius;
        let (cw, ch) = (cell.width as usize, cell.height as usize);
        let (slot_w, slot_h) = (cw as f32 / 2.0, ch as f32 / 4.0);
        let r = radius.clamp(0.0, 1.0) * slot_w.min(slot_h) / 2.0;
        self.mask.clear();
        self.mask.resize(cw * ch, 8);
        for y in 0..ch {
            for x in 0..cw {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let (dx, dy) = ((px / slot_w) as usize, (py / slot_h) as usize);
                let (cx, cy) = ((dx as f32 + 0.5) * slot_w, (dy as f32 + 0.5) * slot_h);
                let (ex, ey) = (px - cx, py - cy);
                if ex * ex + ey * ey <= r * r {
                    self.mask[y * cw + x] = (dy.min(3) * 2 + dx.min(1)) as u8;
                }
            }
        }
    }

    /// Rasterises the top-left `cols × rows` cells of `canvas` into `self.rgba`.
    /// Returns the image size in pixels.
    pub fn draw(
        &mut self,
        canvas: &Canvas,
        cell: CellSize,
        radius: f32,
        palette: &Palette,
        cols: u16,
        rows: u16,
    ) -> (u32, u32) {
        self.prepare(cell, radius);
        let (cw, ch) = (cell.width as usize, cell.height as usize);
        let (w, h) = (cols as usize * cw, rows as usize * ch);
        self.rgba.clear();
        self.rgba.resize(w * h * 4, 0);
        let stride = w * 4;
        for row in 0..rows {
            for col in 0..cols {
                let dots = canvas.cell_dots(col, row);
                if dots.iter().all(|&d| d == 0) {
                    continue;
                }
                // Lookup table: eight dot colours as RGBA plus a transparent entry.
                let mut lut = [[0u8; 4]; 9];
                for (i, &d) in dots.iter().enumerate() {
                    if d != 0 {
                        let c = Color::resolve_packed(d, palette);
                        lut[i] = [c.r, c.g, c.b, 255];
                    }
                }
                let x0 = col as usize * cw * 4;
                for y in 0..ch {
                    let line = &mut self.rgba[(row as usize * ch + y) * stride + x0..][..cw * 4];
                    let mrow = &self.mask[y * cw..(y + 1) * cw];
                    for (px, &m) in line.chunks_exact_mut(4).zip(mrow) {
                        px.copy_from_slice(&lut[m as usize]);
                    }
                }
            }
        }
        (w as u32, h as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rgb;

    #[test]
    fn dots_land_in_their_slots() {
        let mut c = Canvas::new(2, 1);
        c.set(3, 3, Rgb::hex(0x0000ff)); // bottom-right dot of cell 1
        let mut r = Raster::default();
        let (w, h) = r.draw(&c, CellSize { width: 8, height: 16 }, 1.0, &Palette::default(), 2, 1);
        assert_eq!((w, h), (16, 16));
        let px = |x: usize, y: usize| &r.rgba[(y * 16 + x) * 4..][..4];
        assert_eq!(px(14, 14), &[0, 0, 255, 255]); // slot centre
        assert_eq!(px(1, 1), &[0, 0, 0, 0]); // other cell stays clear
        assert_eq!(px(12, 3), &[0, 0, 0, 0]); // same cell, different slot
    }

    #[test]
    fn palette_colours_resolve_through_palette() {
        let mut c = Canvas::new(1, 1);
        c.set(0, 0, Color::Indexed(1));
        c.set(1, 0, Color::Foreground);
        let mut p = Palette::default();
        p.colors[1] = Rgb::hex(0x123456);
        p.foreground = Rgb::hex(0xabcdef);
        let mut r = Raster::default();
        r.draw(&c, CellSize { width: 8, height: 16 }, 1.0, &p, 1, 1);
        let px = |x: usize, y: usize| &r.rgba[(y * 8 + x) * 4..][..4];
        assert_eq!(px(2, 2), &[0x12, 0x34, 0x56, 255]);
        assert_eq!(px(6, 2), &[0xab, 0xcd, 0xef, 255]);
    }
}
