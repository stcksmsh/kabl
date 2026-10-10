//! Four demo patches (`patches/sound-engines/demos/*`) for the FM operator and the wavetable
//! oscillator: sounds the virtual-analogue palette cannot make. Built here so every value has
//! its reason written next to it; `docs/sound-engines/README.md` has the playing notes.
//!
//! - `tine-keys`: FM electric piano. A body modulator (ratio 1) and a short, bright tine
//!   modulator (ratio 14) bend one carrier; velocity brightens both.
//! - `glass-bells`: two inharmonic FM pairs (1 : 3.5 and 2 : 5.04) that ring out on their own.
//! - `vowel-drift`: two wavetable oscillators (Vowels and Choir) detuned a few cents apart,
//!   each sweeping its table on a different slow LFO at audio rate.
//! - `imported-morph`: a .wav imported into the patch (a resonance sweep, 24 frames), its
//!   position swept by the envelope on every note, over an FM sub with feedback.
//!
//! Four more use the six-operator voice and an enveloped wavetable sweep:
//!
//! - `dx-keys`: six-operator electric piano, three modulator/carrier pairs, per-operator
//!   envelopes and velocity (no envelope modules at all).
//! - `iron-bell`: six-operator bell, three inharmonic pairs that ring for seconds.
//! - `rising-pad`: two wavetable oscillators whose positions the note's envelope sweeps open.
//! - `rust-bass`: six-operator bass, a punchy pair beside a four-operator growl with feedback.
//!
//! Rewrite the patches with
//! `cargo test -p kabl-ui --test sound_engines write_engine_patches -- --ignored`
//! and the audio clips (`target/sound-engines/*.wav`) with `write_engine_renders`.

