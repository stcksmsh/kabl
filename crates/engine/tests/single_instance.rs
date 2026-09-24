//! Single-instance compilation (voice-rate modules no `midi.in` reaches run once) against the
//! per-voice reference (`compile_per_voice`): MIDI through jacks and through modulation routes,
//! mixed global/per-voice graphs, feedback cycles, bypassed routes, voice independence, and
//! swaps that add or remove MIDI influence while playing (overlapping ones too).
//!
//! Equivalence is within 1e-5: the voice average of N identical lanes can round differently
//! from the one lane in its last bit. Where exact equivalence is impossible (per-voice →
//! single instance while voices differ), the defined behaviour is: the single instance continues
//! voice 0's state, the other voices' differences end with the swap's 15 ms crossfade, and the
//! output converges to the reference as the states converge.

use basedrop::Collector;
use kabl_core::{CableState, ModuleId, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::compile::{compile, compile_per_voice, CompiledPatch};
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::PatchEngine;

const SR: f32 = 48000.0;
const VOICES: usize = 8;

// Ids.
const CLOCK: ModuleId = 1;
const SEQ: ModuleId = 2;
const OSC_A: ModuleId = 3;
const FILT_A: ModuleId = 4;
const ENV_A: ModuleId = 5;
const VCA_A: ModuleId = 6;
const MIDI: ModuleId = 7;
const OSC_B: ModuleId = 8;
const ENV_B: ModuleId = 9;
const VCA_B: ModuleId = 10;
const MIX: ModuleId = 11;
const OUT: ModuleId = 12;
const LFO: ModuleId = 13;
const DELAY: ModuleId = 14;

fn port(id: ModuleId, p: &str) -> PortRef {
    PortRef::Module { id, port: p.into() }
}

fn param(id: ModuleId, p: &str) -> PortRef {
    PortRef::Param {
        id,
        param: p.into(),
    }
}

struct B(PatchState, u64);

impl B {
    fn m(&mut self, id: ModuleId, kind: &str, params: &[(&str, f32)]) -> &mut Self {
        self.0.modules.insert(
            id,
            ModuleState {
                kind: kind.into(),
                pos: Vec2::default(),
                params: params.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
            },
        );
        self
    }
    fn c(&mut self, from: PortRef, to: PortRef) -> &mut Self {
        self.cp(from, to, &[])
    }
    fn cp(&mut self, from: PortRef, to: PortRef, params: &[(&str, f32)]) -> &mut Self {
        self.1 += 1;
        self.0.cables.insert(
            self.1,
            CableState {
                from,
                to,
                params: params.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
                steps: vec![],
            },
        );
        self
    }
}

/// A sequenced voice (global-driven: single instance) and a MIDI voice, mixed, with an LFO on
/// the sequenced filter, a stereo delay, and a feedback cable from the sequenced VCA back into
/// its filter's cutoff CV (a cycle inside the single-instance chain).
fn mixed() -> PatchState {
    let mut b = B(PatchState::new(), 0);
    b.m(CLOCK, "clock", &[("bpm", 150.0)])
        .m(
            SEQ,
            "seq",
            &[("gate_mode", 1.0), ("gate_len", 40.0), ("v2", 30.0)],
        )
        .m(OSC_A, "osc.va", &[("waveform", 2.0)])
        .m(
            FILT_A,
            "filter.svf",
            &[("cutoff_hz", 800.0), ("resonance", 0.5)],
        )
        .m(ENV_A, "env.adsr", &[("decay_ms", 90.0), ("sustain", 0.2)])
        .m(VCA_A, "vca", &[("gain", 0.0)])
        .m(MIDI, "midi.in", &[])
        .m(OSC_B, "osc.va", &[("waveform", 3.0)])
        .m(ENV_B, "env.adsr", &[("release_ms", 200.0)])
        .m(VCA_B, "vca", &[("gain", 0.0)])
        .m(MIX, "mixer", &[("level1", 0.5), ("level2", 0.5)])
        .m(OUT, "out", &[])
        .m(LFO, "lfo", &[("rate_hz", 3.0)])
        .m(
            DELAY,
            "delay",
            &[("sync", 0.0), ("time_ms", 90.0), ("feedback", 50.0)],
        );
    b.c(port(CLOCK, "gate"), port(SEQ, "clock"))
        .c(port(SEQ, "pitch"), port(OSC_A, "pitch"))
        .c(port(SEQ, "gate"), port(ENV_A, "gate"))
        .c(port(OSC_A, "out"), port(FILT_A, "in"))
        .c(port(FILT_A, "lp"), port(VCA_A, "in"))
        .c(port(ENV_A, "out"), port(VCA_A, "cv"))
        .c(port(MIDI, "pitch"), port(OSC_B, "pitch"))
        .c(port(MIDI, "gate"), port(ENV_B, "gate"))
        .c(port(OSC_B, "out"), port(VCA_B, "in"))
        .c(port(ENV_B, "out"), port(VCA_B, "cv"))
        .c(port(VCA_A, "out"), port(MIX, "in1"))
        .c(port(VCA_B, "out"), port(MIX, "in2"))
        .c(port(VCA_A, "out"), port(FILT_A, "cutoff_cv"))
        .c(port(MIX, "out"), port(DELAY, "in"))
        .c(port(DELAY, "left"), port(OUT, "left"))
        .c(port(DELAY, "right"), port(OUT, "right"))
        .cp(
            port(LFO, "out"),
            param(FILT_A, "cutoff_hz"),
            &[("amount", 0.2)],
        );
    b.0
}

/// `mixed()` plus a MIDI velocity route onto the sequenced filter: MIDI now reaches it (and
/// the VCA, the cycle and the mixer after it).
fn with_velocity_route(bypass: bool) -> PatchState {
    let mut p = mixed();
    p.cables.insert(
        500,
        CableState {
            from: port(MIDI, "velocity"),
            to: param(FILT_A, "cutoff_hz"),
            params: [
                ("amount".to_string(), 0.5),
                ("bypass".to_string(), bypass as u8 as f32),
            ]
            .into(),
            steps: vec![],
        },
    );
    p
}

/// `mixed()` with the MIDI pitch cable moved onto the sequenced oscillator (MIDI through a
/// jack into the single-instance chain).
fn midi_pitch_on_osc_a() -> PatchState {
    let mut p = mixed();
    let seq_pitch = *p
        .cables
        .iter()
        .find(|(_, c)| c.to == port(OSC_A, "pitch"))
        .unwrap()
        .0;
    p.cables.get_mut(&seq_pitch).unwrap().from = port(MIDI, "pitch");
    p
}

/// Note events (block, voice, semitones or None for off). Voices get different notes and
/// velocities, so a per-voice chain really differs between voices.
const NOTES: &[(usize, usize, Option<(f32, f32)>)] = &[
    (20, 0, Some((0.0, 1.0))),
    (60, 3, Some((7.0, 0.4))),
    (140, 5, Some((-5.0, 0.7))),
    (220, 0, None),
    (300, 3, None),
    (330, 5, None),
];

fn render(mut c: CompiledPatch, blocks: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(blocks * BLOCK * 2);
    for b in 0..blocks {
        for &(at, v, e) in NOTES {
            if at == b {
                match e {
                    Some((st, vel)) => c.note_on(v, st, vel),
                    None => c.note_off(v),
                }
            }
        }
        c.process_block();
        out.extend_from_slice(c.left());
        out.extend_from_slice(c.right());
    }
    out
}

fn max_diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f32::max)
}

