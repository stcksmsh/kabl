//! The factory composite `kabl.space` v1 (docs/composites/SPACE.md): it loads from the bank,
//! renders a wet tail on its own, answers its controls inside a voice, and reproduces the
//! Space tail of Soft Sine, its source patch, sample for sample.
use kabl_core::{CompositeId, ModuleId, PatchState, PortRef};
use kabl_ui::{composites, PatchEditor};
use std::path::Path;

const BLOCK: usize = 64;

fn jack(id: ModuleId, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

fn find(p: &PatchState, kind: &str) -> ModuleId {
    *p.modules
        .iter()
        .find(|(_, m)| m.kind == kind)
        .unwrap_or_else(|| panic!("no {kind}"))
        .0
}

fn space_package() -> composites::Package {
    let bank = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../composites");
    let entry = composites::factory_entries(&bank)
        .into_iter()
        .find(|e| e.definition.id == "kabl.space" && e.definition.version == 1)
        .expect("kabl.space v1 is in the bank");
    composites::read_factory(&entry).unwrap()
}

/// Inserts Space and returns it with the leaf targets of its `In` and `Out` ports.
fn insert_space(e: &mut PatchEditor) -> (CompositeId, PortRef, PortRef) {
    let id = composites::insert(e, &space_package()).unwrap();
    let port = |label: &str| {
        e.state().composites[&id]
            .ports
            .values()
            .find(|x| x.label == label)
            .unwrap()
            .target
            .clone()
    };
    let (input, output) = (port("In"), port("Out"));
    (id, input, output)
}

fn control(e: &PatchEditor, id: CompositeId, label: &str) -> PortRef {
    e.state().composites[&id]
        .controls
        .values()
        .find(|x| x.label == label)
        .unwrap()
        .target
        .clone()
}

fn rms(s: &[f32]) -> f32 {
    (s.iter().map(|v| v * v).sum::<f32>() / s.len().max(1) as f32).sqrt()
}

/// `blocks` blocks of interleaved stereo; a note on `voice` 0 for the first `held` blocks.
fn render_notes(p: &PatchState, held: usize, blocks: usize) -> Vec<f32> {
    let mut engine = kabl_engine::compile::compile(p, 48000.0, 8).unwrap();
    engine.note_on(0, 0.0, 0.8);
    let mut audio = Vec::new();
    for b in 0..blocks {
        if b == held {
            engine.note_off(0);
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

#[test]
fn space_loads_from_the_bank_with_its_interface() {
    let pkg = space_package();
    let c = &pkg.patch.composites[&pkg.root];
    assert_eq!(c.name, "Space");
    let labels = |m: &std::collections::BTreeMap<u64, kabl_core::Exposure>| {
        m.values().map(|e| e.label.clone()).collect::<Vec<_>>()
    };
    assert_eq!(
        labels(&c.ports),
        [
            "In",
            "Clock",
            "Out",
            "Echo mod",
            "Room mod",
            "Decay mod",
            "Feedback mod"
        ]
    );
    assert_eq!(
        labels(&c.controls),
        [
            "Echo", "Time", "Sync", "Feedback", "Tone", "Room", "Decay", "Damp", "Width",
            "Predelay"
        ]
    );
    let kinds: Vec<_> = pkg
        .patch
        .modules
        .values()
        .map(|m| m.kind.as_str())
        .collect();
    assert_eq!(kinds, ["split", "delay", "reverb"]);
}

/// `audio.in → Space → out`: a 50 ms tone, then silence. Nothing dry is left after the tone,
/// so what sounds afterwards is the echo and the room.
#[test]
fn space_alone_renders_a_wet_tail() {
    let mut e = PatchEditor::seed_from(&PatchState::default());
    let src = e.add_module("audio.in", kabl_core::Vec2 { x: 24.0, y: 20.0 });
    let out = e.add_module("out", kabl_core::Vec2 { x: 600.0, y: 20.0 });
    let (_, input, output) = insert_space(&mut e);
    e.connect(jack(src, "out"), input);
    e.connect(output, jack(out, "in"));
    let mut engine = kabl_engine::compile::compile(e.state(), 48000.0, 1).unwrap();
    let tone_blocks = 2400 / BLOCK;
    let mut during = Vec::new();
    let mut after = Vec::new();
    for b in 0..(48000 / BLOCK) {
        let mut l = [0.0f32; BLOCK];
        if b < tone_blocks {
            for (i, s) in l.iter_mut().enumerate() {
                let t = (b * BLOCK + i) as f32 / 48000.0;
                *s = 0.5 * (std::f32::consts::TAU * 440.0 * t).sin();
            }
        }
        engine.feed_audio(&l, &l);
        engine.process_block();
        let dst = if b < tone_blocks {
            &mut during
        } else {
            &mut after
        };
        dst.extend(engine.left().iter().chain(engine.right()));
    }
    assert!(rms(&during) > 0.01, "the dry tone passes: {}", rms(&during));
    // The echo repeats at 310 ms; the room rings past it.
    assert!(rms(&after) > 0.001, "a wet tail follows: {}", rms(&after));
}

/// The default voice into Space into the output: turning Room up changes the sound.
#[test]
fn a_space_control_changes_the_render_inside_a_voice() {
    let mut e = PatchEditor::seed_from(&kabl_standalone::default_patch());
    let (vca, out) = (find(e.state(), "vca"), find(e.state(), "out"));
    let to_out = *e
        .state()
        .cables
        .iter()
        .find(|(_, c)| c.to.module_id() == Some(out))
        .unwrap()
        .0;
    e.disconnect(to_out);
    let (id, input, output) = insert_space(&mut e);
    e.connect(jack(vca, "out"), input);
    e.connect(output, jack(out, "in"));
    let before = render_notes(e.state(), 100, 600);
    assert!(rms(&before) > 0.001);
    let PortRef::Param { id: verb, param } = control(&e, id, "Room") else {
        panic!("Room is a parameter");
    };
    assert_eq!(e.state().modules[&verb].params[&param], 10.0);
    e.set_param(verb, &param, 60.0);
    let after = render_notes(e.state(), 100, 600);
    assert_eq!(before.len(), after.len());
    let diff: Vec<f32> = before.iter().zip(&after).map(|(a, b)| a - b).collect();
    assert!(rms(&diff) > 0.001, "Room 60 % differs: {}", rms(&diff));
}

/// Soft Sine flat, then the same patch with its split, delay and reverb replaced by an
/// inserted `kabl.space` v1 at its defaults, wired from the compressor to the output.
#[test]
fn space_reproduces_soft_sine_bit_for_bit() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/presets/soft-sine");
    let mut e = PatchEditor::from_log(kabl_core::load(&dir).unwrap());
    let flat = render_notes(e.state(), 300, 1500);
    assert!(rms(&flat) > 0.001);
    let (comp, out) = (find(e.state(), "compressor"), find(e.state(), "out"));
    for kind in ["split", "delay", "reverb"] {
        let m = find(e.state(), kind);
        e.remove_module(m);
    }
    let (_, input, output) = insert_space(&mut e);
    e.connect(jack(comp, "out"), input);
    e.connect(output, jack(out, "in"));
    let through = render_notes(e.state(), 300, 1500);
    assert_eq!(flat.len(), through.len());
    let first = flat
        .iter()
        .zip(&through)
        .position(|(a, b)| a.to_bits() != b.to_bits());
    assert_eq!(first, None, "first differing sample");
}
