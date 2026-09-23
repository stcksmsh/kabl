//! Live-editing correctness on the real `PatchEngine` path: MIDI notes through active /
//! incoming / pending graphs, `midi.in` discovery by kind (not by id), reclamation of retired
//! graphs, the bounded swap queue, and allocation-freedom of every audio-thread call involved.
//!
//! Probe patch: `midi.in.gate -> vca.in`, `vca.out -> out`, 1 voice. The output is exactly
//! `gate × gain`, so a stuck or lost note shows up as a wrong constant, and each swap can use a
//! different `gain` to tell the graphs apart.

use std::collections::BTreeMap;

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::{Collector, Handle, Owned};
use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::compile::{compile, CompiledPatch};
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::{swap_channel, PatchEngine};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;
/// Enough blocks for any fade (15 ms = 11.25 blocks) plus a queued one to finish.
const SETTLE: usize = 40;

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

/// `midi.in` deliberately at id 7, not 1.
fn probe(gain: f32) -> PatchState {
    let mut p = PatchState::new();
    p.modules.insert(7, module("midi.in", &[]));
    p.modules.insert(2, module("vca", &[("gain", gain)]));
    p.modules.insert(3, module("out", &[]));
    p.cables.insert(1, cable((7, "gate"), (2, "in")));
    p.cables.insert(2, cable((2, "out"), (3, "left")));
    p
}

fn build(handle: &Handle, patch: &PatchState) -> Owned<CompiledPatch> {
    Owned::new(handle, compile(patch, SR, 1).unwrap())
}

/// Runs `n` blocks with allocation forbidden and returns the last block's left channel.
fn run(engine: &mut PatchEngine, n: usize) -> [f32; BLOCK] {
    let mut l = [0.0; BLOCK];
    let mut r = [0.0; BLOCK];
    for _ in 0..n {
        assert_no_alloc(|| engine.process_block(&mut l, &mut r));
    }
    l
}

fn assert_const(block: &[f32; BLOCK], v: f32, what: &str) {
    for &s in block {
        assert!((s - v).abs() < 1e-6, "{what}: expected {v}, got {s}");
    }
}

#[test]
fn note_off_during_a_crossfade_reaches_the_incoming_graph() {
    let collector = Collector::new();
    let h = collector.handle();
    let mut engine = PatchEngine::new(&h, &probe(1.0), SR, 1).unwrap();
    assert_no_alloc(|| engine.note_on(0, 0.0, 1.0));
    assert_const(&run(&mut engine, 4), 1.0, "held before swap");

    let g = build(&h, &probe(0.8));
    assert_no_alloc(|| engine.receive_swap(g));
    run(&mut engine, 3); // mid-fade
    assert!(engine.is_swapping());
    assert_no_alloc(|| engine.note_off(0));
    assert_const(&run(&mut engine, SETTLE), 0.0, "released after fade");
}

#[test]
fn note_on_during_a_crossfade_reaches_the_incoming_graph() {
    let collector = Collector::new();
    let h = collector.handle();
    let mut engine = PatchEngine::new(&h, &probe(1.0), SR, 1).unwrap();
    run(&mut engine, 2);
    let g = build(&h, &probe(0.8));
    engine.receive_swap(g);
    run(&mut engine, 3);
    assert_no_alloc(|| engine.note_on(0, 0.0, 1.0));
    assert_const(
        &run(&mut engine, SETTLE),
        0.8,
        "note from mid-fade still sounds",
    );
}

/// The pending graph is built while the note is held and released before it starts. Its
/// `midi.in` state must come from the graph playing when its fade starts, not from build time.
#[test]
fn a_queued_graph_does_not_revive_a_note_released_while_it_waited() {
    let collector = Collector::new();
    let h = collector.handle();
    let mut engine = PatchEngine::new(&h, &probe(1.0), SR, 1).unwrap();
    engine.note_on(0, 0.0, 1.0);
    run(&mut engine, 4);

    let a = build(&h, &probe(0.9));
    let b = build(&h, &probe(0.7));
    engine.receive_swap(a);
    run(&mut engine, 2);
    engine.receive_swap(b); // queued behind `a`
    run(&mut engine, 2);
    assert_no_alloc(|| engine.note_off(0));
    assert_const(&run(&mut engine, SETTLE), 0.0, "no revived note");
    assert!(!engine.is_swapping());
}

#[test]
fn a_queued_graph_plays_a_note_started_while_it_waited() {
    let collector = Collector::new();
    let h = collector.handle();
    let mut engine = PatchEngine::new(&h, &probe(1.0), SR, 1).unwrap();
    run(&mut engine, 2);
    engine.receive_swap(build(&h, &probe(0.9)));
    run(&mut engine, 2);
    engine.receive_swap(build(&h, &probe(0.7)));
    engine.note_on(0, 0.0, 1.0);
    assert_const(&run(&mut engine, SETTLE), 0.7, "note reaches final graph");
}

/// Randomized interleaving of edits and notes: after everything settles, the output must match
/// the last gate state times the last gain, every time.
#[test]
fn overlapping_swaps_and_notes_never_leave_a_wrong_gate() {
    let collector = Collector::new();
    let h = collector.handle();
    let mut engine = PatchEngine::new(&h, &probe(1.0), SR, 1).unwrap();
    let mut rng = 0x9e37_79b9u32;
    let mut next = || {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        rng
    };
    let (mut gate, mut gain) = (false, 1.0f32);
    for round in 0..200 {
        for _ in 0..(next() % 4) {
            match next() % 3 {
                0 => {
                    gain = 0.1 + (next() % 9) as f32 * 0.1;
                    engine.receive_swap(build(&h, &probe(gain)));
                }
                1 => {
                    gate = true;
                    engine.note_on(0, 0.0, 1.0);
                }
                _ => {
                    gate = false;
                    engine.note_off(0);
                }
            }
            run(&mut engine, (next() % 3) as usize);
        }
        if round % 20 == 19 {
            let expect = if gate { gain } else { 0.0 };
            assert_const(&run(&mut engine, SETTLE), expect, "settled output");
        }
    }
}

