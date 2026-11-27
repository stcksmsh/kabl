//! What the factory composite generators (`drum_kit_composite`, `sequenced_part_composite`)
//! share: exposure helpers, and the clocked standalone patch they render to a WAV. Included
//! with `#[path]`, so the tests that include a generator compile only the helpers.
use kabl_core::{Exposure, ModuleId, PortRef};

pub(crate) fn jack(id: ModuleId, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

pub(crate) fn param(id: ModuleId, name: &str) -> PortRef {
    PortRef::Param {
        id,
        param: name.into(),
    }
}

pub(crate) fn expose(label: &str, target: PortRef) -> Exposure {
    Exposure {
        label: label.into(),
        target,
        also: Vec::new(),
    }
}

/// A clock at 120 BPM into `kabl.<name>` v1 from the bank, into the output.
#[cfg(not(test))]
fn standalone(name: &str) -> kabl_ui::PatchEditor {
    use kabl_core::Vec2;
    use kabl_ui::{composites, PatchEditor};
    let mut e = PatchEditor::new();
    let clock = e.add_module("clock", Vec2 { x: 24.0, y: 20.0 });
    e.set_param(clock, "bpm", 120.0);
    let out = e.add_module("out", Vec2 { x: 1100.0, y: 20.0 });
    let bank = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../composites");
    let def = format!("kabl.{name}");
    let entry = composites::factory_entries(&bank)
        .into_iter()
        .find(|f| f.definition.id == def && f.definition.version == 1)
        .unwrap_or_else(|| panic!("{def} v1 is not in the bank; run its generator's `write`"));
    let pkg = composites::read_factory(&entry).expect("read the definition");
    let id = composites::insert(&mut e, &pkg).expect("insert the composite");
    let port = |e: &PatchEditor, label: &str| {
        e.state().composites[&id]
            .ports
            .values()
            .find(|x| x.label == label)
            .unwrap_or_else(|| panic!("{def} has no port {label}"))
            .target
            .clone()
    };
    let (c, r, o) = (port(&e, "Clock"), port(&e, "Reset"), port(&e, "Out"));
    e.connect(jack(clock, "gate"), c);
    e.connect(jack(clock, "reset"), r);
    e.connect(o, jack(out, "in"));
    e
}

/// Eight seconds of `standalone(name)`, rendered offline to `DIR/<name>.wav`; prints peak and
/// RMS.
#[cfg(not(test))]
pub(crate) fn wav(dir: &std::path::Path, name: &str) {
    const SR: usize = 48000;
    std::fs::create_dir_all(dir).expect("out folder");
    let mut engine =
        kabl_engine::compile::compile(standalone(name).state(), SR as f32, 8).expect("compile");
    let mut audio = Vec::with_capacity(8 * SR * 2);
    while audio.len() < 8 * SR * 2 {
        engine.process_block();
        for (l, r) in engine.left().iter().zip(engine.right()) {
            audio.push(*l);
            audio.push(*r);
        }
    }
    let path = dir.join(format!("{name}.wav"));
    let mut w = hound::WavWriter::create(
        &path,
        hound::WavSpec {
            channels: 2,
            sample_rate: SR as u32,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .expect("wav");
    for s in &audio {
        w.write_sample(*s).expect("sample");
    }
    w.finalize().expect("finalize");
    let peak = audio.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let rms = (audio.iter().map(|s| s * s).sum::<f32>() / audio.len() as f32).sqrt();
    println!(
        "{}: peak {:.1} dBFS, RMS {:.1} dBFS",
        path.display(),
        20.0 * peak.log10(),
        20.0 * rms.log10()
    );
}
