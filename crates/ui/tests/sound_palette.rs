//! The sound palette patch library (`patches/palette/*`) and the piece patch
//! (`patches/sound-palette`), built here so every value is written down with its reason.
//!
//! Six playable patches, each with its own synthesis, not just its own reverb:
//! - `strings`: animated ensemble strings. Two saws (4- and 2-voice unison, the second an
//!   octave down), key-tracked ladder, slow bow envelope, vibrato, a slow filter drift, a
//!   stereo chorus.
//! - `pad`: warm evolving pad. A pulse with slow PWM, a detuned saw, a little pink noise, a
//!   driven ladder breathing on an 8-second triangle, a long envelope, echo and a big room.
//! - `lead`: singing mono lead. LEGATO with LAST priority and legato-only glide; a saw plus
//!   a hard-synced pulse whose sweep follows the envelope (a vowel on each attack), resonant
//!   ladder, drive, delayed-feel vibrato on a macro, ping-pong echo.
//! - `bass`: resonant sequence bass. Saw + square sub, ladder at high resonance with
//!   envelope and accent (velocity) into the cutoff, per-voice drive, dry.
//! - `breath`: airy wind/breath texture. Per-voice pink noise through a narrow, key-tracked
//!   band-pass (the note is the band), a quiet sine for the pitch centre, gusts from a slow
//!   LFO on the band, wide chorus, long room.
//! - `perc`: noise percussion. A pitched-sine kick, a noise + tone snare and a high-passed
//!   noise hat on three sequencers from one clock, a drum-bus drive, a short room.
//!
//! Each has four named macros (CC 20–23, channel 1), a Perform panel with readable labels,
//! and a measured headroom (`headroom_of_every_patch`, `-- --nocapture` prints the table).
//! The piece patch combines them: sequenced bass and percussion with cues for the sections,
//! and the four keyboard layers (breath, strings, pad, lead) on faders (CC 24–27), since
//! every `midi.in` hears every key.

