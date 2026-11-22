//! Offline segment renders for the promo animatic (docs/promo/STORYBOARD-V2.md,
//! docs/promo/build_animatic.py).
//!
//! ```sh
//! cargo run --release -p kabl-tools --example promo_render -- render SPEC.json OUT_DIR
//! cargo run --release -p kabl-tools --example promo_render -- list PATCH [PATCH ...]
//! ```
//!
//! `SPEC.json` is `{"segments": [{name, patch, secs, preroll, bpm?, run?, macros?, ramps?, cues?,
//! hits?}]}`. `patch` is a preset folder or a `.kabl` file. Times are seconds from the start of the
//! written audio; `preroll` seconds are rendered first and thrown away (times inside it are
//! negative), so a self-playing piece can already be in full swing on the first sample. `bpm` sets
//! every clock's tempo; `run` (default `-preroll`) is when the clocks start. `macros` sets
//! `{"k": value}` before the engine starts, `ramps` moves macro k at runtime (`[k, t0, t1, v0, v1]`,
//! like a hand on the knob), `cues` launches cue n at once (`[t, n]`), `hits` plays the keyboard
//! (`[t, note, velocity|0]`). Each segment is written as `OUT_DIR/<name>.wav` (32-bit float, 48 kHz).
//!
//! `list` prints each patch's clock tempo, macros (number, name, stored value) and cues, to write
//! the spec from.

use std::path::Path;

use basedrop::Collector;
use kabl_core::{ModuleId, PatchState};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::{Command, PatchEngine, Timing};
use kabl_engine::runtime::RuntimeTarget;
use kabl_modules::builtins::Transport;
use kabl_modules::registry;
use kabl_ui::cues;
use serde_json::Value;

const SR: f32 = 48000.0;
/// Blocks between runtime writes of a ramped macro (about 5 ms).
const WRITE_EVERY: usize = 4;

fn load(path: &str) -> PatchState {
    let p = Path::new(path);
    let log = if p.extension().is_some_and(|e| e == "kabl") {
        kabl_core::format::load_kabl(p)
    } else {
        kabl_core::load(p)
    };
    log.unwrap_or_else(|e| panic!("load {path}: {e:?}"))
        .state()
        .clone()
}

fn of(state: &PatchState, kind: &str) -> Option<ModuleId> {
    state
        .modules
        .iter()
        .find(|(_, m)| m.kind == kind)
        .map(|(&id, _)| id)
}

fn f(v: &Value) -> f32 {
    v.as_f64().expect("number") as f32
}

fn write_wav(path: &Path, left: &[f32], right: &[f32]) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR as u32,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for (l, r) in left.iter().zip(right) {
        w.write_sample(*l).unwrap();
        w.write_sample(*r).unwrap();
    }
    w.finalize().unwrap();
}

