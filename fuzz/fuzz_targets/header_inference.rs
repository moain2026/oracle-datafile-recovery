#![no_main]
//! Arbitrary-byte inference: no panic, ten candidates, bounded scores.
//! Synthetic-only until the Oracle lab provides permitted golden seeds.
use libfuzzer_sys::fuzz_target;
use odr_core::scan::header::{Decision, infer_image};

fuzz_target!(|data: &[u8]| {
    let result = infer_image(data);
    assert!(result.is_ok());
    if let Ok(report) = result {
        assert_eq!(report.candidates.len(), 10);
        for candidate in &report.candidates {
            assert!(candidate.score <= candidate.sampled);
        }
        if let Decision::Accepted {
            block_size,
            order,
            relative_file,
        } = report.decision
        {
            assert!(report.candidates.iter().any(|c| {
                c.block_size == block_size
                    && c.order == order
                    && c.relative_file == Some(relative_file)
                    && c.file_cohorts == 1
                    && c.score >= 16
            }));
        }
    }
});
