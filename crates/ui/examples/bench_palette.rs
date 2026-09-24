//! Audio-callback timing for the sound palette (docs/sound-palette-batch/README.md): the whole
//! callback as `main.rs` runs it (swaps, key events, engine blocks, the recorder tap writing
//! every frame to a WAV on its own thread).
//!
//! Scenes:
//! - `piece dense`: `patches/sound-palette` on its Lift banks with all four keyboard layers up
//!   and a 6-note chord held (every `midi.in` hears every key: 6 voices of breath, strings and
//!   pad, the mono lead), chorus + drive + delay + two plates.
//! - `strings 8×unison`: `patches/palette/strings` with both oscillators at 4-voice unison and
//!   an 8-note chord (every voice, the most unison a realistic patch reaches).
//! - `lead fx`: `patches/palette/lead` (sync, ladder, drive, delay, plate), a held note.
//! Each steady, then with controller traffic and live edits: a macro edit (a graph swap)
//! every 4th callback, three at once every 40th, and a key event every callback (the chord
//! re-voiced note by note). Swap graphs are compiled outside the timing, as the UI does.
//! `cargo run --release -p kabl-ui --example bench_palette [rate] [frames]`
use basedrop::Collector;
use kabl_core::PatchState;
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::PatchEngine;
use std::time::{Duration, Instant};

const CALLBACKS: usize = 4000;
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

fn scene(name: &str, p: PatchState, chord: &[u8], macro_id: u64, sr: f32, frames: usize) {
    let mut collector = Collector::new();
    let handle = collector.handle();
    let mut e = PatchEngine::new(&handle, &p, sr, 8).unwrap();
    let dir = std::env::temp_dir().join(format!("kabl-bench-palette-{}", std::process::id()));
    let (mut rec, mut tap) = kabl_ui::record::pair(sr as u32);
    rec.start_at(dir.join("bench.wav")).unwrap();
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    let mut ring = std::collections::VecDeque::with_capacity(frames * 4);
    let mut callback = |e: &mut PatchEngine, swaps: Vec<_>, key: Option<KeyEvent>| {
        let t = Instant::now();
        for g in swaps {
            e.receive_swap(g);
        }
        if let Some(k) = key {
            e.key(k);
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
    for &n in chord {
        callback(
            &mut e,
            vec![],
            Some(KeyEvent::On {
                note: n,
                velocity: 100,
            }),
        );
    }
    // Warm up: attacks done, effects full.
    for _ in 0..(4.0 * sr) as usize / frames {
        callback(&mut e, vec![], None);
    }
    let mut steady = Stats(Vec::new());
    for _ in 0..CALLBACKS {
        steady.0.push(callback(&mut e, vec![], None));
    }
    let edit = |k: usize| {
        let mut q = p.clone();
        q.modules
            .get_mut(&macro_id)
            .unwrap()
            .params
            .insert("m1".into(), (k % 50) as f32 / 50.0);
        q
    };
    // Key events alone (no graphs compiled in between), then with the edits.
    let mut keys = Stats(vec![]);
    for i in 0..CALLBACKS {
        let note = chord[(i / 2) % chord.len()];
        let key = if i % 2 == 0 {
            KeyEvent::Off { note }
        } else {
            KeyEvent::On { note, velocity: 90 }
        };
        keys.0.push(callback(&mut e, vec![], Some(key)));
    }
    let (mut busy, mut swap, mut burst) = (Stats(vec![]), Stats(vec![]), Stats(vec![]));
    for i in 0..CALLBACKS {
        let n = match i % (SWAP_EVERY * 10) {
            0 => 3,
            k if k % SWAP_EVERY == 0 => 1,
            _ => 0,
        };
        let graphs: Vec<_> = (0..n)
            .map(|k| e.build_swap(&handle, &edit(i + k)).unwrap())
            .collect();
        // Re-voice the chord one key at a time: off on even callbacks, on again on odd.
        let note = chord[(i / 2) % chord.len()];
        let key = if i % 2 == 0 {
            KeyEvent::Off { note }
        } else {
            KeyEvent::On { note, velocity: 90 }
        };
        let d = callback(&mut e, graphs, Some(key));
        match n {
            0 => busy.0.push(d),
            1 => swap.0.push(d),
            _ => burst.0.push(d),
        }
        collector.collect();
    }
    let outcome = rec.stop();
    let budget = frames as f64 / sr as f64 * 1e6;
    println!("{name}, {sr} Hz, {frames} frames ({budget:.0} µs budget), recording on");
    println!("  steady            {}", steady.line(budget));
    println!("  key every cb      {}", keys.line(budget));
    println!("  keys + edits: no-swap cb {}", busy.line(budget));
    println!("  + swap            {}", swap.line(budget));
    println!("  + 3-graph burst   {}", burst.line(budget));
    println!(
        "  take: {:?}",
        outcome.map(|o| kabl_ui::record::describe(&o))
    );
    let _ = std::fs::remove_dir_all(dir);
}

fn main() {
    let args: Vec<usize> = std::env::args()
        .skip(1)
        .filter_map(|s| s.parse().ok())
        .collect();
    let sr = *args.first().unwrap_or(&48000) as f32;
    let frames = *args.get(1).unwrap_or(&256);

    let mut piece = load("sound-palette");
    for m in piece.modules.values_mut() {
        match m.kind.as_str() {
            "seq" => {
                m.params.insert("bank".into(), 1.0);
            }
            "mixer" if m.params.get("level4") == Some(&0.0) => {
                for lvl in ["level1", "level2", "level3", "level4"] {
                    m.params.insert(lvl.into(), 0.8);
                }
            }
            _ => {}
        }
    }
    scene(
        "piece dense (6-note chord on 4 layers)",
        piece,
        &[40, 52, 59, 62, 66, 71],
        3,
        sr,
        frames,
    );

    let mut strings = load("palette/strings");
    for m in strings.modules.values_mut() {
        if m.kind == "osc.va" {
            m.params.insert("unison".into(), 4.0);
        }
    }
    scene(
        "strings 8 voices x 2 osc x 4-voice unison",
        strings,
        &[40, 47, 52, 55, 59, 64, 67, 71],
        15,
        sr,
        frames,
    );
    scene("lead fx", load("palette/lead"), &[64], 1, sr, frames);
}
