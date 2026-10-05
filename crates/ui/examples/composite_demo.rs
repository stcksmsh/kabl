//! Reproducible D09 example documents/packages and scripted engine audio (not hardware input).
use kabl_core::{PortRef, Vec2};
use kabl_ui::{composites, PatchEditor};
use std::{collections::BTreeSet, path::Path};
fn port(id: u64, name: &str) -> PortRef {
    PortRef::Module {
        id,
        port: name.into(),
    }
}
fn render(p: &kabl_core::PatchState, path: &Path) -> Vec<f32> {
    let mut engine = kabl_engine::compile::compile(p, 48000.0, 8).unwrap();
    let mut pcm = Vec::new();
    for block in 0..9000 {
        if block % 1500 == 0 {
            engine.note_on(0, [0., 4., 7., 12., 7., 4.][block / 1500], 0.8);
        }
        if block % 1500 == 900 {
            engine.note_off(0);
        }
        engine.process_block();
        for (&l, &r) in engine.left().iter().zip(engine.right()) {
            pcm.extend([l, r]);
        }
    }
    let mut writer = hound::WavWriter::create(
        path,
        hound::WavSpec {
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .unwrap();
    for &v in &pcm {
        writer.write_sample(v).unwrap();
    }
    writer.finalize().unwrap();
    pcm
}
fn main() {
    let root = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "docs/composites/examples".into());
    let root = Path::new(&root);
    std::fs::create_dir_all(root).unwrap();
    let mut e = PatchEditor::seed_from(&kabl_standalone::default_patch());
    for (kind, param, name) in [
        ("filter.svf", "cutoff_hz", "Brightness"),
        ("filter.svf", "resonance", "Resonance"),
        ("env.adsr", "attack_ms", "Attack"),
        ("env.adsr", "release_ms", "Release"),
    ] {
        let id = *e
            .state()
            .modules
            .iter()
            .find(|(_, m)| m.kind == kind)
            .unwrap()
            .0;
        e.set_param(id, &format!("pin.{param}"), e.state().modules.len() as f32);
        e.set_label(id, &format!("pin.{param}"), Some(name.into()));
    }
    kabl_core::save(&root.join("flat-voice"), e.log()).unwrap();
    let flat = e.state().clone();
    let members = flat
        .modules
        .iter()
        .filter(|(_, m)| m.kind != "out" && m.kind != "midi.in")
        .map(|(&id, _)| id)
        .collect();
    let voice =
        composites::encapsulate(&mut e, members, BTreeSet::new(), "Subtractive voice").unwrap();
    let a = render(&flat, &root.join("voice-flat.wav"));
    let b = render(e.state(), &root.join("voice-composite.wav"));
    assert_eq!(a, b);
    kabl_core::save(&root.join("voice"), e.log()).unwrap();
    let pkg = composites::package(e.state(), voice).unwrap();
    std::fs::write(
        root.join("voice.json"),
        serde_json::to_vec_pretty(&pkg).unwrap(),
    )
    .unwrap();
    let chorus = e.add_module("chorus", Vec2 { x: 240., y: 780. });
    let reverb = e.add_module("reverb", Vec2 { x: 480., y: 780. });
    let out = *e
        .state()
        .modules
        .iter()
        .find(|(_, m)| m.kind == "out")
        .unwrap()
        .0;
    let vca = *e
        .state()
        .modules
        .iter()
        .find(|(_, m)| m.kind == "vca")
        .unwrap()
        .0;
    e.connect(port(vca, "out"), port(chorus, "in_l"));
    e.connect(port(vca, "out"), port(chorus, "in_r"));
    for (src, dst) in [("left", "in_l"), ("right", "in_r")] {
        e.connect(port(chorus, src), port(reverb, dst));
    }
    for side in ["left", "right"] {
        e.connect(port(reverb, side), port(out, side));
    }
    let flat = e.state().clone();
    let fx = composites::encapsulate(
        &mut e,
        BTreeSet::from([chorus, reverb]),
        BTreeSet::new(),
        "Stereo chorus room",
    )
    .unwrap();
    for (id, param, label) in [
        (chorus, "depth", "Motion"),
        (chorus, "mix", "Chorus mix"),
        (reverb, "decay_s", "Room time"),
        (reverb, "mix", "Room mix"),
    ] {
        composites::add_exposure(
            &mut e,
            fx,
            PortRef::Param {
                id,
                param: param.into(),
            },
            true,
        )
        .unwrap();
        let mut c = e.state().composites[&fx].clone();
        c.controls
            .values_mut()
            .find(|v| matches!(&v.target,PortRef::Param{param:p,..} if p==param))
            .unwrap()
            .label = label.into();
        composites::set(&mut e, fx, c).unwrap();
    }
    let a = render(&flat, &root.join("stereo-flat.wav"));
    let b = render(e.state(), &root.join("stereo-composite.wav"));
    assert_eq!(a, b);
    std::fs::write(
        root.join("stereo-effect.json"),
        serde_json::to_vec_pretty(&composites::package(e.state(), fx).unwrap()).unwrap(),
    )
    .unwrap();
    let copy = composites::duplicate(&mut e, voice).unwrap();
    let leaf = *kabl_core::composite::leaves(e.state(), copy)
        .iter()
        .find(|&&id| e.state().modules[&id].kind == "filter.svf")
        .unwrap();
    e.set_param(leaf, "cutoff_hz", 900.0);
    kabl_core::save(&root.join("two-instances"), e.log()).unwrap();
    std::fs::write(
        root.join("state.json"),
        serde_json::to_vec_pretty(e.state()).unwrap(),
    )
    .unwrap();
    println!("Voice and stereo flat/composite PCM exact; two independent instances saved. Audio is scripted engine rendering.");
}