fn render(seg: &Value) -> (Vec<f32>, Vec<f32>) {
    let mut state = load(seg["patch"].as_str().expect("patch"));
    let knob = of(&state, "macro");
    let clocks: Vec<ModuleId> = state
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "clock")
        .map(|(&id, _)| id)
        .collect();
    if let Some(bpm) = seg.get("bpm").and_then(Value::as_f64) {
        for c in &clocks {
            state
                .modules
                .get_mut(c)
                .unwrap()
                .params
                .insert("bpm".into(), bpm as f32);
        }
    }
    if let (Some(m), Some(k)) = (seg.get("macros").and_then(Value::as_object), knob) {
        for (n, v) in m {
            state
                .modules
                .get_mut(&k)
                .unwrap()
                .params
                .insert(format!("m{n}"), f(v));
        }
    }
    let list = |key: &str| {
        seg.get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let (ramps, cue_list, hits) = (list("ramps"), list("cues"), list("hits"));
    let preroll = seg.get("preroll").map_or(0.0, f);
    let secs = f(&seg["secs"]);
    let run = seg.get("run").map_or(-preroll, f);
    let macro_params = registry::create("macro").unwrap().info().params;
    let index = |k: u64| {
        macro_params
            .iter()
            .position(|p| p.name == format!("m{k}"))
            .expect("macro k") as u16
    };
    let cue_id = of(&state, "cues");

    let collector = Collector::new();
    let mut engine = PatchEngine::new(&collector.handle(), &state, SR, 8).expect("compiles");
    let (mut left, mut right) = (Vec::new(), Vec::new());
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    let blocks = ((secs + preroll) * SR) as usize / BLOCK + 1;
    for b in 0..blocks {
        let t0 = (b * BLOCK) as f32 / SR - preroll;
        let t1 = t0 + BLOCK as f32 / SR;
        let now = |t: f32| t >= t0 && t < t1;
        if b == 0 {
            for &c in &clocks {
                engine.transport(c, Transport::Stop);
            }
        }
        if now(run) || (b == 0 && run < t0) {
            for &c in &clocks {
                engine.transport(c, Transport::Run);
            }
        }
        for c in cue_list.iter().filter(|c| now(f(&c[0]))) {
            let mut cmds = Vec::new();
            let n = c[1].as_u64().expect("cue n") as usize;
            cues::launch(&mut cmds, &state, cue_id.expect("a cues module"), n).expect("launch");
            for mut cmd in cmds {
                if let Command::Launch(la) = &mut cmd {
                    la.timing = Timing::Now;
                }
                engine.command(&cmd);
            }
        }
        if let (Some(k), true) = (knob, b % WRITE_EVERY == 0) {
            for rp in &ramps {
                let (t_a, t_b) = (f(&rp[1]), f(&rp[2]));
                if t0 < t_a || t0 > t_b + 0.05 {
                    continue;
                }
                let x = ((t0 - t_a) / (t_b - t_a).max(1e-3)).clamp(0.0, 1.0);
                let v = f(&rp[3]) + (f(&rp[4]) - f(&rp[3])) * x;
                engine.active_mut().set_runtime(
                    RuntimeTarget::Param {
                        id: k,
                        kind: "macro",
                        index: index(rp[0].as_u64().expect("k")),
                    },
                    v,
                    true,
                );
            }
        }
        for h in hits.iter().filter(|h| now(f(&h[0]))) {
            let note = h[1].as_u64().expect("note") as u8;
            let v = h[2].as_u64().expect("velocity") as u8;
            engine.key(if v > 0 {
                KeyEvent::On { note, velocity: v }
            } else {
                KeyEvent::Off { note }
            });
        }
        engine.process_block(&mut l, &mut r);
        left.extend_from_slice(&l);
        right.extend_from_slice(&r);
    }
    let skip = (preroll * SR).round() as usize;
    let n = (secs * SR).round() as usize;
    let cut = |v: Vec<f32>| v[skip..skip + n].to_vec();
    (cut(left), cut(right))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("render") if args.len() == 4 => {
            let spec: Value =
                serde_json::from_str(&std::fs::read_to_string(&args[2]).expect("spec"))
                    .expect("json");
            let out = Path::new(&args[3]);
            std::fs::create_dir_all(out).unwrap();
            for seg in spec["segments"].as_array().expect("segments") {
                let name = seg["name"].as_str().expect("name");
                let (l, r) = render(seg);
                write_wav(&out.join(format!("{name}.wav")), &l, &r);
                let peak = l.iter().chain(&r).fold(0.0f32, |m, s| m.max(s.abs()));
                println!("{name:28} peak {:6.1} dBFS", 20.0 * peak.max(1e-9).log10());
            }
        }
        Some("list") => {
            for path in &args[2..] {
                let state = load(path);
                let bpm: Vec<f32> = state
                    .modules
                    .values()
                    .filter(|m| m.kind == "clock")
                    .map(|m| m.params.get("bpm").copied().unwrap_or(120.0))
                    .collect();
                let mut macros = Vec::new();
                if let Some(k) = of(&state, "macro") {
                    let p = &state.modules[&k].params;
                    for n in (1..=8).filter(|n| p.contains_key(&format!("pin.m{n}"))) {
                        let name =
                            kabl_ui::macro_name(&state, k, &format!("m{n}")).unwrap_or_default();
                        let v = p.get(&format!("m{n}")).copied().unwrap_or(0.0);
                        macros.push(format!("m{n} {name}={v:.2}"));
                    }
                }
                let cue_names: Vec<String> = of(&state, "cues")
                    .map(|id| {
                        cues::cues(&state, id)
                            .into_iter()
                            .map(|c| format!("{} {}", c.n, c.name))
                            .collect()
                    })
                    .unwrap_or_default();
                println!(
                    "{path} | bpm {bpm:?} | {} | cues: {}",
                    macros.join(", "),
                    cue_names.join(", ")
                );
            }
        }
        _ => eprintln!("usage: promo_render render SPEC.json OUT_DIR | list PATCH [PATCH ...]"),
    }
}
