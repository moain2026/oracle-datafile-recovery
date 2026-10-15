//! Oracle `NUMBER` (internal type 2): exact decoding to a decimal string.
//!
//! Encoding (\[O2\], \[W4\]):
//! * `[0x80]` is zero. `[0x00]` is −∞; `[0xFF, 0x65]` is +∞.
//! * Byte 0 holds sign and base-100 exponent. Positive: `e = b0 − 0xC1`;
//!   negative: `e = 0x3E − b0`.
//! * Then 1–20 mantissa bytes, each one base-100 digit. Positive digit
//!   `d = b − 1` (b ∈ 1..=100); negative digit `d = 101 − b` (b ∈ 2..=101).
//! * Negative numbers with fewer than 20 mantissa digits end with `0x66`.
//! * Value = Σ dᵢ · 100^(e − i), i = 0, 1, …
//!
//! Strictness: Oracle always writes normalised numbers (first and last digit
//! non-zero). We **reject** non-normalised input as malformed, because in a
//! damaged file it is a strong corruption signal (wrong value > missing value).

// Arithmetic here is on `u8`/`i16`/`i32` values whose ranges are checked just
// before use (exponent ∈ −65..=62, digits ∈ 0..=99, len ≤ 20), so overflow is
// impossible; property tests and fuzzing enforce this.
#![allow(clippy::arithmetic_side_effects)]

use crate::error::{DecodeError, DecodeResult};
use std::fmt;

const WHAT: &str = "NUMBER";
/// Maximum encoded length in bytes.
pub const MAX_LEN: usize = 22;
const MAX_DIGITS: usize = 20;

/// A decoded Oracle `NUMBER`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OraNumber {
    /// Zero.
    Zero,
    /// Positive infinity.
    PosInfinity,
    /// Negative infinity.
    NegInfinity,
    /// Finite non-zero value: `±0.d₀d₁… × 100^(exponent+1)`.
    Finite {
        /// `true` if negative.
        negative: bool,
        /// Base-100 exponent of the first digit.
        exponent: i16,
        /// Base-100 digits (each 0..=99); only `len` are meaningful.
        digits: [u8; MAX_DIGITS],
        /// Number of meaningful digits (1..=20).
        len: u8,
    },
}

impl OraNumber {
    /// Decode raw column bytes.
    ///
    /// # Errors
    /// [`DecodeError::Malformed`] for any encoding Oracle would not produce.
    pub fn decode(raw: &[u8]) -> DecodeResult<Self> {
        let (&b0, rest) = raw
            .split_first()
            .ok_or(DecodeError::malformed(WHAT, 0, "empty"))?;
        if raw.len() > MAX_LEN {
            return Err(DecodeError::malformed(
                WHAT,
                MAX_LEN,
                "longer than 22 bytes",
            ));
        }
        match (b0, rest) {
            (0x80, []) => return Ok(Self::Zero),
            (0x00, []) => return Ok(Self::NegInfinity),
            (0xFF, [0x65]) => return Ok(Self::PosInfinity),
            (0x80, _) => return Err(DecodeError::malformed(WHAT, 1, "zero with mantissa")),
            (_, []) => return Err(DecodeError::malformed(WHAT, 1, "missing mantissa")),
            _ => {}
        }
        let negative = b0 & 0x80 == 0;
        let (exponent, mantissa) = if negative {
            let e = 0x3E_i16 - i16::from(b0);
            let m = match rest.split_last() {
                Some((&0x66, m)) => m,
                _ if rest.len() == MAX_DIGITS => rest,
                _ => {
                    return Err(DecodeError::malformed(
                        WHAT,
                        raw.len(),
                        "negative without 0x66 terminator",
                    ));
                }
            };
            (e, m)
        } else {
            (i16::from(b0) - 0xC1, rest)
        };
        if mantissa.is_empty() || mantissa.len() > MAX_DIGITS {
            return Err(DecodeError::malformed(
                WHAT,
                1,
                "mantissa length out of range",
            ));
        }
        let mut digits = [0u8; MAX_DIGITS];
        for (i, (&b, slot)) in mantissa.iter().zip(digits.iter_mut()).enumerate() {
            let d = if negative {
                if !(2..=101).contains(&b) {
                    return Err(DecodeError::malformed(
                        WHAT,
                        i + 1,
                        "negative digit out of range",
                    ));
                }
                101 - b
            } else {
                if !(1..=100).contains(&b) {
                    return Err(DecodeError::malformed(
                        WHAT,
                        i + 1,
                        "positive digit out of range",
                    ));
                }
                b - 1
            };
            *slot = d;
        }
        let len = mantissa.len();
        if digits.first() == Some(&0) {
            return Err(DecodeError::malformed(
                WHAT,
                1,
                "leading zero digit (not normalised)",
            ));
        }
        if digits.get(len - 1) == Some(&0) {
            return Err(DecodeError::malformed(
                WHAT,
                len,
                "trailing zero digit (not normalised)",
            ));
        }
        #[allow(clippy::cast_possible_truncation)] // len <= 20
        Ok(Self::Finite {
            negative,
            exponent,
            digits,
            len: len as u8,
        })
    }
}

