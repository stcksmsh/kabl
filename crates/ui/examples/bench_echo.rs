//! Audio-callback timing for `patches/echo` (docs/echo/README.md): steady processing and
//! callbacks that install graph swaps (the delay lines copied at fade start), with no delay,
//! one delay and two. Each swap graph is compiled just before its callback, outside the timing,
//! as the UI thread does. `cargo run --release -p kabl-ui --example bench_echo [frames]`
use basedrop::Collector;
use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::PatchEngine;
use std::time::{Duration, Instant};

const SR: f32 = 48000.0;
const CALLBACKS: usize = 4000;
/// A swap every this many callbacks (every 21 ms at 256 frames: faster than a knob drag).
const SWAP_EVERY: usize = 4;

fn load(name: &str) -> PatchState {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../patches")
        .join(name);
    kabl_core::load(&dir).unwrap().state().clone()
}

fn port(id: u64, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

/// `patches/echo` plus a second delay on the bass, into the left mixer.
fn two_delays() -> PatchState {
    let mut p = load("echo");
    p.modules.insert(
        100,
        ModuleState {
            kind: "delay".into(),
            pos: Vec2 { x: 0.0, y: 0.0 },
            params: [("sync".to_string(), 0.0), ("time_ms".to_string(), 3900.0)].into(),
        },
    );
    let to_in1 = *p
        .cables
        .iter()
        .find(|(_, c)| c.to == port(6, "in1"))
        .unwrap()
        .0;
    p.cables.get_mut(&to_in1).unwrap().from = port(100, "left");
    p.cables.insert(
        1000,
        CableState {
            from: port(11, "out"),
            to: port(100, "in"),
            params: Default::default(),
            steps: vec![],
        },
    );
    p
}

struct Stats(Vec<Duration>);

impl Stats {
    fn line(&mut self) -> String {
        self.0.sort();
        let q = |f: f64| self.0[((self.0.len() - 1) as f64 * f) as usize].as_secs_f64() * 1e6;
        format!(
            "median {:>7.1}  p99 {:>7.1}  max {:>7.1} µs  (n={})",
            q(0.5),
            q(0.99),
            q(1.0),
            self.0.len()
        )
    }
}

fn bench(name: &str, p: &PatchState, frames: usize) {
    let mut collector = Collector::new();
    let handle = collector.handle();
    let mut e = PatchEngine::new(&handle, p, SR, 8).unwrap();
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    let blocks = frames / BLOCK;
    let mut callback = |e: &mut PatchEngine, swaps: Vec<_>| {
        let t = Instant::now();
        for g in swaps {
            e.receive_swap(g);
        }
        for _ in 0..blocks {
            e.process_block(&mut l, &mut r);
        }
        t.elapsed()
    };
    // Warm up: fill every delay line with signal (4 s).
    for _ in 0..(4.5 * SR) as usize / frames {
        callback(&mut e, vec![]);
    }
    let mut steady = Stats(Vec::new());
    for _ in 0..CALLBACKS {
        steady.0.push(callback(&mut e, vec![]));
    }
    let (mut swap, mut burst, mut between) = (Stats(vec![]), Stats(vec![]), Stats(vec![]));
    for i in 0..CALLBACKS {
        let n = match i % (SWAP_EVERY * 10) {
            0 => 3, // three graphs in one callback: one fades, the last one waits
            k if k % SWAP_EVERY == 0 => 1,
            _ => 0,
        };
        let graphs: Vec<_> = (0..n).map(|_| e.build_swap(&handle, p).unwrap()).collect();
        let d = callback(&mut e, graphs);
        match n {
            0 => between.0.push(d),
            1 => swap.0.push(d),
            _ => burst.0.push(d),
        }
        collector.collect();
    }
    let budget = frames as f64 / SR as f64 * 1e6;
    println!("{name}, {frames} frames ({budget:.0} µs budget)");
    println!("  steady            {}", steady.line());
    println!("  swap callbacks    {}", swap.line());
    println!("  3-graph bursts    {}", burst.line());
    println!("  between swaps     {}", between.line());
}

fn main() {
    let frames: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(256);
    bench(
        "no delay (patches/interlocking)",
        &load("interlocking"),
        frames,
    );
    bench("one delay (patches/echo)", &load("echo"), frames);
    bench("two delays", &two_delays(), frames);
}
