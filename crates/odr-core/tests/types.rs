//! Datatype tests: documented `DUMP()` vectors + independent reference encoders
//! + "never panics" properties. Real-file confirmation: LV-013..017.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::many_single_char_names
)]

use odr_core::types::char::{raw_to_hex, trim_char_padding, trim_char_padding_utf16};
use odr_core::types::*;
use proptest::prelude::*;

// ---------- TIMESTAMP ----------
#[test]
fn timestamp_vectors() {
    // SELECT DUMP(TIMESTAMP '2026-10-15 13:45:30.123456789') → Typ=180 Len=11:
    // 120,126,10,15,14,46,31,7,91,205,21
    let t = OraTimestamp::decode(&[120, 126, 10, 15, 14, 46, 31, 7, 91, 205, 21]).unwrap();
    assert_eq!(t.nanos, 123_456_789);
    assert_eq!(t.to_string(), "2026-10-15 13:45:30.123456789");
    // Zero fraction is omitted → 7 bytes.
    let t = OraTimestamp::decode(&[120, 126, 1, 1, 1, 1, 1]).unwrap();
    assert_eq!(t.to_string(), "2026-01-01 00:00:00");
    // 0.5 s → trailing zeros trimmed.
    let half = 500_000_000u32.to_be_bytes();
    let t = OraTimestamp::decode(&[120, 126, 1, 1, 1, 1, 1, half[0], half[1], half[2], half[3]])
        .unwrap();
    assert_eq!(t.to_string(), "2026-01-01 00:00:00.5");
    assert!(
        OraTimestamp::decode(&[120, 126, 1, 1, 1, 1, 1, 0x3B, 0x9A, 0xCA, 0x00]).is_err(),
        "1e9 ns rejected"
    );
    assert!(
        OraTimestamp::decode(&[120, 126, 1, 1, 1, 1, 1, 0]).is_err(),
        "8 bytes rejected"
    );
}

#[test]
fn timestamp_tz_vectors() {
    // Asia/Aden is UTC+03:00 → offset bytes 23, 60. Stored instant is UTC.
    let raw = [120, 126, 10, 15, 11, 1, 1, 0, 0, 0, 0, 23, 60];
    let t = OraTimestamp::decode_tz(&raw).unwrap();
    assert_eq!(t.tz, Some(TimeZone::Offset(180)));
    assert_eq!(t.to_string(), "2026-10-15 10:00:00 UTC+03:00");
    // −05:30 → bytes 15, 30 (minute stored as 60 − 30)
    let raw = [120, 126, 1, 1, 1, 1, 1, 0, 0, 0, 0, 15, 30];
    assert_eq!(
        OraTimestamp::decode_tz(&raw).unwrap().tz,
        Some(TimeZone::Offset(-330))
    );
    // Region form: bit 7 of byte 11 set.
    let raw = [120, 126, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0x80 | 0x05, 0x34 << 2];
    assert_eq!(
        OraTimestamp::decode_tz(&raw).unwrap().tz,
        Some(TimeZone::Region((5 << 6) | 0x34))
    );
    assert!(
        OraTimestamp::decode_tz(&[120, 126, 1, 1, 1, 1, 1, 0, 0, 0, 0, 60, 60]).is_err(),
        "+40h rejected"
    );
    assert!(OraTimestamp::decode_tz(&[120, 126, 1, 1, 1, 1, 1]).is_err());
}

// ---------- INTERVAL ----------
fn enc_ym(y: i32, m: i8) -> Vec<u8> {
    let mut v = ((i64::from(y) + 0x8000_0000) as u32).to_be_bytes().to_vec();
    v.push((i16::from(m) + 60) as u8);
    v
}
fn enc_ds(d: i32, h: i8, mi: i8, s: i8, n: i32) -> Vec<u8> {
    let mut v = ((i64::from(d) + 0x8000_0000) as u32).to_be_bytes().to_vec();
    v.extend([(h + 60) as u8, (mi + 60) as u8, (s + 60) as u8]);
    v.extend(((i64::from(n) + 0x8000_0000) as u32).to_be_bytes());
    v
}

#[test]
fn interval_vectors() {
    // DUMP(INTERVAL '2-3' YEAR TO MONTH) = 128,0,0,2,63
    assert_eq!(
        IntervalYm::decode(&[128, 0, 0, 2, 63]).unwrap().to_string(),
        "+2-03"
    );
    assert_eq!(
        IntervalYm::decode(&enc_ym(-2, -3)).unwrap().to_string(),
        "-2-03"
    );
    assert!(IntervalYm::decode(&enc_ym(2, -3)).is_err(), "mixed sign");
    assert!(
        IntervalYm::decode(&[128, 0, 0, 2, 72]).is_err(),
        "12 months"
    );
    let ds = IntervalDs::decode(&enc_ds(4, 5, 12, 10, 222_000_000)).unwrap();
    assert_eq!(ds.to_string(), "+4 05:12:10.222");
    assert_eq!(
        IntervalDs::decode(&enc_ds(-1, -2, 0, 0, 0))
            .unwrap()
            .to_string(),
        "-1 02:00:00"
    );
    assert!(
        IntervalDs::decode(&enc_ds(1, -2, 0, 0, 0)).is_err(),
        "mixed sign"
    );
    assert!(
        IntervalDs::decode(&enc_ds(0, 24, 0, 0, 0)).is_err(),
        "24 hours"
    );
}

