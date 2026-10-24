//! Stereo cables, step 1 (docs/stereo/DESIGN.md): width comes from the source port, a module with
//! an audio input fed by a stereo cable runs once per channel, a mono cable into a stereo input
//! is broadcast and a stereo cable into a mono input is averaged. Stereo is spelled with the pair
//! aliases `out_st` / `in_st` over the existing half ports until the port change replaces them.
//!
//! Every stereo patch here has an equivalent built from two mono chains and half cables; the two
//! must render the same samples exactly.

use std::collections::BTreeMap;

use basedrop::Collector;
use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::compile::{carry_state, compile};
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::PatchEngine;

const NOISE: u64 = 1;
const PAN: u64 = 2;
const SVF: u64 = 3;
const GAIN: u64 = 4;
const OUT: u64 = 5;
/// Second chain of a two-mono-chains reference.
const SVF_R: u64 = 6;
const GAIN_R: u64 = 7;

fn module(kind: &str, params: &[(&str, f32)]) -> ModuleState {
    ModuleState {
        kind: kind.to_string(),
        pos: Vec2 { x: 0.0, y: 0.0 },
        params: params.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
    }
}

fn wire(p: &mut PatchState, from: u64, fp: &str, to: u64, tp: &str) {
    let port = |id, p: &str| PortRef::Module { id, port: p.into() };
    let id = p.cables.len() as u64 + 1;
    p.cables.insert(
        id,
        CableState {
            from: port(from, fp),
            to: port(to, tp),
            params: BTreeMap::new(),
            steps: Vec::new(),
        },
    );
}

fn base(pan: f32) -> PatchState {
    let mut p = PatchState::new();
    p.modules.insert(NOISE, module("noise", &[]));
    p.modules.insert(PAN, module("pan", &[("pan", pan)]));
    p.modules.insert(OUT, module("out", &[]));
    wire(&mut p, NOISE, "out", PAN, "in");
    p
}

fn add_chain(p: &mut PatchState, svf: u64, gain: u64) {
    p.modules
        .insert(svf, module("filter.svf", &[("cutoff_hz", 900.0)]));
    p.modules.insert(gain, module("gain", &[("gain_db", -3.0)]));
    wire(p, svf, "lp", gain, "in");
}

fn render(p: &PatchState, blocks: usize) -> (Vec<f32>, Vec<f32>) {
    let c = Collector::new();
    let mut e = PatchEngine::new(&c.handle(), p, 48000.0, 1).unwrap();
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    let (mut ls, mut rs) = (Vec::new(), Vec::new());
    for _ in 0..blocks {
        e.process_block(&mut l, &mut r);
        ls.extend_from_slice(&l);
        rs.extend_from_slice(&r);
    }
    (ls, rs)
}

#[test]
fn a_stereo_chain_equals_two_mono_chains() {
    // Stereo: pan -> svf -> gain -> out as three single cables.
    let mut st = base(0.6);
    add_chain(&mut st, SVF, GAIN);
    wire(&mut st, PAN, "out_st", SVF, "in");
    wire(&mut st, GAIN, "out", OUT, "in_st");
    // Reference: a chain per side, half cables.
    let mut mono = base(0.6);
    add_chain(&mut mono, SVF, GAIN);
    add_chain(&mut mono, SVF_R, GAIN_R);
    wire(&mut mono, PAN, "left", SVF, "in");
    wire(&mut mono, PAN, "right", SVF_R, "in");
    wire(&mut mono, GAIN, "out", OUT, "left");
    wire(&mut mono, GAIN_R, "out", OUT, "right");

    let (sl, sr) = render(&st, 40);
    let (ml, mr) = render(&mono, 40);
    assert!(sl.iter().any(|&x| x != 0.0), "silent");
    assert_ne!(sl, sr, "pan 0.6 must make the channels differ");
    assert_eq!(sl, ml);
    assert_eq!(sr, mr);
}

#[test]
fn a_mono_cable_into_a_stereo_input_plays_on_both_sides() {
    let mut st = base(0.0);
    st.modules.remove(&PAN);
    st.cables.clear();
    wire(&mut st, NOISE, "out", OUT, "in_st");
    let mut mono = st.clone();
    mono.cables.clear();
    wire(&mut mono, NOISE, "out", OUT, "left");
    wire(&mut mono, NOISE, "out", OUT, "right");

    let (sl, sr) = render(&st, 10);
    let (ml, mr) = render(&mono, 10);
    assert!(sl.iter().any(|&x| x != 0.0), "silent");
    assert_eq!((&sl, &sr), (&ml, &mr));
    assert_eq!(sl, sr);
}

#[test]
fn a_stereo_cable_into_a_mono_input_is_averaged() {
    // Pan hard right: the sides differ, and the mono input gets (L + R) / 2.
    let mut st = base(1.0);
    st.modules.insert(GAIN, module("gain", &[("gain_db", 0.0)]));
    wire(&mut st, PAN, "out_st", GAIN, "in");
    wire(&mut st, GAIN, "out", OUT, "left");
    let mut sides = base(1.0);
    wire(&mut sides, PAN, "left", OUT, "left");
    wire(&mut sides, PAN, "right", OUT, "right");

    let (avg, _) = render(&st, 10);
    let (l, r) = render(&sides, 10);
    assert!(r.iter().any(|&x| x != 0.0), "silent");
    assert_ne!(l, r);
    for i in 0..avg.len() {
        assert!((avg[i] - (l[i] + r[i]) * 0.5).abs() < 1e-6, "sample {i}");
    }
}

