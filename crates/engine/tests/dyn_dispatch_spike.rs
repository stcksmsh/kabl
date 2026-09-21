//! Correctness gate for the `dyn Module` dispatch spike (`src/dyn_dispatch_spike.rs`): before
//! trusting any ns/block comparison against `patch_demo.rs`'s static-dispatch `Patch`, prove
//! `DynPatch` computes the identical signal. It's the same modules, same wiring, same params —
//! only the call mechanism (vtable vs. static) differs — so bit-exact agreement is the expected
//! result, not a lucky pass, same reasoning as spike S3's scalar-vs-SIMD gate.

use kabl_engine::dyn_dispatch_spike::DynPatch;
use kabl_engine::patch_demo::Patch;

#[test]
fn dyn_dispatch_matches_static_dispatch_bit_exact() {
    let mut static_patch = Patch::new();
    static_patch.prepare();
    static_patch.note_on_chord();

    let mut dyn_patch = DynPatch::new();
    dyn_patch.prepare();

    for block in 0..200 {
        let a = static_patch.process_block();
        let b = dyn_patch.process_block();
        for i in 0..a.len() {
            assert_eq!(
                a[i], b[i],
                "block {block} sample {i}: static={} dyn={}",
                a[i], b[i]
            );
        }
    }
}
