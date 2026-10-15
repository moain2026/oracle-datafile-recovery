//! `ROWID` (type 69).
//!
//! Extended ROWID, 10 bytes big-endian (`lab-verify` LV-017):
//! `data_object_id` u32 · `rdba` u32 (relative file# 10 bits | block# 22 bits) · `slot` u16.
//! Text form is Oracle's base-64 `OOOOOOFFFBBBBBBRRR`
//! (alphabet `A–Z a–z 0–9 + /`): 6 chars object, 3 file, 6 block, 3 row.
//!
//! Restricted ROWID (Oracle 7, 6 bytes: block u32, slot u16) is accepted and
//! printed as `BBBBBBBB.RRRR.FFFF` hex (file unknown → shown from rdba).

use crate::block::Rdba;
use crate::error::{DecodeError, DecodeResult};
use std::fmt;

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// A decoded ROWID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OraRowid {
    /// Extended (8i+) ROWID.
    Extended {
        /// Data object id.
        objd: u32,
        /// Relative data block address.
        rdba: Rdba,
        /// Row slot.
        slot: u16,
    },
    /// Restricted (Oracle 7 format) ROWID.
    Restricted {
        /// Relative data block address.
        rdba: Rdba,
        /// Row slot.
        slot: u16,
    },
}

impl OraRowid {
    /// Decode 10 (extended) or 6 (restricted) bytes.
    ///
    /// # Errors
    /// [`DecodeError::Malformed`] for any other length.
    pub fn decode(raw: &[u8]) -> DecodeResult<Self> {
        match *raw {
            [o0, o1, o2, o3, r0, r1, r2, r3, s0, s1] => Ok(Self::Extended {
                objd: u32::from_be_bytes([o0, o1, o2, o3]),
                rdba: Rdba(u32::from_be_bytes([r0, r1, r2, r3])),
                slot: u16::from_be_bytes([s0, s1]),
            }),
            [r0, r1, r2, r3, s0, s1] => Ok(Self::Restricted {
                rdba: Rdba(u32::from_be_bytes([r0, r1, r2, r3])),
                slot: u16::from_be_bytes([s0, s1]),
            }),
            _ => Err(DecodeError::malformed("ROWID", 0, "length is not 6 or 10")),
        }
    }
}

fn b64(f: &mut fmt::Formatter<'_>, v: u32, digits: u32) -> fmt::Result {
    for i in (0..digits).rev() {
        let idx = (v >> (6 * i)) & 0x3F;
        let c = B64
            .get(usize::try_from(idx).unwrap_or(0))
            .copied()
            .unwrap_or(b'A');
        write!(f, "{}", char::from(c))?;
    }
    Ok(())
}

impl fmt::Display for OraRowid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Extended { objd, rdba, slot } => {
                b64(f, objd, 6)?;
                b64(f, rdba.file(), 3)?;
                b64(f, rdba.block(), 6)?;
                b64(f, u32::from(slot), 3)
            }
            Self::Restricted { rdba, slot } => {
                write!(f, "{:08X}.{:04X}.{:04X}", rdba.block(), slot, rdba.file())
            }
        }
    }
}
