//! Whole-row assembly across row pieces (REQ-003.2).
//!
//! A row can be split into pieces in different blocks:
//! * **chained** (row larger than a block): `H F` head with `nrid` → middle
//!   pieces → last piece with `L`;
//! * **migrated** (row moved by an update): an `H` head with no columns whose
//!   `nrid` points at an `F L` piece (with `hrid` back to the head);
//! * a **column split** between pieces: the earlier piece has flag `N`, the
//!   next has flag `P`; their boundary column bytes are concatenated.
//!
//! Safety properties (all tested):
//! * the walk is bounded by [`MAX_PIECES`] and detects cycles, so corrupt
//!   `nrid` pointers can never loop forever;
//! * a missing or unreadable piece yields an error naming its address: never
//!   a row with silently missing columns;
//! * flag sequences that Oracle would not write are rejected.

use super::{RowPiece, flag};
use crate::error::{DecodeError, DecodeResult};
use std::collections::HashSet;

/// Upper bound on pieces per row. Oracle allows at most 1000 columns and each
/// piece holds at least one column or column fragment, so a legitimate chain
/// is far shorter than this; anything longer is corruption.
pub const MAX_PIECES: usize = 4096;

/// Supplies row pieces by address. Implemented by the caller (block cache,
/// scanner index…); `odr-core::row` itself performs no I/O.
pub trait PieceSource {
    /// The piece at `(rdba, slot)`: `None` if the block or slot is not
    /// available, `Some(Err)` if it exists but does not decode.
    fn piece(&self, rdba: u32, slot: u16) -> Option<DecodeResult<RowPiece<'_>>>;
}

/// Why a row could not be assembled.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AssembleError {
    /// The starting piece is not a row head (flag `H` missing).
    NotHead,
    /// The row is marked deleted (caller decides whether to recover it).
    Deleted,
    /// A referenced piece is not available.
    MissingPiece {
        /// Block address.
        rdba: u32,
        /// Slot.
        slot: u16,
    },
    /// A referenced piece exists but failed to decode.
    BadPiece {
        /// Block address.
        rdba: u32,
        /// Slot.
        slot: u16,
        /// Decoder error.
        error: DecodeError,
    },
    /// The `nrid` chain revisits a piece.
    Cycle {
        /// Block address of the repeated piece.
        rdba: u32,
        /// Slot of the repeated piece.
        slot: u16,
    },
    /// More than [`MAX_PIECES`] pieces.
    TooLong,
    /// Flags inconsistent with the piece's position in the chain.
    BadFlags {
        /// Index of the piece in the chain.
        index: usize,
        /// Reason.
        reason: &'static str,
    },
}

/// A fully assembled row: owned column bytes in table column order (trailing
/// NULL columns are not present; the caller pads to the table's width).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssembledRow {
    /// Column values; `None` = NULL.
    pub columns: Vec<Option<Vec<u8>>>,
    /// Addresses of every piece, head first.
    pub pieces: Vec<(u32, u16)>,
}

/// Assemble the row whose head piece is `head` at `(rdba, slot)`.
///
/// # Errors
/// See [`AssembleError`].
pub fn assemble<S: PieceSource + ?Sized>(
    src: &S,
    rdba: u32,
    head: &RowPiece<'_>,
) -> Result<AssembledRow, AssembleError> {
    if !head.is_head() {
        return Err(AssembleError::NotHead);
    }
    if head.is_deleted() {
        return Err(AssembleError::Deleted);
    }
    let mut out = AssembledRow {
        columns: Vec::new(),
        pieces: vec![(rdba, head.slot)],
    };
    let mut seen: HashSet<(u32, u16)> = HashSet::from([(rdba, head.slot)]);
    let mut continues = false; // previous piece had flag N
    append(&mut out.columns, head, continues, 0)?;
    continues = head.flag & flag::N != 0;
    let mut next = head.next;
    while let Some((nrdba, nslot)) = next {
        if out.pieces.len() >= MAX_PIECES {
            return Err(AssembleError::TooLong);
        }
        if !seen.insert((nrdba, nslot)) {
            return Err(AssembleError::Cycle {
                rdba: nrdba,
                slot: nslot,
            });
        }
        let piece = match src.piece(nrdba, nslot) {
            None => {
                return Err(AssembleError::MissingPiece {
                    rdba: nrdba,
                    slot: nslot,
                });
            }
            Some(Err(error)) => {
                return Err(AssembleError::BadPiece {
                    rdba: nrdba,
                    slot: nslot,
                    error,
                });
            }
            Some(Ok(p)) => p,
        };
        let index = out.pieces.len();
        if piece.is_head() {
            return Err(AssembleError::BadFlags {
                index,
                reason: "continuation piece has H flag",
            });
        }
        append(&mut out.columns, &piece, continues, index)?;
        out.pieces.push((nrdba, nslot));
        continues = piece.flag & flag::N != 0;
        next = piece.next;
    }
    if continues {
        return Err(AssembleError::BadFlags {
            index: out.pieces.len().saturating_sub(1),
            reason: "last piece says its column continues (N) but there is no next piece",
        });
    }
    Ok(out)
}

/// Append a piece's columns, merging its first column into the previous
/// piece's last column when the split flags say so.
fn append(
    cols: &mut Vec<Option<Vec<u8>>>,
    piece: &RowPiece<'_>,
    prev_continues: bool,
    index: usize,
) -> Result<(), AssembleError> {
    let this_starts_with_tail = piece.flag & flag::P != 0;
    if prev_continues != this_starts_with_tail {
        return Err(AssembleError::BadFlags {
            index,
            reason: "N/P column-split flags do not match",
        });
    }
    let mut it = piece.columns.iter();
    if this_starts_with_tail {
        let first = it.next().ok_or(AssembleError::BadFlags {
            index,
            reason: "P flag on piece with no columns",
        })?;
        match (cols.last_mut(), first) {
            (Some(Some(prev)), Some(part)) => prev.extend_from_slice(part),
            _ => {
                return Err(AssembleError::BadFlags {
                    index,
                    reason: "split column is NULL on one side",
                });
            }
        }
    }
    cols.extend(it.map(|c| c.map(<[u8]>::to_vec)));
    Ok(())
}
