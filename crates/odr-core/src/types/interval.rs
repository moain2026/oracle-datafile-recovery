//! `INTERVAL YEAR TO MONTH` (type 182, 5 bytes) and
//! `INTERVAL DAY TO SECOND` (type 183, 11 bytes).
//!
//! Every field is stored big-endian with an excess bias so the bytes sort
//! correctly (`lab-verify` LV-015):
//! * YM: `years + 0x8000_0000` (u32), `months + 60` (u8).
//! * DS: `days + 0x8000_0000` (u32), `hours + 60`, `minutes + 60`,
//!   `seconds + 60` (u8 each), `nanos + 0x8000_0000` (u32).
//!
//! All fields of one interval share the same sign.

use crate::error::{DecodeError, DecodeResult};
use std::fmt;

const BIAS32: i64 = 0x8000_0000;

fn excess32(b: [u8; 4]) -> i64 {
    i64::from(u32::from_be_bytes(b)) - BIAS32
}

/// `INTERVAL YEAR TO MONTH`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntervalYm {
    /// Years (signed).
    pub years: i32,
    /// Months −11..=11, same sign as `years`.
    pub months: i8,
}

impl IntervalYm {
    /// Decode 5 bytes.
    ///
    /// # Errors
    /// [`DecodeError::Malformed`] on a wrong length, range or mixed signs.
    pub fn decode(raw: &[u8]) -> DecodeResult<Self> {
        let &[y0, y1, y2, y3, mb] = raw else {
            return Err(DecodeError::malformed("INTERVAL YM", 0, "length is not 5"));
        };
        let years = i32::try_from(excess32([y0, y1, y2, y3]))
            .map_err(|_| DecodeError::malformed("INTERVAL YM", 0, "years out of range"))?;
        let months = i8::try_from(i16::from(mb) - 60)
            .map_err(|_| DecodeError::malformed("INTERVAL YM", 4, "months out of range"))?;
        if !(-11..=11).contains(&months) || (years > 0 && months < 0) || (years < 0 && months > 0) {
            return Err(DecodeError::malformed(
                "INTERVAL YM",
                4,
                "months out of range or sign mismatch",
            ));
        }
        Ok(Self { years, months })
    }
}

impl fmt::Display for IntervalYm {
    /// SQL literal form: `+YY-MM` / `-YY-MM`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let neg = self.years < 0 || self.months < 0;
        write!(
            f,
            "{}{}-{:02}",
            if neg { '-' } else { '+' },
            self.years.unsigned_abs(),
            self.months.unsigned_abs()
        )
    }
}

/// `INTERVAL DAY TO SECOND`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntervalDs {
    /// Days (signed).
    pub days: i32,
    /// Hours −23..=23.
    pub hours: i8,
    /// Minutes −59..=59.
    pub minutes: i8,
    /// Seconds −59..=59.
    pub seconds: i8,
    /// Nanoseconds −999 999 999..=999 999 999.
    pub nanos: i32,
}

impl IntervalDs {
    /// Decode 11 bytes.
    ///
    /// # Errors
    /// [`DecodeError::Malformed`] on a wrong length, range or mixed signs.
    pub fn decode(raw: &[u8]) -> DecodeResult<Self> {
        const W: &str = "INTERVAL DS";
        let &[d0, d1, d2, d3, h, mi, s, n0, n1, n2, n3] = raw else {
            return Err(DecodeError::malformed(W, 0, "length is not 11"));
        };
        let small = |v: u8, max: i16, off: usize| {
            let x = i16::from(v) - 60;
            if (-max..=max).contains(&x) {
                i8::try_from(x).map_err(|_| DecodeError::malformed(W, off, "field out of range"))
            } else {
                Err(DecodeError::malformed(W, off, "field out of range"))
            }
        };
        let days = i32::try_from(excess32([d0, d1, d2, d3]))
            .map_err(|_| DecodeError::malformed(W, 0, "days out of range"))?;
        let hours = small(h, 23, 4)?;
        let minutes = small(mi, 59, 5)?;
        let seconds = small(s, 59, 6)?;
        let nanos = i32::try_from(excess32([n0, n1, n2, n3]))
            .map_err(|_| DecodeError::malformed(W, 7, "nanos out of range"))?;
        if !(-999_999_999..=999_999_999).contains(&nanos) {
            return Err(DecodeError::malformed(W, 7, "nanos out of range"));
        }
        let signs = [
            i64::from(days),
            i64::from(hours),
            i64::from(minutes),
            i64::from(seconds),
            i64::from(nanos),
        ];
        if signs.iter().any(|&v| v > 0) && signs.iter().any(|&v| v < 0) {
            return Err(DecodeError::malformed(W, 0, "fields have mixed signs"));
        }
        Ok(Self {
            days,
            hours,
            minutes,
            seconds,
            nanos,
        })
    }
}

impl fmt::Display for IntervalDs {
    /// SQL literal form: `+D HH:MM:SS[.fffffffff]`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let neg = self.days < 0
            || self.hours < 0
            || self.minutes < 0
            || self.seconds < 0
            || self.nanos < 0;
        write!(
            f,
            "{}{} {:02}:{:02}:{:02}",
            if neg { '-' } else { '+' },
            self.days.unsigned_abs(),
            self.hours.unsigned_abs(),
            self.minutes.unsigned_abs(),
            self.seconds.unsigned_abs()
        )?;
        if self.nanos != 0 {
            let frac = format!("{:09}", self.nanos.unsigned_abs());
            write!(f, ".{}", frac.trim_end_matches('0'))?;
        }
        Ok(())
    }
}