#[test]
fn a_stereo_chain_fed_equal_channels_collapses_to_the_mono_sound() {
    // Centre pan gives L == R, so averaging the stereo chain's output is the mono chain.
    let mut st = base(0.0);
    add_chain(&mut st, SVF, GAIN);
    st.modules
        .insert(GAIN_R, module("gain", &[("gain_db", 0.0)]));
    wire(&mut st, PAN, "out_st", SVF, "in");
    wire(&mut st, GAIN, "out", GAIN_R, "in");
    wire(&mut st, GAIN_R, "out", OUT, "left");
    let mut mono = base(0.0);
    add_chain(&mut mono, SVF, GAIN);
    wire(&mut mono, PAN, "left", SVF, "in");
    wire(&mut mono, GAIN, "out", OUT, "left");

    let (a, _) = render(&st, 20);
    let (b, _) = render(&mono, 20);
    assert!(a.iter().any(|&x| x != 0.0), "silent");
    assert_eq!(a, b);
}

#[test]
fn a_second_channel_starts_from_the_first_ones_state() {
    // A mono chain edited to stereo while it plays: the new right instance takes the left
    // instance's filter state, so the first block after the edit has no step.
    let mut mono = base(0.0);
    add_chain(&mut mono, SVF, GAIN);
    wire(&mut mono, PAN, "left", SVF, "in");
    wire(&mut mono, GAIN, "out", OUT, "left");
    let mut st = base(0.0);
    add_chain(&mut st, SVF, GAIN);
    wire(&mut st, PAN, "out_st", SVF, "in");
    wire(&mut st, GAIN, "out", OUT, "in_st");

    let mut old = compile(&mono, 48000.0, 1).unwrap();
    for _ in 0..30 {
        old.process_block();
    }
    let mut new = compile(&st, 48000.0, 1).unwrap();
    carry_state(&mut old, &mut new);
    old.process_block();
    new.process_block();
    for i in 0..BLOCK {
        assert_eq!(new.left()[i], new.right()[i]);
        assert!((new.left()[i] - old.left()[i]).abs() < 1e-6, "sample {i}");
    }
}

#[test]
fn a_feedback_loop_through_a_stereo_cable_compiles_and_runs() {
    // gain -> out_st-fed svf -> gain: the back edge is a stereo delayed cable.
    let mut p = base(0.4);
    p.modules
        .insert(SVF, module("filter.svf", &[("cutoff_hz", 700.0)]));
    p.modules.insert(GAIN, module("mixer", &[]));
    wire(&mut p, PAN, "out_st", GAIN, "in1");
    wire(&mut p, SVF, "lp", GAIN, "in2");
    wire(&mut p, GAIN, "out", SVF, "in");
    wire(&mut p, GAIN, "out", OUT, "in_st");
    let (l, r) = render(&p, 20);
    assert!(l.iter().chain(&r).all(|x| x.is_finite()));
    assert!(l.iter().any(|&x| x != 0.0));
}

/// Voice chains: a stereo chain per voice, summed per channel, equals two mono chains.
#[test]
fn voices_sum_per_channel() {
    let build = |stereo: bool| {
        let mut p = PatchState::new();
        p.modules.insert(10, module("midi.in", &[]));
        p.modules.insert(11, module("osc.va", &[("waveform", 0.0)]));
        p.modules.insert(PAN, module("pan", &[("pan", -0.7)]));
        p.modules.insert(OUT, module("out", &[]));
        add_chain(&mut p, SVF, GAIN);
        wire(&mut p, 10, "pitch", 11, "pitch");
        wire(&mut p, 11, "out", PAN, "in");
        if stereo {
            wire(&mut p, PAN, "out_st", SVF, "in");
            wire(&mut p, GAIN, "out", OUT, "in_st");
        } else {
            add_chain(&mut p, SVF_R, GAIN_R);
            wire(&mut p, PAN, "left", SVF, "in");
            wire(&mut p, PAN, "right", SVF_R, "in");
            wire(&mut p, GAIN, "out", OUT, "left");
            wire(&mut p, GAIN_R, "out", OUT, "right");
        }
        let mut c = compile(&p, 48000.0, 4).unwrap();
        c.note_on(0, 0.0, 1.0);
        c.note_on(1, 7.0, 0.8);
        c.note_on(2, 12.0, 0.6);
        let mut out = (Vec::new(), Vec::new());
        for _ in 0..30 {
            c.process_block();
            out.0.extend_from_slice(c.left());
            out.1.extend_from_slice(c.right());
        }
        out
    };
    let (sl, sr) = build(true);
    let (ml, mr) = build(false);
    assert!(sl.iter().any(|&x| x != 0.0), "silent");
    assert_ne!(sl, sr);
    assert_eq!((sl, sr), (ml, mr));
}
