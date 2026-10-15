//! Conservative smallfile block-size/byte-order inference (S1, REQ-001).
//!
//! # Contract and evidence
//! [`infer_image`] borrows arbitrary bytes; [`infer_evidence`] reads an already
//! opened read-only evidence handle. Neither changes bytes nor decodes rows.
//! Both return all ten scores and an explicit decision, including for empty or
//! truncated images. This is NOT a `kcvfh` parser and does not support bigfile
//! RDBAs. There is no claim that an accepted layout proves Oracle provenance.
//!
//! Binary semantics are inherited, not independently established: \[T3\]
//! Stawiarski (2018), as cited in `block.rs`, for format/checksum and tail
//! (`lab-verify`, LV-002); \[T4\] smallfile RDBA versus physical block index
//! (`lab-verify`, tracking issue #6). No new binary offsets are introduced.
//! Block sizes come from the frozen [`BLOCK_SIZES`] contract. All sampling and
//! confidence constants below are S1 product policy, not Oracle format facts.
//!
//! # Sampling, scores, and refusal
//! For F <= 1 GiB all complete blocks are sampled. Otherwise, exactly 4096
//! uniformly spaced complete-block indices include both endpoints. Seed 0
//! denotes a deterministic non-random endpoint grid. Partial blocks contribute
//! no score. Each score is the largest equal-relative-file-number cohort of
//! valid blocks whose RDBA block number equals their physical index. Acceptance
//! additionally requires ONE file-number cohort, >= max(16, ceil(n/100)) hits,
//! and best/(best+runner-up) >= 99/100. Cross-products use u128, not floats.
//! Mixed-file populations, ties, and insufficient evidence remain ambiguous.
//!
//! # Probability limitations
//! \[T4\] Under independent uniformly random bytes, conditioned on the checksum
//! flag being set, format + tail + checksum pass with probability approximately
//! 2^-4 * 2^-32 * 2^-16 = 2^-52. Without that flag the checksum factor is absent.
//! This is NOT a bound for adversarial data or a posterior confidence estimate.
//! For a genuine B-sized block considered as two B/2 blocks, the first half
//! generally lacks its tail and physical index j becomes 2j (except j=0);
//! the second half generally has no header. Conversely, concatenation typically
//! mismatches tail and index. These separate hypotheses on ordinary images,
//! not on deliberately crafted ones. No cryptographic authenticity is implied.
//!
//! # Complexity and invariants
//! With `n_h` sampled blocks of size `B_h`, inference costs `O(sum(n_h * B_h))`
//! time and `O(max(B_h) + R + 10)` auxiliary space, R <= 1024 from the frozen
//! smallfile RDBA contract. No F-sized allocation occurs. T=1 (no threads).
//! Evidence verification adds O(F) hashing time and the existing Evidence hash
//! buffer; opening Evidence already hashes F bytes separately. A stable source
//! is a precondition: post-read verification detects ordinary changes but does
//! not prove snapshot isolation against a writer restoring old bytes on Unix.

use crate::block::{BLOCK_SIZES, BlockHeader, BlockStatus, classify};
use crate::bytes::{ByteOrder, slice_at};
use crate::error::{DecodeError, OdrError};
use crate::source::Evidence;
use std::collections::BTreeMap;
use std::io::{Error, ErrorKind};

/// S1 policy: exhaustive sampling through one GiB (inclusive).
pub const EXHAUSTIVE_LIMIT: u64 = 1 << 30;
/// S1 policy: large-file endpoint-grid sample count per hypothesis.
pub const STRATIFIED_SAMPLES: u64 = 4096;
/// S1 policy: grid sampling is non-random; this fixed seed identifies it.
pub const SAMPLE_SEED: u64 = 0;

/// One of the ten hypotheses, with unnormalized counts (not probabilities).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// Legal block size from the frozen block contract.
    pub block_size: usize,
    /// Candidate byte order.
    pub order: ByteOrder,
    /// Number of complete blocks sampled, including failed checks.
    pub sampled: u64,
    /// Size of the largest constant-file-number valid cohort.
    pub score: u64,
    /// Number of distinct file numbers among otherwise qualifying blocks.
    pub file_cohorts: usize,
    /// Unique file number, only when exactly one cohort exists.
    pub relative_file: Option<u32>,
}

/// Inference decision. Ambiguity is an ordinary result, never an I/O error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Product-policy tests passed; Oracle correctness is still lab-unverified.
    Accepted {
        /// Inferred legal block size.
        block_size: usize,
        /// Inferred byte order.
        order: ByteOrder,
        /// Consistent smallfile relative file number.
        relative_file: u32,
    },
    /// Insufficient, competing, or inconsistent evidence; do not guess.
    Ambiguous,
}

/// Reproducible inference output, retaining losing hypotheses for audit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeReport {
    /// File length at the beginning of inference.
    pub image_len: u64,
    /// Fixed grid seed (no PRNG is used).
    pub sample_seed: u64,
    /// Descending score; ties retain `BLOCK_SIZES` order, Little before Big.
    pub candidates: Vec<Candidate>,
    /// Explicit acceptance or refusal.
    pub decision: Decision,
}

/// Infer a layout from any byte slice without copying the whole image.
///
/// Preconditions: none. Partial final blocks are ignored, never padded.
/// Postconditions: ten candidates, deterministic order, explicit decision.
/// Time `O(sum(n_h * B_h))`, space `O(max(B_h) + R)`, T=1; see module contract.
///
/// # Errors
/// Returns an error if length/offset conversion or checked arithmetic fails.
/// These conditions are unreachable for addressable slices and legal sizes,
/// but are checked rather than relying on target pointer width.
pub fn infer_image(image: &[u8]) -> Result<ProbeReport, OdrError> {
    let len = u64::try_from(image.len()).map_err(|_| arithmetic_error())?;
    infer_with(len, |offset, buf| {
        let offset = usize::try_from(offset).map_err(|_| arithmetic_error())?;
        let source = slice_at(image, offset, buf.len())?;
        buf.copy_from_slice(source);
        Ok(())
    })
}

