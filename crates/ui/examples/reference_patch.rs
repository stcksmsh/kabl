//! The modulation-slice reference patch, built through the real `PatchEditor` op log, plus
//! deterministic offline renders of both envelope timing modes.
//!
//! ```sh
//! cargo run -p kabl-ui --example reference_patch -- write patches/reference
//! cargo run -p kabl-ui --example reference_patch -- render patches/reference docs/modulation-slice/renders
//! ```
//!
//! Patch: `midi.in -> osc.va (saw) -> filter.svf -> vca -> out`, `env.adsr` on the VCA.
//! Attack has two sources (slow LFO +30 %, velocity -25 %). Cutoff has four (slow LFO,
//! fast LFO, envelope, velocity): the selection stress case.

use std::path::Path;

use kabl_core::{PortRef, Vec2};
use kabl_engine::compile::compile;
use kabl_engine::graph::BLOCK;
use kabl_ui::PatchEditor;

fn jack(id: u64, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

fn build() -> PatchEditor {
    let mut e = PatchEditor::new();
    let at = |x: f32, y: f32| Vec2 { x, y };
    let midi = e.add_module("midi.in", at(20.0, 20.0));
    let osc = e.add_module("osc.va", at(140.0, 20.0));
    let filter = e.add_module("filter.svf", at(360.0, 20.0));
    let env = e.add_module("env.adsr", at(520.0, 300.0));
    let vca = e.add_module("vca", at(520.0, 20.0));
    let out = e.add_module("out", at(660.0, 20.0));
    let slow = e.add_module("lfo", at(140.0, 300.0));
    let fast = e.add_module("lfo", at(300.0, 300.0));

    e.set_param(osc, "waveform", 2.0);
    e.set_param(filter, "cutoff_hz", 1400.0);
    e.set_param(filter, "resonance", 0.35);
    e.set_param(env, "attack_ms", 60.0);
    e.set_param(env, "decay_ms", 350.0);
    e.set_param(env, "sustain", 0.6);
    e.set_param(env, "release_ms", 450.0);
    e.set_param(vca, "gain", 0.0);
    e.set_param(slow, "rate_hz", 0.4);
    e.set_param(slow, "waveform", 1.0);
    e.set_param(fast, "rate_hz", 3.1);

    e.connect(jack(midi, "pitch"), jack(osc, "pitch"));
    e.connect(jack(osc, "out"), jack(filter, "in"));
    e.connect(jack(filter, "lp"), jack(vca, "in"));
    e.connect(jack(midi, "gate"), jack(env, "gate"));
    e.connect(jack(env, "out"), jack(vca, "cv"));
    e.connect(jack(vca, "out"), jack(out, "left"));
    e.connect(jack(vca, "out"), jack(out, "right"));

    // Two sources on Attack.
    let a1 = e.connect_route(jack(slow, "out"), env, "attack_ms");
    e.set_route_amount(a1, 0.30, true);
    let a2 = e.connect_route(jack(midi, "velocity"), env, "attack_ms");
    e.set_route_amount(a2, -0.25, true);
    // Four sources on Cutoff.
    for (src, amount) in [
        (jack(slow, "out"), 0.12),
        (jack(fast, "out"), 0.05),
        (jack(env, "out"), 0.30),
        (jack(midi, "velocity"), 0.15),
    ] {
        let c = e.connect_route(src, filter, "cutoff_hz");
        e.set_route_amount(c, amount, true);
    }
    e
}

/// 8 voices, four overlapping notes with different velocities, 5 s mono at 48 kHz.
fn render(state: &kabl_core::PatchState) -> Vec<f32> {
    const SR: f32 = 48000.0;
    let mut c = compile(state, SR, 8).expect("reference patch compiles");
    // (start s, length s, voice, semitones, velocity)
    let notes = [
        (0.0, 0.9, 0, -12.0, 0.4),
        (1.0, 0.9, 1, -8.0, 0.9),
        (2.0, 0.9, 2, -5.0, 0.6),
        (3.0, 1.2, 3, 0.0, 1.0),
    ];
    let blocks = (5.0 * SR) as usize / BLOCK;
    let mut out = Vec::with_capacity(blocks * BLOCK);
    for b in 0..blocks {
        let t = (b * BLOCK) as f32 / SR;
        let t_next = ((b + 1) * BLOCK) as f32 / SR;
        for &(start, len, voice, semis, vel) in &notes {
            if start >= t && start < t_next {
                c.note_on(voice, semis, vel);
            }
            let end: f32 = start + len;
            if end >= t && end < t_next {
                c.note_off(voice);
            }
        }
        c.process_block();
        out.extend_from_slice(c.left());
    }
    out
}

fn write_wav(path: &Path, samples: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48000,
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("write") => {
            let dir = Path::new(&args[2]);
            kabl_core::save(dir, build().log()).expect("save");
            println!("wrote {}", dir.display());
        }
        Some("render") => {
            let log = kabl_core::load(Path::new(&args[2])).expect("load patch");
            let out_dir = Path::new(&args[3]);
            std::fs::create_dir_all(out_dir).unwrap();
            let env = log
                .state()
                .modules
                .iter()
                .find(|(_, m)| m.kind == "env.adsr")
                .map(|(&id, _)| id)
                .unwrap();
            let mut results = Vec::new();
            for (mode, timing) in [("continuous", 0.0), ("key-trigger", 1.0)] {
                let mut state = log.state().clone();
                state
                    .modules
                    .get_mut(&env)
                    .unwrap()
                    .params
                    .insert("timing".into(), timing);
                let a = render(&state);
                assert_eq!(a, render(&state), "render is deterministic");
                let path = out_dir.join(format!("reference-{mode}.wav"));
                write_wav(&path, &a);
                let peak = a.iter().fold(0.0f32, |m, s| m.max(s.abs()));
                let rms = (a.iter().map(|s| s * s).sum::<f32>() / a.len() as f32).sqrt();
                println!("{}: peak {peak:.3}, rms {rms:.4}", path.display());
                results.push(a);
            }
            let diff = results[0]
                .iter()
                .zip(&results[1])
                .map(|(x, y)| (x - y) * (x - y))
                .sum::<f32>()
                / results[0].len() as f32;
            let first = results[0].iter().zip(&results[1]).position(|(x, y)| x != y);
            println!(
                "modes differ: rms diff {:.4}, first differing sample {first:?}",
                diff.sqrt()
            );
        }
        _ => eprintln!("usage: reference_patch write <dir> | render <patch dir> <out dir>"),
    }
}
