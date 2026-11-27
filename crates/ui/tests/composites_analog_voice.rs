//! The factory composite `kabl.analog-voice` v1 (docs/composites/ANALOG-VOICE.md): it is what
//! its generator writes, has the catalogue's interface, plays notes in tune and keeps silence
//! silent, renders Cinder Block's voice when set to its values, moves its filter with Env
//! Amount (a cable parameter, G1), and plays the same inside a parent composite.
use kabl_core::{CableId, CompositeId, ModuleId, PatchState, PortRef, Vec2};
use kabl_ui::{composites, PatchEditor};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[path = "../../../tools/examples/analog_voice_composite.rs"]
mod generator;

const SR: f32 = 48000.0;
/// An eighth at 125 BPM, the generator's phrase step.
const STEP: usize = 11520;

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
        .find(|e| e.definition.id == "kabl.analog-voice" && e.definition.version == 1)
        .expect("kabl.analog-voice v1 is in the bank");
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

/// The cable whose amount Env Amount is.
fn env_route(e: &PatchEditor, id: CompositeId) -> CableId {
    match port(e, id, "Env Amount") {
        PortRef::CableParam { cable, .. } => cable,
        other => panic!("Env Amount: {other:?}"),
    }
}

/// A `midi.in` into Pitch and Gate (every target of each), Analog Voice into the output.
fn voice(e: &mut PatchEditor) -> CompositeId {
    let midi = e.add_module("midi.in", Vec2 { x: 24.0, y: 20.0 });
    let out = e.add_module("out", Vec2 { x: 24.0, y: 420.0 });
    let id = composites::insert(e, &package()).unwrap();
    let open = BTreeSet::new();
    let (pitch, gate) = (port(e, id, "Pitch"), port(e, id, "Gate"));
    composites::connect(e, &open, jack(midi, "pitch"), pitch);
    composites::connect(e, &open, jack(midi, "gate"), gate);
    e.connect(port(e, id, "Out"), jack(out, "in"));
    id
}

