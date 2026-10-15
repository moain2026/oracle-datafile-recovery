//! Data-block layer (`ktbbh` → `kdbh` → `kdbt` → `kdbr`) and row pieces.
//! **Owner: Lead** (`lead/*`).
//!
//! Contract (stable — change only via ADR):
//! * [`RowPiece`] borrows from the block buffer (zero-copy).
//! * Column values are raw bytes; typed decoding is in [`crate::types`].
//! * Malformed directories/rows yield `Err` for that row only, never for the
//!   whole block.

use serde::{Deserialize, Serialize};

/// Row-piece flag bits (`flag` byte, \[O1\], \[W6\]).
pub mod flag {
    /// Cluster key.
    pub const K: u8 = 0x80;
    /// Cluster member.
    pub const C: u8 = 0x40;
    /// Head piece of row.
    pub const H: u8 = 0x20;
    /// Deleted.
    pub const D: u8 = 0x10;
    /// First data piece.
    pub const F: u8 = 0x08;
    /// Last data piece.
    pub const L: u8 = 0x04;
    /// First column continues from previous piece.
    pub const P: u8 = 0x02;
    /// Last column continues in next piece.
    pub const N: u8 = 0x01;
}

/// One row piece inside a data block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowPiece<'a> {
    /// Slot number in the row directory.
    pub slot: u16,
    /// Flag byte (see [`flag`]).
    pub flag: u8,
    /// ITL lock byte.
    pub lock: u8,
    /// Column values; `None` = NULL.
    pub columns: Vec<Option<&'a [u8]>>,
    /// Next-piece pointer (rdba, slot) when the row continues elsewhere.
    pub next: Option<(u32, u16)>,
}

/// Provenance of an extracted row (REQ-003.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Provenance {
    /// Index of the evidence file in the case.
    pub file: u32,
    /// Block RDBA.
    pub rdba: u32,
    /// Row directory slot.
    pub slot: u16,
    /// Block SCN.
    pub scn: u64,
}
