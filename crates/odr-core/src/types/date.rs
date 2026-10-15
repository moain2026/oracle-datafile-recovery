//! Oracle `DATE` (internal type 12): 7 bytes.
//!
//! `[century+100, year_of_century+100, month, day, hour+1, minute+1, second+1]`.
//! Years before 1 AD have century/year bytes below 100 (BC dates).
//! Oracle uses the Julian calendar before 1582-10-15 and the Gregorian after;
//! 1582-10-05..=1582-10-14 do not exist.

// Arithmetic is on `u8`-derived `i16` values (<= 255), far below overflow.
#![allow(clippy::arithmetic_side_effects)]

use crate::error::{DecodeError, DecodeResult};
use std::fmt;

const WHAT: &str = "DATE";

/// A decoded Oracle `DATE`. `year` is astronomical-free Oracle style:
/// negative for BC (−4712..=−1), positive for AD (1..=9999); there is no year 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OraDate {
    /// Year (−4712..=9999, never 0).
    pub year: i16,
    /// Month 1..=12.
    pub month: u8,
    /// Day 1..=31 (validated against the month).
    pub day: u8,
    /// Hour 0..=23.
    pub hour: u8,
    /// Minute 0..=59.
    pub minute: u8,
    /// Second 0..=59.
    pub second: u8,
}

impl OraDate {
    /// Decode the 7 raw bytes (TIMESTAMP decoders reuse this for bytes 0..7).
    ///
    /// # Errors
    /// [`DecodeError::Malformed`] if the length or any field is invalid.
    pub fn decode(raw: &[u8]) -> DecodeResult<Self> {
        let &[c, y, month, day, h, mi, s] = raw else {
            return Err(DecodeError::malformed(WHAT, 0, "length is not 7"));
        };
        let year = (i16::from(c) - 100) * 100 + (i16::from(y) - 100);
        if year == 0 || !(-4712..=9999).contains(&year) {
            return Err(DecodeError::malformed(WHAT, 0, "year out of range"));
        }
        if !(1..=12).contains(&month) {
            return Err(DecodeError::malformed(WHAT, 2, "month out of range"));
        }
        if day == 0 || day > days_in_month(year, month) {
            return Err(DecodeError::malformed(WHAT, 3, "day out of range"));
        }
        if year == 1582 && month == 10 && (5..=14).contains(&day) {
            return Err(DecodeError::malformed(
                WHAT,
                3,
                "date removed by Gregorian reform",
            ));
        }
        let (hour, minute, second) = (h.wrapping_sub(1), mi.wrapping_sub(1), s.wrapping_sub(1));
        if hour > 23 || minute > 59 || second > 59 {
            return Err(DecodeError::malformed(WHAT, 4, "time out of range"));
        }
        Ok(Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        })
    }
}

/// Days in month for Oracle's hybrid Julian/Gregorian calendar.
fn days_in_month(year: i16, month: u8) -> u8 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 31,
    }
}

fn is_leap(year: i16) -> bool {
    // BC years: Oracle's 1 BC is a leap year (astronomical year 0).
    let y = i32::from(if year < 0 { year + 1 } else { year });
    if year < 1582 {
        y.rem_euclid(4) == 0
    } else {
        (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
    }
}

impl fmt::Display for OraDate {
    /// ISO-8601-like `YYYY-MM-DD HH:MM:SS`; BC years as `-YYYY`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.year < 0 { "-" } else { "" };
        write!(
            f,
            "{sign}{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year.unsigned_abs(),
            self.month,
            self.day,
            self.hour,
            self.minute,
            self.second
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_known() {
        let d = OraDate::decode(&[120, 126, 10, 15, 3, 9, 1]).map(|d| d.to_string());
        assert_eq!(d.as_deref(), Ok("2026-10-15 02:08:00"));
        let d = OraDate::decode(&[120, 100, 1, 1, 1, 1, 1]).map(|d| d.to_string());
        assert_eq!(d.as_deref(), Ok("2000-01-01 00:00:00"));
        // DUMP(DATE '-4712-01-01') = 53,88,1,1,1,1,1
        let d = OraDate::decode(&[53, 88, 1, 1, 1, 1, 1]).map(|d| d.to_string());
        assert_eq!(d.as_deref(), Ok("-4712-01-01 00:00:00"));
    }

    #[test]
    fn validates_calendar() {
        assert!(OraDate::decode(&[120, 123, 2, 29, 1, 1, 1]).is_err()); // 2023-02-29
        assert!(OraDate::decode(&[120, 124, 2, 29, 1, 1, 1]).is_ok()); // 2024-02-29
        assert!(OraDate::decode(&[119, 100, 2, 29, 1, 1, 1]).is_err()); // 1900 not leap
        assert!(OraDate::decode(&[115, 182, 10, 10, 1, 1, 1]).is_err()); // 1582-10-10
        assert!(OraDate::decode(&[120, 126, 13, 1, 1, 1, 1]).is_err());
        assert!(OraDate::decode(&[120, 126, 1, 1, 25, 1, 1]).is_err());
        assert!(OraDate::decode(&[120, 126, 1, 1, 0, 1, 1]).is_err()); // hour byte 0
        assert!(OraDate::decode(&[120, 126, 1]).is_err());
    }
}
