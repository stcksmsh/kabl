//! Parameter modulation through the real compiler, against hand-computed expected values:
//! taper-space summing, one clamp, bypass, inversion, unipolar / bipolar / pitch sources,
//! exponential and stepped destinations, per-voice routing and feedback.
//!
//! Probe: `midi.in.gate -> vca.in -> out.left` (1.0 × gain), so the output is exactly the
//! modulated `vca.gain` (linear 0..1, taper = identity). Constant bipolar source: an `lfo`
//! square at 0.01 Hz, which stays at +1 for its first 50 s.

use std::collections::BTreeMap;

use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::compile::{compile, CompiledPatch};

const SR: f32 = 48000.0;
const MIDI: u64 = 1;
const VCA: u64 = 2;
const OUT: u64 = 3;
const SQUARE: u64 = 10;

fn module(kind: &str, params: &[(&str, f32)]) -> ModuleState {
    ModuleState {
        kind: kind.to_string(),
        pos: Vec2 { x: 0.0, y: 0.0 },
        params: params.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
    }
}

fn jack(from: (u64, &str), to: (u64, &str)) -> CableState {
    CableState {
        from: PortRef::Module {
            id: from.0,
            port: from.1.into(),
        },
        to: PortRef::Module {
            id: to.0,
            port: to.1.into(),
        },
        params: BTreeMap::new(),
        steps: Vec::new(),
    }
}

fn route(from: (u64, &str), to: (u64, &str), amount: f32, bypass: bool) -> CableState {
    let mut params = BTreeMap::from([("amount".to_string(), amount)]);
    if bypass {
        params.insert("bypass".into(), 1.0);
    }
    CableState {
        from: PortRef::Module {
            id: from.0,
            port: from.1.into(),
        },
        to: PortRef::Param {
            id: to.0,
            param: to.1.into(),
        },
        params,
        steps: Vec::new(),
    }
}

fn probe(base_gain: f32) -> PatchState {
    let mut p = PatchState::new();
    p.modules.insert(MIDI, module("midi.in", &[]));
    p.modules.insert(VCA, module("vca", &[("gain", base_gain)]));
    p.modules.insert(OUT, module("out", &[]));
    p.modules.insert(
        SQUARE,
        module("lfo", &[("rate_hz", 0.01), ("waveform", 3.0)]),
    );
    p.cables.insert(1, jack((MIDI, "gate"), (VCA, "in")));
    p.cables.insert(2, jack((VCA, "out"), (OUT, "left")));
    p
}

fn play(p: &PatchState, voices: usize, notes: &[(usize, f32, f32)]) -> CompiledPatch {
    let mut c = compile(p, SR, voices).expect("compiles");
    for &(v, semis, vel) in notes {
        c.note_on(v, semis, vel);
    }
    c.process_block();
    c
}

