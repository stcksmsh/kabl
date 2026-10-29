//! Builds, and renders offline, the Grid Cues demo: one drum grid whose four cues pick its
//! pattern, so a self-playing piece goes Intro (hats only) to Groove to Fill to Break.
//!
//!     cargo run --release -p kabl-ui --example grid_cues_demo -- write patches/grid-cues
//!     cargo run --release -p kabl-ui --example grid_cues_demo -- render patches/grid-cues out.wav
//!
//! The render launches every cue on the bar before its section, the way a player would, and
//! writes a 32-bit float stereo WAV (`docs/grid-cues/scripts/clips.py` encodes it).

use std::path::Path;

use basedrop::Collector;
use kabl_core::{ModuleId, PortRef, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::{Command, PatchEngine, Timing};
use kabl_modules::builtins::{cell_index, make_code, with_code, VELOCITY};
use kabl_ui::perform::{BTN_PREFIX, CUE_PADS, PIN_PREFIX, TRANSPORT};
use kabl_ui::{cues, PatchEditor};

const SR: f32 = 48000.0;
const BPM: f32 = 124.0;
const BAR_S: f32 = 4.0 * 60.0 / BPM;

fn jack(id: ModuleId, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

fn at(x: f32, y: f32) -> Vec2 {
    Vec2 { x, y }
}

/// Cue name, grid pattern (0 = A), bar it lands on. Pattern D holds the hats-only intro.
const CUES: [(&str, usize, u32); 4] = [
    ("Intro", 3, 0),
    ("Groove", 0, 2),
    ("Fill", 1, 6),
    ("Break", 2, 7),
];
const BARS: u32 = 10;

/// Writes `(step, velocity)` pairs into one row of one pattern.
fn row(e: &mut PatchEditor, grid: ModuleId, pattern: usize, row: usize, steps: &[(usize, f32)]) {
    let mut chunks = [0.0f32; 4];
    for &(step, vel) in steps {
        let level = (0..4)
            .min_by(|&a, &b| {
                (VELOCITY[a] - vel)
                    .abs()
                    .total_cmp(&(VELOCITY[b] - vel).abs())
            })
            .unwrap();
        chunks[step / 4] = with_code(chunks[step / 4], step % 4, make_code(level, 0));
    }
    let params = kabl_modules::registry::info_for("drum.grid")
        .unwrap()
        .params;
    for (c, v) in chunks.into_iter().enumerate() {
        e.set_param(grid, params[cell_index(pattern, row, c)].name, v);
    }
}

/// Every step in `from` of both halves of the bar.
fn twice(a: &[(usize, f32)]) -> Vec<(usize, f32)> {
    a.iter()
        .copied()
        .chain(a.iter().map(|&(s, v)| (s + 8, v)))
        .collect()
}

fn build() -> (PatchEditor, ModuleId) {
    let mut e = PatchEditor::new();
    let clock = e.add_module("clock", at(20.0, 20.0));
    let grid = e.add_module("drum.grid", at(20.0, 300.0));
    let kick = e.add_module("drum.kick", at(400.0, 20.0));
    let snare = e.add_module("drum.snare", at(400.0, 300.0));
    let clap = e.add_module("drum.clap", at(400.0, 560.0));
    let hat = e.add_module("drum.hat", at(700.0, 20.0));
    let tom = e.add_module("drum.tom", at(700.0, 300.0));
    let kit = e.add_module("mixer", at(1000.0, 20.0));
    let fill = e.add_module("mixer", at(1000.0, 300.0));
    let out = e.add_module("out", at(1000.0, 560.0));
    let cue = e.add_module("cues", at(20.0, 560.0));

    e.set_param(clock, "bpm", BPM);
    e.set_param(snare, "level_db", -6.0);
    e.set_param(clap, "level_db", -6.0);
    e.set_param(hat, "level_db", -6.0);
    e.set_param(tom, "level_db", -5.0);
    e.set_param(tom, "tune_hz", 110.0);
    for (k, v) in [0.9, 0.55, 0.5, 0.55].into_iter().enumerate() {
        e.set_param(kit, &format!("level{}", k + 1), v);
    }
    e.set_param(fill, "level1", 0.42);
    e.set_param(fill, "level2", 0.3);

    let kick_steps = [(0, 1.0), (4, 0.95), (8, 1.0), (12, 0.95)];
    let back = [(4, 1.0), (12, 1.0)];
    let closed = twice(&[(0, 0.7), (1, 0.35), (3, 0.6), (4, 0.7), (5, 0.35), (7, 0.6)]);
    let open = twice(&[(2, 0.85), (6, 0.85)]);
    // A: the groove.
    row(&mut e, grid, 0, 0, &kick_steps);
    row(&mut e, grid, 0, 1, &back);
    row(&mut e, grid, 0, 2, &back);
    row(&mut e, grid, 0, 3, &closed);
    row(&mut e, grid, 0, 4, &open);
    // B: the fill. Groove for the first half, then a snare roll that builds and toms under it.
    let roll: Vec<(usize, f32)> = (8..16).map(|s| (s, 0.3 + 0.1 * (s - 8) as f32)).collect();
    row(&mut e, grid, 1, 0, &[(0, 1.0), (4, 0.95), (8, 1.0)]);
    row(&mut e, grid, 1, 1, &[&[(4, 1.0)][..], &roll].concat());
    row(&mut e, grid, 1, 3, &closed[..closed.len() / 2]);
    row(&mut e, grid, 1, 4, &[(2, 0.85), (6, 0.85)]);
    row(
        &mut e,
        grid,
        1,
        5,
        &[(10, 0.7), (12, 0.8), (14, 0.9), (15, 1.0)],
    );
    // C: the break. A kick on the one, a clap on the "and" of three, toms and one open hat.
    row(&mut e, grid, 2, 0, &[(0, 1.0)]);
    row(&mut e, grid, 2, 2, &[(14, 0.8)]);
    row(&mut e, grid, 2, 4, &[(8, 0.7)]);
    row(&mut e, grid, 2, 5, &[(4, 0.6), (6, 0.7)]);
    // D: the intro, hats only.
    row(
        &mut e,
        grid,
        3,
        3,
        &twice(&[(0, 0.7), (2, 0.5), (4, 0.7), (6, 0.5)]),
    );

    e.connect(jack(clock, "gate"), jack(grid, "clock"));
    e.connect(jack(clock, "reset"), jack(grid, "reset"));
    for (from, to, port) in [
        ("kick", kick, "gate"),
        ("snare", snare, "gate"),
        ("clap", clap, "gate"),
        ("chat", hat, "closed"),
        ("ohat", hat, "open"),
        ("tom", tom, "gate"),
    ] {
        e.connect(jack(grid, from), jack(to, port));
    }
    for (src, port) in [(kick, "in1"), (snare, "in2"), (clap, "in3"), (hat, "in4")] {
        e.connect(jack(src, "out"), jack(kit, port));
    }
    e.connect(jack(kit, "out"), jack(fill, "in1"));
    e.connect(jack(tom, "out"), jack(fill, "in2"));
    e.connect(jack(fill, "out"), jack(out, "left"));
    e.connect(jack(fill, "out"), jack(out, "right"));

    for &(name, pattern, _) in &CUES {
        let n = cues::add(&mut e, cue, &|_| 0).expect("cue");
        cues::rename(&mut e, cue, n, name);
        cues::set_target(&mut e, cue, n, grid, Some(pattern));
        if n == 1 {
            // The opener lands at once; the others wait for the bar line.
            cues::set_timing(&mut e, cue, n, Timing::Now);
        }
    }
    let mut changes = vec![
        (clock, format!("{PIN_PREFIX}{TRANSPORT}"), Some(0.0)),
        (cue, format!("{PIN_PREFIX}{CUE_PADS}"), Some(1.0)),
    ];
    for (n, cc) in (1..=CUES.len()).zip(40..) {
        changes.push((cue, format!("{BTN_PREFIX}cue.{n}"), Some(cc as f32)));
    }
    e.set_presentation(&changes);
    (e, cue)
}

const SOUND_TOML: &str = r#"name = "Grid Cues"
category = "Piece"
tags = ["sequenced", "cues", "drums", "drum grid", "124bpm"]
description = "A drum grid played by cues. Four cues (Intro, Groove, Fill, Break) each pick one of the grid's patterns, and the pattern changes on the next bar, so a piece moves from hats to a groove to a fill to a breakdown without a hand on the grid."
keys = false
sequence = true
"#;

fn write_wav(path: &Path, left: &[f32], right: &[f32]) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR as u32,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for (l, r) in left.iter().zip(right) {
        w.write_sample(*l).unwrap();
        w.write_sample(*r).unwrap();
    }
    w.finalize().unwrap();
}

fn render(state: &kabl_core::PatchState, cue: ModuleId) -> (Vec<f32>, Vec<f32>) {
    let collector = Collector::new();
    let mut engine = PatchEngine::new(&collector.handle(), state, SR, 4).expect("compiles");
    let blocks = ((BARS as f32 * BAR_S * SR) as usize).div_ceil(BLOCK);
    let (mut left, mut right) = (Vec::new(), Vec::new());
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    let mut next = 0;
    for b in 0..blocks {
        let t = (b * BLOCK) as f32 / SR / BAR_S;
        if let Some(&(_, _, bar)) = CUES.get(next) {
            // Half a bar before the section, so NextBar has a line to land on.
            if t >= bar as f32 - 0.5 || (bar == 0 && b == 0) {
                let mut cmds = Vec::new();
                cues::launch(&mut cmds, state, cue, next + 1).expect("launch");
                for mut c in cmds {
                    if let Command::Launch(l) = &mut c {
                        if bar == 0 {
                            l.timing = Timing::Now;
                        }
                    }
                    engine.command(&c);
                }
                next += 1;
            }
        }
        engine.process_block(&mut l, &mut r);
        left.extend_from_slice(&l);
        right.extend_from_slice(&r);
    }
    (left, right)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (e, cue) = build();
    let e = e.migrated();
    match args.get(1).map(String::as_str) {
        Some("write") => {
            let dir = Path::new(&args[2]);
            kabl_core::save(dir, e.log()).expect("save");
            std::fs::write(dir.join("sound.toml"), SOUND_TOML).expect("sound.toml");
            println!("wrote {} (cues module {cue})", dir.display());
        }
        Some("render") => {
            let log = kabl_core::load(Path::new(&args[2])).expect("load patch");
            let (l, r) = render(log.state(), cue);
            let (l2, _) = render(log.state(), cue);
            assert_eq!(l, l2, "render is deterministic");
            write_wav(Path::new(&args[3]), &l, &r);
            let peak = l.iter().chain(&r).fold(0.0f32, |m, s| m.max(s.abs()));
            println!("peak {peak:.3}");
            for bar in 0..BARS as usize {
                let w = &l[(bar as f32 * BAR_S * SR) as usize..][..(BAR_S * SR) as usize];
                let rms = (w.iter().map(|s| s * s).sum::<f32>() / w.len() as f32).sqrt();
                println!("bar {bar:2}: rms {rms:.3}");
            }
        }
        _ => eprintln!("usage: grid_cues_demo write <patch dir> | render <patch dir> <wav>"),
    }
}
