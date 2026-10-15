//! `CHAR`/`NCHAR` (type 96), `VARCHAR2`/`NVARCHAR2` (type 1) and `RAW` (type 23).
//!
//! Text types are stored as bytes in the database (or national) character set:
//! decode them with [`crate::charset::decode`]. `CHAR` is blank-padded to its
//! declared length; Oracle comparison semantics ignore the padding, but the
//! padding **is** part of the stored value, so ODR keeps it by default and
//! offers [`trim_char_padding`] for exporters that want SQL-equivalent text.

/// Remove trailing ASCII spaces (`CHAR` blank padding in single-byte and
/// UTF-8 sets). For `AL16UTF16` padding is `00 20` pairs — use
/// [`trim_char_padding_utf16`].
pub fn trim_char_padding(raw: &[u8]) -> &[u8] {
    let end = raw
        .iter()
        .rposition(|&b| b != b' ')
        .map_or(0, |p| p.saturating_add(1));
    raw.get(..end).unwrap_or_default()
}

/// Remove trailing UTF-16BE spaces (`00 20`).
pub fn trim_char_padding_utf16(raw: &[u8]) -> &[u8] {
    let mut end = raw.len() - raw.len() % 2;
    while end >= 2 && raw.get(end - 2..end) == Some(&[0x00, 0x20]) {
        end -= 2;
    }
    raw.get(..end).unwrap_or_default()
}

/// Uppercase hexadecimal text of a `RAW` value (Oracle's `RAWTOHEX` form).
pub fn raw_to_hex(raw: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut s = String::with_capacity(raw.len().saturating_mul(2));
    for &b in raw {
        s.push(char::from(
            HEX.get(usize::from(b >> 4)).copied().unwrap_or(b'0'),
        ));
        s.push(char::from(
            HEX.get(usize::from(b & 0x0F)).copied().unwrap_or(b'0'),
        ));
    }
    s
}
