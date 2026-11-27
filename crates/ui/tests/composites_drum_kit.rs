//! The factory composite `kabl.drum-kit` v1 (docs/composites/DRUM-KIT.md): it is what its
//! generator writes, has the catalogue's interface, plays a beat on its own when clocked, and
//! still plays inside a parent composite.
use kabl_core::{CompositeId, ModuleId, PatchState, PortRef, Vec2};
use kabl_ui::{composites, PatchEditor};
use std::collections::BTreeSet;
use std::path::Path;

#[path = "../../../tools/examples/drum_kit_composite.rs"]
mod generator;

fn jack(id: ModuleId, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

fn drum_kit_package() -> composites::Package {
    let bank = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../composites");
    let entry = composites::factory_entries(&bank)
        .into_iter()
        .find(|e| e.definition.id == "kabl.drum-kit" && e.definition.version == 1)
        .expect("kabl.drum-kit v1 is in the bank");
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

/// A clock and an output; Drum Kit inserted with Clock, Reset and Out patched to them. Returns
/// the clock and Drum Kit.
fn clocked_kit(e: &mut PatchEditor) -> (ModuleId, ModuleId, CompositeId) {
    let clock = e.add_module("clock", Vec2 { x: 24.0, y: 20.0 });
    let out = e.add_module("out", Vec2 { x: 1100.0, y: 20.0 });
    let id = composites::insert(e, &drum_kit_package()).unwrap();
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
    let mut engine = kabl_engine::compile::compile(p, 48000.0, 8).unwrap();
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

/// The committed definition is what the generator writes, so the two cannot drift apart.
#[test]
fn the_committed_drum_kit_is_what_the_generator_writes() {
    let committed = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../composites/drum-kit/v1.json"),
    )
    .unwrap();
    assert!(
        generator::bytes() == committed,
        "composites/drum-kit/v1.json differs from `drum_kit_composite write composites`"
    );
}

/// Ports, and every control's target with its range and stored default, as DRUM-KIT.md lists
/// them.
#[test]
fn drum_kit_has_the_catalogue_interface() {
    let pkg = drum_kit_package();
    let p = &pkg.patch;
    let c = &p.composites[&pkg.root];
    assert_eq!(c.name, "Drum Kit");
    let ends = |m: &std::collections::BTreeMap<u64, kabl_core::Exposure>| {
        m.values()
            .map(|e| {
                let (id, end) = match &e.target {
                    PortRef::Module { id, port } => (*id, port.clone()),
                    PortRef::Param { id, param } => (*id, param.clone()),
                    PortRef::CableParam { .. } => panic!("{}: a cable parameter", e.label),
                };
                assert!(e.also.is_empty(), "{}: one target", e.label);
                (e.label.clone(), p.modules[&id].kind.clone(), end)
            })
            .collect::<Vec<_>>()
    };
    let want = |rows: &[(&str, &str, &str)]| {
        rows.iter()
            .map(|(l, k, e)| (l.to_string(), k.to_string(), e.to_string()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        ends(&c.ports),
        want(&[
            ("Clock", "drum.grid", "clock"),
            ("Reset", "drum.grid", "reset"),
            ("Out", "mixer", "out"),
            ("Kick", "drum.kick", "out"),
            ("Hats/Bus", "filter.ladder", "out"),
            ("Kick mod", "mixer", "level1"),
            ("Clap mod", "mixer", "level1"),
            ("Hat mod", "mixer", "level2"),
            ("Tom mod", "mixer", "level2"),
            ("Tone mod", "filter.ladder", "cutoff_hz"),
        ])
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
        ("Kick Tune", "drum.kick", "tune_hz", 30.0, 120.0, 55.0),
        ("Kick Decay", "drum.kick", "decay_ms", 60.0, 2000.0, 250.0),
        ("Kick Drive", "drum.kick", "drive_db", 0.0, 30.0, 8.0),
        ("Snare Decay", "drum.snare", "decay_ms", 60.0, 1200.0, 200.0),
        ("Snare Level", "drum.snare", "level_db", -24.0, 6.0, -8.0),
        ("Clap Level", "drum.clap", "level_db", -24.0, 6.0, -10.0),
        ("Hat Level", "drum.hat", "level_db", -24.0, 6.0, -16.0),
        ("Tom Tune", "drum.tom", "tune_hz", 60.0, 500.0, 120.0),
        ("Tom Level", "drum.tom", "level_db", -24.0, 6.0, -10.0),
        (
            "Bus Tone",
            "filter.ladder",
            "cutoff_hz",
            20.0,
            20000.0,
            20000.0,
        ),
    ]
    .into_iter()
    .map(|(l, k, n, lo, hi, d)| (l.to_string(), k.to_string(), n.to_string(), lo, hi, d))
    .collect();
    assert_eq!(controls, expected);
    // The mod ports land on the voices' shares: kick and tom in the kit mixer (the one Out
    // reads), clap and hats in the bus mixer (the one the ladder reads).
    let target = |label: &str| match &c.ports.values().find(|e| e.label == label).unwrap().target {
        PortRef::Param { id, .. } => *id,
        _ => unreachable!(),
    };
    let PortRef::Module { id: out, .. } = &c.ports[&3].target else {
        panic!("Out is a jack");
    };
    assert_eq!(target("Kick mod"), *out);
    assert_eq!(target("Tom mod"), *out);
    assert_eq!(target("Clap mod"), target("Hat mod"));
    assert!(p
        .cables
        .values()
        .any(|cable| cable.from.module_id() == Some(target("Clap mod"))
            && p.modules[&cable.to.module_id().unwrap()].kind == "filter.ladder"));
}

/// Clock into Drum Kit into the output: the beat sounds, and without the clock it is silent.
#[test]
fn drum_kit_alone_plays_when_clocked() {
    let mut e = PatchEditor::seed_from(&PatchState::default());
    let (clock, _, _) = clocked_kit(&mut e);
    let audio = render(e.state());
    assert!(rms(&audio) > 0.01, "the beat plays: RMS {}", rms(&audio));
    e.remove_module(clock);
    let unclocked = render(e.state());
    assert!(
        rms(&unclocked) < 1e-6,
        "nothing plays unclocked: RMS {}",
        rms(&unclocked)
    );
}

/// Every grid row plays its voice: with only that row's trigger cable left, the kit still
/// sounds, so a kit where only the kick fires cannot pass.
#[test]
fn every_drum_kit_voice_triggers() {
    let mut e = PatchEditor::seed_from(&PatchState::default());
    clocked_kit(&mut e);
    let grid = e
        .state()
        .modules
        .iter()
        .find(|(_, m)| m.kind == "drum.grid")
        .map(|(id, _)| *id)
        .unwrap();
    let triggers: Vec<_> = e
        .state()
        .cables
        .iter()
        .filter(|(_, c)| c.from.module_id() == Some(grid))
        .map(|(id, c)| (*id, c.clone()))
        .collect();
    let rows: BTreeSet<_> = triggers
        .iter()
        .map(|(_, c)| match &c.from {
            PortRef::Module { port, .. } => port.clone(),
            _ => panic!("a grid row is a jack"),
        })
        .collect();
    assert_eq!(
        rows,
        BTreeSet::from(["kick", "snare", "clap", "chat", "ohat", "tom"].map(String::from))
    );
    for (keep, cable) in &triggers {
        let mut one = e.state().clone();
        one.cables
            .retain(|id, _| id == keep || !triggers.iter().any(|(t, _)| t == id));
        let audio = render(&one);
        assert!(
            rms(&audio) > 1e-3,
            "{:?} alone plays: RMS {}",
            cable.from,
            rms(&audio)
        );
    }
}

/// The same patch with the clock, the output and Drum Kit wrapped in a parent composite renders
/// the same beat.
#[test]
fn drum_kit_plays_inside_a_parent_composite() {
    let mut e = PatchEditor::seed_from(&PatchState::default());
    let (clock, out, kit) = clocked_kit(&mut e);
    let flat = render(e.state());
    let parent = composites::encapsulate(
        &mut e,
        BTreeSet::from([clock, out]),
        BTreeSet::from([kit]),
        "Beat",
    )
    .unwrap();
    assert_eq!(e.state().composites[&kit].parent, Some(parent));
    let nested = render(e.state());
    assert!(rms(&nested) > 0.01, "the beat plays: RMS {}", rms(&nested));
    assert!(flat == nested, "grouping changes nothing in the render");
}
