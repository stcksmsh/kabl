//! The `patches/performance` demo: two interlocking sequence layers with velocity and gate
//! length, a drone pad, a MIDI lead through a synced echo, and a plate reverb on everything but
//! the bass, with pinned performance controls and CC mappings (channel 1, CC 20–29).
//!
//! Signal flow (mono mixers; `Voice`-rate chains are averaged into the global effects):
//! - bass `seq A` (16ths, 8 steps) → osc → filter → VCA; its envelope × step velocity
//!   (`ringmod`) is the VCA's CV. → `dry` level 1 → both main mixers (dry, centred).
//! - arp `seq B` (16ths, 7 steps against the bass's 8) → same shape → `atmos` level 1, and
//!   `echo` level 2 (an echo send).
//! - pad: two detuned saws → `padmix` → filter (slow LFO) → `atmos` level 2.
//! - lead: `midi.in` → osc → filter → VCA → `gain` +12 dB (one MIDI note is 1/8 of the voice
//!   average; this makes up most of it) → `echo` level 1 → delay.
//! - `atmos` and the delay's left/right → `bus L`/`bus R` → reverb (an insert on this bus, mix
//!   35 %) → main L/R level 2; main L/R → out.

use kabl_core::{ModuleId, PortRef, Vec2};
use kabl_ui::perform::{CC_PREFIX, PIN_PREFIX, TRANSPORT};
use kabl_ui::PatchEditor;

