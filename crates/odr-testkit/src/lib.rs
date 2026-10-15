//! Test kit. **Owner: Astra** (`agent/astra`).
//!
//! Synthetic blocks are a *stand-in* until the Oracle lab (M0) produces real
//! golden files. Anything validated only against synthetic blocks is marked
//! `lab-verify` in code and must be re-validated on lab files.
// Test-only code: panicking on misuse is acceptable here.
#![allow(
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation
)]

// Re-export the builder's input type for core unit tests: the dev-dependency
// cycle compiles odr-core as both a library and a distinct unit-test crate.
pub use odr_core::ByteOrder;
use odr_core::block::{FLG_CHECKSUM, KCBH_LEN, TAIL_LEN, xor_fold};

/// Build a synthetic block with a valid header, tail and checksum.
pub fn synth_block(
    block_size: usize,
    order: ByteOrder,
    type_byte: u8,
    rdba: u32,
    scn_base: u32,
    seq: u8,
    fill_seed: u64,
) -> Vec<u8> {
    let mut b = vec![0u8; block_size];
    b[0] = type_byte;
    b[1] = 0xA2;
    put_u32(&mut b, 4, rdba, order);
    put_u32(&mut b, 8, scn_base, order);
    put_u16(&mut b, 12, 0, order);
    b[14] = seq;
    b[15] = FLG_CHECKSUM;
    let mut rng = SplitMix64(fill_seed);
    for x in &mut b[KCBH_LEN..block_size - TAIL_LEN] {
        *x = rng.next_u8();
    }
    let tail = ((scn_base & 0xFFFF) << 16) | (u32::from(type_byte) << 8) | u32::from(seq);
    put_u32(&mut b, block_size - TAIL_LEN, tail, order);
    // chkval = fold of everything else, so the whole block folds to 0.
    // `xor_fold` folds byte pairs; writing its result as the same byte pair
    // is correct for both byte orders (a word read in file order then equals
    // the file-order fold of the other words).
    let c = xor_fold(&b);
    b[16..18].copy_from_slice(&c.to_le_bytes());
    b
}

/// Build an image of `n` valid data blocks.
pub fn synth_image(n: usize, block_size: usize, order: ByteOrder, seed: u64) -> Vec<u8> {
    let mut img = Vec::with_capacity(n * block_size);
    for i in 0..n {
        let i32_ = u32::try_from(i).unwrap_or(u32::MAX);
        img.extend(synth_block(
            block_size,
            order,
            0x06,
            (1 << 22) | i32_,
            1000 + i32_,
            1,
            seed ^ i as u64,
        ));
    }
    img
}

/// Deterministic corruption patterns (ADR-0004).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corruption {
    /// Flip one random byte inside the block body.
    BitRot,
    /// Overwrite the tail check.
    TornWrite,
    /// Zero the whole block.
    Wipe,
    /// Fill with random bytes (ransomware-like).
    Encrypt,
}

/// Apply `kind` to block `idx` of `image`, deterministically from `seed`.
pub fn corrupt(image: &mut [u8], block_size: usize, idx: usize, kind: Corruption, seed: u64) {
    let blk = &mut image[idx * block_size..(idx + 1) * block_size];
    let mut rng = SplitMix64(seed);
    match kind {
        Corruption::BitRot => {
            let span = block_size - KCBH_LEN - TAIL_LEN;
            let pos = KCBH_LEN + (rng.next_u64() as usize % span);
            blk[pos] ^= 1 << (rng.next_u64() % 8);
        }
        Corruption::TornWrite => {
            for x in &mut blk[block_size - TAIL_LEN..] {
                *x = !*x;
            }
        }
        Corruption::Wipe => blk.fill(0),
        Corruption::Encrypt => blk.iter_mut().for_each(|x| *x = rng.next_u8()),
    }
}

fn put_u16(b: &mut [u8], off: usize, v: u16, o: ByteOrder) {
    let a = match o {
        ByteOrder::Little => v.to_le_bytes(),
        ByteOrder::Big => v.to_be_bytes(),
    };
    b[off..off + 2].copy_from_slice(&a);
}

fn put_u32(b: &mut [u8], off: usize, v: u32, o: ByteOrder) {
    let a = match o {
        ByteOrder::Little => v.to_le_bytes(),
        ByteOrder::Big => v.to_be_bytes(),
    };
    b[off..off + 4].copy_from_slice(&a);
}

/// Small deterministic PRNG (no dependency; identical on every platform).
#[derive(Debug, Clone)]
pub struct SplitMix64(pub u64);

impl SplitMix64 {
    /// Next 64-bit value.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Next byte.
    #[allow(clippy::cast_possible_truncation)]
    pub fn next_u8(&mut self) -> u8 {
        (self.next_u64() >> 56) as u8
    }
}
