//! ns/block comparison: static dispatch (`patch_demo::Patch`, concrete typed fields) vs. `dyn
//! Module` dispatch (`dyn_dispatch_spike::DynPatch`, `Box<dyn Module>`), identical topology and
//! wiring otherwise. Answers the handover question in `docs/STATUS.md`: measure `dyn Module`
//! dispatch cost before assuming the real compiler's heterogeneous `Box<dyn Module>` collection
//! costs the same as `patch_demo.rs`'s static-dispatch ns/block number. Run under `taskset -c 0`,
//! same noise caveat as S1-S3 (shared cloud VM):
//!
//!   taskset -c 0 cargo bench -p kabl-engine --bench dyn_dispatch_spike

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use kabl_engine::dyn_dispatch_spike::DynPatch;
use kabl_engine::patch_demo::Patch;

fn bench_static(c: &mut Criterion) {
    let mut patch = Patch::new();
    patch.prepare();
    patch.note_on_chord();
    c.bench_function("dispatch_static_block", |b| {
        b.iter(|| black_box(patch.process_block()))
    });
}

fn bench_dyn(c: &mut Criterion) {
    let mut patch = DynPatch::new();
    patch.prepare();
    c.bench_function("dispatch_dyn_block", |b| {
        b.iter(|| black_box(patch.process_block()))
    });
}

criterion_group!(benches, bench_static, bench_dyn);
criterion_main!(benches);