use kabl_core::{ModuleId, PatchState, PortRef, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::{PatchEngine, Timing};
use kabl_modules::builtins::seq::bank_param;
use kabl_ui::perform::{BANKS, BTN_PREFIX, CC_PREFIX, CUE_PADS, PIN_PREFIX, TRANSPORT};
use kabl_ui::{banks, cues, PatchEditor};

fn port(id: ModuleId, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

/// Places modules left to right along rows; the rack packs them without overlap anyway.
struct Rack {
    e: PatchEditor,
    row: usize,
    x: f32,
}

impl Rack {
    fn new() -> Self {
        Rack {
            e: PatchEditor::new(),
            row: 0,
            x: 24.0,
        }
    }
    fn row(&mut self, row: usize) {
        self.row = row;
        self.x = 24.0;
    }
    fn add(&mut self, kind: &str, params: &[(&str, f32)]) -> ModuleId {
        let pos = Vec2 {
            x: self.x,
            y: kabl_ui::rack::row_y(self.row),
        };
        let id = self.e.add_module(kind, pos);
        let w = kabl_modules::registry::info_for(kind).unwrap().width_units as f32 * 30.0;
        self.x += w.max(150.0) + 30.0;
        for &(p, v) in params {
            self.e.set_param(id, p, v);
        }
        id
    }
    fn set(&mut self, id: ModuleId, params: &[(&str, f32)]) {
        for &(p, v) in params {
            self.e.set_param(id, p, v);
        }
    }
    fn wire(&mut self, from: (ModuleId, &str), to: (ModuleId, &str)) {
        self.e.connect(port(from.0, from.1), port(to.0, to.1));
    }
    fn route(&mut self, from: (ModuleId, &str), to: ModuleId, param: &str, amount: f32) {
        let r = self.e.connect_route(port(from.0, from.1), to, param);
        self.e.set_route_amount(r, amount, true);
    }
    /// A macro module with four named knobs at their starting values.
    fn macros(&mut self, names: [(&str, f32); 4]) -> ModuleId {
        let m = self.add("macro", &[]);
        for (k, (name, v)) in names.into_iter().enumerate() {
            let key = format!("m{}", k + 1);
            self.e
                .set_label(m, &format!("name.{key}"), Some(name.into()));
            self.e.set_param(m, &key, v);
        }
        m
    }
    /// Perform panel pins in order, with CC numbers (channel 1) and labels.
    fn pins(&mut self, pins: &[(ModuleId, &str, Option<u8>, Option<&str>)]) {
        let mut changes = Vec::new();
        for (i, (id, key, cc, _)) in pins.iter().enumerate() {
            changes.push((*id, format!("{PIN_PREFIX}{key}"), Some(i as f32)));
            if let Some(cc) = cc {
                changes.push((*id, format!("{CC_PREFIX}{key}"), Some(*cc as f32)));
            }
        }
        self.e.set_presentation(&changes);
        for (id, key, _, label) in pins {
            if let Some(label) = label {
                self.e
                    .set_label(*id, &format!("{PIN_PREFIX}{key}"), Some(label.to_string()));
            }
        }
    }
}

/// A sequencer bank: pitches, gates, velocities, length, LENGTH gate %.
struct Bank {
    name: &'static str,
    p: [f32; 8],
    g: [u8; 8],
    v: [f32; 8],
    len: usize,
    gate_len: f32,
}

fn write_bank(r: &mut Rack, seq: ModuleId, b: usize, bank: &Bank) {
    for k in 0..8 {
        r.e.set_param(seq, bank_param(b, k), bank.p[k]);
        if bank.g[k] == 0 {
            r.e.set_param(seq, bank_param(b, 8 + k), 0.0);
        }
        r.e.set_param(seq, bank_param(b, 17 + k), bank.v[k]);
    }
    r.e.set_param(seq, bank_param(b, 16), bank.len as f32);
    r.e.set_param(seq, bank_param(b, 26), 1.0); // LENGTH gates
    r.e.set_param(seq, bank_param(b, 25), bank.gate_len);
    banks::rename(&mut r.e, seq, b, bank.name);
}

const SILENT: Bank = Bank {
    name: "Rest",
    p: [0.0; 8],
    g: [0; 8],
    v: [100.0; 8],
    len: 8,
    gate_len: 50.0,
};

// ---------------------------------------------------------------------------------------------
// Voices. Each returns the ids a patch's macros and pins reach; the mono voice output is
// `gain`'s `out` (the voice sum is the voice average, so a gain brings one note up to level).

struct Strings {
    midi: ModuleId,
    osc1: ModuleId,
    filt: ModuleId,
    env: ModuleId,
    gain: ModuleId,
}

fn strings(r: &mut Rack, row: usize) -> Strings {
    r.row(row);
    let midi = r.add("midi.in", &[]);
    // Two saws: 4-voice unison at the note and 2-voice an octave down, slightly flat, so the
    // ensemble beats slowly instead of phasing in step.
    let osc1 = r.add(
        "osc.va",
        &[("waveform", 2.0), ("unison", 4.0), ("detune", 35.0)],
    );
    let osc2 = r.add(
        "osc.va",
        &[
            ("waveform", 2.0),
            ("base_hz", 130.81),
            ("unison", 2.0),
            ("detune", 20.0),
            ("fine", -6.0),
        ],
    );
    let mix = r.add("mixer", &[("level1", 0.5), ("level2", 0.35)]);
    let filt = r.add(
        "filter.ladder",
        &[
            ("cutoff_hz", 2400.0),
            ("resonance", 0.12),
            ("drive_db", 2.0),
        ],
    );
    r.row(row + 1);
    let env = r.add(
        "env.adsr",
        &[
            ("attack_ms", 280.0),
            ("decay_ms", 600.0),
            ("sustain", 0.85),
            ("release_ms", 1100.0),
        ],
    );
    let vel = r.add("ringmod", &[]);
    let vca = r.add("vca", &[("gain", 0.0)]);
    let gain = r.add("gain", &[("gain_db", 5.0)]);
    // Motion: a slow triangle drifts the filter; a 5.3 Hz sine is the vibrato.
    let drift = r.add("lfo", &[("rate_hz", 0.23), ("waveform", 1.0)]);
    let vib = r.add("lfo", &[("rate_hz", 5.3)]);
    r.wire((midi, "pitch"), (osc1, "pitch"));
    r.wire((midi, "pitch"), (osc2, "pitch"));
    r.wire((midi, "gate"), (env, "gate"));
    r.wire((osc1, "out"), (mix, "in1"));
    r.wire((osc2, "out"), (mix, "in2"));
    r.wire((mix, "out"), (filt, "in"));
    r.wire((filt, "out"), (vca, "in"));
    r.wire((env, "out"), (vel, "a"));
    r.wire((midi, "velocity"), (vel, "b"));
    r.wire((vel, "out"), (vca, "cv"));
    r.wire((vca, "out"), (gain, "in"));
    // Key tracking at 70 % (0.502 is 1:1 on the ladder's 20 Hz–20 kHz taper), the bow
    // envelope opens the filter a little, velocity a little more.
    r.route((midi, "pitch"), filt, "cutoff_hz", 0.35);
    r.route((env, "out"), filt, "cutoff_hz", 0.08);
    r.route((midi, "velocity"), filt, "cutoff_hz", 0.08);
    r.route((drift, "out"), filt, "cutoff_hz", 0.05);
    r.route((vib, "out"), osc1, "fine", 0.05);
    r.route((vib, "out"), osc2, "fine", 0.05);
    Strings {
        midi,
        osc1,
        filt,
        env,
        gain,
    }
}

struct Pad {
    midi: ModuleId,
    noise: ModuleId,
    filt: ModuleId,
    evolve: ModuleId,
    env: ModuleId,
    gain: ModuleId,
}

fn pad(r: &mut Rack, row: usize, macro_src: (ModuleId, &str)) -> Pad {
    r.row(row);
    let midi = r.add("midi.in", &[]);
    let osc1 = r.add(
        "osc.va",
        &[
            ("waveform", 3.0),
            ("pw", 35.0),
            ("unison", 2.0),
            ("detune", 12.0),
        ],
    );
    let osc2 = r.add("osc.va", &[("waveform", 2.0), ("fine", -9.0)]);
    let noise = r.add("noise", &[("color", 1.0), ("level_db", -30.0)]);
    let mix = r.add(
        "mixer",
        &[("level1", 0.45), ("level2", 0.35), ("level3", 1.0)],
    );
    let filt = r.add(
        "filter.ladder",
        &[("cutoff_hz", 650.0), ("resonance", 0.35), ("drive_db", 4.0)],
    );
    r.row(row + 1);
    let env = r.add(
        "env.adsr",
        &[
            ("attack_ms", 1400.0),
            ("decay_ms", 2000.0),
            ("sustain", 0.9),
            ("release_ms", 2800.0),
        ],
    );
    let vca = r.add("vca", &[("gain", 0.0)]);
    let gain = r.add("gain", &[("gain_db", -3.0)]);
    // PWM from a slow sine; the filter breathes on an 8 s triangle scaled by a macro.
    let pwm = r.add("lfo", &[("rate_hz", 0.11)]);
    let breathe = r.add("lfo", &[("rate_hz", 0.125), ("waveform", 1.0)]);
    let evolve = r.add("ringmod", &[]);
    r.wire((midi, "pitch"), (osc1, "pitch"));
    r.wire((midi, "pitch"), (osc2, "pitch"));
    r.wire((midi, "gate"), (env, "gate"));
    r.wire((osc1, "out"), (mix, "in1"));
    r.wire((osc2, "out"), (mix, "in2"));
    r.wire((noise, "out"), (mix, "in3"));
    r.wire((mix, "out"), (filt, "in"));
    r.wire((filt, "out"), (vca, "in"));
    r.wire((env, "out"), (vca, "cv"));
    r.wire((vca, "out"), (gain, "in"));
    r.wire((breathe, "out"), (evolve, "a"));
    r.wire(macro_src, (evolve, "b"));
    r.route((pwm, "out"), osc1, "pw", 0.25);
    r.route((evolve, "out"), filt, "cutoff_hz", 0.12);
    r.route((midi, "pitch"), filt, "cutoff_hz", 0.2);
    r.route((env, "out"), filt, "cutoff_hz", 0.06);
    Pad {
        midi,
        noise,
        filt,
        evolve,
        env,
        gain,
    }
}

struct Lead {
    midi: ModuleId,
    filt: ModuleId,
    drive: ModuleId,
    vibrato: ModuleId,
    gain: ModuleId,
}

fn lead(r: &mut Rack, row: usize, macro_src: (ModuleId, &str)) -> Lead {
    r.row(row);
    let midi = r.add(
        "midi.in",
        &[("mode", 2.0), ("glide", 2.0), ("glide_ms", 90.0)],
    );
    let osc1 = r.add("osc.va", &[("waveform", 2.0)]);
    // A pulse synced to the saw, tuned a fifth up; the envelope sweeps it on each attack.
    let osc2 = r.add(
        "osc.va",
        &[("waveform", 3.0), ("pw", 45.0), ("base_hz", 392.0)],
    );
    let mix = r.add("mixer", &[("level1", 0.5), ("level2", 0.3)]);
    let filt = r.add(
        "filter.ladder",
        &[
            ("cutoff_hz", 1700.0),
            ("resonance", 0.45),
            ("drive_db", 6.0),
        ],
    );
    r.row(row + 1);
    let env = r.add(
        "env.adsr",
        &[
            ("attack_ms", 30.0),
            ("decay_ms", 700.0),
            ("sustain", 0.7),
            ("release_ms", 450.0),
        ],
    );
    let vca = r.add("vca", &[("gain", 0.0)]);
    let gain = r.add("gain", &[("gain_db", 20.0)]);
    let drive = r.add(
        "drive",
        &[("drive_db", 8.0), ("mix", 60.0), ("trim_db", -2.0)],
    );
    let vib = r.add("lfo", &[("rate_hz", 5.2)]);
    let vibrato = r.add("ringmod", &[]);
    r.wire((midi, "pitch"), (osc1, "pitch"));
    r.wire((midi, "pitch"), (osc2, "pitch"));
    r.wire((osc1, "out"), (osc2, "sync"));
    r.wire((midi, "gate"), (env, "gate"));
    r.wire((osc1, "out"), (mix, "in1"));
    r.wire((osc2, "out"), (mix, "in2"));
    r.wire((mix, "out"), (filt, "in"));
    r.wire((filt, "out"), (vca, "in"));
    r.wire((env, "out"), (vca, "cv"));
    r.wire((vca, "out"), (gain, "in"));
    r.wire((gain, "out"), (drive, "in"));
    r.wire((vib, "out"), (vibrato, "a"));
    r.wire(macro_src, (vibrato, "b"));
    r.route((env, "out"), osc2, "base_hz", 0.08);
    r.route((env, "out"), filt, "cutoff_hz", 0.18);
    r.route((midi, "pitch"), filt, "cutoff_hz", 0.25);
    r.route((midi, "velocity"), filt, "cutoff_hz", 0.1);
    r.route((vibrato, "out"), osc1, "fine", 0.12);
    r.route((vibrato, "out"), osc2, "fine", 0.12);
    Lead {
        midi,
        filt,
        drive,
        vibrato,
        gain,
    }
}

struct Bass {
    seq: ModuleId,
    filt: ModuleId,
    env: ModuleId,
    drive: ModuleId,
}

fn bass(r: &mut Rack, row: usize, clock: ModuleId) -> Bass {
    r.row(row);
    let seq = r.add("seq", &[]);
    let osc1 = r.add("osc.va", &[("waveform", 2.0), ("base_hz", 65.41)]);
    let osc2 = r.add("osc.va", &[("waveform", 3.0), ("base_hz", 32.70)]);
    let mix = r.add("mixer", &[("level1", 0.55), ("level2", 0.35)]);
    r.row(row + 1);
    let filt = r.add(
        "filter.ladder",
        &[("cutoff_hz", 240.0), ("resonance", 0.72), ("drive_db", 9.0)],
    );
    let env = r.add(
        "env.adsr",
        &[
            ("attack_ms", 1.0),
            ("decay_ms", 220.0),
            ("sustain", 0.15),
            ("release_ms", 120.0),
        ],
    );
    let accent = r.add("ringmod", &[]);
    let vca = r.add("vca", &[("gain", 0.0)]);
    let drive = r.add(
        "drive",
        &[("drive_db", 9.0), ("mix", 55.0), ("trim_db", -3.0)],
    );
    r.wire((clock, "gate"), (seq, "clock"));
    r.wire((clock, "reset"), (seq, "reset"));
    r.wire((seq, "pitch"), (osc1, "pitch"));
    r.wire((seq, "pitch"), (osc2, "pitch"));
    r.wire((seq, "gate"), (env, "gate"));
    r.wire((osc1, "out"), (mix, "in1"));
    r.wire((osc2, "out"), (mix, "in2"));
    r.wire((mix, "out"), (filt, "in"));
    r.wire((filt, "out"), (vca, "in"));
    r.wire((env, "out"), (accent, "a"));
    r.wire((seq, "velocity"), (accent, "b"));
    r.wire((accent, "out"), (vca, "cv"));
    r.wire((vca, "out"), (drive, "in"));
    r.route((env, "out"), filt, "cutoff_hz", 0.28);
    r.route((seq, "velocity"), filt, "cutoff_hz", 0.12);
    // E minor, C2 = 0 st (the oscillators sit at C2 and C1, the steps reach ±24 st): the line, an octave-jumping lift, a held breakdown, rests.
    let banks = [
        Bank {
            name: "Line",
            p: [-8.0, -8.0, 4.0, -8.0, -3.0, -8.0, -1.0, -5.0],
            g: [1, 1, 1, 0, 1, 1, 1, 1],
            v: [100.0, 45.0, 85.0, 40.0, 90.0, 50.0, 75.0, 60.0],
            len: 8,
            gate_len: 45.0,
        },
        Bank {
            name: "Lift",
            p: [-3.0, 9.0, -3.0, 4.0, 0.0, 12.0, 2.0, 4.0],
            g: [1; 8],
            v: [100.0, 60.0, 80.0, 55.0, 95.0, 65.0, 80.0, 70.0],
            len: 8,
            gate_len: 40.0,
        },
        Bank {
            name: "Hold",
            p: [-8.0, -5.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            g: [1, 1, 0, 0, 0, 0, 0, 0],
            v: [60.0, 50.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0],
            len: 2,
            gate_len: 95.0,
        },
        SILENT,
    ];
    for (b, bank) in banks.iter().enumerate() {
        write_bank(r, seq, b, bank);
    }
    Bass {
        seq,
        filt,
        env,
        drive,
    }
}

struct Breath {
    midi: ModuleId,
    band: ModuleId,
    air: ModuleId,
    mix: ModuleId,
    gust: ModuleId,
    gain: ModuleId,
}

fn breath(r: &mut Rack, row: usize, macro_src: (ModuleId, &str)) -> Breath {
    r.row(row);
    let midi = r.add("midi.in", &[]);
    // One pink noise module feeding only the voice chain, so every voice has its own stream.
    let noise = r.add("noise", &[("color", 1.0)]);
    let band = r.add("filter.svf", &[("cutoff_hz", 261.63), ("resonance", 0.88)]);
    let air = r.add(
        "filter.ladder",
        &[("cutoff_hz", 6000.0), ("resonance", 0.1)],
    );
    let tone = r.add("osc.va", &[("waveform", 0.0)]);
    let mix = r.add(
        "mixer",
        &[("level1", 1.0), ("level2", 0.12), ("level3", 0.35)],
    );
    r.row(row + 1);
    let env = r.add(
        "env.adsr",
        &[
            ("attack_ms", 450.0),
            ("decay_ms", 900.0),
            ("sustain", 0.8),
            ("release_ms", 1500.0),
        ],
    );
    let vca = r.add("vca", &[("gain", 0.0)]);
    let gain = r.add("gain", &[("gain_db", 2.0)]);
    let lfo = r.add("lfo", &[("rate_hz", 0.13)]);
    let gust = r.add("ringmod", &[]);
    r.wire((midi, "pitch"), (tone, "pitch"));
    r.wire((midi, "gate"), (env, "gate"));
    r.wire((noise, "out"), (band, "in"));
    r.wire((noise, "out"), (air, "in"));
    r.wire((band, "bp"), (mix, "in1"));
    r.wire((tone, "out"), (mix, "in2"));
    r.wire((air, "out"), (mix, "in3"));
    r.wire((mix, "out"), (vca, "in"));
    r.wire((env, "out"), (vca, "cv"));
    r.wire((vca, "out"), (gain, "in"));
    r.wire((lfo, "out"), (gust, "a"));
    r.wire(macro_src, (gust, "b"));
    // The band follows the key 1:1 (0.502 on a 20 Hz–20 kHz taper), so the note is played.
    r.route((midi, "pitch"), band, "cutoff_hz", 0.502);
    r.route((gust, "out"), band, "cutoff_hz", 0.05);
    r.route((gust, "out"), air, "cutoff_hz", 0.12);
    r.route((env, "out"), air, "cutoff_hz", 0.1);
    Breath {
        midi,
        band,
        air,
        mix,
        gust,
        gain,
    }
}

struct Perc {
    kick_seq: ModuleId,
    snare_seq: ModuleId,
    hat_seq: ModuleId,
    kick_env: ModuleId,
    snare_env: ModuleId,
    hat_env: ModuleId,
    snare_bp: ModuleId,
    hat_hp: ModuleId,
    bus: ModuleId,
    drive: ModuleId,
}

fn perc(r: &mut Rack, row: usize, clock: ModuleId) -> Perc {
    r.row(row);
    let kick_seq = r.add("seq", &[]);
    let snare_seq = r.add("seq", &[]);
    let hat_seq = r.add("seq", &[]);
    r.row(row + 1);
    // Kick: a sine at 46 Hz, swept down from ~2.5 octaves above by a 45 ms envelope.
    let kick = r.add("osc.va", &[("waveform", 0.0), ("base_hz", 46.0)]);
    let kick_pitch = r.add(
        "env.adsr",
        &[
            ("attack_ms", 0.1),
            ("decay_ms", 45.0),
            ("sustain", 0.0),
            ("release_ms", 40.0),
        ],
    );
    let kick_env = r.add(
        "env.adsr",
        &[
            ("attack_ms", 0.5),
            ("decay_ms", 280.0),
            ("sustain", 0.0),
            ("release_ms", 150.0),
        ],
    );
    let kick_vel = r.add("ringmod", &[]);
    let kick_vca = r.add("vca", &[("gain", 0.0)]);
    // Snare: white noise through a 1.9 kHz band plus a 185 Hz body.
    let snare_noise = r.add("noise", &[]);
    let snare_bp = r.add("filter.svf", &[("cutoff_hz", 1900.0), ("resonance", 0.3)]);
    let snare_body = r.add("osc.va", &[("waveform", 0.0), ("base_hz", 185.0)]);
    r.row(row + 2);
    let snare_env = r.add(
        "env.adsr",
        &[
            ("attack_ms", 0.3),
            ("decay_ms", 150.0),
            ("sustain", 0.0),
            ("release_ms", 90.0),
        ],
    );
    let snare_mix = r.add("mixer", &[("level1", 1.0), ("level2", 0.35)]);
    let snare_vel = r.add("ringmod", &[]);
    let snare_vca = r.add("vca", &[("gain", 0.0)]);
    // Hat: white noise above 7 kHz, 35 ms.
    let hat_noise = r.add("noise", &[]);
    let hat_hp = r.add("filter.svf", &[("cutoff_hz", 7000.0), ("resonance", 0.2)]);
    let hat_env = r.add(
        "env.adsr",
        &[
            ("attack_ms", 0.2),
            ("decay_ms", 35.0),
            ("sustain", 0.0),
            ("release_ms", 30.0),
        ],
    );
    r.row(row + 3);
    let hat_vel = r.add("ringmod", &[]);
    let hat_vca = r.add("vca", &[("gain", 0.0)]);
    let bus = r.add(
        "mixer",
        &[("level1", 0.55), ("level2", 0.8), ("level3", 0.6)],
    );
    let drive = r.add(
        "drive",
        &[("drive_db", 6.0), ("mix", 50.0), ("trim_db", -6.5)],
    );
    // The band-passed noise is quiet next to the kick's sine: +9 dB brings the snare up.
    let snare_gain = r.add("gain", &[("gain_db", 9.0)]);
    for s in [kick_seq, snare_seq, hat_seq] {
        r.wire((clock, "gate"), (s, "clock"));
        r.wire((clock, "reset"), (s, "reset"));
    }
    r.wire((kick_seq, "gate"), (kick_pitch, "gate"));
    r.wire((kick_seq, "gate"), (kick_env, "gate"));
    r.route((kick_pitch, "out"), kick, "base_hz", 0.25);
    r.wire((kick, "out"), (kick_vca, "in"));
    r.wire((kick_env, "out"), (kick_vel, "a"));
    r.wire((kick_seq, "velocity"), (kick_vel, "b"));
    r.wire((kick_vel, "out"), (kick_vca, "cv"));
    r.wire((snare_seq, "gate"), (snare_env, "gate"));
    r.wire((snare_noise, "out"), (snare_bp, "in"));
    r.wire((snare_bp, "bp"), (snare_mix, "in1"));
    r.wire((snare_body, "out"), (snare_mix, "in2"));
    r.wire((snare_mix, "out"), (snare_vca, "in"));
    r.wire((snare_env, "out"), (snare_vel, "a"));
    r.wire((snare_seq, "velocity"), (snare_vel, "b"));
    r.wire((snare_vel, "out"), (snare_vca, "cv"));
    r.wire((hat_seq, "gate"), (hat_env, "gate"));
    r.wire((hat_noise, "out"), (hat_hp, "in"));
    r.wire((hat_hp, "hp"), (hat_vca, "in"));
    r.wire((hat_env, "out"), (hat_vel, "a"));
    r.wire((hat_seq, "velocity"), (hat_vel, "b"));
    r.wire((hat_vel, "out"), (hat_vca, "cv"));
    r.wire((kick_vca, "out"), (bus, "in1"));
    r.wire((snare_vca, "out"), (snare_gain, "in"));
    r.wire((snare_gain, "out"), (bus, "in2"));
    r.wire((hat_vca, "out"), (bus, "in3"));
    r.wire((bus, "out"), (drive, "in"));
    // Patterns (8 sixteenths = half a bar). The pitch knobs are unused by the drums.
    let v = |x: [f32; 8]| x;
    let kick_banks = [
        Bank {
            name: "Four",
            p: [0.0; 8],
            g: [1, 0, 0, 0, 1, 0, 0, 0],
            v: v([100.0; 8]),
            len: 8,
            gate_len: 50.0,
        },
        Bank {
            name: "Push",
            p: [0.0; 8],
            g: [1, 0, 0, 1, 1, 0, 1, 0],
            v: v([100.0, 50.0, 50.0, 60.0, 100.0, 50.0, 70.0, 50.0]),
            len: 8,
            gate_len: 50.0,
        },
        Bank {
            name: "Pulse",
            p: [0.0; 8],
            g: [1, 0, 0, 0, 0, 0, 0, 0],
            v: v([70.0; 8]),
            len: 8,
            gate_len: 50.0,
        },
        SILENT,
    ];
    let snare_banks = [
        Bank {
            name: "Back",
            p: [0.0; 8],
            g: [0, 0, 0, 0, 1, 0, 0, 0],
            v: v([100.0; 8]),
            len: 8,
            gate_len: 50.0,
        },
        Bank {
            name: "Ghost",
            p: [0.0; 8],
            g: [0, 0, 1, 0, 1, 0, 0, 1],
            v: v([30.0, 30.0, 35.0, 30.0, 100.0, 30.0, 30.0, 40.0]),
            len: 8,
            gate_len: 50.0,
        },
        SILENT,
        SILENT,
    ];
    let hat_banks = [
        Bank {
            name: "Eighths",
            p: [0.0; 8],
            g: [0, 0, 1, 0, 0, 0, 1, 0],
            v: v([60.0, 40.0, 80.0, 40.0, 60.0, 40.0, 80.0, 40.0]),
            len: 8,
            gate_len: 50.0,
        },
        Bank {
            name: "Run",
            p: [0.0; 8],
            g: [1; 8],
            v: v([70.0, 35.0, 90.0, 40.0, 65.0, 35.0, 85.0, 45.0]),
            len: 8,
            gate_len: 50.0,
        },
        Bank {
            name: "Tick",
            p: [0.0; 8],
            g: [0, 0, 1, 0, 0, 0, 0, 0],
            v: v([40.0; 8]),
            len: 8,
            gate_len: 50.0,
        },
        SILENT,
    ];
    for (seq, bs) in [
        (kick_seq, &kick_banks),
        (snare_seq, &snare_banks),
        (hat_seq, &hat_banks),
    ] {
        for (b, bank) in bs.iter().enumerate() {
            write_bank(r, seq, b, bank);
        }
    }
    Perc {
        kick_seq,
        snare_seq,
        hat_seq,
        kick_env,
        snare_env,
        hat_env,
        snare_bp,
        hat_hp,
        bus,
        drive,
    }
}

// ---------------------------------------------------------------------------------------------
// Patches.

/// `out` both sides from a stereo pair.
fn out_stereo(r: &mut Rack, l: (ModuleId, &str), rr: (ModuleId, &str)) -> ModuleId {
    let out = r.add("out", &[]);
    r.wire(l, (out, "left"));
    r.wire(rr, (out, "right"));
    out
}

pub fn strings_patch() -> PatchEditor {
    let mut r = Rack::new();
    let v = strings(&mut r, 0);
    r.row(2);
    let chorus = r.add(
        "chorus",
        &[
            ("rate_hz", 0.35),
            ("depth", 65.0),
            ("mix", 55.0),
            ("width", 100.0),
        ],
    );
    let reverb = r.add(
        "reverb",
        &[
            ("decay_s", 2.2),
            ("mix", 18.0),
            ("predelay_ms", 20.0),
            ("damp_hz", 6000.0),
        ],
    );
    r.wire((v.gain, "out"), (chorus, "in_l"));
    r.wire((v.gain, "out"), (chorus, "in_r"));
    r.wire((chorus, "left"), (reverb, "in_l"));
    r.wire((chorus, "right"), (reverb, "in_r"));
    out_stereo(&mut r, (reverb, "left"), (reverb, "right"));
    let m = r.macros([
        ("Bright", 0.4),
        ("Ensemble", 0.5),
        ("Bow", 0.3),
        ("Space", 0.3),
    ]);
    r.route((m, "m1"), v.filt, "cutoff_hz", 0.25);
    r.route((m, "m2"), chorus, "depth", 0.3);
    r.route((m, "m2"), chorus, "mix", 0.2);
    r.route((m, "m2"), v.osc1, "detune", 0.3);
    r.route((m, "m3"), v.env, "attack_ms", 0.14);
    r.route((m, "m3"), v.env, "release_ms", 0.08);
    r.route((m, "m4"), reverb, "mix", 0.3);
    r.route((m, "m4"), reverb, "decay_s", 0.15);
    r.pins(&[
        (m, "m1", Some(20), None),
        (m, "m2", Some(21), None),
        (m, "m3", Some(22), None),
        (m, "m4", Some(23), None),
        (chorus, "rate_hz", Some(24), Some("Chorus rate")),
        (v.env, "release_ms", Some(25), Some("Release")),
        (v.gain, "gain_db", Some(26), Some("Level")),
        (v.midi, "mode", None, Some("Keys")),
    ]);
    r.e
}

pub fn pad_patch() -> PatchEditor {
    let mut r = Rack::new();
    r.row(4);
    let m = r.macros([
        ("Warmth", 0.4),
        ("Evolve", 0.5),
        ("Air", 0.3),
        ("Space", 0.4),
    ]);
    let v = pad(&mut r, 0, (m, "m2"));
    r.row(2);
    let chorus = r.add(
        "chorus",
        &[("rate_hz", 0.18), ("depth", 40.0), ("mix", 35.0)],
    );
    let delay = r.add(
        "delay",
        &[
            ("sync", 0.0),
            ("time_ms", 480.0),
            ("feedback", 45.0),
            ("mix", 18.0),
            ("tone_hz", 2500.0),
            ("mode", 1.0),
        ],
    );
    let bus_l = r.add("mixer", &[]);
    let bus_r = r.add("mixer", &[]);
    r.row(3);
    let reverb = r.add(
        "reverb",
        &[
            ("decay_s", 5.5),
            ("mix", 32.0),
            ("predelay_ms", 35.0),
            ("damp_hz", 4200.0),
        ],
    );
    r.wire((v.gain, "out"), (chorus, "in_l"));
    r.wire((v.gain, "out"), (chorus, "in_r"));
    r.wire((v.gain, "out"), (delay, "in"));
    r.wire((chorus, "left"), (bus_l, "in1"));
    r.wire((chorus, "right"), (bus_r, "in1"));
    r.wire((delay, "left"), (bus_l, "in2"));
    r.wire((delay, "right"), (bus_r, "in2"));
    r.set(bus_l, &[("level2", 0.5)]);
    r.set(bus_r, &[("level2", 0.5)]);
    r.wire((bus_l, "out"), (reverb, "in_l"));
    r.wire((bus_r, "out"), (reverb, "in_r"));
    out_stereo(&mut r, (reverb, "left"), (reverb, "right"));
    r.route((m, "m1"), v.filt, "cutoff_hz", 0.2);
    r.route((m, "m1"), v.filt, "drive_db", 0.3);
    r.route((m, "m3"), v.noise, "level_db", 0.35);
    r.route((m, "m4"), reverb, "mix", 0.3);
    r.route((m, "m4"), delay, "mix", 0.25);
    let _ = v.evolve;
    r.pins(&[
        (m, "m1", Some(20), None),
        (m, "m2", Some(21), None),
        (m, "m3", Some(22), None),
        (m, "m4", Some(23), None),
        (v.env, "attack_ms", Some(24), Some("Attack")),
        (v.env, "release_ms", Some(25), Some("Release")),
        (v.gain, "gain_db", Some(26), Some("Level")),
        (v.midi, "mode", None, Some("Keys")),
    ]);
    r.e
}

pub fn lead_patch() -> PatchEditor {
    let mut r = Rack::new();
    r.row(3);
    let m = r.macros([
        ("Tone", 0.5),
        ("Vibrato", 0.3),
        ("Bite", 0.3),
        ("Echo", 0.4),
    ]);
    let v = lead(&mut r, 0, (m, "m2"));
    r.row(2);
    let delay = r.add(
        "delay",
        &[
            ("sync", 0.0),
            ("time_ms", 375.0),
            ("feedback", 35.0),
            ("mix", 22.0),
            ("tone_hz", 3500.0),
            ("mode", 1.0),
        ],
    );
    let reverb = r.add(
        "reverb",
        &[("decay_s", 2.8), ("mix", 20.0), ("predelay_ms", 25.0)],
    );
    r.wire((v.drive, "out"), (delay, "in"));
    r.wire((delay, "left"), (reverb, "in_l"));
    r.wire((delay, "right"), (reverb, "in_r"));
    out_stereo(&mut r, (reverb, "left"), (reverb, "right"));
    r.route((m, "m1"), v.filt, "cutoff_hz", 0.2);
    r.route((m, "m3"), v.drive, "drive_db", 0.3);
    r.route((m, "m3"), v.filt, "resonance", 0.15);
    r.route((m, "m4"), delay, "mix", 0.25);
    r.route((m, "m4"), delay, "feedback", 0.15);
    let _ = v.vibrato;
    r.pins(&[
        (m, "m1", Some(20), None),
        (m, "m2", Some(21), None),
        (m, "m3", Some(22), None),
        (m, "m4", Some(23), None),
        (v.midi, "glide_ms", Some(24), Some("Glide time")),
        (v.gain, "gain_db", Some(26), Some("Level")),
        (v.midi, "mode", None, Some("Keys")),
        (v.midi, "glide", None, Some("Glide")),
    ]);
    r.e
}

pub fn bass_patch() -> PatchEditor {
    let mut r = Rack::new();
    r.row(3);
    let clock = r.add("clock", &[("bpm", 112.0)]);
    let m = r.macros([
        ("Cutoff", 0.35),
        ("Resonance", 0.4),
        ("Accent", 0.4),
        ("Drive", 0.4),
    ]);
    let v = bass(&mut r, 0, clock);
    r.row(2);
    let level = r.add("gain", &[("gain_db", -2.0)]);
    r.wire((v.drive, "out"), (level, "in"));
    out_stereo(&mut r, (level, "out"), (level, "out"));
    r.route((m, "m1"), v.filt, "cutoff_hz", 0.25);
    r.route((m, "m2"), v.filt, "resonance", 0.25);
    r.route((m, "m3"), v.env, "decay_ms", 0.15);
    r.route((m, "m3"), v.filt, "drive_db", 0.25);
    r.route((m, "m4"), v.drive, "drive_db", 0.35);
    r.pins(&[
        (clock, TRANSPORT, None, None),
        (m, "m1", Some(20), None),
        (m, "m2", Some(21), None),
        (m, "m3", Some(22), None),
        (m, "m4", Some(23), None),
        (v.seq, BANKS, None, Some("Bass banks")),
        (v.seq, "transpose", None, Some("Transpose")),
        (clock, "bpm", Some(25), Some("Tempo")),
        (level, "gain_db", Some(26), Some("Level")),
    ]);
    r.e
}

pub fn breath_patch() -> PatchEditor {
    let mut r = Rack::new();
    r.row(3);
    let m = r.macros([
        ("Breath", 0.5),
        ("Gust", 0.5),
        ("Color", 0.4),
        ("Space", 0.5),
    ]);
    let v = breath(&mut r, 0, (m, "m2"));
    r.row(2);
    let chorus = r.add(
        "chorus",
        &[
            ("rate_hz", 0.6),
            ("depth", 80.0),
            ("mix", 40.0),
            ("width", 100.0),
        ],
    );
    let reverb = r.add(
        "reverb",
        &[("decay_s", 4.5), ("mix", 35.0), ("predelay_ms", 40.0)],
    );
    r.wire((v.gain, "out"), (chorus, "in_l"));
    r.wire((v.gain, "out"), (chorus, "in_r"));
    r.wire((chorus, "left"), (reverb, "in_l"));
    r.wire((chorus, "right"), (reverb, "in_r"));
    out_stereo(&mut r, (reverb, "left"), (reverb, "right"));
    // Breath: more of the airy wide band; Color: a narrower, brighter band.
    r.route((m, "m1"), v.mix, "level3", 0.06);
    r.route((m, "m3"), v.band, "resonance", 0.08);
    r.route((m, "m3"), v.air, "cutoff_hz", 0.15);
    r.route((m, "m4"), reverb, "mix", 0.3);
    r.route((m, "m4"), reverb, "decay_s", 0.1);
    let _ = v.gust;
    r.pins(&[
        (m, "m1", Some(20), None),
        (m, "m2", Some(21), None),
        (m, "m3", Some(22), None),
        (m, "m4", Some(23), None),
        (chorus, "width", Some(24), Some("Width")),
        (v.gain, "gain_db", Some(26), Some("Level")),
        (v.midi, "mode", None, Some("Keys")),
    ]);
    r.e
}

pub fn perc_patch() -> PatchEditor {
    let mut r = Rack::new();
    r.row(4);
    let clock = r.add("clock", &[("bpm", 112.0)]);
    let m = r.macros([
        ("Decay", 0.4),
        ("Tone", 0.5),
        ("Crunch", 0.4),
        ("Room", 0.3),
    ]);
    let v = perc(&mut r, 0, clock);
    let reverb = r.add(
        "reverb",
        &[
            ("decay_s", 0.9),
            ("mix", 12.0),
            ("damp_hz", 7000.0),
            ("predelay_ms", 8.0),
        ],
    );
    r.wire((v.drive, "out"), (reverb, "in_l"));
    r.wire((v.drive, "out"), (reverb, "in_r"));
    out_stereo(&mut r, (reverb, "left"), (reverb, "right"));
    macro_perc(&mut r, m, &v, reverb);
    r.pins(&[
        (clock, TRANSPORT, None, None),
        (m, "m1", Some(20), None),
        (m, "m2", Some(21), None),
        (m, "m3", Some(22), None),
        (m, "m4", Some(23), None),
        (v.kick_seq, BANKS, None, Some("Kick")),
        (v.snare_seq, BANKS, None, Some("Snare")),
        (v.hat_seq, BANKS, None, Some("Hat")),
        (v.bus, "level3", Some(24), Some("Hat level")),
    ]);
    r.e
}

fn macro_perc(r: &mut Rack, m: ModuleId, v: &Perc, reverb: ModuleId) {
    r.route((m, "m1"), v.kick_env, "decay_ms", 0.1);
    r.route((m, "m1"), v.snare_env, "decay_ms", 0.12);
    r.route((m, "m1"), v.hat_env, "decay_ms", 0.15);
    r.route((m, "m2"), v.snare_bp, "cutoff_hz", 0.12);
    r.route((m, "m2"), v.hat_hp, "cutoff_hz", 0.1);
    r.route((m, "m3"), v.drive, "drive_db", 0.25);
    r.route((m, "m4"), reverb, "mix", 0.3);
    r.route((m, "m4"), reverb, "decay_s", 0.15);
}

/// The piece: sequenced bass and percussion on one clock, cued by section; four keyboard
/// layers on faders; shared chorus, echo and plate.
pub fn piece_patch() -> PatchEditor {
    let mut r = Rack::new();
    r.row(14);
    let clock = r.add("clock", &[("bpm", 104.0)]);
    let cue_mod = r.add("cues", &[]);
    let m = r.macros([
        ("Energy", 0.3),
        ("Motion", 0.4),
        ("Bright", 0.4),
        ("Space", 0.4),
    ]);
    let p = perc(&mut r, 0, clock);
    let b = bass(&mut r, 4, clock);
    let br = breath(&mut r, 6, (m, "m2"));
    let s = strings(&mut r, 8);
    let pd = pad(&mut r, 10, (m, "m2"));
    let ld = lead(&mut r, 12, (m, "m2"));
    // Startup: the sequences rest (bank D) so the piece opens on the atmosphere.
    for seq in [p.kick_seq, p.snare_seq, p.hat_seq, b.seq] {
        r.set(seq, &[("bank", 3.0)]);
    }
    // Layer faders: breath, strings, pad, lead (CC 24–27).
    r.row(15);
    let layers = r.add(
        "mixer",
        &[
            ("level1", 0.8),
            ("level2", 0.0),
            ("level3", 0.0),
            ("level4", 0.0),
        ],
    );
    let lead_send = r.add("mixer", &[("level1", 0.0)]);
    let dry = r.add("mixer", &[("level1", 0.55), ("level2", 0.5)]);
    let chorus = r.add(
        "chorus",
        &[
            ("rate_hz", 0.35),
            ("depth", 60.0),
            ("mix", 50.0),
            ("width", 100.0),
        ],
    );
    let delay = r.add(
        "delay",
        &[
            ("sync", 3.0),
            ("feedback", 35.0),
            ("mix", 100.0),
            ("tone_hz", 3200.0),
            ("mode", 1.0),
        ],
    );
    r.row(16);
    let bus_l = r.add("mixer", &[("level2", 0.3)]);
    let bus_r = r.add("mixer", &[("level2", 0.3)]);
    let reverb = r.add(
        "reverb",
        &[
            ("decay_s", 3.8),
            ("mix", 28.0),
            ("predelay_ms", 30.0),
            ("damp_hz", 5000.0),
        ],
    );
    let main_l = r.add(
        "mixer",
        &[("level1", 1.0), ("level2", 1.0), ("level3", 1.0)],
    );
    let main_r = r.add(
        "mixer",
        &[("level1", 1.0), ("level2", 1.0), ("level3", 1.0)],
    );
    r.row(17);
    let perc_rev = r.add(
        "reverb",
        &[("decay_s", 0.9), ("mix", 12.0), ("damp_hz", 7000.0)],
    );
    r.wire((br.gain, "out"), (layers, "in1"));
    r.wire((s.gain, "out"), (layers, "in2"));
    r.wire((pd.gain, "out"), (layers, "in3"));
    r.wire((ld.drive, "out"), (layers, "in4"));
    r.wire((ld.drive, "out"), (lead_send, "in1"));
    r.wire((layers, "out"), (chorus, "in_l"));
    r.wire((layers, "out"), (chorus, "in_r"));
    r.wire((lead_send, "out"), (delay, "in"));
    r.wire((clock, "gate"), (delay, "clock"));
    for (bus, side) in [(bus_l, "left"), (bus_r, "right")] {
        r.wire((chorus, side), (bus, "in1"));
        r.wire((delay, side), (bus, "in2"));
    }
    r.wire((bus_l, "out"), (reverb, "in_l"));
    r.wire((bus_r, "out"), (reverb, "in_r"));
    r.wire((p.drive, "out"), (dry, "in1"));
    r.wire((b.drive, "out"), (dry, "in2"));
    r.wire((p.drive, "out"), (perc_rev, "in_l"));
    r.wire((p.drive, "out"), (perc_rev, "in_r"));
    for (main, side) in [(main_l, "left"), (main_r, "right")] {
        r.wire((dry, "out"), (main, "in1"));
        r.wire((reverb, side), (main, "in2"));
        r.wire((perc_rev, side), (main, "in3"));
    }
    r.set(dry, &[("level1", 0.4), ("level2", 0.4)]);
    r.set(main_l, &[("level2", 0.6), ("level3", 0.5)]);
    r.set(main_r, &[("level2", 0.6), ("level3", 0.5)]);
    // Master: the four layers, the sequences and every macro up can add to more than full
    // scale; this trims the whole piece so its densest moment keeps 1 dB (no limiter).
    let master_l = r.add("gain", &[("gain_db", -4.0)]);
    let master_r = r.add("gain", &[("gain_db", -4.0)]);
    r.wire((main_l, "out"), (master_l, "in"));
    r.wire((main_r, "out"), (master_r, "in"));
    out_stereo(&mut r, (master_l, "out"), (master_r, "out"));
    // Energy: the sequences open and bite. Motion: every slow modulation deeper (the ringmods
    // take m2 directly). Bright: the keyboard layers' filters. Space: plate and echo.
    r.route((m, "m1"), b.filt, "cutoff_hz", 0.2);
    r.route((m, "m1"), b.drive, "drive_db", 0.25);
    r.route((m, "m1"), p.drive, "drive_db", 0.3);
    r.route((m, "m1"), p.bus, "level3", 0.25);
    r.route((m, "m2"), chorus, "depth", 0.25);
    r.route((m, "m3"), s.filt, "cutoff_hz", 0.2);
    r.route((m, "m3"), pd.filt, "cutoff_hz", 0.2);
    r.route((m, "m3"), ld.filt, "cutoff_hz", 0.15);
    r.route((m, "m3"), br.air, "cutoff_hz", 0.15);
    r.route((m, "m4"), reverb, "mix", 0.3);
    r.route((m, "m4"), reverb, "decay_s", 0.12);
    r.route((m, "m4"), delay, "feedback", 0.15);
    let _ = (pd.noise, pd.env, s.env, s.osc1, br.band, br.mix);
    // Cues on the next bar: sequences only (the layers are faders).
    let seqs = [p.kick_seq, p.snare_seq, p.hat_seq, b.seq];
    for (name, banks) in [
        ("Air", [3, 3, 3, 3]),
        ("Pulse", [2, 3, 2, 3]),
        ("Groove", [0, 0, 0, 0]),
        ("Lift", [1, 1, 1, 1]),
        ("Break", [3, 3, 2, 2]),
    ] {
        let n = cues::add(&mut r.e, cue_mod, &|_| 0).unwrap();
        cues::rename(&mut r.e, cue_mod, n, name);
        for (seq, bank) in seqs.iter().zip(banks) {
            cues::set_target(&mut r.e, cue_mod, n, *seq, Some(bank));
        }
        assert_eq!(
            cues::cues(r.e.state(), cue_mod)[n - 1].timing,
            Timing::NextBar
        );
    }
    r.pins(&[
        (clock, TRANSPORT, None, None),
        (cue_mod, CUE_PADS, None, None),
        (m, "m1", Some(20), None),
        (m, "m2", Some(21), None),
        (m, "m3", Some(22), None),
        (m, "m4", Some(23), None),
        (layers, "level1", Some(24), Some("Breath")),
        (layers, "level2", Some(25), Some("Strings")),
        (layers, "level3", Some(26), Some("Pad")),
        (layers, "level4", Some(27), Some("Lead")),
        (lead_send, "level1", Some(28), Some("Lead echo")),
        (ld.midi, "glide_ms", Some(29), Some("Glide time")),
    ]);
    let mut changes = Vec::new();
    for (n, cc) in (1..=5).zip(40..) {
        changes.push((cue_mod, format!("{BTN_PREFIX}cue.{n}"), Some(cc as f32)));
    }
    changes.push((cue_mod, format!("{BTN_PREFIX}cancel"), Some(45.0)));
    changes.push((clock, format!("{BTN_PREFIX}run"), Some(46.0)));
    changes.push((clock, format!("{BTN_PREFIX}restart"), Some(47.0)));
    r.e.set_presentation(&changes);
    r.e
}

type Builder = fn() -> PatchEditor;

/// (directory under `patches/`, builder, played by the keyboard).
const PATCHES: [(&str, Builder, bool); 7] = [
    ("palette/strings", strings_patch, true),
    ("palette/pad", pad_patch, true),
    ("palette/lead", lead_patch, true),
    ("palette/bass", bass_patch, false),
    ("palette/breath", breath_patch, true),
    ("palette/perc", perc_patch, false),
    ("sound-palette", piece_patch, true),
];

fn patch_dir(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../patches")
        .join(name)
}

/// Not a check: writes `patches/palette/*` and `patches/sound-palette`.
/// `cargo test -p kabl-ui --test sound_palette write_palette_patches -- --ignored`.
#[test]
#[ignore]
fn write_palette_patches() {
    for (name, build, _) in PATCHES {
        let dir = patch_dir(name);
        let _ = std::fs::remove_dir_all(&dir);
        kabl_core::save(&dir, build().log()).unwrap();
    }
}

/// Each committed patch is what its builder builds, every stored value is in range, it
/// compiles, and it has four named macros and CC-mapped pins.
#[test]
fn committed_palette_patches_match_their_builders() {
    for (name, build, _) in PATCHES {
        let saved = kabl_core::load(&patch_dir(name)).unwrap();
        let built = build();
        assert_eq!(saved.state(), built.state(), "{name}");
        let st = saved.state();
        for m in st.modules.values() {
            let info = kabl_modules::registry::info_for(&m.kind).unwrap();
            for p in info.params {
                if let Some(&v) = m.params.get(p.name) {
                    assert!(
                        (p.min..=p.max).contains(&v),
                        "{name}: {} {} = {v}",
                        m.kind,
                        p.name
                    );
                }
            }
        }
        kabl_engine::compile::compile(st, 48000.0, 8).expect("compiles");
        let (&mid, _) = st.modules.iter().find(|(_, m)| m.kind == "macro").unwrap();
        for k in 1..=4 {
            assert!(
                st.label(mid, &format!("name.m{k}")).is_some(),
                "{name} m{k}"
            );
        }
        let pins = kabl_ui::perform::pins(st);
        assert!(pins.len() >= 7, "{name}: {} pins", pins.len());
        let ccs = st
            .modules
            .values()
            .flat_map(|m| m.params.keys())
            .filter(|k| k.starts_with(CC_PREFIX))
            .count();
        assert!(ccs >= 5, "{name}: {ccs} CC maps");
    }
}

// ---------------------------------------------------------------------------------------------
// Headroom.

const SR: f32 = 48000.0;

fn db(v: f32) -> f32 {
    20.0 * v.max(1e-9).log10()
}

/// Renders `st` with `notes` held from 0.1 s to `hold` s, for `total` s. Returns (peak, RMS)
/// over the whole render, stereo.
fn render(st: &PatchState, notes: &[u8], hold: f32, total: f32) -> (f32, f32, Vec<f32>) {
    let collector = basedrop::Collector::new();
    let handle = collector.handle();
    let mut e = PatchEngine::new(&handle, st, SR, 8).unwrap();
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    let blocks = (total * SR) as usize / BLOCK;
    let (mut peak, mut sum) = (0f32, 0f64);
    let mut left = Vec::with_capacity(blocks * BLOCK);
    for b in 0..blocks {
        let t = (b * BLOCK) as f32 / SR;
        if b == (0.1 * SR) as usize / BLOCK {
            for &n in notes {
                e.key(KeyEvent::On {
                    note: n,
                    velocity: 100,
                });
            }
        }
        if b == (hold * SR) as usize / BLOCK {
            for &n in notes {
                e.key(KeyEvent::Off { note: n });
            }
        }
        let _ = t;
        e.process_block(&mut l, &mut r);
        for (&a, &c) in l.iter().zip(&r) {
            assert!(a.is_finite() && c.is_finite());
            peak = peak.max(a.abs()).max(c.abs());
            sum += (a as f64).powi(2) + (c as f64).powi(2);
        }
        left.extend_from_slice(&l);
    }
    let rms = (sum / (2 * blocks * BLOCK) as f64).sqrt() as f32;
    (peak, rms, left)
}

/// Every patch at its saved settings with every macro at 1 (the loudest corner): one note,
/// a 4-note and a 6-note chord (sequenced patches: 8 bars running). Peaks must stay below
/// −1 dBFS; `-- --nocapture` prints the table for the README.
#[test]
fn headroom_of_every_patch() {
    println!("| patch | take | peak dBFS | RMS dBFS | peak, macros at 1 |");
    println!("|---|---|---|---|---|");
    let mut bad = Vec::new();
    for (name, build, keys) in PATCHES {
        let st = build().state().clone();
        let mut hot = st.clone();
        for m in hot.modules.values_mut() {
            if m.kind == "macro" {
                for k in 1..=4 {
                    m.params.insert(format!("m{k}"), 1.0);
                }
            }
            if name == "sound-palette" && m.kind == "mixer" && m.params.get("level4") == Some(&0.0)
            {
                // The densest moment the piece plays: breath low, strings, pad and lead up
                // together (the faders' 0.8 mark), sequences on Lift, every macro at 1.
                for (lvl, v) in [
                    ("level1", 0.4),
                    ("level2", 0.8),
                    ("level3", 0.8),
                    ("level4", 0.8),
                ] {
                    m.params.insert(lvl.into(), v);
                }
            }
            if m.kind == "seq" && name == "sound-palette" {
                m.params.insert("bank".into(), 1.0);
            }
        }
        let takes: &[(&str, &[u8])] = if keys {
            &[
                ("one note (C4)", &[60]),
                ("4-note chord", &[48, 55, 60, 64]),
                ("6-note chord", &[40, 52, 59, 62, 66, 71]),
            ]
        } else {
            &[("8 bars", &[])]
        };
        for (take, notes) in takes {
            let (hold, total) = if keys { (4.0, 7.0) } else { (8.0, 9.0) };
            let (peak, rms, _) = render(&st, notes, hold, total);
            let (hot_peak, _, _) = render(&hot, notes, hold, total);
            println!(
                "| {name} | {take} | {:.1} | {:.1} | {:.1} |",
                db(peak),
                db(rms),
                db(hot_peak)
            );
            if db(peak) >= -1.0 || db(hot_peak) >= -0.5 || db(rms) <= -45.0 {
                bad.push(format!("{name} {take}"));
            }
        }
    }
    assert!(bad.is_empty(), "out of range: {bad:?}");
}

/// Not a check: offline renders of each patch playing a short phrase, for analysis
/// (`target/palette-offline/*.wav`). The recorded examples come from the real app.
/// `cargo test --release -p kabl-ui --test sound_palette write_offline_renders -- --ignored`.
#[test]
#[ignore]
fn write_offline_renders() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/palette-offline");
    std::fs::create_dir_all(&dir).unwrap();
    // (time s, note, on): E minor chords, then a legato line with overlaps.
    let chords: Vec<(f32, u8, bool)> = [
        (0.2, [52u8, 59, 64, 67]),
        (3.2, [48, 55, 60, 64]),
        (6.2, [50, 57, 62, 66]),
    ]
    .iter()
    .flat_map(|&(t, ns)| {
        ns.into_iter()
            .flat_map(move |n| [(t, n, true), (t + 2.8, n, false)])
    })
    .collect();
    let line: Vec<(f32, u8, bool)> = [
        (64u8, 0.2, 0.9),
        (67, 0.8, 1.6),
        (71, 1.5, 2.2),
        (69, 2.4, 3.4),
        (67, 3.3, 4.0),
        (64, 4.2, 5.6),
    ]
    .iter()
    .flat_map(|&(n, a, b)| [(a, n, true), (b, n, false)])
    .collect();
    for (name, build, keys) in PATCHES {
        let st = build().state().clone();
        let events = if name.ends_with("lead") {
            &line
        } else {
            &chords
        };
        let collector = basedrop::Collector::new();
        let handle = collector.handle();
        let mut e = PatchEngine::new(&handle, &st, SR, 8).unwrap();
        let mut st2 = st.clone();
        if name == "sound-palette" {
            for m in st2.modules.values_mut() {
                if m.kind == "seq" {
                    m.params.insert("bank".into(), 0.0);
                }
            }
            e = PatchEngine::new(&handle, &st2, SR, 8).unwrap();
        }
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: SR as u32,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let path = dir.join(format!("{}.wav", name.replace('/', "-")));
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
        let blocks = (11.0 * SR) as usize / BLOCK;
        for b in 0..blocks {
            let t0 = (b * BLOCK) as f32 / SR;
            let t1 = t0 + BLOCK as f32 / SR;
            if keys {
                for &(t, n, on) in events.iter() {
                    if t >= t0 && t < t1 {
                        e.key(if on {
                            KeyEvent::On {
                                note: n,
                                velocity: 100,
                            }
                        } else {
                            KeyEvent::Off { note: n }
                        });
                    }
                }
            }
            e.process_block(&mut l, &mut r);
            for (a, c) in l.iter().zip(&r) {
                w.write_sample(*a).unwrap();
                w.write_sample(*c).unwrap();
            }
        }
        w.finalize().unwrap();
    }
}