fn peak(v: &[f32]) -> f32 {
    v.iter().fold(0.0, |m, x| m.max(x.abs()))
}

fn equivalent(name: &str, p: &PatchState) {
    let fast = render(compile(p, SR, VOICES).unwrap(), 500);
    let reference = render(compile_per_voice(p, SR, VOICES).unwrap(), 500);
    let d = max_diff(&fast, &reference);
    println!(
        "{name}: max difference {d:.2e}, peak {:.3}",
        peak(&reference)
    );
    assert!(peak(&reference) > 0.01, "{name}: silent test");
    assert!(fast.iter().all(|x| x.is_finite()));
    assert!(d < 1e-5, "{name}: differs from per-voice by {d}");
}

#[test]
fn single_instance_matches_per_voice_on_every_graph_shape() {
    equivalent("mixed graph with a feedback cycle", &mixed());
    equivalent(
        "MIDI velocity route into the sequenced chain",
        &with_velocity_route(false),
    );
    equivalent("bypassed MIDI route", &with_velocity_route(true));
    equivalent(
        "MIDI pitch jack into the sequenced chain",
        &midi_pitch_on_osc_a(),
    );
}

#[test]
fn only_what_midi_reaches_is_per_voice() {
    let mut c = compile(&mixed(), SR, VOICES).unwrap();
    for id in [OSC_A, FILT_A, ENV_A, VCA_A, LFO] {
        assert!(c.module_mut(id, None).is_some(), "{id} single");
    }
    for id in [MIDI, OSC_B, ENV_B, VCA_B, MIX] {
        assert!(
            c.module_mut(id, Some(VOICES - 1)).is_some(),
            "{id} per voice"
        );
    }
    // A bypassed route carries nothing, so it doesn't make its target per voice.
    let mut c = compile(&with_velocity_route(true), SR, VOICES).unwrap();
    assert!(c.module_mut(FILT_A, None).is_some());
    let mut c = compile(&with_velocity_route(false), SR, VOICES).unwrap();
    assert!(c.module_mut(FILT_A, Some(0)).is_some());
    // MIDI reaching the cycle makes the whole cycle per voice.
    for id in [OSC_A, FILT_A, VCA_A] {
        let mut c = compile(&midi_pitch_on_osc_a(), SR, VOICES).unwrap();
        assert!(c.module_mut(id, Some(0)).is_some(), "{id}");
    }
}

