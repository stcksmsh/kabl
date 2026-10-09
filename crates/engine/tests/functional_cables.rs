//! Functional cables through the real compiler: pattern timing against the clock's own pulses,
//! probability reproducibility, runtime edits, undo and state recall, no-alloc, legacy shape.
//!
//! Probe: `lfo` square at 0.01 Hz (a constant +1 CV for the first 50 s) -> cable 1 -> `out.left`,
//! and `clock.gate` -> `out.right` so every clock pulse is visible in the output. At 300 bpm a
//! pulse is 2400 samples.

use std::collections::BTreeMap;

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use kabl_cables::{seed_of, Settings};
use kabl_core::{
    CableState, Op, ParamTarget, PatchLog, PatchState, PortRef, Source, Vec2, CURRENT_SCHEMA_VERSION,
};
use kabl_engine::compile::{compile, CompiledPatch};
use kabl_engine::graph::BLOCK;
use kabl_engine::runtime::{runtime_changes, RuntimeTarget};
use kabl_modules::builtins::Transport;

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;
const CLOCK: u64 = 1;
const LFO: u64 = 2;
const OUT: u64 = 3;
const CABLE: u64 = 1;

fn params(kv: &[(&str, f32)]) -> BTreeMap<String, f32> {
    kv.iter().map(|&(k, v)| (k.to_string(), v)).collect()
}

fn module(kind: &str, kv: &[(&str, f32)]) -> kabl_core::ModuleState {
    kabl_core::ModuleState {
        kind: kind.into(),
        pos: Vec2 { x: 0.0, y: 0.0 },
        params: params(kv),
    }
}

fn jack(from: (u64, &str), to: (u64, &str), kv: &[(&str, f32)]) -> CableState {
    let port = |(id, port): (u64, &str)| PortRef::Module {
        id,
        port: port.into(),
    };
    CableState {
        from: port(from),
        to: port(to),
        params: params(kv),
        steps: Vec::new(),
    }
}

fn probe(cable: &[(&str, f32)]) -> PatchState {
    let mut p = PatchState::new();
    p.modules.insert(CLOCK, module("clock", &[("bpm", 300.0)]));
    p.modules.insert(
        LFO,
        module("lfo", &[("rate_hz", 0.01), ("waveform", 3.0)]),
    );
    p.modules.insert(OUT, module("out", &[]));
    p.cables.insert(CABLE, jack((LFO, "out"), (OUT, "left"), cable));
    p.cables.insert(2, jack((CLOCK, "gate"), (OUT, "right"), &[]));
    p
}

/// What the cable must output, rebuilt from the clock's visible pulses (`right`) and the
/// restart commands (sample index of the command; the next pulse is tick 0).
fn expected(patch: &PatchState, right: &[f32], restarts: &[usize]) -> Vec<f32> {
    let s = Settings::from_params(&patch.cables[&CABLE].params);
    let seed = seed_of(CABLE);
    let (mut tick, mut prev) = (None::<u64>, 0.0);
    let mut restarts = restarts.iter().peekable();
    let mut pending = false;
    right
        .iter()
        .enumerate()
        .map(|(i, &r)| {
            while restarts.peek().is_some_and(|&&at| at <= i) {
                restarts.next();
                pending = true;
            }
            if r > 0.5 && prev <= 0.5 {
                tick = Some(match (pending, tick) {
                    (false, Some(t)) => t + 1,
                    _ => 0,
                });
                pending = false;
            }
            prev = r;
            tick.map_or(1.0, |t| s.level_at(seed, t))
        })
        .collect()
}

fn render(
    c: &mut CompiledPatch,
    blocks: usize,
    mut each: impl FnMut(&mut CompiledPatch, usize),
) -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut r) = (Vec::new(), Vec::new());
    for b in 0..blocks {
        each(c, b);
        c.process_block();
        l.extend_from_slice(c.left());
        r.extend_from_slice(c.right());
    }
    (l, r)
}

fn first_diff(a: &[f32], b: &[f32]) -> Option<usize> {
    a.iter().zip(b).position(|(x, y)| x != y)
}

