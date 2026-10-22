//! The pilot factory presets (docs/presets/BRIEF.md): builds eight patches from the existing
//! modules, writes them under `patches/presets/`, and renders one clip per preset by playing a
//! short MIDI phrase through the real engine.
//!
//! ```sh
//! cargo run -p kabl-ui --example presets_pilot -- write patches
//! cargo run --release -p kabl-ui --example presets_pilot -- clips patches docs/presets/clips
//! ```
//!
//! Every preset has the same four macros, in this order: Filter, Drive, Space, Motion
//! (CC 20-23). Effect mixes are 0 on the module and driven by the Space macro, so a macro at 0
//! is dry and the stored macro value is the baked-in wet amount.

use std::path::Path;

use basedrop::Collector;
use kabl_core::{ModuleId, PortRef, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::{Command, PatchEngine, Timing};
use kabl_modules::builtins::Transport;
use kabl_ui::library::Meta;
use kabl_ui::perform::{BTN_PREFIX, CC_PREFIX, CUE_PADS, PIN_PREFIX, TRANSPORT};
use kabl_ui::{cues, PatchEditor};

const SR: f32 = 48000.0;

fn jack(id: ModuleId, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

/// A rack under construction: modules laid out left to right, wrapping to a new row.
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
    fn wire(&mut self, a: ModuleId, ap: &str, b: ModuleId, bp: &str) {
        self.e.connect(jack(a, ap), jack(b, bp));
    }
    /// Left and right inputs of a stereo module from one mono source.
    fn wire_both(&mut self, a: ModuleId, ap: &str, b: ModuleId) {
        self.wire(a, ap, b, "in_l");
        self.wire(a, ap, b, "in_r");
    }
    fn stereo(&mut self, a: ModuleId, b: ModuleId) {
        self.wire(a, "left", b, "in_l");
        self.wire(a, "right", b, "in_r");
    }
    /// A modulation route into a knob; `amount` is the fraction of the knob's travel the
    /// source sweeps.
    fn route(&mut self, a: ModuleId, ap: &str, b: ModuleId, param: &str, amount: f32) {
        let cable = self.e.connect_route(jack(a, ap), b, param);
        self.e.set_route_amount(cable, amount, true);
    }
    /// An LFO whose depth is the Motion macro: the LFO runs through a VCA the macro opens.
    fn motion(&mut self, knob: ModuleId, rate_hz: f32) -> ModuleId {
        let lfo = self.add("lfo", &[("rate_hz", rate_hz)]);
        let depth = self.add("vca", &[("gain", 0.0)]);
        self.wire(lfo, "out", depth, "in");
        self.route(knob, "m4", depth, "gain", 1.0);
        depth
    }
    /// The last stage: a limiter just under the level target's ceiling, so no macro setting
    /// can clip the output, then the output.
    fn out_stereo(&mut self, from: ModuleId) {
        let limiter = self.add("limiter", &[("ceiling_db", -1.5), ("release_ms", 150.0)]);
        let out = self.add("out", &[]);
        self.stereo(from, limiter);
        self.wire(limiter, "left", out, "left");
        self.wire(limiter, "right", out, "right");
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

/// The bank's four macros: Filter, Drive, Space, Motion, with their stored values and CC 20-23.
fn macros(r: &mut Rack, values: [f32; 4]) -> ModuleId {
    let knob = r.add("macro", &[]);
    for (i, name) in ["Filter", "Drive", "Space", "Motion"].iter().enumerate() {
        let key = format!("m{}", i + 1);
        r.e.set_label(knob, &format!("name.{key}"), Some((*name).into()));
        r.e.set_param(knob, &key, values[i]);
    }
    knob
}

/// Pins (Perform panel order) and CC numbers: transport first when the patch has a clock, the
/// cue pads next, then the four macros.
fn perform(r: &mut Rack, knob: ModuleId, clock: Option<ModuleId>, cue: Option<(ModuleId, usize)>) {
    let mut pins = Vec::new();
    if let Some(c) = clock {
        pins.push((c, TRANSPORT.to_string()));
    }
    if let Some((c, _)) = cue {
        pins.push((c, CUE_PADS.to_string()));
    }
    pins.extend((1..=4).map(|k| (knob, format!("m{k}"))));
    let mut changes = Vec::new();
    for (i, (id, key)) in pins.iter().enumerate() {
        changes.push((*id, format!("{PIN_PREFIX}{key}"), Some(i as f32)));
        if let Some(k) = key.strip_prefix('m') {
            changes.push((
                *id,
                format!("{CC_PREFIX}{key}"),
                Some(19.0 + k.parse::<f32>().unwrap()),
            ));
        }
    }
    if let Some((c, n)) = cue {
        for (i, cc) in (1..=n).zip(40..) {
            changes.push((c, format!("{BTN_PREFIX}cue.{i}"), Some(cc as f32)));
        }
    }
    r.e.set_presentation(&changes);
}

// ---- 1. Cherry Chainsaw: distorted, filtered mono bass --------------------------------------

fn cherry_chainsaw() -> Rack {
    let mut r = Rack::new();
    let knob = macros(&mut r, [0.40, 0.50, 0.30, 0.30]);
    let m = r.add(
        "midi.in",
        &[
            ("mode", 1.0),
            ("glide", 2.0),
            ("glide_ms", 55.0),
            ("bend", 2.0),
        ],
    );
    let saw = r.add(
        "osc.va",
        &[("waveform", 2.0), ("unison", 3.0), ("detune", 22.0)],
    );
    let sub = r.add("osc.va", &[("base_hz", 130.81), ("waveform", 3.0)]);
    r.next_row();
    let mix = r.add("mixer", &[("level1", 0.55), ("level2", 0.5)]);
    let drive = r.add("drive", &[("drive_db", 8.0), ("trim_db", -5.0)]);
    let lad = r.add(
        "filter.ladder",
        &[("cutoff_hz", 200.0), ("resonance", 0.42), ("drive_db", 6.0)],
    );
    let fenv = r.add("env.adsr", &adsr(2.0, 260.0, 0.25, 120.0));
    let aenv = r.add("env.adsr", &adsr(2.0, 300.0, 0.85, 110.0));
    r.next_row();
    let vca = r.add("vca", &[("gain", 0.0)]);
    let trim = r.add("gain", &[("gain_db", 6.0)]);
    let comp = r.add(
        "compressor",
        &[
            ("threshold_db", -20.0),
            ("ratio", 4.0),
            ("attack_ms", 6.0),
            ("release_ms", 120.0),
            ("makeup_db", 3.0),
        ],
    );
    let verb = r.add(
        "reverb",
        &[
            ("decay_s", 0.9),
            ("damp_hz", 3500.0),
            ("mix", 0.0),
            ("predelay_ms", 0.0),
            ("width", 60.0),
        ],
    );
    let wobble = r.motion(knob, 4.0);

    r.wire(m, "pitch", saw, "pitch");
    r.wire(m, "pitch", sub, "pitch");
    r.wire(saw, "out", mix, "in1");
    r.wire(sub, "out", mix, "in2");
    r.wire(mix, "out", drive, "in");
    r.wire(drive, "out", lad, "in");
    r.wire(m, "gate", fenv, "gate");
    r.wire(m, "gate", aenv, "gate");
    r.wire(lad, "out", vca, "in");
    r.wire(aenv, "out", vca, "cv");
    r.wire(vca, "out", trim, "in");
    r.wire_both(trim, "out", comp);
    r.stereo(comp, verb);
    r.out_stereo(verb);

    r.route(fenv, "out", lad, "cutoff_hz", 0.38);
    r.route(m, "wheel", lad, "cutoff_hz", 0.25);
    r.route(knob, "m1", lad, "cutoff_hz", 0.45);
    r.route(knob, "m2", drive, "drive_db", 0.5);
    r.route(knob, "m3", verb, "mix", 0.3);
    r.route(wobble, "out", lad, "cutoff_hz", 0.22);
    perform(&mut r, knob, None, None);
    r
}

// ---- 2. Neon Highway: legato lead ----------------------------------------------------------

fn neon_highway() -> Rack {
    let mut r = Rack::new();
    let knob = macros(&mut r, [0.55, 0.40, 0.60, 0.35]);
    let m = r.add(
        "midi.in",
        &[
            ("mode", 2.0),
            ("glide", 2.0),
            ("glide_ms", 70.0),
            ("bend", 2.0),
        ],
    );
    let a = r.add(
        "osc.va",
        &[("waveform", 2.0), ("unison", 3.0), ("detune", 16.0)],
    );
    let b = r.add("osc.va", &[("waveform", 3.0), ("pw", 40.0), ("fine", 7.0)]);
    r.next_row();
    let mix = r.add("mixer", &[("level1", 0.5), ("level2", 0.35)]);
    let drive = r.add("drive", &[("drive_db", 6.0), ("trim_db", -3.0)]);
    let lad = r.add(
        "filter.ladder",
        &[("cutoff_hz", 1500.0), ("resonance", 0.3), ("drive_db", 3.0)],
    );
    let env = r.add("env.adsr", &adsr(6.0, 400.0, 0.7, 280.0));
    let vca = r.add("vca", &[("gain", 0.0)]);
    r.next_row();
    let trim = r.add("gain", &[("gain_db", 9.0)]);
    let delay = r.add(
        "delay",
        &[
            ("time_ms", 375.0),
            ("feedback", 40.0),
            ("mix", 0.0),
            ("tone_hz", 4500.0),
        ],
    );
    let chorus = r.add(
        "chorus",
        &[("rate_hz", 0.6), ("depth", 40.0), ("mix", 35.0)],
    );
    let verb = r.add(
        "reverb",
        &[("decay_s", 2.2), ("damp_hz", 5000.0), ("mix", 0.0)],
    );
    let vibrato = r.motion(knob, 5.2);

    r.wire(m, "pitch", a, "pitch");
    r.wire(m, "pitch", b, "pitch");
    r.wire(a, "out", mix, "in1");
    r.wire(b, "out", mix, "in2");
    r.wire(mix, "out", drive, "in");
    r.wire(drive, "out", lad, "in");
    r.wire(lad, "out", vca, "in");
    r.wire(m, "gate", env, "gate");
    r.wire(env, "out", vca, "cv");
    r.wire(vca, "out", trim, "in");
    r.wire(trim, "out", delay, "in");
    r.stereo(delay, chorus);
    r.stereo(chorus, verb);
    r.out_stereo(verb);

    r.route(knob, "m1", lad, "cutoff_hz", 0.5);
    r.route(knob, "m2", drive, "drive_db", 0.45);
    r.route(knob, "m3", delay, "mix", 0.3);
    r.route(knob, "m3", verb, "mix", 0.3);
    r.route(vibrato, "out", a, "fine", 0.1);
    r.route(vibrato, "out", b, "fine", 0.1);
    r.route(m, "velocity", lad, "cutoff_hz", 0.12);
    perform(&mut r, knob, None, None);
    r
}

// ---- 3. Roller Disco: phased chord stabs ---------------------------------------------------

fn roller_disco() -> Rack {
    let mut r = Rack::new();
    let knob = macros(&mut r, [0.55, 0.30, 0.55, 0.60]);
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let a = r.add(
        "osc.va",
        &[("waveform", 2.0), ("unison", 2.0), ("detune", 10.0)],
    );
    let b = r.add("osc.va", &[("waveform", 3.0), ("fine", -8.0)]);
    r.next_row();
    let mix = r.add("mixer", &[("level1", 0.55), ("level2", 0.35)]);
    let drive = r.add("drive", &[("drive_db", 3.0), ("trim_db", -2.0)]);
    let lad = r.add(
        "filter.ladder",
        &[("cutoff_hz", 1800.0), ("resonance", 0.2)],
    );
    let fenv = r.add("env.adsr", &adsr(2.0, 220.0, 0.2, 160.0));
    let aenv = r.add("env.adsr", &adsr(2.0, 280.0, 0.2, 180.0));
    let vca = r.add("vca", &[("gain", 0.0)]);
    r.next_row();
    let trim = r.add("gain", &[("gain_db", 11.0)]);
    let phaser = r.add(
        "phaser",
        &[
            ("stages", 6.0),
            ("rate_hz", 0.35),
            ("depth", 80.0),
            ("feedback", 55.0),
            ("mix", 0.0),
        ],
    );
    let chorus = r.add(
        "chorus",
        &[("rate_hz", 0.5), ("depth", 30.0), ("mix", 25.0)],
    );
    let delay = r.add(
        "delay",
        &[
            ("time_ms", 250.0),
            ("feedback", 30.0),
            ("mix", 0.0),
            ("tone_hz", 5000.0),
        ],
    );
    let verb = r.add(
        "reverb",
        &[("decay_s", 1.6), ("damp_hz", 6000.0), ("mix", 0.0)],
    );

    for o in [a, b] {
        r.wire(m, "pitch", o, "pitch");
    }
    r.wire(a, "out", mix, "in1");
    r.wire(b, "out", mix, "in2");
    r.wire(mix, "out", drive, "in");
    r.wire(drive, "out", lad, "in");
    r.wire(m, "gate", fenv, "gate");
    r.wire(m, "gate", aenv, "gate");
    r.wire(lad, "out", vca, "in");
    r.wire(aenv, "out", vca, "cv");
    r.wire(vca, "out", trim, "in");
    r.wire_both(trim, "out", phaser);
    r.stereo(phaser, chorus);
    // The delay is a mono-in send: it hears the chorus' left side and adds its stereo echo.
    r.wire(chorus, "left", delay, "in");
    r.stereo(chorus, verb);
    r.wire(delay, "left", verb, "in_l");
    r.wire(delay, "right", verb, "in_r");
    r.out_stereo(verb);

    r.route(fenv, "out", lad, "cutoff_hz", 0.3);
    r.route(m, "velocity", lad, "cutoff_hz", 0.15);
    r.route(knob, "m1", lad, "cutoff_hz", 0.5);
    r.route(knob, "m2", drive, "drive_db", 0.45);
    r.route(knob, "m3", delay, "mix", 0.25);
    r.route(knob, "m3", verb, "mix", 0.3);
    r.route(knob, "m4", phaser, "mix", 0.65);
    r.route(knob, "m4", phaser, "rate_hz", 0.2);
    perform(&mut r, knob, None, None);
    r
}

// ---- 4. Violet Hour: supersaw pad ----------------------------------------------------------

fn violet_hour() -> Rack {
    let mut r = Rack::new();
    let knob = macros(&mut r, [0.45, 0.30, 0.65, 0.50]);
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let a = r.add(
        "osc.va",
        &[("waveform", 2.0), ("unison", 4.0), ("detune", 32.0)],
    );
    let b = r.add(
        "osc.va",
        &[
            ("base_hz", 523.25),
            ("waveform", 2.0),
            ("unison", 4.0),
            ("detune", 28.0),
        ],
    );
    r.next_row();
    let mix = r.add("mixer", &[("level1", 0.42), ("level2", 0.2)]);
    let drive = r.add("drive", &[("drive_db", 2.0), ("trim_db", -1.0)]);
    let lad = r.add(
        "filter.ladder",
        &[("cutoff_hz", 1800.0), ("resonance", 0.12)],
    );
    let fenv = r.add("env.adsr", &adsr(500.0, 1500.0, 0.8, 1600.0));
    let aenv = r.add("env.adsr", &adsr(700.0, 1200.0, 0.85, 2200.0));
    let vca = r.add("vca", &[("gain", 0.0)]);
    r.next_row();
    let trim = r.add("gain", &[("gain_db", 6.0)]);
    let chorus = r.add(
        "chorus",
        &[("rate_hz", 0.35), ("depth", 60.0), ("mix", 40.0)],
    );
    let delay = r.add(
        "delay",
        &[
            ("time_ms", 560.0),
            ("feedback", 45.0),
            ("mix", 0.0),
            ("tone_hz", 4000.0),
        ],
    );
    let verb = r.add(
        "reverb",
        &[
            ("decay_s", 7.0),
            ("damp_hz", 5000.0),
            ("mix", 0.0),
            ("predelay_ms", 30.0),
        ],
    );
    let drift = r.motion(knob, 0.12);

    for o in [a, b] {
        r.wire(m, "pitch", o, "pitch");
    }
    r.wire(a, "out", mix, "in1");
    r.wire(b, "out", mix, "in2");
    r.wire(mix, "out", drive, "in");
    r.wire(drive, "out", lad, "in");
    r.wire(m, "gate", fenv, "gate");
    r.wire(m, "gate", aenv, "gate");
    r.wire(lad, "out", vca, "in");
    r.wire(aenv, "out", vca, "cv");
    r.wire(vca, "out", trim, "in");
    r.wire_both(trim, "out", chorus);
    r.wire(chorus, "left", delay, "in");
    r.stereo(chorus, verb);
    r.wire(delay, "left", verb, "in_l");
    r.wire(delay, "right", verb, "in_r");
    r.out_stereo(verb);

    r.route(fenv, "out", lad, "cutoff_hz", 0.25);
    r.route(knob, "m1", lad, "cutoff_hz", 0.5);
    r.route(knob, "m2", drive, "drive_db", 0.4);
    r.route(knob, "m3", delay, "mix", 0.22);
    r.route(knob, "m3", verb, "mix", 0.45);
    r.route(drift, "out", lad, "cutoff_hz", 0.25);
    r.route(knob, "m4", chorus, "rate_hz", 0.1);
    perform(&mut r, knob, None, None);
    r
}

// ---- 5. Glass Elevator: FM keys ------------------------------------------------------------

fn glass_elevator() -> Rack {
    let mut r = Rack::new();
    let knob = macros(&mut r, [0.60, 0.25, 0.60, 0.50]);
    let m = r.add("midi.in", &[("bend", 2.0)]);
    // Algorithm 1 (2 into 1, 4 into 3, 6 into 5): a mellow body pair, a bright tine pair that
    // is gone in a tenth of a second, and a soft detuned shimmer.
    let voice = r.add("osc.fm6", &[("algorithm", 1.0), ("index", 0.4)]);
    // ratio, fine, level, attack, decay, sustain, release, velocity
    let ops: [[f32; 8]; 6] = [
        [1.0, 0.0, 1.0, 1.0, 2400.0, 0.1, 260.0, 0.35],
        [1.0, 0.0, 0.9, 1.0, 650.0, 0.06, 200.0, 0.8],
        [1.0, 6.0, 0.7, 1.0, 2800.0, 0.1, 260.0, 0.35],
        [14.0, 0.0, 0.5, 0.5, 90.0, 0.0, 60.0, 0.9],
        [1.0, -5.0, 0.4, 1.0, 3200.0, 0.1, 260.0, 0.3],
        [2.0, 0.0, 0.5, 1.0, 1400.0, 0.05, 200.0, 0.6],
    ];
    for (k, op) in ops.iter().enumerate() {
        let names = [
            "ratio", "fine", "level", "attack", "decay", "sustain", "release", "vel",
        ];
        for (name, &v) in names.iter().zip(op) {
            r.e.set_param(voice, &format!("{name}{}", k + 1), v);
        }
    }
    r.next_row();
    let drive = r.add("drive", &[("drive_db", 0.0), ("trim_db", -1.0)]);
    let lad = r.add(
        "filter.ladder",
        &[("cutoff_hz", 5000.0), ("resonance", 0.05)],
    );
    let trim = r.add("gain", &[("gain_db", 8.5)]);
    let chorus = r.add("chorus", &[("rate_hz", 0.7), ("depth", 35.0), ("mix", 0.0)]);
    let delay = r.add(
        "delay",
        &[
            ("time_ms", 375.0),
            ("feedback", 35.0),
            ("mix", 0.0),
            ("tone_hz", 6000.0),
            ("mode", 1.0),
        ],
    );
    let verb = r.add(
        "reverb",
        &[("decay_s", 3.5), ("damp_hz", 7000.0), ("mix", 0.0)],
    );

    r.wire(m, "pitch", voice, "pitch");
    r.wire(m, "gate", voice, "gate");
    r.wire(m, "velocity", voice, "velocity");
    r.wire(voice, "out", drive, "in");
    r.wire(drive, "out", lad, "in");
    r.wire(lad, "out", trim, "in");
    r.wire_both(trim, "out", chorus);
    r.wire(chorus, "left", delay, "in");
    r.stereo(chorus, verb);
    r.wire(delay, "left", verb, "in_l");
    r.wire(delay, "right", verb, "in_r");
    r.out_stereo(verb);

    r.route(knob, "m1", voice, "index", 0.5);
    r.route(knob, "m1", lad, "cutoff_hz", 0.3);
    r.route(knob, "m2", drive, "drive_db", 0.35);
    r.route(knob, "m3", delay, "mix", 0.25);
    r.route(knob, "m3", verb, "mix", 0.35);
    r.route(knob, "m4", chorus, "mix", 0.6);
    r.route(knob, "m4", chorus, "rate_hz", 0.15);
    perform(&mut r, knob, None, None);
    r
}

// ---- 6. Chrome Chase: arpeggio over a pumping held chord -----------------------------------

fn chrome_chase() -> Rack {
    let mut r = Rack::new();
    let knob = macros(&mut r, [0.50, 0.35, 0.60, 0.50]);
    let clock = r.add("clock", &[("bpm", 118.0)]);
    let div = r.add("clock.div", &[("div", 4.0)]);
    let arp = r.add(
        "arp",
        &[("mode", 2.0), ("octaves", 2.0), ("gate_len", 55.0)],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    r.next_row();
    let lead = r.add(
        "osc.va",
        &[("waveform", 2.0), ("unison", 2.0), ("detune", 12.0)],
    );
    let lead_env = r.add("env.adsr", &adsr(2.0, 180.0, 0.1, 120.0));
    let lead_vca = r.add("vca", &[("gain", 0.0)]);
    let pad = r.add(
        "osc.va",
        &[("waveform", 2.0), ("unison", 3.0), ("detune", 20.0)],
    );
    let pad_env = r.add("env.adsr", &adsr(60.0, 400.0, 0.8, 400.0));
    let pad_vca = r.add("vca", &[("gain", 0.0)]);
    r.next_row();
    // The pump: every beat an envelope ducks the held chord, as much as Motion allows.
    let pump_env = r.add("env.adsr", &adsr(1.0, 240.0, 0.0, 30.0));
    let pump_depth = r.add("vca", &[("gain", 0.0)]);
    let pump_cv = r.add("attenuverter", &[("amount", -0.85), ("offset", 1.0)]);
    let duck = r.add("vca", &[("gain", 0.0)]);
    let mix = r.add("mixer", &[("level1", 0.5), ("level2", 0.4)]);
    r.next_row();
    let drive = r.add("drive", &[("drive_db", 5.0), ("trim_db", -3.0)]);
    let lad = r.add(
        "filter.ladder",
        &[("cutoff_hz", 1500.0), ("resonance", 0.3), ("drive_db", 2.0)],
    );
    let trim = r.add("gain", &[("gain_db", -6.0)]);
    let delay = r.add(
        "delay",
        &[
            ("sync", 3.0),
            ("time_ms", 375.0),
            ("feedback", 45.0),
            ("mix", 0.0),
            ("tone_hz", 4500.0),
        ],
    );
    let verb = r.add(
        "reverb",
        &[("decay_s", 2.6), ("damp_hz", 5000.0), ("mix", 0.0)],
    );
    let sweep = r.motion(knob, 0.2);

    r.wire(clock, "gate", arp, "clock");
    r.wire(clock, "reset", arp, "reset");
    r.wire(clock, "gate", div, "clock");
    r.wire(clock, "reset", div, "reset");
    r.wire(clock, "gate", delay, "clock");
    r.wire(arp, "pitch", lead, "pitch");
    r.wire(arp, "gate", lead_env, "gate");
    r.wire(lead, "out", lead_vca, "in");
    r.wire(lead_env, "out", lead_vca, "cv");
    r.wire(m, "pitch", pad, "pitch");
    r.wire(m, "gate", pad_env, "gate");
    r.wire(pad, "out", pad_vca, "in");
    r.wire(pad_env, "out", pad_vca, "cv");
    r.wire(div, "gate", pump_env, "gate");
    r.wire(pump_env, "out", pump_depth, "in");
    r.route(knob, "m4", pump_depth, "gain", 1.0);
    r.wire(pump_depth, "out", pump_cv, "in");
    r.wire(pad_vca, "out", duck, "in");
    r.wire(pump_cv, "out", duck, "cv");
    r.wire(lead_vca, "out", mix, "in1");
    r.wire(duck, "out", mix, "in2");
    r.wire(mix, "out", drive, "in");
    r.wire(drive, "out", lad, "in");
    r.wire(lad, "out", trim, "in");
    r.wire(trim, "out", delay, "in");
    r.stereo(delay, verb);
    r.out_stereo(verb);

    r.route(knob, "m1", lad, "cutoff_hz", 0.5);
    r.route(knob, "m2", drive, "drive_db", 0.45);
    r.route(knob, "m3", delay, "mix", 0.3);
    r.route(knob, "m3", verb, "mix", 0.35);
    r.route(sweep, "out", lad, "cutoff_hz", 0.25);
    perform(&mut r, knob, Some(clock), None);
    r
}

// ---- 7. Midnight Drop: self-playing piece with cues ----------------------------------------

/// (name, [Filter, Drive, Space, Motion] targets, morph in beats, launch time in the clip, s)
const DROP_CUES: [(&str, [f32; 4], f32, f32); 4] = [
    ("Intro", [0.15, 0.20, 0.35, 0.20], 0.0, 0.0),
    ("Build", [0.90, 0.55, 0.50, 0.85], 16.0, 4.0),
    ("Drop", [0.70, 0.90, 0.20, 0.50], 0.0, 12.0),
    ("Break", [0.25, 0.10, 0.85, 0.30], 4.0, 20.0),
];

fn midnight_drop() -> Rack {
    let mut r = Rack::new();
    let knob = macros(&mut r, DROP_CUES[0].1);
    let cue = r.add("cues", &[]);
    let clock = r.add("clock", &[("bpm", 120.0)]);
    let bass_seq = r.add("seq", &[("length", 8.0), ("gate_len", 60.0)]);
    let lead_seq = r.add("seq", &[("length", 8.0), ("gate_len", 35.0)]);
    r.next_row();
    // Bass line: E, E, E+octave, E, B, E, D, A (semitones from E1 up in the saw's C2 reference).
    for (k, p) in [4.0, 4.0, 16.0, 4.0, 11.0, 4.0, 2.0, 9.0]
        .into_iter()
        .enumerate()
    {
        r.e.set_param(bass_seq, &format!("p{}", k + 1), p - 24.0);
    }
    for (k, p) in [16.0, 23.0, 28.0, 23.0, 19.0, 23.0, 16.0, 11.0]
        .into_iter()
        .enumerate()
    {
        r.e.set_param(lead_seq, &format!("p{}", k + 1), p);
        r.e.set_param(
            lead_seq,
            &format!("g{}", k + 1),
            [1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 1.0, 0.0][k],
        );
    }
    let bass = r.add(
        "osc.va",
        &[("waveform", 2.0), ("unison", 2.0), ("detune", 14.0)],
    );
    let bass_env = r.add("env.adsr", &adsr(2.0, 220.0, 0.35, 90.0));
    let bass_vca = r.add("vca", &[("gain", 0.0)]);
    let lead = r.add(
        "osc.va",
        &[
            ("waveform", 3.0),
            ("pw", 35.0),
            ("unison", 2.0),
            ("detune", 10.0),
        ],
    );
    let lead_env = r.add("env.adsr", &adsr(2.0, 160.0, 0.0, 100.0));
    let lead_vca = r.add("vca", &[("gain", 0.0)]);
    r.next_row();
    let m = r.add(
        "midi.in",
        &[
            ("mode", 2.0),
            ("glide", 2.0),
            ("glide_ms", 60.0),
            ("bend", 2.0),
        ],
    );
    let play = r.add(
        "osc.va",
        &[("waveform", 2.0), ("unison", 3.0), ("detune", 15.0)],
    );
    let play_env = r.add("env.adsr", &adsr(6.0, 300.0, 0.7, 260.0));
    let play_vca = r.add("vca", &[("gain", 0.0)]);
    let mix = r.add(
        "mixer",
        &[("level1", 0.55), ("level2", 0.3), ("level3", 0.35)],
    );
    r.next_row();
    let drive = r.add("drive", &[("drive_db", 4.0), ("trim_db", -3.0)]);
    let lad = r.add(
        "filter.ladder",
        &[("cutoff_hz", 220.0), ("resonance", 0.35), ("drive_db", 3.0)],
    );
    let trim = r.add("gain", &[("gain_db", -7.0)]);
    let delay = r.add(
        "delay",
        &[
            ("sync", 3.0),
            ("time_ms", 375.0),
            ("feedback", 42.0),
            ("mix", 0.0),
            ("tone_hz", 4500.0),
        ],
    );
    let verb = r.add(
        "reverb",
        &[("decay_s", 3.2), ("damp_hz", 5000.0), ("mix", 0.0)],
    );
    let shake = r.motion(knob, 0.5);

    r.wire(clock, "gate", bass_seq, "clock");
    r.wire(clock, "reset", bass_seq, "reset");
    r.wire(clock, "gate", lead_seq, "clock");
    r.wire(clock, "reset", lead_seq, "reset");
    r.wire(clock, "gate", delay, "clock");
    r.wire(bass_seq, "pitch", bass, "pitch");
    r.wire(bass_seq, "gate", bass_env, "gate");
    r.wire(bass, "out", bass_vca, "in");
    r.wire(bass_env, "out", bass_vca, "cv");
    r.wire(lead_seq, "pitch", lead, "pitch");
    r.wire(lead_seq, "gate", lead_env, "gate");
    r.wire(lead, "out", lead_vca, "in");
    r.wire(lead_env, "out", lead_vca, "cv");
    r.wire(m, "pitch", play, "pitch");
    r.wire(m, "gate", play_env, "gate");
    r.wire(play, "out", play_vca, "in");
    r.wire(play_env, "out", play_vca, "cv");
    r.wire(bass_vca, "out", mix, "in1");
    r.wire(lead_vca, "out", mix, "in2");
    r.wire(play_vca, "out", mix, "in3");
    r.wire(mix, "out", drive, "in");
    r.wire(drive, "out", lad, "in");
    r.wire(lad, "out", trim, "in");
    r.wire(trim, "out", delay, "in");
    r.stereo(delay, verb);
    r.out_stereo(verb);

    r.route(knob, "m1", lad, "cutoff_hz", 0.55);
    r.route(knob, "m2", drive, "drive_db", 0.5);
    r.route(knob, "m3", delay, "mix", 0.3);
    r.route(knob, "m3", verb, "mix", 0.4);
    r.route(shake, "out", lad, "cutoff_hz", 0.2);

    for (name, targets, beats, _) in DROP_CUES {
        let n = cues::add(&mut r.e, cue, &|_| 0).expect("cue");
        cues::rename(&mut r.e, cue, n, name);
        // The cues move macros only: both sequencers keep playing through every section.
        cues::set_target(&mut r.e, cue, n, bass_seq, None);
        cues::set_target(&mut r.e, cue, n, lead_seq, None);
        for (k, v) in targets.into_iter().enumerate() {
            cues::set_macro(&mut r.e, cue, n, knob, k as u8, Some(v));
        }
        cues::set_morph(&mut r.e, cue, n, beats);
    }
    perform(&mut r, knob, Some(clock), Some((cue, DROP_CUES.len())));
    r
}

// ---- 8. Android Choir: robot vowel voice ---------------------------------------------------

fn android_choir() -> Rack {
    let mut r = Rack::new();
    let knob = macros(&mut r, [0.30, 0.30, 0.60, 0.40]);
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let osc = r.add(
        "osc.va",
        &[
            ("base_hz", 130.81),
            ("waveform", 2.0),
            ("unison", 2.0),
            ("detune", 8.0),
        ],
    );
    let env = r.add("env.adsr", &adsr(40.0, 400.0, 0.8, 300.0));
    let vca = r.add("vca", &[("gain", 0.0)]);
    r.next_row();
    let formant = r.add("filter.formant", &[("vowel", 0.5), ("q", 1.6)]);
    let trim = r.add("gain", &[("gain_db", 14.0)]);
    let crush = r.add(
        "bitcrusher",
        &[("bits", 7.0), ("downsample", 3.0), ("mix", 0.0)],
    );
    let chorus = r.add(
        "chorus",
        &[("rate_hz", 0.4), ("depth", 45.0), ("mix", 40.0)],
    );
    let delay = r.add(
        "delay",
        &[
            ("time_ms", 430.0),
            ("feedback", 38.0),
            ("mix", 0.0),
            ("tone_hz", 4000.0),
        ],
    );
    let verb = r.add(
        "reverb",
        &[("decay_s", 3.0), ("damp_hz", 6000.0), ("mix", 0.0)],
    );
    let wobble = r.motion(knob, 0.6);

    r.wire(m, "pitch", osc, "pitch");
    r.wire(m, "gate", env, "gate");
    r.wire(osc, "out", vca, "in");
    r.wire(env, "out", vca, "cv");
    r.wire(vca, "out", formant, "in");
    r.wire(formant, "out", trim, "in");
    r.wire_both(trim, "out", crush);
    r.stereo(crush, chorus);
    r.wire(chorus, "left", delay, "in");
    r.stereo(chorus, verb);
    r.wire(delay, "left", verb, "in_l");
    r.wire(delay, "right", verb, "in_r");
    r.out_stereo(verb);

    r.route(knob, "m1", formant, "vowel", 0.75);
    r.route(knob, "m2", crush, "mix", 0.8);
    r.route(knob, "m3", delay, "mix", 0.25);
    r.route(knob, "m3", verb, "mix", 0.35);
    r.route(wobble, "out", formant, "vowel", 0.12);
    r.route(wobble, "out", osc, "fine", 0.08);
    perform(&mut r, knob, None, None);
    r
}

// ---- the bank entries and their clips -------------------------------------------------------

/// One thing that happens in a clip: a key goes down (velocity) or up.
struct Hit(f32, u8, Option<u8>);

fn on(t: f32, n: u8) -> Hit {
    Hit(t, n, Some(100))
}
fn off(t: f32, n: u8) -> Hit {
    Hit(t, n, None)
}
fn chord(t: f32, len: f32, notes: &[u8]) -> Vec<Hit> {
    notes
        .iter()
        .flat_map(|&n| [on(t, n), off(t + len, n)])
        .collect()
}

struct Preset {
    slug: &'static str,
    name: &'static str,
    category: &'static str,
    tags: &'static [&'static str],
    description: &'static str,
    /// What to listen for in the clip.
    listen: &'static str,
    build: fn() -> Rack,
    phrase: fn() -> Vec<Hit>,
    /// Seconds of the clip.
    secs: f32,
}

fn bass_phrase() -> Vec<Hit> {
    let notes = [
        40u8, 40, 52, 40, 47, 40, 50, 45, 40, 40, 52, 43, 47, 40, 38, 40,
    ];
    let mut v = Vec::new();
    for (i, &n) in notes.iter().enumerate() {
        let t = i as f32 * 0.5;
        // Every fourth note overlaps the next so the legato glide is heard.
        let len = if i % 4 == 3 { 0.62 } else { 0.38 };
        v.push(on(t, n));
        v.push(off(t + len, n));
    }
    v
}

fn lead_phrase() -> Vec<Hit> {
    let mut v = Vec::new();
    for (i, &n) in [64u8, 67, 71, 69, 67, 64, 66, 71, 76, 74, 71, 67]
        .iter()
        .enumerate()
    {
        let t = i as f32 * 0.5;
        v.push(on(t, n));
        v.push(off(t + 0.58, n));
    }
    v
}

fn disco_phrase() -> Vec<Hit> {
    let mut v = Vec::new();
    // Em9 and A7 stabs on the off-beats, twice each.
    for bar in 0..4 {
        let notes: &[u8] = if bar % 2 == 0 {
            &[52, 59, 62, 66, 71]
        } else {
            &[57, 61, 64, 67, 71]
        };
        for &t in &[0.25, 0.75, 1.0, 1.5] {
            v.extend(chord(bar as f32 * 2.0 + t, 0.18, notes));
        }
    }
    v
}

fn pad_phrase() -> Vec<Hit> {
    let mut v = chord(0.0, 4.5, &[52, 59, 64, 67, 71]);
    v.extend(chord(4.5, 4.5, &[48, 55, 60, 64, 67]));
    v.extend(chord(9.0, 5.0, &[50, 57, 62, 66, 69]));
    v
}

fn keys_phrase() -> Vec<Hit> {
    let mut v = Vec::new();
    for (i, &n) in [64u8, 71, 76, 74, 71, 67, 69, 71].iter().enumerate() {
        v.push(on(i as f32 * 0.45, n));
        v.push(off(i as f32 * 0.45 + 0.4, n));
    }
    v.extend(chord(3.8, 2.5, &[52, 59, 64, 67, 71]));
    v.extend(chord(6.5, 3.0, &[53, 57, 64, 69, 72]));
    v
}

fn chase_phrase() -> Vec<Hit> {
    let mut v = chord(0.0, 8.0, &[52, 59, 64]);
    v.extend(chord(8.0, 8.0, &[48, 55, 60, 64]));
    v
}

fn drop_phrase() -> Vec<Hit> {
    // A hand-played line over the last third: the Drop and the Break.
    let mut v = Vec::new();
    for (i, &n) in [76u8, 79, 83, 81, 79, 76].iter().enumerate() {
        v.push(on(12.0 + i as f32 * 1.0, n));
        v.push(off(12.0 + i as f32 * 1.0 + 0.9, n));
    }
    v
}

fn choir_phrase() -> Vec<Hit> {
    let mut v = chord(0.0, 3.5, &[52, 59, 64]);
    v.extend(chord(3.5, 3.5, &[55, 62, 67]));
    v.extend(chord(7.0, 3.5, &[57, 64, 69]));
    v.extend(chord(10.5, 4.0, &[52, 59, 64, 71]));
    v
}

fn presets() -> Vec<Preset> {
    vec![
        Preset {
            slug: "cherry-chainsaw",
            name: "Cherry Chainsaw",
            category: "Bass",
            tags: &["mono", "distorted", "filtered", "pumping", "french touch"],
            description: "A mono bass in the Justice mould: three detuned saws over a square sub, pushed into a drive and a resonant ladder filter that snaps open on every note, then squeezed by a compressor. Filter opens the tone, Drive adds grit, Space adds a short room, Motion adds a filter wobble; the mod wheel also opens the filter.",
            listen: "The snap of the filter on each note and the growl as Drive goes up; the glide on the overlapped notes.",
            build: cherry_chainsaw,
            phrase: bass_phrase,
            secs: 10.0,
        },
        Preset {
            slug: "neon-highway",
            name: "Neon Highway",
            category: "Lead",
            tags: &["mono", "legato", "glide", "bright", "80s"],
            description: "A wide mono lead for late-night driving: a detuned saw stack and a pulse, a light drive, a ladder filter that opens with velocity, chorus, a dotted echo and a room. Legato notes glide. Filter brightens, Drive adds bite, Space adds echo and room, Motion adds vibrato.",
            listen: "The glide between overlapped notes and the echo trailing each phrase.",
            build: neon_highway,
            phrase: lead_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "roller-disco",
            name: "Roller Disco",
            category: "Keys",
            tags: &["poly", "chords", "stabs", "phased", "disco", "french touch"],
            description: "Disco chord stabs through a six-stage phaser: saw and square under a plucked filter, chorus, a short echo and a room. Play short chords on the off-beat. Filter opens the stab, Drive adds crunch, Space adds echo and room, Motion brings the phaser in and speeds it up.",
            listen: "The phaser sweep moving through each stab; turn Motion to zero and it goes still.",
            build: roller_disco,
            phrase: disco_phrase,
            secs: 9.5,
        },
        Preset {
            slug: "violet-hour",
            name: "Violet Hour",
            category: "Pad",
            tags: &["poly", "supersaw", "slow", "80s", "evolving"],
            description: "A wide supersaw pad in the M83 mould: two stacks of detuned saws an octave apart, a slow filter bloom, chorus, echo and a long room. Hold chords and let them breathe. Filter opens the bloom, Drive thickens the saws, Space is echo and room, Motion drifts the filter and the chorus.",
            listen: "The slow bloom of the filter on each chord and the long tail after release.",
            build: violet_hour,
            phrase: pad_phrase,
            secs: 17.0,
        },
        Preset {
            slug: "glass-elevator",
            name: "Glass Elevator",
            category: "Keys",
            tags: &["poly", "fm", "electric piano", "bell", "french touch"],
            description: "A six-operator FM electric piano with a glassy tine that fades in a tenth of a second, over a mellow body. Velocity brightens it. Filter sets the FM brightness, Drive adds grit, Space adds ping-pong echo and room, Motion brings in the chorus.",
            listen: "The tine on the attack against the mellow tail, and the way Filter opens the whole spectrum.",
            build: glass_elevator,
            phrase: keys_phrase,
            secs: 11.0,
        },
        Preset {
            slug: "chrome-chase",
            name: "Chrome Chase",
            category: "Groove",
            tags: &["arp", "pumping", "poly", "118bpm", "french touch"],
            description: "Hold a chord and the arpeggiator plays it up and down over two octaves at 118 BPM, while the held chord also sounds underneath as a pad that ducks on every beat. Press Run first. Filter opens the arpeggio, Drive adds bite, Space adds echo and room, Motion deepens the pumping duck and a slow filter sweep.",
            listen: "The pad breathing under the arpeggio on each beat; raise Motion and it pumps harder.",
            build: chrome_chase,
            phrase: chase_phrase,
            secs: 17.0,
        },
        Preset {
            slug: "midnight-drop",
            name: "Midnight Drop",
            category: "Piece",
            tags: &["sequenced", "cues", "build-up", "drop", "120bpm", "french touch"],
            description: "A self-playing bass and lead with a hand-played voice on top, in four cues: Intro (dark and quiet), Build (Filter opens over four bars and Motion rises), Drop (Drive up, full and hard), Break (washed in Space). Press Run, then launch the cues on the next bar with the pads or CC 40-43; turn a macro to take it over.",
            listen: "The filter opening through the Build, the Drop landing hard, and the Break draining into echo.",
            build: midnight_drop,
            phrase: drop_phrase,
            secs: 30.0,
        },
        Preset {
            slug: "android-choir",
            name: "Android Choir",
            category: "Voice",
            tags: &["poly", "vowel", "robot", "formant", "chords"],
            description: "A choir of machines: a saw stack through the formant filter, then a bitcrusher, chorus, echo and room. Play slow chords. Filter sweeps the vowel from A to U, Drive crushes the voice into a robot, Space adds echo and room, Motion makes the vowel wander and adds vibrato.",
            listen: "The vowel wandering on a held chord; raise Drive and the choir turns to a robot.",
            build: android_choir,
            phrase: choir_phrase,
            secs: 16.0,
        },
    ]
}

fn write_wav(path: &Path, left: &[f32], right: &[f32]) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for (l, r) in left.iter().zip(right) {
        for s in [l, r] {
            w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16)
                .unwrap();
        }
    }
    w.finalize().unwrap();
}

/// Plays `hits` through the engine for `secs` seconds. A clocked patch is run at 0.5 s; the
/// piece also launches its cues at their times.
fn render(
    state: &kabl_core::PatchState,
    hits: &[Hit],
    secs: f32,
    piece: bool,
) -> (Vec<f32>, Vec<f32>) {
    let collector = Collector::new();
    let mut engine = PatchEngine::new(&collector.handle(), state, SR, 8).expect("compiles");
    let clocks: Vec<ModuleId> = state
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "clock")
        .map(|(&id, _)| id)
        .collect();
    let cue_id = state
        .modules
        .iter()
        .find(|(_, m)| m.kind == "cues")
        .map(|(&id, _)| id);
    let (mut left, mut right) = (Vec::new(), Vec::new());
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    let blocks = (secs * SR) as usize / BLOCK;
    let mut next_cue = 0;
    for b in 0..blocks {
        let t0 = (b * BLOCK) as f32 / SR;
        let t1 = t0 + BLOCK as f32 / SR;
        if b == 0 {
            for &c in &clocks {
                engine.transport(c, Transport::Stop);
            }
        }
        if t0 <= 0.5 && 0.5 < t1 {
            for &c in &clocks {
                engine.transport(c, Transport::Run);
            }
        }
        if piece {
            if let (Some(id), Some(&(_, _, _, at))) = (cue_id, DROP_CUES.get(next_cue)) {
                // Launch half a bar early so the cue lands on its bar line (a bar is 2 s).
                if t1 > at + 0.5 - 1.0 {
                    let mut cmds = Vec::new();
                    cues::launch(&mut cmds, state, id, next_cue + 1).expect("launch");
                    for mut c in cmds {
                        if let Command::Launch(l) = &mut c {
                            if next_cue == 0 {
                                l.timing = Timing::Now;
                            }
                        }
                        engine.command(&c);
                    }
                    next_cue += 1;
                }
            }
        }
        for h in hits {
            if h.0 >= t0 && h.0 < t1 {
                engine.key(match h.2 {
                    Some(velocity) => KeyEvent::On {
                        note: h.1,
                        velocity,
                    },
                    None => KeyEvent::Off { note: h.1 },
                });
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
    match args.get(1).map(String::as_str) {
        Some("write") => {
            for p in presets() {
                let rack = (p.build)();
                let dir = Path::new(&args[2]).join("presets").join(p.slug);
                let _ = std::fs::remove_dir_all(&dir);
                kabl_core::save(&dir, rack.e.log()).expect("save");
                let (keys, sequence) = Meta::play_of(rack.e.state());
                let meta = Meta {
                    name: p.name.into(),
                    category: p.category.into(),
                    tags: p.tags.iter().map(|t| t.to_string()).collect(),
                    description: p.description.into(),
                    keys,
                    sequence,
                };
                std::fs::write(
                    dir.join("sound.toml"),
                    toml::to_string_pretty(&meta).unwrap(),
                )
                .expect("sound.toml");
                println!("wrote {}", dir.display());
            }
        }
        Some("clips") => {
            let out = Path::new(&args[3]);
            std::fs::create_dir_all(out).unwrap();
            for p in presets() {
                let log = kabl_core::load(&Path::new(&args[2]).join("presets").join(p.slug))
                    .expect("load patch");
                let (l, r) = render(
                    log.state(),
                    &(p.phrase)(),
                    p.secs,
                    p.slug == "midnight-drop",
                );
                let (l2, _) = render(
                    log.state(),
                    &(p.phrase)(),
                    p.secs,
                    p.slug == "midnight-drop",
                );
                assert_eq!(l, l2, "{}: render is deterministic", p.slug);
                write_wav(&out.join(format!("{}.wav", p.slug)), &l, &r);
                let peak = l.iter().chain(&r).fold(0.0f32, |m, s| m.max(s.abs()));
                println!("{:18} peak {peak:.3}  | {}", p.slug, p.listen);
            }
        }
        Some("phrases") => {
            // For docs/presets/scripts/host.py: {slug: {secs, hits: [[t, note, velocity|0]], cues: [[cc, t]]}}.
            let entries: Vec<String> = presets()
                .iter()
                .map(|p| {
                    let hits: Vec<String> = (p.phrase)()
                        .iter()
                        .map(|h| format!("[{:.3},{},{}]", h.0, h.1, h.2.unwrap_or(0)))
                        .collect();
                    let cues: Vec<String> = if p.slug == "midnight-drop" {
                        DROP_CUES
                            .iter()
                            .zip(40..)
                            .map(|((_, _, _, at), cc)| format!("[{cc},{:.1}]", at + 1.0))
                            .collect()
                    } else {
                        Vec::new()
                    };
                    format!(
                        "\"{}\":{{\"secs\":{},\"hits\":[{}],\"cues\":[{}]}}",
                        p.slug,
                        p.secs,
                        hits.join(","),
                        cues.join(",")
                    )
                })
                .collect();
            std::fs::write(&args[2], format!("{{{}}}\n", entries.join(","))).unwrap();
        }
        Some("macros") => {
            // Peak and loudness (dBFS) with every macro at 0, at 1, and with only Space at 0.
            let db = |v: f32| 20.0 * v.max(1e-9).log10();
            for p in presets() {
                // No cues launched: the macros stay where this loop puts them.
                let piece = false;
                let mut line = format!("{:16}", p.slug);
                for (label, set) in [
                    ("all 0", [Some(0.0); 4]),
                    ("all 1", [Some(1.0); 4]),
                    ("no Space", [None, None, Some(0.0), None]),
                ] {
                    let mut rack = (p.build)();
                    let knob = *rack
                        .e
                        .state()
                        .modules
                        .iter()
                        .find(|(_, m)| m.kind == "macro")
                        .unwrap()
                        .0;
                    for (k, v) in set.iter().enumerate() {
                        if let Some(v) = v {
                            rack.e.set_param(knob, &format!("m{}", k + 1), *v);
                        }
                    }
                    let (l, r) = render(rack.e.state(), &(p.phrase)(), p.secs, piece);
                    let peak = l.iter().chain(&r).fold(0.0f32, |m, s| m.max(s.abs()));
                    let ms = l.iter().chain(&r).map(|s| s * s).sum::<f32>() / (2 * l.len()) as f32;
                    assert!(peak.is_finite(), "{}: finite", p.slug);
                    line += &format!(
                        "  {label}: peak {:6.1} rms {:6.1} |",
                        db(peak),
                        db(ms.sqrt())
                    );
                }
                println!("{line}");
            }
        }
        _ => eprintln!("usage: presets_pilot write|clips <patches dir> [out dir] | macros"),
    }
}
