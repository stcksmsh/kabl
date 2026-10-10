//! Routes into a cable's own parameters (`PortRef::CableParam`): one source moves the morph,
//! probability or glide of several cables. Timing is checked against the source's own samples,
//! the nesting rule through the compiler's errors, and undo, save and reload through the log.
//!
//! Probe: as `functional_cables.rs`, a square `lfo` (a constant +1) through cable 1 into
//! `out.left`, the clock's gate into `out.right`; at 300 bpm a pulse is 2400 samples.

use std::collections::BTreeMap;

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use kabl_cables::{seed_of, Mods, Settings, MORPH};
use kabl_core::{
    CableId, CableState, Op, ParamTarget, PatchLog, PatchState, PortRef, Source, Vec2,
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
const SRC: u64 = 4;
const MACRO: u64 = 5;
const CABLE: u64 = 1;
const ROUTE: u64 = 10;

const A: &[(&str, f32)] = &[("length", 3.0), ("s2", 0.0), ("r3", 60.0)];
const B: &[(&str, f32)] = &[
    ("length", 4.0),
    ("s1", 0.5),
    ("s3", 0.2),
    ("r2", 40.0),
    ("r4", 70.0),
];

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

fn route(from: (u64, &str), cable: u64, param: &str, amount: f32) -> CableState {
    CableState {
        from: PortRef::Module {
            id: from.0,
            port: from.1.into(),
        },
        to: PortRef::CableParam {
            cable,
            param: param.into(),
        },
        params: params(&[("amount", amount)]),
        steps: Vec::new(),
    }
}

/// The probe with patterns A and B on cable 1 and `morph` as its base.
fn probe(morph: f32) -> PatchState {
    let mut p = PatchState::new();
    p.modules.insert(CLOCK, module("clock", &[("bpm", 300.0)]));
    p.modules
        .insert(LFO, module("lfo", &[("rate_hz", 0.01), ("waveform", 3.0)]));
    p.modules.insert(OUT, module("out", &[]));
    let mut cable = jack((LFO, "out"), (OUT, "left"), A);
    for (k, v) in B {
        cable.params.insert(format!("b.{k}"), *v);
    }
    cable.params.insert("morph".into(), morph);
    p.cables.insert(CABLE, cable);
    p.cables
        .insert(2, jack((CLOCK, "gate"), (OUT, "right"), &[]));
    p
}

/// The probe plus a macro whose `m1` sits at `value`, routed into cable 1's morph.
fn with_macro(value: f32, amount: f32) -> PatchState {
    let mut p = probe(0.0);
    p.modules.insert(MACRO, module("macro", &[("m1", value)]));
    p.cables
        .insert(ROUTE, route((MACRO, "m1"), CABLE, "morph", amount));
    p
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

fn left_of(p: &PatchState, blocks: usize) -> Vec<f32> {
    render(&mut compile(p, SR, 1).unwrap(), blocks, |_, _| {}).0
}

fn first_diff(a: &[f32], b: &[f32]) -> Option<usize> {
    a.iter().zip(b).position(|(x, y)| x != y)
}

/// What cable 1 must output when each pulse reads `mods(sample)`: the pulse's level from the
/// settings with that modulation, held to the next pulse. Pulses are the clock's visible gate
/// edges (`right`), restarts as in `functional_cables.rs`.
fn expected_mod(
    p: &PatchState,
    right: &[f32],
    restarts: &[usize],
    mods: impl Fn(usize) -> Mods,
) -> Vec<f32> {
    let s = Settings::from_params(&p.cables[&CABLE].params);
    let seed = seed_of(CABLE);
    let (mut tick, mut prev, mut level) = (None::<u64>, 0.0, None::<f32>);
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
                let t = match (pending, tick) {
                    (false, Some(t)) => t + 1,
                    _ => 0,
                };
                (tick, pending) = (Some(t), false);
                level = Some(s.level_at_mod(seed, t, mods(i)));
            }
            prev = r;
            level.unwrap_or(1.0)
        })
        .collect()
}