fn gain_out(p: &PatchState) -> f32 {
    let c = play(p, 1, &[(0, 0.0, 1.0)]);
    let v = c.left()[0];
    assert!(
        c.left().iter().all(|&s| s == v),
        "block-rate: constant in block"
    );
    v
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

#[test]
fn bipolar_source_adds_amount_in_knob_travel() {
    let mut p = probe(0.5);
    p.cables
        .insert(10, route((SQUARE, "out"), (VCA, "gain"), 0.25, false));
    assert!(close(gain_out(&p), 0.75), "0.5 + 0.25 × (+1)");
}

#[test]
fn inverted_route_subtracts() {
    let mut p = probe(0.5);
    p.cables
        .insert(10, route((SQUARE, "out"), (VCA, "gain"), -0.25, false));
    assert!(close(gain_out(&p), 0.25));
}

#[test]
fn result_clamps_at_the_param_limit() {
    let mut p = probe(0.9);
    p.cables
        .insert(10, route((SQUARE, "out"), (VCA, "gain"), 0.3, false));
    assert!(close(gain_out(&p), 1.0));
}

#[test]
fn routes_sum_before_a_single_clamp() {
    // Per-route clamping would give min(0.9 + 0.3, 1) - 0.3 = 0.7. Summing first gives 0.9.
    let mut p = probe(0.9);
    p.cables
        .insert(10, route((SQUARE, "out"), (VCA, "gain"), 0.3, false));
    p.cables
        .insert(11, route((SQUARE, "out"), (VCA, "gain"), -0.3, false));
    assert!(close(gain_out(&p), 0.9));
}

#[test]
fn bypassed_route_contributes_nothing_and_keeps_its_settings() {
    let mut p = probe(0.5);
    p.cables
        .insert(10, route((SQUARE, "out"), (VCA, "gain"), 0.3, true));
    assert!(close(gain_out(&p), 0.5));
    assert_eq!(p.cables[&10].params["amount"], 0.3);
}

#[test]
fn absent_amount_uses_the_default_quarter_travel() {
    let mut p = probe(0.5);
    let mut r = route((SQUARE, "out"), (VCA, "gain"), 0.0, false);
    r.params.clear();
    p.cables.insert(10, r);
    assert!(close(gain_out(&p), 0.75));
}

#[test]
fn unipolar_source_is_not_treated_as_bipolar() {
    // Velocity 0.5 (0..1 source) × amount 0.4 = +0.2. A bipolar reading (2v-1 = 0) would add 0.
    let mut p = probe(0.5);
    p.cables
        .insert(10, route((MIDI, "velocity"), (VCA, "gain"), 0.4, false));
    let c = play(&p, 1, &[(0, 0.0, 0.5)]);
    assert!(close(c.left()[0], 0.7));
}

#[test]
fn pitch_source_scales_by_its_nominal_full_scale() {
    // +12 semitones of a ±60 st full scale × amount 0.5 = +0.1.
    let mut p = probe(0.5);
    p.cables
        .insert(10, route((MIDI, "pitch"), (VCA, "gain"), 0.5, false));
    let c = play(&p, 1, &[(0, 12.0, 1.0)]);
    assert!(close(c.left()[0], 0.6));
}

#[test]
fn each_voice_gets_its_own_source_value() {
    // Voice 0 velocity 0 → gain 0.5; voice 1 velocity 1 → gain 0.9. Out averages 2 voices.
    let mut p = probe(0.5);
    p.cables
        .insert(10, route((MIDI, "velocity"), (VCA, "gain"), 0.4, false));
    let c = play(&p, 2, &[(0, 0.0, 0.0), (1, 0.0, 1.0)]);
    assert!(close(c.left()[0], (0.5 + 0.9) / 2.0));
}

/// A saw LFO's per-sample step is `2 × rate / SR`, which reveals its (block-rate) rate.
fn saw_rate(p: &PatchState) -> f32 {
    let c = play(p, 1, &[]);
    let out = c.left();
    (out[2] - out[1]) * SR / 2.0
}

fn saw_probe() -> PatchState {
    let mut p = probe(1.0);
    p.modules
        .insert(20, module("lfo", &[("rate_hz", 1.0), ("waveform", 2.0)]));
    // saw -> vca.in replaces the gate; gain 1 passes it through.
    p.cables.insert(1, jack((20, "out"), (VCA, "in")));
    p
}

#[test]
fn exponential_param_is_modulated_in_log_space() {
    // rate_hz: 0.01..100 Hz exponential. Base 1 Hz sits at ln(100)/ln(10000) = 0.5 of travel.
    // +0.25 → 0.75 of travel → 0.01 × 10000^0.75 = 10 Hz (a linear taper would give ~25 Hz).
    let mut p = saw_probe();
    assert!((saw_rate(&p) - 1.0).abs() < 1e-2, "unmodulated 1 Hz");
    p.cables
        .insert(10, route((SQUARE, "out"), (20, "rate_hz"), 0.25, false));
    let rate = saw_rate(&p);
    assert!((rate - 10.0).abs() < 0.05, "expected 10 Hz, got {rate}");
}

#[test]
fn stepped_param_moves_by_whole_options() {
    // waveform 0..4 stepped. Base 1 (triangle) + 0.5 × 4 options' span = 3 (square): only ±1.
    let mut p = saw_probe();
    p.modules
        .get_mut(&20)
        .unwrap()
        .params
        .insert("waveform".into(), 1.0);
    p.cables
        .insert(10, route((SQUARE, "out"), (20, "waveform"), 0.5, false));
    let c = play(&p, 1, &[]);
    assert!(c.left().iter().all(|&s| s == 1.0 || s == -1.0), "square");
}

#[test]
fn feedback_route_compiles_with_a_one_block_delay() {
    // An LFO modulating its own rate: the first block reads the delay buffer (silence), so it
    // runs at the base rate; later blocks see its own previous output.
    let mut p = saw_probe();
    p.cables
        .insert(10, route((20, "out"), (20, "rate_hz"), 0.25, false));
    let mut c = compile(&p, SR, 1).expect("feedback compiles");
    c.process_block();
    let first = (c.left()[2] - c.left()[1]) * SR / 2.0;
    assert!(
        (first - 1.0).abs() < 1e-2,
        "block 0 at base rate, got {first}"
    );
    // Block 1 reads block 0's first sample (saw at phase 0 = -1): 0.5 - 0.25 of travel
    // → 0.01 × 10000^0.25 = 0.1 Hz.
    c.process_block();
    let second = (c.left()[2] - c.left()[1]) * SR / 2.0;
    assert!(
        (second - 0.1).abs() < 1e-3,
        "block 1 one block late, got {second}"
    );
    for _ in 0..2000 {
        c.process_block();
        assert!(c.left().iter().all(|s| s.is_finite()));
    }
}

#[test]
fn a_route_to_an_unknown_param_is_a_compile_error() {
    let mut p = probe(0.5);
    p.cables
        .insert(10, route((SQUARE, "out"), (VCA, "nope"), 0.3, false));
    assert!(compile(&p, SR, 1).is_err());
}
