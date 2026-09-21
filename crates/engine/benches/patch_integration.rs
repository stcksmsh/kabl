//! ns/block for the first real patch built from `Module` trait objects (`src/patch_demo.rs`) —
//! 4 voices x (midi.in, osc.va, filter.svf, env.adsr, vca) + mixer + out, wired through
//! `ProcessIo`, not hand-rolled DSP. Run under `taskset -c 0` — same noise caveat as S1-S3
//! (shared cloud VM).
//!
//!   taskset -c 0 cargo bench -p kabl-engine --bench patch_integration

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use kabl_engine::patch_demo::Patch;

fn bench_patch(c: &mut Criterion) {
    let mut patch = Patch::new();
    patch.prepare();
    patch.note_on_chord();
    c.bench_function("patch_integration_block", |b| {
        b.iter(|| black_box(patch.process_block()))
    });
}

criterion_group!(benches, bench_patch);
criterion_main!(benches);
