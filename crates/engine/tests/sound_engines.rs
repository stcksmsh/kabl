//! `osc.fm` and `osc.wt` through the real compiler: polyphony, allocation-freedom, live edits,
//! and patches that carry their wavetables (save, reload, host state) without the files they
//! were imported from.

use std::collections::BTreeMap;

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use kabl_core::{CableState, ModuleState, Op, PatchLog, PatchState, PortRef, Source, Table, Vec2};
use kabl_engine::compile::{compile, recompile};
use kabl_engine::graph::BLOCK;
use kabl_modules::wavetable;

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

/// midi.in -> modulator (x2) -> carrier.pm -> out, feedback on the carrier.
fn fm_patch() -> PatchState {
    let mut p = PatchState::new();
    p.modules.insert(1, module("midi.in", &[]));
    p.modules
        .insert(2, module("osc.fm", &[("ratio", 2.0), ("level", 0.8)]));
    p.modules
        .insert(3, module("osc.fm", &[("index", 3.0), ("feedback", 0.3)]));
    p.modules.insert(4, module("out", &[]));
    p.cables.insert(1, cable((1, "pitch"), (2, "pitch")));
    p.cables.insert(2, cable((1, "pitch"), (3, "pitch")));
    p.cables.insert(3, cable((2, "out"), (3, "pm")));
    gate_to_out(&mut p, 3);
    p
}

/// Source `from` -> vca (env.adsr on the gate) -> out, so an idle voice is silent.
fn gate_to_out(p: &mut PatchState, from: u64) {
    p.modules.insert(5, module("vca", &[("gain", 0.0)]));
    p.modules.insert(
        6,
        module(
            "env.adsr",
            &[("attack_ms", 2.0), ("sustain", 1.0), ("release_ms", 50.0)],
        ),
    );
    p.cables.insert(20, cable((from, "out"), (5, "in")));
    p.cables.insert(21, cable((1, "gate"), (6, "gate")));
    p.cables.insert(22, cable((6, "out"), (5, "cv")));
    p.cables.insert(23, cable((5, "out"), (4, "left")));
    p.cables.insert(24, cable((5, "out"), (4, "right")));
}

/// midi.in -> osc.wt (an LFO on its pos jack) -> out.
fn wt_patch(user: f32) -> PatchState {
    let mut p = PatchState::new();
    p.modules.insert(1, module("midi.in", &[]));
    p.modules.insert(
        2,
        module(
            "osc.wt",
            &[("table", 5.0), ("user", user), ("position", 0.5)],
        ),
    );
    p.modules
        .insert(3, module("lfo", &[("rate_hz", 3.0), ("waveform", 0.0)]));
    p.modules.insert(4, module("out", &[]));
    p.cables.insert(1, cable((1, "pitch"), (2, "pitch")));
    p.cables.insert(2, cable((3, "out"), (2, "pos")));
    gate_to_out(&mut p, 2);
    p
}

/// A one-frame user table: the 3rd harmonic only, so it is audibly not any factory table.
fn user_table() -> Table {
    let mut w = Vec::new();
    for k in 0..wavetable::FRAME {
        let v = (std::f64::consts::TAU * 3.0 * k as f64 / wavetable::FRAME as f64).sin();
        w.extend(((v * 30000.0) as i16).to_le_bytes());
    }
    let mut b = b"RIFF".to_vec();
    b.extend((36 + w.len() as u32).to_le_bytes());
    b.extend(b"WAVEfmt ");
    b.extend(16u32.to_le_bytes());
    b.extend([1, 0, 1, 0]);
    b.extend(48000u32.to_le_bytes());
    b.extend(96000u32.to_le_bytes());
    b.extend([2, 0, 16, 0]);
    b.extend(b"data");
    b.extend((w.len() as u32).to_le_bytes());
    b.extend(w);
    Table {
        name: "third.wav".into(),
        wav: wavetable::import_wav(&b).unwrap(),
    }
}

fn render(
    c: &mut kabl_engine::compile::CompiledPatch,
    notes: &[(usize, f32)],
    blocks: usize,
) -> Vec<f32> {
    for &(v, semis) in notes {
        c.note_on(v, semis, 0.8);
    }
    let mut out = Vec::new();
    for _ in 0..blocks {
        assert_no_alloc(|| c.process_block());
        out.extend_from_slice(c.left());
    }
    out
}

fn max_diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f32::max)
}

