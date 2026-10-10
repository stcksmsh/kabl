//! Macro Morph: one macro moves the morph of three pattern cables at different depths. Each
//! voice's gate runs through a cable with a pattern A and a pattern B; macro `m1` has a route
//! into every cable's morph (`PortRef::CableParam`): +100 %, +50 % and -100 % of the morph's
//! travel, so one knob thickens two voices and thins the third.
//!
//! ```sh
//! cargo run --release -p kabl-ui --example cable_routes_demo -- write patches/functional-cables/macro-morph
//! cargo run --release -p kabl-ui --example cable_routes_demo -- render out.wav   # asserts determinism
//! ```

use std::path::Path;

use kabl_core::{CableId, ModuleId, ParamTarget, PortRef, Vec2};
use kabl_engine::compile::compile;
use kabl_engine::graph::BLOCK;
use kabl_engine::runtime::RuntimeTarget;
use kabl_ui::PatchEditor;

const SR: f32 = 48000.0;
const BPM: f32 = 120.0;
const BAR_S: f32 = 16.0 * 60.0 / (BPM * 4.0);
const BARS: f32 = 20.0;

/// The macro knob over the piece, 0..1: still, a rise over eight bars, a hold, a fall.
fn knob_at(bar: f32) -> f32 {
    match bar {
        b if b < 2.0 => 0.0,
        b if b < 10.0 => (b - 2.0) / 8.0,
        b if b < 14.0 => 1.0,
        b if b < 18.0 => 1.0 - (b - 14.0) / 4.0,
        _ => 0.0,
    }
}

fn jack(id: ModuleId, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

fn at(x: f32, y: f32) -> Vec2 {
    Vec2 { x, y }
}

fn cable(e: &mut PatchEditor, id: CableId, params: &[(String, f32)]) {
    for (name, v) in params {
        e.set_param_gesture(
            ParamTarget::Cable {
                id,
                param: name.clone(),
            },
            *v,
            true,
        );
    }
}

/// Pattern A and B levels (a step per entry), the chance of listed steps (1-based), base morph.
fn pattern(a: &[f32], b: &[f32], chance_a: &[(usize, f32)], morph: f32) -> Vec<(String, f32)> {
    let mut p = vec![
        ("length".into(), a.len() as f32),
        ("b.length".into(), b.len() as f32),
        ("morph".into(), morph),
    ];
    for (k, v) in a.iter().enumerate() {
        p.push((format!("s{}", k + 1), *v));
    }
    for (k, v) in b.iter().enumerate() {
        p.push((format!("b.s{}", k + 1), *v));
    }
    for (k, v) in chance_a {
        p.push((format!("r{k}"), *v));
    }
    p
}

/// The patch: (editor, macro module id, the three route cable ids).
fn build() -> (PatchEditor, ModuleId, [CableId; 3]) {
    let mut e = PatchEditor::new();
    let clock = e.add_module("clock", at(30.0, 10.0));
    let knob = e.add_module("macro", at(230.0, 10.0));
    let mix = e.add_module("mixer", at(630.0, 380.0));
    let out = e.add_module("out", at(830.0, 380.0));
    e.set_param(clock, "bpm", BPM);
    e.set_param(mix, "level1", 0.45);
    e.set_param(mix, "level2", 0.35);
    e.set_param(mix, "level3", 0.25);

    // (name, wave, base_hz, decay ms, pattern A, pattern B, chances in A, base morph, depth)
    type Voice = (&'static str, f32, f32, f32, Vec<f32>, Vec<f32>, Vec<(usize, f32)>, f32, f32);
    let voices: [Voice; 3] = [
        (
            "bass",
            1.0,
            55.0,
            260.0,
            vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
            vec![1.0, 0.0, 0.6, 1.0, 0.0, 0.8, 0.0, 1.0, 1.0, 0.0, 0.6, 0.0, 0.0, 1.0, 0.0, 0.8],
            vec![],
            0.0,
            1.0,
        ),
        (
            "mid",
            2.0,
            220.0,
            120.0,
            vec![0.0, 0.0, 1.0, 0.0],
            vec![1.0, 1.0, 0.0, 1.0, 1.0, 0.0],
            vec![(3, 70.0)],
            0.0,
            0.5,
        ),
        (
            "high",
            2.0,
            880.0,
            70.0,
            vec![1.0, 0.5, 0.0, 0.0, 0.7, 0.0, 0.0, 0.5, 1.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.7, 0.0],
            vec![1.0, 1.0, 1.0, 0.0, 1.0],
            vec![(1, 80.0), (9, 60.0)],
            1.0,
            -1.0,
        ),
    ];
    let mut routes = [0; 3];
    for (k, (name, wave, hz, decay, a, b, chance, morph, depth)) in voices.into_iter().enumerate() {
        let y = 190.0 * k as f32;
        let osc = e.add_module("osc.va", at(30.0, 200.0 + y));
        let env = e.add_module("env.adsr", at(230.0, 200.0 + y));
        let vca = e.add_module("vca", at(430.0, 200.0 + y));
        e.set_param(osc, "waveform", wave);
        e.set_param(osc, "base_hz", hz);
        e.set_param(env, "attack_ms", 1.0);
        e.set_param(env, "decay_ms", decay);
        e.set_param(env, "sustain", 0.0);
        e.set_param(env, "release_ms", 40.0);
        e.set_param(vca, "gain", 0.0);
        e.connect(jack(osc, "out"), jack(vca, "in"));
        e.connect(jack(env, "out"), jack(vca, "cv"));
        e.connect(jack(vca, "out"), jack(mix, &format!("in{}", k + 1)));
        let gate = e.connect(jack(clock, "gate"), jack(env, "gate"));
        cable(&mut e, gate, &pattern(&a, &b, &chance, morph));
        // The one macro output into this cable's morph, at its own depth.
        let route = e
            .connect_cable_route(jack(knob, "m1"), gate, "morph")
            .unwrap_or_else(|| panic!("{name}: route"));
        e.set_route_amount(route, depth, true);
        routes[k] = route;
    }
    e.connect(jack(mix, "out"), jack(out, "left"));
    e.connect(jack(mix, "out"), jack(out, "right"));
    (e, knob, routes)
}

fn render(state: &kabl_core::PatchState, knob: ModuleId) -> Vec<f32> {
    let mut c = compile(state, SR, 4).expect("the demo compiles");
    let blocks = (BARS * BAR_S * SR) as usize / BLOCK;
    let mut out = Vec::with_capacity(blocks * BLOCK * 2);
    for b in 0..blocks {
        let bar = (b * BLOCK) as f32 / SR / BAR_S;
        c.set_runtime(
            RuntimeTarget::Param {
                id: knob,
                kind: "macro",
                index: 0,
            },
            knob_at(bar),
            false,
        );
        c.process_block();
        for (l, r) in c.left().iter().zip(c.right()) {
            out.push(*l);
            out.push(*r);
        }
    }
    out
}

fn write_wav(path: &Path, samples: &[f32]) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for &s in samples {
        w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16)
            .unwrap();
    }
    w.finalize().unwrap();
}

