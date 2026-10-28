//! The eight-macro demo piece: a drifting pad with eight named macros. The seventh, Motion,
//! does not move a knob: it is routed into the amount of a slow LFO's route into the filter
//! cutoff, so it fades a gentle breath in from nothing to full. `render` plays the saved patch
//! through the real engine, moving Motion at 4 s and 8 s, to a WAV.
//!
//! ```sh
//! cargo run -p kabl-ui --example macros_demo -- write patches/macros-eight
//! cargo run --release -p kabl-ui --example macros_demo -- render patches/macros-eight macros.wav
//! ```

use std::path::Path;

use basedrop::Collector;
use kabl_core::{ModuleId, PortRef, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::PatchEngine;
use kabl_engine::runtime::RuntimeTarget;
use kabl_ui::perform::{CC_PREFIX, PIN_PREFIX, TRANSPORT};
use kabl_ui::PatchEditor;

const SR: f32 = 48000.0;
const SECONDS: usize = 12;
/// Index of Motion among the macro's outputs.
const MOTION: u16 = 6;

fn jack(id: ModuleId, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

fn at(x: f32, y: f32) -> Vec2 {
    Vec2 { x, y }
}

/// (name, knob start value). Order is m1..m8.
const MACROS: [(&str, f32); 8] = [
    ("Cutoff", 0.5),
    ("Reso", 0.3),
    ("Detune", 0.4),
    ("Fifth", 0.5),
    ("Delay", 0.1),
    ("Space", 0.3),
    ("Motion", 0.0),
    ("Level", 0.5),
];

/// How far the full-Motion wobble moves the cutoff, in knob travel either way.
const WOBBLE: f32 = 0.07;

fn build() -> (PatchEditor, ModuleId) {
    let mut e = PatchEditor::new();
    let clock = e.add_module("clock", at(20.0, 20.0));
    let root = e.add_module("osc.va", at(20.0, 300.0));
    let fifth = e.add_module("osc.va", at(20.0, 560.0));
    let mixer = e.add_module("mixer", at(240.0, 300.0));
    let ladder = e.add_module("filter.ladder", at(460.0, 300.0));
    let lfo = e.add_module("lfo", at(460.0, 560.0));
    let delay = e.add_module("delay", at(680.0, 300.0));
    let verb = e.add_module("reverb", at(900.0, 300.0));
    let out = e.add_module("out", at(1120.0, 300.0));
    let knob = e.add_module("macro", at(460.0, 20.0));

    e.set_param(clock, "bpm", 100.0);
    for (osc, hz) in [(root, 110.0), (fifth, 164.81)] {
        e.set_param(osc, "waveform", 2.0);
        e.set_param(osc, "base_hz", hz);
        e.set_param(osc, "unison", 4.0);
        e.set_param(osc, "detune", 6.0);
    }
    e.set_param(mixer, "level1", 0.07);
    e.set_param(mixer, "level2", 0.0);
    e.set_param(ladder, "cutoff_hz", 700.0);
    e.set_param(ladder, "resonance", 0.2);
    // Slow and gentle: one slow breath every five seconds.
    e.set_param(lfo, "rate_hz", 0.2);
    e.set_param(delay, "sync", 3.0);
    e.set_param(delay, "feedback", 40.0);
    e.set_param(delay, "mix", 0.0);
    e.set_param(verb, "decay_s", 5.0);
    e.set_param(verb, "mix", 0.0);

    e.connect(jack(root, "out"), jack(mixer, "in1"));
    e.connect(jack(fifth, "out"), jack(mixer, "in2"));
    e.connect(jack(mixer, "out"), jack(ladder, "in"));
    e.connect(jack(ladder, "out"), jack(delay, "in"));
    e.connect(jack(clock, "gate"), jack(delay, "clock"));
    e.connect(jack(delay, "left"), jack(verb, "in_l"));
    e.connect(jack(delay, "right"), jack(verb, "in_r"));
    e.connect(jack(verb, "left"), jack(out, "left"));
    e.connect(jack(verb, "right"), jack(out, "right"));

    for (dest, param, name, amount) in [
        (ladder, "cutoff_hz", "m1", 0.5),
        (ladder, "resonance", "m2", 0.4),
        (root, "detune", "m3", 0.6),
        (fifth, "detune", "m3", 0.6),
        (mixer, "level2", "m4", 0.4),
        (delay, "mix", "m5", 0.4),
        (verb, "mix", "m6", 0.5),
        (mixer, "level1", "m8", 0.3),
    ] {
        let r = e.connect_route(jack(knob, name), dest, param);
        e.set_route_amount(r, amount, true);
    }

    // The breath: an LFO on the cutoff at amount 0, and Motion into that route's amount.
    let breath = e.connect_route(jack(lfo, "out"), ladder, "cutoff_hz");
    e.set_route_amount(breath, 0.0, true);
    let depth = e
        .connect_cable_route(jack(knob, "m7"), breath, "amount")
        .expect("a route into a knob has a depth");
    e.set_route_amount(depth, WOBBLE, true);

    for (i, (name, v)) in MACROS.iter().enumerate() {
        let key = format!("m{}", i + 1);
        e.set_label(knob, &format!("name.{key}"), Some((*name).into()));
        e.set_param(knob, &key, *v);
    }

    let mut pins = vec![(clock, TRANSPORT.to_string())];
    pins.extend((1..=8).map(|k| (knob, format!("m{k}"))));
    let mut changes = Vec::new();
    for (i, (id, key)) in pins.iter().enumerate() {
        changes.push((*id, format!("{PIN_PREFIX}{key}"), Some(i as f32)));
        if let Some(k) = key.strip_prefix('m') {
            let cc: f32 = 19.0 + k.parse::<f32>().unwrap();
            changes.push((*id, format!("{CC_PREFIX}{key}"), Some(cc)));
        }
    }
    e.set_presentation(&changes);
    (e, knob)
}

const SOUND_TOML: &str = r#"name = "Eight Macros"
category = "Piece"
tags = ["pad", "macros", "performance", "drone"]
description = "A drifting two-voice pad with eight named macros on the Perform page. Motion does not move a knob: it fades in a slow, gentle breath on the filter cutoff, from still to full. Run, then turn Motion."
keys = false
sequence = false
"#;

fn write_wav(path: &Path, left: &[f32], right: &[f32]) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for (l, r) in left.iter().zip(right) {
        for s in [l, r] {
            w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16)
                .unwrap();
        }
    }
    w.finalize().unwrap();
}

