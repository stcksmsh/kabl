//! The two functional-cable demo patches, built through the real `PatchEditor`, and offline
//! renders that play the same patch first with its cables plain, then as saved.
//!
//! ```sh
//! cargo run -p kabl-ui --example functional_cable_demos -- write patches/functional-cables
//! cargo run -p kabl-ui --example functional_cable_demos -- render patches/functional-cables docs/functional-cables
//! ```
//!
//! Both patches play by themselves (a clock and a sequencer, no keyboard).

use std::path::Path;

use kabl_core::{CableId, ParamTarget, PortRef, Vec2};
use kabl_engine::compile::compile;
use kabl_engine::graph::BLOCK;
use kabl_ui::PatchEditor;

const SR: f32 = 48000.0;

fn jack(id: u64, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

fn at(x: f32, y: f32) -> Vec2 {
    Vec2 { x, y }
}

fn cable_params(e: &mut PatchEditor, cable: CableId, params: &[(&str, f32)]) {
    for &(name, v) in params {
        e.set_param_gesture(
            ParamTarget::Cable {
                id: cable,
                param: name.into(),
            },
            v,
            true,
        );
    }
}

/// An arpeggio whose sends to a delay are patterned and left to chance, and whose pitch cable
/// sometimes holds the last note instead of moving.
fn echo_throws() -> PatchEditor {
    let mut e = PatchEditor::new();
    let clock = e.add_module("clock", at(20.0, 20.0));
    let seq = e.add_module("seq", at(200.0, 20.0));
    let osc = e.add_module("osc.va", at(20.0, 300.0));
    let filter = e.add_module("filter.svf", at(240.0, 300.0));
    let env = e.add_module("env.adsr", at(460.0, 300.0));
    let vca = e.add_module("vca", at(660.0, 300.0));
    let delay = e.add_module("delay", at(660.0, 20.0));
    let mixer = e.add_module("mixer", at(880.0, 160.0));
    let out = e.add_module("out", at(1080.0, 160.0));

    e.set_param(clock, "bpm", 112.0);
    for (k, p) in [0.0, 3.0, 7.0, 12.0, 10.0, 7.0, 3.0, 15.0]
        .into_iter()
        .enumerate()
    {
        e.set_param(seq, &format!("p{}", k + 1), p - 12.0);
    }
    e.set_param(osc, "waveform", 2.0);
    e.set_param(osc, "base_hz", 261.63);
    e.set_param(filter, "cutoff_hz", 2200.0);
    e.set_param(filter, "resonance", 0.3);
    e.set_param(env, "attack_ms", 3.0);
    e.set_param(env, "decay_ms", 160.0);
    e.set_param(env, "sustain", 0.0);
    e.set_param(env, "release_ms", 120.0);
    e.set_param(vca, "gain", 0.0);
    e.set_param(delay, "sync", 3.0);
    e.set_param(delay, "feedback", 55.0);
    e.set_param(delay, "mix", 100.0);
    e.set_param(delay, "tone_hz", 3200.0);
    e.set_param(delay, "mode", 1.0);
    e.set_param(mixer, "level1", 0.35);
    e.set_param(mixer, "level2", 0.3);
    e.set_param(mixer, "level3", 0.3);

    e.connect(jack(clock, "gate"), jack(seq, "clock"));
    e.connect(jack(clock, "reset"), jack(seq, "reset"));
    // Pitch cable: 70 % of the steps move to the new note, the rest keep sounding the last one.
    let pitch = e.connect(jack(seq, "pitch"), jack(osc, "pitch"));
    cable_params(&mut e, pitch, &[("prob", 70.0)]);
    e.connect(jack(seq, "gate"), jack(env, "gate"));
    e.connect(jack(osc, "out"), jack(filter, "in"));
    e.connect(jack(filter, "lp"), jack(vca, "in"));
    e.connect(jack(env, "out"), jack(vca, "cv"));
    e.connect(jack(vca, "out"), jack(mixer, "in1"));
    // Send cable: eight steps, only some open, each throw happening 60 % of the time.
    let send = e.connect(jack(vca, "out"), jack(delay, "in"));
    cable_params(
        &mut e,
        send,
        &[
            ("length", 8.0),
            ("s2", 0.0),
            ("s3", 0.0),
            ("s4", 0.7),
            ("s5", 0.0),
            ("s6", 0.0),
            ("s8", 0.0),
            ("prob", 60.0),
        ],
    );
    e.connect(jack(clock, "gate"), jack(delay, "clock"));
    e.connect(jack(delay, "left"), jack(mixer, "in2"));
    e.connect(jack(delay, "right"), jack(mixer, "in3"));
    e.connect(jack(mixer, "out"), jack(out, "left"));
    e.connect(jack(mixer, "out"), jack(out, "right"));
    e
}

/// A bass line of eight steps whose filter is moved by a five-step route, with a pad chopped by
/// a sixteen-step audio cable.
fn five_against_eight() -> PatchEditor {
    let mut e = PatchEditor::new();
    let clock = e.add_module("clock", at(20.0, 20.0));
    let seq = e.add_module("seq", at(200.0, 20.0));
    let bass = e.add_module("osc.va", at(20.0, 300.0));
    let ladder = e.add_module("filter.ladder", at(240.0, 300.0));
    let env = e.add_module("env.adsr", at(460.0, 300.0));
    let vca = e.add_module("vca", at(660.0, 300.0));
    let knob = e.add_module("macro", at(460.0, 20.0));
    let pad = e.add_module("osc.va", at(20.0, 560.0));
    let mixer = e.add_module("mixer", at(880.0, 160.0));
    let out = e.add_module("out", at(1080.0, 160.0));

    e.set_param(clock, "bpm", 104.0);
    for (k, p) in [0.0, 0.0, 12.0, 0.0, 7.0, 0.0, 10.0, 5.0]
        .into_iter()
        .enumerate()
    {
        e.set_param(seq, &format!("p{}", k + 1), p - 24.0);
    }
    e.set_param(bass, "waveform", 2.0);
    e.set_param(ladder, "cutoff_hz", 260.0);
    e.set_param(ladder, "resonance", 0.55);
    e.set_param(env, "attack_ms", 2.0);
    e.set_param(env, "decay_ms", 220.0);
    e.set_param(env, "sustain", 0.25);
    e.set_param(env, "release_ms", 90.0);
    e.set_param(vca, "gain", 0.0);
    e.set_param(knob, "m1", 1.0);
    e.set_param(pad, "waveform", 2.0);
    e.set_param(pad, "base_hz", 130.81);
    e.set_param(pad, "unison", 4.0);
    e.set_param(pad, "detune", 18.0);
    e.set_param(mixer, "level1", 0.55);
    e.set_param(mixer, "level2", 0.2);

    e.connect(jack(clock, "gate"), jack(seq, "clock"));
    e.connect(jack(clock, "reset"), jack(seq, "reset"));
    e.connect(jack(seq, "pitch"), jack(bass, "pitch"));
    e.connect(jack(seq, "gate"), jack(env, "gate"));
    e.connect(jack(bass, "out"), jack(ladder, "in"));
    e.connect(jack(ladder, "out"), jack(vca, "in"));
    e.connect(jack(env, "out"), jack(vca, "cv"));
    e.connect(jack(vca, "out"), jack(mixer, "in1"));
    // A five-step pattern moves the cutoff against the eight-step line: the two realign only
    // every forty steps. Step 4 is shut, step 2 happens 70 % of the time.
    let sweep = e.connect_route(jack(knob, "m1"), ladder, "cutoff_hz");
    e.set_route_amount(sweep, 0.5, true);
    cable_params(
        &mut e,
        sweep,
        &[
            ("length", 5.0),
            ("s2", 0.25),
            ("s3", 0.6),
            ("s4", 0.0),
            ("s5", 0.8),
            ("r2", 70.0),
        ],
    );
    // A gate on the pad: sixteen steps, hard on and off.
    let chop = e.connect(jack(pad, "out"), jack(mixer, "in2"));
    cable_params(
        &mut e,
        chop,
        &[
            ("length", 16.0),
            ("s2", 0.0),
            ("s5", 0.0),
            ("s7", 0.0),
            ("s9", 0.0),
            ("s10", 0.0),
            ("s13", 0.0),
            ("s15", 0.0),
        ],
    );
    e.connect(jack(mixer, "out"), jack(out, "left"));
    e.connect(jack(mixer, "out"), jack(out, "right"));
    e
}

/// `state` with every functional-cable param removed: the same patch with plain cables.
fn plain(state: &kabl_core::PatchState) -> kabl_core::PatchState {
    let mut p = state.clone();
    for c in p.cables.values_mut() {
        c.params.retain(|k, _| k == "amount" || k == "bypass");
    }
    p
}

fn render(state: &kabl_core::PatchState, seconds: f32) -> Vec<f32> {
    let mut c = compile(state, SR, 4).expect("demo compiles");
    let blocks = (seconds * SR) as usize / BLOCK;
    let mut out = Vec::with_capacity(blocks * BLOCK);
    for _ in 0..blocks {
        c.process_block();
        out.extend(c.left().iter().zip(c.right()).map(|(l, r)| 0.5 * (l + r)));
    }
    out
}

fn write_wav(path: &Path, samples: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
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

type Demo = (&'static str, fn() -> PatchEditor);
const DEMOS: [Demo; 2] = [
    ("echo-throws", echo_throws),
    ("five-against-eight", five_against_eight),
];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("write") => {
            for (name, build) in DEMOS {
                let dir = Path::new(&args[2]).join(name);
                kabl_core::save(&dir, build().log()).expect("save");
                println!("wrote {}", dir.display());
            }
        }
        Some("render") => {
            let out_dir = Path::new(&args[3]);
            std::fs::create_dir_all(out_dir).unwrap();
            for (name, _) in DEMOS {
                let log = kabl_core::load(&Path::new(&args[2]).join(name)).expect("load patch");
                let (before, after) = (render(&plain(log.state()), 8.0), render(log.state(), 24.0));
                assert_eq!(after, render(log.state(), 24.0), "render is deterministic");
                // Eight seconds plain, a second of silence, then the patch as saved.
                let mut clip = before.clone();
                clip.extend(std::iter::repeat_n(0.0, SR as usize));
                clip.extend_from_slice(&after);
                let path = out_dir.join(format!("{name}.wav"));
                write_wav(&path, &clip);
                let peak = clip.iter().fold(0.0f32, |m, s| m.max(s.abs()));
                println!("{}: peak {peak:.3}", path.display());
            }
        }
        _ => eprintln!("usage: functional_cable_demos write|render <patches dir> [<clips dir>]"),
    }
}
