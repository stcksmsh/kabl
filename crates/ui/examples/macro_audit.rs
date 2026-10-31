//! Offline renders for the macro audit (docs/presets/scripts/macro_audit.py reads them).
//!
//!     macro_audit <patches dir> <phrases.json> <out dir> [slug ...]
//!
//! `phrases.json` is what a generator's `phrases` command writes (`{slug: {secs, hits: [[t,
//! note, velocity|0]]}}`). For every preset and every pinned macro (`pin.mK`) it writes:
//!   `<slug>.json`            the routes the macro drives, with the parameter value they give at
//!                            macro 0, 0.5 and 1 (base + macro x amount, in knob travel);
//!   `steps/<slug>-mK-sN.wav` the phrase with that macro alone at N/7 (N = 0..7), 10 s;
//!   `sweep/<slug>-mK.wav`    the phrase while the macro moves 0 to 1 to 0 over 8 s (1 s to 9 s),
//!                            10 s, the macro ramped at runtime like a hand on the knob.
//! The other macros stay at their stored values. Any category works; nothing here is groove-only.

use std::path::Path;

use basedrop::Collector;
use kabl_core::{ModuleId, PatchState, PortRef};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::PatchEngine;
use kabl_engine::runtime::RuntimeTarget;
use kabl_modules::builtins::Transport;
use kabl_modules::registry;

const SR: f32 = 48000.0;
const SECS: f32 = 10.0;
const STEPS: usize = 8;
/// Seconds the macro is held at 0 before the sweep, and the length of the up and the down leg.
const SWEEP_FROM: f32 = 1.0;
const LEG: f32 = 4.0;
/// Blocks between runtime writes of the swept macro (about 21 ms).
const WRITE_EVERY: usize = 16;

struct Hit(f32, u8, Option<u8>);

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

/// The macro value at `t` while it is swept: 0, up over `LEG`, down over `LEG`, 0.
fn sweep(t: f32) -> f32 {
    let x = (t - SWEEP_FROM) / LEG;
    if x <= 0.0 || x >= 2.0 {
        0.0
    } else if x < 1.0 {
        x
    } else {
        2.0 - x
    }
}

/// Plays `hits` for `SECS` seconds, a clocked patch running from 0.5 s. `swept`: a macro module
/// param (module, index in its info) moved with `sweep` at runtime.
fn render(
    state: &PatchState,
    hits: &[Hit],
    swept: Option<(ModuleId, u16)>,
) -> (Vec<f32>, Vec<f32>) {
    let collector = Collector::new();
    let mut engine = PatchEngine::new(&collector.handle(), state, SR, 8).expect("compiles");
    let clocks: Vec<ModuleId> = state
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "clock")
        .map(|(&id, _)| id)
        .collect();
    let (mut left, mut right) = (Vec::new(), Vec::new());
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    for b in 0..(SECS * SR) as usize / BLOCK {
        let t0 = (b * BLOCK) as f32 / SR;
        let t1 = t0 + BLOCK as f32 / SR;
        if b == 0 {
            for &c in &clocks {
                engine.transport(c, Transport::Stop);
            }
        }
        if t0 <= 0.5 && 0.5 < t1 {
            for &c in &clocks {
                engine.transport(c, Transport::Run);
            }
        }
        if let (Some((id, index)), true) = (swept, b % WRITE_EVERY == 0) {
            engine.active_mut().set_runtime(
                RuntimeTarget::Param {
                    id,
                    kind: "macro",
                    index,
                },
                sweep(t0),
                true,
            );
        }
        for h in hits.iter().filter(|h| h.0 >= t0 && h.0 < t1) {
            engine.key(match h.2 {
                Some(velocity) => KeyEvent::On {
                    note: h.1,
                    velocity,
                },
                None => KeyEvent::Off { note: h.1 },
            });
        }
        engine.process_block(&mut l, &mut r);
        left.extend_from_slice(&l);
        right.extend_from_slice(&r);
    }
    (left, right)
}

fn param_info(kind: &str, name: &str) -> Option<kabl_modules::ParamInfo> {
    registry::create(kind)?
        .info()
        .params
        .iter()
        .find(|p| p.name == name)
        .copied()
}

