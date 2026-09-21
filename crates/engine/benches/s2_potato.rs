//! Spike S2 benchmark: ns/block for the naive vs. control-rate-optimized paths of
//! `PotatoPatch` (brief section 11, S2; see `crates/engine/src/potato.rs`). Run under
//! `taskset -c 0` to pin to one core for a cleaner (if not Pi-4-accurate — see
//! docs/decisions.md) comparison:
//!
//!   taskset -c 0 cargo bench -p kabl-engine --bench s2_potato

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use kabl_engine::graph::BLOCK;
use kabl_engine::potato::PotatoPatch;

const SAMPLE_RATE: f32 = 48000.0;

fn bench_naive(c: &mut Criterion) {
    let mut patch = PotatoPatch::new(SAMPLE_RATE);
    let mut out = [0f32; BLOCK];
    c.bench_function("potato_naive_block", |b| {
        b.iter(|| {
            patch.process_naive(&mut out);
            black_box(out[0]);
        })
    });
}

fn bench_optimized(c: &mut Criterion) {
    let mut patch = PotatoPatch::new(SAMPLE_RATE);
    let mut out = [0f32; BLOCK];
    c.bench_function("potato_optimized_block", |b| {
        b.iter(|| {
            patch.process_optimized(&mut out);
            black_box(out[0]);
        })
    });
}

criterion_group!(benches, bench_naive, bench_optimized);
criterion_main!(benches);
