//! `TIMESTAMP[(p)]` (type 180), `TIMESTAMP WITH TIME ZONE` (181) and
//! `TIMESTAMP WITH LOCAL TIME ZONE` (231).
//!
//! Layout (\[O2\], `lab-verify` LV-013/014):
//! * Bytes 0..7: exactly the `DATE` encoding ([`OraDate`]).
//! * Bytes 7..11 (optional): fractional seconds in **nanoseconds**, u32
//!   **big-endian on every platform**. Oracle omits them when they are zero,
//!   so a `TIMESTAMP` value is 7 or 11 bytes.
//! * WITH TIME ZONE adds 2 bytes (always 13 bytes total) and the date/time
//!   part is stored **in UTC**:
//!   - byte 11 bit 7 clear → numeric offset: `hour = b11 − 20`, `minute = b12 − 60`;
//!   - byte 11 bit 7 set → region id: 12-bit region in `(b11 & 0x7F) << 6 | b12 >> 2`
//!     (`lab-verify` LV-014). Region names need Oracle's timezone file and are
//!     reported as an id, never guessed.
//! * WITH LOCAL TIME ZONE uses the TIMESTAMP layout, normalised to the
//!   database time zone.

use super::date::OraDate;
use crate::error::{DecodeError, DecodeResult};
use std::fmt;

const WHAT: &str = "TIMESTAMP";

/// Time zone of a `TIMESTAMP WITH TIME ZONE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeZone {
    /// Fixed offset from UTC in minutes (−12:00 … +14:00).
    Offset(i16),
    /// Oracle time-zone region id (needs the timezone file to name it).
    Region(u16),
}

/// A decoded `TIMESTAMP`. For WITH TIME ZONE values, `utc` is the stored UTC
/// instant and `tz` the original zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OraTimestamp {
    /// Date and time (UTC when `tz` is set).
    pub utc: OraDate,
    /// Nanoseconds 0..=999 999 999.
    pub nanos: u32,
    /// Time zone, for type 181.
    pub tz: Option<TimeZone>,
}

impl OraTimestamp {
    /// Decode `TIMESTAMP` / `TIMESTAMP WITH LOCAL TIME ZONE` (7 or 11 bytes).
    ///
    /// # Errors
    /// [`DecodeError::Malformed`] on a wrong length or invalid field.
    pub fn decode(raw: &[u8]) -> DecodeResult<Self> {
        match raw.len() {
            7 | 11 => Self::decode_parts(raw, None),
            _ => Err(DecodeError::malformed(WHAT, 0, "length is not 7 or 11")),
        }
    }

    /// Decode `TIMESTAMP WITH TIME ZONE` (13 bytes).
    ///
    /// # Errors
    /// [`DecodeError::Malformed`] on a wrong length, invalid field or offset.
    pub fn decode_tz(raw: &[u8]) -> DecodeResult<Self> {
        let &[.., z0, z1] = raw else {
            return Err(DecodeError::malformed(WHAT, 0, "length is not 13"));
        };
        if raw.len() != 13 {
            return Err(DecodeError::malformed(WHAT, 0, "length is not 13"));
        }
        let tz = if z0 & 0x80 == 0 {
            let h = i16::from(z0) - 20;
            let m = i16::from(z1) - 60;
            let total = h * 60 + if h < 0 { -m.abs() } else { m };
            if !(-12..=14).contains(&h)
                || !(-59..=59).contains(&m)
                || !(-720..=840).contains(&total)
            {
                return Err(DecodeError::malformed(
                    WHAT,
                    11,
                    "time-zone offset out of range",
                ));
            }
            TimeZone::Offset(total)
        } else {
            TimeZone::Region((u16::from(z0 & 0x7F) << 6) | u16::from(z1 >> 2))
        };
        Self::decode_parts(raw.get(..11).unwrap_or_default(), Some(tz))
    }

    fn decode_parts(raw: &[u8], tz: Option<TimeZone>) -> DecodeResult<Self> {
        let utc = OraDate::decode(raw.get(..7).unwrap_or_default())?;
        let nanos = match raw.get(7..11) {
            Some(&[a, b, c, d]) => u32::from_be_bytes([a, b, c, d]),
            _ => 0,
        };
        if nanos > 999_999_999 {
            return Err(DecodeError::malformed(WHAT, 7, "fractional seconds >= 1s"));
        }
        Ok(Self { utc, nanos, tz })
    }
}

impl fmt::Display for OraTimestamp {
    /// `YYYY-MM-DD HH:MM:SS.fffffffff` with trailing zeros of the fraction
    /// removed (none if zero); zone as `+HH:MM` (UTC instant shown) or
    /// `region:<id>`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.utc)?;
        if self.nanos != 0 {
            let frac = format!("{:09}", self.nanos);
            write!(f, ".{}", frac.trim_end_matches('0'))?;
        }
        match self.tz {
            None => Ok(()),
            Some(TimeZone::Offset(m)) => {
                let sign = if m < 0 { '-' } else { '+' };
                write!(
                    f,
                    " UTC{sign}{:02}:{:02}",
                    m.unsigned_abs() / 60,
                    m.unsigned_abs() % 60
                )
            }
            Some(TimeZone::Region(r)) => write!(f, " UTC region:{r}"),
        }
    }
}
