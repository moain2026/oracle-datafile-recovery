//! Indexed-cluster row pieces (REQ-002 prerequisite: `TAB$`, `COL$`, `USER$`,
//! `TS$`… live in clusters `C_OBJ#`, `C_USER#`, `C_TS#`).
//!
//! A cluster block holds one *key table* (`kdbt[0]`) and one directory entry
//! per member table (`kdbt[1..]`, in `TAB$.TAB#` order). Layouts after the
//! common `flag lock cc` bytes (`lab-verify` LV-020/021):
//!
//! ```text
//! key row    (flag K, e.g. 0xAC "K-H-FL--"):
//!     curc u16 · comc u16 · prev rowid (dba u32, slot u16) · next rowid (6) · key columns
//! member row (flag C, e.g. 0x6C "-CH-FL--"):
//!     cki u8 · non-key columns
//! ```
//!
//! * `curc`/`comc`: current / committed member-row count for this key in this
//!   block; `prev`/`next`: the same key's previous/next block (they point at
//!   the row itself when the key fits in one block). Multi-byte fields use the
//!   file byte order (a published BBED edit of the low byte of `comc` on a
//!   little-endian file changed its value).
//! * `cki` is the index of the member's key row **within `kdbt[0]`**.
//! * Member rows do **not** store the cluster-key columns. The segment column
//!   order is *key columns first, then the member's own columns*; mapping back
//!   to `COL#` is `COL$.SEGCOL#`'s job (LV-021).
//!
//! Chained cluster pieces and cluster rows in `ROWDEPENDENCIES` blocks have
//! no published byte layout yet; [`super::piece::parse_piece`] reports them as
//! [`crate::DecodeError::Unsupported`] instead of guessing.
//!
//! Sources (independent, all real dumps): Hui Xingxing, *深入解析Oracle table
//! cluster结构* (itpub 2020, BBED of `C_USER#` file 1 block 209); 惜分飞-style
//! cluster study on zhizuobiao (2018, BBED offsets + deleted member `0x7C`);
//! chinaunix *Clustering table 存储结构深入分析* (dept/emp block dump).

use super::RowPiece;
use crate::error::{DecodeError, DecodeResult};
use serde::{Deserialize, Serialize};

/// Cluster-specific header of a row piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClusterPart {
    /// Cluster-key row (flag `K`).
    Key {
        /// Current member-row count for this key in this block (`curc`).
        current: u16,
        /// Committed member-row count for this key in this block (`comc`).
        committed: u16,
        /// Previous block holding this key (`pk`: rdba, slot).
        prev: (u32, u16),
        /// Next block holding this key (`nk`: rdba, slot).
        next: (u32, u16),
    },
    /// Cluster member row (flag `C`).
    Member {
        /// Index of the key row inside `kdbt[0]` (`cki`).
        key_index: u8,
    },
}

/// Segment-order columns of a clustered row: the key row's columns padded with
/// NULL to `clucols` (`TAB$.CLUCOLS`), followed by the member's columns.
///
/// Padding matters: trailing NULL columns are never stored, so a key row with
/// a NULL last key column has fewer than `clucols` columns, and concatenating
/// without padding would shift every member column by one.
///
/// # Errors
/// [`DecodeError::Malformed`] if `key` is not a key row, `member` is not a
/// member row, or the key row stores more than `clucols` columns.
pub fn cluster_columns<'a>(
    key: &RowPiece<'a>,
    member: &RowPiece<'a>,
    clucols: usize,
) -> DecodeResult<Vec<Option<&'a [u8]>>> {
    if !matches!(key.cluster, Some(ClusterPart::Key { .. })) {
        return Err(DecodeError::malformed(
            "cluster row",
            0,
            "key piece is not a cluster-key row",
        ));
    }
    if !matches!(member.cluster, Some(ClusterPart::Member { .. })) {
        return Err(DecodeError::malformed(
            "cluster row",
            0,
            "member piece is not a cluster member row",
        ));
    }
    if key.columns.len() > clucols {
        return Err(DecodeError::malformed(
            "cluster row",
            0,
            "key row stores more columns than CLUCOLS",
        ));
    }
    let mut out = Vec::with_capacity(clucols.saturating_add(member.columns.len()));
    out.extend(key.columns.iter().copied());
    out.resize(clucols, None);
    out.extend(member.columns.iter().copied());
    Ok(out)
}
