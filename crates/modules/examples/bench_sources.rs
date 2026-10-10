//! Cost of one voice of each sound source, measured on the module alone.
//!
//!     cargo run --release -p kabl-modules --example bench_sources
//!
//! Prints nanoseconds per output sample (the median of 15 runs of 2 s of audio) and the share of
//! one core a single voice takes at 48 kHz. Run it under `taskset -c 2` for steadier numbers.

use kabl_modules::builtins::OscFm;
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{registry, Module, ProcessIo, Signal};
use std::time::Instant;

const SR: f32 = 48000.0;
const BLOCK: usize = 64;
const SECONDS: usize = 2;

fn quality() -> QualityConfig {
    QualityConfig {
        tier: QualityTier::Live,
    }
}

/// Runs `ops` modules in a chain (each module's first output into the next one's `feed` input,
/// if it has one) for `SECONDS` and returns nanoseconds per sample of the whole chain.
fn time(kind_chain: &[&str], set: &[(&str, f32)], tweak: impl Fn(&mut dyn Module)) -> f64 {
    let mut modules: Vec<Box<dyn Module>> = kind_chain
        .iter()
        .map(|k| registry::create(k).unwrap())
        .collect();
    for m in &mut modules {
        m.prepare(SR, BLOCK, &quality());
        tweak(m.as_mut());
    }
    let infos: Vec<_> = modules.iter().map(|m| m.info()).collect();
    let params: Vec<Vec<f32>> = infos
        .iter()
        .map(|i| {
            i.params
                .iter()
                .map(|p| {
                    set.iter()
                        .find(|(n, _)| *n == p.name)
                        .map_or(p.default, |&(_, v)| v)
                })
                .collect()
        })
        .collect();
    let blocks = SECONDS * SR as usize / BLOCK;
    let mut best = Vec::new();
    for _ in 0..15 {
        let mut feed;
        let mut outs: Vec<[f32; BLOCK]> = vec![[0.0; BLOCK]; infos.len()];
        let pitch = [3.0f32; BLOCK];
        let start = Instant::now();
        for _ in 0..blocks {
            feed = [0.0; BLOCK];
            for (k, m) in modules.iter_mut().enumerate() {
                let ins: Vec<Signal> = infos[k]
                    .ports
                    .iter()
                    .filter(|p| p.direction == kabl_modules::PortDirection::Input)
                    .map(|p| match p.name {
                        "pitch" => Signal::Buffer(&pitch),
                        "pm" | "in" => Signal::Buffer(&feed),
                        _ => Signal::Buffer(&[0.0; BLOCK]),
                    })
                    .collect();
                let ps: Vec<Signal> = params[k].iter().map(|&v| Signal::Scalar(v)).collect();
                let mut out = [0.0f32; BLOCK];
                {
                    let mut refs: Vec<&mut [f32]> = vec![&mut out[..]];
                    let mut io = ProcessIo::new(&ins, &mut refs, &ps, BLOCK);
                    m.process(&mut io);
                }
                outs[k] = out;
                feed = out;
            }
            std::hint::black_box(&outs);
        }
        best.push(start.elapsed().as_secs_f64() * 1e9 / (blocks * BLOCK) as f64);
    }
    best.sort_by(|a, b| a.total_cmp(b));
    best[best.len() / 2]
}

fn main() {
    let plain = |m: &mut dyn Module| {
        if let Some(fm) = m.as_any_mut().downcast_mut::<OscFm>() {
            fm.set_oversample(false);
        }
    };
    let rows: Vec<(&str, f64)> = vec![
        ("osc.va saw (reference)", time(&["osc.va"], &[], |_| {})),
        (
            "osc.va saw, unison 4",
            time(&["osc.va"], &[("unison", 4.0)], |_| {}),
        ),
        (
            "osc.wt, one frame",
            time(&["osc.wt"], &[("table", 0.0)], |_| {}),
        ),
        (
            "osc.wt, between two frames",
            time(&["osc.wt"], &[("position", 0.5)], |_| {}),
        ),
        ("osc.fm operator, 1x", time(&["osc.fm"], &[], plain)),
        (
            "osc.fm operator, 2x (shipped)",
            time(&["osc.fm"], &[], |_| {}),
        ),
        (
            "osc.fm operator, 2x, feedback",
            time(&["osc.fm"], &[("feedback", 0.5)], |_| {}),
        ),
        (
            "two-operator stack, 2x",
            time(&["osc.fm", "osc.fm"], &[], |_| {}),
        ),
        (
            "four-operator chain, 2x",
            time(&["osc.fm", "osc.fm", "osc.fm", "osc.fm"], &[], |_| {}),
        ),
    ];
    println!(
        "{:<34} {:>10} {:>14}",
        "source", "ns/sample", "% core / voice"
    );
    for (name, ns) in rows {
        println!(
            "{name:<34} {ns:>10.1} {:>13.2}%",
            ns * SR as f64 / 1e9 * 100.0
        );
    }
}