const PATTERN: &[(&str, f32)] = &[
    ("length", 5.0),
    ("s2", 0.5),
    ("s3", 0.0),
    ("s5", 0.25),
    ("r4", 50.0),
];

#[test]
fn pattern_changes_on_the_clock_pulse_sample() {
    let p = probe(PATTERN);
    let mut c = compile(&p, SR, 1).unwrap();
    let (l, r) = render(&mut c, 600, |_, _| {});
    assert!(r.iter().any(|&x| x > 0.5));
    assert_eq!(first_diff(&l, &expected(&p, &r, &[])), None);
    let levels: std::collections::BTreeSet<u32> = l.iter().map(|x| x.to_bits()).collect();
    assert!(levels.len() >= 3, "pattern is audible: {levels:?}");
}

#[test]
fn restart_stop_and_run_follow_the_clock() {
    let p = probe(PATTERN);
    let mut c = compile(&p, SR, 1).unwrap();
    let (l, r) = render(&mut c, 3000, |c, b| match b {
        700 => c.transport(CLOCK, Transport::Restart),
        1301 => c.transport(CLOCK, Transport::Stop),
        1700 => c.transport(CLOCK, Transport::Run),
        2200 => c.transport(CLOCK, Transport::Restart),
        _ => {}
    });
    let restarts = [700 * BLOCK, 2200 * BLOCK];
    assert_eq!(first_diff(&l, &expected(&p, &r, &restarts)), None);
}

#[test]
fn host_sync_pulses_drive_the_pattern() {
    let p = probe(PATTERN);
    let mut c = compile(&p, SR, 1).unwrap();
    let per_block = BLOCK as f64 / SR as f64 * 140.0 / 60.0;
    let mut beats = 3.3;
    let (l, r) = render(&mut c, 4000, |c, b| {
        match b {
            1500 => beats -= 2.9, // loop jump
            2500 => beats += 7.1, // locate
            _ => {}
        }
        // Stopped by the host for a stretch.
        c.host_clock(Some((beats, 140.0, !(1800..2100).contains(&b))));
        beats += per_block;
    });
    assert_eq!(first_diff(&l, &expected(&p, &r, &[])), None);
}

#[test]
fn probability_is_reproducible_and_restarts_replay_it() {
    let p = probe(&[("length", 3.0), ("prob", 50.0), ("r1", 80.0)]);
    let run = || {
        let mut c = compile(&p, SR, 1).unwrap();
        render(&mut c, 1500, |c, b| {
            if b == 750 {
                c.transport(CLOCK, Transport::Restart);
            }
        })
    };
    let (a, r) = run();
    assert_eq!(first_diff(&a, &run().0), None, "two renders are identical");
    assert_eq!(first_diff(&a, &expected(&p, &r, &[750 * BLOCK])), None);
    // Sampling mid-pulse, epoch two replays epoch one pulse for pulse.
    let at = |blocks: std::ops::Range<usize>| -> Vec<f32> {
        blocks.step_by(50).map(|b| a[b * BLOCK + 1200]).collect()
    };
    let (first, second) = (at(0..700), at(750..1450));
    assert_eq!(first[..10], second[..10]);
    let open = first.iter().filter(|&&x| x > 0.0).count();
    assert!(open > 0 && open < first.len(), "some pulses pass, some do not");
}