/// Infer from read-only evidence with bounded reads and post-read verification.
///
/// Preconditions: a stable file; the handle was opened using [`Evidence::open`].
/// Postconditions on success: same report as [`infer_image`] for those bytes, and
/// `verify_unchanged` returned true. No write access or whole-file load occurs.
/// Time `O(F + sum(n_h * B_h))`, space `O(max(B_h) + R)` plus Evidence's hash buffer.
/// T=1. The pre-existing hash pass at open is not included in this call's cost.
///
/// # Errors
/// Propagates read/hash I/O errors; `UnexpectedEof` for a sampled short read;
/// `InvalidData` if verification detects a change; checked conversion/arithmetic
/// errors as in [`infer_image`]. An I/O failure never becomes a low candidate score.
pub fn infer_evidence(evidence: &mut Evidence) -> Result<ProbeReport, OdrError> {
    let report = infer_with(evidence.len(), |offset, buf| {
        if evidence.read_at(offset, buf)? != buf.len() {
            return Err(Error::new(ErrorKind::UnexpectedEof, "short header-probe read").into());
        }
        Ok(())
    })?;
    if !evidence.verify_unchanged()? {
        return Err(Error::new(ErrorKind::InvalidData, "evidence changed during probe").into());
    }
    Ok(report)
}

fn arithmetic_error() -> OdrError {
    DecodeError::Unsupported("header probe offset cannot be represented").into()
}

// For n=0 no caller requests an offset. For n=1 only block zero is sampled.
// Otherwise 0 <= i < n <= full, so floor(i*(full-1)/(n-1)) < full.
// Products of two u64 fit u128; quotient*B < full*B <= F <= u64::MAX.
fn sample_offset(full: u64, n: u64, i: u64, size: u64) -> Result<u64, OdrError> {
    let index = if n <= 1 {
        0
    } else {
        u64::try_from(
            u128::from(i)
                .checked_mul(u128::from(full.saturating_sub(1)))
                .and_then(|v| v.checked_div(u128::from(n.saturating_sub(1))))
                .ok_or_else(arithmetic_error)?,
        )
        .map_err(|_| arithmetic_error())?
    };
    index.checked_mul(size).ok_or_else(arithmetic_error)
}

fn infer_with(
    len: u64,
    mut read: impl FnMut(u64, &mut [u8]) -> Result<(), OdrError>,
) -> Result<ProbeReport, OdrError> {
    let mut candidates = Vec::new();
    // Five legal sizes, two orders: both loops are finite. Only one block
    // allocation is live at any instant; every read fills exactly that block.
    for size in BLOCK_SIZES {
        let size64 = u64::try_from(size).map_err(|_| arithmetic_error())?;
        let full = len.checked_div(size64).ok_or_else(arithmetic_error)?;
        let n = if len <= EXHAUSTIVE_LIMIT {
            full
        } else {
            full.min(STRATIFIED_SAMPLES)
        };
        let mut block = vec![0; size];
        for order in [ByteOrder::Little, ByteOrder::Big] {
            let mut cohorts = BTreeMap::<u32, u64>::new();
            // n <= 524288 for F <= 1 GiB, otherwise n <= 4096. This finite
            // range terminates even for len=u64::MAX; no offset is truncated.
            for i in 0..n {
                let offset = sample_offset(full, n, i, size64)?;
                read(offset, &mut block)?;
                if classify(&block, size, order) != BlockStatus::Valid {
                    continue;
                }
                let header = BlockHeader::parse(&block, order)?;
                if u64::from(header.rdba.block())
                    != offset.checked_div(size64).ok_or_else(arithmetic_error)?
                {
                    continue;
                }
                let count = cohorts.entry(header.rdba.file()).or_default();
                // Each sample increments at most once: count <= n <= 524288.
                *count = count.checked_add(1).ok_or_else(arithmetic_error)?;
            }
            candidates.push(Candidate {
                block_size: size,
                order,
                sampled: n,
                score: cohorts.values().copied().max().unwrap_or(0),
                file_cohorts: cohorts.len(),
                relative_file: if cohorts.len() == 1 {
                    cohorts.keys().next().copied()
                } else {
                    None
                },
            });
        }
    }
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.score));
    let decision = decide(&candidates);
    Ok(ProbeReport {
        image_len: len,
        sample_seed: SAMPLE_SEED,
        candidates,
        decision,
    })
}

fn decide(candidates: &[Candidate]) -> Decision {
    let Some(best) = candidates.first() else {
        return Decision::Ambiguous;
    };
    let Some(relative_file) = best.relative_file else {
        return Decision::Ambiguous;
    };
    let runner = candidates.get(1).map_or(0, |c| c.score);
    // At most two u64 counts and factors <=100: both sides fit u128.
    // Saturating operations cannot saturate under these bounds. Ceil division
    // has a constant nonzero divisor; it avoids floating-point boundary drift.
    let threshold = best.sampled.div_ceil(100).max(16);
    let left = u128::from(best.score).saturating_mul(100);
    let right = u128::from(best.score)
        .saturating_add(u128::from(runner))
        .saturating_mul(99);
    if best.file_cohorts == 1 && best.score >= threshold && left >= right {
        Decision::Accepted {
            block_size: best.block_size,
            order: best.order,
            relative_file,
        }
    } else {
        Decision::Ambiguous
    }
}

#[cfg(test)]
#[path = "header_tests.rs"]
mod tests;
