//! MIDI notes reach the drum voices through the real engine: `midi.note` hears the note on its
//! sample, a gate is not averaged over voice lanes, a kick patched to the compressor's sidechain
//! ducks a pad, a patch with drums is deterministic, and nothing allocates.

use std::collections::BTreeMap;

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::Collector;
use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::{KeyEvent, MidiEvent, Source};
use kabl_engine::patch_engine::PatchEngine;

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;

fn module(kind: &str, params: &[(&str, f32)]) -> ModuleState {
    ModuleState {
        kind: kind.to_string(),
        pos: Vec2 { x: 0.0, y: 0.0 },
        params: params.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
    }
}

fn wire(p: &mut PatchState, a: (u64, &str), b: (u64, &str)) {
    let id = p.cables.len() as u64 + 1;
    let port = |(id, port): (u64, &str)| PortRef::Module {
        id,
        port: port.into(),
    };
    p.cables.insert(
        id,
        CableState {
            from: port(a),
            to: port(b),
            params: BTreeMap::new(),
            steps: Vec::new(),
        },
    );
}

/// midi.note 36 -> kick and midi.note 38 -> snare -> out (kick left, snare right).
fn kit() -> PatchState {
    let mut p = PatchState::default();
    p.modules.insert(1, module("midi.note", &[("note", 36.0)]));
    p.modules.insert(2, module("drum.kick", &[]));
    p.modules.insert(3, module("drum.snare", &[]));
    p.modules.insert(4, module("out", &[]));
    p.modules.insert(5, module("midi.note", &[("note", 38.0)]));
    wire(&mut p, (1, "gate"), (2, "gate"));
    wire(&mut p, (1, "velocity"), (2, "velocity"));
    wire(&mut p, (5, "gate"), (3, "gate"));
    wire(&mut p, (5, "velocity"), (3, "velocity"));
    wire(&mut p, (2, "out"), (4, "in"));
    wire(&mut p, (3, "out"), (4, "in"));
    p
}

fn engine(p: &PatchState) -> (Collector, PatchEngine) {
    let c = Collector::new();
    let e = PatchEngine::new(&c.handle(), p, SR, 8).unwrap();
    (c, e)
}

fn note(e: &mut PatchEngine, n: u8, v: u8, offset: usize) {
    e.key_at(
        MidiEvent::new(
            Source::Controller,
            9,
            KeyEvent::On {
                note: n,
                velocity: v,
            },
        ),
        offset,
    );
}

fn blocks(e: &mut PatchEngine, n: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for _ in 0..n {
        e.process_block(&mut l, &mut r);
        a.extend_from_slice(&l);
        b.extend_from_slice(&r);
    }
    (a, b)
}

fn first_sound(x: &[f32]) -> Option<usize> {
    x.iter().position(|v| v.abs() > 1e-6)
}

#[test]
fn a_note_starts_its_drum_on_the_sample_and_only_that_drum() {
    let (_c, mut e) = engine(&kit());
    blocks(&mut e, 2);
    note(&mut e, 36, 100, 21);
    let (l, r) = blocks(&mut e, 40);
    let at = first_sound(&l).expect("kick sounds");
    // The gate starts on the note's sample; the kick's sine is 0 on it and rises on the next.
    assert!((21..=23).contains(&at), "kick starts at {at}");
    assert!(first_sound(&r).is_none(), "snare stayed silent");
    note(&mut e, 38, 100, 40);
    let (l2, r2) = blocks(&mut e, 40);
    let at = first_sound(&r2).expect("snare sounds");
    assert!((40..=42).contains(&at), "snare starts at {at}");
    assert!(l2.iter().all(|v| v.is_finite()));
}

#[test]
fn velocity_travels_with_the_note() {
    let hit = |vel| {
        let (_c, mut e) = engine(&kit());
        blocks(&mut e, 2);
        note(&mut e, 36, vel, 0);
        let (l, _) = blocks(&mut e, 40);
        l.iter().fold(0f32, |m, v| m.max(v.abs()))
    };
    let (hard, soft) = (hit(127), hit(30));
    println!("kick peak: velocity 127 {hard:.3}, 30 {soft:.3}");
    assert!(soft < hard * 0.7);
}

#[test]
fn midi_drums_and_a_voiced_keyboard_patch_do_not_disturb_each_other() {
    let mut p = kit();
    p.modules.insert(6, module("midi.in", &[]));
    p.modules.insert(7, module("osc.va", &[]));
    p.modules.insert(8, module("vca", &[("gain", 0.0)]));
    wire(&mut p, (6, "pitch"), (7, "pitch"));
    wire(&mut p, (7, "out"), (8, "in"));
    wire(&mut p, (6, "gate"), (8, "cv"));
    wire(&mut p, (8, "out"), (4, "in"));
    let (_c, mut e) = engine(&p);
    blocks(&mut e, 2);
    note(&mut e, 36, 100, 0);
    let (l, _) = blocks(&mut e, 40);
    // midi.in hears note 36 as a key too (a voice plays), and the kick plays as well.
    assert!(first_sound(&l).is_some());
    assert!(l.iter().all(|v| v.is_finite()));
}

#[test]
fn the_kick_ducks_a_pad_through_the_compressor_sidechain() {
    let mut p = PatchState::default();
    p.modules.insert(1, module("midi.note", &[("note", 36.0)]));
    p.modules
        .insert(2, module("drum.kick", &[("level_db", -6.0)]));
    p.modules.insert(3, module("osc.va", &[]));
    p.modules.insert(
        4,
        module(
            "compressor",
            &[
                ("threshold_db", -30.0),
                ("ratio", 8.0),
                ("attack_ms", 1.0),
                ("release_ms", 150.0),
            ],
        ),
    );
    p.modules.insert(5, module("out", &[]));
    wire(&mut p, (1, "gate"), (2, "gate"));
    wire(&mut p, (3, "out"), (4, "in"));
    wire(&mut p, (2, "out"), (4, "sc"));
    wire(&mut p, (4, "out"), (5, "in"));
    let (_c, mut e) = engine(&p);
    blocks(&mut e, 100);
    let level = |x: &[f32]| (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
    let (before, _) = blocks(&mut e, 20);
    note(&mut e, 36, 127, 0);
    let (during, _) = blocks(&mut e, 40);
    let (after, _) = blocks(&mut e, 1000);
    let (b, d, a) = (
        level(&before),
        level(&during[800..2500]),
        level(&after[after.len() - 8000..]),
    );
    println!("pad rms: before {b:.4}, during kick {d:.4}, recovered {a:.4}");
    assert!(b > 0.01);
    assert!(d < b * 0.6, "the kick must duck the pad");
    assert!(a > b * 0.8, "the pad must come back");
}

#[test]
fn drums_allocate_nothing_and_render_the_same_twice() {
    let run = || {
        let (_c, mut e) = engine(&kit());
        blocks(&mut e, 2);
        let mut all = Vec::with_capacity(120);
        assert_no_alloc(|| {
            let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
            for b in 0..120 {
                if b % 20 == 0 {
                    note(&mut e, 36, 110, b % 7);
                    note(&mut e, 38, 90, 30);
                }
                e.process_block(&mut l, &mut r);
                all.push((l[10], r[10]));
            }
        });
        all
    };
    assert_eq!(run(), run());
}
