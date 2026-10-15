//! Synthetic vectors only: odr-testkit models the existing block contract.
//! No vector here is an Oracle DUMP or T1 evidence. See lab-verify issue #6.
use super::*;
use crate::block::{FLG_CHECKSUM, TAIL_LEN};
use crate::bytes::{read_u8, read_u16, read_u32};
use odr_testkit::{synth_block, synth_image};
use proptest::prelude::*;

fn kit_order(order: ByteOrder) -> odr_testkit::ByteOrder {
    match order {
        ByteOrder::Little => odr_testkit::ByteOrder::Little,
        ByteOrder::Big => odr_testkit::ByteOrder::Big,
    }
}

// Independent simple scorer: no classify, BlockHeader, expected_tail, xor_fold,
// sample_offset, or decide calls. Test images are below the exhaustive limit.
fn reference(image: &[u8]) -> Result<ProbeReport, OdrError> {
    let mut candidates = Vec::new();
    for size in BLOCK_SIZES {
        for order in [ByteOrder::Little, ByteOrder::Big] {
            let mut files = BTreeMap::<u32, u64>::new();
            let mut sampled = 0u64;
            for (block, index) in image.chunks_exact(size).zip(0u64..) {
                sampled = sampled.saturating_add(1);
                if read_u8(block, 1)? & 15 != 2 {
                    continue;
                }
                let tail = (read_u32(block, 8, order)? & 65535) << 16
                    | u32::from(read_u8(block, 0)?) << 8
                    | u32::from(read_u8(block, 14)?);
                if read_u32(block, size.saturating_sub(TAIL_LEN), order)? != tail {
                    continue;
                }
                if read_u8(block, 15)? & FLG_CHECKSUM != 0 {
                    let mut fold = 0u16;
                    for pair in block.as_chunks::<2>().0 {
                        fold ^= read_u16(pair, 0, order)?;
                    }
                    if fold != 0 {
                        continue;
                    }
                }
                let rdba = read_u32(block, 4, order)?;
                // T4 smallfile split copied as a specification, not T1 truth.
                if u64::from(rdba & 0x003f_ffff) == index {
                    let entry = files.entry(rdba >> 22).or_default();
                    *entry = entry.saturating_add(1);
                }
            }
            candidates.push(Candidate {
                block_size: size,
                order,
                sampled,
                score: files.values().copied().max().unwrap_or(0),
                file_cohorts: files.len(),
                relative_file: if files.len() == 1 {
                    files.keys().next().copied()
                } else {
                    None
                },
            });
        }
    }
    candidates.sort_by_key(|c| std::cmp::Reverse(c.score));
    let mut decision = Decision::Ambiguous;
    if let (Some(best), Some(runner)) = (candidates.first(), candidates.get(1)) {
        // Independent ratio rearrangement: a >= 99*b, after 100a >= 99(a+b).
        if best.score >= 16
            && best.score.saturating_mul(100) >= best.sampled
            && best.score >= runner.score.saturating_mul(99)
            && let Some(relative_file) = best.relative_file
        {
            decision = Decision::Accepted {
                block_size: best.block_size,
                order: best.order,
                relative_file,
            };
        }
    }
    Ok(ProbeReport {
        image_len: u64::try_from(image.len()).map_err(|_| arithmetic_error())?,
        sample_seed: 0,
        candidates,
        decision,
    })
}

