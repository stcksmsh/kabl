//! Cost of one drum voice, measured on the module alone.
//!
//!     cargo run --release -p kabl-modules --example bench_drums
//!
//! Prints nanoseconds per output sample (the median of 15 runs of 2 s of audio) and the share of
//! one core at 48 kHz, for a voice that is idle (no gate) and for the worst case, a voice
//! retriggered every 1/16 s so it never stops (a 960 bpm 16th train). Run it under
//! `taskset -c 2` for steadier numbers.

use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{registry, PortDirection, ProcessIo, Signal};
use std::time::Instant;

const SR: f32 = 48000.0;
const BLOCK: usize = 64;

fn run(kind: &str, every: Option<usize>) -> f64 {
    let mut m = registry::create(kind).unwrap();
    m.prepare(
        SR,
        BLOCK,
        &QualityConfig {
            tier: QualityTier::Live,
        },
    );
    let info = m.info();
    let params: Vec<Signal> = info
        .params
        .iter()
        .map(|p| Signal::Scalar(p.default))
        .collect();
    let n_in = info
        .ports
        .iter()
        .filter(|p| p.direction == PortDirection::Input)
        .count();
    let blocks = 2 * SR as usize / BLOCK;
    let mut times = Vec::new();
    for _ in 0..15 {
        let mut gate = [0f32; BLOCK];
        let ones = [1f32; BLOCK];
        let mut out = [0f32; BLOCK];
        let mut sink = 0f32;
        let start = Instant::now();
        for b in 0..blocks {
            for (i, g) in gate.iter_mut().enumerate() {
                let t = b * BLOCK + i;
                *g = every.map_or(0.0, |e| f32::from(u8::from(t % e < e / 2)));
            }
            let ins: Vec<Signal> = (0..n_in)
                .map(|k| match k {
                    0 => Signal::Buffer(&gate),
                    _ if k + 1 == n_in => Signal::Buffer(&ones),
                    _ => Signal::Buffer(&[0.0; BLOCK]),
                })
                .collect();
            let mut outs: [&mut [f32]; 1] = [&mut out];
            let mut io = ProcessIo::new(&ins, &mut outs, &params, BLOCK);
            m.process(&mut io);
            sink += out[0];
        }
        std::hint::black_box(sink);
        times.push(start.elapsed().as_secs_f64() * 1e9 / (blocks * BLOCK) as f64);
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    times[times.len() / 2]
}

fn main() {
    println!(
        "{:<12} {:>12} {:>9} {:>12} {:>9}",
        "voice", "idle ns/smp", "% core", "busy ns/smp", "% core"
    );
    for kind in [
        "drum.kick",
        "drum.snare",
        "drum.clap",
        "drum.hat",
        "drum.tom",
    ] {
        let (idle, busy) = (run(kind, None), run(kind, Some(3000)));
        let pct = |ns: f64| 100.0 * ns * SR as f64 / 1e9;
        println!(
            "{kind:<12} {idle:>12.1} {:>8.3}% {busy:>12.1} {:>8.3}%",
            pct(idle),
            pct(busy)
        );
    }
}
