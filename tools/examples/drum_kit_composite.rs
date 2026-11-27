//! The factory composite `kabl.drum-kit` v1 (catalogue C4, roadmap CB2): a drum grid playing
//! kick, snare, clap, hats and tom into a submix.
//!
//! The inner graph is the kit of the factory pieces (`add_kit` in `presets_piece.rs`, the
//! catalogue's four-stage variant): the kick into its own mixer, clap, hats and snare into a bus
//! mixer and through a ladder filter (open at 20 kHz), and the three into the kit mixer with the
//! tom. Defaults follow Laundromat's kit. See `docs/composites/DRUM-KIT.md`.
//!
//! `cargo run -p kabl-tools --example drum_kit_composite -- write composites` writes
//! `composites/drum-kit/v1.json`. Running it again writes the same bytes: the package holds the
//! patch state only, never the log's wall-clock timestamps.
//!
//! `cargo run --release -p kabl-tools --example drum_kit_composite -- wav DIR` renders eight
//! seconds of a clock at 120 BPM into Drum Kit into the output, offline, to
//! `DIR/drum-kit.wav`, and prints peak and RMS.
//!
//! The captures are scenes of `chrome_capture`: `drum-kit-rack`, `drum-kit-face`,
//! `drum-kit-design`.
//!
//! The test `composites_drum_kit` includes this file to compare `bytes()` with the committed
//! file, so only the generator is compiled there.
use kabl_core::panel::{Kind, Panel, Placement, Source};
use kabl_core::{Definition, ModuleId, Vec2};
use kabl_modules::builtins::{cell_index, make_code, with_code, GRID_ROW_NAMES};
use kabl_ui::{composites, PatchEditor};
use std::collections::BTreeSet;

#[path = "composite_gen/mod.rs"]
mod composite_gen;
use composite_gen::{expose, jack, param};

/// Grid rows in output order: kick, snare, clap, closed hat, open hat, tom. Steps as in
/// `GROOVE.md`: `.` off, `o` `x` `X` rising velocity.
const ROWS: [&str; 6] = [
    "X...X...X...X...",
    "....X.......X...",
    "....X.......X..o",
    "x.x.x.x.x.x.x.x.",
    "..o...o...o...o.",
    "..............x.",
];

/// The step params of one grid row, pattern A.
fn grid_row(e: &mut PatchEditor, grid: ModuleId, row: usize, text: &str) {
    let mut chunks = [0.0f32; 4];
    for (step, c) in text.chars().enumerate() {
        let vel = match c {
            'o' => 1,
            'x' => 2,
            'X' => 3,
            _ => continue,
        };
        chunks[step / 4] = with_code(chunks[step / 4], step % 4, make_code(vel, 0));
    }
    let params = kabl_modules::registry::info_for("drum.grid")
        .expect("drum.grid")
        .params;
    for (c, v) in chunks.into_iter().enumerate() {
        e.set_param(grid, params[cell_index(0, row, c)].name, v);
    }
}

fn placement(kind: Kind, x: f32, y: f32, width: f32, height: f32) -> Placement {
    Placement {
        kind,
        x,
        y,
        width,
        height,
        source: None,
        label: None,
        icons: Vec::new(),
    }
}

