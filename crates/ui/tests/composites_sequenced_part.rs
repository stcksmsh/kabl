//! The factory composites `kabl.sequenced-part` v1 and `kabl.sequenced-part-sub` v1
//! (docs/composites/SEQUENCED-PART.md): each is what its generator writes, has the catalogue's
//! interface, plays its line on its own when clocked, still plays inside a parent composite,
//! and a cue switches its inner sequencer's pattern while the card stays closed.
use basedrop::Collector;
use kabl_core::{CompositeId, ModuleId, PatchState, PortRef, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::{Command, PatchEngine};
use kabl_ui::{composites, cues, PatchEditor};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[path = "../../../tools/examples/sequenced_part_composite.rs"]
mod generator;

const SR: f32 = 48000.0;
/// 120 BPM: a 16th is 6000 samples, a bar 96000.
const BAR: usize = 16 * 6000;

fn jack(id: ModuleId, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

fn package(name: &str) -> composites::Package {
    let bank = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../composites");
    let def = format!("kabl.{name}");
    let entry = composites::factory_entries(&bank)
        .into_iter()
        .find(|e| e.definition.id == def && e.definition.version == 1)
        .unwrap_or_else(|| panic!("{def} v1 is in the bank"));
    composites::read_factory(&entry).unwrap()
}

fn port(e: &PatchEditor, id: CompositeId, label: &str) -> PortRef {
    e.state().composites[&id]
        .ports
        .values()
        .find(|x| x.label == label)
        .unwrap_or_else(|| panic!("no port {label}"))
        .target
        .clone()
}

/// A clock at 120 BPM and an output; the part inserted with Clock, Reset and Out patched to
/// them. Returns the clock, the output and the part.
fn clocked_part(e: &mut PatchEditor, name: &str) -> (ModuleId, ModuleId, CompositeId) {
    let clock = e.add_module("clock", Vec2 { x: 24.0, y: 20.0 });
    e.set_param(clock, "bpm", 120.0);
    let out = e.add_module("out", Vec2 { x: 1100.0, y: 20.0 });
    let id = composites::insert(e, &package(name)).unwrap();
    let (c, r, o) = (
        port(e, id, "Clock"),
        port(e, id, "Reset"),
        port(e, id, "Out"),
    );
    e.connect(jack(clock, "gate"), c);
    e.connect(jack(clock, "reset"), r);
    e.connect(o, jack(out, "in"));
    (clock, out, id)
}

/// Two seconds, interleaved stereo.
fn render(p: &PatchState) -> Vec<f32> {
    let mut engine = kabl_engine::compile::compile(p, SR, 8).unwrap();
    let mut audio = Vec::new();
    while audio.len() < 2 * 48000 * 2 {
        engine.process_block();
        audio.extend(
            engine
                .left()
                .iter()
                .zip(engine.right())
                .flat_map(|(l, r)| [*l, *r]),
        );
    }
    audio
}

fn rms(s: &[f32]) -> f32 {
    (s.iter().map(|v| v * v).sum::<f32>() / s.len().max(1) as f32).sqrt()
}

fn only(p: &PatchState, kind: &str) -> ModuleId {
    let ids: Vec<_> = p
        .modules
        .iter()
        .filter(|(_, m)| m.kind == kind)
        .map(|(id, _)| *id)
        .collect();
    assert_eq!(ids.len(), 1, "one {kind}");
    ids[0]
}

/// The committed definitions are what the generator writes, so they cannot drift apart.
#[test]
fn the_committed_sequenced_parts_are_what_the_generator_writes() {
    for name in generator::NAMES {
        let committed = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../composites/{name}/v1.json")),
        )
        .unwrap();
        assert!(
            generator::bytes(name) == committed,
            "composites/{name}/v1.json differs from `sequenced_part_composite write composites`"
        );
    }
}

/// Ports, and every control's target with its range and stored default, as SEQUENCED-PART.md
/// lists them.
#[test]
fn sequenced_parts_have_the_catalogue_interface() {
    // Per definition: name, card name, then the stored Gate, Wave, Unison, Detune, envelope,
    // Level and Tone.
    let level_b = 20.0 * 0.7f32.log10();
    for (name, label, gate, wave, unison, detune, env, level, tone) in [
        (
            "sequenced-part",
            "Sequenced Part",
            50.0,
            3.0,
            2.0,
            14.0,
            [1.0, 200.0, 0.7, 40.0],
            -6.0,
            3000.0,
        ),
        (
            "sequenced-part-sub",
            "Sequenced Part + Sub",
            55.0,
            2.0,
            1.0,
            6.0,
            [1.0, 140.0, 0.4, 40.0],
            level_b,
            8000.0,
        ),
    ] {
        let pkg = package(name);
        let p = &pkg.patch;
        let c = &p.composites[&pkg.root];
        assert_eq!(c.name, label);
        let ends = |m: &BTreeMap<u64, kabl_core::Exposure>| {
            m.iter()
                .map(|(&key, e)| {
                    let (id, end) = match &e.target {
                        PortRef::Module { id, port } => (*id, port.clone()),
                        PortRef::Param { id, param } => (*id, param.clone()),
                        PortRef::CableParam { .. } => panic!("{}: a cable parameter", e.label),
                    };
                    assert!(e.also.is_empty(), "{}: one target", e.label);
                    (key, e.label.clone(), p.modules[&id].kind.clone(), end)
                })
                .collect::<Vec<_>>()
        };
        let want = |rows: &[(u64, &str, &str, &str)]| {
            rows.iter()
                .map(|(n, l, k, e)| (*n, l.to_string(), k.to_string(), e.to_string()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            ends(&c.ports),
            want(&[
                (1, "Clock", "seq", "clock"),
                (2, "Reset", "seq", "reset"),
                (3, "Out", "filter.svf", "lp"),
                (4, "Pitch out", "seq", "pitch"),
                (5, "Gate out", "seq", "gate"),
                (6, "Gate mod", "seq", "gate_len"),
                (7, "Tone mod", "filter.svf", "cutoff_hz"),
                (8, "Level mod", "gain", "gain_db"),
            ]),
            "{name}"
        );
        // (label, kind, param, min, max, default).
        let controls: Vec<_> = c
            .controls
            .values()
            .map(|e| {
                let PortRef::Param { id, param } = &e.target else {
                    panic!("{} is not a parameter", e.label);
                };
                let m = &p.modules[id];
                let info = kabl_modules::registry::info_for(&m.kind).unwrap();
                let q = info.params.iter().find(|q| q.name == param).unwrap();
                let stored = m.params.get(param).copied().unwrap_or(q.default);
                (
                    e.label.clone(),
                    m.kind.clone(),
                    param.clone(),
                    q.min,
                    q.max,
                    stored,
                )
            })
            .collect();
        let expected: Vec<_> = [
            ("Steps", "seq", "length", 1.0, 32.0, 16.0),
            ("Gate", "seq", "gate_len", 1.0, 100.0, gate),
            ("Transpose", "seq", "transpose", -24.0, 24.0, 0.0),
            ("Wave", "osc.va", "waveform", 0.0, 3.0, wave),
            ("Unison", "osc.va", "unison", 1.0, 16.0, unison),
            ("Detune", "osc.va", "detune", 0.0, 100.0, detune),
            ("Attack", "env.adsr", "attack_ms", 0.1, 10000.0, env[0]),
            ("Decay", "env.adsr", "decay_ms", 0.1, 10000.0, env[1]),
            ("Sustain", "env.adsr", "sustain", 0.0, 1.0, env[2]),
            ("Release", "env.adsr", "release_ms", 0.1, 10000.0, env[3]),
            ("Level", "gain", "gain_db", -60.0, 24.0, level),
            ("Tone", "filter.svf", "cutoff_hz", 20.0, 20000.0, tone),
        ]
        .into_iter()
        .map(|(l, k, n, lo, hi, d)| (l.to_string(), k.to_string(), n.to_string(), lo, hi, d))
        .collect();
        assert_eq!(controls, expected, "{name}");
        // Wave and Unison belong to the oscillator the sequencer's pitch plays: in the sub
        // part, the main oscillator, not the square under it.
        let osc = |label: &str| match &c
            .controls
            .values()
            .find(|e| e.label == label)
            .unwrap()
            .target
        {
            PortRef::Param { id, .. } => *id,
            _ => unreachable!(),
        };
        assert_eq!(osc("Wave"), osc("Unison"));
        assert_eq!(osc("Wave"), osc("Detune"));
    }
}

/// Clock into each part into the output: the line sounds, and without the clock it is silent.
#[test]
fn sequenced_parts_alone_play_when_clocked() {
    for name in generator::NAMES {
        let mut e = PatchEditor::seed_from(&PatchState::default());
        let (clock, _, _) = clocked_part(&mut e, name);
        let audio = render(e.state());
        assert!(rms(&audio) > 0.01, "{name} plays: RMS {}", rms(&audio));
        e.remove_module(clock);
        let unclocked = render(e.state());
        assert!(
            rms(&unclocked) < 1e-6,
            "{name}: nothing plays unclocked: RMS {}",
            rms(&unclocked)
        );
    }
}

/// Both oscillators of the sub part sound: with either one's cable into the mixer removed, the
/// part still plays, so a part where the sub is dead cannot pass.
#[test]
fn both_oscillators_of_the_sub_part_sound() {
    let mut e = PatchEditor::seed_from(&PatchState::default());
    clocked_part(&mut e, "sequenced-part-sub");
    let mixer = only(e.state(), "mixer");
    let feeds: Vec<_> = e
        .state()
        .cables
        .iter()
        .filter(|(_, c)| c.to.module_id() == Some(mixer))
        .map(|(id, c)| (*id, c.from.module_id().unwrap()))
        .collect();
    assert_eq!(feeds.len(), 2, "main and sub into the mixer");
    assert!(feeds
        .iter()
        .all(|(_, from)| e.state().modules[from].kind == "osc.va"));
    for (keep, from) in &feeds {
        let mut one = e.state().clone();
        one.cables
            .retain(|id, _| id == keep || !feeds.iter().any(|(f, _)| f == id));
        let audio = render(&one);
        assert!(
            rms(&audio) > 1e-3,
            "oscillator #{from} alone plays: RMS {}",
            rms(&audio)
        );
    }
}

/// The same patch with the clock, the output and the part wrapped in a parent composite renders
/// the same samples.
#[test]
fn sequenced_parts_play_inside_a_parent_composite() {
    for name in generator::NAMES {
        let mut e = PatchEditor::seed_from(&PatchState::default());
        let (clock, out, part) = clocked_part(&mut e, name);
        let flat = render(e.state());
        let parent = composites::encapsulate(
            &mut e,
            BTreeSet::from([clock, out]),
            BTreeSet::from([part]),
            "Song",
        )
        .unwrap();
        assert_eq!(e.state().composites[&part].parent, Some(parent));
        let nested = render(e.state());
        assert!(rms(&nested) > 0.01, "{name} plays: RMS {}", rms(&nested));
        assert!(
            flat == nested,
            "{name}: grouping changes nothing in the render"
        );
    }
}

/// A cue addresses the sequencer inside the part by its leaf id: launching it switches that
/// sequencer to bank B (all rests here) on the next bar, while the card stays closed.
#[test]
fn a_cue_launches_the_inner_pattern_with_the_part_closed() {
    for name in generator::NAMES {
        let mut e = PatchEditor::seed_from(&PatchState::default());
        let (_, _, part) = clocked_part(&mut e, name);
        let seq = only(e.state(), "seq");
        assert!(e.state().composites[&part].members.contains(&seq));
        assert!(
            composites::hidden(e.state(), &BTreeSet::new(), seq),
            "the sequencer is inside the closed card"
        );
        e.set_param(seq, "b.length", 16.0);
        for n in 1..=16 {
            e.set_param(seq, &format!("b.g{n}"), 0.0);
        }
        let deck = e.add_module("cues", Vec2 { x: 24.0, y: 600.0 });
        let n = cues::add(&mut e, deck, &|_| 0).unwrap();
        cues::set_target(&mut e, deck, n, seq, Some(1));
        assert!(e.state().modules[&deck]
            .params
            .contains_key(&format!("cue{n}.seq.{seq}")));
        composites::validate(e.state()).unwrap();

        let collector = Collector::new();
        let mut engine = PatchEngine::new(&collector.handle(), e.state(), SR, 8).unwrap();
        let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
        let mut left = Vec::new();
        let mut run_to = |engine: &mut PatchEngine, samples: usize| {
            while left.len() < samples {
                engine.process_block(&mut l, &mut r);
                left.extend_from_slice(&l);
            }
            left.clone()
        };
        run_to(&mut engine, BAR / 2);
        let mut launches = Vec::new();
        cues::launch(&mut launches, e.state(), deck, n).unwrap();
        match launches.as_slice() {
            [Command::Launch(l)] => assert_eq!(l.targets[0], (seq, 1)),
            other => panic!("{other:?}"),
        }
        for c in &launches {
            engine.command(c);
        }
        let audio = run_to(&mut engine, 3 * BAR);
        assert!(
            rms(&audio[..BAR]) > 0.01,
            "{name}: bank A plays in the first bar: RMS {}",
            rms(&audio[..BAR])
        );
        assert!(
            rms(&audio[2 * BAR..]) < 1e-6,
            "{name}: bank B's rests play from the cue's bar on: RMS {}",
            rms(&audio[2 * BAR..])
        );
    }
}