#[test]
fn route_pattern_gates_the_modulation_amount() {
    // lfo +1 -> vca.gain route (amount 0.5): gain follows the pattern at block rate.
    let mut p = PatchState::new();
    p.modules.insert(CLOCK, module("clock", &[("bpm", 300.0)]));
    p.modules.insert(
        LFO,
        module("lfo", &[("rate_hz", 0.01), ("waveform", 3.0)]),
    );
    p.modules.insert(4, module("midi.in", &[]));
    p.modules.insert(5, module("vca", &[("gain", 0.0)]));
    p.modules.insert(OUT, module("out", &[]));
    p.cables.insert(10, jack((4, "gate"), (5, "in"), &[]));
    p.cables.insert(11, jack((5, "out"), (OUT, "left"), &[]));
    p.cables.insert(12, jack((CLOCK, "gate"), (OUT, "right"), &[]));
    p.cables.insert(
        CABLE,
        CableState {
            from: PortRef::Module {
                id: LFO,
                port: "out".into(),
            },
            to: PortRef::Param {
                id: 5,
                param: "gain".into(),
            },
            params: params(&[("amount", 0.5), ("length", 2.0), ("s2", 0.0)]),
            steps: Vec::new(),
        },
    );
    let mut c = compile(&p, SR, 1).unwrap();
    c.note_on(0, 0.0, 1.0);
    let (l, r) = render(&mut c, 400, |_, _| {});
    let pulses: Vec<usize> = (0..r.len())
        .filter(|&i| r[i] > 0.5 && (i == 0 || r[i - 1] <= 0.5))
        .collect();
    assert!(pulses.len() >= 4);
    for (tick, &at) in pulses.iter().enumerate().take(4) {
        let want = if tick % 2 == 0 { 0.5 } else { 0.0 };
        let got = l[at + 1500];
        assert!((got - want).abs() < 1e-3, "pulse {tick}: {got} vs {want}");
    }
}

#[test]
fn runtime_edit_changes_the_next_pulse_in_place() {
    let a = probe(&[("length", 2.0), ("s2", 1.0)]);
    let mut b = a.clone();
    b.cables
        .get_mut(&CABLE)
        .unwrap()
        .params
        .insert("s2".into(), 0.0);
    let changes = runtime_changes(&a, &b).expect("edit of a functional cable is runtime");
    assert_eq!(
        changes,
        vec![(RuntimeTarget::Cable { cable: CABLE, slot: 3 }, 0.0)]
    );

    let mut c = compile(&a, SR, 1).unwrap();
    let steps = c.profile_counts();
    render(&mut c, 10, |_, _| {});
    for (t, v) in changes {
        assert!(c.set_runtime(t, v, true));
    }
    assert_eq!(c.profile_counts(), steps, "no rebuild");
    assert_eq!(c.ramps(), 0);
    // Same render as a graph compiled from `b` in the same position, from this block on.
    let mut d = compile(&b, SR, 1).unwrap();
    render(&mut d, 10, |_, _| {});
    let (l1, _) = render(&mut c, 300, |_, _| {});
    let (l2, _) = render(&mut d, 300, |_, _| {});
    assert_eq!(first_diff(&l1, &l2), None);
    assert!(l1.iter().any(|&x| x == 0.0) && l1.iter().any(|&x| x == 1.0));
}

#[test]
fn becoming_or_ending_functional_needs_a_compile() {
    let plain = probe(&[("amount", 0.3)]);
    let mut on = plain.clone();
    on.cables
        .get_mut(&CABLE)
        .unwrap()
        .params
        .insert("length".into(), 4.0);
    assert_eq!(runtime_changes(&plain, &on), None);
    assert_eq!(runtime_changes(&on, &plain), None);
    // Edits that leave a plain cable plain stay free.
    let mut other = plain.clone();
    other
        .cables
        .get_mut(&CABLE)
        .unwrap()
        .params
        .insert("amount".into(), 0.9);
    assert_eq!(runtime_changes(&plain, &other), Some(vec![]));
}

#[test]
fn plain_cables_add_no_work() {
    let steps = |p: &PatchState| compile(p, SR, 1).unwrap().profile_counts().2;
    let none = steps(&probe(&[]));
    assert_eq!(steps(&probe(&[("amount", 0.5), ("bypass", 0.0)])), none);
    assert_eq!(steps(&probe(&[("length", 0.0), ("prob", 100.0)])), none);
    assert_eq!(steps(&probe(PATTERN)), none + 1);
}

#[test]
fn no_clock_means_a_plain_cable() {
    let mut p = probe(PATTERN);
    p.modules.remove(&CLOCK);
    p.cables.remove(&2);
    let mut c = compile(&p, SR, 1).unwrap();
    let (l, _) = render(&mut c, 20, |_, _| {});
    assert!(l.iter().all(|&x| x == 1.0));
}