// ---------- BINARY_FLOAT / DOUBLE ----------
fn enc_f64(x: f64) -> [u8; 8] {
    let b = x.to_bits();
    (if b >> 63 == 0 { b | (1 << 63) } else { !b }).to_be_bytes()
}
fn enc_f32(x: f32) -> [u8; 4] {
    let b = x.to_bits();
    (if b >> 31 == 0 { b | (1 << 31) } else { !b }).to_be_bytes()
}

#[test]
fn binary_float_vectors() {
    // DUMP(1.5f) = 191,192,0,0 ; DUMP(-1.5f) = 64,63,255,255
    assert_eq!(decode_binary_float(&[191, 192, 0, 0]).unwrap(), 1.5);
    assert_eq!(decode_binary_float(&[64, 63, 255, 255]).unwrap(), -1.5);
    assert_eq!(
        decode_binary_double(&enc_f64(0.0)).unwrap().to_bits(),
        0.0f64.to_bits()
    );
    assert!(decode_binary_double(&enc_f64(f64::NAN)).unwrap().is_nan());
    assert_eq!(
        decode_binary_double(&enc_f64(f64::NEG_INFINITY)).unwrap(),
        f64::NEG_INFINITY
    );
    assert!(decode_binary_float(&[1, 2, 3]).is_err());
}

// ---------- ROWID / CHAR / RAW ----------
#[test]
fn rowid_vectors() {
    // objd 74565, file 4, block 135, slot 0 → 'AAASNFAAEAAAACHAAA'
    let mut raw = 74_565u32.to_be_bytes().to_vec();
    raw.extend(((4u32 << 22) | 0x87).to_be_bytes());
    raw.extend(0u16.to_be_bytes());
    let r = OraRowid::decode(&raw).unwrap();
    assert_eq!(r.to_string(), "AAASNFAAEAAAACHAAA");
    assert!(OraRowid::decode(&raw[..9]).is_err());
}

#[test]
fn char_and_raw() {
    assert_eq!(trim_char_padding(b"ABC   "), b"ABC");
    assert_eq!(trim_char_padding(b"   "), b"");
    assert_eq!(
        trim_char_padding_utf16(&[0x06, 0x39, 0x00, 0x20, 0x00, 0x20]),
        &[0x06, 0x39]
    );
    assert_eq!(raw_to_hex(&[0x00, 0xAB, 0xFF]), "00ABFF");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn never_panics(raw in proptest::collection::vec(any::<u8>(), 0..16)) {
        let _ = OraTimestamp::decode(&raw).map(|t| t.to_string());
        let _ = OraTimestamp::decode_tz(&raw).map(|t| t.to_string());
        let _ = IntervalYm::decode(&raw).map(|t| t.to_string());
        let _ = IntervalDs::decode(&raw).map(|t| t.to_string());
        let _ = decode_binary_float(&raw);
        let _ = decode_binary_double(&raw);
        let _ = OraRowid::decode(&raw).map(|r| r.to_string());
        let _ = trim_char_padding_utf16(&raw);
    }

    #[test]
    fn binary_roundtrip(x in any::<f64>(), y in any::<f32>()) {
        prop_assert_eq!(decode_binary_double(&enc_f64(x)).unwrap().to_bits(), x.to_bits());
        prop_assert_eq!(decode_binary_float(&enc_f32(y)).unwrap().to_bits(), y.to_bits());
    }

    #[test]
    fn binary_encoding_preserves_order(a in -1e300f64..1e300, b in -1e300f64..1e300) {
        // The whole point of Oracle's transform: byte order == numeric order.
        prop_assert_eq!(enc_f64(a).cmp(&enc_f64(b)), a.partial_cmp(&b).unwrap());
    }

    #[test]
    fn interval_roundtrip(d in 0i32..1_000_000, h in 0i8..24, mi in 0i8..60, s in 0i8..60, n in 0i32..1_000_000_000, neg in any::<bool>()) {
        let k = if neg { -1 } else { 1 };
        let v = IntervalDs::decode(&enc_ds(k * d, k as i8 * h, k as i8 * mi, k as i8 * s, k * n)).unwrap();
        prop_assert_eq!((v.days, v.hours, v.minutes, v.seconds, v.nanos), (k * d, k as i8 * h, k as i8 * mi, k as i8 * s, k * n));
    }
}
