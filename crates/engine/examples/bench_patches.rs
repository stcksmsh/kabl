//! Audio-thread cost of whole patches with all voices sounding.
//!
//!     cargo build --release -p kabl-engine --example bench_patches
//!     taskset -c 2 target/release/examples/bench_patches patches/sound-engines/* patches/palette/*
//!
//! For each patch directory: compile at 48 kHz with 8 and with 16 voices, hold the notes, and time
//! `process_block` (64 samples, a budget of 1333 us) over 20 000 blocks. Prints the median and
//! the 99.9th percentile in microseconds. A patch with a filter is also timed as a stereo
//! variant (`+st`): a `pan` before every filter input, and each `in_l`/`in_r` pair fed by one
//! source becomes one stereo cable, so every module after the first filter runs once per channel.

use std::time::Instant;

use kabl_core::PatchState;

#[path = "support/stereo.rs"]
mod stereo;

fn time(patch: &PatchState, voices: usize) -> Option<(f64, f64)> {
    let mut c = kabl_engine::compile::compile(patch, 48000.0, voices).ok()?;
    for v in 0..voices {
        c.note_on(v, v as f32 * 2.0 - 7.0, 0.8);
    }
    for _ in 0..2000 {
        c.process_block();
    }
    let mut times: Vec<f64> = (0..20000)
        .map(|_| {
            let start = Instant::now();
            c.process_block();
            std::hint::black_box(c.left());
            start.elapsed().as_secs_f64() * 1e6
        })
        .collect();
    times.sort_by(|a, b| a.total_cmp(b));
    Some((times[times.len() / 2], times[times.len() * 999 / 1000]))
}

fn main() {
    println!(
        "{:<38} {:>6} {:>10} {:>10} {:>9}",
        "patch", "voices", "median us", "p99.9 us", "% budget"
    );
    for dir in std::env::args().skip(1) {
        let Ok(log) = kabl_core::load(std::path::Path::new(&dir)) else {
            continue;
        };
        let name = dir.trim_end_matches('/');
        let variants = [
            (name.to_string(), Some(log.state().clone())),
            (
                format!("{name} +st"),
                stereo::stereo_variant(log.state(), false),
            ),
        ];
        for (label, patch) in variants {
            let Some(patch) = patch else { continue };
            for voices in [8, 16] {
                let Some((median, p999)) = time(&patch, voices) else {
                    continue;
                };
                println!(
                    "{label:<38} {voices:>6} {median:>10.1} {p999:>10.1} {:>8.1}%",
                    median / (64.0 / 48000.0 * 1e6) * 100.0
                );
            }
        }
    }
}
