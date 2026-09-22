//! Proves `PatchEngine` handles a second `build_swap`/`receive_swap` arriving while a fade is
//! still in flight (brief section 7.6's "overlapping swaps unhandled" gap — see
//! docs/decisions.md "PatchEngine: overlapping swaps"): the second swap is neither dropped nor
//! allowed to snap the blended output to a different signal mid-crossfade, just queued and
//! started as an ordinary fade the moment the first one finishes.
//!
//! Method: three patches identical except for `vca`'s `gain` param (1.0, 0.5, 0.0) — a
//! compile-time-constant param (`vca` is stateless, `save_state`/`load_state` are no-ops), so it
//! never carries across a recompile the way module state does. All three share one `osc.va` at
//! the same `base_hz`, so — like `compile.rs`'s own recompile test already establishes bit-exact
//! — every instance's oscillator phase stays perfectly in lockstep with the others regardless of
//! which one is "active" when. That means two independent, freshly-`compile()`d references (one
//! per non-1.0 gain) can be run alongside the engine the whole time and compared bit-exact
//! against it in the steady windows outside each crossfade, the same proven pattern
//! `patch_engine_swap.rs` already uses for its single-swap case, just for two swaps back to back.

use std::collections::BTreeMap;

use basedrop::Collector;
use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::compile::compile;
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::PatchEngine;

const SR: f32 = 48000.0;
const BASE_HZ: f32 = 220.0;

fn module(kind: &str, params: &[(&str, f32)]) -> ModuleState {
    ModuleState {
        kind: kind.to_string(),
        pos: Vec2 { x: 0.0, y: 0.0 },
        params: params.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
    }
}

fn cable(from_id: u64, from_port: &str, to_id: u64, to_port: &str) -> CableState {
    CableState {
        from: PortRef::Module {
            id: from_id,
            port: from_port.to_string(),
        },
        to: PortRef::Module {
            id: to_id,
            port: to_port.to_string(),
        },
        params: BTreeMap::new(),
        steps: Vec::new(),
    }
}

/// `osc.va` (unconnected pitch, so `base_hz` directly) -> `vca` (the one thing that differs
/// between calls) -> `out`.
fn osc_vca_patch(gain: f32) -> PatchState {
    let mut patch = PatchState::new();
    patch.modules.insert(
        1,
        module("osc.va", &[("base_hz", BASE_HZ), ("waveform", 2.0)]),
    );
    patch
        .modules
        .insert(2, module("vca", &[("gain", gain), ("exponential", 0.0)]));
    patch.modules.insert(3, module("out", &[]));
    patch.cables.insert(1, cable(1, "out", 2, "in"));
    patch.cables.insert(2, cable(2, "out", 3, "left"));
    patch.cables.insert(3, cable(2, "out", 3, "right"));
    patch
}