#[test]
fn all_sizes_orders_and_short_boundaries() -> Result<(), OdrError> {
    for size in BLOCK_SIZES {
        for order in [ByteOrder::Little, ByteOrder::Big] {
            for len in [0, 1, size.saturating_sub(1), size, size.saturating_add(1)] {
                let image = vec![0; len];
                let report = infer_image(&image)?;
                assert_eq!(report, reference(&image)?);
                assert_eq!(report.decision, Decision::Ambiguous);
                assert_eq!(report.candidates.len(), 10);
            }
            for count in [15, 16, 31] {
                let mut image = synth_image(count, size, kit_order(order), 71);
                image.push(0xff); // final partial block is excluded, not padded
                let report = infer_image(&image)?;
                assert_eq!(report, reference(&image)?);
                let expected = if count < 16 {
                    Decision::Ambiguous
                } else {
                    Decision::Accepted {
                        block_size: size,
                        order,
                        relative_file: 1,
                    }
                };
                assert_eq!(report.decision, expected);
            }
        }
    }
    Ok(())
}

#[test]
fn wiped_header_and_wrong_rdba() -> Result<(), OdrError> {
    let size = 8192usize;
    let mut image = synth_image(32, size, kit_order(ByteOrder::Little), 29);
    let prefix = image
        .get_mut(..size.saturating_mul(2))
        .ok_or_else(arithmetic_error)?;
    prefix.fill(0);
    assert_eq!(infer_image(&image)?, reference(&image)?);
    assert!(matches!(
        infer_image(&image)?.decision,
        Decision::Accepted { .. }
    ));
    // Repeated valid block zero cannot masquerade as a continuous file.
    let block = synth_block(size, kit_order(ByteOrder::Little), 6, 1 << 22, 123, 1, 42);
    let repeated = block.repeat(32);
    assert_eq!(infer_image(&repeated)?, reference(&repeated)?);
    assert_eq!(infer_image(&repeated)?.decision, Decision::Ambiguous);
    Ok(())
}

#[test]
fn checksum_flag_and_mixed_file_veto() -> Result<(), OdrError> {
    for order in [ByteOrder::Little, ByteOrder::Big] {
        let mut image = synth_image(32, 2048, kit_order(order), 18);
        // Toggle a payload byte without fixing XOR: flagged blocks stop scoring.
        for block in image.as_chunks_mut::<2048>().0 {
            let byte = block.get_mut(100).ok_or_else(arithmetic_error)?;
            *byte ^= 1;
        }
        assert_eq!(infer_image(&image)?.decision, Decision::Ambiguous);
        for block in image.as_chunks_mut::<2048>().0 {
            let flags = block.get_mut(15).ok_or_else(arithmetic_error)?;
            *flags &= !FLG_CHECKSUM;
        }
        assert_eq!(infer_image(&image)?, reference(&image)?);
        assert!(matches!(
            infer_image(&image)?.decision,
            Decision::Accepted { .. }
        ));
        let replacement = synth_block(2048, kit_order(order), 6, (2 << 22) | 31, 77, 1, 2);
        let last = image
            .get_mut(31usize.saturating_mul(2048)..)
            .ok_or_else(arithmetic_error)?;
        last.copy_from_slice(&replacement);
        assert_eq!(infer_image(&image)?, reference(&image)?);
        assert_eq!(infer_image(&image)?.decision, Decision::Ambiguous);
    }
    Ok(())
}

fn candidate(score: u64, sampled: u64) -> Candidate {
    Candidate {
        block_size: 8192,
        order: ByteOrder::Little,
        sampled,
        score,
        file_cohorts: 1,
        relative_file: Some(1023),
    }
}

#[test]
fn exact_thresholds_ties_and_max_counts() {
    assert_eq!(decide(&[]), Decision::Ambiguous);
    assert_eq!(decide(&[candidate(15, 16)]), Decision::Ambiguous);
    assert_eq!(decide(&[candidate(16, 1601)]), Decision::Ambiguous);
    assert!(matches!(
        decide(&[candidate(16, 1600)]),
        Decision::Accepted { .. }
    ));
    assert_eq!(
        decide(&[candidate(98, 100), candidate(1, 100)]),
        Decision::Ambiguous
    );
    assert!(matches!(
        decide(&[candidate(99, 100), candidate(1, 100)]),
        Decision::Accepted { .. }
    ));
    assert_eq!(
        decide(&[candidate(16, 16), candidate(16, 16)]),
        Decision::Ambiguous
    );
    assert!(matches!(
        decide(&[candidate(u64::MAX, u64::MAX)]),
        Decision::Accepted { .. }
    ));
    assert_eq!(
        decide(&[candidate(u64::MAX, u64::MAX), candidate(u64::MAX, u64::MAX)]),
        Decision::Ambiguous
    );
}