#[test]
fn a_still_macro_is_the_same_as_that_morph_stored_on_the_cable() {
    // 0.5 is exact in f32, so the route adds exactly the stored morph: bit for bit.
    assert_eq!(
        left_of(&with_macro(0.5, 1.0), 600),
        left_of(&probe(0.5), 600)
    );
    // Amount scales the macro: a quarter of full travel at macro 1.0 is morph 0.25.
    assert_eq!(
        left_of(&with_macro(1.0, 0.25), 600),
        left_of(&probe(0.25), 600)
    );
    // A negative amount moves the other way from a stored morph.
    let mut p = with_macro(0.25, -1.0);
    p.cables
        .get_mut(&CABLE)
        .unwrap()
        .params
        .insert("morph".into(), 0.75);
    assert_eq!(left_of(&p, 600), left_of(&probe(0.5), 600));
}

#[test]
fn a_moving_source_is_read_at_each_pulse_sample() {
    // A 3 Hz sine as the source; its own samples, rendered alone, are the reference.
    let sine = [("rate_hz", 3.0), ("waveform", 0.0)];
    let mut p = probe(0.0);
    p.modules.insert(SRC, module("lfo", &sine));
    p.cables
        .insert(ROUTE, route((SRC, "out"), CABLE, "morph", 0.8));
    let mut alone = PatchState::new();
    alone.modules.insert(SRC, module("lfo", &sine));
    alone.modules.insert(OUT, module("out", &[]));
    alone
        .cables
        .insert(1, jack((SRC, "out"), (OUT, "left"), &[]));
    let blocks = 1200;
    let reference = left_of(&alone, blocks);
    let (l, r) = render(&mut compile(&p, SR, 1).unwrap(), blocks, |_, _| {});
    let want = expected_mod(&p, &r, &[], |i| Mods {
        morph: 0.8 * reference[i],
        ..Mods::default()
    });
    assert_eq!(first_diff(&l, &want), None);
    // The source really moved the morph: not the stored (zero) morph's output.
    assert_ne!(l, left_of(&probe(0.0), blocks));
}

#[test]
fn a_restart_replays_the_modulated_cable_and_two_renders_agree() {
    let p = with_macro(0.4, 1.0);
    let run = || {
        render(&mut compile(&p, SR, 1).unwrap(), 2400, |c, b| match b {
            700 => c.transport(CLOCK, Transport::Restart),
            1301 => c.transport(CLOCK, Transport::Stop),
            1700 => c.transport(CLOCK, Transport::Run),
            2000 => c.transport(CLOCK, Transport::Restart),
            _ => {}
        })
    };
    let (l, r) = run();
    assert_eq!(first_diff(&l, &run().0), None, "two renders are identical");
    let restarts = [700 * BLOCK, 2000 * BLOCK];
    let want = expected_mod(&p, &r, &restarts, |_| Mods {
        morph: 0.4,
        ..Mods::default()
    });
    assert_eq!(first_diff(&l, &want), None);
}

/// Three cables, each its own square source with the probe's patterns, into a mixer. `routed`:
/// one macro (`m1` at 0.5) moves all three morphs at depths 1, 0.5 and -0.5 from a stored 0.5;
/// else the same morphs are stored on the cables, 1.0, 0.75 and 0.25.
fn three_cables(routed: bool) -> PatchState {
    let mut p = PatchState::new();
    p.modules.insert(CLOCK, module("clock", &[("bpm", 300.0)]));
    p.modules.insert(OUT, module("out", &[]));
    p.modules.insert(MACRO, module("macro", &[("m1", 0.5)]));
    p.modules.insert(6, module("mixer", &[]));
    p.cables.insert(1, jack((6, "out"), (OUT, "left"), &[]));
    p.cables
        .insert(2, jack((CLOCK, "gate"), (OUT, "right"), &[]));
    let depths = [1.0, 0.5, -0.5];
    for (k, depth) in depths.iter().enumerate() {
        let (lfo, cable) = (20 + k as u64, 30 + k as u64);
        let port = ["in1", "in2", "in3"][k];
        p.modules
            .insert(lfo, module("lfo", &[("rate_hz", 0.01), ("waveform", 3.0)]));
        let mut c = jack((lfo, "out"), (6, port), A);
        for (key, v) in B {
            c.params.insert(format!("b.{key}"), *v);
        }
        if routed {
            c.params.insert("morph".into(), 0.5);
            p.cables
                .insert(40 + k as u64, route((MACRO, "m1"), cable, "morph", *depth));
        } else {
            c.params.insert("morph".into(), 0.5 + depth * 0.5);
        }
        p.cables.insert(cable, c);
    }
    p
}