use kabl_core::{ModuleId, PatchState, PortRef, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::PatchEngine;
use kabl_ui::PatchEditor;

const SR: f32 = 48000.0;

fn port(id: ModuleId, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

struct Rack {
    e: PatchEditor,
    col: f32,
    row: f32,
}

impl Rack {
    fn new() -> Self {
        Rack {
            e: PatchEditor::new(),
            col: 24.0,
            row: 20.0,
        }
    }
    fn next_row(&mut self) {
        self.row += 300.0;
        self.col = 24.0;
    }
    fn add(&mut self, kind: &str, params: &[(&str, f32)]) -> ModuleId {
        let id = self.e.add_module(
            kind,
            Vec2 {
                x: self.col,
                y: self.row,
            },
        );
        self.col += 190.0;
        for &(name, v) in params {
            self.e.set_param(id, name, v);
        }
        id
    }
    /// An `osc.fm6` with `ops[k]` = (ratio, fine ct, level, attack ms, decay ms, sustain, release
    /// ms, velocity sensitivity) for operator k + 1.
    fn fm6(&mut self, algorithm: f32, index: f32, feedback: f32, ops: [[f32; 8]; 6]) -> ModuleId {
        let id = self.add(
            "osc.fm6",
            &[
                ("algorithm", algorithm),
                ("index", index),
                ("feedback", feedback),
            ],
        );
        for (k, op) in ops.iter().enumerate() {
            let names = [
                "ratio", "fine", "level", "attack", "decay", "sustain", "release", "vel",
            ];
            for (name, &v) in names.iter().zip(op) {
                self.e.set_param(id, &format!("{name}{}", k + 1), v);
            }
        }
        id
    }
    fn wire(&mut self, a: ModuleId, ap: &str, b: ModuleId, bp: &str) {
        self.e.connect(port(a, ap), port(b, bp));
    }
    /// A modulation route into a knob; `amount` is the knob travel the source sweeps.
    fn route(&mut self, a: ModuleId, ap: &str, b: ModuleId, param: &str, amount: f32) {
        let cable = self.e.connect_route(port(a, ap), b, param);
        self.e.set_route_amount(cable, amount, true);
    }
}

fn adsr(a: f32, d: f32, s: f32, r: f32) -> [(&'static str, f32); 4] {
    [
        ("attack_ms", a),
        ("decay_ms", d),
        ("sustain", s),
        ("release_ms", r),
    ]
}

fn tine_keys() -> PatchEditor {
    let mut r = Rack::new();
    let m = r.add("midi.in", &[]);
    let body = r.add("osc.fm", &[("ratio", 1.0), ("level", 0.0)]);
    let tine = r.add("osc.fm", &[("ratio", 14.0), ("level", 0.0)]);
    let car = r.add("osc.fm", &[("ratio", 1.0), ("index", 2.4)]);
    r.next_row();
    let eb = r.add("env.adsr", &adsr(1.0, 800.0, 0.12, 200.0));
    let et = r.add("env.adsr", &adsr(0.5, 90.0, 0.0, 40.0));
    let ea = r.add("env.adsr", &adsr(1.0, 2600.0, 0.1, 300.0));
    let mix = r.add("mixer", &[("level1", 1.0), ("level2", 0.55)]);
    r.next_row();
    let vca = r.add("vca", &[("gain", 0.0)]);
    let gain = r.add("gain", &[("gain_db", 4.6)]);
    let chorus = r.add(
        "chorus",
        &[
            ("rate_hz", 0.7),
            ("depth", 35.0),
            ("mix", 30.0),
            ("width", 100.0),
        ],
    );
    let out = r.add("out", &[]);
    for osc in [body, tine, car] {
        r.wire(m, "pitch", osc, "pitch");
    }
    r.wire(body, "out", mix, "in1");
    r.wire(tine, "out", mix, "in2");
    r.wire(mix, "out", car, "pm");
    r.wire(car, "out", vca, "in");
    for env in [eb, et, ea] {
        r.wire(m, "gate", env, "gate");
    }
    r.wire(ea, "out", vca, "cv");
    r.wire(vca, "out", gain, "in");
    r.wire(gain, "out", chorus, "in_l");
    r.wire(gain, "out", chorus, "in_r");
    r.wire(chorus, "left", out, "left");
    r.wire(chorus, "right", out, "right");
    // The operators' own envelopes shape how far each bends the carrier, as on a DX.
    r.route(eb, "out", body, "level", 0.8);
    r.route(m, "velocity", body, "level", 0.25);
    r.route(et, "out", tine, "level", 0.35);
    r.route(m, "velocity", tine, "level", 0.2);
    r.e
}

fn glass_bells() -> PatchEditor {
    let mut r = Rack::new();
    let m = r.add("midi.in", &[]);
    // 1 : 3.5 -- the modulator is x4 minus 231 cents, an inharmonic bell strike.
    let mod_a = r.add(
        "osc.fm",
        &[("ratio", 4.0), ("fine", -231.0), ("level", 0.0)],
    );
    let car_a = r.add("osc.fm", &[("ratio", 1.0), ("index", 4.5)]);
    // An octave up, 2 : 5.04: the metallic upper partials, shorter.
    let mod_b = r.add("osc.fm", &[("ratio", 5.0), ("fine", 14.0), ("level", 0.0)]);
    let car_b = r.add("osc.fm", &[("ratio", 2.0), ("index", 3.0)]);
    r.next_row();
    let env_ma = r.add("env.adsr", &adsr(0.5, 2600.0, 0.0, 800.0));
    let env_mb = r.add("env.adsr", &adsr(0.5, 900.0, 0.0, 400.0));
    let env_ca = r.add("env.adsr", &adsr(1.0, 5200.0, 0.0, 3000.0));
    let env_cb = r.add("env.adsr", &adsr(1.0, 2800.0, 0.0, 1500.0));
    r.next_row();
    let vca_a = r.add("vca", &[("gain", 0.0)]);
    let vca_b = r.add("vca", &[("gain", 0.0)]);
    let mix = r.add("mixer", &[("level1", 1.0), ("level2", 0.45)]);
    let gain = r.add("gain", &[("gain_db", 0.0)]);
    r.next_row();
    let reverb = r.add(
        "reverb",
        &[
            ("decay_s", 7.0),
            ("damp_hz", 7000.0),
            ("mix", 38.0),
            ("width", 100.0),
        ],
    );
    let out = r.add("out", &[]);
    for osc in [mod_a, car_a, mod_b, car_b] {
        r.wire(m, "pitch", osc, "pitch");
    }
    r.wire(mod_a, "out", car_a, "pm");
    r.wire(mod_b, "out", car_b, "pm");
    for env in [env_ma, env_mb, env_ca, env_cb] {
        r.wire(m, "gate", env, "gate");
    }
    r.wire(car_a, "out", vca_a, "in");
    r.wire(env_ca, "out", vca_a, "cv");
    r.wire(car_b, "out", vca_b, "in");
    r.wire(env_cb, "out", vca_b, "cv");
    r.wire(vca_a, "out", mix, "in1");
    r.wire(vca_b, "out", mix, "in2");
    r.wire(mix, "out", gain, "in");
    r.wire(gain, "out", reverb, "in_l");
    r.wire(gain, "out", reverb, "in_r");
    r.wire(reverb, "left", out, "left");
    r.wire(reverb, "right", out, "right");
    r.route(env_ma, "out", mod_a, "level", 1.0);
    r.route(env_mb, "out", mod_b, "level", 1.0);
    r.route(m, "velocity", car_a, "index", 0.1);
    r.e
}

fn vowel_drift() -> PatchEditor {
    let mut r = Rack::new();
    let m = r.add("midi.in", &[]);
    // Table 2 = Vowels, 7 = Choir. `pos_mod` sets how far each LFO moves its table.
    let wt_a = r.add(
        "osc.wt",
        &[
            ("table", 2.0),
            ("position", 0.5),
            ("pos_mod", 0.5),
            ("fine", -7.0),
        ],
    );
    let wt_b = r.add(
        "osc.wt",
        &[
            ("table", 7.0),
            ("position", 0.4),
            ("pos_mod", 0.4),
            ("fine", 7.0),
        ],
    );
    let lfo_a = r.add("lfo", &[("rate_hz", 0.047), ("waveform", 1.0)]);
    let lfo_b = r.add("lfo", &[("rate_hz", 0.071), ("waveform", 0.0)]);
    r.next_row();
    let mix = r.add("mixer", &[("level1", 1.0), ("level2", 0.8)]);
    let filter = r.add(
        "filter.ladder",
        &[
            ("cutoff_hz", 3200.0),
            ("resonance", 0.18),
            ("drive_db", 0.0),
        ],
    );
    let env = r.add("env.adsr", &adsr(1400.0, 800.0, 0.85, 2800.0));
    let vca = r.add("vca", &[("gain", 0.0)]);
    r.next_row();
    let gain = r.add("gain", &[("gain_db", 5.3)]);
    let chorus = r.add(
        "chorus",
        &[
            ("rate_hz", 0.22),
            ("depth", 65.0),
            ("mix", 45.0),
            ("width", 100.0),
        ],
    );
    let reverb = r.add(
        "reverb",
        &[
            ("decay_s", 6.5),
            ("damp_hz", 5000.0),
            ("mix", 32.0),
            ("width", 100.0),
        ],
    );
    let out = r.add("out", &[]);
    for osc in [wt_a, wt_b] {
        r.wire(m, "pitch", osc, "pitch");
    }
    r.wire(lfo_a, "out", wt_a, "pos");
    r.wire(lfo_b, "out", wt_b, "pos");
    r.wire(wt_a, "out", mix, "in1");
    r.wire(wt_b, "out", mix, "in2");
    r.wire(mix, "out", filter, "in");
    r.wire(filter, "out", vca, "in");
    r.wire(m, "gate", env, "gate");
    r.wire(env, "out", vca, "cv");
    r.wire(vca, "out", gain, "in");
    r.wire(gain, "out", chorus, "in_l");
    r.wire(gain, "out", chorus, "in_r");
    r.wire(chorus, "left", reverb, "in_l");
    r.wire(chorus, "right", reverb, "in_r");
    r.wire(reverb, "left", out, "left");
    r.wire(reverb, "right", out, "right");
    r.route(env, "out", filter, "cutoff_hz", 0.12);
    r.e
}

/// The table `imported-morph` embeds: 24 frames of a saw-like spectrum with a resonant peak
/// that climbs from the 2nd to the 22nd harmonic. Written as an ordinary .wav so the demo
/// goes through the same import as a user's file.
pub fn resonance_sweep_wav() -> Vec<u8> {
    let frames = 24;
    let mut pcm = Vec::new();
    for f in 0..frames {
        let centre = 2.0 + 20.0 * f as f64 / (frames - 1) as f64;
        let cycle: Vec<f64> = (0..2048)
            .map(|k| {
                let t = k as f64 / 2048.0;
                (1..=60)
                    .map(|h| {
                        let h = h as f64;
                        let peak = 1.0 + 5.0 * (-0.5 * ((h - centre) / 1.6).powi(2)).exp();
                        peak / h * (std::f64::consts::TAU * h * t).sin()
                    })
                    .sum()
            })
            .collect();
        let top = cycle.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        pcm.extend(cycle.iter().map(|v| ((v / top) * 30000.0) as i16));
    }
    let data: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
    let mut b = b"RIFF".to_vec();
    b.extend((36 + data.len() as u32).to_le_bytes());
    b.extend(b"WAVEfmt ");
    b.extend(16u32.to_le_bytes());
    b.extend([1, 0, 1, 0]);
    b.extend(48000u32.to_le_bytes());
    b.extend(96000u32.to_le_bytes());
    b.extend([2, 0, 16, 0]);
    b.extend(b"data");
    b.extend((data.len() as u32).to_le_bytes());
    b.extend(data);
    b
}

fn imported_morph() -> PatchEditor {
    let mut r = Rack::new();
    r.e.import_table(1, "resonance-sweep.wav", &resonance_sweep_wav())
        .expect("table imports");
    let m = r.add("midi.in", &[]);
    let wt = r.add(
        "osc.wt",
        &[("user", 1.0), ("position", 0.0), ("pos_mod", 1.0)],
    );
    // An octave below the key (ratio 0 is one half), a little feedback for weight.
    let sub = r.add(
        "osc.fm",
        &[
            ("ratio", 0.0),
            ("level", 0.55),
            ("feedback", 0.35),
            ("index", 0.0),
        ],
    );
    let env = r.add("env.adsr", &adsr(3.0, 400.0, 0.7, 180.0));
    r.next_row();
    let mix = r.add("mixer", &[("level1", 1.0), ("level2", 1.0)]);
    let drive = r.add(
        "drive",
        &[("drive_db", 9.0), ("trim_db", -6.0), ("mix", 70.0)],
    );
    let vca = r.add("vca", &[("gain", 0.0)]);
    let gain = r.add("gain", &[("gain_db", 18.3)]);
    r.next_row();
    let reverb = r.add(
        "reverb",
        &[
            ("decay_s", 1.4),
            ("damp_hz", 4000.0),
            ("mix", 12.0),
            ("width", 80.0),
        ],
    );
    let out = r.add("out", &[]);
    for osc in [wt, sub] {
        r.wire(m, "pitch", osc, "pitch");
    }
    r.wire(wt, "out", mix, "in1");
    r.wire(sub, "out", mix, "in2");
    r.wire(mix, "out", drive, "in");
    r.wire(drive, "out", vca, "in");
    r.wire(m, "gate", env, "gate");
    r.wire(env, "out", vca, "cv");
    r.wire(vca, "out", gain, "in");
    r.wire(gain, "out", reverb, "in_l");
    r.wire(gain, "out", reverb, "in_r");
    r.wire(reverb, "left", out, "left");
    r.wire(reverb, "right", out, "right");
    // Every note sweeps the table from its first frame toward the resonant top.
    r.route(env, "out", wt, "position", 0.8);
    r.route(m, "velocity", wt, "position", 0.2);
    r.e
}

/// midi.in -> `voice` (pitch, gate, velocity) -> gain -> chorus -> out. The voice's own
/// operator envelopes shape every note, so there is no VCA.
fn fm6_chain(r: &mut Rack, m: ModuleId, voice: ModuleId, gain_db: f32, chorus: &[(&str, f32)]) {
    r.wire(m, "pitch", voice, "pitch");
    r.wire(m, "gate", voice, "gate");
    r.wire(m, "velocity", voice, "velocity");
    let gain = r.add("gain", &[("gain_db", gain_db)]);
    let fx = r.add("chorus", chorus);
    let out = r.add("out", &[]);
    r.wire(voice, "out", gain, "in");
    r.wire(gain, "out", fx, "in_l");
    r.wire(gain, "out", fx, "in_r");
    r.wire(fx, "left", out, "left");
    r.wire(fx, "right", out, "right");
}

fn dx_keys() -> PatchEditor {
    let mut r = Rack::new();
    let m = r.add("midi.in", &[]);
    // Algorithm 1: 2 into 1, 4 into 3, 6 into 5. Pair 1 is the body (a mellow ratio 1 bend),
    // pair 2 adds the bright tine (ratio 14, gone in a tenth of a second), pair 3 a soft
    // detuned shimmer.
    // ratio, fine, level, attack, decay, sustain, release, velocity
    let voice = r.fm6(
        1.0,
        0.3,
        0.0,
        [
            [1.0, 0.0, 1.0, 1.0, 2600.0, 0.1, 260.0, 0.35],
            [1.0, 0.0, 0.9, 1.0, 700.0, 0.06, 200.0, 0.8],
            [1.0, 6.0, 0.7, 1.0, 3000.0, 0.1, 260.0, 0.35],
            [14.0, 0.0, 0.5, 0.5, 90.0, 0.0, 60.0, 0.9],
            [1.0, -5.0, 0.4, 1.0, 3400.0, 0.1, 260.0, 0.3],
            [1.0, 0.0, 0.6, 1.0, 1400.0, 0.05, 200.0, 0.6],
        ],
    );
    fm6_chain(
        &mut r,
        m,
        voice,
        7.0,
        &[
            ("rate_hz", 0.7),
            ("depth", 35.0),
            ("mix", 30.0),
            ("width", 100.0),
        ],
    );
    r.e
}

fn iron_bell() -> PatchEditor {
    let mut r = Rack::new();
    let m = r.add("midi.in", &[]);
    // Algorithm 1 again, three inharmonic pairs: 1 : 3.5, 2 : 5.04 and 3.1 : 8.9. Carriers
    // ring 4 to 7 seconds; each modulator fades faster, so the bell darkens as it rings.
    let voice = r.fm6(
        1.0,
        0.3,
        0.0,
        [
            [1.0, 0.0, 1.0, 0.5, 7000.0, 0.0, 3500.0, 0.0],
            [4.0, -231.0, 0.9, 0.5, 2600.0, 0.0, 1200.0, 0.15],
            [2.0, 4.0, 0.55, 0.5, 4200.0, 0.0, 2200.0, 0.0],
            [5.0, 14.0, 0.8, 0.5, 1200.0, 0.0, 600.0, 0.15],
            [3.0, 100.0, 0.3, 0.5, 2000.0, 0.0, 1200.0, 0.0],
            [9.0, -20.0, 0.6, 0.5, 500.0, 0.0, 300.0, 0.2],
        ],
    );
    r.wire(m, "pitch", voice, "pitch");
    r.wire(m, "gate", voice, "gate");
    r.wire(m, "velocity", voice, "velocity");
    let reverb = r.add(
        "reverb",
        &[
            ("decay_s", 7.0),
            ("damp_hz", 7000.0),
            ("mix", 38.0),
            ("width", 100.0),
        ],
    );
    let gain = r.add("gain", &[("gain_db", 9.5)]);
    let out = r.add("out", &[]);
    r.wire(voice, "out", gain, "in");
    r.wire(gain, "out", reverb, "in_l");
    r.wire(gain, "out", reverb, "in_r");
    r.wire(reverb, "left", out, "left");
    r.wire(reverb, "right", out, "right");
    r.e
}

fn rising_pad() -> PatchEditor {
    let mut r = Rack::new();
    let m = r.add("midi.in", &[]);
    // Table 3 = Glass Bell (a sine to a bright bell spectrum), 4 = Digital Hollow. The note's envelope (slow attack, long
    // decay to a high sustain) is routed to both positions, so every held chord opens up
    // from a dull sine-ish tone to the full spectrum, at a different pace in each oscillator.
    let a = r.add(
        "osc.wt",
        &[
            ("table", 3.0),
            ("position", 0.0),
            ("pos_mod", 1.0),
            ("fine", -6.0),
        ],
    );
    let b = r.add(
        "osc.wt",
        &[
            ("table", 4.0),
            ("position", 0.1),
            ("pos_mod", 1.0),
            ("fine", 6.0),
        ],
    );
    r.next_row();
    let sweep = r.add("env.adsr", &adsr(3200.0, 4200.0, 0.75, 2600.0));
    let amp = r.add("env.adsr", &adsr(1800.0, 900.0, 0.9, 2800.0));
    let mix = r.add("mixer", &[("level1", 1.0), ("level2", 0.7)]);
    let filter = r.add(
        "filter.ladder",
        &[
            ("cutoff_hz", 4200.0),
            ("resonance", 0.15),
            ("drive_db", 0.0),
        ],
    );
    r.next_row();
    let vca = r.add("vca", &[("gain", 0.0)]);
    let gain = r.add("gain", &[("gain_db", 4.0)]);
    let chorus = r.add(
        "chorus",
        &[
            ("rate_hz", 0.25),
            ("depth", 60.0),
            ("mix", 42.0),
            ("width", 100.0),
        ],
    );
    let reverb = r.add(
        "reverb",
        &[
            ("decay_s", 6.0),
            ("damp_hz", 5500.0),
            ("mix", 30.0),
            ("width", 100.0),
        ],
    );
    let out = r.add("out", &[]);
    for osc in [a, b] {
        r.wire(m, "pitch", osc, "pitch");
    }
    r.wire(a, "out", mix, "in1");
    r.wire(b, "out", mix, "in2");
    r.wire(mix, "out", filter, "in");
    r.wire(filter, "out", vca, "in");
    r.wire(m, "gate", sweep, "gate");
    r.wire(m, "gate", amp, "gate");
    r.wire(amp, "out", vca, "cv");
    r.wire(vca, "out", gain, "in");
    r.wire(gain, "out", chorus, "in_l");
    r.wire(gain, "out", chorus, "in_r");
    r.wire(chorus, "left", reverb, "in_l");
    r.wire(chorus, "right", reverb, "in_r");
    r.wire(reverb, "left", out, "left");
    r.wire(reverb, "right", out, "right");
    r.route(sweep, "out", a, "position", 0.9);
    r.route(sweep, "out", b, "position", 0.6);
    r.route(m, "velocity", a, "position", 0.1);
    r.route(sweep, "out", filter, "cutoff_hz", 0.1);
    r.e
}

fn rust_bass() -> PatchEditor {
    let mut r = Rack::new();
    let m = r.add("midi.in", &[]);
    // Algorithm 3: 2 into 1, and 6 into 5 into 4 into 3. The first pair is the punch (a ratio 1
    // bend that dies in 200 ms), the second a four-operator growl; operator 6 feeds itself.
    let voice = r.fm6(
        3.0,
        0.25,
        0.45,
        [
            [1.0, 0.0, 1.0, 0.5, 900.0, 0.8, 120.0, 0.2],
            [1.0, 0.0, 0.8, 0.5, 220.0, 0.1, 80.0, 0.7],
            [1.0, 3.0, 0.5, 0.5, 700.0, 0.55, 120.0, 0.3],
            [2.0, 0.0, 0.6, 0.5, 500.0, 0.3, 100.0, 0.5],
            [1.0, 0.0, 0.5, 0.5, 400.0, 0.2, 100.0, 0.5],
            [1.0, 0.0, 0.9, 0.5, 300.0, 0.25, 100.0, 0.6],
        ],
    );
    fm6_chain(
        &mut r,
        m,
        voice,
        11.0,
        &[
            ("rate_hz", 0.4),
            ("depth", 20.0),
            ("mix", 18.0),
            ("width", 100.0),
        ],
    );
    r.e
}

/// (time s, note, on)
type Events = Vec<(f32, u8, bool)>;

fn notes(list: &[(u8, f32, f32)]) -> Events {
    list.iter()
        .flat_map(|&(n, a, b)| [(a, n, true), (b, n, false)])
        .collect()
}

fn tine_phrase() -> Events {
    let mut v = notes(&[
        (48, 0.2, 2.4),
        (55, 0.2, 2.4),
        (59, 0.25, 2.4),
        (64, 0.3, 2.4),
        (76, 2.6, 3.2),
        (74, 3.2, 3.8),
        (71, 3.8, 4.4),
        (67, 4.4, 5.2),
        (45, 5.4, 8.0),
        (52, 5.4, 8.0),
        (57, 5.45, 8.0),
        (60, 5.5, 8.0),
        (72, 5.7, 6.4),
        (69, 6.4, 7.1),
        (64, 7.1, 8.4),
    ]);
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    v
}

fn bell_phrase() -> Events {
    notes(&[
        (72, 0.2, 0.5),
        (79, 1.4, 1.7),
        (76, 2.8, 3.1),
        (84, 4.4, 4.7),
        (67, 6.0, 6.3),
        (72, 7.5, 7.8),
        (79, 7.6, 7.9),
    ])
}

fn drift_phrase() -> Events {
    notes(&[
        (52, 0.2, 6.0),
        (59, 0.2, 6.0),
        (64, 0.2, 6.0),
        (67, 0.2, 6.0),
        (48, 7.0, 13.0),
        (55, 7.0, 13.0),
        (60, 7.0, 13.0),
        (64, 7.0, 13.0),
    ])
}

fn morph_phrase() -> Events {
    notes(&[
        (36, 0.2, 0.9),
        (36, 1.0, 1.3),
        (43, 1.5, 2.1),
        (41, 2.4, 3.1),
        (36, 3.2, 3.5),
        (39, 3.6, 4.2),
        (43, 4.4, 5.0),
        (46, 5.2, 6.4),
        (48, 6.6, 7.8),
    ])
}

type Build = fn() -> PatchEditor;
/// (directory, builder, phrase, seconds, browser name, category, tags, description)
#[allow(clippy::type_complexity)]
const PATCHES: [(&str, Build, fn() -> Events, f32, &str, &str, &[&str], &str); 8] = [
    (
        "tine-keys",
        tine_keys,
        tine_phrase,
        10.0,
        "Tine Keys",
        "Keys",
        &["fm", "electric piano", "poly", "bright", "dx"],
        "FM electric piano: a body operator and a short bright tine operator bend one carrier. Play medium and hard: velocity brightens it. Low chords bark, high notes bell.",
    ),
    (
        "glass-bells",
        glass_bells,
        bell_phrase,
        13.0,
        "Glass Bells",
        "Keys",
        &["fm", "bell", "poly", "inharmonic", "ringing"],
        "Two inharmonic FM pairs that ring and fade on their own, whatever the key does. Short notes are enough; leave room for the tails.",
    ),
    (
        "vowel-drift",
        vowel_drift,
        drift_phrase,
        16.0,
        "Vowel Drift",
        "Pad",
        &["wavetable", "pad", "poly", "evolving", "vocal"],
        "Two wavetable oscillators (Vowels and Choir) sweep their tables on slow, unrelated LFOs, so held chords keep changing. Hold chords for at least six seconds.",
    ),
    (
        "imported-morph",
        imported_morph,
        morph_phrase,
        10.0,
        "Imported Morph",
        "Bass",
        &["wavetable", "imported", "fm", "bass", "poly"],
        "A wavetable imported into the sound (a resonance sweep that travels up the harmonics) over an FM sub. Each note sweeps the table; harder notes sweep further. The table travels inside the saved sound.",
    ),
    (
        "dx-keys",
        dx_keys,
        tine_phrase,
        10.0,
        "DX Keys",
        "Keys",
        &["fm", "electric piano", "poly", "six operators", "dx"],
        "Six-operator FM electric piano in three modulator/carrier pairs: a mellow body, a bright tine that is gone in a tenth of a second, and a detuned shimmer. Every operator has its own envelope and velocity response, so play soft and hard.",
    ),
    (
        "iron-bell",
        iron_bell,
        bell_phrase,
        13.0,
        "Iron Bell",
        "Keys",
        &["fm", "bell", "poly", "six operators", "ringing"],
        "Three inharmonic FM pairs in one voice that ring for several seconds and darken as they fade. Short notes are enough; leave room for the tails.",
    ),
    (
        "rising-pad",
        rising_pad,
        drift_phrase,
        16.0,
        "Rising Pad",
        "Pad",
        &["wavetable", "pad", "poly", "evolving", "sweep"],
        "Two wavetable oscillators whose positions the note's own envelope sweeps open: every chord rises from a dull tone to the full spectrum over a few seconds. Hold chords for at least six seconds.",
    ),
    (
        "rust-bass",
        rust_bass,
        morph_phrase,
        10.0,
        "Rust Bass",
        "Bass",
        &["fm", "bass", "poly", "six operators", "growl"],
        "A punchy two-operator pair beside a four-operator chain with feedback: a clean attack, then a gritty growl that settles. Harder notes growl more.",
    ),
];

fn patch_dir(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../patches/sound-engines/demos")
        .join(name)
}

fn sound_toml(name: &str, category: &str, tags: &[&str], description: &str) -> String {
    let tags: Vec<String> = tags.iter().map(|t| format!("    \"{t}\",\n")).collect();
    format!(
        "name = \"{name}\"\ncategory = \"{category}\"\ntags = [\n{}]\ndescription = \"{description}\"\nkeys = true\nsequence = false\n",
        tags.concat()
    )
}

/// Not a check: writes `patches/sound-engines/demos/*`. They sit one level deeper than the
/// factory library scans, so the curated factory bank is unchanged.
#[test]
#[ignore]
fn write_engine_patches() {
    for (dir, build, _, _, name, category, tags, description) in PATCHES {
        let path = patch_dir(dir);
        let _ = std::fs::remove_dir_all(&path);
        kabl_core::save(&path, build().log()).unwrap();
        std::fs::write(
            path.join("sound.toml"),
            sound_toml(name, category, tags, description),
        )
        .unwrap();
    }
}

fn render(st: &PatchState, events: &Events, secs: f32) -> Vec<f32> {
    let collector = basedrop::Collector::new();
    let handle = collector.handle();
    let mut e = PatchEngine::new(&handle, st, SR, 8).unwrap();
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    let mut out = Vec::new();
    for b in 0..(secs * SR) as usize / BLOCK {
        let t0 = (b * BLOCK) as f32 / SR;
        let t1 = t0 + BLOCK as f32 / SR;
        for &(t, n, on) in events {
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
        e.process_block(&mut l, &mut r);
        for (a, c) in l.iter().zip(&r) {
            out.push(*a);
            out.push(*c);
        }
    }
    out
}

/// Not a check: `target/sound-engines/*.wav`, 16-bit stereo, for listening.
#[test]
#[ignore]
fn write_engine_renders() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/sound-engines");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, build, events, secs, ..) in PATCHES {
        let audio = render(build().state(), &events(), secs);
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: SR as u32,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(dir.join(format!("{name}.wav")), spec).unwrap();
        for s in audio {
            w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16)
                .unwrap();
        }
        w.finalize().unwrap();
    }
}

fn peak_rms(v: &[f32]) -> (f32, f32) {
    (
        v.iter().fold(0.0, |m, x| f32::max(m, x.abs())),
        (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt(),
    )
}

/// Each committed patch is what its builder builds, its values are in range, it compiles, plays
/// a phrase at a sane level without clipping, and genuinely uses the new sources.
#[test]
fn committed_engine_patches_match_builders_and_play() {
    let mut bad = Vec::new();
    for (dir, build, events, secs, name, ..) in PATCHES {
        let saved = kabl_core::load(&patch_dir(dir)).unwrap();
        let built = build();
        assert_eq!(saved.state(), built.state(), "{dir}");
        let st = saved.state();
        assert!(
            st.modules
                .values()
                .any(|m| matches!(m.kind.as_str(), "osc.fm" | "osc.fm6" | "osc.wt")),
            "{dir}"
        );
        for m in st.modules.values() {
            let info = kabl_modules::registry::info_for(&m.kind).unwrap();
            for p in info.params {
                if let Some(&v) = m.params.get(p.name) {
                    assert!(
                        (p.min..=p.max).contains(&v),
                        "{dir}: {} {} = {v}",
                        m.kind,
                        p.name
                    );
                }
            }
        }
        let meta = std::fs::read_to_string(patch_dir(dir).join("sound.toml")).unwrap();
        assert!(meta.contains(&format!("name = \"{name}\"")), "{dir}");
        kabl_engine::compile::compile(st, SR, 8).expect("compiles");

        let audio = render(st, &events(), secs);
        assert!(audio.iter().all(|v| v.is_finite()), "{dir}");
        let (peak, rms) = peak_rms(&audio);
        println!("{dir}: peak {peak:.3} rms {rms:.4}");
        if !(peak < 0.98 && peak > 0.15 && rms > 0.01) {
            bad.push(format!("{dir}: peak {peak}, rms {rms}"));
        }
    }
    assert!(bad.is_empty(), "levels: {bad:?}");
}

#[test]
fn imported_morph_carries_its_table_and_needs_no_file() {
    let (dir, ..) = PATCHES[3];
    let saved = kabl_core::load(&patch_dir(dir)).unwrap();
    let table = &saved.state().tables[&1];
    assert_eq!(table.name, "resonance-sweep.wav");
    let frames = kabl_modules::wavetable::decode_canonical(&table.wav).unwrap();
    assert_eq!(frames.len(), 24);
    // The patch plays exactly the same with the table dropped from the state only if it falls
    // back to a factory table, which would sound different: prove the table is what plays.
    let events = morph_phrase();
    let with = render(saved.state(), &events, 4.0);
    let mut without = saved.state().clone();
    without.tables.clear();
    let fallback = render(&without, &events, 4.0);
    let diff = with
        .iter()
        .zip(&fallback)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max);
    assert!(diff > 0.05, "table not used: {diff}");
}
