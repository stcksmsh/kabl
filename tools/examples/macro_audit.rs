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
//! It also writes the combination renders (`combo/<slug>-base|all0|all1|mA-mB.wav`: the stored
//! sound, every macro at 0, every macro at 1, every pair at 1 with the rest stored).
//!
//! Tour mode, for listening: `MACRO_TOUR_BPM=<bpm> macro_audit ... <slug>` (one preset per run, its
//! stored tempo) skips the steps and writes only `sweep/<slug>-mK.wav`, 20 s: the macro held at 0
//! for 6 s, up over 4 s, held at 1 for 6 s, down over 4 s. The phrase loops at its own length
//! rounded up to whole bars of `bpm` (counted from the 0.5 s clock start), so the keys stay on the
//! grid and the sound is continuous for the whole 20 s.

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

/// The tempo of the tour mode (`MACRO_TOUR_BPM`), when on.
fn tour_bpm() -> Option<f32> {
    std::env::var("MACRO_TOUR_BPM").ok()?.parse().ok()
}

/// Seconds rendered: 10, or 20 in tour mode.
fn secs() -> f32 {
    if tour_bpm().is_some() {
        20.0
    } else {
        SECS
    }
}

/// Whole bars rendered and thrown away before a tour starts (`MACRO_TOUR_PREROLL_BARS`, default 0),
/// for pieces whose first bars are an intro: the 20 s then hold one steady state.
fn preroll() -> f32 {
    let bars: f32 = std::env::var("MACRO_TOUR_PREROLL_BARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0);
    tour_bpm().map_or(0.0, |bpm| bars * 4.0 * 60.0 / bpm)
}

/// The macro value at `t` while it is swept: 0, up over `LEG`, down over `LEG`, 0.
fn sweep(t: f32) -> f32 {
    if tour_bpm().is_some() {
        return match t {
            t if t < 6.0 => 0.0,
            t if t < 10.0 => (t - 6.0) / 4.0,
            t if t < 16.0 => 1.0,
            t if t < 20.0 => 1.0 - (t - 16.0) / 4.0,
            _ => 0.0,
        };
    }
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
    for b in 0..((secs() + preroll()) * SR) as usize / BLOCK {
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
                sweep(t0 - preroll()),
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
    let skip = (preroll() * SR) as usize / BLOCK * BLOCK;
    (left.split_off(skip), right.split_off(skip))
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
                let a0 = t.and_then(|t| t.params.get(param)).copied().unwrap_or(0.0);
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

/// The combination renders (the python side reads them): `combo/<slug>-base.wav` (every macro at
/// its stored value), `-all0` and `-all1` (every pinned macro at 0, at 1), and
/// `combo/<slug>-mA-mB.wav` for every pair at 1 with the others at their stored values.
fn combinations(state: &PatchState, knob: ModuleId, hits: &[Hit], slug: &str, out: &Path) {
    let pinned: Vec<usize> = (1..=8)
        .filter(|k| {
            state.modules[&knob]
                .params
                .contains_key(&format!("pin.m{k}"))
        })
        .collect();
    let with = |set: &[(usize, f32)]| {
        let mut st = state.clone();
        for &(k, v) in set {
            st.modules
                .get_mut(&knob)
                .unwrap()
                .params
                .insert(format!("m{k}"), v);
        }
        render(&st, hits, None)
    };
    std::fs::create_dir_all(out.join("combo")).unwrap();
    let mut jobs: Vec<(String, Vec<(usize, f32)>)> = vec![
        ("base".into(), vec![]),
        ("all0".into(), pinned.iter().map(|&k| (k, 0.0)).collect()),
        ("all1".into(), pinned.iter().map(|&k| (k, 1.0)).collect()),
    ];
    for (i, &a) in pinned.iter().enumerate() {
        for &b in &pinned[i + 1..] {
            jobs.push((format!("m{a}-m{b}"), vec![(a, 1.0), (b, 1.0)]));
        }
    }
    for (name, set) in jobs {
        let (l, r) = with(&set);
        write_wav(&out.join(format!("combo/{slug}-{name}.wav")), &l, &r);
    }
    println!("{slug} combinations");
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
        let hits = match tour_bpm() {
            Some(bpm) => {
                // The phrase loops at its own length rounded up to whole bars of 4 beats (so the
                // keys stay on the grid from the 0.5 s clock start and the sound is continuous).
                let bar = 4.0 * 60.0 / bpm;
                let first = hits.iter().map(|h| h.0).fold(f32::MAX, f32::min);
                let last = hits.iter().map(|h| h.0).fold(0.0f32, f32::max);
                // (A phrase that ends just short of a bar line, 0.03 of a bar, fills that bar.)
                let period = (((last - first) / bar - 0.03).ceil().max(1.0)) * bar;
                (0..)
                    .map(|n| n as f32 * period)
                    .take_while(|o| *o < secs() + preroll())
                    .flat_map(|o| hits.iter().map(move |h| Hit(h.0 + o, h.1, h.2)))
                    .filter(|h| h.0 < secs() + preroll())
                    .collect()
            }
            None => hits,
        };
        let knob = *state
            .modules
            .iter()
            .find(|(_, m)| m.kind == "macro")
            .expect("a macro module")
            .0;
        let mut listing = Vec::new();
        for k in (1..=8).filter(|k| {
            state.modules[&knob]
                .params
                .contains_key(&format!("pin.m{k}"))
        }) {
            let name = format!("m{k}");
            let index = macro_params.iter().position(|p| p.name == name).unwrap() as u16;
            listing.push(serde_json::json!({
                "macro": k, "start": state.modules[&knob].params.get(&name),
                "routes": routes(&state, knob, k),
            }));
            for s in (0..STEPS).filter(|_| tour_bpm().is_none()) {
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
            st.modules.get_mut(&knob).unwrap().params.insert(name, 0.0);
            let (l, r) = render(&st, &hits, Some((knob, index)));
            write_wav(&out.join(format!("sweep/{slug}-m{k}.wav")), &l, &r);
            println!("{slug} m{k}");
        }
        if tour_bpm().is_none() {
            combinations(&state, knob, &hits, &slug, out);
        }
        std::fs::write(
            out.join(format!("{slug}.json")),
            serde_json::to_string_pretty(&serde_json::json!({ "slug": slug, "macros": listing }))
                .unwrap(),
        )
        .unwrap();
    }
}
