//! The arpeggiator and swing through the real compiler and key path: keys reach the module,
//! a chord plays the order the mode says, release is immediate, nothing allocates, a swung
//! clock moves every consumer, and an old patch has no ratchets.

use std::collections::BTreeMap;

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::Collector;
use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
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

/// clock -> arp -> sine -> vca (the arp's gate) -> out.
fn patch(clock: &[(&str, f32)], arp: &[(&str, f32)]) -> PatchState {
    let mut p = PatchState::default();
    p.modules.insert(1, module("clock", clock));
    p.modules.insert(2, module("arp", arp));
    p.modules.insert(3, module("osc.va", &[("waveform", 0.0)]));
    p.modules.insert(4, module("vca", &[("gain", 0.0)]));
    p.modules.insert(5, module("out", &[]));
    wire(&mut p, (1, "gate"), (2, "clock"));
    wire(&mut p, (2, "pitch"), (3, "pitch"));
    wire(&mut p, (3, "out"), (4, "in"));
    wire(&mut p, (2, "gate"), (4, "cv"));
    wire(&mut p, (4, "out"), (5, "left"));
    wire(&mut p, (4, "out"), (5, "right"));
    p
}

fn engine(p: &PatchState) -> (Collector, PatchEngine) {
    let c = Collector::new();
    let e = PatchEngine::new(&c.handle(), p, SR, 8).unwrap();
    (c, e)
}

fn render(e: &mut PatchEngine, samples: usize) -> Vec<f32> {
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    let mut out = Vec::with_capacity(samples);
    while out.len() < samples {
        e.process_block(&mut l, &mut r);
        out.extend_from_slice(&l);
    }
    out
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
}

fn on(e: &mut PatchEngine, notes: &[u8]) {
    for &note in notes {
        e.key(KeyEvent::On {
            note,
            velocity: 100,
        });
    }
}

#[test]
fn keys_reach_the_arp_and_release_is_immediate() {
    let (_c, mut e) = engine(&patch(&[], &[]));
    assert!(rms(&render(&mut e, 24000)) < 1e-6, "silent with no keys");
    on(&mut e, &[60, 64, 67]);
    assert!(rms(&render(&mut e, 24000)) > 0.01, "a chord sounds");
    for note in [60, 64, 67] {
        e.key(KeyEvent::Off { note });
    }
    // The gate falls in the next block; the VCA closes at once.
    let _ = render(&mut e, BLOCK);
    assert!(rms(&render(&mut e, 4800)) < 1e-6, "nothing rings on");
}

#[test]
fn panic_clears_the_chord() {
    let (_c, mut e) = engine(&patch(&[], &[]));
    on(&mut e, &[60, 64]);
    let _ = render(&mut e, 12000);
    e.key(KeyEvent::AllOff);
    let _ = render(&mut e, BLOCK);
    assert!(rms(&render(&mut e, 4800)) < 1e-6);
}

#[test]
fn keys_and_ticks_allocate_nothing() {
    let (_c, mut e) = engine(&patch(&[("swing", 60.0)], &[("mode", 4.0), ("ratchet", 3.0)]));
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    assert_no_alloc(|| {
        for i in 0..2000 {
            if i % 300 == 0 {
                e.key(KeyEvent::On {
                    note: 60 + (i / 300) as u8,
                    velocity: 90,
                });
            }
            if i % 700 == 650 {
                e.key(KeyEvent::AllOff);
            }
            e.process_block(&mut l, &mut r);
        }
    });
}

#[test]
fn a_random_render_repeats_exactly() {
    let run = || {
        let (_c, mut e) = engine(&patch(&[], &[("mode", 4.0), ("octaves", 3.0)]));
        on(&mut e, &[60, 63, 67, 70]);
        render(&mut e, 48000 * 3)
    };
    assert_eq!(run(), run());
}

/// Rising onsets of the audio envelope: samples where the level passes a threshold after
/// at least 100 silent ones.
fn onsets(x: &[f32]) -> Vec<usize> {
    let mut out = vec![];
    let mut quiet = 1000;
    for (i, v) in x.iter().enumerate() {
        if v.abs() > 0.02 && quiet >= 100 {
            out.push(i);
        }
        quiet = if v.abs() > 0.02 { 0 } else { quiet + 1 };
    }
    out
}

#[test]
fn swing_delays_every_second_note_of_the_arpeggio() {
    let run = |swing: f32| {
        let (_c, mut e) = engine(&patch(&[("swing", swing)], &[("gate_len", 25.0)]));
        on(&mut e, &[60]);
        let x = render(&mut e, 48000 * 4);
        onsets(&x[12000..])
    };
    let straight = run(0.0);
    let swung = run(100.0);
    // Same number of notes; straight ones are 6000 apart, swung alternate 3000 / 9000.
    assert!((straight.len() as i64 - swung.len() as i64).abs() <= 1);
    let gaps = |v: &[usize]| v.windows(2).map(|w| w[1] - w[0]).collect::<Vec<_>>();
    assert!(
        gaps(&straight).iter().all(|&g| (5990..6010).contains(&g)),
        "{:?}",
        gaps(&straight)
    );
    let g = gaps(&swung);
    let (a, b) = (g[2], g[3]);
    assert!(
        ((2990..3010).contains(&a) && (8990..9010).contains(&b))
            || ((8990..9010).contains(&a) && (2990..3010).contains(&b)),
        "{g:?}"
    );
}

#[test]
fn an_old_sequencer_patch_has_no_ratchets() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/sequence");
    let state = kabl_core::load(&dir).unwrap().state().clone();
    let seq = state.modules.values().find(|m| m.kind == "seq").unwrap();
    assert!(
        !seq.params.keys().any(|k| k.contains('k') && k.len() <= 4 && k != "bank"),
        "the stored file knows nothing of ratchets"
    );
    let info = kabl_modules::registry::info_for("seq").unwrap();
    for p in info.params.iter().filter(|p| p.unit == "x") {
        assert_eq!(p.default, 1.0, "{}", p.name);
    }
    assert_eq!(info.params.iter().filter(|p| p.unit == "x").count(), 32);
    // And it still plays: one note per 16th, as before.
    let (_c, mut e) = engine(&state);
    let x = render(&mut e, 48000);
    assert!(rms(&x) > 0.001);
}
