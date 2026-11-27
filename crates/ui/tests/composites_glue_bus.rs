//! The factory composite `kabl.glue-bus` v1 (docs/composites/GLUE-BUS.md): it is what its
//! generator writes, has the catalogue's interface, passes a signal and keeps silence silent,
//! ducks under Duck In and passes exactly the single-compressor chain without it, plays the same
//! inside a parent composite, and stores values outside its controls' ranges unclamped (G2).
use kabl_core::{CompositeId, ModuleId, PatchState, PortRef, Vec2};
use kabl_ui::{composites, PatchEditor};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[path = "../../../tools/examples/glue_bus_composite.rs"]
mod generator;

const SR: f32 = 48000.0;

fn jack(id: ModuleId, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

fn package() -> composites::Package {
    let bank = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../composites");
    let entry = composites::factory_entries(&bank)
        .into_iter()
        .find(|e| e.definition.id == "kabl.glue-bus" && e.definition.version == 1)
        .expect("kabl.glue-bus v1 is in the bank");
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

/// The leaf a control of composite `id` sets.
fn control(e: &PatchEditor, id: CompositeId, label: &str) -> ModuleId {
    match &e.state().composites[&id]
        .controls
        .values()
        .find(|x| x.label == label)
        .unwrap_or_else(|| panic!("no control {label}"))
        .target
    {
        PortRef::Param { id, .. } => *id,
        other => panic!("{label}: {other:?}"),
    }
}

fn osc(e: &mut PatchEditor, y: f32, waveform: f32, hz: f32) -> ModuleId {
    let id = e.add_module("osc.va", Vec2 { x: 24.0, y });
    e.set_param(id, "waveform", waveform);
    e.set_param(id, "base_hz", hz);
    id
}

/// A saw at 110 Hz into In, Glue Bus into the output; with `duck`, a sine at 55 Hz on Duck In.
/// Returns the saw, the output and the card.
fn bus(e: &mut PatchEditor, duck: bool) -> (ModuleId, ModuleId, CompositeId) {
    let src = osc(e, 20.0, 2.0, 110.0);
    let out = e.add_module("out", Vec2 { x: 1100.0, y: 20.0 });
    let id = composites::insert(e, &package()).unwrap();
    e.connect(jack(src, "out"), port(e, id, "In"));
    if duck {
        let kick = osc(e, 420.0, 0.0, 55.0);
        e.connect(jack(kick, "out"), port(e, id, "Duck In"));
    }
    e.connect(port(e, id, "Out"), jack(out, "in"));
    (src, out, id)
}

/// The modules in no composite.
fn top_level(e: &PatchEditor) -> BTreeSet<ModuleId> {
    let p = e.state();
    p.modules
        .keys()
        .copied()
        .filter(|m| !p.composites.values().any(|c| c.members.contains(m)))
        .collect()
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

/// The second second, after the compressors have settled.
fn settled(audio: &[f32]) -> &[f32] {
    &audio[audio.len() / 2..]
}

/// The committed definition is what the generator writes, so the two cannot drift apart.
#[test]
fn the_committed_glue_bus_is_what_the_generator_writes() {
    let committed = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../composites/glue-bus/v1.json"),
    )
    .unwrap();
    assert!(
        generator::bytes() == committed,
        "composites/glue-bus/v1.json differs from `glue_bus_composite write composites`"
    );
}

/// Ports, and every control's target with its range and stored default, as GLUE-BUS.md lists
/// them; Duck In reaches the second compressor's sidechain; Resonance is off the face.
#[test]
fn glue_bus_has_the_catalogue_interface() {
    let pkg = package();
    let p = &pkg.patch;
    let c = &p.composites[&pkg.root];
    assert_eq!(c.name, "Glue Bus");
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
    let want: Vec<_> = [
        (1, "In", "drive", "in"),
        (2, "Duck In", "gain", "in"),
        (3, "Out", "compressor", "out"),
    ]
    .iter()
    .map(|(n, l, k, e)| (*n, l.to_string(), k.to_string(), e.to_string()))
    .collect();
    assert_eq!(ends(&c.ports), want);
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
        ("Drive", "drive", "drive_db", 0.0, 36.0, 4.0),
        ("Tone", "filter.ladder", "cutoff_hz", 20.0, 20000.0, 1600.0),
        ("Resonance", "filter.ladder", "resonance", 0.0, 1.0, 0.2),
        ("Trim", "gain", "gain_db", -60.0, 24.0, -2.0),
        ("Threshold", "compressor", "threshold_db", -60.0, 0.0, -24.0),
        ("Ratio", "compressor", "ratio", 1.0, 20.0, 4.0),
        ("Attack", "compressor", "attack_ms", 0.1, 200.0, 4.0),
        ("Release", "compressor", "release_ms", 5.0, 2000.0, 110.0),
        ("Duck", "compressor", "threshold_db", -60.0, 0.0, -34.0),
    ]
    .into_iter()
    .map(|(l, k, n, lo, hi, d)| (l.to_string(), k.to_string(), n.to_string(), lo, hi, d))
    .collect();
    assert_eq!(controls, expected);
    // Threshold to Release set the first compressor, Duck the second: the one on Out, whose
    // sidechain the Duck In gain feeds.
    let leaf = |label: &str| match &c
        .controls
        .values()
        .find(|e| e.label == label)
        .unwrap()
        .target
    {
        PortRef::Param { id, .. } => *id,
        _ => unreachable!(),
    };
    let first = leaf("Threshold");
    for label in ["Ratio", "Attack", "Release"] {
        assert_eq!(leaf(label), first, "{label}");
    }
    let (feed, duck) = match (&c.ports[&2].target, &c.ports[&3].target) {
        (PortRef::Module { id: f, .. }, PortRef::Module { id: d, .. }) => (*f, *d),
        _ => unreachable!(),
    };
    assert_eq!(leaf("Duck"), duck);
    assert_ne!(duck, first);
    let wired = |from: ModuleId, fp: &str, to: ModuleId, tp: &str| {
        p.cables
            .values()
            .any(|x| x.from == jack(from, fp) && x.to == jack(to, tp))
    };
    assert!(
        wired(feed, "out", duck, "sc"),
        "Duck In feeds the sidechain"
    );
    assert!(
        wired(first, "out", duck, "in"),
        "the music runs through the duck"
    );
    let face = c.panel.as_ref().expect("a face");
    for (&key, e) in &c.controls {
        assert_eq!(
            face.placements.contains_key(&key),
            e.label != "Resonance",
            "{} on the face",
            e.label
        );
    }
    for &key in c.ports.keys() {
        assert!(face.placements.contains_key(&key), "port {key} on the face");
    }
}

/// A saw into In comes out; with nothing on In, the output is silent: no DC or noise from the
/// drive, the filter or the makeup gain.
#[test]
fn glue_bus_passes_a_signal_and_keeps_silence_silent() {
    let mut e = PatchEditor::seed_from(&PatchState::default());
    let (src, _, _) = bus(&mut e, false);
    let audio = render(e.state());
    assert!(rms(&audio) > 0.01, "Out sounds: RMS {}", rms(&audio));
    e.remove_module(src);
    let silent = render(e.state());
    assert!(silent.iter().all(|s| s.is_finite()));
    assert!(rms(&silent) < 1e-6, "silence in: RMS {}", rms(&silent));
}

/// A loud signal on Duck In turns Out down (measured as gain reduction); unpatched, Out is the
/// single-compressor chain (drive → ladder → trim → compressor, the same settings) sample for
/// sample within float tolerance.
#[test]
fn duck_in_ducks_and_unpatched_it_is_the_single_compressor_chain() {
    let mut e = PatchEditor::seed_from(&PatchState::default());
    let (_, _, id) = bus(&mut e, false);
    let open = render(e.state());
    assert!(open.iter().all(|s| s.is_finite()), "no NaN without Duck In");
    let mut d = PatchEditor::seed_from(&PatchState::default());
    bus(&mut d, true);
    let ducked = render(d.state());
    let gr_db = 20.0 * (rms(settled(&open)) / rms(settled(&ducked))).log10();
    println!("Duck In: {gr_db:.1} dB of gain reduction");
    assert!(gr_db > 6.0, "Duck In turns Out down: {gr_db:.1} dB");

    // The same stages, flat, with no second compressor.
    let mut f = PatchEditor::seed_from(&PatchState::default());
    let src = osc(&mut f, 20.0, 2.0, 110.0);
    let out = f.add_module("out", Vec2 { x: 1100.0, y: 20.0 });
    let mut prev = (src, "out");
    for label in ["Drive", "Tone", "Trim", "Threshold"] {
        let leaf = &e.state().modules[&control(&e, id, label)];
        let m = f.add_module(&leaf.kind, Vec2 { x: 300.0, y: 20.0 });
        for (name, v) in &leaf.params {
            f.set_param(m, name, *v);
        }
        f.connect(jack(prev.0, prev.1), jack(m, "in"));
        prev = (m, "out");
    }
    f.connect(jack(prev.0, prev.1), jack(out, "in"));
    let single = render(f.state());
    assert!(rms(&single) > 0.01);
    let worst = open
        .iter()
        .zip(&single)
        .fold(0.0f32, |m, (a, b)| m.max((a - b).abs()));
    assert!(worst < 1e-6, "unpatched Duck In: off by {worst}");
}

/// The same patch with the sources, the output and the card wrapped in a parent composite
/// renders the same samples.
#[test]
fn glue_bus_plays_inside_a_parent_composite() {
    let mut e = PatchEditor::seed_from(&PatchState::default());
    let (_, _, card) = bus(&mut e, true);
    let flat = render(e.state());
    let leaves = top_level(&e);
    assert_eq!(leaves.len(), 3, "two oscillators and the output");
    let parent = composites::encapsulate(&mut e, leaves, BTreeSet::from([card]), "Song").unwrap();
    assert_eq!(e.state().composites[&card].parent, Some(parent));
    let nested = render(e.state());
    // Ducked hard by the full-scale sine, so quieter than the saw alone.
    assert!(rms(&nested) > 1e-3, "plays: RMS {}", rms(&nested));
    assert!(flat == nested, "grouping changes nothing in the render");
}

/// G2: a stored value outside its control's range (Trim at −70 dB, below the gain's −60, and
/// the compressor's makeup at −2 dB, below its 0, as acid-line stores it) survives packaging the
/// patch and inserting it again, unclamped.
#[test]
fn values_outside_a_control_range_are_stored_unclamped() {
    let mut e = PatchEditor::seed_from(&PatchState::default());
    let (_, _, card) = bus(&mut e, true);
    let (trim, comp) = (control(&e, card, "Trim"), control(&e, card, "Threshold"));
    e.set_param(trim, "gain_db", -70.0);
    e.set_param(comp, "makeup_db", -2.0);
    let leaves = top_level(&e);
    let song = composites::encapsulate(&mut e, leaves, BTreeSet::from([card]), "Song").unwrap();
    let bytes = serde_json::to_vec(&composites::package(e.state(), song).unwrap()).unwrap();
    let pkg = composites::decode(&bytes).unwrap();
    let mut back = PatchEditor::seed_from(&PatchState::default());
    composites::insert(&mut back, &pkg).unwrap();
    let card = *back
        .state()
        .composites
        .iter()
        .find(|(_, c)| c.name == "Glue Bus")
        .unwrap()
        .0;
    let (trim, comp) = (
        control(&back, card, "Trim"),
        control(&back, card, "Threshold"),
    );
    let m = &back.state().modules;
    assert_eq!(m[&trim].params["gain_db"], -70.0);
    assert_eq!(m[&comp].params["makeup_db"], -2.0);
    assert!(rms(&render(back.state())).is_finite());
}