#[test]
fn one_macro_moves_three_cables_at_different_depths_and_signs() {
    let (routed, stored) = (three_cables(true), three_cables(false));
    let (l, r) = render(&mut compile(&routed, SR, 1).unwrap(), 800, |_, _| {});
    let (want, want_r) = render(&mut compile(&stored, SR, 1).unwrap(), 800, |_, _| {});
    assert_eq!(
        first_diff(&l, &want),
        None,
        "same sound as three stored morphs"
    );
    assert_eq!(r, want_r);
    assert!(l.iter().any(|&x| x != 0.0));
    // Each depth is its own runtime value: a change reaches one cable and rebuilds nothing.
    let mut c = compile(&routed, SR, 1).unwrap();
    assert_eq!(c.route_amount(40), Some(1.0));
    assert_eq!(c.route_amount(42), Some(-0.5));
    assert!(c.set_runtime(RuntimeTarget::Route { cable: 41 }, 0.25, false));
    assert_eq!(c.route_amount(41), Some(0.25));
    assert_eq!(c.route_amount(40), Some(1.0), "the others stay");
}

#[test]
fn the_amount_is_a_runtime_value_not_a_rebuild() {
    let a = with_macro(0.5, 1.0);
    let mut b = a.clone();
    b.cables
        .get_mut(&ROUTE)
        .unwrap()
        .params
        .insert("amount".into(), 0.5);
    let changes = runtime_changes(&a, &b).expect("an amount edit needs no compile");
    assert_eq!(changes, vec![(RuntimeTarget::Route { cable: ROUTE }, 0.5)]);
    // And in the graph it lands where the render shows it: halved depth is morph 0.25.
    let mut c = compile(&a, SR, 1).unwrap();
    for (t, v) in changes {
        assert!(c.set_runtime(t, v, false));
    }
    assert_eq!(render(&mut c, 600, |_, _| {}).0, left_of(&probe(0.25), 600));
    // A base edit of a cable whose only life is a route into it is in place too.
    let mut plain = probe(0.0);
    plain.cables.get_mut(&CABLE).unwrap().params.clear();
    plain.modules.insert(MACRO, module("macro", &[("m1", 0.5)]));
    plain
        .cables
        .insert(ROUTE, route((MACRO, "m1"), CABLE, "prob", 0.5));
    let mut edited = plain.clone();
    edited
        .cables
        .get_mut(&CABLE)
        .unwrap()
        .params
        .insert("morph".into(), 0.3);
    let r = runtime_changes(&plain, &edited).expect("the cable has a node already");
    assert_eq!(
        r,
        vec![(
            RuntimeTarget::Cable {
                cable: CABLE,
                slot: MORPH as u8
            },
            0.3
        )]
    );
}

#[test]
fn probability_and_glide_routes_move_their_cable() {
    // A macro at 1.0 into prob with amount -1 closes the cable: probability 100 - 100 = 0.
    let mut p = probe(0.0);
    p.modules.insert(MACRO, module("macro", &[("m1", 1.0)]));
    p.cables
        .insert(ROUTE, route((MACRO, "m1"), CABLE, "prob", -1.0));
    let l = left_of(&p, 600);
    assert!(l[2 * BLOCK..].iter().all(|&x| x == 0.0), "never passes");
    // Glide: a route into glide_ms slews the steps exactly as the stored glide would.
    let mut g = probe(0.0);
    g.modules.insert(MACRO, module("macro", &[("m1", 0.5)]));
    g.cables
        .insert(ROUTE, route((MACRO, "m1"), CABLE, "glide_ms", 0.01));
    let mut stored = probe(0.0);
    stored
        .cables
        .get_mut(&CABLE)
        .unwrap()
        .params
        .insert("glide_ms".into(), 10.0);
    assert_eq!(left_of(&g, 600), left_of(&stored, 600));
}

