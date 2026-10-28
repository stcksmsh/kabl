//! `audio.in` carries what the host hands the engine, and is silent until then.

use std::collections::BTreeMap;

use basedrop::Collector;
use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::PatchEngine;

fn module(kind: &str) -> ModuleState {
    ModuleState {
        kind: kind.to_string(),
        pos: Vec2 { x: 0.0, y: 0.0 },
        params: BTreeMap::new(),
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

#[test]
fn the_hosts_input_reaches_the_output() {
    let mut p = PatchState::new();
    p.modules.insert(1, module("audio.in"));
    p.modules.insert(2, module("out"));
    p.cables.insert(1, cable(1, "out", 2, "in"));
    let c = Collector::new();
    let mut e = PatchEngine::new(&c.handle(), &p, 48000.0, 1).unwrap();
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    e.process_block(&mut l, &mut r);
    assert!(
        l.iter().chain(&r).all(|&v| v == 0.0),
        "silent before a host feeds it"
    );
    let ramp: [f32; BLOCK] = std::array::from_fn(|i| i as f32 / BLOCK as f32 * 0.5);
    let neg = ramp.map(|v| -v);
    e.set_audio_in(&ramp, &neg);
    for _ in 0..3 {
        e.process_block(&mut l, &mut r);
        assert_eq!((l, r), (ramp, neg));
    }
}