const SOUND_TOML: &str = r#"name = "Macro Morph"
category = "Piece"
tags = ["sequenced", "functional cables", "morph", "macro", "120bpm"]
description = "Three gate cables, each with a pattern A and a pattern B, and one macro (m1) routed into all three morphs at +100 %, +50 % and -100 %. Raise the macro: the bass and the middle voice fill in, the high voice thins out. Open a cable's Pattern editor to see its source and change its depth."
keys = false
sequence = true
"#;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (e, knob, routes) = build();
    match args.get(1).map(String::as_str) {
        Some("write") => {
            let dir = Path::new(&args[2]);
            kabl_core::save(dir, e.log()).expect("save");
            std::fs::write(dir.join("sound.toml"), SOUND_TOML).expect("sound.toml");
            println!("wrote {} (macro {knob}, routes {routes:?})", dir.display());
        }
        Some("render") => {
            let state = e.log().state().clone();
            let clip = render(&state, knob);
            assert_eq!(clip, render(&state, knob), "render is deterministic");
            write_wav(Path::new(&args[2]), &clip);
            let peak = clip.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            println!("peak {peak:.3}");
            let bar = (BAR_S * SR) as usize * 2;
            for (k, w) in clip.chunks(bar).enumerate() {
                let rms = (w.iter().map(|s| s * s).sum::<f32>() / w.len() as f32).sqrt();
                println!("bar {k:2}: macro {:.2} rms {rms:.3}", knob_at(k as f32));
            }
        }
        _ => eprintln!("usage: cable_routes_demo write <patch dir> | render <out.wav>"),
    }
}
