//! The `patches/composition` demo: the performance demo's bass, arp, pad and MIDI lead, grown
//! into sections. Each sequencer has four contrasting banks; five cues switch them together
//! (Intro, Main, Variation, Breakdown, Return); two clock-synced LFOs move the filters; four
//! named macros (Energy, Motion, Space, Glow) shape many controls at once.
//!
//! Signal flow as in `performance.rs`, plus:
//! - Motion: the 4-bar bass LFO × macro 2 (`ringmod`) is what reaches the bass and arp
//!   cutoffs, so Motion sets how much the filters move; the 8-bar pad LFO × macro 2 reaches
//!   the pad cutoff.
//! - Energy, Space and Glow reach their destinations directly through routes.
//! - Controls: CC 20–24 (macros, lead), buttons CC 40–44 (cues 1–5), 45 (cancel), 46 (run/stop),
//!   47 (restart), all on channel 1.

use kabl_core::{ModuleId, PortRef, Vec2};
use kabl_engine::patch_engine::Timing;
use kabl_ui::perform::{BANKS, BTN_PREFIX, CC_PREFIX, CUE_PADS, PIN_PREFIX, TRANSPORT};
use kabl_ui::{banks, cues, PatchEditor};

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

pub mod ids {
    pub const CLOCK: u64 = 1;
    pub const CUES: u64 = 2;
    pub const MACROS: u64 = 3;
    pub const LFO_BASS: u64 = 4;
    pub const LFO_PAD: u64 = 5;
    pub const BASS: u64 = 6;
    pub const ARP: u64 = 7;
}

/// A bank: pitches, gates (1 = on), velocities, probabilities, length, LENGTH gate %.
struct Bank {
    name: &'static str,
    p: [f32; 8],
    g: [u8; 8],
    v: [f32; 8],
    r: [f32; 8],
    len: usize,
    gate_len: f32,
}

fn write_bank(e: &mut PatchEditor, seq: ModuleId, b: usize, bank: &Bank) {
    use kabl_modules::builtins::seq::bank_param;
    for k in 0..8 {
        e.set_param(seq, bank_param(b, k), bank.p[k]);
        if bank.g[k] == 0 {
            e.set_param(seq, bank_param(b, 8 + k), 0.0);
        }
        e.set_param(seq, bank_param(b, 17 + k), bank.v[k]);
        if bank.r[k] < 100.0 {
            e.set_param(seq, bank_param(b, 27 + k), bank.r[k]);
        }
    }
    e.set_param(seq, bank_param(b, 16), bank.len as f32);
    e.set_param(seq, bank_param(b, 26), 1.0); // LENGTH gates
    e.set_param(seq, bank_param(b, 25), bank.gate_len);
    banks::rename(e, seq, b, bank.name);
}