#[test]
fn fm_voices_are_independent_and_sum() {
    let p = fm_patch();
    let solo = |notes: &[(usize, f32)]| {
        let mut c = compile(&p, SR, 8).unwrap();
        render(&mut c, notes, 200)
    };
    let a = solo(&[(0, 0.0)]);
    let b = solo(&[(3, 7.0)]);
    let both = solo(&[(0, 0.0), (3, 7.0)]);
    assert!(a.iter().any(|v| v.abs() > 0.1) && b.iter().any(|v| v.abs() > 0.1));
    assert!(max_diff(&a, &b) > 0.1, "voices must differ");
    let sum: Vec<f32> = a.iter().zip(&b).map(|(x, y)| x + y).collect();
    assert!(
        max_diff(&both, &sum) < 1e-4,
        "voices interact: {}",
        max_diff(&both, &sum)
    );
}

#[test]
fn wt_voices_are_independent_and_sum() {
    let p = wt_patch(0.0);
    let solo = |notes: &[(usize, f32)]| {
        let mut c = compile(&p, SR, 8).unwrap();
        render(&mut c, notes, 200)
    };
    let a = solo(&[(1, -5.0)]);
    let b = solo(&[(6, 4.0)]);
    let both = solo(&[(1, -5.0), (6, 4.0)]);
    assert!(max_diff(&a, &b) > 0.1);
    let sum: Vec<f32> = a.iter().zip(&b).map(|(x, y)| x + y).collect();
    assert!(max_diff(&both, &sum) < 1e-4);
}

#[test]
fn eight_voices_of_both_sources_allocate_nothing() {
    let mut p = fm_patch();
    p.tables.insert(1, user_table());
    p.modules.insert(7, module("osc.wt", &[("user", 1.0)]));
    p.cables.insert(6, cable((1, "pitch"), (7, "pitch")));
    p.cables.insert(7, cable((7, "out"), (4, "left")));
    let mut c = compile(&p, SR, 8).unwrap();
    let notes: Vec<(usize, f32)> = (0..8).map(|v| (v, v as f32 * 3.0 - 12.0)).collect();
    let out = render(&mut c, &notes, 400);
    assert!(out.iter().all(|v| v.is_finite()));
    assert!(out.iter().any(|v| v.abs() > 0.05));
}

#[test]
fn user_table_is_what_plays_and_a_missing_slot_falls_back_without_a_panic() {
    let notes = [(0usize, 0.0f32)];
    let factory = render(&mut compile(&wt_patch(0.0), SR, 2).unwrap(), &notes, 100);

    let mut with = wt_patch(1.0);
    with.tables.insert(1, user_table());
    let user = render(&mut compile(&with, SR, 2).unwrap(), &notes, 100);
    assert!(max_diff(&factory, &user) > 0.1, "user table not used");

    // Slot 1 selected but empty (a patch from a build that lost the table): factory sound.
    let missing = render(&mut compile(&wt_patch(1.0), SR, 2).unwrap(), &notes, 100);
    assert_eq!(missing, factory);

    // Corrupt bytes in the slot behave the same.
    let mut corrupt = wt_patch(1.0);
    let mut t = user_table();
    t.wav.truncate(200);
    corrupt.tables.insert(1, t);
    assert_eq!(
        render(&mut compile(&corrupt, SR, 2).unwrap(), &notes, 100),
        factory
    );
}

