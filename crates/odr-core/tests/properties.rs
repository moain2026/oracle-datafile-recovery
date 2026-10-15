//! Property tests: decoders never panic on arbitrary input, and NUMBER
//! round-trips through a reference encoder.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use odr_core::ByteOrder;
use odr_core::block::classify;
use odr_core::types::{OraDate, OraNumber};
use proptest::prelude::*;

/// Reference encoder for positive/negative integers (independent of the decoder).
fn encode_int(v: i128) -> Vec<u8> {
    if v == 0 {
        return vec![0x80];
    }
    let neg = v < 0;
    let mut m = v.unsigned_abs();
    let mut digits = Vec::new();
    while m > 0 {
        digits.push((m % 100) as u8);
        m /= 100;
    }
    digits.reverse();
    let exp = i16::try_from(digits.len()).unwrap() - 1;
    while digits.last() == Some(&0) {
        digits.pop();
    }
    let mut out = Vec::new();
    if neg {
        out.push(u8::try_from(0x3E - exp).unwrap());
        out.extend(digits.iter().map(|d| 101 - d));
        if digits.len() < 20 {
            out.push(0x66);
        }
    } else {
        out.push(u8::try_from(0xC1 + exp).unwrap());
        out.extend(digits.iter().map(|d| d + 1));
    }
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn number_never_panics(raw in proptest::collection::vec(any::<u8>(), 0..30)) {
        let _ = OraNumber::decode(&raw).map(|n| n.to_string());
    }

    #[test]
    fn date_never_panics(raw in proptest::collection::vec(any::<u8>(), 0..10)) {
        let _ = OraDate::decode(&raw).map(|d| d.to_string());
    }

    #[test]
    fn classify_never_panics(raw in proptest::collection::vec(any::<u8>(), 0..9000), big in any::<bool>()) {
        let o = if big { ByteOrder::Big } else { ByteOrder::Little };
        let _ = classify(&raw, 8192, o);
    }

    #[test]
    fn number_integer_roundtrip(v in -(10i128.pow(37))..10i128.pow(37)) {
        let n = OraNumber::decode(&encode_int(v)).unwrap();
        prop_assert_eq!(n.to_string(), v.to_string());
    }
}
