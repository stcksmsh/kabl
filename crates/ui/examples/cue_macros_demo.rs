//! The cue-macros demo piece: a self-playing bass line and pad with three macros (Brightness,
//! Space, Width) and four cues (Verse, Chorus, Bridge, Verse again) whose macro targets
//! differ. `render` plays the saved patch through the real engine, launching each cue on its
//! bar, to a WAV.
//!
//! ```sh
//! cargo run -p kabl-ui --example cue_macros_demo -- write patches/cue-macros
//! cargo run --release -p kabl-ui --example cue_macros_demo -- render patches/cue-macros docs/cue-macros/cue-macros.wav
//! ```

use std::path::Path;

use basedrop::Collector;
use kabl_core::{ModuleId, PortRef, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::{Command, PatchEngine, Timing};
use kabl_ui::perform::{CC_PREFIX, CUE_PADS, PIN_PREFIX, TRANSPORT};
use kabl_ui::{cues, PatchEditor};

const SR: f32 = 48000.0;
const BPM: f32 = 100.0;
/// Seconds per bar at 100 bpm in 4/4.
const BAR_S: f32 = 4.0 * 60.0 / BPM;

fn jack(id: ModuleId, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

fn at(x: f32, y: f32) -> Vec2 {
    Vec2 { x, y }
}

/// (name, [m1, m2, m3] targets, morph in beats, bar the cue launches on in the render)
const CUES: [(&str, [f32; 3], f32, u32); 4] = [
    ("Verse", [0.10, 0.05, 0.00], 0.0, 0),
    ("Chorus", [0.85, 0.60, 0.90], 8.0, 4),
    ("Bridge", [0.35, 0.95, 0.20], 16.0, 8),
    ("Verse 2", [0.10, 0.05, 0.00], 4.0, 14),
];
const BARS: u32 = 18;
const MACRO_NAMES: [&str; 3] = ["Brightness", "Space", "Width"];

fn build() -> (PatchEditor, ModuleId) {
    let mut e = PatchEditor::new();
    let clock = e.add_module("clock", at(20.0, 20.0));
    let seq = e.add_module("seq", at(200.0, 20.0));
    let bass = e.add_module("osc.va", at(20.0, 300.0));
    let ladder = e.add_module("filter.ladder", at(240.0, 300.0));
    let env = e.add_module("env.adsr", at(460.0, 300.0));
    let vca = e.add_module("vca", at(660.0, 300.0));
    let pad = e.add_module("osc.va", at(20.0, 560.0));
    let mixer = e.add_module("mixer", at(880.0, 160.0));
    let delay = e.add_module("delay", at(1060.0, 160.0));
    let verb = e.add_module("reverb", at(1240.0, 160.0));
    let out = e.add_module("out", at(1420.0, 160.0));
    let knob = e.add_module("macro", at(460.0, 20.0));
    let cue = e.add_module("cues", at(660.0, 20.0));

    e.set_param(clock, "bpm", BPM);
    for (k, p) in [0.0, 0.0, 12.0, 0.0, 7.0, 0.0, 10.0, 5.0]
        .into_iter()
        .enumerate()
    {
        e.set_param(seq, &format!("p{}", k + 1), p - 24.0);
    }
    e.set_param(bass, "waveform", 2.0);
    e.set_param(ladder, "cutoff_hz", 180.0);
    e.set_param(ladder, "resonance", 0.45);
    e.set_param(env, "attack_ms", 3.0);
    e.set_param(env, "decay_ms", 240.0);
    e.set_param(env, "sustain", 0.3);
    e.set_param(env, "release_ms", 120.0);
    e.set_param(vca, "gain", 0.0);
    e.set_param(pad, "waveform", 2.0);
    e.set_param(pad, "base_hz", 130.81);
    e.set_param(pad, "unison", 4.0);
    e.set_param(pad, "detune", 4.0);
    e.set_param(mixer, "level1", 0.25);
    e.set_param(mixer, "level2", 0.0);
    e.set_param(delay, "sync", 3.0);
    e.set_param(delay, "feedback", 45.0);
    e.set_param(delay, "mix", 0.0);
    e.set_param(verb, "decay_s", 5.0);
    e.set_param(verb, "mix", 0.0);

    e.connect(jack(clock, "gate"), jack(seq, "clock"));
    e.connect(jack(clock, "reset"), jack(seq, "reset"));
    e.connect(jack(seq, "pitch"), jack(bass, "pitch"));
    e.connect(jack(seq, "gate"), jack(env, "gate"));
    e.connect(jack(bass, "out"), jack(ladder, "in"));
    e.connect(jack(ladder, "out"), jack(vca, "in"));
    e.connect(jack(env, "out"), jack(vca, "cv"));
    e.connect(jack(vca, "out"), jack(mixer, "in1"));
    e.connect(jack(pad, "out"), jack(mixer, "in2"));
    e.connect(jack(mixer, "out"), jack(delay, "in"));
    e.connect(jack(clock, "gate"), jack(delay, "clock"));
    e.connect(jack(delay, "left"), jack(verb, "in_l"));
    e.connect(jack(delay, "right"), jack(verb, "in_r"));
    e.connect(jack(verb, "left"), jack(out, "left"));
    e.connect(jack(verb, "right"), jack(out, "right"));

    // Brightness opens the bass filter; Space sends to the delay and the reverb; Width brings
    // the pad in and spreads its unison.
    for (dest, param, name, amount) in [
        (ladder, "cutoff_hz", "m1", 0.75),
        (delay, "mix", "m2", 0.45),
        (verb, "mix", "m2", 0.55),
        (mixer, "level2", "m3", 0.28),
        (pad, "detune", "m3", 0.7),
    ] {
        let r = e.connect_route(jack(knob, name), dest, param);
        e.set_route_amount(r, amount, true);
    }

    for (i, name) in MACRO_NAMES.iter().enumerate() {
        let key = format!("m{}", i + 1);
        e.set_label(knob, &format!("name.{key}"), Some((*name).into()));
        e.set_param(knob, &key, CUES[0].1[i]);
    }

    for &(name, targets, beats, _) in &CUES {
        let n = cues::add(&mut e, cue, &|_| 0).expect("cue");
        cues::rename(&mut e, cue, n, name);
        // The cues move macros only: the sequencer keeps playing through every section.
        cues::set_target(&mut e, cue, n, seq, None);
        for (k, v) in targets.into_iter().enumerate() {
            cues::set_macro(&mut e, cue, n, knob, k as u8, Some(v));
        }
        cues::set_morph(&mut e, cue, n, beats);
    }

    let mut pins = vec![
        (clock, TRANSPORT.to_string()),
        (cue, CUE_PADS.to_string()),
    ];
    pins.extend((1..=3).map(|k| (knob, format!("m{k}"))));
    let mut changes = Vec::new();
    for (i, (id, key)) in pins.iter().enumerate() {
        changes.push((*id, format!("{PIN_PREFIX}{key}"), Some(i as f32)));
        if let Some(k) = key.strip_prefix('m') {
            let cc: f32 = 19.0 + k.parse::<f32>().unwrap();
            changes.push((*id, format!("{CC_PREFIX}{key}"), Some(cc)));
        }
    }
    e.set_presentation(&changes);
    (e, cue)
}

const SOUND_TOML: &str = r#"name = "Cue Macros"
category = "Piece"
tags = ["sequenced", "cues", "macros", "morph", "100bpm"]
description = "A self-playing bass line and pad. Four cues (Verse, Chorus, Bridge, Verse 2) each set the three macros to their own values and move there over a set number of beats: Verse dark and dry, Chorus open and wide, Bridge washed in space. Run, then launch the cues on the next bar; turn a macro to take it over."
keys = false
sequence = true
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

fn render(state: &kabl_core::PatchState, cue: ModuleId) -> (Vec<f32>, Vec<f32>) {
    let collector = Collector::new();
    let mut engine = PatchEngine::new(&collector.handle(), state, SR, 4).expect("compiles");
    let blocks = (BARS as f32 * BAR_S * SR) as usize / BLOCK;
    let (mut left, mut right) = (Vec::new(), Vec::new());
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    let mut next = 0;
    for b in 0..blocks {
        let t = (b * BLOCK) as f32 / SR / BAR_S;
        // Launch a cue half a bar before its bar, on the next bar line.
        if let Some(&(_, _, _, bar)) = CUES.get(next) {
            if t >= bar as f32 - 0.5 || (bar == 0 && b == 0) {
                let mut cmds = Vec::new();
                cues::launch(&mut cmds, state, cue, next + 1).expect("launch");
                for mut c in cmds {
                    if let Command::Launch(l) = &mut c {
                        if bar == 0 {
                            l.timing = Timing::Now;
                        }
                    }
                    engine.command(&c);
                }
                next += 1;
            }
        }
        engine.process_block(&mut l, &mut r);
        left.extend_from_slice(&l);
        right.extend_from_slice(&r);
    }
    (left, right)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (e, cue) = build();
    match args.get(1).map(String::as_str) {
        Some("write") => {
            let dir = Path::new(&args[2]);
            kabl_core::save(dir, e.log()).expect("save");
            std::fs::write(dir.join("sound.toml"), SOUND_TOML).expect("sound.toml");
            println!("wrote {} (cues module {cue})", dir.display());
        }
        Some("render") => {
            let log = kabl_core::load(Path::new(&args[2])).expect("load patch");
            let (l, r) = render(log.state(), cue);
            let (l2, _) = render(log.state(), cue);
            assert_eq!(l, l2, "render is deterministic");
            write_wav(Path::new(&args[3]), &l, &r);
            let peak = l.iter().chain(&r).fold(0.0f32, |m, s| m.max(s.abs()));
            println!("peak {peak:.3}");
            for bar in 0..BARS as usize {
                let w = &l[(bar as f32 * BAR_S * SR) as usize..][..(BAR_S * SR) as usize];
                let rms = (w.iter().map(|s| s * s).sum::<f32>() / w.len() as f32).sqrt();
                println!("bar {bar:2}: rms {rms:.3}");
            }
        }
        _ => eprintln!("usage: cue_macros_demo write <patch dir> | render <patch dir> <wav>"),
    }
}
