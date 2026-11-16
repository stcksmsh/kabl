//! The output module's Level (dB): a patch saved before the parameter existed (no `level_db`
//! in the saved module) plays at 0 dB, and a set level scales what reaches the device.

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

/// A steady 0.6 from a macro to the output, whose saved params are `out_params`. Returns the
/// last left and right samples after the level has settled.
fn settled(out_params: &[(&str, f32)]) -> (f32, f32) {
    let mut p = PatchState::new();
    p.modules.insert(1, module("macro", &[("m1", 0.6)]));
    p.modules.insert(2, module("out", out_params));
    p.cables.insert(
        1,
        CableState {
            from: PortRef::Module {
                id: 1,
                port: "m1".into(),
            },
            to: PortRef::Module {
                id: 2,
                port: "in".into(),
            },
            params: BTreeMap::new(),
            steps: Vec::new(),
        },
    );
    let c = Collector::new();
    let mut e = PatchEngine::new(&c.handle(), &p, 48000.0, 1).unwrap();
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    for _ in 0..400 {
        e.process_block(&mut l, &mut r);
    }
    (l[BLOCK - 1], r[BLOCK - 1])
}

#[test]
fn an_old_patch_without_level_plays_at_zero_db() {
    let (l, _) = settled(&[]);
    assert!((l - 0.6).abs() < 1e-6, "{l}");
    let (explicit, _) = settled(&[("level_db", 0.0)]);
    assert_eq!(l, explicit);
}

#[test]
fn level_scales_the_output() {
    let (l, _) = settled(&[("level_db", -6.0206)]);
    assert!((l - 0.3).abs() < 1e-3, "{l}");
    let (louder, _) = settled(&[("level_db", 6.0206)]);
    assert!((louder - 1.2).abs() < 1e-2, "{louder}");
}

#[test]
fn the_output_level_is_smoothed_by_the_module_not_ramped_twice() {
    use kabl_engine::runtime::{ramped, smoothed_by_module};
    let level = &kabl_modules::builtins::OUT_INFO.params[0];
    assert_eq!(level.name, "level_db");
    assert!(smoothed_by_module("out", "level_db"));
    assert!(
        !ramped("out", level),
        "the runtime must set it, not ramp it"
    );
}
