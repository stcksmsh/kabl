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
/// Seconds of audio per run and runs per row; `BENCH_FAST=1` uses 1 s and 5 runs (new rows only need a rough number).
fn plan() -> (usize, usize) {
    if std::env::var_os("BENCH_FAST").is_some() {
        (1, 5)
    } else {
        (2, 15)
    }
}

fn quality() -> QualityConfig {
    QualityConfig {
        tier: QualityTier::Live,
    }
}

/// Runs `ops` modules in a chain (each module's first output into the next one's `feed` input,
/// if it has one) for a few seconds and returns nanoseconds per sample of the whole chain.
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
    let (seconds, runs) = plan();
    let blocks = seconds * SR as usize / BLOCK;
    let mut best = Vec::new();
    for _ in 0..runs {
        let mut feed;
        let mut outs: Vec<[f32; BLOCK]> = vec![[0.0; BLOCK]; infos.len()];
        let pitch = [3.0f32; BLOCK];
        let ones = [1.0f32; BLOCK];
        let square: [f32; BLOCK] = std::array::from_fn(|i| (i < BLOCK / 2) as u8 as f32);
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
                        "gate" | "velocity" => Signal::Buffer(&ones),
                        "clock" | "a" | "b" => Signal::Buffer(&square),
                        _ => Signal::Buffer(&[0.0; BLOCK]),
                    })
                    .collect();
                let ps: Vec<Signal> = params[k].iter().map(|&v| Signal::Scalar(v)).collect();
                let n_out = infos[k]
                    .ports
                    .iter()
                    .filter(|p| p.direction == kabl_modules::PortDirection::Output)
                    .count();
                let mut bufs = vec![[0.0f32; BLOCK]; n_out.max(1)];
                {
                    let mut refs: Vec<&mut [f32]> = bufs.iter_mut().map(|b| &mut b[..]).collect();
                    let mut io = ProcessIo::new(&ins, &mut refs, &ps, BLOCK);
                    m.process(&mut io);
                }
                let out = bufs[0];
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

const CHAIN4: [(&str, f32); 2] = [("level3", 0.5), ("level4", 0.4)];
const CHAIN6: [(&str, f32); 4] = [
    ("level3", 0.5),
    ("level4", 0.4),
    ("level5", 0.3),
    ("level6", 0.2),
];
const CHAIN6_4X: [(&str, f32); 5] = [
    ("level3", 0.5),
    ("level4", 0.4),
    ("level5", 0.3),
    ("level6", 0.2),
    ("oversample", 1.0),
];

const RATCHET4: [(&str, f32); 8] = [
    ("k1", 4.0),
    ("k2", 4.0),
    ("k3", 4.0),
    ("k4", 4.0),
    ("k5", 4.0),
    ("k6", 4.0),
    ("k7", 4.0),
    ("k8", 4.0),
];

fn hold_chord(m: &mut dyn Module) {
    if let Some(arp) = m.as_any_mut().downcast_mut::<kabl_modules::builtins::Arp>() {
        for n in [50, 53, 57, 60] {
            arp.note_on(n, 100);
        }
    }
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
        (
            "osc.fm operator, 4x",
            time(&["osc.fm"], &[("oversample", 1.0)], |_| {}),
        ),
        (
            "osc.fm6, 2x, 1 op sounding",
            time(&["osc.fm6"], &[("algorithm", 7.0), ("level2", 0.0)], |_| {}),
        ),
        (
            "osc.fm6, 2x, 2 ops (default)",
            time(&["osc.fm6"], &[], |_| {}),
        ),
        (
            "osc.fm6, 2x, 4-op chain",
            time(&["osc.fm6"], &CHAIN4, |_| {}),
        ),
        (
            "osc.fm6, 2x, 6-op chain",
            time(&["osc.fm6"], &CHAIN6, |_| {}),
        ),
        (
            "osc.fm6, 4x, 6-op chain",
            time(&["osc.fm6"], &CHAIN6_4X, |_| {}),
        ),
    ];
    let rows: Vec<(&str, f64)> = rows
        .into_iter()
        .chain([
            (
                "quantizer (major), steady input",
                time(&["quantizer"], &[], |_| {}),
            ),
            ("noise (for the row below)", time(&["noise"], &[], |_| {})),
            (
                "noise -> quantizer, new input every sample",
                time(&["noise", "quantizer"], &[], |_| {}),
            ),
            ("sample.hold", time(&["sample.hold"], &[], |_| {})),
            ("slew", time(&["slew"], &[], |_| {})),
            ("attenuverter", time(&["attenuverter"], &[], |_| {})),
            ("logic (4 outputs)", time(&["logic"], &[], |_| {})),
            ("comparator", time(&["comparator"], &[], |_| {})),
            ("crossfade", time(&["crossfade"], &[], |_| {})),
            ("pan", time(&["pan"], &[], |_| {})),
            ("random", time(&["random"], &[], |_| {})),
            (
                "random, loop of 8, 50% change",
                time(&["random"], &[("length", 8.0), ("change", 50.0)], |_| {}),
            ),
            ("clock, straight", time(&["clock"], &[], |_| {})),
            (
                "clock, swing 58",
                time(&["clock"], &[("swing", 58.0)], |_| {}),
            ),
            ("seq", time(&["seq"], &[], |_| {})),
            (
                "seq, every step ratcheted 4x",
                time(&["seq"], &RATCHET4, |_| {}),
            ),
            ("arp, up, 4 keys", time(&["arp"], &[], hold_chord)),
            (
                "arp, random, 4 octaves, ratchet 3",
                time(
                    &["arp"],
                    &[("mode", 4.0), ("octaves", 4.0), ("ratchet", 3.0)],
                    hold_chord,
                ),
            ),
        ])
        .collect();
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
