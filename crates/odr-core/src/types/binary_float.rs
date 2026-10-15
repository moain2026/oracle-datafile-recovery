//! `BINARY_FLOAT` (type 100, 4 bytes) and `BINARY_DOUBLE` (type 101, 8 bytes).
//!
//! Oracle stores IEEE-754 bits **big-endian** with an order-preserving
//! transform (`lab-verify` LV-016): for non-negative values the sign bit is
//! set; for negative values all bits are inverted. Decoding reverses it:
//! if the top bit is set clear it, otherwise invert every bit.
//!
//! These are the only Oracle types where floating point is the *exact*
//! stored value, so returning `f32`/`f64` is lossless here (unlike `NUMBER`).

use crate::error::{DecodeError, DecodeResult};

/// Decode `BINARY_FLOAT`.
///
/// # Errors
/// [`DecodeError::Malformed`] if the length is not 4.
pub fn decode_binary_float(raw: &[u8]) -> DecodeResult<f32> {
    let arr: [u8; 4] = raw
        .try_into()
        .map_err(|_| DecodeError::malformed("BINARY_FLOAT", 0, "length is not 4"))?;
    let v = u32::from_be_bytes(arr);
    Ok(f32::from_bits(if v & 0x8000_0000 != 0 {
        v & 0x7FFF_FFFF
    } else {
        !v
    }))
}

/// Decode `BINARY_DOUBLE`.
///
/// # Errors
/// [`DecodeError::Malformed`] if the length is not 8.
pub fn decode_binary_double(raw: &[u8]) -> DecodeResult<f64> {
    let arr: [u8; 8] = raw
        .try_into()
        .map_err(|_| DecodeError::malformed("BINARY_DOUBLE", 0, "length is not 8"))?;
    let v = u64::from_be_bytes(arr);
    Ok(f64::from_bits(if v & 0x8000_0000_0000_0000 != 0 {
        v & 0x7FFF_FFFF_FFFF_FFFF
    } else {
        !v
    }))
}
