//! An unpatched input reaches a module as a held zero (`Signal::Scalar`), not a buffer of
//! zeros: the compressor tells "no sidechain patched" from "sidechain patched" by that.

use std::collections::BTreeMap;

use basedrop::Collector;
use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::PatchEngine;

fn module(kind: &str, params: &[(&str, f32)]) -> ModuleState {
    ModuleState {
        kind: kind.to_string(),
        pos: Vec2 { x: 0.0, y: 0.0 },
        params: params.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
    }
}

fn cable(from: u64, fp: &str, to: u64, tp: &str) -> CableState {
    let port = |id, p: &str| PortRef::Module { id, port: p.into() };
    CableState {
        from: port(from, fp),
        to: port(to, tp),
        params: BTreeMap::new(),
        steps: Vec::new(),
    }
}

/// A steady 0.6 through a compressor (-30 dB threshold, 8:1) to the output; `sidechain` patches
/// the same signal into `sc`.
fn last_sample(sidechain: bool) -> f32 {
    let mut p = PatchState::new();
    p.modules.insert(1, module("macro", &[("m1", 0.6)]));
    p.modules.insert(
        2,
        module(
            "compressor",
            &[("threshold_db", -30.0), ("ratio", 8.0), ("attack_ms", 1.0)],
        ),
    );
    p.modules.insert(3, module("out", &[]));
    p.cables.insert(1, cable(1, "m1", 2, "in"));
    p.cables.insert(2, cable(2, "out", 3, "in"));
    if sidechain {
        p.cables.insert(3, cable(1, "m1", 2, "sc"));
    }
    let c = Collector::new();
    let mut e = PatchEngine::new(&c.handle(), &p, 48000.0, 1).unwrap();
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    for _ in 0..400 {
        e.process_block(&mut l, &mut r);
    }
    l[BLOCK - 1]
}

#[test]
fn the_compressor_works_without_a_sidechain_patched() {
    // 0.6 is -4.4 dB: about 22 dB of reduction, so well under 0.1 once settled.
    let internal = last_sample(false);
    assert!(internal > 0.0 && internal < 0.1, "{internal}");
    // A patched sidechain carrying the same signal gives the same result.
    assert!((last_sample(true) - internal).abs() < 1e-4);
}
