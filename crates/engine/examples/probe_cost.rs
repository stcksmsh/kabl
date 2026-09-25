//! D03 inspector cost, offline on this machine: the same patch and the same scripted input
//! rendered in 256-frame "callbacks" (four 64-sample blocks) with the tap off, on, and on while
//! the selection and the graph change quickly. Prints per-callback execution distributions.
//! Not an audio-device measurement: no arrival lateness or xruns exist here.
//!
//!     cargo run --release -p kabl-engine --example probe_cost -- patches/composition 60
//!
//! Arguments: patch folder, seconds of audio per run (default 60), runs per mode (default 3).

use std::time::Instant;

use basedrop::Collector;
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::{Command, PatchEngine};
use kabl_engine::probe::{ProbeReport, ProbeTarget};

const SR: f32 = 48000.0;
const FRAMES: usize = 256;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Off,
    On,
    /// Off, a graph swap every 20 callbacks: the baseline for `Busy`.
    OffSwaps,
    /// On, a new selection every 10 callbacks and a graph swap every 20.
    Busy,
}

fn run(patch: &kabl_core::PatchState, secs: f32, mode: Mode, target: ProbeTarget) -> Vec<u64> {
    let mut collector = Collector::new();
    let handle = collector.handle();
    let mut e = PatchEngine::new(&handle, patch, SR, 8).unwrap();
    let ids: Vec<(u64, &'static str)> = patch
        .modules
        .iter()
        .map(|(&id, m)| (id, kabl_modules::registry::info_for(&m.kind).unwrap().kind))
        .collect();
    if matches!(mode, Mode::On | Mode::Busy) {
        e.command(&Command::Inspect(Some(target)));
    }
    let (mut tx, mut rx) = rtrb::RingBuffer::<ProbeReport>::new(8);
    let callbacks = (secs * SR) as usize / FRAMES;
    let mut times = Vec::with_capacity(callbacks);
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    for n in 0..callbacks {
        // Control side (not timed): a graph for every 20th callback, chords every second.
        let swap =
            (mode == Mode::Busy && n % 20 == 0).then(|| e.build_swap(&handle, patch).unwrap());
        let t = Instant::now();
        if let Some(g) = swap {
            e.receive_swap(g);
        }
        if mode == Mode::Busy && n % 10 == 0 {
            let (id, kind) = ids[n / 10 % ids.len()];
            e.command(&Command::Inspect(Some(ProbeTarget {
                token: n as u64,
                module: id,
                port: 0,
                kind,
            })));
        }
        if n % 188 == 0 {
            for note in [48, 55, 60, 64] {
                e.key(KeyEvent::On { note, velocity: 90 });
            }
        } else if n % 188 == 150 {
            e.key(KeyEvent::AllOff);
        }
        for _ in 0..FRAMES / BLOCK {
            e.process_block(&mut l, &mut r);
        }
        if let Some(rep) = e.take_probe_report() {
            let _ = tx.push(rep);
        }
        times.push(t.elapsed().as_nanos() as u64);
        while rx.pop().is_ok() {}
        if n % 64 == 0 {
            collector.collect();
        }
    }
    times
}

fn pct(sorted: &[u64], p: f64) -> f64 {
    sorted[((sorted.len() - 1) as f64 * p).round() as usize] as f64 / 1e3
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = args.get(1).map_or("patches/composition", String::as_str);
    let secs: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(60.0);
    let runs: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(3);
    let patch = kabl_core::load(std::path::Path::new(dir))
        .unwrap()
        .state()
        .clone();
    // The tap on the module with the most instances (voices) and outputs: the dearest case.
    let (&id, m) = patch
        .modules
        .iter()
        .filter(|(_, m)| m.kind != "out" && m.kind != "macro")
        .max_by_key(|(_, m)| {
            let i = kabl_modules::registry::info_for(&m.kind).unwrap();
            (i.rate == kabl_modules::Rate::Voice, i.ports.len())
        })
        .unwrap();
    let kind = kabl_modules::registry::info_for(&m.kind).unwrap().kind;
    let target = ProbeTarget {
        token: 1,
        module: id,
        port: 0,
        kind,
    };
    println!(
        "patch {dir}, {} modules; tap on #{id} {kind} output 0; {secs} s per run, {runs} runs \
         per mode, interleaved; {SR} Hz, {FRAMES}-frame callbacks (budget {:.0} µs)",
        patch.modules.len(),
        FRAMES as f64 / SR as f64 * 1e6
    );
    println!(
        "memory: tap state {} B per graph, report {} B, report queue 8 × {} B",
        kabl_engine::probe::TAP_BYTES,
        std::mem::size_of::<ProbeReport>(),
        std::mem::size_of::<ProbeReport>()
    );
    let mut all: [(Mode, &str, Vec<u64>); 4] = [
        (Mode::Off, "off", Vec::new()),
        (Mode::On, "on", Vec::new()),
        (Mode::OffSwaps, "off+swaps", Vec::new()),
        (Mode::Busy, "on+select+swaps", Vec::new()),
    ];
    for _ in 0..runs {
        for (mode, _, v) in all.iter_mut() {
            v.extend(run(&patch, secs, *mode, target));
        }
    }
    println!("mode             callbacks   p50 µs   p90 µs   p99 µs  p99.9 µs   max µs  mean µs");
    for (_, name, v) in all.iter_mut() {
        v.sort_unstable();
        let mean = v.iter().sum::<u64>() as f64 / v.len() as f64 / 1e3;
        println!(
            "{name:<16} {:>9} {:>8.1} {:>8.1} {:>8.1} {:>9.1} {:>8.1} {:>8.1}",
            v.len(),
            pct(v, 0.5),
            pct(v, 0.9),
            pct(v, 0.99),
            pct(v, 0.999),
            *v.last().unwrap() as f64 / 1e3,
            mean
        );
    }
}