fn render(state: &kabl_core::PatchState, knob: ModuleId) -> (Vec<f32>, Vec<f32>) {
    let collector = Collector::new();
    let mut engine = PatchEngine::new(&collector.handle(), state, SR, 4).expect("compiles");
    let blocks = SECONDS * SR as usize / BLOCK;
    let (mut left, mut right) = (Vec::new(), Vec::new());
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    for b in 0..blocks {
        let t = (b * BLOCK) as f32 / SR;
        if b == 0 || b == (4.0 * SR) as usize / BLOCK || b == (8.0 * SR) as usize / BLOCK {
            let v = if t < 4.0 {
                0.0
            } else if t < 8.0 {
                0.5
            } else {
                1.0
            };
            engine.active_mut().set_runtime(
                RuntimeTarget::Param {
                    id: knob,
                    kind: "macro",
                    index: MOTION,
                },
                v,
                true,
            );
        }
        engine.process_block(&mut l, &mut r);
        left.extend_from_slice(&l);
        right.extend_from_slice(&r);
    }
    (left, right)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (e, knob) = build();
    let e = e.migrated();
    match args.get(1).map(String::as_str) {
        Some("write") => {
            let dir = Path::new(&args[2]);
            kabl_core::save(dir, e.log()).expect("save");
            std::fs::write(dir.join("sound.toml"), SOUND_TOML).expect("sound.toml");
            println!("wrote {} (macro module {knob})", dir.display());
        }
        Some("render") => {
            let log = kabl_core::load(Path::new(&args[2])).expect("load patch");
            let (l, r) = render(log.state(), knob);
            write_wav(Path::new(&args[3]), &l, &r);
            // Motion shows as the cutoff breathing: brightness (rms of the first difference over
            // rms) per 250 ms, lowest and highest over 3 s of each 4 s window.
            for w in 0..3 {
                let s = &l[w * 4 * SR as usize + if w == 0 { SR as usize } else { 0 }..]
                    [..3 * SR as usize];
                let rms = |v: &mut dyn Iterator<Item = f32>| {
                    let (n, t) = v.fold((0.0f32, 0.0f32), |(n, t), x| (n + 1.0, t + x * x));
                    (t / n).sqrt()
                };
                let bright: Vec<f32> = s
                    .chunks(SR as usize / 4)
                    .map(|c| {
                        rms(&mut c.windows(2).map(|p| p[1] - p[0])) / rms(&mut c.iter().copied())
                    })
                    .collect();
                let (lo, hi) = bright
                    .iter()
                    .fold((f32::MAX, 0.0f32), |(a, b), &x| (a.min(x), b.max(x)));
                println!("window {w}: brightness per 250 ms {lo:.3} .. {hi:.3}");
            }
        }
        _ => eprintln!("usage: macros_demo write <patch dir> | render <patch dir> <wav>"),
    }
}
