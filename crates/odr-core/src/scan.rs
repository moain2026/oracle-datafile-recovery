//! Block scanner (REQ-001). **Owner: Astra** (`agent/astra`).
//!
//! Contract (stable — change only via ADR):
//! * [`ScanReport`] is the output; one [`BlockRecord`] per block, in file order.
//! * The scanner never aborts on bad bytes; every block gets a status.
//! * Performance target: ≥ 300 MB/s/core, linear in cores (bench in `benches/`).
//!
//! Current state: sequential reference implementation. Astra replaces the
//! internals (parallel chunks, header probe, block-size/endian inference)
//! without changing the public types.

use crate::block::{BlockHeader, BlockStatus, classify};
use crate::bytes::ByteOrder;
use serde::{Deserialize, Serialize};

/// One block's scan result (kept small: the index for a 1 TB file is in RAM).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockRecord {
    /// Block number within the file (offset / block size).
    pub index: u64,
    /// Classification.
    pub status: BlockStatus,
    /// Raw block type byte (if a header could be read).
    pub type_byte: Option<u8>,
    /// SCN of the block (if a header could be read).
    pub scn: Option<u64>,
}

/// Scan output.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanReport {
    /// Block size used.
    pub block_size: usize,
    /// Per-block records.
    pub blocks: Vec<BlockRecord>,
}

impl ScanReport {
    /// Count blocks with a given status.
    pub fn count(&self, status: BlockStatus) -> usize {
        self.blocks.iter().filter(|b| b.status == status).count()
    }
}

/// Scan an in-memory image. Reference implementation (sequential).
pub fn scan_image(image: &[u8], block_size: usize, order: ByteOrder) -> ScanReport {
    let blocks = image
        .chunks(block_size.max(1))
        .zip(0u64..)
        .map(|(blk, index)| {
            let status = classify(blk, block_size, order);
            let h = BlockHeader::parse(blk, order).ok();
            BlockRecord {
                index,
                status,
                type_byte: h.map(|h| h.type_byte),
                scn: h.map(|h| h.scn.0),
            }
        })
        .collect();
    ScanReport { block_size, blocks }
}
