//! Integration: scanner classification on synthetic images, 5 seeds, both byte orders.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use odr_core::ByteOrder;
use odr_core::block::BlockStatus;
use odr_core::scan::scan_image;
use odr_testkit::{Corruption, corrupt, synth_image};

#[test]
fn classifies_every_corruption_pattern() {
    for order in [ByteOrder::Little, ByteOrder::Big] {
        for seed in 1..=5u64 {
            let bs = 8192;
            let mut img = synth_image(64, bs, order, seed);
            corrupt(&mut img, bs, 3, Corruption::BitRot, seed);
            corrupt(&mut img, bs, 7, Corruption::TornWrite, seed);
            corrupt(&mut img, bs, 11, Corruption::Wipe, seed);
            corrupt(&mut img, bs, 13, Corruption::Encrypt, seed);
            img.truncate(img.len() - 100); // truncated last block
            let r = scan_image(&img, bs, order);
            assert_eq!(r.blocks.len(), 64);
            assert_eq!(
                r.blocks[3].status,
                BlockStatus::ChecksumFail,
                "{order:?} seed {seed}"
            );
            assert_eq!(r.blocks[7].status, BlockStatus::TailMismatch);
            assert_eq!(r.blocks[11].status, BlockStatus::Zeroed);
            assert_ne!(r.blocks[13].status, BlockStatus::Valid);
            assert_eq!(r.blocks[63].status, BlockStatus::Truncated);
            assert_eq!(r.count(BlockStatus::Valid), 59);
        }
    }
}

#[test]
fn wrong_byte_order_is_never_valid() {
    let img = synth_image(8, 8192, ByteOrder::Little, 42);
    let r = scan_image(&img, 8192, ByteOrder::Big);
    assert_eq!(r.count(BlockStatus::Valid), 0);
}