/// Two seconds, interleaved stereo, with `notes` (semitones from C4) played one per eighth,
/// each held for three quarters of it, from the first frame.
fn play(p: &PatchState, notes: &[f32]) -> Vec<f32> {
    let _ftz = kabl_engine::ftz::FtzGuard::new();
    let mut engine = kabl_engine::compile::compile(p, SR, 8).unwrap();
    let mut audio = Vec::new();
    while audio.len() < 2 * 48000 * 2 {
        let frame = audio.len() / 2;
        if !notes.is_empty() {
            match frame % STEP {
                0 => engine.note_on(0, notes[frame / STEP % notes.len()], 0.8),
                f if f == STEP * 3 / 4 => engine.note_off(0),
                _ => {}
            }
        }
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

/// Two seconds of one note held throughout.
fn hold(p: &PatchState, note: f32) -> Vec<f32> {
    let _ftz = kabl_engine::ftz::FtzGuard::new();
    let mut engine = kabl_engine::compile::compile(p, SR, 8).unwrap();
    engine.note_on(0, note, 0.8);
    let mut audio = Vec::new();
    while audio.len() < 2 * 48000 {
        engine.process_block();
        audio.extend_from_slice(engine.left());
    }
    audio
}

fn rms(s: &[f32]) -> f32 {
    (s.iter().map(|v| v * v).sum::<f32>() / s.len().max(1) as f32).sqrt()
}

/// Brightness: the RMS of the first difference over the RMS, higher for more high-frequency
/// content.
fn brightness(s: &[f32]) -> f32 {
    let d: Vec<f32> = s.windows(2).map(|w| w[1] - w[0]).collect();
    rms(&d) / rms(s)
}

fn worst(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .fold(0.0f32, |m, (x, y)| m.max((x - y).abs()))
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

/// The same state with every composite dropped: the leaf graph alone.
fn flat(p: &PatchState) -> PatchState {
    let mut p = p.clone();
    p.composites.clear();
    p
}

/// A two-bar line, as the generator's phrase starts.
const LINE: [f32; 8] = [-12.0, -12.0, 0.0, -12.0, -9.0, -12.0, -5.0, -7.0];

/// The committed definition is what the generator writes, so the two cannot drift apart.
#[test]
fn the_committed_analog_voice_is_what_the_generator_writes() {
    let committed = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../composites/analog-voice/v1.json"),
    )
    .unwrap();
    assert!(
        generator::bytes() == committed,
        "composites/analog-voice/v1.json differs from `analog_voice_composite write composites`"
    );
}

/// Ports with every target, and every control's target with its range and stored default, as
/// ANALOG-VOICE.md lists them; the leaves are wired as the catalogue's core; Unison and Detune
/// are off the face and everything else is on it.
#[test]
fn analog_voice_has_the_catalogue_interface() {
    let pkg = package();
    let p = &pkg.patch;
    let c = &p.composites[&pkg.root];
    assert_eq!(c.name, "Analog Voice");
    kabl_engine::compile::validate_composites(p).unwrap();
    let end = |r: &PortRef| match r {
        PortRef::Module { id, port } => (p.modules[id].kind.clone(), port.clone()),
        PortRef::Param { id, param } => (p.modules[id].kind.clone(), param.clone()),
        PortRef::CableParam { cable, param } => ("cable".to_string(), format!("{cable}.{param}")),
    };
    let ports: Vec<_> = c
        .ports
        .iter()
        .map(|(&key, e)| {
            (
                key,
                e.label.clone(),
                e.targets().map(end).collect::<Vec<_>>(),
            )
        })
        .collect();
    let route = match &c.ports[&4].target {
        PortRef::CableParam { cable, .. } => *cable,
        other => panic!("Env Amount: {other:?}"),
    };
    let s = |k: &str, e: &str| (k.to_string(), e.to_string());
    let want = vec![
        (
            1,
            "Pitch".to_string(),
            vec![s("osc.va", "pitch"), s("osc.va", "pitch")],
        ),
        (
            2,
            "Gate".to_string(),
            vec![s("env.adsr", "gate"), s("env.adsr", "gate")],
        ),
        (3, "Out".to_string(), vec![s("gain", "out")]),
        (
            4,
            "Env Amount".to_string(),
            vec![s("cable", &format!("{route}.amount"))],
        ),
        (
            5,
            "Cutoff mod".to_string(),
            vec![s("filter.ladder", "cutoff_hz")],
        ),
        (6, "Level mod".to_string(), vec![s("gain", "gain_db")]),
        (7, "Drive mod".to_string(), vec![s("drive", "drive_db")]),
        (8, "Detune mod".to_string(), vec![s("osc.va", "detune")]),
    ];
    assert_eq!(ports, want);
    // (label, kind, param, min, max, default).
    let controls: Vec<_> = c
        .controls
        .values()
        .map(|e| {
            let PortRef::Param { id, param } = &e.target else {
                panic!("{} is not a parameter", e.label);
            };
            assert!(e.also.is_empty(), "{}: one target", e.label);
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
        ("Wave 1", "osc.va", "waveform", 0.0, 3.0, 2.0),
        ("Octave 1", "osc.va", "base_hz", 20.0, 20000.0, 261.63),
        ("Unison", "osc.va", "unison", 1.0, 16.0, 1.0),
        ("Detune", "osc.va", "detune", 0.0, 100.0, 15.0),
        ("Wave 2", "osc.va", "waveform", 0.0, 3.0, 2.0),
        ("Octave 2", "osc.va", "base_hz", 20.0, 20000.0, 261.63),
        ("Mix 2", "mixer", "level2", 0.0, 1.0, 0.3),
        ("Drive", "drive", "drive_db", 0.0, 36.0, 0.0),
        (
            "Cutoff",
            "filter.ladder",
            "cutoff_hz",
            20.0,
            20000.0,
            3500.0,
        ),
        ("Resonance", "filter.ladder", "resonance", 0.0, 1.0, 0.1),
        ("Filter Attack", "env.adsr", "attack_ms", 0.1, 10000.0, 2.0),
        ("Filter Decay", "env.adsr", "decay_ms", 0.1, 10000.0, 300.0),
        ("Filter Sustain", "env.adsr", "sustain", 0.0, 1.0, 0.3),
        (
            "Filter Release",
            "env.adsr",
            "release_ms",
            0.1,
            10000.0,
            100.0,
        ),
        ("Amp Attack", "env.adsr", "attack_ms", 0.1, 10000.0, 2.0),
        ("Amp Decay", "env.adsr", "decay_ms", 0.1, 10000.0, 300.0),
        ("Amp Sustain", "env.adsr", "sustain", 0.0, 1.0, 0.6),
        ("Amp Release", "env.adsr", "release_ms", 0.1, 10000.0, 90.0),
        ("Level", "gain", "gain_db", -60.0, 24.0, -6.0),
    ]
    .into_iter()
    .map(|(l, k, n, lo, hi, d)| (l.to_string(), k.to_string(), n.to_string(), lo, hi, d))
    .collect();
    assert_eq!(controls, expected);

    // Which leaf each control and port reaches: oscillator 1 is Pitch's first target, the
    // filter envelope Gate's first and the source of the Env Amount route, the amp envelope
    // Gate's second and the VCA's CV.
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
    let (pitch, gate) = (&c.ports[&1], &c.ports[&2]);
    let (osc1, osc2) = (pitch.target.module_id(), pitch.also[0].module_id());
    let (fenv, aenv) = (gate.target.module_id(), gate.also[0].module_id());
    for label in ["Wave 1", "Octave 1", "Unison", "Detune"] {
        assert_eq!(Some(leaf(label)), osc1, "{label}");
    }
    for label in ["Wave 2", "Octave 2"] {
        assert_eq!(Some(leaf(label)), osc2, "{label}");
    }
    assert_eq!(c.ports[&8].target.module_id(), osc1, "Detune mod");
    for label in ["Attack", "Decay", "Sustain", "Release"] {
        assert_eq!(
            Some(leaf(&format!("Filter {label}"))),
            fenv,
            "Filter {label}"
        );
        assert_eq!(Some(leaf(&format!("Amp {label}"))), aenv, "Amp {label}");
    }
    let x = &p.cables[&route];
    assert_eq!(
        x.from.module_id(),
        fenv,
        "the filter envelope drives the route"
    );
    assert_eq!(
        x.to,
        PortRef::Param {
            id: leaf("Cutoff"),
            param: "cutoff_hz".into()
        }
    );
    assert_eq!(x.params["amount"], generator::ENV_AMOUNT);
    let wired = |from: ModuleId, fp: &str, to: ModuleId, tp: &str| {
        p.cables
            .values()
            .any(|x| x.from == jack(from, fp) && x.to == jack(to, tp))
    };
    let mix = leaf("Mix 2");
    let vca = *c
        .members
        .iter()
        .find(|m| p.modules[m].kind == "vca")
        .unwrap();
    assert!(wired(osc1.unwrap(), "out", mix, "in1"));
    assert!(wired(osc2.unwrap(), "out", mix, "in2"));
    assert!(wired(mix, "out", leaf("Drive"), "in"));
    assert!(wired(leaf("Drive"), "out", leaf("Cutoff"), "in"));
    assert!(wired(leaf("Cutoff"), "out", vca, "in"));
    assert!(wired(aenv.unwrap(), "out", vca, "cv"));
    assert!(wired(vca, "out", leaf("Level"), "in"));
    assert_eq!(
        p.modules[&vca].params["gain"], 0.0,
        "the envelope alone opens the VCA"
    );
    assert_eq!(p.modules[&mix].params["level1"], 1.0);

    let face = c.panel.as_ref().expect("a face");
    for (&key, e) in &c.controls {
        assert_eq!(
            face.placements.contains_key(&key),
            !["Unison", "Detune"].contains(&e.label.as_str()),
            "{} on the face",
            e.label
        );
    }
    for &key in c.ports.keys() {
        assert!(face.placements.contains_key(&key), "port {key} on the face");
    }
}

/// Notes on Pitch and Gate sound; with no note the output is silent; A4 (MIDI note 69, nine
/// semitones over C4) on a sine with the filter open is 440 Hz within 1 %.
#[test]
fn analog_voice_plays_notes_in_tune_and_is_silent_ungated() {
    let mut e = PatchEditor::seed_from(&PatchState::default());
    let id = voice(&mut e);
    let gated = play(e.state(), &LINE);
    assert!(gated.iter().all(|s| s.is_finite()));
    assert!(rms(&gated) > 0.01, "notes sound: RMS {}", rms(&gated));
    let silent = play(e.state(), &[]);
    assert!(silent.iter().all(|s| s.is_finite()));
    assert!(rms(&silent) < 1e-6, "no gate: RMS {}", rms(&silent));

    e.set_param(control(&e, id, "Wave 1"), "waveform", 0.0);
    e.set_param(control(&e, id, "Mix 2"), "level2", 0.0);
    let lad = control(&e, id, "Cutoff");
    e.set_param(lad, "cutoff_hz", 20000.0);
    e.set_param(lad, "resonance", 0.0);
    let route = env_route(&e, id);
    e.set_route_amount(route, 0.0, true);
    let a4 = hold(e.state(), 69.0 - 60.0);
    // Rising zero crossings over the second second, after the attack.
    let tail = &a4[48000..];
    let rising: Vec<usize> = tail
        .windows(2)
        .enumerate()
        .filter(|(_, w)| w[0] <= 0.0 && w[1] > 0.0)
        .map(|(i, _)| i)
        .collect();
    assert!(rising.len() > 100, "a tone: {} crossings", rising.len());
    let periods = (rising.len() - 1) as f32;
    let hz = periods * SR / (rising[rising.len() - 1] - rising[0]) as f32;
    println!("note 69: {hz:.2} Hz");
    assert!((hz / 440.0 - 1.0).abs() < 0.01, "note 69 at {hz} Hz");
}

/// Cinder Block's voice, flat from the preset, against the card set to its values with the
/// same midi.in, macros and routes on its ports: equal to float rounding. The cutoff sums
/// three routes (Wheel, Brightness, the envelope's), and the engine adds them in cable order;
/// in the card the envelope's route is the card's own cable, made before the outside ones, so
/// it is added first, and float addition does not associate. Re-made in the card's order, the
/// flat voice renders sample-identical.
#[test]
fn with_cinder_blocks_values_it_renders_cinder_blocks_voice() {
    let patches = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches");
    let generator::CinderBlock { mut flat, card } = generator::cinder_block(&package(), &patches);
    let ours = play(card.state(), &LINE);
    let theirs = play(flat.state(), &LINE);
    assert!(
        rms(&theirs) > 0.01,
        "Cinder Block sounds: RMS {}",
        rms(&theirs)
    );
    let off = worst(&ours, &theirs);
    println!("off by {off:e} at most");
    assert!(off < 1e-6, "card against preset: off by {off}");

    // The preset's Wheel and Brightness routes into the cutoff made again, after its envelope
    // route: the card's order.
    let lad = *flat
        .state()
        .modules
        .iter()
        .find(|(_, m)| m.kind == "filter.ladder")
        .unwrap()
        .0;
    let cutoff = PortRef::Param {
        id: lad,
        param: "cutoff_hz".into(),
    };
    let outside: BTreeMap<CableId, (PortRef, f32)> = flat
        .state()
        .cables
        .iter()
        .filter(|(_, x)| {
            x.to == cutoff && flat.state().modules[&x.from.module_id().unwrap()].kind != "env.adsr"
        })
        .map(|(&id, x)| (id, (x.from.clone(), x.params["amount"])))
        .collect();
    assert_eq!(outside.len(), 2, "Wheel and Brightness");
    for (id, (from, amount)) in outside {
        flat.disconnect(id);
        let again = flat.connect(from, cutoff.clone());
        flat.set_route_amount(again, amount, true);
    }
    let reordered = play(flat.state(), &LINE);
    assert!(ours == reordered, "in the same route order: identical");
}

/// Env Amount is the envelope's route depth: setting it changes the cable's `amount`, more
/// opens the filter wider on each note (brighter) and less closes it (darker); −1 renders
/// finite and audible. Patched from outside, a macro on the port deepens the route too.
#[test]
fn env_amount_moves_the_filter_envelope_route() {
    let mut e = PatchEditor::seed_from(&PatchState::default());
    let id = voice(&mut e);
    let route = env_route(&e, id);
    let mut bright = Vec::new();
    for amount in [-0.5, 0.0, 0.5] {
        e.set_route_amount(route, amount, true);
        assert_eq!(e.state().cables[&route].params["amount"], amount);
        let audio = play(e.state(), &LINE);
        assert!(
            rms(&audio) > 0.001,
            "Env Amount {amount}: RMS {}",
            rms(&audio)
        );
        bright.push(brightness(&audio));
    }
    println!("brightness at -0.5, 0, 0.5: {bright:?}");
    assert!(
        bright[0] < bright[1] && bright[1] < bright[2],
        "Env Amount brightens: {bright:?}"
    );
    e.set_route_amount(route, -1.0, true);
    let lowest = play(e.state(), &LINE);
    assert!(
        lowest.iter().all(|s| s.is_finite()),
        "Env Amount -1 is finite"
    );
    assert!(
        rms(&lowest) > 1e-4,
        "Env Amount -1 sounds: RMS {}",
        rms(&lowest)
    );

    e.set_route_amount(route, 0.0, true);
    let none = brightness(&play(e.state(), &LINE));
    let m = e.add_module("macro", Vec2 { x: 24.0, y: 820.0 });
    e.set_param(m, "m1", 1.0);
    let to = port(&e, id, "Env Amount");
    let cable = composites::connect(&mut e, &BTreeSet::new(), jack(m, "m1"), to);
    e.set_route_amount(cable, 0.5, true);
    let driven = brightness(&play(e.state(), &LINE));
    assert!(
        driven > none,
        "a macro on Env Amount brightens: {driven} over {none}"
    );
}

/// The card with its midi.in and output outside it, all in a parent composite, renders the
/// same samples as before grouping and as the bare leaf graph.
#[test]
fn analog_voice_plays_inside_a_parent_composite() {
    let mut e = PatchEditor::seed_from(&PatchState::default());
    let card = voice(&mut e);
    let before = play(e.state(), &LINE);
    let leaves = top_level(&e);
    assert_eq!(leaves.len(), 2, "the midi.in and the output");
    let parent = composites::encapsulate(&mut e, leaves, BTreeSet::from([card]), "Song").unwrap();
    assert_eq!(e.state().composites[&card].parent, Some(parent));
    kabl_engine::compile::validate_composites(e.state()).unwrap();
    let nested = play(e.state(), &LINE);
    assert!(rms(&nested) > 0.01, "plays: RMS {}", rms(&nested));
    assert!(before == nested, "grouping changes nothing in the render");
    assert!(
        play(&flat(e.state()), &LINE) == nested,
        "the same as the leaf graph alone"
    );
}
