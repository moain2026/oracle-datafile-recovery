//! Reproduce: cargo bench --manifest-path bench/header-probe/Cargo.toml
//! Warm, resident synthetic input; NOT disk throughput or Oracle accuracy.
//! Ten-hypothesis inference and one-hypothesis reference scan do different work.
//! Criterion raw estimates include the bootstrap median and its 95% CI.
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use odr_core::ByteOrder;
use odr_core::scan::{header::infer_image, scan_image};
use odr_testkit::synth_image;
use std::hint::black_box;
use std::time::Duration;

fn bench(c: &mut Criterion) {
    // Product benchmark parameters, not binary format assertions: 1 MiB,
    // 128 synthetic 8 KiB blocks, fixed PRNG seed 71. Setup is not timed.
    let image = synth_image(128, 8192, ByteOrder::Little, 71);
    let mut group = c.benchmark_group("header_probe_warm_1MiB");
    group.throughput(Throughput::Bytes(1 << 20));
    group.sample_size(40);
    group.confidence_level(0.95);
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(5));
    group.bench_function("infer_ten_hypotheses", |b| {
        b.iter(|| black_box(infer_image(black_box(&image))))
    });
    group.bench_function("scan_reference_one_hypothesis", |b| {
        b.iter(|| black_box(scan_image(black_box(&image), 8192, ByteOrder::Little)))
    });
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
