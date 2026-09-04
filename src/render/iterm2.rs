//! iTerm2 inline images (OSC 1337), PNG payload sized in pixels.

use crate::encode::base64;

pub(super) fn frame(png: &[u8], width: u32, height: u32, out: &mut Vec<u8>) {
    let header = format!(
        "\x1b]1337;File=inline=1;size={};width={width}px;height={height}px;preserveAspectRatio=0;doNotMoveCursor=1:",
        png.len()
    );
    out.extend_from_slice(header.as_bytes());
    base64::encode(png, out);
    out.push(0x07);
}