impl OraNumber {
    /// The value as an exact `i64`, for dictionary columns (`OBJ#`, `FILE#`,
    /// `BLOCK#`, `TYPE#`…). `None` if it has a fractional part, is infinite,
    /// or does not fit — never a rounded or truncated value.
    pub fn to_i64(&self) -> Option<i64> {
        let (negative, exponent, digits, len) = match *self {
            Self::Zero => return Some(0),
            Self::PosInfinity | Self::NegInfinity => return None,
            Self::Finite {
                negative,
                exponent,
                digits,
                len,
            } => (negative, exponent, digits, len),
        };
        // value = Σ digits[k] · 100^(exponent − k); integral iff every
        // meaningful digit has a non-negative power (trailing zero digits are
        // never stored, so `len` already excludes them).
        let last = i32::from(len).checked_sub(1)?;
        if exponent < 0 || i32::from(exponent) < last {
            return None;
        }
        let mut acc: i128 = 0;
        for k in 0..=i32::from(exponent) {
            let d = if k <= last {
                i128::from(*digits.get(usize::try_from(k).ok()?)?)
            } else {
                0
            };
            acc = acc.checked_mul(100)?.checked_add(d)?;
            if acc > i128::from(i64::MAX) + 1 {
                return None;
            }
        }
        let signed = if negative { acc.checked_neg()? } else { acc };
        i64::try_from(signed).ok()
    }
}

impl fmt::Display for OraNumber {
    /// Canonical decimal text, e.g. `0`, `-12.5`, `0.000123`, `1E+125` is
    /// written out in full (`1` followed by 125 zeros). Infinity: `~` / `-~`
    /// (as Oracle's SQL*Plus prints them).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (negative, exponent, digits, len) = match *self {
            Self::Zero => return f.write_str("0"),
            Self::PosInfinity => return f.write_str("~"),
            Self::NegInfinity => return f.write_str("-~"),
            Self::Finite {
                negative,
                exponent,
                digits,
                len,
            } => (negative, exponent, digits, len),
        };
        // Expand base-100 digits into decimal digits.
        let mut dec = String::with_capacity(usize::from(len) * 2);
        for d in digits.iter().take(usize::from(len)) {
            dec.push(char::from(b'0' + d / 10));
            dec.push(char::from(b'0' + d % 10));
        }
        // Decimal position of the point, measured from the start of `dec`.
        let point = (i32::from(exponent) + 1) * 2;
        let dec = dec.trim_end_matches('0');
        let lead = dec.len() - dec.trim_start_matches('0').len();
        let dec = dec.get(lead..).unwrap_or_default();
        let point = point - i32::try_from(lead).unwrap_or(0);
        let n = i32::try_from(dec.len()).unwrap_or(i32::MAX);
        if negative {
            f.write_str("-")?;
        }
        if point <= 0 {
            f.write_str("0.")?;
            for _ in 0..-point {
                f.write_str("0")?;
            }
            f.write_str(dec)
        } else if point >= n {
            f.write_str(dec)?;
            for _ in 0..point - n {
                f.write_str("0")?;
            }
            Ok(())
        } else {
            let p = usize::try_from(point).unwrap_or(0);
            let (int, frac) = dec.split_at(p);
            write!(f, "{int}.{frac}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(raw: &[u8]) -> String {
        OraNumber::decode(raw).map_or_else(|e| format!("ERR {e}"), |n| n.to_string())
    }

    #[test]
    fn known_vectors() {
        // Vectors from Oracle `SELECT DUMP(x) FROM dual` (documented in \[W4\]).
        assert_eq!(s(&[0x80]), "0");
        assert_eq!(s(&[0xC1, 0x02]), "1");
        assert_eq!(s(&[0xC2, 0x02]), "100");
        assert_eq!(s(&[0xC2, 0x02, 0x02]), "101");
        assert_eq!(s(&[0xC0, 0x33]), "0.5");
        assert_eq!(s(&[0xC1, 0x0D, 0x22]), "12.33");
        assert_eq!(s(&[0x3E, 0x64, 0x66]), "-1");
        assert_eq!(s(&[0x3D, 0x64, 0x66]), "-100");
        assert_eq!(s(&[0x3E, 0x59, 0x44, 0x66]), "-12.33");
        assert_eq!(s(&[0xBF, 0x02]), "0.0001");
        assert_eq!(s(&[0xFF, 0x65]), "~");
        assert_eq!(s(&[0x00]), "-~");
    }

    #[test]
    fn rejects_malformed() {
        for bad in [
            &[][..],
            &[0xC1],
            &[0xC1, 0x00],
            &[0xC1, 0x66],
            &[0x3E, 0x64],
            &[0xC1, 0x01],
            &[0x80, 0x01],
        ] {
            assert!(
                OraNumber::decode(bad).is_err(),
                "{bad:02X?} must be rejected"
            );
        }
        assert!(OraNumber::decode(&[0xC1; 23]).is_err());
    }
}
