//! `env.adsr` timing modes through the real compiler and `PatchEngine` swaps.
//!
//! Probe: `midi.in.gate -> env.adsr -> out.left`, 1 voice, so the output is the envelope level.
//! Attack is a one-pole rise: from level `L`, after `n` samples at attack time `A` ms the level
//! is `1 - (1 - L)·c^n` with `c = exp(-1 / (A·SR/1000))`. Release: `L·c^n`. Tests compare
//! against these closed forms, computed independently of the engine.

use basedrop::{Collector, Handle, Owned};
use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::compile::compile;
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::PatchEngine;

const SR: f32 = 48000.0;
const ENV: u64 = 2;
/// Crossfade (720 samples) rounded up to whole blocks, plus margin.
const FADE_BLOCKS: usize = 13;

fn module(kind: &str, params: &[(&str, f32)]) -> ModuleState {
    ModuleState {
        kind: kind.to_string(),
        pos: Vec2 { x: 0.0, y: 0.0 },
        params: params.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
    }
}

fn cable(from: (u64, &str), to: PortRef, params: &[(&str, f32)]) -> CableState {
    CableState {
        from: PortRef::Module {
            id: from.0,
            port: from.1.into(),
        },
        to,
        params: params.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
        steps: Vec::new(),
    }
}