/// What macro `k` of `knob` drives: a parameter (base + macro x amount in knob travel) or the
/// depth of another route (an LFO or envelope whose depth is the macro).
fn routes(state: &PatchState, knob: ModuleId, k: usize) -> Vec<serde_json::Value> {
    let port = format!("m{k}");
    let mut out = Vec::new();
    for (&cid, c) in &state.cables {
        let PortRef::Module { id, port: p } = &c.from else {
            continue;
        };
        if *id != knob || *p != port {
            continue;
        }
        let amount = c.params.get("amount").copied().unwrap_or(0.25);
        match &c.to {
            PortRef::Param { id, param } => {
                let m = &state.modules[id];
                let Some(info) = param_info(&m.kind, param) else {
                    continue;
                };
                let base = m.params.get(param).copied().unwrap_or(info.default);
                let n0 = info.to_norm(base);
                let at = |v: f32| info.from_norm(n0 + v * amount);
                out.push(serde_json::json!({
                    "macro": k, "type": "param", "module": id, "kind": m.kind,
                    "param": param, "unit": info.unit, "min": info.min, "max": info.max,
                    "taper": format!("{:?}", info.taper), "base": base, "amount": amount,
                    "norm_base": n0, "at": [at(0.0), at(0.5), at(1.0)],
                }));
            }
            PortRef::CableParam { cable, param } => {
                let t = state.cables.get(cable);
                let (src, dst) = t.map_or((String::new(), String::new()), |t| {
                    (describe(state, &t.from), describe(state, &t.to))
                });
                let a0 = t
                    .and_then(|t| t.params.get(param))
                    .copied()
                    .unwrap_or(0.0);
                out.push(serde_json::json!({
                    "macro": k, "type": "depth", "cable": cable, "param": param,
                    "from": src, "to": dst, "route_amount": a0, "depth": amount,
                    "at": [a0, a0 + 0.5 * amount, a0 + amount],
                }));
            }
            PortRef::Module { .. } => {}
        }
        let _ = cid;
    }
    out
}

fn describe(state: &PatchState, p: &PortRef) -> String {
    match p {
        PortRef::Module { id, port } => format!("{}#{}.{port}", state.modules[id].kind, id),
        PortRef::Param { id, param } => format!("{}#{}.{param}", state.modules[id].kind, id),
        PortRef::CableParam { cable, param } => format!("cable#{}.{param}", cable),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: macro_audit <patches dir> <phrases.json> <out dir> [slug ...]");
        std::process::exit(2);
    }
    let (patches, out) = (Path::new(&args[1]), Path::new(&args[3]));
    let phrases: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&args[2]).unwrap()).unwrap();
    for sub in ["steps", "sweep"] {
        std::fs::create_dir_all(out.join(sub)).unwrap();
    }
    let slugs: Vec<String> = if args.len() > 4 {
        args[4..].to_vec()
    } else {
        phrases.as_object().unwrap().keys().cloned().collect()
    };
    let macro_params = registry::create("macro").unwrap().info().params;
    for slug in slugs {
        let log = kabl_core::load(&patches.join("presets").join(&slug)).expect("load patch");
        let state = log.state().clone();
        let hits: Vec<Hit> = phrases[&slug]["hits"]
            .as_array()
            .expect("slug in phrases.json")
            .iter()
            .map(|h| {
                let v = h[2].as_u64().unwrap() as u8;
                Hit(
                    h[0].as_f64().unwrap() as f32,
                    h[1].as_u64().unwrap() as u8,
                    (v > 0).then_some(v),
                )
            })
            .collect();
        let knob = *state
            .modules
            .iter()
            .find(|(_, m)| m.kind == "macro")
            .expect("a macro module")
            .0;
        let mut listing = Vec::new();
        for k in (1..=8).filter(|k| state.modules[&knob].params.contains_key(&format!("pin.m{k}")))
        {
            let name = format!("m{k}");
            let index = macro_params.iter().position(|p| p.name == name).unwrap() as u16;
            listing.push(serde_json::json!({
                "macro": k, "start": state.modules[&knob].params.get(&name),
                "routes": routes(&state, knob, k),
            }));
            for s in 0..STEPS {
                let mut st = state.clone();
                st.modules
                    .get_mut(&knob)
                    .unwrap()
                    .params
                    .insert(name.clone(), s as f32 / (STEPS - 1) as f32);
                let (l, r) = render(&st, &hits, None);
                write_wav(&out.join(format!("steps/{slug}-m{k}-s{s}.wav")), &l, &r);
            }
            let mut st = state.clone();
            st.modules
                .get_mut(&knob)
                .unwrap()
                .params
                .insert(name, 0.0);
            let (l, r) = render(&st, &hits, Some((knob, index)));
            write_wav(&out.join(format!("sweep/{slug}-m{k}.wav")), &l, &r);
            println!("{slug} m{k}");
        }
        std::fs::write(
            out.join(format!("{slug}.json")),
            serde_json::to_string_pretty(&serde_json::json!({ "slug": slug, "macros": listing }))
                .unwrap(),
        )
        .unwrap();
    }
}