#[test]
fn grid_is_unique_aligned_and_spans_large_files() -> Result<(), OdrError> {
    for len in [EXHAUSTIVE_LIMIT.saturating_add(1), 5u64 << 30, u64::MAX] {
        for size in BLOCK_SIZES {
            let size = u64::try_from(size).map_err(|_| arithmetic_error())?;
            let full = len.checked_div(size).ok_or_else(arithmetic_error)?;
            let mut previous = None;
            for i in 0..STRATIFIED_SAMPLES {
                let offset = sample_offset(full, STRATIFIED_SAMPLES, i, size)?;
                assert_eq!(offset.checked_rem(size), Some(0));
                assert!(offset <= len.saturating_sub(size));
                if let Some(prev) = previous {
                    assert!(offset > prev);
                }
                previous = Some(offset);
            }
            assert_eq!(sample_offset(full, STRATIFIED_SAMPLES, 0, size)?, 0);
            assert_eq!(previous, full.saturating_sub(1).checked_mul(size));
        }
    }
    Ok(())
}

#[test]
fn reader_failure_is_not_ambiguity() {
    let result = infer_with(2048, |_, _| {
        Err(Error::new(ErrorKind::PermissionDenied, "injected").into())
    });
    assert!(matches!(result, Err(OdrError::Io(e)) if e.kind() == ErrorKind::PermissionDenied));
}

#[test]
fn maximum_binary_fields_are_not_silently_accepted() -> Result<(), OdrError> {
    for size in BLOCK_SIZES {
        for order in [ByteOrder::Little, ByteOrder::Big] {
            let block = synth_block(
                size,
                kit_order(order),
                u8::MAX,
                u32::MAX,
                u32::MAX,
                u8::MAX,
                u64::MAX,
            );
            assert_eq!(infer_image(&block)?, reference(&block)?);
            assert_eq!(infer_image(&block)?.decision, Decision::Ambiguous);
        }
    }
    Ok(())
}

// Only synthetic test files are created. A unique directory also permits
// concurrent tests; handles are dropped before cleanup for Windows sharing.
struct TestDir(std::path::PathBuf);

