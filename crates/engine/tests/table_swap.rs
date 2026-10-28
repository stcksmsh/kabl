//! Replacing an embedded wavetable while a note sounds, the way the table editor does on every
//! edit: the new table is compiled off the audio thread and swapped in through the ordinary
//! crossfade. Nothing on the audio thread allocates, and the output stays continuous.

use std::collections::BTreeMap;

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::Collector;
use kabl_core::{CableState, ModuleState, PatchState, PortRef, Table, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::PatchEngine;
use kabl_modules::wavetable::{canonical_wav, FRAME};

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
    let port = |id, port: &str| PortRef::Module {
        id,
        port: port.into(),
    };
    CableState {
        from: port(from, fp),
        to: port(to, tp),
        params: BTreeMap::new(),
        steps: Vec::new(),
    }
}

/// Four identical frames of harmonic `h` at phase `phase`, scaled by `level`: tables that differ
/// by a quarter turn or more, so a hard switch between two of them is a large step.
fn table(h: usize, phase: f32, level: f32) -> Table {
    let frame: Vec<f32> = (0..FRAME)
        .map(|i| {
            let x = std::f32::consts::TAU * i as f32 / FRAME as f32;
            level * (h as f32 * x + phase).sin()
        })
        .collect();
    Table {
        name: "t.wav".into(),
        wav: canonical_wav(&vec![frame; 4]),
    }
}

/// MIDI In -> osc.wt reading embedded slot 1 -> Output (the oscillator is the sound).
fn keys(t: Table) -> PatchState {
    let mut p = PatchState::new();
    p.modules.insert(1, module("midi.in", &[]));
    p.modules
        .insert(2, module("osc.wt", &[("user", 1.0), ("position", 0.5)]));
    p.modules.insert(3, module("out", &[]));
    p.cables.insert(1, cable(1, "pitch", 2, "pitch"));
    p.cables.insert(2, cable(2, "out", 3, "in"));
    p.tables.insert(1, t);
    p
}

fn steepest(x: &[f32]) -> f32 {
    x.windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0, f32::max)
}

#[test]
fn replacing_the_table_under_a_held_note_never_allocates_and_never_clicks() {
    let c = Collector::new();
    let h = c.handle();
    let mut e = PatchEngine::new(&h, &keys(table(2, 0.0, 0.9)), SR, 4).unwrap();
    // Twenty edits, each a different table, built before they are needed (off the audio thread).
    let mut graphs: Vec<_> = (0..20)
        .map(|i| {
            let t = table(1 + (i % 2), 1.7 * i as f32, 0.9);
            e.build_swap(&h, &keys(t)).unwrap()
        })
        .collect();
    graphs.reverse();
    e.key(KeyEvent::On {
        note: 57,
        velocity: 100,
    });
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    let mut out = Vec::new();
    let mut swap_at = Vec::new();
    for i in 0..400 {
        let g = (i > 20 && i % 15 == 0).then(|| graphs.pop()).flatten();
        if g.is_some() {
            swap_at.push(out.len());
        }
        assert_no_alloc(|| {
            if let Some(g) = g {
                e.receive_swap(g);
            }
            e.process_block(&mut l, &mut r);
        });
        out.extend_from_slice(&l);
    }
    assert!(swap_at.len() >= 15, "{} swaps", swap_at.len());
    assert!(out.iter().all(|v| v.is_finite()));
    // The note sounds throughout, and a swap is a crossfade: no step is larger than the steepest
    // slope of the tables themselves (harmonic 2 at 220 Hz, level 0.9), plus a little for the
    // fade. A hard switch between these tables would step by more than 0.5.
    let settled = &out[BLOCK * 10..];
    assert!(settled.iter().any(|v| v.abs() > 0.1));
    let limit = std::f32::consts::TAU * 2.0 * 220.0 * 0.9 / SR * 1.2;
    assert!(
        steepest(settled) < limit,
        "step {} over {limit}",
        steepest(settled)
    );
}
