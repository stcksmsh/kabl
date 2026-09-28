//! D06 demo patches (docs/midi-timing/README.md):
//!
//! - `patches/expressive/lead`: the palette's Singing Lead with the modulation wheel routed to
//!   its vibrato depth (macro m2, which scales the LFO into both oscillators' fine tune) and
//!   a ±2 semitone bend (the default, saved explicitly).
//! - `patches/expressive/split`: two keyboards played apart by MIDI channel: channel 1 plays
//!   a polyphonic saw keys voice whose filter opens with the wheel, channel 2 a legato bass
//!   with glide and a ±12 semitone bend.
//!
//! Rewrite them with
//! `cargo test -p kabl-ui --test expressive write_expressive_patches -- --ignored`, then the
//! library metadata with `cargo test -p kabl-ui --test library write_factory_library -- --ignored`.

use std::path::{Path, PathBuf};

use kabl_core::{ModuleId, PortRef, Vec2};
use kabl_engine::compile::compile;
use kabl_ui::rack::row_y;
use kabl_ui::PatchEditor;

fn port(id: ModuleId, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

fn patches() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches")
}

fn at(x: f32, row: usize) -> Vec2 {
    Vec2 { x, y: row_y(row) }
}

pub fn lead() -> PatchEditor {
    let log = kabl_core::load(&patches().join("palette/lead")).unwrap();
    let mut e = PatchEditor::from_log(log);
    let (midi, macros) = (2, 1);
    assert_eq!(e.state().modules[&midi].kind, "midi.in");
    assert_eq!(e.state().modules[&macros].kind, "macro");
    e.set_param(midi, "bend", 2.0);
    let wheel = e.connect_route(port(midi, "wheel"), macros, "m2");
    e.set_route_amount(wheel, 0.8, true);
    e
}

pub fn split() -> PatchEditor {
    let mut e = PatchEditor::new();
    // Channel 1: polyphonic keys.
    let keys = e.add_module("midi.in", at(24.0, 0));
    let saw = e.add_module("osc.va", at(174.0, 0));
    let filter = e.add_module("filter.svf", at(324.0, 0));
    let env = e.add_module("env.adsr", at(474.0, 0));
    let vca = e.add_module("vca", at(624.0, 0));
    e.set_param(keys, "channel", 1.0);
    e.set_param(saw, "waveform", 2.0);
    e.set_param(filter, "cutoff_hz", 700.0);
    e.set_param(filter, "resonance", 0.35);
    e.set_param(env, "attack_ms", 8.0);
    e.set_param(env, "release_ms", 450.0);
    e.connect(port(keys, "pitch"), port(saw, "pitch"));
    e.connect(port(saw, "out"), port(filter, "in"));
    e.connect(port(filter, "lp"), port(vca, "in"));
    e.connect(port(keys, "gate"), port(env, "gate"));
    e.connect(port(env, "out"), port(vca, "cv"));
    let wheel = e.connect_route(port(keys, "wheel"), filter, "cutoff_hz");
    e.set_route_amount(wheel, 0.55, true);
    // Channel 2: legato bass with glide and a wide bend.
    let bass = e.add_module("midi.in", at(24.0, 1));
    let sq = e.add_module("osc.va", at(174.0, 1));
    let low = e.add_module("filter.svf", at(324.0, 1));
    let benv = e.add_module("env.adsr", at(474.0, 1));
    let bvca = e.add_module("vca", at(624.0, 1));
    e.set_param(bass, "channel", 2.0);
    e.set_param(bass, "mode", 2.0);
    e.set_param(bass, "glide", 2.0);
    e.set_param(bass, "glide_ms", 90.0);
    e.set_param(bass, "bend", 12.0);
    e.set_param(sq, "waveform", 3.0);
    e.set_param(sq, "base_hz", 130.81);
    e.set_param(low, "cutoff_hz", 420.0);
    e.set_param(benv, "release_ms", 200.0);
    e.set_param(benv, "sustain", 0.8);
    e.connect(port(bass, "pitch"), port(sq, "pitch"));
    e.connect(port(sq, "out"), port(low, "in"));
    e.connect(port(low, "lp"), port(bvca, "in"));
    e.connect(port(bass, "gate"), port(benv, "gate"));
    e.connect(port(benv, "out"), port(bvca, "cv"));
    // Both to the output, a little apart.
    let mix_l = e.add_module("mixer", at(774.0, 0));
    let mix_r = e.add_module("mixer", at(774.0, 1));
    let out = e.add_module("out", at(924.0, 0));
    e.connect(port(vca, "out"), port(mix_l, "in1"));
    e.connect(port(bvca, "out"), port(mix_l, "in2"));
    e.connect(port(vca, "out"), port(mix_r, "in1"));
    e.connect(port(bvca, "out"), port(mix_r, "in2"));
    for (m, keys_level, bass_level) in [(mix_l, 0.5, 0.42), (mix_r, 0.42, 0.5)] {
        e.set_param(m, "level1", keys_level);
        e.set_param(m, "level2", bass_level);
    }
    e.connect(port(mix_l, "out"), port(out, "left"));
    e.connect(port(mix_r, "out"), port(out, "right"));
    e
}

#[test]
#[ignore = "writes patches/expressive"]
fn write_expressive_patches() {
    kabl_core::save(&patches().join("expressive/lead"), lead().log()).unwrap();
    kabl_core::save(&patches().join("expressive/split"), split().log()).unwrap();
}

#[test]
fn committed_expressive_patches_match_their_builders() {
    for (dir, e) in [("expressive/lead", lead()), ("expressive/split", split())] {
        let saved = kabl_core::load(&patches().join(dir)).unwrap();
        assert_eq!(saved.state(), e.state(), "{dir}");
    }
}

/// Bend range, channel and the wheel route are ordinary saved params and cables: they survive
/// save and reopen, compile into the keyboard settings, and undo like any knob.
#[test]
fn bend_channel_and_wheel_route_persist_and_undo() {
    let tmp = tempfile::tempdir().unwrap();
    let mut e = split();
    kabl_core::save(tmp.path(), e.log()).unwrap();
    let back = kabl_core::load(tmp.path()).unwrap();
    assert_eq!(back.state(), e.state());
    let graph = compile(back.state(), 48000.0, 8).unwrap();
    let mut seen = Vec::new();
    graph.keyboards(|id, s| seen.push((id, s.channel, s.mode)));
    seen.sort();
    assert_eq!(seen, [(1, 1, 0), (6, 2, 2)]);
    let bass = 6;
    assert_eq!(e.state().modules[&bass].params["bend"], 12.0);
    e.set_param(bass, "bend", 5.0);
    assert!(e.undo());
    assert_eq!(e.state().modules[&bass].params["bend"], 12.0);
}
