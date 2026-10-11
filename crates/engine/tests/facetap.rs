//! Face taps (`facetap.rs`): several outputs read at once for the live displays on module faces.
//! They report the right numbers and views, never change the sound and never allocate.

use std::collections::BTreeMap;

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::Collector;
use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::facetap::{FaceReport, FaceTarget, FaceTargets};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::{Command, PatchEngine};

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

fn cable(from: u64, fp: &str, to: u64, tp: &str) -> CableState {
    CableState {
        from: PortRef::Module {
            id: from,
            port: fp.into(),
        },
        to: PortRef::Module {
            id: to,
            port: tp.into(),
        },
        params: BTreeMap::new(),
        steps: Vec::new(),
    }
}

/// Macro m1 (0.3) → VCA #2 (×0.5) → VCA #3 (×0.5) → Output; an LFO beside it.
fn chain() -> PatchState {
    let mut p = PatchState::new();
    p.modules.insert(1, module("macro", &[("m1", 0.3)]));
    p.modules.insert(2, module("vca", &[("gain", 0.5)]));
    p.modules.insert(3, module("vca", &[("gain", 0.5)]));
    p.modules.insert(4, module("out", &[]));
    p.modules.insert(5, module("lfo", &[("rate_hz", 2.0)]));
    p.cables.insert(1, cable(1, "m1", 2, "in"));
    p.cables.insert(2, cable(2, "out", 3, "in"));
    p.cables.insert(3, cable(3, "out", 4, "left"));
    p
}

/// A keyboard voice: MIDI In → Osc → VCA ← ADSR, to Output.
fn keys() -> PatchState {
    let mut p = PatchState::new();
    p.modules.insert(1, module("midi.in", &[]));
    p.modules.insert(2, module("osc.wt", &[("position", 0.4)]));
    p.modules.insert(
        3,
        module("env.adsr", &[("attack_ms", 5.0), ("sustain", 0.5)]),
    );
    p.modules.insert(4, module("vca", &[("gain", 0.0)]));
    p.modules.insert(5, module("out", &[]));
    p.cables.insert(1, cable(1, "pitch", 2, "pitch"));
    p.cables.insert(2, cable(1, "gate", 3, "gate"));
    p.cables.insert(3, cable(2, "out", 4, "in"));
    p.cables.insert(4, cable(3, "out", 4, "cv"));
    p.cables.insert(5, cable(4, "out", 5, "left"));
    p.cables.insert(6, cable(4, "out", 5, "right"));
    p
}

fn t(module: u64, port: u8, kind: &'static str, view: bool) -> FaceTarget {
    FaceTarget {
        module,
        port,
        kind,
        view,
    }
}

fn engine(p: &PatchState, voices: usize) -> (Collector, PatchEngine) {
    let c = Collector::new();
    let e = PatchEngine::new(&c.handle(), p, SR, voices).unwrap();
    (c, e)
}

fn next_report(e: &mut PatchEngine) -> FaceReport {
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    for _ in 0..200 {
        assert_no_alloc(|| e.process_block(&mut l, &mut r));
        if let Some(rep) = e.take_face_report() {
            return rep;
        }
    }
    panic!("no face report in 200 blocks");
}

#[test]
fn one_report_reads_several_outputs_and_names_what_it_cannot_find() {
    let (_c, mut e) = engine(&chain(), 4);
    let targets = FaceTargets::new(
        7,
        &[
            t(1, 0, "macro", false),
            t(2, 0, "vca", false),
            t(3, 0, "vca", false),
            t(5, 0, "lfo", true),
            t(9, 0, "vca", false),
            t(2, 0, "lfo", false),
        ],
    );
    e.command(&Command::Faces(Some(targets)));
    let r = next_report(&mut e);
    assert_eq!((r.token, r.n), (7, 6));
    for (i, level) in [(0, 0.3f32), (1, 0.15), (2, 0.075)] {
        let en = &r.entries[i];
        assert!(en.found, "{i}");
        assert!(
            (en.last - level).abs() < 1e-6 && (en.min - level).abs() < 1e-6,
            "{en:?}"
        );
    }
    let lfo = &r.entries[3];
    assert!(lfo.found && lfo.view_valid);
    assert!(lfo.min < -0.1 && lfo.max > 0.1 || lfo.max - lfo.min > 0.01);
    assert!((0.0..1.0).contains(&lfo.position));
    assert!(!r.entries[4].found, "no module 9");
    assert!(!r.entries[5].found, "module 2 is not an lfo");
}

#[test]
fn views_follow_the_loudest_voice() {
    let (_c, mut e) = engine(&keys(), 8);
    e.command(&Command::Faces(Some(FaceTargets::new(
        1,
        &[t(2, 0, "osc.wt", true), t(3, 0, "env.adsr", true)],
    ))));
    let idle = next_report(&mut e);
    assert_eq!(idle.entries[1].position, 0.0, "envelope idle");
    e.key(KeyEvent::On {
        note: 60,
        velocity: 100,
    });
    next_report(&mut e);
    let r = next_report(&mut e);
    let (osc, env) = (&r.entries[0], &r.entries[1]);
    assert!(osc.view_valid && (osc.position - 0.4).abs() < 1e-3);
    assert!(osc.cycle.iter().any(|&c| c.abs() > 10));
    assert!(
        env.view_valid && env.position >= 1.0 && env.last > 0.1,
        "{env:?}"
    );
}

#[test]
fn the_sound_is_the_same_with_taps_on_and_nothing_allocates_across_swaps() {
    let render = |taps: bool| {
        let (c, mut e) = engine(&keys(), 4);
        let h = c.handle();
        let p = keys();
        let mut graphs: Vec<_> = (0..6).map(|_| e.build_swap(&h, &p).unwrap()).collect();
        if taps {
            e.command(&Command::Faces(Some(FaceTargets::new(
                3,
                &[
                    t(2, 0, "osc.wt", true),
                    t(3, 0, "env.adsr", true),
                    t(4, 0, "vca", false),
                ],
            ))));
        }
        e.key(KeyEvent::On {
            note: 64,
            velocity: 90,
        });
        let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
        let mut out = Vec::new();
        let mut reports = 0;
        for i in 0..600 {
            let g = (i % 100 == 0).then(|| graphs.pop()).flatten();
            assert_no_alloc(|| {
                if let Some(g) = g {
                    e.receive_swap(g);
                }
                if taps && i == 300 {
                    e.command(&Command::Faces(None));
                }
                e.process_block(&mut l, &mut r);
            });
            if e.take_face_report().is_some() {
                reports += 1;
            }
            out.extend_from_slice(&l);
        }
        (out, reports)
    };
    let (off, none) = render(false);
    let (on, some) = render(true);
    assert_eq!(none, 0);
    assert!(some > 10, "{some} reports across swaps");
    assert!(off == on, "taps changed the output");
}
