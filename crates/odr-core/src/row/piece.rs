//! Row-piece decoding.
//!
//! A row piece is (all lengths in bytes):
//!
//! ```text
//! flag(1) lock(1) cc(1) [nrid: dba(4) slot(2)] [hrid: dba(4) slot(2)] col_1 … col_cc
//! ```
//!
//! * `nrid` is present when the piece is **not** the last one (flag `L` clear)
//!   and points at the next piece (chained / migrated rows) — LV-008.
//! * `hrid` is present on the destination piece of a migrated row: flag `F`
//!   set, `H` clear — LV-010.
//! * Column length byte: `0..=250` = length, `0xFE` = next two bytes hold the
//!   length (big-endian on every platform, LV-009), `0xFF` = NULL;
//!   `0xFB..=0xFD` never occur and are rejected.
//! * Trailing NULL columns are not stored: `cc` counts stored columns only.
//! * Cluster pieces (`K`/`C` flags) carry extra header bytes and are reported
//!   as [`DecodeError::Unsupported`] until confirmed on lab files.
//!
//! Sources: Oracle Concepts \[O1\]; Lewis \[W6\]; Widlake \[W7\]; oradul \[G1\].

use super::{RowPiece, flag};
use crate::bytes::{ByteOrder, Cursor, read_u8};
use crate::error::{DecodeError, DecodeResult};

const WHAT: &str = "row piece";

/// Parse the row piece starting at `off` in `heap` (block bytes up to, not
/// including, the tail check — a piece can never extend into it).
///
/// # Errors
/// [`DecodeError`] if the piece is malformed or runs past the heap.
pub fn parse_piece(
    heap: &[u8],
    off: usize,
    slot: u16,
    order: ByteOrder,
) -> DecodeResult<RowPiece<'_>> {
    let mut c = Cursor::new(heap, order);
    c.seek(off)?;
    let fl = c.u8()?;
    let lock = c.u8()?;
    if fl & (flag::K | flag::C) != 0 {
        return Err(DecodeError::Unsupported("cluster row piece"));
    }
    let cc = c.u8()?;
    let rid = |c: &mut Cursor<'_>| -> DecodeResult<(u32, u16)> { Ok((c.u32()?, c.u16()?)) };
    let next_rid = if fl & flag::L == 0 {
        Some(rid(&mut c)?)
    } else {
        None
    };
    let head_rid = if fl & flag::F != 0 && fl & flag::H == 0 {
        Some(rid(&mut c)?)
    } else {
        None
    };
    let mut columns = Vec::with_capacity(usize::from(cc));
    for _ in 0..cc {
        let pos = c.position();
        let len = match read_u8(heap, pos)? {
            0xFF => {
                c.skip(1)?;
                columns.push(None);
                continue;
            }
            0xFE => {
                c.skip(1)?;
                let hi = c.u8()?;
                let lo = c.u8()?;
                usize::from(u16::from_be_bytes([hi, lo]))
            }
            l @ 0..=250 => {
                c.skip(1)?;
                usize::from(l)
            }
            _ => {
                return Err(DecodeError::malformed(
                    WHAT,
                    pos,
                    "reserved length byte 0xFB..=0xFD",
                ));
            }
        };
        columns.push(Some(c.take(len)?));
    }
    let len = c.position().saturating_sub(off);
    Ok(RowPiece {
        slot,
        flag: fl,
        lock,
        columns,
        next: next_rid,
        head: head_rid,
        len,
    })
}