#[test]
fn audio_path_does_not_allocate_even_while_edited() {
    let p = probe(PATTERN);
    let mut c = compile(&p, SR, 1).unwrap();
    for b in 0..400 {
        assert_no_alloc(|| {
            if b % 7 == 0 {
                for slot in 0..kabl_cables::SLOTS as u8 {
                    c.set_runtime(
                        RuntimeTarget::Cable { cable: CABLE, slot },
                        (b % 5) as f32,
                        true,
                    );
                }
            }
            c.process_block();
        });
    }
    assert!(c.left().iter().all(|x| x.is_finite()));
}

fn cable_ops() -> Vec<Op> {
    let mut ops = vec![
        Op::AddModule { id: CLOCK, kind: "clock".into(), pos: Vec2::default() },
        Op::AddModule { id: LFO, kind: "lfo".into(), pos: Vec2::default() },
        Op::AddModule { id: OUT, kind: "out".into(), pos: Vec2::default() },
        Op::Connect {
            id: CABLE,
            from: PortRef::Module { id: LFO, port: "out".into() },
            to: PortRef::Module { id: OUT, port: "left".into() },
        },
        Op::Connect {
            id: 2,
            from: PortRef::Module { id: CLOCK, port: "gate".into() },
            to: PortRef::Module { id: OUT, port: "right".into() },
        },
    ];
    for (k, v) in [("rate_hz", 0.01), ("waveform", 3.0)] {
        ops.push(Op::SetParam {
            target: ParamTarget::Module { id: LFO, param: k.into() },
            value: v,
        });
    }
    ops.push(Op::SetParam {
        target: ParamTarget::Module { id: CLOCK, param: "bpm".into() },
        value: 300.0,
    });
    ops
}

fn set_cable(k: &str, v: f32) -> Op {
    Op::SetParam {
        target: ParamTarget::Cable { id: CABLE, param: k.into() },
        value: v,
    }
}

fn render_state(p: &PatchState) -> Vec<f32> {
    let mut c = compile(p, SR, 1).unwrap();
    render(&mut c, 200, |_, _| {}).0
}

#[test]
fn undo_save_and_reload_recall_the_cable() {
    let mut log = PatchLog::new();
    for op in cable_ops() {
        log.append_new(op, 0, Source::User);
    }
    let plain = log.state().clone();
    for (k, v) in [("length", 4.0), ("s2", 0.0), ("prob", 70.0)] {
        log.append_new(set_cable(k, v), 0, Source::User);
    }
    let functional = log.state().clone();
    let sound = render_state(&functional);
    assert_ne!(sound, render_state(&plain));

    assert!(log.undo());
    assert!(log.undo());
    assert!(log.undo());
    assert_eq!(log.state(), &plain, "undo removes the params, not zeroes them");
    assert!(log.redo() && log.redo() && log.redo());
    assert_eq!(log.state(), &functional);

    let dir = tempfile::tempdir().unwrap();
    kabl_core::save(dir.path(), &log).unwrap();
    let meta = std::fs::read_to_string(dir.path().join("meta.toml")).unwrap();
    assert!(meta.contains(&format!("schema_version = {CURRENT_SCHEMA_VERSION}")));
    assert_eq!(CURRENT_SCHEMA_VERSION, 5);
    let back = kabl_core::load(dir.path()).unwrap();
    assert_eq!(back.state(), &functional);
    assert_eq!(render_state(back.state()), sound, "recall sounds the same");
    // The reloaded log keeps its history: undo still works.
    let mut back = back;
    assert!(back.undo());
}

#[test]
fn a_cable_removed_and_restored_keeps_its_pattern() {
    let mut log = PatchLog::new();
    for op in cable_ops() {
        log.append_new(op, 0, Source::User);
    }
    for (k, v) in [("length", 3.0), ("s3", 0.4), ("r2", 10.0)] {
        log.append_new(set_cable(k, v), 0, Source::User);
    }
    let before = log.state().clone();
    log.append_new(Op::Disconnect { id: CABLE }, 0, Source::User);
    assert!(log.undo());
    assert_eq!(log.state(), &before);
}