fn package() -> composites::Package {
    let mut e = PatchEditor::new();
    let at = |x: f32, y: f32| Vec2 { x, y };
    let grid = e.add_module("drum.grid", at(24.0, 20.0));
    for (row, text) in ROWS.iter().enumerate() {
        grid_row(&mut e, grid, row, text);
    }
    // Laundromat's kit, with the snare in the mix (Laundromat mutes it at 0).
    let voice = |e: &mut PatchEditor, kind: &str, x: f32, params: &[(&str, f32)]| {
        let id = e.add_module(kind, at(x, 420.0));
        for &(name, v) in params {
            e.set_param(id, name, v);
        }
        id
    };
    let kick = voice(
        &mut e,
        "drum.kick",
        24.0,
        &[
            ("level_db", -8.0),
            ("decay_ms", 250.0),
            ("drive_db", 8.0),
            ("tune_hz", 55.0),
        ],
    );
    let snare = voice(
        &mut e,
        "drum.snare",
        214.0,
        &[("level_db", -8.0), ("decay_ms", 200.0)],
    );
    let clap = voice(&mut e, "drum.clap", 404.0, &[("level_db", -10.0)]);
    let hat = voice(&mut e, "drum.hat", 594.0, &[("level_db", -16.0)]);
    let tom = voice(
        &mut e,
        "drum.tom",
        784.0,
        &[("level_db", -10.0), ("tune_hz", 120.0), ("decay_ms", 300.0)],
    );
    for (row, (to, port)) in [
        (kick, "gate"),
        (snare, "gate"),
        (clap, "gate"),
        (hat, "closed"),
        (hat, "open"),
        (tom, "gate"),
    ]
    .into_iter()
    .enumerate()
    {
        let from = GRID_ROW_NAMES[row];
        e.connect(jack(grid, from), jack(to, port));
    }
    let mixer = |e: &mut PatchEditor, x: f32, levels: &[f32]| {
        let id = e.add_module("mixer", at(x, 820.0));
        for (i, &v) in levels.iter().enumerate() {
            e.set_param(id, &format!("level{}", i + 1), v);
        }
        id
    };
    let beat = mixer(&mut e, 24.0, &[0.7]);
    // in1 clap, in2 hats, in3 snare.
    let top = mixer(&mut e, 214.0, &[0.5, 0.3, 0.5]);
    let lad = e.add_module("filter.ladder", at(404.0, 820.0));
    e.set_param(lad, "cutoff_hz", 20000.0);
    e.set_param(lad, "resonance", 0.2);
    let out = mixer(&mut e, 594.0, &[1.0, 0.3, 1.0]);
    e.connect(jack(kick, "out"), jack(beat, "in1"));
    e.connect(jack(clap, "out"), jack(top, "in1"));
    e.connect(jack(hat, "out"), jack(top, "in2"));
    e.connect(jack(snare, "out"), jack(top, "in3"));
    e.connect(jack(top, "out"), jack(lad, "in"));
    e.connect(jack(beat, "out"), jack(out, "in1"));
    e.connect(jack(tom, "out"), jack(out, "in2"));
    e.connect(jack(lad, "out"), jack(out, "in3"));
    // Interface keys: ports 1..=10, then controls 11..=20, in the order listed.
    let ports = [
        expose("Clock", jack(grid, "clock")),
        expose("Reset", jack(grid, "reset")),
        expose("Out", jack(out, "out")),
        expose("Kick", jack(kick, "out")),
        expose("Hats/Bus", jack(lad, "out")),
        // The levels the pieces' and grooves' macros drive: each voice's share of the mix.
        expose("Kick mod", param(out, "level1")),
        expose("Clap mod", param(top, "level1")),
        expose("Hat mod", param(top, "level2")),
        expose("Tom mod", param(out, "level2")),
        expose("Tone mod", param(lad, "cutoff_hz")),
    ];
    let controls = [
        expose("Kick Tune", param(kick, "tune_hz")),
        expose("Kick Decay", param(kick, "decay_ms")),
        expose("Kick Drive", param(kick, "drive_db")),
        expose("Snare Decay", param(snare, "decay_ms")),
        expose("Snare Level", param(snare, "level_db")),
        expose("Clap Level", param(clap, "level_db")),
        expose("Hat Level", param(hat, "level_db")),
        expose("Tom Tune", param(tom, "tune_hz")),
        expose("Tom Level", param(tom, "level_db")),
        expose("Bus Tone", param(lad, "cutoff_hz")),
    ];
    // One column per voice: its display over its knobs, then Bus Tone; the jacks along the
    // bottom. Clap and hats share a column under the hat display.
    let mut panel = Panel {
        width: 840.0,
        ..Panel::default()
    };
    let columns = [
        (&[11, 12, 13][..], Some((kick, "Kick"))),
        (&[14, 15], Some((snare, "Snare"))),
        (&[16, 17], Some((hat, "Clap · Hats"))),
        (&[18, 19], Some((tom, "Tom"))),
        (&[20], None),
    ];
    let mut displays = Vec::new();
    let mut x = 8.0;
    for (keys, display) in columns {
        let left = x;
        for &key in keys {
            // Wide enough to print "Snare Decay" whole.
            let width = if key == 14 { 74.0 } else { 70.0 };
            panel
                .placements
                .insert(key, placement(Kind::Knob, x, 132.0, width, 110.0));
            x += width + 6.0;
        }
        if let Some(d) = display {
            displays.push((d, left, x - 6.0 - left));
        }
        x += 14.0;
    }
    let mut x = 8.0;
    for key in 1..=10u64 {
        // Wide enough to print "OUT Hats/Bus" whole.
        let width = if key == 5 { 84.0 } else { 78.0 };
        panel
            .placements
            .insert(key, placement(Kind::Jack, x, 250.0, width, 58.0));
        x += width + 2.0;
    }
    let mut log = e.log().clone();
    let id = composites::encapsulate_with(
        &mut log,
        "Drum Kit",
        Definition {
            id: "kabl.drum-kit".into(),
            version: 1,
        },
        BTreeSet::from([grid, kick, snare, clap, hat, tom, beat, top, lad, out]),
        &ports,
        &controls,
        Some(panel),
    )
    .expect("Drum Kit wraps");
    // Displays are composite-level elements: their keys come after the interface's.
    let mut e = PatchEditor::from_log(log);
    let mut c = e.state().composites[&id].clone();
    let panel = c.panel.as_mut().expect("the face");
    for ((leaf, label), x, width) in displays {
        let mut p = placement(Kind::Display, x, 64.0, width.max(120.0), 60.0);
        p.source = Some(Source::Module {
            id: leaf,
            action: None,
        });
        p.label = Some(label.into());
        panel.placements.insert(c.next_interface, p);
        c.next_interface += 1;
    }
    composites::set(&mut e, id, c).expect("Drum Kit face");
    composites::package(e.state(), id).expect("Drum Kit packages")
}

/// `composites/drum-kit/v1.json` exactly as committed.
pub(crate) fn bytes() -> Vec<u8> {
    serde_json::to_vec_pretty(&package()).expect("serialise")
}

#[cfg(not(test))]
mod tool {
    use super::*;
    use std::path::Path;

    fn write(root: &Path) {
        let dir = root.join("drum-kit");
        std::fs::create_dir_all(&dir).expect("bank folder");
        let bytes = bytes();
        composites::decode(&bytes).expect("the package decodes");
        let path = dir.join("v1.json");
        std::fs::write(&path, bytes).expect("write");
        println!("wrote {}", path.display());
    }

    pub(super) fn main() {
        let a: Vec<String> = std::env::args().skip(1).collect();
        let dir = std::path::PathBuf::from(a.get(1).cloned().unwrap_or_else(|| ".".into()));
        match a.first().map(String::as_str) {
            Some("write") => write(&dir),
            Some("wav") => composite_gen::wav(&dir, "drum-kit"),
            _ => eprintln!("usage: drum_kit_composite write|wav DIR"),
        }
    }
}

#[cfg(not(test))]
fn main() {
    tool::main();
}
