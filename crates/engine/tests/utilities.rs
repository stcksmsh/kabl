//! The utility modules through the real compiler: a clocked random melody held to a scale,
//! per-voice vs shared behaviour, a patch of all of them allocating nothing, and edits that
//! apply in place.

use std::collections::BTreeMap;

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::Collector;
use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::PatchEngine;
use kabl_engine::probe::{ProbeReport, ProbeTarget};

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

fn cable(from: (u64, &str), to: (u64, &str)) -> CableState {
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

fn wire(p: &mut PatchState, a: (u64, &str), b: (u64, &str)) {
    let id = p.cables.len() as u64 + 1;
    p.cables.insert(id, cable(a, b));
}

fn engine(p: &PatchState, voices: usize) -> (Collector, PatchEngine) {
    let c = Collector::new();
    let e = PatchEngine::new(&c.handle(), p, SR, voices).unwrap();
    (c, e)
}

fn next_report(e: &mut PatchEngine) -> ProbeReport {
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    for _ in 0..200 {
        e.process_block(&mut l, &mut r);
        if let Some(rep) = e.take_probe_report() {
            return rep;
        }
    }
    panic!("no report");
}

fn probe(e: &mut PatchEngine, module: u64, kind: &'static str, port: u8) {
    e.inspect(Some(ProbeTarget {
        token: 1,
        module,
        port,
        kind,
    }));
}

/// clock -> random -> quantizer (minor, root D) -> osc.va -> out.
fn melody() -> PatchState {
    let mut p = PatchState::new();
    p.modules.insert(1, module("clock", &[("bpm", 480.0)]));
    p.modules
        .insert(2, module("random", &[("range", 12.0), ("length", 0.0)]));
    p.modules
        .insert(3, module("quantizer", &[("scale", 2.0), ("root", 2.0)]));
    p.modules.insert(4, module("osc.va", &[]));
    p.modules.insert(5, module("out", &[]));
    wire(&mut p, (1, "gate"), (2, "clock"));
    wire(&mut p, (2, "out"), (3, "in"));
    wire(&mut p, (3, "out"), (4, "pitch"));
    wire(&mut p, (4, "out"), (5, "left"));
    wire(&mut p, (4, "out"), (5, "right"));
    p
}

#[test]
fn a_clocked_random_melody_stays_in_the_scale_and_is_the_same_every_time() {
    let notes = || {
        let (_c, mut e) = engine(&melody(), 4);
        probe(&mut e, 3, "quantizer", 0);
        let mut v = Vec::new();
        for _ in 0..120 {
            let r = next_report(&mut e);
            assert!(!r.voiced, "a global clock gives one shared instance");
            v.push(r.lane[0].last);
        }
        v
    };
    let a = notes();
    assert_eq!(a, notes(), "the same patch plays the same melody");
    let minor_d = [0, 2, 3, 5, 7, 8, 10];
    for &n in &a {
        assert!(
            minor_d.contains(&(((n.round() as i32) - 2).rem_euclid(12)))
                && (n - n.round()).abs() < 1e-4,
            "{n} is not in D minor"
        );
    }
    let distinct: std::collections::BTreeSet<i32> = a.iter().map(|&n| n.round() as i32).collect();
    assert!(
        distinct.len() >= 5,
        "only {} different notes",
        distinct.len()
    );
    assert!(
        a.iter().all(|&n| (-14.0..=14.0).contains(&n)),
        "range 12 spans ±12"
    );
}

#[test]
fn a_global_clock_shares_one_value_and_a_key_clock_gives_each_voice_its_own() {
    // midi.in gate -> random clock: one stream per voice.
    let mut p = PatchState::new();
    p.modules.insert(1, module("midi.in", &[]));
    p.modules.insert(2, module("random", &[]));
    p.modules.insert(3, module("vca", &[]));
    p.modules.insert(4, module("out", &[]));
    wire(&mut p, (1, "gate"), (2, "clock"));
    wire(&mut p, (2, "out"), (3, "in"));
    wire(&mut p, (3, "out"), (4, "left"));
    let (_c, mut e) = engine(&p, 4);
    probe(&mut e, 2, "random", 0);
    for note in [60, 64, 67] {
        e.key(KeyEvent::On {
            note,
            velocity: 100,
        });
    }
    let r = next_report(&mut e);
    assert!(r.voiced && r.lanes == 4);
    let values: Vec<f32> = (0..4).map(|l| r.lane[l].last).collect();
    let held: Vec<f32> = values.iter().copied().filter(|&v| v != 0.0).collect();
    assert_eq!(held.len(), 3, "three keys, three values: {values:?}");
    assert!(
        held[0] != held[1] && held[1] != held[2] && held[0] != held[2],
        "{held:?}"
    );
}

/// All nine utilities in one voice chain driven by a key.
fn everything() -> PatchState {
    let mut p = PatchState::new();
    p.modules.insert(1, module("midi.in", &[]));
    p.modules.insert(
        2,
        module(
            "random",
            &[("length", 4.0), ("change", 20.0), ("range", 12.0)],
        ),
    );
    p.modules.insert(3, module("sample.hold", &[]));
    p.modules.insert(4, module("quantizer", &[("scale", 9.0)]));
    p.modules
        .insert(5, module("slew", &[("rise_ms", 20.0), ("fall_ms", 40.0)]));
    p.modules.insert(
        6,
        module("attenuverter", &[("amount", 0.5), ("offset", 0.1)]),
    );
    p.modules
        .insert(7, module("comparator", &[("threshold", 0.0)]));
    p.modules.insert(8, module("logic", &[]));
    p.modules.insert(9, module("osc.va", &[]));
    p.modules.insert(10, module("osc.va", &[]));
    p.modules
        .insert(11, module("crossfade", &[("mix", 0.4), ("curve", 0.5)]));
    p.modules.insert(12, module("pan", &[("pan", 0.3)]));
    p.modules.insert(13, module("out", &[]));
    wire(&mut p, (1, "gate"), (2, "clock"));
    wire(&mut p, (1, "gate"), (2, "reset"));
    wire(&mut p, (2, "out"), (3, "in"));
    wire(&mut p, (1, "gate"), (3, "clock"));
    wire(&mut p, (3, "out"), (4, "in"));
    wire(&mut p, (4, "out"), (5, "in"));
    wire(&mut p, (5, "out"), (6, "in"));
    wire(&mut p, (6, "out"), (7, "in"));
    wire(&mut p, (7, "out"), (8, "a"));
    wire(&mut p, (1, "gate"), (8, "b"));
    wire(&mut p, (1, "pitch"), (9, "pitch"));
    wire(&mut p, (4, "out"), (10, "pitch"));
    wire(&mut p, (9, "out"), (11, "a"));
    wire(&mut p, (10, "out"), (11, "b"));
    wire(&mut p, (8, "and"), (11, "fade"));
    wire(&mut p, (11, "out"), (12, "in"));
    wire(&mut p, (5, "out"), (12, "pan"));
    wire(&mut p, (12, "left"), (13, "left"));
    wire(&mut p, (12, "right"), (13, "right"));
    p
}

#[test]
fn all_nine_utilities_in_eight_voices_allocate_nothing() {
    let (_c, mut e) = engine(&everything(), 8);
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    for note in 48..56 {
        e.key(KeyEvent::On { note, velocity: 90 });
    }
    let mut peak = 0.0f32;
    for b in 0..1500 {
        if b == 700 {
            for note in 48..56 {
                e.key(KeyEvent::Off { note });
            }
        }
        assert_no_alloc(|| e.process_block(&mut l, &mut r));
        assert!(l.iter().chain(&r).all(|v| v.is_finite()));
        peak = peak.max(l.iter().fold(0.0, |m, v| f32::max(m, v.abs())));
    }
    assert!(peak > 0.05, "the chain makes sound: {peak}");
}

#[test]
fn utility_params_apply_in_place() {
    use kabl_engine::runtime::{runtime_changes, RuntimeTarget};
    let base = everything();
    for (id, param, v) in [
        (2, "length", 8.0),
        (2, "change", 60.0),
        (4, "scale", 3.0),
        (4, "transpose", 7.0),
        (5, "rise_ms", 300.0),
        (5, "mode", 1.0),
        (6, "amount", -1.0),
        (7, "threshold", 0.3),
        (11, "mix", 0.9),
        (12, "pan", -0.7),
    ] {
        let mut edited = base.clone();
        edited
            .modules
            .get_mut(&id)
            .unwrap()
            .params
            .insert(param.into(), v);
        let changes = runtime_changes(&base, &edited)
            .unwrap_or_else(|| panic!("{param} should apply without a compile"));
        assert!(matches!(changes[0].0, RuntimeTarget::Param { .. }));
    }
}