impl TestDir {
    fn new(label: &str) -> Result<Self, OdrError> {
        let path = std::env::temp_dir().join(format!("odr-header-{}-{label}", std::process::id()));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn evidence_matches_image_on_arabic_and_long_paths() -> Result<(), OdrError> {
    let dir = TestDir::new("paths")?;
    let mut parent = dir.0.clone();
    // Four 72-character components exceed MAX_PATH but not component limits.
    for _ in 0..4 {
        parent.push("longpath_".repeat(8));
    }
    std::fs::create_dir_all(&parent)?;
    // canonicalize supplies a verbatim \\?\ Windows prefix on that platform.
    let path = parent.canonicalize()?.join("حسابات.img");
    let image = synth_image(32, 8192, kit_order(ByteOrder::Big), 92);
    std::fs::write(&path, &image)?;
    let mut evidence = Evidence::open(&path)?;
    let initial_hash = *evidence.sha256();
    assert_eq!(infer_evidence(&mut evidence)?, infer_image(&image)?);
    assert_eq!(*evidence.sha256(), initial_hash);
    assert!(evidence.verify_unchanged()?);
    drop(evidence);
    Ok(())
}

#[test]
#[cfg(unix)]
fn evidence_rejects_changed_or_shortened_source() -> Result<(), OdrError> {
    let dir = TestDir::new("changes")?;
    let path = dir.0.join("synthetic.img");
    for truncate in [false, true] {
        let image = synth_image(16, 2048, kit_order(ByteOrder::Little), 1);
        std::fs::write(&path, &image)?;
        let mut evidence = Evidence::open(&path)?;
        // Deliberate test-only concurrent writer; never done in production.
        let replacement = vec![0; if truncate { 1 } else { image.len() }];
        std::fs::write(&path, replacement)?;
        let result = infer_evidence(&mut evidence);
        let expected = if truncate {
            ErrorKind::UnexpectedEof
        } else {
            ErrorKind::InvalidData
        };
        assert!(matches!(result, Err(OdrError::Io(e)) if e.kind() == expected));
    }
    Ok(())
}

#[test]
#[cfg(windows)]
fn evidence_rejects_existing_writer_on_windows() -> Result<(), OdrError> {
    let dir = TestDir::new("sharing")?;
    let path = dir.0.join("synthetic.img");
    let writer = std::fs::File::create(&path)?;
    assert!(Evidence::open(&path).is_err());
    drop(writer);
    Ok(())
}

#[test]
#[ignore = "explicit slow I/O test: hashes a synthetic 5 GiB sparse image twice"]
fn evidence_sparse_over_four_gib() -> Result<(), OdrError> {
    use std::io::{Seek, SeekFrom, Write};
    let dir = TestDir::new("sparse")?;
    let path = dir.0.join("synthetic_sparse.img");
    let len = 5u64 << 30;
    let mut file = std::fs::File::create(&path)?;
    file.set_len(len)?;
    let size = 8192u64;
    let index = len
        .checked_div(size)
        .ok_or_else(arithmetic_error)?
        .saturating_sub(1);
    // Last grid endpoint is beyond 4 GiB. One valid maximum-SCN block there
    // must be scored, even though one hit alone cannot authorize acceptance.
    let rdba = (1023 << 22) | u32::try_from(index).map_err(|_| arithmetic_error())?;
    let block = synth_block(
        8192,
        kit_order(ByteOrder::Little),
        6,
        rdba,
        u32::MAX,
        255,
        2,
    );
    file.seek(SeekFrom::Start(
        index.checked_mul(size).ok_or_else(arithmetic_error)?,
    ))?;
    file.write_all(&block)?;
    drop(file);
    let mut evidence = Evidence::open(&path)?;
    let report = infer_evidence(&mut evidence)?;
    assert_eq!(report.image_len, len);
    assert_eq!(report.decision, Decision::Ambiguous);
    assert!(report.candidates.iter().all(|c| c.sampled == 4096));
    let candidate = report
        .candidates
        .iter()
        .find(|c| c.block_size == 8192 && c.order == ByteOrder::Little);
    assert!(matches!(candidate, Some(c) if c.score == 1 && c.relative_file == Some(1023)));
    drop(evidence);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn arbitrary_bytes_never_panic_and_match_reference(image in prop::collection::vec(any::<u8>(), 0..65537)) {
        let actual = infer_image(&image).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let expected = reference(&image).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(actual, expected);
    }

    #[test]
    fn generated_valid_corrupt_truncated_match_reference(
        size in prop::sample::select(BLOCK_SIZES.to_vec()),
        big in any::<bool>(), n in 0usize..34, seed in any::<u64>(),
        cut in 0usize..32769, flip in any::<usize>(),
    ) {
        let order = if big { ByteOrder::Big } else { ByteOrder::Little };
        let mut image = synth_image(n, size, kit_order(order), seed);
        if !image.is_empty() {
            if let Some(index) = flip.checked_rem(image.len())
                && let Some(byte) = image.get_mut(index) { *byte ^= 1; }
            image.truncate(image.len().saturating_sub(cut));
        }
        let actual = infer_image(&image).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let expected = reference(&image).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(actual, expected);
    }
}