#[test]
fn voices_stay_independent() {
    // One note on voice 3 only: every other MIDI voice stays silent in both compilers, and
    // the sequenced chain is the same whatever the MIDI voices do.
    let p = with_velocity_route(false);
    let mut fast = compile(&p, SR, VOICES).unwrap();
    let mut reference = compile_per_voice(&p, SR, VOICES).unwrap();
    fast.note_on(3, 12.0, 0.9);
    reference.note_on(3, 12.0, 0.9);
    for _ in 0..200 {
        fast.process_block();
        reference.process_block();
        assert!(max_diff(fast.left(), reference.left()) < 1e-5);
    }
}

/// Engine run with swaps at (block, patch), every graph from `build`.
fn run_swaps(
    start: &PatchState,
    swaps: &[(usize, PatchState)],
    blocks: usize,
    build: fn(&PatchState, f32, usize) -> Result<CompiledPatch, kabl_engine::compile::CompileError>,
) -> Vec<f32> {
    let collector = Collector::new();
    let handle = collector.handle();
    let mut e = PatchEngine::with_compiled(&handle, build(start, SR, VOICES).unwrap(), VOICES);
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    let mut out = Vec::new();
    for b in 0..blocks {
        for (_, p) in swaps.iter().filter(|(at, _)| *at == b) {
            e.receive_swap(basedrop::Owned::new(&handle, build(p, SR, VOICES).unwrap()));
        }
        for &(at, v, ev) in NOTES {
            if at == b {
                match ev {
                    Some((st, vel)) => e.note_on(v, st, vel),
                    None => e.note_off(v),
                }
            }
        }
        e.process_block(&mut l, &mut r);
        out.extend_from_slice(&l);
    }
    out
}

#[test]
fn adding_midi_influence_while_playing_matches_the_reference() {
    // Single → per voice: every voice inherits the one instance's state, which every
    // reference lane had too, so the output stays equivalent through the swap.
    let swaps = [(100, with_velocity_route(false))];
    let fast = run_swaps(&mixed(), &swaps, 500, compile);
    let reference = run_swaps(&mixed(), &swaps, 500, compile_per_voice);
    let d = max_diff(&fast, &reference);
    println!("adding MIDI influence: max difference {d:.2e}");
    assert!(d < 1e-5);
}

#[test]
fn removing_midi_influence_continues_voice_zero_and_converges() {
    // Per voice → single, while voices 0, 3 and 5 play different notes: the single instance
    // continues voice 0. It can't equal the reference's average of eight different lanes, but
    // it has no click and converges once the lanes' inputs agree.
    let swaps = [(160, mixed())];
    let start = with_velocity_route(false);
    let fast = run_swaps(&start, &swaps, 1500, compile);
    let reference = run_swaps(&start, &swaps, 1500, compile_per_voice);
    let at = 160 * BLOCK;
    assert!(max_diff(&fast[..at], &reference[..at]) < 1e-5);
    let step = |v: &[f32]| v.windows(2).fold(0f32, |m, w| m.max((w[1] - w[0]).abs()));
    let around = at - 2 * BLOCK..at + 20 * BLOCK;
    println!(
        "removing MIDI influence: step at swap {:.4} (reference {:.4}), difference 1 s later {:.2e}",
        step(&fast[around.clone()]),
        step(&reference[around.clone()]),
        max_diff(&fast[at + 750 * BLOCK..], &reference[at + 750 * BLOCK..])
    );
    assert!(step(&fast[around.clone()]) < 2.0 * step(&reference[around]) + 1e-3);
    assert!(max_diff(&fast[at + 750 * BLOCK..], &reference[at + 750 * BLOCK..]) < 1e-3);
}

#[test]
fn overlapping_transition_swaps_stay_finite_and_converge() {
    // Three swaps landing together and back to back, alternating the two shapes.
    let swaps = [
        (100, with_velocity_route(false)),
        (100, mixed()),
        (101, with_velocity_route(false)),
        (110, mixed()),
        (110, midi_pitch_on_osc_a()),
        (111, mixed()),
    ];
    let fast = run_swaps(&mixed(), &swaps, 1200, compile);
    let reference = run_swaps(&mixed(), &swaps, 1200, compile_per_voice);
    assert!(fast.iter().all(|x| x.is_finite()));
    let tail = 900 * BLOCK;
    let d = max_diff(&fast[tail..], &reference[tail..]);
    println!("overlapping transitions: difference after 8 s {d:.2e}");
    assert!(d < 1e-3);
}
