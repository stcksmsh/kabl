//! The factory composite `kabl.glue-bus` v1 (catalogue C5, roadmap CB4): the master colour and
//! dynamics stage of the grooves, drive into a ladder tone filter into a trim into a
//! compressor into a second compressor the signal on Duck In ducks through its `sc` input.
//!
//! One definition covers both catalogue variants: with Duck In unpatched the second compressor
//! passes the music sample for sample, so the single-compressor patches are this card with
//! Duck In left open. Duck In feeds `sc` through a unity `gain`: a compressor whose `sc` is
//! unpatched falls back to its own detector and would compress the music a second time, while
//! the gain's output is always a signal, silent when nothing is patched to it. Defaults follow
//! Corner Store's bus. See `docs/composites/GLUE-BUS.md`.
//!
//! `cargo run -p kabl-tools --example glue_bus_composite -- write composites` writes
//! `composites/glue-bus/v1.json`. Running it again writes the same bytes: the package holds the
//! patch state only, never the log's wall-clock timestamps.
//!
//! `cargo run --release -p kabl-tools --example glue_bus_composite -- wav DIR` renders eight
//! seconds of a clock at 120 BPM into Drum Kit into Glue Bus into the output, offline: to
//! `DIR/glue-bus.wav` with Drum Kit's Kick on Duck In, and to `DIR/glue-bus-no-duck.wav`
//! without, beside the dry loop in `DIR/drum-kit.wav`. It prints peak and RMS of each.
//!
//! The captures are scenes of `chrome_capture`: `glue-bus-rack`, `glue-bus-face`,
//! `glue-bus-design`.
//!
//! The test `composites_glue_bus` includes this file to compare `bytes()` with the committed
//! file, so only the generator is compiled there.
use kabl_core::panel::{Kind, Panel, Placement, Source};
use kabl_core::{Definition, Vec2};
use kabl_ui::{composites, PatchEditor};
use std::collections::BTreeSet;

#[path = "composite_gen/mod.rs"]
mod composite_gen;
use composite_gen::{expose, jack, param};

/// A face element with no source, label or icons.
fn element(kind: Kind, x: f32, y: f32, width: f32, height: f32) -> Placement {
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
    let mut stage = |kind: &str, x: f32, y: f32, params: &[(&str, f32)]| {
        let id = e.add_module(kind, Vec2 { x, y });
        for &(name, v) in params {
            e.set_param(id, name, v);
        }
        id
    };
    // Corner Store's bus (`groove` in `presets_groove.rs`), with the duck's ratio at 4:1: the
    // grooves store 1:1 there and let their Pump macro raise it.
    let drive = stage("drive", 24.0, 20.0, &[("drive_db", 4.0), ("trim_db", -2.0)]);
    let tone = stage(
        "filter.ladder",
        214.0,
        20.0,
        &[("cutoff_hz", 1600.0), ("resonance", 0.2), ("drive_db", 2.0)],
    );
    let trim = stage("gain", 404.0, 20.0, &[("gain_db", -2.0)]);
    let comp = stage(
        "compressor",
        594.0,
        20.0,
        &[
            ("threshold_db", -24.0),
            ("ratio", 4.0),
            ("attack_ms", 4.0),
            ("release_ms", 110.0),
            ("knee_db", 4.0),
            ("makeup_db", 4.0),
        ],
    );
    let feed = stage("gain", 594.0, 420.0, &[("gain_db", 0.0)]);
    let duck = stage(
        "compressor",
        784.0,
        20.0,
        &[
            ("threshold_db", -34.0),
            ("ratio", 4.0),
            ("attack_ms", 1.0),
            ("release_ms", 180.0),
            ("knee_db", 3.0),
            ("makeup_db", 0.0),
        ],
    );
    e.connect(jack(drive, "out"), jack(tone, "in"));
    e.connect(jack(tone, "out"), jack(trim, "in"));
    e.connect(jack(trim, "out"), jack(comp, "in"));
    e.connect(jack(comp, "out"), jack(duck, "in"));
    e.connect(jack(feed, "out"), jack(duck, "sc"));
    // Interface keys: ports 1..=3, then controls 4..=12, in the order listed.
    let ports = [
        expose("In", jack(drive, "in")),
        expose("Duck In", jack(feed, "in")),
        expose("Out", jack(duck, "out")),
    ];
    let controls = [
        expose("Drive", param(drive, "drive_db")),
        expose("Tone", param(tone, "cutoff_hz")),
        expose("Resonance", param(tone, "resonance")),
        expose("Trim", param(trim, "gain_db")),
        expose("Threshold", param(comp, "threshold_db")),
        expose("Ratio", param(comp, "ratio")),
        expose("Attack", param(comp, "attack_ms")),
        expose("Release", param(comp, "release_ms")),
        expose("Duck", param(duck, "threshold_db")),
    ];
    // The jacks along the top, so a source above the card patches down into In and Duck In,
    // clear of the card's name, and Out leaves at the right. In and Duck In are as far apart as
    // Drum Kit's Out and Kick, so the cables from a Drum Kit above run parallel. The displays in
    // the middle band over their knobs, the knobs in one row along the bottom: knobs, not
    // sliders, because a face slider follows the player's orientation setting. Resonance is the
    // catalogue's advanced control, off the face.
    let mut panel = Panel {
        width: 720.0,
        ..Panel::default()
    };
    for (key, x) in [(1, 164.0), (2, 242.0), (3, 634.0)] {
        panel
            .placements
            .insert(key, element(Kind::Jack, x, 64.0, 78.0, 58.0));
    }
    // Drive Tone | Trim | Threshold Ratio Attack Release | Duck.
    let mut x = 8.0;
    for (key, gap) in [
        (4, 2.0),
        (5, 18.0),
        (7, 18.0),
        (8, 2.0),
        (9, 2.0),
        (10, 2.0),
        (11, 18.0),
        (12, 0.0),
    ] {
        panel
            .placements
            .insert(key, element(Kind::Knob, x, 194.0, 70.0, 110.0));
        x += 70.0 + gap;
    }
    let displays = [
        (tone, "Tone", 8.0, 142.0),
        (comp, "Compressor", 256.0, 286.0),
        (duck, "Duck", 570.0, 120.0),
    ];
    let mut log = e.log().clone();
    let id = composites::encapsulate_with(
        &mut log,
        "Glue Bus",
        Definition {
            id: "kabl.glue-bus".into(),
            version: 1,
        },
        BTreeSet::from([drive, tone, trim, comp, feed, duck]),
        &ports,
        &controls,
        Some(panel),
    )
    .expect("Glue Bus wraps");
    // Displays are composite-level elements: their keys come after the interface's.
    let mut e = PatchEditor::from_log(log);
    let mut c = e.state().composites[&id].clone();
    let face = c.panel.as_mut().expect("the face");
    for (leaf, label, x, width) in displays {
        let mut p = element(Kind::Display, x, 128.0, width, 60.0);
        p.source = Some(Source::Module {
            id: leaf,
            action: None,
        });
        p.label = Some(label.into());
        face.placements.insert(c.next_interface, p);
        c.next_interface += 1;
    }
    composites::set(&mut e, id, c).expect("Glue Bus face");
    composites::package(e.state(), id).expect("Glue Bus packages")
}