fn jack(id: u64, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

/// `timing` 0 = continuous, 1 = key-trigger. Sustain 1.0 so the level after attack stays put.
fn env_patch(timing: f32, attack: f32, release: f32) -> PatchState {
    let mut p = PatchState::new();
    p.modules.insert(1, module("midi.in", &[]));
    p.modules.insert(
        ENV,
        module(
            "env.adsr",
            &[
                ("attack_ms", attack),
                ("decay_ms", 100.0),
                ("sustain", 1.0),
                ("release_ms", release),
                ("timing", timing),
            ],
        ),
    );
    p.modules.insert(3, module("out", &[]));
    p.cables
        .insert(1, cable((1, "gate"), jack(ENV, "gate"), &[]));
    p.cables
        .insert(2, cable((ENV, "out"), jack(3, "left"), &[]));
    p
}

fn coeff(ms: f32) -> f64 {
    (-1.0 / (ms as f64 * 0.001 * SR as f64)).exp()
}

fn attack_level(from: f64, ms: f32, n: usize) -> f64 {
    1.0 - (1.0 - from) * coeff(ms).powi(n as i32)
}

fn swap(engine: &mut PatchEngine, h: &Handle, p: &PatchState) {
    engine.receive_swap(Owned::new(h, compile(p, SR, 1).unwrap()));
}

/// Runs `n` blocks, returns the last sample.
fn run(engine: &mut PatchEngine, n: usize) -> f32 {
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    for _ in 0..n {
        engine.process_block(&mut l, &mut r);
    }
    l[BLOCK - 1]
}

/// Held note, attack 1000 ms. After 10 blocks, a live edit changes attack to 5 ms. Returns the
/// level once the fade is over and the sample count at the swap.
fn held_note_attack_edit(timing: f32) -> (f32, usize) {
    let collector = Collector::new();
    let h = collector.handle();
    let mut e = PatchEngine::new(&h, &env_patch(timing, 1000.0, 300.0), SR, 1).unwrap();
    e.note_on(0, 0.0, 1.0);
    run(&mut e, 10);
    swap(&mut e, &h, &env_patch(timing, 5.0, 300.0));
    (run(&mut e, FADE_BLOCKS), 10 * BLOCK)
}

#[test]
fn key_trigger_keeps_the_attack_captured_at_note_on_across_a_live_edit() {
    let (level, at_swap) = held_note_attack_edit(1.0);
    let expect = attack_level(0.0, 1000.0, at_swap + FADE_BLOCKS * BLOCK);
    assert!(
        (level as f64 - expect).abs() < 1e-4,
        "still the 1000 ms attack: {level} vs {expect}"
    );
}

#[test]
fn continuous_follows_the_edited_attack_from_the_current_level() {
    let (level, at_swap) = held_note_attack_edit(0.0);
    // The new graph starts at the carried level and rises at 5 ms from there.
    let carried = attack_level(0.0, 1000.0, at_swap);
    let expect = attack_level(carried, 5.0, FADE_BLOCKS * BLOCK);
    assert!(
        (level as f64 - expect).abs() < 1e-4,
        "continuous: {level} vs {expect}"
    );
}

/// Held until sustain, release edited 1000 ms -> 5 ms before note-off.
fn release_after_edit(timing: f32) -> f32 {
    let collector = Collector::new();
    let h = collector.handle();
    let mut e = PatchEngine::new(&h, &env_patch(timing, 1.0, 1000.0), SR, 1).unwrap();
    e.note_on(0, 0.0, 1.0);
    run(&mut e, 20); // attack done, sustain at 1.0
    swap(&mut e, &h, &env_patch(timing, 1.0, 5.0));
    run(&mut e, FADE_BLOCKS);
    e.note_off(0);
    run(&mut e, 10)
}

#[test]
fn key_trigger_release_uses_the_release_captured_at_note_on() {
    let level = release_after_edit(1.0);
    let expect = coeff(1000.0).powi(10 * BLOCK as i32);
    assert!(
        (level as f64 - expect).abs() < 1e-4,
        "1000 ms release: {level} vs {expect}"
    );
}

#[test]
fn continuous_release_uses_the_current_release() {
    let level = release_after_edit(0.0);
    let expect = coeff(5.0).powi(10 * BLOCK as i32);
    assert!(
        (level as f64 - expect).abs() < 1e-4,
        "5 ms release: {level} vs {expect}"
    );
}

#[test]
fn key_trigger_retrigger_captures_fresh_values() {
    let collector = Collector::new();
    let h = collector.handle();
    let mut e = PatchEngine::new(&h, &env_patch(1.0, 1000.0, 1.0), SR, 1).unwrap();
    e.note_on(0, 0.0, 1.0);
    run(&mut e, 4);
    swap(&mut e, &h, &env_patch(1.0, 5.0, 1.0));
    run(&mut e, FADE_BLOCKS);
    e.note_off(0);
    run(&mut e, 20); // 1 ms release: back to idle
    e.note_on(0, 0.0, 1.0);
    let level = run(&mut e, 1);
    let expect = attack_level(0.0, 5.0, BLOCK);
    assert!(
        (level as f64 - expect).abs() < 1e-4,
        "new note captured 5 ms: {level} vs {expect}"
    );
}

#[test]
fn many_live_edits_during_a_held_note_never_retrigger() {
    // 60 overlapping edits of decay while a note sits in sustain: the level must stay at the
    // sustain value throughout (a retrigger would restart the attack from a lower level).
    let collector = Collector::new();
    let h = collector.handle();
    for timing in [0.0, 1.0] {
        let mut e = PatchEngine::new(&h, &env_patch(timing, 1.0, 300.0), SR, 1).unwrap();
        e.note_on(0, 0.0, 1.0);
        run(&mut e, 20);
        let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
        for i in 0..60 {
            let mut p = env_patch(timing, 1.0, 300.0);
            p.modules
                .get_mut(&ENV)
                .unwrap()
                .params
                .insert("decay_ms".into(), 50.0 + i as f32);
            swap(&mut e, &h, &p);
            for _ in 0..3 {
                e.process_block(&mut l, &mut r);
                let worst = l.iter().fold(0.0f32, |m, &s| m.max((s - 1.0).abs()));
                assert!(worst < 1e-6, "timing {timing}: off by {worst}");
            }
        }
    }
}

/// LFO modulating attack. Key-trigger must equal a patch with a fixed attack equal to the
/// modulated value at note-on; continuous must not.
fn lfo_modulated(timing: f32) -> Vec<f32> {
    let mut p = env_patch(timing, 100.0, 300.0);
    // Saw at 5 Hz starts at -1 at phase 0.
    p.modules
        .insert(9, module("lfo", &[("rate_hz", 5.0), ("waveform", 2.0)]));
    p.cables.insert(
        9,
        cable(
            (9, "out"),
            PortRef::Param {
                id: ENV,
                param: "attack_ms".into(),
            },
            &[("amount", 0.5)],
        ),
    );
    render(&p)
}

fn render(p: &PatchState) -> Vec<f32> {
    let mut c = compile(p, SR, 1).unwrap();
    c.note_on(0, 0.0, 1.0);
    let mut out = Vec::new();
    for _ in 0..200 {
        c.process_block();
        out.extend_from_slice(c.left());
    }
    out
}

#[test]
fn key_trigger_with_an_lfo_on_attack_equals_the_value_captured_at_note_on() {
    let info = kabl_modules::registry::info_for("env.adsr").unwrap().params[0];
    // At note-on the saw reads -1: attack = from_norm(to_norm(100 ms) - 0.5).
    let captured = info.from_norm(info.to_norm(100.0) - 0.5);
    let fixed = render(&env_patch(1.0, captured, 300.0));
    assert_eq!(
        lfo_modulated(1.0),
        fixed,
        "key-trigger holds the note-on attack"
    );
    assert_ne!(lfo_modulated(0.0), fixed, "continuous follows the LFO");
}

#[test]
fn switching_to_key_trigger_mid_note_captures_at_the_switch() {
    let collector = Collector::new();
    let h = collector.handle();
    let mut e = PatchEngine::new(&h, &env_patch(0.0, 1000.0, 300.0), SR, 1).unwrap();
    e.note_on(0, 0.0, 1.0);
    run(&mut e, 5);
    // Switch to key-trigger with attack 2000 ms: captured now, held until the next note.
    swap(&mut e, &h, &env_patch(1.0, 2000.0, 300.0));
    let l0 = run(&mut e, FADE_BLOCKS) as f64;
    swap(&mut e, &h, &env_patch(1.0, 5.0, 300.0));
    let l1 = run(&mut e, FADE_BLOCKS) as f64;
    let expect = attack_level(l0, 2000.0, FADE_BLOCKS * BLOCK);
    assert!((l1 - expect).abs() < 1e-4, "{l1} vs {expect}");
}
