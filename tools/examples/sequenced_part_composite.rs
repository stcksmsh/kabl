//! The factory composites `kabl.sequenced-part` v1 and `kabl.sequenced-part-sub` v1 (catalogue
//! C3, roadmap CB3): a self-playing line, a step sequencer into a voice into a level into a tone
//! filter.
//!
//! Two definitions, one per catalogue family, because their wiring differs:
//! - `kabl.sequenced-part` is family A, the pieces' line (`add_line` in `presets_piece.rs`):
//!   seq → osc → vca (env) → gain → svf. Defaults follow Laundromat's first stab.
//! - `kabl.sequenced-part-sub` is family B, the grooves' part (`seq_part` in
//!   `presets_groove.rs`): seq → osc and a square sub an octave down → mixer → vca (env) → gain
//!   → svf. Defaults follow Corner Store's bass.
//!
//! See `docs/composites/SEQUENCED-PART.md`.
//!
//! `cargo run -p kabl-tools --example sequenced_part_composite -- write composites` writes
//! `composites/sequenced-part/v1.json` and `composites/sequenced-part-sub/v1.json`. Running it
//! again writes the same bytes: a package holds the patch state only, never the log's
//! wall-clock timestamps.
//!
//! `cargo run --release -p kabl-tools --example sequenced_part_composite -- wav DIR` renders
//! eight seconds of a clock at 120 BPM into each definition into the output, offline, to
//! `DIR/<name>.wav`, and prints peak and RMS.
//!
//! The captures are scenes of `chrome_capture`: `sequenced-part-rack`, `-face`, `-design` and
//! the same three for `sequenced-part-sub`.
//!
//! The test `composites_sequenced_part` includes this file to compare `bytes()` with the
//! committed files, so only the generator is compiled there.
use kabl_core::panel::{Kind, Panel, Placement, Source};
use kabl_core::{Definition, ModuleId, Vec2};
use kabl_ui::{composites, PatchEditor};
use std::collections::BTreeSet;

/// The bank folders, family A then family B.
pub(crate) const NAMES: [&str; 2] = ["sequenced-part", "sequenced-part-sub"];

#[path = "composite_gen/mod.rs"]
mod composite_gen;
use composite_gen::{expose, jack, param};

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

fn adsr(e: &mut PatchEditor, id: ModuleId, [a, d, s, r]: [f32; 4]) {
    for (name, v) in [
        ("attack_ms", a),
        ("decay_ms", d),
        ("sustain", s),
        ("release_ms", r),
    ] {
        e.set_param(id, name, v);
    }
}

fn set(e: &mut PatchEditor, id: ModuleId, params: &[(&str, f32)]) {
    for &(name, v) in params {
        e.set_param(id, name, v);
    }
}