pub fn composition() -> PatchEditor {
    let mut e = PatchEditor::new();
    // Row 0: time, sections, performance controls, motion sources.
    let clock = e.add_module("clock", at(24.0, 0));
    let cue_mod = e.add_module("cues", at(204.0, 0));
    let macros = e.add_module("macro", at(714.0, 0));
    let lfo_bass = e.add_module("lfo", at(1044.0, 0));
    let lfo_pad = e.add_module("lfo", at(1284.0, 0));
    // Row 1: the two sequencers.
    let bass = e.add_module("seq", at(24.0, 1));
    let arp = e.add_module("seq", at(714.0, 1));
    let motion_a = e.add_module("ringmod", at(1404.0, 1));
    let motion_b = e.add_module("ringmod", at(1554.0, 1));
    // Row 2: bass and arp voices.
    let osc_a = e.add_module("osc.va", at(24.0, 2));
    let filt_a = e.add_module("filter.svf", at(234.0, 2));
    let env_a = e.add_module("env.adsr", at(444.0, 2));
    let rm_a = e.add_module("ringmod", at(684.0, 2));
    let vca_a = e.add_module("vca", at(834.0, 2));
    let osc_b = e.add_module("osc.va", at(1014.0, 2));
    let filt_b = e.add_module("filter.svf", at(1224.0, 2));
    let env_b = e.add_module("env.adsr", at(1434.0, 2));
    let rm_b = e.add_module("ringmod", at(1674.0, 2));
    let vca_b = e.add_module("vca", at(1824.0, 2));
    // Row 3: pad and lead.
    let pad_1 = e.add_module("osc.va", at(24.0, 3));
    let pad_2 = e.add_module("osc.va", at(234.0, 3));
    let pad_mix = e.add_module("mixer", at(444.0, 3));
    let pad_filt = e.add_module("filter.svf", at(684.0, 3));
    let midi = e.add_module("midi.in", at(894.0, 3));
    let osc_l = e.add_module("osc.va", at(1044.0, 3));
    let filt_l = e.add_module("filter.svf", at(1254.0, 3));
    let env_l = e.add_module("env.adsr", at(1464.0, 3));
    let vca_l = e.add_module("vca", at(1704.0, 3));
    let lead_gain = e.add_module("gain", at(1884.0, 3));
    // Row 4: mixing and effects.
    let dry = e.add_module("mixer", at(24.0, 4));
    let atmos = e.add_module("mixer", at(264.0, 4));
    let echo = e.add_module("mixer", at(504.0, 4));
    let delay = e.add_module("delay", at(744.0, 4));
    let bus_l = e.add_module("mixer", at(984.0, 4));
    let bus_r = e.add_module("mixer", at(1224.0, 4));
    let reverb = e.add_module("reverb", at(1464.0, 4));
    let main_l = e.add_module("mixer", at(1704.0, 4));
    let main_r = e.add_module("mixer", at(1944.0, 4));
    let out = e.add_module("out", at(2184.0, 4));
    use ids::*;
    for (id, want) in [
        (clock, CLOCK),
        (cue_mod, CUES),
        (macros, MACROS),
        (lfo_bass, LFO_BASS),
        (lfo_pad, LFO_PAD),
        (bass, BASS),
        (arp, ARP),
    ] {
        assert_eq!(id, want);
    }
    let set = |e: &mut PatchEditor, id, p: &str, v: f32| e.set_param(id, p, v);
    set(&mut e, clock, "bpm", 112.0);

    // Bass (E minor, C4 = 0 st): a sparse intro, the driving line, a lifted variation with
    // ghost notes, and long held notes for the breakdown.
    let bass_banks = [
        Bank {
            name: "Intro",
            p: [-20.0, -20.0, -20.0, -8.0, -20.0, -20.0, -13.0, -20.0],
            g: [1, 0, 0, 1, 0, 0, 1, 0],
            v: [90.0, 40.0, 40.0, 60.0, 40.0, 40.0, 70.0, 40.0],
            r: [100.0; 8],
            len: 8,
            gate_len: 70.0,
        },
        Bank {
            name: "Drive",
            p: [-20.0, -20.0, -8.0, -20.0, -10.0, -20.0, -13.0, -17.0],
            g: [1; 8],
            v: [100.0, 45.0, 80.0, 50.0, 90.0, 45.0, 75.0, 60.0],
            r: [100.0; 8],
            len: 8,
            gate_len: 35.0,
        },
        Bank {
            name: "Lift",
            p: [-15.0, -15.0, -3.0, -15.0, -5.0, -15.0, -8.0, -12.0],
            g: [1; 8],
            v: [100.0, 40.0, 85.0, 45.0, 95.0, 40.0, 80.0, 55.0],
            r: [100.0, 55.0, 100.0, 60.0, 100.0, 50.0, 100.0, 70.0],
            len: 8,
            gate_len: 40.0,
        },
        Bank {
            name: "Hold",
            p: [-20.0, -13.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            g: [1, 1, 0, 0, 0, 0, 0, 0],
            v: [55.0, 45.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0],
            r: [100.0; 8],
            len: 2,
            gate_len: 95.0,
        },
    ];
    // Arp: bells for the intro, the 7-step line, a 6-step answer with chance, sparse drops.
    let arp_banks = [
        Bank {
            name: "Bells",
            p: [11.0, 14.0, 19.0, 14.0, 7.0, 0.0, 0.0, 0.0],
            g: [1, 0, 1, 0, 1, 0, 0, 0],
            v: [70.0, 40.0, 55.0, 40.0, 60.0, 100.0, 100.0, 100.0],
            r: [100.0, 100.0, 70.0, 100.0, 60.0, 100.0, 100.0, 100.0],
            len: 5,
            gate_len: 80.0,
        },
        Bank {
            name: "Line",
            p: [-1.0, 4.0, 7.0, 11.0, 14.0, 11.0, 7.0, 0.0],
            g: [1, 1, 1, 1, 0, 1, 1, 1],
            v: [95.0, 50.0, 70.0, 55.0, 85.0, 45.0, 65.0, 100.0],
            r: [100.0; 8],
            len: 7,
            gate_len: 60.0,
        },
        Bank {
            name: "Answer",
            p: [4.0, 7.0, 11.0, 16.0, 11.0, 9.0, 0.0, 0.0],
            g: [1; 8],
            v: [90.0, 55.0, 75.0, 95.0, 60.0, 50.0, 100.0, 100.0],
            r: [100.0, 80.0, 100.0, 90.0, 65.0, 75.0, 100.0, 100.0],
            len: 6,
            gate_len: 50.0,
        },
        Bank {
            name: "Drops",
            p: [19.0, 0.0, 14.0, 0.0, 0.0, 11.0, 0.0, 0.0],
            g: [1, 0, 1, 0, 0, 1, 0, 0],
            v: [55.0, 100.0, 45.0, 100.0, 100.0, 50.0, 100.0, 100.0],
            r: [100.0, 100.0, 60.0, 100.0, 100.0, 50.0, 100.0, 100.0],
            len: 8,
            gate_len: 90.0,
        },
    ];
    for (b, bank) in bass_banks.iter().enumerate() {
        write_bank(&mut e, bass, b, bank);
    }
    for (b, bank) in arp_banks.iter().enumerate() {
        write_bank(&mut e, arp, b, bank);
    }
    // Both start on their intro banks (A, the default).

    for seq in [bass, arp] {
        e.connect(port(clock, "gate"), port(seq, "clock"));
        e.connect(port(clock, "reset"), port(seq, "reset"));
    }
    for (seq, osc, filt, env, rm, vca) in [
        (bass, osc_a, filt_a, env_a, rm_a, vca_a),
        (arp, osc_b, filt_b, env_b, rm_b, vca_b),
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
    set(&mut e, filt_a, "cutoff_hz", 330.0);
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
    set(&mut e, filt_b, "cutoff_hz", 1300.0);
    set(&mut e, filt_b, "resonance", 0.3);
    for (p, v) in [
        ("attack_ms", 3.0),
        ("decay_ms", 220.0),
        ("sustain", 0.2),
        ("release_ms", 180.0),
    ] {
        set(&mut e, env_b, p, v);
    }

    // Synced motion: a 4-bar triangle for the sequenced filters, an 8-bar sine a quarter
    // cycle later for the pad; both restart with the transport's Restart.
    for (lfo, sync, wave, phase) in [(lfo_bass, 7.0, 1.0, 0.0), (lfo_pad, 8.0, 0.0, 90.0)] {
        set(&mut e, lfo, "sync", sync);
        set(&mut e, lfo, "waveform", wave);
        set(&mut e, lfo, "phase", phase);
        set(&mut e, lfo, "rate_hz", 0.1);
        e.connect(port(clock, "gate"), port(lfo, "clock"));
        e.connect(port(clock, "reset"), port(lfo, "reset"));
    }
    // Motion (macro 2) scales both LFOs before they reach the filters.
    e.connect(port(lfo_bass, "out"), port(motion_a, "a"));
    e.connect(port(macros, "m2"), port(motion_a, "b"));
    e.connect(port(lfo_pad, "out"), port(motion_b, "a"));
    e.connect(port(macros, "m2"), port(motion_b, "b"));
    for (src, dst, amount) in [
        (motion_a, filt_a, 0.22),
        (motion_a, filt_b, -0.16),
        (motion_b, pad_filt, 0.25),
    ] {
        let r = e.connect_route(port(src, "out"), dst, "cutoff_hz");
        e.set_route_amount(r, amount, true);
    }

    // Pad.
    set(&mut e, pad_1, "base_hz", 164.81);
    set(&mut e, pad_2, "base_hz", 247.7);
    e.connect(port(pad_1, "out"), port(pad_mix, "in1"));
    e.connect(port(pad_2, "out"), port(pad_mix, "in2"));
    set(&mut e, pad_mix, "level1", 0.5);
    set(&mut e, pad_mix, "level2", 0.4);
    e.connect(port(pad_mix, "out"), port(pad_filt, "in"));
    set(&mut e, pad_filt, "cutoff_hz", 520.0);
    set(&mut e, pad_filt, "resonance", 0.25);

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

    // Mixing and effects, as in the performance demo.
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
    set(&mut e, dry, "level1", 0.22);
    set(&mut e, atmos, "level1", 0.045);
    set(&mut e, atmos, "level2", 0.1);
    set(&mut e, echo, "level1", 0.25);
    set(&mut e, echo, "level2", 0.035);
    set(&mut e, delay, "sync", 3.0);
    set(&mut e, delay, "feedback", 40.0);
    set(&mut e, delay, "mix", 25.0);
    set(&mut e, delay, "tone_hz", 3000.0);
    set(&mut e, delay, "mode", 1.0);
    set(&mut e, reverb, "decay_s", 5.0);
    set(&mut e, reverb, "damp_hz", 4500.0);
    set(&mut e, reverb, "mix", 25.0);
    set(&mut e, reverb, "predelay_ms", 30.0);

    // Macros. Ranges were set by ear to stay musical over the whole travel: at 0 each is a
    // restrained version of the section, at 1 the fullest that still sits in the mix.
    for (k, (name, value)) in [
        ("Energy", 0.4),
        ("Motion", 0.5),
        ("Space", 0.3),
        ("Glow", 0.3),
    ]
    .into_iter()
    .enumerate()
    {
        let m = format!("m{}", k + 1);
        e.set_label(macros, &format!("name.{m}"), Some(name.into()));
        set(&mut e, macros, &m, value);
    }
    let routes: [(&str, ModuleId, &str, f32); 11] = [
        // Energy: brighter, punchier sequences, a little more arp.
        ("m1", filt_a, "cutoff_hz", 0.2),
        ("m1", filt_b, "cutoff_hz", 0.15),
        ("m1", env_a, "decay_ms", 0.12),
        ("m1", atmos, "level1", 0.035),
        // Space: more and longer reverb, a wetter echo that repeats longer.
        ("m3", reverb, "mix", 0.3),
        ("m3", reverb, "decay_s", 0.12),
        ("m3", delay, "mix", 0.25),
        ("m3", delay, "feedback", 0.15),
        // Glow: the pad opens and comes forward.
        ("m4", pad_filt, "cutoff_hz", 0.25),
        ("m4", atmos, "level2", 0.05),
        ("m4", pad_filt, "resonance", 0.1),
    ];
    for (m, dst, p, amount) in routes {
        let r = e.connect_route(port(macros, m), dst, p);
        e.set_route_amount(r, amount, true);
    }

    // Cues: Intro, Main, Variation, Breakdown, Return, on the clock's next bar.
    for (name, b, a) in [
        ("Intro", 0, 0),
        ("Main", 1, 1),
        ("Variation", 2, 2),
        ("Breakdown", 3, 3),
        ("Return", 1, 2),
    ] {
        let n = cues::add(&mut e, cue_mod, &|_| 0).unwrap();
        cues::rename(&mut e, cue_mod, n, name);
        cues::set_target(&mut e, cue_mod, n, bass, Some(b));
        cues::set_target(&mut e, cue_mod, n, arp, Some(a));
        assert_eq!(
            cues::cues(e.state(), cue_mod)[n - 1].timing,
            Timing::NextBar
        );
    }

    // Perform panel and MIDI (channel 1).
    let pins: [(ModuleId, &str, Option<u8>, Option<&str>); 11] = [
        (clock, TRANSPORT, None, None),
        (cue_mod, CUE_PADS, None, None),
        (macros, "m1", Some(20), None),
        (macros, "m2", Some(21), None),
        (macros, "m3", Some(22), None),
        (macros, "m4", Some(23), None),
        (bass, BANKS, None, Some("Bass banks")),
        (arp, BANKS, None, Some("Arp banks")),
        (arp, "direction", None, Some("Arp direction")),
        (echo, "level1", Some(24), Some("Lead")),
        (bass, "transpose", None, Some("Bass transpose")),
    ];
    let mut changes = Vec::new();
    for (i, (id, key, cc, _)) in pins.iter().enumerate() {
        changes.push((*id, format!("{PIN_PREFIX}{key}"), Some(i as f32)));
        if let Some(cc) = cc {
            changes.push((*id, format!("{CC_PREFIX}{key}"), Some(*cc as f32)));
        }
    }
    for (n, cc) in (1..=5).zip(40..) {
        changes.push((cue_mod, format!("{BTN_PREFIX}cue.{n}"), Some(cc as f32)));
    }
    changes.push((cue_mod, format!("{BTN_PREFIX}cancel"), Some(45.0)));
    changes.push((clock, format!("{BTN_PREFIX}run"), Some(46.0)));
    changes.push((clock, format!("{BTN_PREFIX}restart"), Some(47.0)));
    e.set_presentation(&changes);
    for (id, key, _, label) in pins {
        if let Some(label) = label {
            e.set_label(id, &format!("{PIN_PREFIX}{key}"), Some(label.to_string()));
        }
    }
    e
}

/// Not a check: writes `patches/composition`.
/// `cargo test -p kabl-ui --test composition write_composition_patch -- --ignored`.
#[test]
#[ignore]
fn write_composition_patch() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/composition");
    let _ = std::fs::remove_dir_all(&dir);
    kabl_core::save(&dir, composition().log()).unwrap();
}

/// The committed patch is what `composition()` builds, every stored value is in range, it
/// compiles, and its cues and pins are complete.
#[test]
fn committed_composition_patch_matches_the_builder() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/composition");
    let saved = kabl_core::load(&dir).unwrap();
    assert_eq!(saved.state(), composition().state());
    for m in saved.state().modules.values() {
        let info = kabl_modules::registry::info_for(&m.kind).unwrap();
        for p in info.params {
            if let Some(&v) = m.params.get(p.name) {
                assert!((p.min..=p.max).contains(&v), "{} {} = {v}", m.kind, p.name);
            }
        }
    }
    kabl_engine::compile::compile(saved.state(), 48000.0, 8).expect("compiles");
    assert_eq!(kabl_ui::perform::pins(saved.state()).len(), 11);
    let cs = cues::cues(saved.state(), ids::CUES);
    let names: Vec<&str> = cs.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Intro", "Main", "Variation", "Breakdown", "Return"]);
    assert!(cs
        .iter()
        .all(|c| c.clock == Some(ids::CLOCK) && c.targets.len() == 2));
}