#[test]
fn a_patternless_cable_with_a_route_into_it_still_passes_its_signal() {
    let mut p = probe(0.0);
    p.cables.get_mut(&CABLE).unwrap().params.clear();
    p.modules.insert(MACRO, module("macro", &[("m1", 0.0)]));
    p.cables
        .insert(ROUTE, route((MACRO, "m1"), CABLE, "morph", 1.0));
    let mut plain = probe(0.0);
    plain.cables.get_mut(&CABLE).unwrap().params.clear();
    assert_eq!(left_of(&p, 300), left_of(&plain, 300));
}

#[test]
fn a_route_may_carry_a_pattern_and_be_moved_by_another_route() {
    // Route 10 (macro into cable 1's morph) has its own pattern: it gates the macro. Route 11
    // moves route 10's probability from a second macro output. The chain compiles and runs.
    let mut p = with_macro(1.0, 1.0);
    p.cables
        .get_mut(&ROUTE)
        .unwrap()
        .params
        .extend(params(&[("length", 2.0), ("s2", 0.0)]));
    p.cables
        .insert(11, route((MACRO, "m2"), ROUTE, "prob", -1.0));
    let mut c = compile(&p, SR, 1).unwrap();
    let (l, _) = render(&mut c, 600, |_, _| {});
    assert!(l.iter().all(|x| x.is_finite()));
    // m2 is 0, so route 11 adds nothing: the same as without it.
    let mut without = p.clone();
    without.cables.remove(&11);
    assert_eq!(l, left_of(&without, 600));
    // The gated route differs from the ungated one: its closed steps let no macro through.
    assert_ne!(l, left_of(&with_macro(1.0, 1.0), 600));
    // With m2 at 1.0 route 11 closes route 10 for good: the morph stays at its stored 0.
    let mut shut = p.clone();
    shut.modules
        .insert(MACRO, module("macro", &[("m1", 1.0), ("m2", 1.0)]));
    assert_eq!(
        left_of(&shut, 600)[3 * 2400..],
        left_of(&probe(0.0), 600)[3 * 2400..]
    );
}

