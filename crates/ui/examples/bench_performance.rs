//! Audio-callback timing for `patches/performance` (docs/performance-batch/README.md): the
//! whole callback as `main.rs` runs it (swaps, a held MIDI lead note, engine blocks, the
//! recorder tap writing every frame to a WAV on its own thread), steady and with live graph
//! swaps (a parameter edit every 4th callback, three at once every 40th). Swap graphs are
//! compiled outside the timing, as the UI thread does.
//! `cargo run --release -p kabl-ui --example bench_performance [rate] [frames]`
use basedrop::Collector;
use kabl_core::PatchState;
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::PatchEngine;
use std::time::{Duration, Instant};

const CALLBACKS: usize = 6000;
const SWAP_EVERY: usize = 4;

fn load(name: &str) -> PatchState {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../patches")
        .join(name);
    kabl_core::load(&dir).unwrap().state().clone()
}

struct Stats(Vec<Duration>);

impl Stats {
    fn line(&mut self, budget: f64) -> String {
        self.0.sort();
        let q = |f: f64| self.0[((self.0.len() - 1) as f64 * f) as usize].as_secs_f64() * 1e6;
        format!(
            "median {:>6.0}  p99 {:>6.0}  p99.9 {:>6.0}  max {:>6.0} µs  ({:.0} % of budget at max, n={})",
            q(0.5),
            q(0.99),
            q(0.999),
            q(1.0),
            q(1.0) / budget * 100.0,
            self.0.len()
        )
    }
}

fn main() {
    let args: Vec<usize> = std::env::args()
        .skip(1)
        .filter_map(|s| s.parse().ok())
        .collect();
    let sr = *args.first().unwrap_or(&48000) as f32;
    let frames = *args.get(1).unwrap_or(&256);
    let p = load("performance");
    let mut collector = Collector::new();
    let handle = collector.handle();
    let mut e = PatchEngine::new(&handle, &p, sr, 8).unwrap();
    let dir = std::env::temp_dir().join("kabl-bench-take");
    let (mut rec, mut tap) = kabl_ui::record::pair(sr as u32);
    rec.start_at(dir.join("bench.wav")).unwrap();
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    let mut ring = std::collections::VecDeque::with_capacity(frames * 4);
    let mut callback = |e: &mut PatchEngine, swaps: Vec<_>| {
        let t = Instant::now();
        for g in swaps {
            e.receive_swap(g);
        }
        while ring.len() < frames {
            e.process_block(&mut l, &mut r);
            ring.extend(l.iter().copied().zip(r.iter().copied()));
        }
        let on = tap.begin(frames);
        for _ in 0..frames {
            let (a, b) = ring.pop_front().unwrap();
            if on {
                tap.frame(a, b);
            }
        }
        t.elapsed()
    };
    e.note_on(0, 7.0, 0.9);
    // Warm up: fill the delay and reverb with signal.
    for _ in 0..(5.0 * sr) as usize / frames {
        callback(&mut e, vec![]);
    }
    let mut steady = Stats(Vec::new());
    for _ in 0..CALLBACKS {
        steady.0.push(callback(&mut e, vec![]));
    }
    let edit = |k: usize| {
        let mut q = p.clone();
        q.modules
            .get_mut(&6)
            .unwrap()
            .params
            .insert("cutoff_hz".into(), 300.0 + (k % 50) as f32 * 10.0);
        q
    };
    let (mut swap, mut burst) = (Stats(vec![]), Stats(vec![]));
    for i in 0..CALLBACKS {
        let n = match i % (SWAP_EVERY * 10) {
            0 => 3,
            k if k % SWAP_EVERY == 0 => 1,
            _ => 0,
        };
        let graphs: Vec<_> = (0..n)
            .map(|k| e.build_swap(&handle, &edit(i + k)).unwrap())
            .collect();
        let d = callback(&mut e, graphs);
        match n {
            0 => steady.0.push(d),
            1 => swap.0.push(d),
            _ => burst.0.push(d),
        }
        collector.collect();
    }
    let outcome = rec.stop();
    let budget = frames as f64 / sr as f64 * 1e6;
    println!("patches/performance, {sr} Hz, {frames} frames ({budget:.0} µs budget), recording on");
    println!("  no swap           {}", steady.line(budget));
    println!("  swap callbacks    {}", swap.line(budget));
    println!("  3-graph bursts    {}", burst.line(budget));
    println!(
        "  take: {:?}",
        outcome.map(|o| kabl_ui::record::describe(&o))
    );
    let _ = std::fs::remove_dir_all(dir);
}
