//! Builders of the recipe teaching patches (`patches/recipes/*`). The factory files are what
//! these build; `tests/library.rs` regenerates and checks them.

use kabl_core::{ModuleId, PortRef, Vec2};

use crate::perform::PIN_PREFIX;
use crate::PatchEditor;

fn port(id: ModuleId, p: &str) -> PortRef {
    PortRef::Module { id, port: p.into() }
}

fn at(row: usize, x: f32) -> Vec2 {
    Vec2 {
        x,
        y: crate::rack::row_y(row),
    }
}

fn pin(e: &mut PatchEditor, pins: &[(ModuleId, &str, &str)]) {
    let changes: Vec<_> = pins
        .iter()
        .enumerate()
        .map(|(i, (id, key, _))| (*id, format!("{PIN_PREFIX}{key}"), Some(i as f32)))
        .collect();
    e.set_presentation(&changes);
    for (id, key, label) in pins {
        e.set_label(*id, &format!("{PIN_PREFIX}{key}"), Some(label.to_string()));
    }
}

/// A keyboard voice: MIDI In #1 → Osc #2 → Filter #3 → VCA #4 → Output #5, ADSR #6 on the VCA.
fn voice(cutoff: f32, res: f32, env: [f32; 4]) -> PatchEditor {
    let mut e = PatchEditor::new();
    let midi = e.add_module("midi.in", at(0, 24.0));
    let osc = e.add_module("osc.va", at(0, 240.0));
    let filter = e.add_module("filter.svf", at(0, 420.0));
    let vca = e.add_module("vca", at(0, 600.0));
    let out = e.add_module("out", at(0, 760.0));
    let adsr = e.add_module("env.adsr", at(1, 600.0));
    e.set_param(osc, "waveform", 2.0); // saw
    e.set_param(filter, "cutoff_hz", cutoff);
    e.set_param(filter, "resonance", res);
    for (k, v) in ["attack_ms", "decay_ms", "sustain", "release_ms"]
        .iter()
        .zip(env)
    {
        e.set_param(adsr, k, v);
    }
    e.set_param(vca, "gain", 0.0);
    e.connect(port(midi, "pitch"), port(osc, "pitch"));
    e.connect(port(osc, "out"), port(filter, "in"));
    e.connect(port(filter, "lp"), port(vca, "in"));
    e.connect(port(midi, "gate"), port(adsr, "gate"));
    e.connect(port(adsr, "out"), port(vca, "cv"));
    e.connect(port(vca, "out"), port(out, "in"));
    e
}

/// `patches/recipes/pluck-to-pad`: a plucked saw, envelope controls pinned.
pub fn pluck_to_pad() -> PatchEditor {
    let mut e = voice(2400.0, 0.2, [2.0, 220.0, 0.0, 120.0]);
    pin(
        &mut e,
        &[
            (6, "attack_ms", "Attack"),
            (6, "decay_ms", "Decay"),
            (6, "sustain", "Sustain"),
            (6, "release_ms", "Release"),
        ],
    );
    // The VCA only lowers a signal; this Gain after it brings the recipe to the factory level
    // target (docs/factory-levels/README.md).
    let gain = e.add_module("gain", at(0, 900.0));
    e.set_param(gain, "gain_db", 6.5);
    e.connect(port(4, "out"), port(gain, "in"));
    e.connect(port(gain, "out"), port(5, "in"));
    e
}

/// `patches/recipes/filter-movement`: a held saw through a resonant low-pass, with LFO #7
/// routed to Cutoff (+35 %) but bypassed.
pub fn filter_movement() -> PatchEditor {
    let mut e = voice(900.0, 0.45, [10.0, 300.0, 0.85, 300.0]);
    let lfo = e.add_module("lfo", at(1, 240.0));
    e.set_param(lfo, "rate_hz", 0.5);
    let route = e.connect_route(port(lfo, "out"), 3, "cutoff_hz");
    e.set_route_amount(route, 0.35, true);
    e.set_route_bypass(route, true);
    pin(
        &mut e,
        &[
            (3, "cutoff_hz", "Cutoff"),
            (3, "resonance", "Resonance"),
            (lfo, "rate_hz", "LFO rate"),
        ],
    );
    e
}