#[test]
fn a_second_swap_queued_mid_crossfade_is_not_dropped_and_does_not_click() {
    let patch_a = osc_vca_patch(1.0);
    let patch_b = osc_vca_patch(0.5);
    let patch_c = osc_vca_patch(0.0);

    let mut collector = Collector::new();
    let handle = collector.handle();

    let mut engine = PatchEngine::new(&handle, &patch_a, SR, 1).expect("engine should compile");
    let crossfade_samples = engine.crossfade_samples();
    // 15ms @ 48kHz = 720 samples = 11.25 blocks -> a fade actually finishes on the 12th call.
    let crossfade_blocks = crossfade_samples.div_ceil(BLOCK);

    // Independent references for the two non-starting gains, run in lockstep with the engine the
    // whole time -- their oscillator phase stays bit-identical to whichever instance is active
    // inside `engine` at any given moment (same base_hz throughout, proven generally by
    // compile.rs's recompile state-carryover test).
    let mut hard_b = compile(&patch_b, SR, 1).expect("hard_b should compile");
    let mut hard_c = compile(&patch_c, SR, 1).expect("hard_c should compile");

    // Fade 1: patch_a (active) -> patch_b (incoming). Starts before any block has been processed,
    // so hard_b's freshly-compiled phase (0) matches what build_swap's snapshot carries.
    let swap_b = engine.build_swap(&handle, &patch_b).expect("build_swap b");
    engine.receive_swap(swap_b);

    // Brief section 7.6: a second swap while fade 1 is still in flight. Must queue, not overwrite
    // `incoming` (which would restart the fade from scratch against a different signal -- a
    // click) and must not be silently dropped either.
    let swap_c = engine.build_swap(&handle, &patch_c).expect("build_swap c");
    engine.receive_swap(swap_c);
    assert!(
        engine.is_swapping(),
        "should be swapping (or queued to) right after the overlapping receive_swap"
    );

    let total_blocks = 2 * crossfade_blocks + 4; // fade1 + fade2 + a clean steady-state tail
    let total_samples = total_blocks * BLOCK;
    let mut engine_left: Vec<f32> = Vec::with_capacity(total_samples);
    let mut hard_b_left: Vec<f32> = Vec::with_capacity(total_samples);
    let mut hard_c_left: Vec<f32> = Vec::with_capacity(total_samples);

    for _ in 0..total_blocks {
        let mut l = [0f32; BLOCK];
        let mut r = [0f32; BLOCK];
        engine.process_block(&mut l, &mut r);
        engine_left.extend_from_slice(&l);

        hard_b.process_block();
        hard_b_left.extend_from_slice(&hard_b.left()[..]);
        hard_c.process_block();
        hard_c_left.extend_from_slice(&hard_c.left()[..]);
    }
    collector.collect();

    assert_eq!(engine_left.len(), total_samples);
    assert!(
        engine_left.iter().all(|v| v.is_finite()),
        "output must stay finite across both overlapping fades"
    );
    assert!(
        !engine.is_swapping(),
        "both fades should have long finished by now"
    );

    // Steady window between the two fades: fade 1 finished (active == patch_b), fade 2's own
    // pre-crossfade-start blend region hasn't diverged from pure old yet, so this whole stretch
    // must read exactly like an untouched patch_b instance -- crucially, this is the proof that
    // the queued swap didn't skip patch_b entirely (a buggy "overwrite incoming in place" fix
    // would land here too, at the *wrong* value -- still mid-way through fading from patch_a).
    let window1 = crossfade_samples..(crossfade_blocks * BLOCK);
    assert!(!window1.is_empty(), "test setup should leave a window 1");
    assert_eq!(
        engine_left[window1.clone()],
        hard_b_left[window1],
        "between the two fades, output must match patch_b exactly -- the queued second swap \
         must not have skipped or corrupted the first one"
    );

    // Steady window after fade 2: active == patch_c (gain 0.0, silence) -- proof the queued swap
    // was actually applied, not dropped.
    let window2 = (crossfade_blocks * BLOCK + crossfade_samples)..total_samples;
    assert!(!window2.is_empty(), "test setup should leave a window 2");
    assert_eq!(
        engine_left[window2.clone()],
        hard_c_left[window2],
        "after both fades, output must match patch_c (silence) exactly"
    );
    assert!(
        engine_left[crossfade_blocks * BLOCK + crossfade_samples..]
            .iter()
            .all(|&v| v == 0.0),
        "patch_c's gain is 0.0 -- steady-state output after both fades must be exact silence"
    );

    // No click at the fade1 -> fade2 handoff: fade 2's very first sample (equal-power's t=0 --
    // all old, no new yet: cos(0)=1, sin(0)=0 exactly) must be an exact continuation of patch_b,
    // the same reference window1 already matched -- not a jump to something else.
    let handoff = crossfade_blocks * BLOCK;
    assert_eq!(
        engine_left[handoff], hard_b_left[handoff],
        "fade 2 starting must not introduce a discontinuity -- its first sample is still pure \
         old (patch_b), so it must match patch_b's own reference exactly"
    );
}