/// `name` is one of `NAMES`.
fn package(name: &str) -> composites::Package {
    let sub = match name {
        "sequenced-part" => false,
        "sequenced-part-sub" => true,
        _ => panic!("no definition {name}"),
    };
    let mut e = PatchEditor::new();
    let at = |x: f32, y: f32| Vec2 { x, y };
    let seq = e.add_module("seq", at(24.0, 20.0));
    // The modules as the factory builders add them, with their settings in the same order.
    let (osc, env, gain, tone, mut members) = if !sub {
        // Laundromat's first stab (`stab("- - 9 ...", -6.0)` in `presets_piece.rs`).
        set(
            &mut e,
            seq,
            &[("length", 16.0), ("gate_len", 50.0), ("gate_mode", 1.0)],
        );
        for (k, tok) in "- - 9 - - 9 - - - - 9 - 12 - - -"
            .split_whitespace()
            .enumerate()
        {
            let n = k + 1;
            if tok == "-" {
                e.set_param(seq, &format!("g{n}"), 0.0);
                continue;
            }
            e.set_param(seq, &format!("p{n}"), tok.parse().expect(tok));
            e.set_param(seq, &format!("g{n}"), 1.0);
            e.set_param(seq, &format!("v{n}"), 70.0);
        }
        let osc = e.add_module("osc.va", at(24.0, 420.0));
        set(
            &mut e,
            osc,
            &[
                ("base_hz", 261.63 * 2f32.powf(-1.0)),
                ("waveform", 3.0),
                ("unison", 2.0),
                ("detune", 14.0),
                ("pw", 35.0),
            ],
        );
        e.connect(jack(seq, "pitch"), jack(osc, "pitch"));
        let env = e.add_module("env.adsr", at(214.0, 420.0));
        adsr(&mut e, env, [1.0, 200.0, 0.7, 40.0]);
        e.connect(jack(seq, "gate"), jack(env, "gate"));
        let vca = e.add_module("vca", at(404.0, 420.0));
        e.set_param(vca, "gain", 0.0);
        e.connect(jack(env, "out"), jack(vca, "cv"));
        e.connect(jack(osc, "out"), jack(vca, "in"));
        let gain = e.add_module("gain", at(594.0, 420.0));
        e.set_param(gain, "gain_db", -6.0);
        e.connect(jack(vca, "out"), jack(gain, "in"));
        let tone = e.add_module("filter.svf", at(784.0, 420.0));
        set(&mut e, tone, &[("cutoff_hz", 3000.0), ("resonance", 0.1)]);
        e.connect(jack(gain, "out"), jack(tone, "in"));
        (osc, env, gain, tone, vec![seq, osc, env, vca, gain, tone])
    } else {
        // Corner Store's bass (`part("0 - 12 0 ...", ..).lp(8000.0)` in `presets_groove.rs`).
        set(&mut e, seq, &[("length", 16.0), ("gate_len", 55.0)]);
        for (k, tok) in "0 - 12 0 - 3 - 5 7 - 5 - 3 - 0 -"
            .split_whitespace()
            .enumerate()
        {
            let step: Option<f32> = tok.parse().ok();
            e.set_param(seq, &format!("p{}", k + 1), step.unwrap_or(0.0));
            e.set_param(
                seq,
                &format!("g{}", k + 1),
                f32::from(u8::from(step.is_some())),
            );
        }
        let osc = e.add_module("osc.va", at(24.0, 420.0));
        set(
            &mut e,
            osc,
            &[
                ("base_hz", 261.63 * 2f32.powf(-2.0)),
                ("waveform", 2.0),
                ("unison", 1.0),
                ("detune", 6.0),
                ("pw", 50.0),
            ],
        );
        let low = e.add_module("osc.va", at(24.0, 820.0));
        set(
            &mut e,
            low,
            &[
                ("base_hz", 261.63 * 2f32.powf(-2.0 + -12.0 / 12.0)),
                ("waveform", 3.0),
            ],
        );
        let env = e.add_module("env.adsr", at(214.0, 420.0));
        adsr(&mut e, env, [1.0, 140.0, 0.4, 40.0]);
        let mix = e.add_module("mixer", at(214.0, 820.0));
        set(&mut e, mix, &[("level1", 1.0), ("level2", 0.3)]);
        let vca = e.add_module("vca", at(404.0, 420.0));
        e.set_param(vca, "gain", 0.0);
        let gain = e.add_module("gain", at(594.0, 420.0));
        e.set_param(gain, "gain_db", 20.0 * 0.7f32.max(1e-3).log10());
        e.connect(jack(seq, "pitch"), jack(osc, "pitch"));
        e.connect(jack(seq, "pitch"), jack(low, "pitch"));
        e.connect(jack(seq, "gate"), jack(env, "gate"));
        e.connect(jack(osc, "out"), jack(mix, "in1"));
        e.connect(jack(low, "out"), jack(mix, "in2"));
        e.connect(jack(mix, "out"), jack(vca, "in"));
        e.connect(jack(env, "out"), jack(vca, "cv"));
        e.connect(jack(vca, "out"), jack(gain, "in"));
        let tone = e.add_module("filter.svf", at(784.0, 420.0));
        set(&mut e, tone, &[("cutoff_hz", 8000.0), ("resonance", 0.0)]);
        e.connect(jack(gain, "out"), jack(tone, "in"));
        (
            osc,
            env,
            gain,
            tone,
            vec![seq, osc, low, env, mix, vca, gain, tone],
        )
    };
    members.sort_unstable();
    // Interface keys: ports 1..=8, then controls 9..=20, in the order listed.
    let ports = [
        expose("Clock", jack(seq, "clock")),
        expose("Reset", jack(seq, "reset")),
        expose("Out", jack(tone, "lp")),
        expose("Pitch out", jack(seq, "pitch")),
        expose("Gate out", jack(seq, "gate")),
        expose("Gate mod", param(seq, "gate_len")),
        expose("Tone mod", param(tone, "cutoff_hz")),
        expose("Level mod", param(gain, "gain_db")),
    ];
    let controls = [
        expose("Steps", param(seq, "length")),
        expose("Gate", param(seq, "gate_len")),
        expose("Transpose", param(seq, "transpose")),
        expose("Wave", param(osc, "waveform")),
        expose("Unison", param(osc, "unison")),
        expose("Detune", param(osc, "detune")),
        expose("Attack", param(env, "attack_ms")),
        expose("Decay", param(env, "decay_ms")),
        expose("Sustain", param(env, "sustain")),
        expose("Release", param(env, "release_ms")),
        expose("Level", param(gain, "gain_db")),
        expose("Tone", param(tone, "cutoff_hz")),
    ];
    // The sequencer along the top: Steps (a selector of 32 at 18 points each), with Gate and
    // Transpose as knobs at the right. The oscillator at the left of the middle: Wave's glyphs
    // beside its display, Unison's 16 choices under them, then Detune and the envelope. Level
    // and Tone at the bottom right, beside the jacks. Knobs, not sliders: a slider follows the
    // player's orientation setting, and a vertical one needs more height than a row has.
    let mut panel = Panel {
        width: 840.0,
        ..Panel::default()
    };
    let mut put = |key: u64, p: Placement| {
        panel.placements.insert(key, p);
    };
    put(9, placement(Kind::Selector, 8.0, 64.0, 576.0, 55.0));
    put(10, placement(Kind::Knob, 672.0, 64.0, 70.0, 110.0));
    put(11, placement(Kind::Knob, 752.0, 64.0, 70.0, 110.0));
    put(12, placement(Kind::Icons, 8.0, 124.0, 84.0, 60.0));
    put(13, placement(Kind::Selector, 8.0, 188.0, 288.0, 55.0));
    // Detune | Attack Decay Sustain Release.
    let mut x = 304.0;
    for (key, gap) in [(14, 10.0), (15, 2.0), (16, 2.0), (17, 2.0), (18, 0.0)] {
        put(key, placement(Kind::Knob, x, 128.0, 70.0, 110.0));
        x += 70.0 + gap;
    }
    put(19, placement(Kind::Knob, 672.0, 186.0, 70.0, 110.0));
    put(20, placement(Kind::Knob, 752.0, 186.0, 70.0, 110.0));
    let mut x = 8.0;
    for key in 1..=8u64 {
        put(key, placement(Kind::Jack, x, 250.0, 80.0, 58.0));
        x += 82.0;
    }
    let mut log = e.log().clone();
    let (label, id) = if sub {
        ("Sequenced Part + Sub", "kabl.sequenced-part-sub")
    } else {
        ("Sequenced Part", "kabl.sequenced-part")
    };
    let id = composites::encapsulate_with(
        &mut log,
        label,
        Definition {
            id: id.into(),
            version: 1,
        },
        BTreeSet::from_iter(members),
        &ports,
        &controls,
        Some(panel),
    )
    .expect("Sequenced Part wraps");
    // The display is a composite-level element: its key comes after the interface's.
    let mut e = PatchEditor::from_log(log);
    let mut c = e.state().composites[&id].clone();
    let face = c.panel.as_mut().expect("the face");
    let mut p = placement(Kind::Display, 96.0, 124.0, 200.0, 60.0);
    p.source = Some(Source::Module {
        id: osc,
        action: None,
    });
    p.label = Some("Oscillator".into());
    face.placements.insert(c.next_interface, p);
    c.next_interface += 1;
    composites::set(&mut e, id, c).expect("Sequenced Part face");
    composites::package(e.state(), id).expect("Sequenced Part packages")
}

/// `composites/<name>/v1.json` exactly as committed; `name` is one of `NAMES`.
pub(crate) fn bytes(name: &str) -> Vec<u8> {
    serde_json::to_vec_pretty(&package(name)).expect("serialise")
}

#[cfg(not(test))]
mod tool {
    use super::*;
    use std::path::Path;

    fn write(root: &Path) {
        for name in NAMES {
            let dir = root.join(name);
            std::fs::create_dir_all(&dir).expect("bank folder");
            let bytes = bytes(name);
            composites::decode(&bytes).expect("the package decodes");
            let path = dir.join("v1.json");
            std::fs::write(&path, bytes).expect("write");
            println!("wrote {}", path.display());
        }
    }

    pub(super) fn main() {
        let a: Vec<String> = std::env::args().skip(1).collect();
        let dir = std::path::PathBuf::from(a.get(1).cloned().unwrap_or_else(|| ".".into()));
        match a.first().map(String::as_str) {
            Some("write") => write(&dir),
            Some("wav") => NAMES.iter().for_each(|name| composite_gen::wav(&dir, name)),
            _ => eprintln!("usage: sequenced_part_composite write|wav DIR"),
        }
    }
}

#[cfg(not(test))]
fn main() {
    tool::main();
}
