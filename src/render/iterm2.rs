//! iTerm2 inline images (OSC 1337), PNG payload placed over a box of cells.

use crate::encode::base64;

/// `cols` and `rows` size the picture in cells, so it covers exactly the room the
/// renderer made for it rather than however many cells its pixels come to.
pub(super) fn frame(png: &[u8], cols: u16, rows: u16, out: &mut Vec<u8>) {
    let header = format!(
        "\x1b]1337;File=inline=1;size={};width={cols};height={rows};preserveAspectRatio=0;doNotMoveCursor=1:",
        png.len()
    );
    out.extend_from_slice(header.as_bytes());
    base64::encode(png, out);
    out.push(0x07);
}