/// `composites/glue-bus/v1.json` exactly as committed.
pub(crate) fn bytes() -> Vec<u8> {
    serde_json::to_vec_pretty(&package()).expect("serialise")
}

#[cfg(not(test))]
mod tool {
    use super::*;
    use composite_gen::port;
    use std::path::Path;

    fn write(root: &Path) {
        let dir = root.join("glue-bus");
        std::fs::create_dir_all(&dir).expect("bank folder");
        let bytes = bytes();
        composites::decode(&bytes).expect("the package decodes");
        let path = dir.join("v1.json");
        std::fs::write(&path, bytes).expect("write");
        println!("wrote {}", path.display());
    }

    /// A clock at 120 BPM into Drum Kit, its Out into Glue Bus, Glue Bus into the output; with
    /// `duck`, Drum Kit's Kick on Duck In.
    fn drums(duck: bool) -> PatchEditor {
        let mut e = PatchEditor::new();
        let clock = e.add_module("clock", Vec2 { x: 24.0, y: 20.0 });
        e.set_param(clock, "bpm", 120.0);
        let out = e.add_module(
            "out",
            Vec2 {
                x: 1100.0,
                y: 420.0,
            },
        );
        let kit = composite_gen::insert(&mut e, "drum-kit");
        let bus = composite_gen::insert(&mut e, "glue-bus");
        e.connect(jack(clock, "gate"), port(&e, kit, "Clock"));
        e.connect(jack(clock, "reset"), port(&e, kit, "Reset"));
        e.connect(port(&e, kit, "Out"), port(&e, bus, "In"));
        if duck {
            e.connect(port(&e, kit, "Kick"), port(&e, bus, "Duck In"));
        }
        e.connect(port(&e, bus, "Out"), jack(out, "in"));
        e
    }

    pub(super) fn main() {
        let a: Vec<String> = std::env::args().skip(1).collect();
        let dir = std::path::PathBuf::from(a.get(1).cloned().unwrap_or_else(|| ".".into()));
        match a.first().map(String::as_str) {
            Some("write") => write(&dir),
            Some("wav") => {
                // The loop dry, for comparison.
                composite_gen::wav(&dir, "drum-kit");
                composite_gen::render(drums(true).state(), &dir.join("glue-bus.wav"));
                composite_gen::render(drums(false).state(), &dir.join("glue-bus-no-duck.wav"));
            }
            _ => eprintln!("usage: glue_bus_composite write|wav DIR"),
        }
    }
}

#[cfg(not(test))]
fn main() {
    tool::main();
}
