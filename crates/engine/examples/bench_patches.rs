//! Audio-thread cost of whole patches with all eight voices sounding.
//!
//!     cargo build --release -p kabl-engine --example bench_patches
//!     taskset -c 2 target/release/examples/bench_patches patches/sound-engines/* patches/palette/*
//!
//! For each patch directory: compile at 48 kHz with 8 voices, hold eight notes, and time
//! `process_block` (64 samples, a budget of 1333 us) over 20 000 blocks. Prints the median and
//! the 99.9th percentile in microseconds.

use std::time::Instant;

fn main() {
    println!(
        "{:<38} {:>10} {:>10} {:>9}",
        "patch", "median us", "p99.9 us", "% budget"
    );
    for dir in std::env::args().skip(1) {
        let Ok(log) = kabl_core::load(std::path::Path::new(&dir)) else {
            continue;
        };
        let Ok(mut c) = kabl_engine::compile::compile(log.state(), 48000.0, 8) else {
            continue;
        };
        for v in 0..8 {
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
        let (median, p999) = (times[times.len() / 2], times[times.len() * 999 / 1000]);
        println!(
            "{:<38} {median:>10.1} {p999:>10.1} {:>8.1}%",
            dir.trim_end_matches('/'),
            median / (64.0 / 48000.0 * 1e6) * 100.0
        );
    }
}
