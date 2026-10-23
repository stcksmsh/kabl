//! The pilot factory presets (docs/presets/BRIEF.md): builds eight patches from the existing
//! modules, writes them under `patches/presets/`, and renders one clip per preset by playing a
//! short MIDI phrase through the real engine.
//!
//! ```sh
//! cargo run -p kabl-ui --example presets_pilot -- write patches
//! cargo run --release -p kabl-ui --example presets_pilot -- clips patches docs/presets/clips
//! ```
//!
//! Every preset names its own macros after what they do for that sound (four in the pilot, on
//! CC 20-23). Effect mixes are 0 on the module and driven by the space macro (m3), so that
//! macro at 0 is dry and the stored macro value is the baked-in wet amount.

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

const SR_DEFAULT: f32 = 48000.0;

/// The render rate: 48 kHz, or `PP_SR` for an A/B at another rate.
fn sr() -> f32 {
    std::env::var("PP_SR")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(SR_DEFAULT)
}

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
    /// An LFO whose depth is macro 4: the LFO runs through a VCA the macro opens.
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

/// A preset's four macros: the names it gives them and their stored values, on CC 20-23.
fn macros(r: &mut Rack, names: [&str; 4], values: [f32; 4]) -> ModuleId {
    let knob = r.add("macro", &[]);
    for (i, name) in names.iter().enumerate() {
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

// ---- shared stages ---------------------------------------------------------------------------

/// The tail every preset shares: a short mono-in echo (a slapback, or a tempo-synced echo when
/// `clock` is given) and a tight room, both at mix 0 until the Space macro opens them. `src` is
/// a stereo module (`left`/`right`); returns the room, whose outputs go on to the limiter.
#[allow(clippy::too_many_arguments)]
fn space_tail(
    r: &mut Rack,
    knob: ModuleId,
    src: ModuleId,
    echo: &[(&str, f32)],
    clock: Option<ModuleId>,
    echo_max: f32,
    room_decay_s: f32,
    room_max: f32,
) -> ModuleId {
    let delay = r.add("delay", echo);
    let verb = r.add(
        "reverb",
        &[
            ("decay_s", room_decay_s),
            ("damp_hz", 3500.0),
            ("mix", 0.0),
            ("predelay_ms", 0.0),
            ("width", 60.0),
        ],
    );
    r.wire(src, "left", delay, "in");
    if let Some(c) = clock {
        r.wire(c, "gate", delay, "clock");
    }
    r.stereo(src, verb);
    r.wire(delay, "left", verb, "in_l");
    r.wire(delay, "right", verb, "in_r");
    r.route(knob, "m3", delay, "mix", echo_max);
    r.route(knob, "m3", verb, "mix", room_max);
    verb
}

// ---- 1. Cinder Block: blunt, compressed mono bass --------------------------------------------

fn cinder_block() -> Rack {
    let mut r = Rack::new();
    let knob = macros(
        &mut r,
        ["Bite", "Grit", "Room", "Crush"],
        [0.40, 0.60, 0.25, 0.05],
    );
    let m = r.add("midi.in", &[("mode", 1.0), ("bend", 2.0)]);
    let saw = r.add(
        "osc.va",
        &[("waveform", 2.0), ("unison", 2.0), ("detune", 9.0)],
    );
    let sub = r.add("osc.va", &[("base_hz", 130.81), ("waveform", 3.0)]);
    r.next_row();
    let mix = r.add("mixer", &[("level1", 0.6), ("level2", 0.5)]);
    let drive = r.add("drive", &[("drive_db", 18.0), ("trim_db", -9.0)]);
    let lad = r.add(
        "filter.ladder",
        &[("cutoff_hz", 260.0), ("resonance", 0.3), ("drive_db", 8.0)],
    );
    let fenv = r.add("env.adsr", &adsr(1.0, 150.0, 0.1, 80.0));
    let aenv = r.add("env.adsr", &adsr(1.0, 200.0, 0.75, 50.0));
    r.next_row();
    let vca = r.add("vca", &[("gain", 0.0)]);
    let trim = r.add("gain", &[("gain_db", 11.5)]);
    let crush = r.add(
        "bitcrusher",
        &[("bits", 9.0), ("downsample", 2.0), ("mix", 0.0)],
    );
    let comp = r.add(
        "compressor",
        &[
            ("threshold_db", -26.0),
            ("ratio", 8.0),
            ("attack_ms", 2.0),
            ("release_ms", 90.0),
            ("knee_db", 3.0),
            ("makeup_db", 9.0),
        ],
    );

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
    r.wire_both(trim, "out", crush);
    r.stereo(crush, comp);
    let tail = space_tail(
        &mut r,
        knob,
        comp,
        &[
            ("time_ms", 95.0),
            ("feedback", 18.0),
            ("mix", 0.0),
            ("tone_hz", 3500.0),
        ],
        None,
        0.22,
        0.45,
        0.18,
    );
    r.out_stereo(tail);

    r.route(fenv, "out", lad, "cutoff_hz", 0.2);
    r.route(m, "wheel", lad, "cutoff_hz", 0.25);
    r.route(knob, "m1", lad, "cutoff_hz", 0.5);
    r.route(knob, "m2", drive, "drive_db", 0.35);
    r.route(knob, "m4", crush, "mix", 0.6);
    perform(&mut r, knob, None, None);
    r
}

// ---- 3. Parquet Loop: a held chord as a filtered disco loop ---------------------------------

fn parquet_loop() -> Rack {
    let mut r = Rack::new();
    let knob = macros(
        &mut r,
        ["Cutoff", "Crunch", "Space", "Glide"],
        [0.50, 0.45, 0.25, 0.50],
    );
    let clock = r.add("clock", &[("bpm", 122.0), ("swing", 25.0)]);
    // Poly with glide on: each voice slides from the last pitch it played, so a chord change
    // moves the chord tones into each other. The Glide macro sets the time; at 0 it is 5 ms.
    let m = r.add(
        "midi.in",
        &[("glide", 1.0), ("glide_ms", 5.0), ("bend", 2.0)],
    );
    let a = r.add(
        "osc.va",
        &[("waveform", 2.0), ("unison", 2.0), ("detune", 8.0)],
    );
    let b = r.add("osc.va", &[("waveform", 3.0), ("fine", -7.0)]);
    r.next_row();
    let mix = r.add("mixer", &[("level1", 0.55), ("level2", 0.35)]);
    let drive = r.add("drive", &[("drive_db", 12.0), ("trim_db", -5.0)]);
    let lad = r.add(
        "filter.ladder",
        &[("cutoff_hz", 650.0), ("resonance", 0.55)],
    );
    let fenv = r.add("env.adsr", &adsr(1.0, 200.0, 0.3, 90.0));
    let aenv = r.add("env.adsr", &adsr(1.0, 400.0, 0.65, 90.0));
    let vca = r.add("vca", &[("gain", 0.0)]);
    r.next_row();
    // The filter loop: a saw LFO locked to the clock closes the filter once a beat.
    let chop = r.add("lfo", &[("waveform", 2.0), ("sync", 3.0), ("rate_hz", 2.0)]);
    let trim = r.add("gain", &[("gain_db", 10.5)]);
    let phaser = r.add(
        "phaser",
        &[
            ("stages", 4.0),
            ("rate_hz", 0.5),
            ("depth", 85.0),
            ("feedback", 60.0),
            ("mix", 45.0),
        ],
    );
    let comp = r.add(
        "compressor",
        &[
            ("threshold_db", -24.0),
            ("ratio", 4.0),
            ("attack_ms", 8.0),
            ("release_ms", 100.0),
            ("makeup_db", 6.0),
        ],
    );

    r.wire(clock, "gate", chop, "clock");
    r.wire(clock, "reset", chop, "reset");
    r.wire(m, "pitch", a, "pitch");
    r.wire(m, "pitch", b, "pitch");
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
    r.stereo(phaser, comp);
    let tail = space_tail(
        &mut r,
        knob,
        comp,
        &[
            ("sync", 3.0),
            ("time_ms", 370.0),
            ("feedback", 30.0),
            ("mix", 0.0),
            ("tone_hz", 4000.0),
        ],
        Some(clock),
        0.2,
        0.6,
        0.15,
    );
    r.out_stereo(tail);

    r.route(fenv, "out", lad, "cutoff_hz", 0.3);
    r.route(chop, "out", lad, "cutoff_hz", 0.3);
    r.route(m, "velocity", lad, "cutoff_hz", 0.15);
    r.route(knob, "m1", lad, "cutoff_hz", 0.5);
    r.route(knob, "m2", drive, "drive_db", 0.4);
    r.route(knob, "m4", m, "glide_ms", 0.8);
    perform(&mut r, knob, Some(clock), None);
    r
}

/// A sequencer lane for the drums: gates on the given 1-based steps of its 8 (two beats).
fn drum_lane(r: &mut Rack, on: &[usize]) -> ModuleId {
    let seq = r.add("seq", &[]);
    for k in 1..=8 {
        r.e.set_param(seq, &format!("g{k}"), f32::from(u8::from(on.contains(&k))));
        r.e.set_param(seq, &format!("v{k}"), 0.9);
    }
    seq
}

// ---- 7. Loading Dock: filter-house piece with cues ------------------------------------------

/// (name, [Sweep, Drive, Wash, Pump] targets, morph in beats, launch time in the clip, s)
const DROP_CUES: [(&str, [f32; 4], f32, f32); 5] = [
    ("Intro", [0.12, 0.30, 0.00, 0.50], 0.0, 0.0),
    ("Build", [0.90, 0.75, 0.65, 0.80], 12.0, 4.0),
    // The pre-drop suck: filter shut, the wash still ringing, the pump at full.
    ("Gap", [0.08, 0.60, 0.85, 1.00], 0.0, 10.0),
    ("Drop", [0.78, 1.00, 0.10, 1.00], 0.0, 12.0),
    ("Break", [0.20, 0.25, 0.60, 0.30], 4.0, 20.0),
];

fn loading_dock() -> Rack {
    let mut r = Rack::new();
    let knob = macros(&mut r, ["Sweep", "Drive", "Wash", "Pump"], DROP_CUES[0].1);
    let cue = r.add("cues", &[]);
    let clock = r.add("clock", &[("bpm", 124.0), ("swing", 28.0)]);
    let bass_seq = r.add("seq", &[("length", 8.0), ("gate_len", 45.0)]);
    let stab_seq = r.add("seq", &[("length", 8.0), ("gate_len", 30.0)]);
    r.next_row();
    // Bass line: syncopated, E root with the octave and the fifth (semitones from the saw's C2).
    for (k, p) in [4.0, 4.0, 16.0, 4.0, 11.0, 4.0, 2.0, 9.0]
        .into_iter()
        .enumerate()
    {
        r.e.set_param(bass_seq, &format!("p{}", k + 1), p - 24.0);
        r.e.set_param(
            bass_seq,
            &format!("g{}", k + 1),
            [1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 1.0, 0.0][k],
        );
    }
    // Stabs on the off-beats.
    for (k, p) in [16.0, 16.0, 19.0, 16.0, 23.0, 19.0, 16.0, 14.0]
        .into_iter()
        .enumerate()
    {
        r.e.set_param(stab_seq, &format!("p{}", k + 1), p);
        r.e.set_param(
            stab_seq,
            &format!("g{}", k + 1),
            [0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0][k],
        );
    }
    let bass = r.add(
        "osc.va",
        &[("waveform", 2.0), ("unison", 2.0), ("detune", 12.0)],
    );
    let bass_env = r.add("env.adsr", &adsr(1.0, 200.0, 0.35, 60.0));
    let bass_vca = r.add("vca", &[("gain", 0.0)]);
    let stab = r.add(
        "osc.va",
        &[
            ("waveform", 3.0),
            ("pw", 35.0),
            ("unison", 2.0),
            ("detune", 10.0),
        ],
    );
    let stab_env = r.add("env.adsr", &adsr(1.0, 120.0, 0.0, 60.0));
    let stab_vca = r.add("vca", &[("gain", 0.0)]);
    r.next_row();
    let m = r.add("midi.in", &[("mode", 1.0), ("bend", 2.0)]);
    let play = r.add(
        "osc.va",
        &[("waveform", 2.0), ("unison", 2.0), ("detune", 8.0)],
    );
    let play_env = r.add("env.adsr", &adsr(2.0, 200.0, 0.6, 90.0));
    let play_vca = r.add("vca", &[("gain", 0.0)]);
    let mix = r.add(
        "mixer",
        &[("level1", 0.55), ("level2", 0.3), ("level3", 0.35)],
    );
    // The riser: pink noise the Wash macro lets in, joining after the drive (the drive turned
    // it into a loud, steady hiss even at a low Wash) and before the filter, which the Sweep
    // macro opens over it.
    let noise = r.add("noise", &[("color", 1.0)]);
    let riser = r.add("vca", &[("gain", 0.0)]);
    let post = r.add("mixer", &[("level1", 1.0), ("level2", 0.5)]);
    r.next_row();
    // Real drums (drum.kick and friends): a four-on-the-floor kick, a clap on the 2 and 4 and
    // open hats on the off-beats, each on a 2-beat sequencer lane. The kick also feeds the
    // sidechain of the compressor that ducks the music: the Pump macro is that ratio.
    let kick_seq = drum_lane(&mut r, &[1, 5]);
    let clap_seq = drum_lane(&mut r, &[5]);
    let hat_seq = drum_lane(&mut r, &[3, 7]);
    let kick = r.add(
        "drum.kick",
        &[("level_db", -3.0), ("decay_ms", 300.0), ("drive_db", 10.0)],
    );
    let clap = r.add("drum.clap", &[("level_db", -9.0)]);
    let hat = r.add("drum.hat", &[("level_db", -10.0)]);
    let drums = r.add(
        "mixer",
        &[("level1", 0.45), ("level2", 0.2), ("level3", 0.15)],
    );
    let duck = r.add(
        "compressor",
        &[
            ("threshold_db", -34.0),
            ("ratio", 1.0),
            ("attack_ms", 1.0),
            ("release_ms", 220.0),
            ("knee_db", 3.0),
        ],
    );
    let mix_l = r.add("mixer", &[("level1", 1.0), ("level2", 1.0)]);
    let mix_r = r.add("mixer", &[("level1", 1.0), ("level2", 1.0)]);
    r.next_row();
    let drive = r.add("drive", &[("drive_db", 20.0), ("trim_db", -9.0)]);
    let lad = r.add(
        "filter.ladder",
        &[("cutoff_hz", 170.0), ("resonance", 0.6), ("drive_db", 5.0)],
    );
    let trim = r.add("gain", &[("gain_db", 9.0)]);
    let crush = r.add(
        "bitcrusher",
        &[("bits", 9.0), ("downsample", 2.0), ("mix", 0.0)],
    );
    let comp = r.add(
        "compressor",
        &[
            ("threshold_db", -26.0),
            ("ratio", 8.0),
            ("attack_ms", 3.0),
            ("release_ms", 100.0),
            ("makeup_db", 6.0),
        ],
    );

    r.wire(clock, "gate", bass_seq, "clock");
    r.wire(clock, "reset", bass_seq, "reset");
    r.wire(clock, "gate", stab_seq, "clock");
    r.wire(clock, "reset", stab_seq, "reset");
    r.wire(bass_seq, "pitch", bass, "pitch");
    r.wire(bass_seq, "gate", bass_env, "gate");
    r.wire(bass, "out", bass_vca, "in");
    r.wire(bass_env, "out", bass_vca, "cv");
    r.wire(stab_seq, "pitch", stab, "pitch");
    r.wire(stab_seq, "gate", stab_env, "gate");
    r.wire(stab, "out", stab_vca, "in");
    r.wire(stab_env, "out", stab_vca, "cv");
    r.wire(m, "pitch", play, "pitch");
    r.wire(m, "gate", play_env, "gate");
    r.wire(play, "out", play_vca, "in");
    r.wire(play_env, "out", play_vca, "cv");
    r.wire(bass_vca, "out", mix, "in1");
    r.wire(stab_vca, "out", mix, "in2");
    r.wire(play_vca, "out", mix, "in3");
    r.wire(mix, "out", drive, "in");
    r.wire(drive, "out", post, "in1");
    r.wire(post, "out", lad, "in");
    r.wire(lad, "out", trim, "in");
    r.wire_both(trim, "out", crush);
    r.stereo(crush, comp);
    r.wire(comp, "left", duck, "in_l");
    r.wire(comp, "right", duck, "in_r");
    r.wire(kick, "out", duck, "sc");
    for (seq, voice, gate) in [
        (kick_seq, kick, "gate"),
        (clap_seq, clap, "gate"),
        (hat_seq, hat, "open"),
    ] {
        r.wire(clock, "gate", seq, "clock");
        r.wire(clock, "reset", seq, "reset");
        r.wire(seq, "gate", voice, gate);
        r.wire(seq, "velocity", voice, "velocity");
    }
    r.wire(kick, "out", drums, "in1");
    r.wire(clap, "out", drums, "in2");
    r.wire(hat, "out", drums, "in3");
    let tail = space_tail(
        &mut r,
        knob,
        duck,
        &[
            ("sync", 3.0),
            ("time_ms", 360.0),
            ("feedback", 40.0),
            ("mix", 0.0),
            ("tone_hz", 4000.0),
        ],
        Some(clock),
        0.3,
        1.2,
        0.3,
    );
    // The drums join after the echo and the room, then the limiter.
    r.wire(tail, "left", mix_l, "in1");
    r.wire(tail, "right", mix_r, "in1");
    r.wire(drums, "out", mix_l, "in2");
    r.wire(drums, "out", mix_r, "in2");
    let limiter = r.add("limiter", &[("ceiling_db", -1.5), ("release_ms", 150.0)]);
    let out = r.add("out", &[]);
    r.wire(mix_l, "out", limiter, "in_l");
    r.wire(mix_r, "out", limiter, "in_r");
    r.wire(limiter, "left", out, "left");
    r.wire(limiter, "right", out, "right");

    r.route(knob, "m4", duck, "ratio", 0.95);
    r.wire(noise, "out", riser, "in");
    r.wire(riser, "out", post, "in2");
    r.route(knob, "m3", riser, "gain", 0.9);
    r.route(knob, "m1", lad, "cutoff_hz", 0.65);
    r.route(knob, "m2", drive, "drive_db", 0.6);
    r.route(knob, "m2", crush, "mix", 0.6);

    for (name, targets, beats, _) in DROP_CUES {
        let n = cues::add(&mut r.e, cue, &|_| 0).expect("cue");
        cues::rename(&mut r.e, cue, n, name);
        // The cues move macros only: both sequencers keep playing through every section.
        cues::set_target(&mut r.e, cue, n, bass_seq, None);
        cues::set_target(&mut r.e, cue, n, stab_seq, None);
        for (k, v) in targets.into_iter().enumerate() {
            cues::set_macro(&mut r.e, cue, n, knob, k as u8, Some(v));
        }
        cues::set_morph(&mut r.e, cue, n, beats);
    }
    perform(&mut r, knob, Some(clock), Some((cue, DROP_CUES.len())));
    r
}

// ---- 2. Neon Highway: legato lead ----------------------------------------------------------

fn neon_highway() -> Rack {
    let mut r = Rack::new();
    let knob = macros(
        &mut r,
        ["Cutoff", "Bite", "Space", "Vibrato"],
        [0.55, 0.40, 0.60, 0.35],
    );
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

// ---- 5. Glass Elevator: FM keys ------------------------------------------------------------

fn glass_elevator() -> Rack {
    let mut r = Rack::new();
    let knob = macros(
        &mut r,
        ["Tine", "Grit", "Space", "Chorus"],
        [0.60, 0.25, 0.60, 0.50],
    );
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

/// Seconds of one 16th at 124 BPM: the grid the keyboard phrases play on.
const S16: f32 = 60.0 / 124.0 / 4.0;

/// Notes on the 16th grid: (step, note, length in steps).
fn steps(pattern: &[(u32, u8, f32)]) -> Vec<Hit> {
    pattern
        .iter()
        .flat_map(|&(s, n, len)| {
            let t = s as f32 * S16;
            [on(t, n), off(t + len * S16 * 0.9, n)]
        })
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
    let bar = [
        (0, 40, 1.0),
        (3, 40, 1.0),
        (6, 52, 1.0),
        (8, 40, 1.0),
        (10, 47, 1.0),
        (12, 40, 1.0),
        (14, 38, 1.0),
        (15, 40, 1.0),
    ];
    let mut p: Vec<(u32, u8, f32)> = bar.to_vec();
    p.extend(
        bar.iter()
            .map(|&(s, n, l)| (s + 16, if n == 47 { 43 } else { n }, l)),
    );
    p.extend(bar.iter().map(|&(s, n, l)| (s + 32, n, l)));
    p.extend(
        bar.iter()
            .map(|&(s, n, l)| (s + 48, if n == 52 { 55 } else { n }, l)),
    );
    steps(&p)
}

fn loop_phrase() -> Vec<Hit> {
    // Em9 for two bars, then A7, then Em9 again: the loop is the filter, the chord just sits.
    let bar = 16.0 * S16;
    let mut v = chord(0.0, 2.0 * bar - 0.1, &[52, 59, 62, 66, 71]);
    v.extend(chord(2.0 * bar, 2.0 * bar - 0.1, &[57, 61, 64, 67, 71]));
    v.extend(chord(4.0 * bar, 1.5 * bar, &[52, 59, 62, 66, 71]));
    v
}

fn drop_phrase() -> Vec<Hit> {
    // A hand-played line over the Drop and the Break.
    steps(&[
        (100, 76, 2.0),
        (104, 79, 2.0),
        (108, 83, 2.0),
        (112, 81, 3.0),
        (118, 79, 2.0),
        (122, 76, 4.0),
        (164, 71, 2.0),
        (168, 74, 6.0),
    ])
}

fn neon_phrase() -> Vec<Hit> {
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

fn glass_phrase() -> Vec<Hit> {
    let mut v = Vec::new();
    for (i, &n) in [64u8, 71, 76, 74, 71, 67, 69, 71].iter().enumerate() {
        v.push(on(i as f32 * 0.45, n));
        v.push(off(i as f32 * 0.45 + 0.4, n));
    }
    v.extend(chord(3.8, 2.5, &[52, 59, 64, 67, 71]));
    v.extend(chord(6.5, 3.0, &[53, 57, 64, 69, 72]));
    v
}

// ---- Undertow: reese sub bass for hip-hop and drum and bass ----------------------------------

fn undertow() -> Rack {
    let mut r = Rack::new();
    let knob = macros(
        &mut r,
        ["Cutoff", "Weight", "Room", "Beating"],
        [0.45, 0.35, 0.30, 0.40],
    );
    let m = r.add(
        "midi.in",
        &[
            ("mode", 2.0),
            ("glide", 2.0),
            ("glide_ms", 40.0),
            ("bend", 2.0),
        ],
    );
    // Two saws a few cents apart beat slowly against each other; a sine an octave down holds the weight.
    let a = r.add("osc.va", &[("waveform", 2.0), ("fine", 12.0)]);
    let b = r.add("osc.va", &[("waveform", 2.0), ("fine", -12.0)]);
    let sub = r.add("osc.va", &[("base_hz", 130.81), ("waveform", 0.0)]);
    r.next_row();
    let mix = r.add(
        "mixer",
        &[("level1", 0.45), ("level2", 0.45), ("level3", 0.5)],
    );
    let drive = r.add("drive", &[("drive_db", 8.0), ("trim_db", -4.0)]);
    let lad = r.add(
        "filter.ladder",
        &[("cutoff_hz", 420.0), ("resonance", 0.25)],
    );
    let fenv = r.add("env.adsr", &adsr(5.0, 600.0, 0.5, 200.0));
    let aenv = r.add("env.adsr", &adsr(5.0, 800.0, 0.9, 150.0));
    let vca = r.add("vca", &[("gain", 0.0)]);
    r.next_row();
    let trim = r.add("gain", &[("gain_db", 7.8)]);
    let comp = r.add(
        "compressor",
        &[
            ("threshold_db", -22.0),
            ("ratio", 4.0),
            ("attack_ms", 10.0),
            ("release_ms", 150.0),
            ("makeup_db", 3.0),
        ],
    );
    let verb = r.add(
        "reverb",
        &[
            ("decay_s", 0.7),
            ("damp_hz", 2500.0),
            ("mix", 0.0),
            ("predelay_ms", 0.0),
            ("width", 50.0),
        ],
    );
    let sweep = r.motion(knob, 0.4);

    for o in [a, b, sub] {
        r.wire(m, "pitch", o, "pitch");
    }
    r.wire(a, "out", mix, "in1");
    r.wire(b, "out", mix, "in2");
    r.wire(sub, "out", mix, "in3");
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

    r.route(fenv, "out", lad, "cutoff_hz", 0.25);
    r.route(m, "wheel", lad, "cutoff_hz", 0.25);
    r.route(knob, "m1", lad, "cutoff_hz", 0.5);
    r.route(knob, "m2", drive, "drive_db", 0.4);
    r.route(knob, "m3", verb, "mix", 0.25);
    r.route(sweep, "out", lad, "cutoff_hz", 0.3);
    // Beating also pulls the two saws apart, so the beating speeds up.
    r.route(knob, "m4", a, "fine", 0.08);
    r.route(knob, "m4", b, "fine", -0.08);
    perform(&mut r, knob, None, None);
    r
}

// ---- Salt Flat: slow ambient pad ------------------------------------------------------------

fn salt_flat() -> Rack {
    let mut r = Rack::new();
    let knob = macros(
        &mut r,
        ["Bloom", "Saturation", "Space", "Texture"],
        [0.50, 0.20, 0.65, 0.0],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let a = r.add(
        "osc.va",
        &[("waveform", 1.0), ("unison", 4.0), ("detune", 4.0)],
    );
    let b = r.add(
        "osc.va",
        &[("base_hz", 523.25), ("waveform", 1.0), ("fine", 5.0)],
    );
    r.next_row();
    let mix = r.add("mixer", &[("level1", 0.45), ("level2", 0.0)]);
    let drive = r.add("drive", &[("drive_db", 0.0), ("trim_db", 0.0)]);
    let lad = r.add(
        "filter.ladder",
        &[("cutoff_hz", 1400.0), ("resonance", 0.1)],
    );
    let fenv = r.add("env.adsr", &adsr(2500.0, 3000.0, 0.7, 3000.0));
    let aenv = r.add("env.adsr", &adsr(2200.0, 2000.0, 0.9, 5000.0));
    let vca = r.add("vca", &[("gain", 0.0)]);
    r.next_row();
    let trim = r.add("gain", &[("gain_db", 8.0)]);
    let chorus = r.add(
        "chorus",
        &[("rate_hz", 0.15), ("depth", 55.0), ("mix", 18.0)],
    );
    let delay = r.add(
        "delay",
        &[
            ("time_ms", 700.0),
            ("feedback", 50.0),
            ("mix", 0.0),
            ("tone_hz", 3000.0),
        ],
    );
    let verb = r.add(
        "reverb",
        &[
            ("decay_s", 9.0),
            ("damp_hz", 4500.0),
            ("mix", 0.0),
            ("predelay_ms", 40.0),
        ],
    );
    // Slow filter drift at a fixed depth.
    let drift = r.add("lfo", &[("rate_hz", 0.08)]);

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
    r.route(knob, "m3", delay, "mix", 0.3);
    r.route(knob, "m3", verb, "mix", 0.6);
    r.route(drift, "out", lad, "cutoff_hz", 0.15);
    // Texture: the shimmer layer, the chorus smear and the unison spread come in together.
    r.route(knob, "m4", mix, "level2", 0.3);
    r.route(knob, "m4", chorus, "mix", 0.3);
    r.route(knob, "m4", a, "detune", 0.1);
    perform(&mut r, knob, None, None);
    r
}

// ---- Boxwood: FM mallet pluck ---------------------------------------------------------------

fn boxwood() -> Rack {
    let mut r = Rack::new();
    let knob = macros(
        &mut r,
        ["Brightness", "Warmth", "Space", "Chorus"],
        [0.60, 0.20, 0.45, 0.40],
    );
    let m = r.add("midi.in", &[("bend", 2.0)]);
    let voice = r.add("osc.fm6", &[("algorithm", 1.0), ("index", 0.6)]);
    // ratio, fine, level, attack, decay, sustain, release, velocity: a wooden bar, a bright
    // overtone that is gone at once, and a click.
    let ops: [[f32; 8]; 6] = [
        [1.0, 0.0, 1.0, 0.5, 700.0, 0.0, 150.0, 0.3],
        [4.0, 0.0, 0.6, 0.5, 90.0, 0.0, 50.0, 0.8],
        [1.0, 3.0, 0.5, 0.5, 520.0, 0.0, 130.0, 0.3],
        [10.0, 0.0, 0.4, 0.5, 30.0, 0.0, 20.0, 0.9],
        [2.0, 0.0, 0.3, 0.5, 320.0, 0.0, 100.0, 0.3],
        [1.0, 0.0, 0.3, 0.5, 260.0, 0.0, 100.0, 0.4],
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
    let drive = r.add("drive", &[("drive_db", 0.0), ("trim_db", 0.0)]);
    let lad = r.add(
        "filter.ladder",
        &[("cutoff_hz", 5000.0), ("resonance", 0.05)],
    );
    let trim = r.add("gain", &[("gain_db", 8.0)]);
    let chorus = r.add("chorus", &[("rate_hz", 0.5), ("depth", 30.0), ("mix", 0.0)]);
    let delay = r.add(
        "delay",
        &[
            ("time_ms", 280.0),
            ("feedback", 30.0),
            ("mix", 0.0),
            ("tone_hz", 5000.0),
        ],
    );
    let verb = r.add(
        "reverb",
        &[("decay_s", 1.8), ("damp_hz", 6000.0), ("mix", 0.0)],
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
    r.route(knob, "m3", delay, "mix", 0.2);
    r.route(knob, "m3", verb, "mix", 0.35);
    r.route(knob, "m4", chorus, "mix", 0.6);
    r.route(knob, "m4", chorus, "rate_hz", 0.15);
    perform(&mut r, knob, None, None);
    r
}

fn undertow_phrase() -> Vec<Hit> {
    // Half-time: long low notes, two overlapped so the glide slides.
    vec![
        on(0.0, 28),
        off(2.0, 28),
        on(1.9, 31),
        off(3.6, 31),
        on(4.0, 26),
        off(5.6, 26),
        on(5.5, 33),
        off(7.5, 33),
        on(8.0, 28),
        off(10.0, 28),
    ]
}

fn salt_phrase() -> Vec<Hit> {
    let mut v = chord(0.0, 5.0, &[50, 57, 62, 65, 69]);
    v.extend(chord(5.0, 5.0, &[46, 53, 58, 62, 65]));
    v.extend(chord(10.0, 6.0, &[48, 55, 60, 64, 67]));
    v
}

fn boxwood_phrase() -> Vec<Hit> {
    let mut v = Vec::new();
    let line = [
        60u8, 64, 67, 72, 71, 67, 64, 62, 60, 64, 67, 71, 69, 65, 62, 60,
    ];
    for (i, &n) in line.iter().enumerate() {
        let t = i as f32 * 0.25;
        v.push(on(t, n));
        v.push(off(t + 0.2, n));
    }
    v.extend(chord(4.2, 1.5, &[48, 55, 64]));
    v.extend(chord(5.8, 2.0, &[50, 57, 65]));
    v
}

fn presets() -> Vec<Preset> {
    vec![
        Preset {
            slug: "cinder-block",
            name: "Cinder Block",
            category: "Bass",
            tags: &["mono", "distorted", "compressed", "french touch", "signature"],
            description: "A blunt mono bass: two saws over a square sub, hit with 18 dB of drive, a filter that closes on every note and a compressor squeezing it flat. No wobble, just dirt. Play short, off-beat notes. Bite opens the filter (the mod wheel does too), Grit pushes the drive harder, Room adds a slapback and a tight room, Crush mixes in a bitcrusher.",
            listen: "The blunt, flattened attack and the dry, steady dirt: nothing moves on its own; Grit and Crush make it nastier.",
            build: cinder_block,
            phrase: bass_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "parquet-loop",
            name: "Parquet Loop",
            category: "Keys",
            tags: &["poly", "filtered", "phased", "disco", "122bpm", "signature"],
            description: "Hold a chord and it becomes a filtered disco loop: saws and pulse into crunch, a resonant filter that closes on every beat (locked to the clock), a four-stage phaser and a compressor, at 122 BPM with swing. Press Run for the filter loop. Notes slide into each other: change chords and the voices glide. Cutoff opens the filter, Crunch adds grit, Space adds a dotted echo and a tight room, Glide sets how long the slide takes (all the way down, notes just step).",
            listen: "The filter closing on the beat under a held chord, and the chord tones sliding into the next chord; Glide down removes the slide.",
            build: parquet_loop,
            phrase: loop_phrase,
            secs: 10.0,
        },
        Preset {
            slug: "loading-dock",
            name: "Loading Dock",
            category: "Piece",
            tags: &["sequenced", "cues", "drums", "drop", "124bpm", "filter house", "signature"],
            description: "A self-playing filter-house piece at 124 BPM with swing: a four-on-the-floor kick with a clap and open hats, a bass and a stab line that duck hard to the kick through the compressor sidechain, a resonant filter, a noise riser and a hand-played voice on top. Five cues: Intro (filter shut, dry, silent until Run), Build (the filter opens over three bars as the riser and the wash rise), Gap (everything shut but the wash), Drop (drive and pump at full), Break (back down, wet). Press Run, then launch the cues with the pads or CC 40-44; turn a macro to take over. Sweep opens the filter, Drive pushes the drive and the bitcrusher, Wash brings in the riser, the echo and the room, Pump sets how hard the kick ducks the bass and stabs.",
            listen: "The filter and riser rising through the Build, the gap, the Drop landing with the pump at full, and the Break pulling back to a dark loop.",
            build: loading_dock,
            phrase: drop_phrase,
            secs: 30.0,
        },
        Preset {
            slug: "glass-elevator",
            name: "Glass Elevator",
            category: "Keys",
            tags: &["poly", "fm", "electric piano", "bell", "pop"],
            description: "A six-operator FM electric piano with a glassy tine that fades in a tenth of a second, over a mellow body. Velocity brightens it. Tine sets the FM brightness, Grit adds grit, Space adds ping-pong echo and room, Chorus brings in the chorus.",
            listen: "The tine on the attack against the mellow tail, and the way Tine opens the whole spectrum.",
            build: glass_elevator,
            phrase: glass_phrase,
            secs: 11.0,
        },
        Preset {
            slug: "undertow",
            name: "Undertow",
            category: "Bass",
            tags: &["mono", "reese", "sub", "glide", "drum and bass", "hip-hop"],
            description: "A reese sub bass: two saws a few cents apart beating slowly against each other over a sine an octave down, through a gentle drive and a low-pass filter, with a short glide between overlapped notes. Play long low notes. Cutoff opens the tone, Weight thickens it with drive, Room adds a short room, Beating widens the beating and sweeps the filter.",
            listen: "The slow beating and the weight of the sub; raise Beating and it speeds up while the filter sweeps.",
            build: undertow,
            phrase: undertow_phrase,
            secs: 11.0,
        },
        Preset {
            slug: "salt-flat",
            name: "Salt Flat",
            category: "Pad",
            tags: &["poly", "ambient", "cinematic", "slow", "evolving"],
            description: "A slow, wide ambient pad: soft triangle stacks with a slow filter bloom, a light chorus, a long echo and a long room. Hold open chords and let them breathe. It starts clean. Bloom opens the filter, Saturation adds a little drive, Space is the echo and the room, Texture brings in a grainy, staticky layer: a high shimmer voice, a wider chorus and a looser unison.",
            listen: "The slow bloom of each chord and the long tail that carries into the next; then raise Texture to about 0.7 for the grainy, staticky version.",
            build: salt_flat,
            phrase: salt_phrase,
            secs: 18.0,
        },
        Preset {
            slug: "boxwood",
            name: "Boxwood",
            category: "Pluck",
            tags: &["poly", "mallet", "fm", "wooden", "velocity"],
            description: "An FM mallet, like a wooden bar: a round body with a bright overtone that vanishes at once and a small click. Velocity brings out the overtone. Brightness sets the FM brightness, Warmth adds warmth, Space adds an echo and a medium room, Chorus brings in the chorus.",
            listen: "The click and the wooden ring of each note, and the short bloom when Space is raised.",
            build: boxwood,
            phrase: boxwood_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "neon-highway",
            name: "Neon Highway",
            category: "Lead",
            tags: &["mono", "legato", "glide", "bright", "synthwave", "80s"],
            description: "A wide mono lead for late-night driving: a detuned saw stack and a pulse, a light drive, a ladder filter that opens with velocity, chorus, a dotted echo and a room. Legato notes glide. Cutoff brightens, Bite adds drive, Space adds echo and room, Vibrato adds vibrato.",
            listen: "The glide between overlapped notes and the echo trailing each phrase.",
            build: neon_highway,
            phrase: neon_phrase,
            secs: 9.0,
        },
    ]
}

fn write_wav(path: &Path, left: &[f32], right: &[f32]) {
    // `PP_F32` writes 32-bit float, for measuring without the 16-bit step.
    let float = std::env::var("PP_F32").is_ok();
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: sr() as u32,
        bits_per_sample: if float { 32 } else { 16 },
        sample_format: if float {
            hound::SampleFormat::Float
        } else {
            hound::SampleFormat::Int
        },
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for (l, r) in left.iter().zip(right) {
        for s in [l, r] {
            if float {
                w.write_sample(*s).unwrap();
            } else {
                w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16)
                    .unwrap();
            }
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
    // `PP_SET="kind.param=value,..."` overrides a param on every module of that kind (A/B).
    let mut state = state.clone();
    for item in std::env::var("PP_SET").unwrap_or_default().split(',') {
        if let Some((target, v)) = item.split_once('=') {
            let (kind, param) = target.split_once('.').expect("kind.param");
            for m in state.modules.values_mut().filter(|m| m.kind == kind) {
                m.params.insert(param.into(), v.parse().expect("number"));
            }
        }
    }
    let state = &state;
    // `PP_HITS="t:note:velocity;..."` (velocity 0 = key up) replaces the phrase.
    let custom: Vec<Hit> = std::env::var("PP_HITS")
        .unwrap_or_default()
        .split(';')
        .filter_map(|h| {
            let mut f = h.split(':');
            let (t, n, v): (f32, u8, u8) = (
                f.next()?.parse().ok()?,
                f.next()?.parse().ok()?,
                f.next()?.parse().ok()?,
            );
            Some(Hit(t, n, (v > 0).then_some(v)))
        })
        .collect();
    let hits = if custom.is_empty() { hits } else { &custom[..] };
    let rate = sr();
    let mut engine = PatchEngine::new(&collector.handle(), state, rate, 8).expect("compiles");
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
    let blocks = (secs * rate) as usize / BLOCK;
    let mut next_cue = 0;
    for b in 0..blocks {
        let t0 = (b * BLOCK) as f32 / rate;
        let t1 = t0 + BLOCK as f32 / rate;
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
                let (l, r) = render(log.state(), &(p.phrase)(), p.secs, p.slug == "loading-dock");
                let (l2, _) = render(log.state(), &(p.phrase)(), p.secs, p.slug == "loading-dock");
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
                    let cues: Vec<String> = if p.slug == "loading-dock" {
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