fn port(id: ModuleId, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

fn at(x: f32, row: usize) -> Vec2 {
    Vec2 {
        x,
        y: kabl_ui::rack::row_y(row),
    }
}

/// Module ids the builder assigns (checked in `performance()`).
pub mod ids {
    pub const CLOCK: u64 = 1;
    pub const SEQ_A: u64 = 3;
    pub const SEQ_B: u64 = 4;
    pub const FILT_A: u64 = 6;
    pub const FILT_B: u64 = 11;
    pub const PAD_FILT: u64 = 19;
    pub const DRY: u64 = 26;
    pub const ATMOS: u64 = 27;
    pub const ECHO: u64 = 28;
    pub const DELAY: u64 = 29;
    pub const REVERB: u64 = 32;
}

pub fn performance() -> PatchEditor {
    let mut e = PatchEditor::new();
    // Row 0: timing and patterns.
    let clock = e.add_module("clock", at(24.0, 0));
    let lfo = e.add_module("lfo", at(174.0, 0));
    let seq_a = e.add_module("seq", at(384.0, 0));
    let seq_b = e.add_module("seq", at(1044.0, 0));
    // Row 1: bass and arp voices.
    let osc_a = e.add_module("osc.va", at(24.0, 1));
    let filt_a = e.add_module("filter.svf", at(234.0, 1));
    let env_a = e.add_module("env.adsr", at(444.0, 1));
    let rm_a = e.add_module("ringmod", at(684.0, 1));
    let vca_a = e.add_module("vca", at(834.0, 1));
    let osc_b = e.add_module("osc.va", at(1014.0, 1));
    let filt_b = e.add_module("filter.svf", at(1224.0, 1));
    let env_b = e.add_module("env.adsr", at(1434.0, 1));
    let rm_b = e.add_module("ringmod", at(1674.0, 1));
    let vca_b = e.add_module("vca", at(1824.0, 1));
    // Row 2: pad and lead.
    let lfo_pad = e.add_module("lfo", at(24.0, 2));
    let pad_1 = e.add_module("osc.va", at(234.0, 2));
    let pad_2 = e.add_module("osc.va", at(444.0, 2));
    let pad_mix = e.add_module("mixer", at(654.0, 2));
    let pad_filt = e.add_module("filter.svf", at(894.0, 2));
    let midi = e.add_module("midi.in", at(1104.0, 2));
    let osc_l = e.add_module("osc.va", at(1254.0, 2));
    let filt_l = e.add_module("filter.svf", at(1464.0, 2));
    let env_l = e.add_module("env.adsr", at(1674.0, 2));
    let vca_l = e.add_module("vca", at(1914.0, 2));
    let lead_gain = e.add_module("gain", at(2094.0, 2));
    // Row 3: mixing and effects.
    let dry = e.add_module("mixer", at(24.0, 3));
    let atmos = e.add_module("mixer", at(264.0, 3));
    let echo = e.add_module("mixer", at(504.0, 3));
    let delay = e.add_module("delay", at(744.0, 3));
    let bus_l = e.add_module("mixer", at(984.0, 3));
    let bus_r = e.add_module("mixer", at(1224.0, 3));
    let reverb = e.add_module("reverb", at(1464.0, 3));
    let main_l = e.add_module("mixer", at(1704.0, 3));
    let main_r = e.add_module("mixer", at(1944.0, 3));
    let out = e.add_module("out", at(2184.0, 3));
    use ids::*;
    for (id, want) in [
        (clock, CLOCK),
        (seq_a, SEQ_A),
        (seq_b, SEQ_B),
        (filt_a, FILT_A),
        (filt_b, FILT_B),
        (pad_filt, PAD_FILT),
        (dry, DRY),
        (atmos, ATMOS),
        (echo, ECHO),
        (delay, DELAY),
        (reverb, REVERB),
    ] {
        assert_eq!(id, want);
    }

    let set = |e: &mut PatchEditor, id, p: &str, v: f32| e.set_param(id, p, v);
    set(&mut e, clock, "bpm", 112.0);

    // Bass: E minor, 8 sixteenths, short gates, accented by velocity (C4 = 0 st).
    let bass = [-20.0, -20.0, -8.0, -20.0, -10.0, -20.0, -13.0, -17.0];
    let bass_vel = [100.0, 45.0, 80.0, 50.0, 90.0, 45.0, 75.0, 60.0];
    for k in 0..8 {
        set(&mut e, seq_a, &format!("p{}", k + 1), bass[k]);
        set(&mut e, seq_a, &format!("v{}", k + 1), bass_vel[k]);
    }
    set(&mut e, seq_a, "gate_mode", 1.0);
    set(&mut e, seq_a, "gate_len", 35.0);

    // Arp: 7 sixteenths against the bass's 8 (they meet every 56), a rest on step 5, longer
    // gates.
    let arp = [-1.0, 4.0, 7.0, 11.0, 14.0, 11.0, 7.0];
    let arp_vel = [95.0, 50.0, 70.0, 55.0, 85.0, 45.0, 65.0];
    for k in 0..7 {
        set(&mut e, seq_b, &format!("p{}", k + 1), arp[k]);
        set(&mut e, seq_b, &format!("v{}", k + 1), arp_vel[k]);
    }
    set(&mut e, seq_b, "g5", 0.0);
    set(&mut e, seq_b, "length", 7.0);
    set(&mut e, seq_b, "gate_mode", 1.0);
    set(&mut e, seq_b, "gate_len", 60.0);

    e.connect(port(clock, "gate"), port(seq_a, "clock"));
    e.connect(port(clock, "gate"), port(seq_b, "clock"));
    e.connect(port(clock, "reset"), port(seq_a, "reset"));
    e.connect(port(clock, "reset"), port(seq_b, "reset"));

    // Sequenced voices: velocity scales the envelope (ringmod = multiply) and opens the filter.
    for (seq, osc, filt, env, rm, vca) in [
        (seq_a, osc_a, filt_a, env_a, rm_a, vca_a),
        (seq_b, osc_b, filt_b, env_b, rm_b, vca_b),
    ] {
        e.connect(port(seq, "pitch"), port(osc, "pitch"));
        e.connect(port(seq, "gate"), port(env, "gate"));
        e.connect(port(osc, "out"), port(filt, "in"));
        e.connect(port(filt, "lp"), port(vca, "in"));
        e.connect(port(env, "out"), port(rm, "a"));
        e.connect(port(seq, "velocity"), port(rm, "b"));
        e.connect(port(rm, "out"), port(vca, "cv"));
        set(&mut e, vca, "gain", 0.0);
        let r = e.connect_route(port(env, "out"), filt, "cutoff_hz");
        e.set_route_amount(r, 0.3, true);
        let r = e.connect_route(port(seq, "velocity"), filt, "cutoff_hz");
        e.set_route_amount(r, 0.12, true);
    }
    set(&mut e, osc_a, "waveform", 2.0);
    set(&mut e, filt_a, "cutoff_hz", 360.0);
    set(&mut e, filt_a, "resonance", 0.55);
    for (p, v) in [
        ("attack_ms", 1.0),
        ("decay_ms", 180.0),
        ("sustain", 0.25),
        ("release_ms", 90.0),
    ] {
        set(&mut e, env_a, p, v);
    }
    set(&mut e, osc_b, "waveform", 3.0);
    set(&mut e, filt_b, "cutoff_hz", 1500.0);
    set(&mut e, filt_b, "resonance", 0.3);
    for (p, v) in [
        ("attack_ms", 3.0),
        ("decay_ms", 220.0),
        ("sustain", 0.2),
        ("release_ms", 180.0),
    ] {
        set(&mut e, env_b, p, v);
    }
    // A slow sweep on the bass filter.
    set(&mut e, lfo, "rate_hz", 0.04);
    set(&mut e, lfo, "waveform", 1.0);
    let r = e.connect_route(port(lfo, "out"), filt_a, "cutoff_hz");
    e.set_route_amount(r, 0.12, true);

    // Pad: E3 and a slightly sharp B3, dark and slowly moving.
    set(&mut e, pad_1, "base_hz", 164.81);
    set(&mut e, pad_2, "base_hz", 247.7);
    e.connect(port(pad_1, "out"), port(pad_mix, "in1"));
    e.connect(port(pad_2, "out"), port(pad_mix, "in2"));
    set(&mut e, pad_mix, "level1", 0.5);
    set(&mut e, pad_mix, "level2", 0.4);
    e.connect(port(pad_mix, "out"), port(pad_filt, "in"));
    set(&mut e, pad_filt, "cutoff_hz", 650.0);
    set(&mut e, pad_filt, "resonance", 0.25);
    set(&mut e, lfo_pad, "rate_hz", 0.03);
    set(&mut e, lfo_pad, "waveform", 1.0);
    let r = e.connect_route(port(lfo_pad, "out"), pad_filt, "cutoff_hz");
    e.set_route_amount(r, 0.2, true);

    // Lead, played on the keyboard.
    e.connect(port(midi, "pitch"), port(osc_l, "pitch"));
    e.connect(port(midi, "gate"), port(env_l, "gate"));
    e.connect(port(osc_l, "out"), port(filt_l, "in"));
    e.connect(port(filt_l, "lp"), port(vca_l, "in"));
    e.connect(port(env_l, "out"), port(vca_l, "cv"));
    set(&mut e, vca_l, "gain", 0.0);
    set(&mut e, osc_l, "waveform", 2.0);
    set(&mut e, filt_l, "cutoff_hz", 1800.0);
    set(&mut e, filt_l, "resonance", 0.25);
    let r = e.connect_route(port(env_l, "out"), filt_l, "cutoff_hz");
    e.set_route_amount(r, 0.25, true);
    for (p, v) in [
        ("attack_ms", 25.0),
        ("decay_ms", 500.0),
        ("sustain", 0.6),
        ("release_ms", 700.0),
    ] {
        set(&mut e, env_l, p, v);
    }
    e.connect(port(vca_l, "out"), port(lead_gain, "in"));
    set(&mut e, lead_gain, "gain_db", 12.0);

    // Mixing.
    e.connect(port(vca_a, "out"), port(dry, "in1"));
    e.connect(port(vca_b, "out"), port(atmos, "in1"));
    e.connect(port(pad_filt, "lp"), port(atmos, "in2"));
    e.connect(port(lead_gain, "out"), port(echo, "in1"));
    e.connect(port(vca_b, "out"), port(echo, "in2"));
    e.connect(port(echo, "out"), port(delay, "in"));
    e.connect(port(clock, "gate"), port(delay, "clock"));
    for (bus, side) in [(bus_l, "left"), (bus_r, "right")] {
        e.connect(port(atmos, "out"), port(bus, "in1"));
        e.connect(port(delay, side), port(bus, "in2"));
    }
    e.connect(port(bus_l, "out"), port(reverb, "in_l"));
    e.connect(port(bus_r, "out"), port(reverb, "in_r"));
    for (main, side) in [(main_l, "left"), (main_r, "right")] {
        e.connect(port(dry, "out"), port(main, "in1"));
        e.connect(port(reverb, side), port(main, "in2"));
        e.connect(port(main, "out"), port(out, side));
    }

    // Levels.
    set(&mut e, dry, "level1", 0.22);
    set(&mut e, atmos, "level1", 0.05);
    set(&mut e, atmos, "level2", 0.11);
    set(&mut e, echo, "level1", 0.25);
    set(&mut e, echo, "level2", 0.035);

    // Effects: dotted eighths bouncing left and right; a long, darkish plate.
    set(&mut e, delay, "sync", 3.0);
    set(&mut e, delay, "feedback", 45.0);
    set(&mut e, delay, "mix", 30.0);
    set(&mut e, delay, "tone_hz", 3000.0);
    set(&mut e, delay, "mode", 1.0);
    set(&mut e, reverb, "decay_s", 6.0);
    set(&mut e, reverb, "damp_hz", 4500.0);
    set(&mut e, reverb, "mix", 35.0);
    set(&mut e, reverb, "predelay_ms", 30.0);

    // Performance pins and CC mappings (channel 1).
    let controls: [(ModuleId, &str, Option<u8>, &str); 12] = [
        (clock, TRANSPORT, None, "Transport"),
        (dry, "level1", Some(20), "Bass"),
        (atmos, "level1", Some(21), "Arp"),
        (atmos, "level2", Some(22), "Pad"),
        (echo, "level1", Some(23), "Lead"),
        (filt_a, "cutoff_hz", Some(24), "Bass cutoff"),
        (filt_b, "cutoff_hz", Some(25), "Arp cutoff"),
        (pad_filt, "cutoff_hz", Some(26), "Pad cutoff"),
        (seq_a, "transpose", None, "Bass transpose"),
        (delay, "feedback", Some(27), "Echo feedback"),
        (reverb, "mix", Some(28), "Reverb"),
        (reverb, "decay_s", Some(29), "Reverb decay"),
    ];
    let mut changes = Vec::new();
    for (i, (id, key, cc, _)) in controls.iter().enumerate() {
        changes.push((*id, format!("{PIN_PREFIX}{key}"), Some(i as f32)));
        if let Some(cc) = cc {
            changes.push((*id, format!("{CC_PREFIX}{key}"), Some(*cc as f32)));
        }
    }
    e.set_presentation(&changes);
    for (id, key, _, label) in controls {
        e.set_label(id, &format!("{PIN_PREFIX}{key}"), Some(label.to_string()));
    }
    e
}

/// Not a check: writes `patches/performance`.
/// `cargo test -p kabl-ui --test performance write_performance_patch -- --ignored`.
#[test]
#[ignore]
fn write_performance_patch() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/performance");
    let _ = std::fs::remove_dir_all(&dir);
    kabl_core::save(&dir, performance().log()).unwrap();
}

/// The committed patch is what `performance()` builds, every stored value is in range, and it
/// compiles.
#[test]
fn committed_performance_patch_matches_the_builder() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/performance");
    let saved = kabl_core::load(&dir).unwrap();
    assert_eq!(saved.state(), performance().state());
    for m in saved.state().modules.values() {
        let info = kabl_modules::registry::info_for(&m.kind).unwrap();
        for p in info.params {
            if let Some(&v) = m.params.get(p.name) {
                assert!((p.min..=p.max).contains(&v), "{} {} = {v}", m.kind, p.name);
            }
        }
    }
    kabl_engine::compile::compile(saved.state(), 48000.0, 8).expect("compiles");
    assert_eq!(kabl_ui::perform::pins(saved.state()).len(), 12);
}
