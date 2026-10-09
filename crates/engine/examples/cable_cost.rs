//! Cost of functional cables, offline: `process_block` time of a patch with its cables plain
//! and with them functional (`length` 8, `prob` 80 on every cable that goes to a jack), and of
//! the smallest possible patch with one functional cable. Not an audio-device measurement.
//!
//!     taskset -c 2 cargo run --release -p kabl-engine --example cable_cost -- patches/composition
//!
//! `KABL_MORPH=1` adds pattern B (5 steps), morph 0.5 and 20 ms glide to every functional cable;
//! `KABL_IDENTITY=1` makes every step open and never rejected.
//!
//! Arguments: patch folder (default patches/composition), blocks per run (default 40000).

use std::time::Instant;

use kabl_core::{PatchState, PortRef};
use kabl_engine::compile::{compile, CompiledPatch};

const SR: f32 = 48000.0;
const RUNS: usize = 9;

/// Fastest of `RUNS` runs of the mean ns/block (least disturbed by other load), then the median and slowest.
fn measure(c: &mut CompiledPatch, blocks: usize) -> (f64, f64, f64) {
    for _ in 0..2000 {
        c.process_block();
    }
    let mut runs: Vec<f64> = (0..RUNS)
        .map(|_| {
            let t = Instant::now();
            for _ in 0..blocks {
                c.process_block();
            }
            t.elapsed().as_nanos() as f64 / blocks as f64
        })
        .collect();
    runs.sort_by(f64::total_cmp);
    (runs[0], runs[RUNS / 2], runs[RUNS - 1])
}

fn functional(p: &PatchState) -> (PatchState, usize) {
    let mut q = p.clone();
    let mut n = 0;
    for c in q.cables.values_mut() {
        if matches!(c.to, PortRef::Module { .. }) {
            c.params.insert("length".into(), 8.0);
            // Identity pattern (every step open, never rejected) isolates the node from what gating does downstream.
            let prob = if std::env::var_os("KABL_IDENTITY").is_some() {
                100.0
            } else {
                80.0
            };
            c.params.insert("prob".into(), prob);
            morph_params(&mut c.params);
            n += 1;
        }
    }
    (q, n)
}

/// With `KABL_MORPH` set: a second pattern of five steps, half a morph and 20 ms of glide.
fn morph_params(params: &mut std::collections::BTreeMap<String, f32>) {
    if std::env::var_os("KABL_MORPH").is_some() {
        for (k, v) in [("b.length", 5.0), ("morph", 0.5), ("glide_ms", 20.0)] {
            params.insert(k.into(), v);
        }
    }
}

fn play(c: &mut CompiledPatch) {
    for (v, s) in [(0, 0.0), (1, 4.0), (2, 7.0), (3, 12.0)] {
        c.note_on(v, s, 0.8);
    }
}

fn report(name: &str, p: &PatchState, voices: usize, blocks: usize) -> f64 {
    let mut c = compile(p, SR, voices).unwrap();
    play(&mut c);
    let (min, med, hi) = measure(&mut c, blocks);
    let (_, _, steps) = c.profile_counts();
    println!("{name:<34} {min:>9.0} ns/block  (median {med:.0}, slowest {hi:.0}, {steps} steps)");
    min
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = args.get(1).map_or("patches/composition", String::as_str);
    let blocks: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(40000);
    let patch = kabl_core::load(std::path::Path::new(dir)).unwrap();
    let state = patch.state();
    let (fun, n) = functional(state);
    println!("{dir}: {n} jack cables made functional, 8 voices, {blocks} blocks x {RUNS} runs");
    let plain = report("plain", state, 8, blocks);
    let with = report("all jack cables functional", &fun, 8, blocks);
    println!(
        "difference {:+.0} ns/block = {:+.0} ns per functional cable ({:.2} % of a 1333 us block budget)",
        with - plain,
        (with - plain) / n as f64,
        (with - plain) / 1_333_333.0 * 100.0
    );

    // One lfo -> out cable and a clock: the node alone.
    let mut min = PatchState::new();
    let m = |kind: &str| kabl_core::ModuleState {
        kind: kind.into(),
        pos: kabl_core::Vec2::default(),
        params: Default::default(),
    };
    min.modules.insert(1, m("clock"));
    min.modules.insert(2, m("lfo"));
    min.modules.insert(3, m("out"));
    let jack = |from: (u64, &str), to: (u64, &str)| kabl_core::CableState {
        from: PortRef::Module {
            id: from.0,
            port: from.1.into(),
        },
        to: PortRef::Module {
            id: to.0,
            port: to.1.into(),
        },
        params: Default::default(),
        steps: vec![],
    };
    min.cables.insert(1, jack((2, "out"), (3, "left")));
    let a = report("minimal: plain cable", &min, 1, blocks);
    min.cables
        .get_mut(&1)
        .unwrap()
        .params
        .insert("length".into(), 8.0);
    morph_params(&mut min.cables.get_mut(&1).unwrap().params);
    let b = report("minimal: one functional cable", &min, 1, blocks);
    println!("node alone {:+.0} ns/block", b - a);
}