#[test]
fn every_midi_in_receives_notes_whatever_its_id() {
    let mut p = probe(1.0);
    // A second midi.in at id 40 feeding the right channel.
    p.modules.insert(40, module("midi.in", &[]));
    p.cables.insert(3, cable((40, "gate"), (3, "right")));
    let mut compiled = compile(&p, SR, 2).unwrap();
    compiled.note_on(1, 0.0, 1.0);
    compiled.process_block();
    // Voice-rate -> global out averages over 2 voices: one voice held = 0.5 on both channels.
    assert_const(compiled.left(), 0.5, "midi.in #7");
    assert_const(compiled.right(), 0.5, "midi.in #40");
    compiled.note_off(1);
    compiled.process_block();
    assert_const(compiled.left(), 0.0, "released #7");
    assert_const(compiled.right(), 0.0, "released #40");
}

#[test]
fn retired_graphs_are_reclaimed_by_collect() {
    let mut collector = Collector::new();
    let h = collector.handle();
    let mut engine = PatchEngine::new(&h, &probe(1.0), SR, 1).unwrap();
    for i in 0..300 {
        engine.receive_swap(build(&h, &probe(0.5 + (i % 2) as f32 * 0.1)));
        run(&mut engine, 13);
        collector.collect();
        // active + at most the node just retired this iteration's collect already freed.
        assert!(
            collector.alloc_count() <= 2,
            "leak: {}",
            collector.alloc_count()
        );
    }
}

#[test]
fn without_collect_retired_graphs_accumulate() {
    // The failure mode the UI had: it never called `collect()`.
    let collector = Collector::new();
    let h = collector.handle();
    let mut engine = PatchEngine::new(&h, &probe(1.0), SR, 1).unwrap();
    for _ in 0..50 {
        engine.receive_swap(build(&h, &probe(0.5)));
        run(&mut engine, 13);
    }
    assert!(collector.alloc_count() > 40);
}

#[test]
fn swap_queue_bounds_holds_newest_and_drains_without_alloc() {
    let mut collector = Collector::new();
    let h = collector.handle();
    let mut engine = PatchEngine::new(&h, &probe(1.0), SR, 1).unwrap();
    let (mut tx, mut rx) = swap_channel(2);

    // Audio thread stalled: 10 edits, queue holds 2, sender keeps only the newest (gain 1.0).
    let mut held = false;
    for i in 1..=10 {
        held = tx.send(build(&h, &probe(i as f32 * 0.1)));
    }
    assert!(held, "overflowing edit is held, not dropped silently");
    collector.collect();
    // 1 active + 2 queued + 1 held; the 7 superseded graphs were freed.
    assert_eq!(collector.alloc_count(), 4);

    engine.note_on(0, 0.0, 1.0);
    assert_no_alloc(|| engine.drain_swaps(&mut rx));
    assert!(!tx.flush(), "held graph fits once the queue drained");
    run(&mut engine, SETTLE);
    assert_no_alloc(|| engine.drain_swaps(&mut rx));
    assert_const(&run(&mut engine, SETTLE), 1.0, "newest edit wins");
    collector.collect();
    assert_eq!(collector.alloc_count(), 1, "only the active graph remains");
}

/// The committed reference patch (6 routes: 2 on Attack, 4 on Cutoff) through every audio-thread
/// path this slice touches, with allocation forbidden: modulated params, key-trigger capture,
/// swaps queued mid-fade (state carry), MIDI to active + incoming.
#[test]
fn reference_patch_audio_thread_paths_do_not_allocate() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/reference");
    let patch = kabl_core::load(&dir)
        .expect("reference patch")
        .state()
        .clone();
    let mut key = patch.clone();
    key.modules
        .get_mut(&4)
        .unwrap()
        .params
        .insert("timing".into(), 1.0);
    let mut deeper = key.clone();
    deeper
        .cables
        .get_mut(&12)
        .unwrap()
        .params
        .insert("amount".into(), 0.6);

    let mut collector = Collector::new();
    let h = collector.handle();
    let mut engine = PatchEngine::new(&h, &patch, SR, 8).unwrap();
    let (mut tx, mut rx) = swap_channel(4);
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    let mut peak = 0.0f32;
    for block in 0..600 {
        if block % 40 == 5 {
            // Two edits back to back: the second queues behind the first fade.
            tx.send(Owned::new(&h, compile(&key, SR, 8).unwrap()));
            tx.send(Owned::new(&h, compile(&deeper, SR, 8).unwrap()));
        }
        assert_no_alloc(|| {
            engine.drain_swaps(&mut rx);
            if block % 50 == 0 {
                engine.note_on((block / 50) % 8, (block % 12) as f32, 0.8);
            }
            if block % 50 == 30 {
                engine.note_off((block / 50) % 8);
            }
            engine.process_block(&mut l, &mut r);
        });
        tx.flush();
        collector.collect();
        peak = l.iter().fold(peak, |m, s| m.max(s.abs()));
    }
    assert!(peak > 0.01, "reference patch makes sound ({peak})");
    assert!(l.iter().chain(&r).all(|s| s.is_finite()));
}
