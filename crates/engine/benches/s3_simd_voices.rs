//! Spike S3 benchmark: scalar 4-voice osc+filter vs. f32x4-batched. Both per-sample
//! (`voices4_*_sample`) and per-block (`voices4_*_block`, BLOCK=64 — matching how the real
//! engine actually calls into voice processing) to check whether call-overhead granularity was
//! skewing the per-sample numbers; it wasn't, both give the same ratio. Run under `taskset -c 0`
//! — see docs/decisions.md "Spike S3" for why this container's absolute numbers come with the
//! same noise caveat as S2's.
//!
//!   taskset -c 0 cargo bench -p kabl-engine --bench s3_simd_voices

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use kabl_engine::graph::BLOCK;
use kabl_engine::simd_voices::{ScalarVoices, SimdVoices};

const SAMPLE_RATE: f32 = 48000.0;

fn bench_scalar_sample(c: &mut Criterion) {
    let mut voices = ScalarVoices::new(SAMPLE_RATE);
    c.bench_function("voices4_scalar_sample", |b| {
        b.iter(|| black_box(voices.next(SAMPLE_RATE)))
    });
}

fn bench_simd_sample(c: &mut Criterion) {
    let mut voices = SimdVoices::new(SAMPLE_RATE);
    c.bench_function("voices4_simd_sample", |b| {
        b.iter(|| black_box(voices.next(SAMPLE_RATE)))
    });
}

// Block-at-a-time (BLOCK=64 samples/call): matches how the real engine actually calls into
// voice processing, per brief section 7's flat schedule. See docs/decisions.md "Spike S3" for
// why the per-sample numbers above undersell SIMD (per-call overhead dominates at ~5-10ns/call).
fn bench_scalar_block(c: &mut Criterion) {
    let mut voices = ScalarVoices::new(SAMPLE_RATE);
    let mut out = [[0f32; 4]; BLOCK];
    c.bench_function("voices4_scalar_block", |b| {
        b.iter(|| {
            voices.process_block(SAMPLE_RATE, &mut out);
            black_box(out[0]);
        })
    });
}

fn bench_simd_block(c: &mut Criterion) {
    let mut voices = SimdVoices::new(SAMPLE_RATE);
    let mut out = [wide::f32x4::splat(0.0); BLOCK];
    c.bench_function("voices4_simd_block", |b| {
        b.iter(|| {
            voices.process_block(SAMPLE_RATE, &mut out);
            black_box(out[0]);
        })
    });
}

criterion_group!(
    benches,
    bench_scalar_sample,
    bench_simd_sample,
    bench_scalar_block,
    bench_simd_block
);
criterion_main!(benches);

// A filter-only micro-benchmark (isolating the SVF's branchless arithmetic from the saw
// oscillator's branchy PolyBLEP correction, to test whether PolyBLEP's unconditional both-
// regions computation explains the <2.5x ratio above) was tried and dropped: it measured both
// the scalar AND SIMD isolated-filter paths at ~15-20x slower than their share of the combined
// osc+filter benchmark, which isn't physically sensible (isolating one cheaper piece of a
// pipeline shouldn't make it slower than the whole pipeline) and reads as a benchmarking
// artifact rather than a real signal. Didn't chase it further — that's proper profiling-tool
// territory (perf stat/record), not a criterion micro-benchmark, and disproportionate for a
// spike. See docs/decisions.md "Spike S3" for the full writeup and what's actually established
// vs. hypothesized.