#[test]
fn a_patch_carries_its_tables_through_save_reload_and_json_without_the_wav_file() {
    // Import from a real file, then delete it: nothing may depend on the path afterwards.
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("third.wav");
    let t = user_table();
    std::fs::write(&src, &t.wav).unwrap();
    let imported = wavetable::import_wav(&std::fs::read(&src).unwrap()).unwrap();
    std::fs::remove_file(&src).unwrap();

    let mut log = PatchLog::new();
    log.append(
        Op::SetTable {
            slot: 1,
            value: Some(Table {
                name: "third.wav".into(),
                wav: imported,
            }),
        },
        0,
        Source::User,
    );
    for (id, m) in &wt_patch(1.0).modules {
        log.append(
            Op::AddModule {
                id: *id,
                kind: m.kind.clone(),
                pos: m.pos,
            },
            0,
            Source::User,
        );
        for (name, v) in &m.params {
            log.append(
                Op::SetParam {
                    target: kabl_core::ParamTarget::Module {
                        id: *id,
                        param: name.clone(),
                    },
                    value: *v,
                },
                0,
                Source::User,
            );
        }
    }
    for (id, c) in &wt_patch(1.0).cables {
        log.append(
            Op::Connect {
                id: *id,
                from: c.from.clone(),
                to: c.to.clone(),
            },
            0,
            Source::User,
        );
    }
    let notes = [(0usize, 2.0f32)];
    let original = render(&mut compile(log.state(), SR, 2).unwrap(), &notes, 150);

    let saved = dir.path().join("patch");
    kabl_core::save(&saved, &log).unwrap();
    let reloaded = kabl_core::load(&saved).unwrap();
    assert_eq!(reloaded.state(), log.state());
    assert_eq!(
        render(&mut compile(reloaded.state(), SR, 2).unwrap(), &notes, 150),
        original
    );

    // The checkpoint alone (what a host project stores) reproduces it as well.
    let json = serde_json::to_vec(log.state()).unwrap();
    let from_json: PatchState = serde_json::from_slice(&json).unwrap();
    assert_eq!(
        render(&mut compile(&from_json, SR, 2).unwrap(), &notes, 150),
        original
    );
}

#[test]
fn live_edit_keeps_phase_and_a_table_change_swaps_the_table() {
    let mut p = wt_patch(0.0);
    p.modules.remove(&3); // no LFO: a steady tone to compare across the edit
    p.cables.remove(&2);
    let notes = [(0usize, 0.0f32)];

    let mut straight = compile(&p, SR, 2).unwrap();
    let reference = render(&mut straight, &notes, 120);

    let mut a = compile(&p, SR, 2).unwrap();
    let first = render(&mut a, &notes, 60);
    // Same patch recompiled: state carried, the tone continues bit for bit.
    let mut b = recompile(&mut a, &p, SR, 2).unwrap();
    b.note_on(0, 0.0, 0.8);
    let mut second = Vec::new();
    for _ in 0..60 {
        assert_no_alloc(|| b.process_block());
        second.extend_from_slice(b.left());
    }
    let mut joined = first;
    joined.extend(second);
    // The note-on restarts the envelope-less voice only in its gate; the oscillator keeps going.
    assert!(max_diff(&joined[..60 * BLOCK], &reference[..60 * BLOCK]) == 0.0);
    assert!(max_diff(&joined[60 * BLOCK..], &reference[60 * BLOCK..]) < 1e-6);

    // Switching to a user table changes the sound but keeps the phase running.
    let mut edited = p.clone();
    edited.tables.insert(1, user_table());
    edited
        .modules
        .get_mut(&2)
        .unwrap()
        .params
        .insert("user".into(), 1.0);
    let mut c = recompile(&mut a, &edited, SR, 2).unwrap();
    c.note_on(0, 0.0, 0.8);
    c.process_block();
    assert!(c.left().iter().any(|v| v.abs() > 0.01));
}

#[test]
fn runtime_param_edits_never_need_a_compile_except_the_table_choice() {
    use kabl_engine::runtime::{runtime_changes, RuntimeTarget};
    let base = fm_patch();
    let mut edited = base.clone();
    edited
        .modules
        .get_mut(&3)
        .unwrap()
        .params
        .insert("index".into(), 9.0);
    edited
        .modules
        .get_mut(&3)
        .unwrap()
        .params
        .insert("feedback".into(), 0.6);
    let changes = runtime_changes(&base, &edited).expect("index and feedback apply in place");
    assert_eq!(changes.len(), 2);
    assert!(changes
        .iter()
        .all(|(t, _)| matches!(t, RuntimeTarget::Param { id: 3, .. })));

    let wt = wt_patch(0.0);
    let mut moved = wt.clone();
    moved
        .modules
        .get_mut(&2)
        .unwrap()
        .params
        .insert("position".into(), 0.9);
    assert!(runtime_changes(&wt, &moved).is_some());
    for (param, v) in [("table", 2.0), ("user", 1.0)] {
        let mut e = wt.clone();
        e.modules
            .get_mut(&2)
            .unwrap()
            .params
            .insert(param.into(), v);
        assert!(
            runtime_changes(&wt, &e).is_none(),
            "{param} needs a compile"
        );
    }
    let mut with_table = wt.clone();
    with_table.tables.insert(1, user_table());
    assert!(
        runtime_changes(&wt, &with_table).is_none(),
        "a new table needs a compile"
    );
}
