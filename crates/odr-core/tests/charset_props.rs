//! Charset property tests: never panic; valid input round-trips exactly.
// Test code: the reference encoder masks values before casting.
#![allow(
    clippy::unwrap_used,
    clippy::cast_possible_truncation,
    clippy::arithmetic_side_effects
)]

use odr_core::charset::{Charset, decode};
use proptest::prelude::*;

const ALL: [Charset; 8] = [
    Charset::Us7Ascii,
    Charset::We8Iso8859P1,
    Charset::We8MsWin1252,
    Charset::Ar8Iso8859P6,
    Charset::Ar8MsWin1256,
    Charset::Utf8Cesu,
    Charset::Al32Utf8,
    Charset::Al16Utf16,
];

/// Reference CESU-8 encoder (independent of the decoder).
fn cesu8(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for u in s.encode_utf16() {
        let c = u32::from(u);
        match c {
            0..=0x7F => out.push(c as u8),
            0x80..=0x7FF => out.extend([0xC0 | (c >> 6) as u8, 0x80 | (c & 0x3F) as u8]),
            _ => out.extend([
                0xE0 | (c >> 12) as u8,
                0x80 | ((c >> 6) & 0x3F) as u8,
                0x80 | (c & 0x3F) as u8,
            ]),
        }
    }
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn never_panics(raw in proptest::collection::vec(any::<u8>(), 0..64)) {
        for cs in ALL {
            let d = decode(&raw, cs);
            prop_assert!(d.invalid <= raw.len() + 1);
        }
    }

    #[test]
    fn unicode_roundtrips(s in "\\PC{0,40}") {
        prop_assert_eq!(decode(s.as_bytes(), Charset::Al32Utf8).text, s.as_str());
        let utf16: Vec<u8> = s.encode_utf16().flat_map(u16::to_be_bytes).collect();
        let d = decode(&utf16, Charset::Al16Utf16);
        prop_assert_eq!(&d.text, s.as_str());
        let c = cesu8(&s);
        let d = decode(&c, Charset::Utf8Cesu);
        prop_assert_eq!((d.text.as_ref(), d.invalid), (s.as_str(), 0));
    }

    #[test]
    fn single_byte_sets_decode_one_char_per_byte(raw in proptest::collection::vec(any::<u8>(), 0..64)) {
        for cs in [Charset::We8Iso8859P1, Charset::We8MsWin1252, Charset::Ar8Iso8859P6, Charset::Ar8MsWin1256] {
            prop_assert_eq!(decode(&raw, cs).text.chars().count(), raw.len());
        }
    }
}