fn compile_error(p: &PatchState) -> String {
    match compile(p, SR, 1) {
        Ok(_) => panic!("expected a compile error"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn the_nesting_rule_names_the_fix() {
    // A route into its own parameter.
    let mut p = with_macro(0.5, 1.0);
    p.cables
        .insert(ROUTE, route((MACRO, "m1"), ROUTE, "morph", 1.0));
    let e = compile_error(&p);
    assert!(
        e.contains("route 10") && e.contains("cannot modulate itself"),
        "{e}"
    );
    // Two routes that move each other.
    let mut p = with_macro(0.5, 1.0);
    p.cables
        .insert(ROUTE, route((MACRO, "m1"), 11, "prob", 1.0));
    p.cables
        .insert(11, route((MACRO, "m2"), ROUTE, "prob", 1.0));
    let e = compile_error(&p);
    assert!(
        e.contains("cable 10 -> cable 11") && e.contains("loop"),
        "{e}"
    );
    // A longer ring: 10 -> 11 -> 12 -> 10.
    p.cables
        .insert(ROUTE, route((MACRO, "m1"), 11, "prob", 1.0));
    p.cables.insert(11, route((MACRO, "m2"), 12, "prob", 1.0));
    p.cables
        .insert(12, route((MACRO, "m3"), ROUTE, "prob", 1.0));
    let e = compile_error(&p);
    assert!(e.contains("cable 10 -> cable 11 -> cable 12"), "{e}");
    // A param a route cannot move, and a cable that is not there.
    let p = {
        let mut p = with_macro(0.5, 1.0);
        p.cables
            .insert(ROUTE, route((MACRO, "m1"), CABLE, "length", 1.0));
        p
    };
    assert!(compile_error(&p).contains("prob"), "names what can move");
    let mut p = with_macro(0.5, 1.0);
    p.cables
        .insert(ROUTE, route((MACRO, "m1"), 99, "morph", 1.0));
    assert!(compile_error(&p).contains("cable 99, which is not in the patch"));
    // A bypassed route is not compiled, so it cannot close a loop.
    let mut p = with_macro(0.5, 1.0);
    p.cables
        .insert(ROUTE, route((MACRO, "m1"), ROUTE, "morph", 1.0));
    p.cables
        .get_mut(&ROUTE)
        .unwrap()
        .params
        .insert("bypass".into(), 1.0);
    assert!(compile(&p, SR, 1).is_ok());
}

fn ops() -> Vec<Op> {
    let m = |id, kind: &str| Op::AddModule {
        id,
        kind: kind.into(),
        pos: Vec2::default(),
    };
    let jack = |id, from: (u64, &str), to: (u64, &str)| Op::Connect {
        id,
        from: PortRef::Module {
            id: from.0,
            port: from.1.into(),
        },
        to: PortRef::Module {
            id: to.0,
            port: to.1.into(),
        },
    };
    let set = |id, k: &str, v| Op::SetParam {
        target: ParamTarget::Cable {
            id,
            param: k.into(),
        },
        value: v,
    };
    let mut ops = vec![
        m(CLOCK, "clock"),
        m(LFO, "lfo"),
        m(OUT, "out"),
        m(MACRO, "macro"),
        jack(CABLE, (LFO, "out"), (OUT, "left")),
        jack(2, (CLOCK, "gate"), (OUT, "right")),
        Op::Connect {
            id: ROUTE,
            from: PortRef::Module {
                id: MACRO,
                port: "m1".into(),
            },
            to: PortRef::CableParam {
                cable: CABLE,
                param: "morph".into(),
            },
        },
        set(ROUTE, "amount", 0.75),
    ];
    for (k, v) in A {
        ops.push(set(CABLE, k, *v));
    }
    for (k, v) in B {
        ops.push(set(CABLE, &format!("b.{k}"), *v));
    }
    for (id, k, v) in [
        (CLOCK, "bpm", 300.0),
        (LFO, "rate_hz", 0.01),
        (LFO, "waveform", 3.0),
        (MACRO, "m1", 0.6),
    ] {
        ops.push(Op::SetParam {
            target: ParamTarget::Module {
                id,
                param: k.into(),
            },
            value: v,
        });
    }
    ops
}

#[test]
fn undo_save_and_reload_keep_routes_into_cables() {
    let mut log = PatchLog::new();
    for op in ops() {
        log.append_new(op, 0, Source::User);
    }
    let full = log.state().clone();
    let sound = left_of(&full, 400);

    // Removing the cable takes the route into it along, and undo brings both back.
    log.append_new(Op::Disconnect { id: CABLE }, 0, Source::User);
    assert!(
        !log.state().cables.contains_key(&ROUTE),
        "no dangling route"
    );
    assert!(log.undo());
    assert_eq!(log.state(), &full, "undo restores the route exactly");
    // So does removing the source module of the cable.
    log.append_new(Op::RemoveModule { id: LFO }, 0, Source::User);
    assert!(!log.state().cables.contains_key(&ROUTE));
    assert!(log.undo());
    assert_eq!(log.state(), &full);
    // Removing the macro removes only its routes.
    log.append_new(Op::RemoveModule { id: MACRO }, 0, Source::User);
    assert!(log.state().cables.contains_key(&CABLE) && !log.state().cables.contains_key(&ROUTE));
    assert!(log.undo());
    assert_eq!(log.state(), &full);

    let dir = tempfile::tempdir().unwrap();
    kabl_core::save(dir.path(), &log).unwrap();
    let back = kabl_core::load(dir.path()).unwrap();
    assert_eq!(back.state(), &full);
    assert_eq!(left_of(back.state(), 400), sound, "recall sounds the same");
    let mut back = back;
    assert!(back.undo(), "history survives the reload");
}

#[test]
fn a_chain_goes_with_the_cable_it_hangs_from() {
    let mut log = PatchLog::new();
    for op in ops() {
        log.append_new(op, 0, Source::User);
    }
    // Route 11 moves route 10's probability.
    log.append_new(
        Op::Connect {
            id: 11,
            from: PortRef::Module {
                id: MACRO,
                port: "m2".into(),
            },
            to: PortRef::CableParam {
                cable: ROUTE,
                param: "prob".into(),
            },
        },
        0,
        Source::User,
    );
    let full = log.state().clone();
    log.append_new(Op::Disconnect { id: CABLE }, 0, Source::User);
    assert!(log
        .state()
        .cables
        .keys()
        .all(|id| *id != ROUTE && *id != 11));
    assert!(log.undo());
    assert_eq!(log.state(), &full);
}

#[test]
fn files_saved_before_cable_routes_load_and_render_unchanged() {
    // A v6 file: the log format is the same, only `meta.toml` says 6. Its render must equal the
    // model that has always defined a cable's output (`Settings::level_at` per pulse).
    let mut log = PatchLog::new();
    for op in ops() {
        if !matches!(
            &op,
            Op::Connect {
                to: PortRef::CableParam { .. },
                ..
            }
        ) && !matches!(
            &op,
            Op::SetParam {
                target: ParamTarget::Cable { id: ROUTE, .. },
                ..
            }
        ) {
            log.append_new(op, 0, Source::User);
        }
    }
    let dir = tempfile::tempdir().unwrap();
    kabl_core::save(dir.path(), &log).unwrap();
    let meta = std::fs::read_to_string(dir.path().join("meta.toml")).unwrap();
    std::fs::write(
        dir.path().join("meta.toml"),
        meta.replace("schema_version = 7", "schema_version = 6"),
    )
    .unwrap();
    let back = kabl_core::load(dir.path()).expect("a v6 file loads");
    assert_eq!(back.state(), log.state());
    let (l, r) = render(&mut compile(back.state(), SR, 1).unwrap(), 600, |_, _| {});
    let want = expected_mod(back.state(), &r, &[], |_| Mods::default());
    assert_eq!(first_diff(&l, &want), None);
}

#[test]
fn audio_path_does_not_allocate_with_routes_running_and_edited() {
    let mut c = compile(&with_macro(0.5, 1.0), SR, 1).unwrap();
    for b in 0..400 {
        assert_no_alloc(|| {
            if b % 3 == 0 {
                c.set_runtime(
                    RuntimeTarget::Route { cable: ROUTE },
                    (b % 7) as f32 / 7.0 - 0.3,
                    b % 2 == 0,
                );
            }
            c.process_block();
        });
    }
    assert!(c.left().iter().all(|x| x.is_finite()));
}

#[test]
fn the_macro_morph_demo_patch_loads_and_its_macro_changes_the_sound() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../patches/functional-cables/macro-morph");
    let log = kabl_core::load(&dir).expect("the demo patch loads");
    let state = log.state();
    let routes: Vec<CableId> = state
        .cables
        .iter()
        .filter(|(_, c)| matches!(&c.to, PortRef::CableParam { param, .. } if param == "morph"))
        .map(|(&id, _)| id)
        .collect();
    assert_eq!(routes.len(), 3, "one macro, three morphs");
    let knob = *state
        .modules
        .iter()
        .find(|(_, m)| m.kind == "macro")
        .map(|(id, _)| id)
        .unwrap();
    let run = |m1: f32| {
        let mut c = compile(state, SR, 4).unwrap();
        let depths: Vec<f32> = routes.iter().map(|&r| c.route_amount(r).unwrap()).collect();
        assert_eq!(depths, vec![1.0, 0.5, -1.0], "three depths, one negative");
        let target = RuntimeTarget::Param {
            id: knob,
            kind: "macro",
            index: 0,
        };
        render(&mut c, 3000, |c, _| {
            c.set_runtime(target, m1, false);
        })
        .0
    };
    let (low, high) = (run(0.0), run(1.0));
    assert!(low.iter().chain(&high).all(|x| x.is_finite()));
    assert_ne!(low, high, "the macro moves the cables");
    assert_eq!(low, run(0.0), "and the render repeats");
}
