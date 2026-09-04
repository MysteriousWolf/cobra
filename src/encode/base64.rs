//! Standard base64 (RFC 4648, with padding).

const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Appends the base64 encoding of `input` to `out`.
pub(crate) fn encode(input: &[u8], out: &mut Vec<u8>) {
    out.reserve(input.len().div_ceil(3) * 4);
    let (chunks, rest) = input.as_chunks::<3>();
    for c in chunks {
        let n = (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32;
        out.extend_from_slice(&[
            TABLE[(n >> 18) as usize & 63],
            TABLE[(n >> 12) as usize & 63],
            TABLE[(n >> 6) as usize & 63],
            TABLE[n as usize & 63],
        ]);
    }
    match rest.len() {
        1 => {
            let n = (rest[0] as u32) << 16;
            out.extend_from_slice(&[TABLE[(n >> 18) as usize & 63], TABLE[(n >> 12) as usize & 63], b'=', b'=']);
        }
        2 => {
            let n = (rest[0] as u32) << 16 | (rest[1] as u32) << 8;
            out.extend_from_slice(&[
                TABLE[(n >> 18) as usize & 63],
                TABLE[(n >> 12) as usize & 63],
                TABLE[(n >> 6) as usize & 63],
                b'=',
            ]);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn rfc_vectors() {
        for (i, o) in
            [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg=="), ("foobar", "Zm9vYmFy")]
        {
            let mut v = Vec::new();
            super::encode(i.as_bytes(), &mut v);
            assert_eq!(v, o.as_bytes());
        }
    }
}
