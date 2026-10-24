//! The factory basses and leads (docs/presets/BRIEF.md, first part of the bank): builds the
//! patches from the existing modules, writes them under `patches/presets/`, renders a clip per
//! preset by playing a short MIDI phrase through the real engine, and prints the level and the
//! library entries.
//!
//! ```sh
//! cargo run -p kabl-ui --example presets_bass_lead -- write patches
//! cargo run --release -p kabl-ui --example presets_bass_lead -- clips patches out_dir [group]
//! cargo run --release -p kabl-ui --example presets_bass_lead -- levels patches
//! ```
//!
//! Same recipe as `presets_pilot.rs`: every preset names its macros after what they do (CC 20
//! upward), echo and room mixes are 0 on the module and driven by the space macro, so the stored
//! value is the baked-in wet amount and that macro at 0 is dry. A macro that fades a modulator in
//! is a depth route (the macro scales the route's amount).

use std::path::Path;

use basedrop::Collector;
use kabl_core::{ModuleId, PortRef, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::PatchEngine;
use kabl_modules::builtins::Transport;
use kabl_ui::library::Meta;
use kabl_ui::perform::{CC_PREFIX, PIN_PREFIX, TRANSPORT};
use kabl_ui::PatchEditor;

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
    /// A route that sits at 0 until macro `mac` of `knob` opens it, up to `depth` of the knob's
    /// travel: an LFO or envelope whose depth is a macro.
    #[allow(clippy::too_many_arguments)]
    fn depth_route(
        &mut self,
        a: ModuleId,
        ap: &str,
        b: ModuleId,
        param: &str,
        knob: ModuleId,
        mac: &str,
        depth: f32,
    ) {
        let cable = self.e.connect_route(jack(a, ap), b, param);
        self.e.set_route_amount(cable, 0.0, true);
        let d = self
            .e
            .connect_cable_route(jack(knob, mac), cable, "amount")
            .expect("a route has a depth");
        self.e.set_route_amount(d, depth, true);
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

/// A preset's macros: the names it gives them and their stored values, on CC 20 upward.
fn macros(r: &mut Rack, set: &[(&str, f32)]) -> ModuleId {
    let knob = r.add("macro", &[]);
    for (i, (name, v)) in set.iter().enumerate() {
        let key = format!("m{}", i + 1);
        r.e.set_label(knob, &format!("name.{key}"), Some((*name).into()));
        r.e.set_param(knob, &key, *v);
    }
    knob
}

/// Pins (Perform panel order) and CC numbers: transport first when the patch has a clock, then
/// the `n` macros on CC 20 upward.
fn perform(r: &mut Rack, knob: ModuleId, n: usize, clock: Option<ModuleId>) {
    let mut pins = Vec::new();
    if let Some(c) = clock {
        pins.push((c, TRANSPORT.to_string()));
    }
    pins.extend((1..=n).map(|k| (knob, format!("m{k}"))));
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
    r.e.set_presentation(&changes);
}

// ---- shared stages ---------------------------------------------------------------------------

/// The tail every preset shares: a short mono-in echo (a slapback, or a tempo-synced echo when
/// `clock` is given) and a tight room, both at mix 0 until the space macro `mac` opens them. `src` is
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
    mac: &str,
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
    r.route(knob, mac, delay, "mix", echo_max);
    r.route(knob, mac, verb, "mix", room_max);
    verb
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
    /// Delivery group for listening (1 = first eight).
    group: u8,
    phrase: fn() -> Vec<Hit>,
    /// Seconds of the clip.
    secs: f32,
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

/// Plays `hits` through the engine for `secs` seconds. A clocked patch is run at 0.5 s.
fn render(state: &kabl_core::PatchState, hits: &[Hit], secs: f32) -> (Vec<f32>, Vec<f32>) {
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
    let (mut left, mut right) = (Vec::new(), Vec::new());
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    let blocks = (secs * rate) as usize / BLOCK;
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

// ---- the voice recipe -----------------------------------------------------------------------

/// One oscillator of a voice: waveform (0 sine, 1 triangle, 2 saw, 3 square), octave against the
/// key (0 plays the note, -1 an octave down), fine tune in cents, unison, pulse width, level.
struct Osc {
    wave: f32,
    oct: i32,
    fine: f32,
    unison: f32,
    detune: f32,
    pw: f32,
    level: f32,
}

fn osc(wave: f32, oct: i32, level: f32) -> Osc {
    Osc {
        wave,
        oct,
        fine: 0.0,
        unison: 1.0,
        detune: 15.0,
        pw: 50.0,
        level,
    }
}

impl Osc {
    fn fine(mut self, cents: f32) -> Self {
        self.fine = cents;
        self
    }
    fn unison(mut self, voices: f32, detune: f32) -> Self {
        self.unison = voices;
        self.detune = detune;
        self
    }
    fn pw(mut self, pw: f32) -> Self {
        self.pw = pw;
        self
    }
}

/// The mono part every bass and lead shares: `midi.in`, oscillators, mixer, drive, ladder
/// filter, amp envelope, trim. Stereo stages follow with `Voice::stage`, then `finish`.
struct Recipe<'a> {
    /// A bass has `midi.in` first (the level test then plays it at C3, not E4).
    bass: bool,
    macros: &'a [(&'a str, f32)],
    /// `midi.in` mode, glide mode and time.
    midi: [f32; 3],
    oscs: Vec<Osc>,
    /// Drive and its trim, dB.
    drive: [f32; 2],
    /// Ladder cutoff Hz, resonance, drive dB.
    lad: [f32; 3],
    fenv: [f32; 4],
    /// How far the filter envelope sweeps the cutoff, and the mod wheel.
    fenv_amt: f32,
    wheel_amt: f32,
    aenv: [f32; 4],
    trim_db: f32,
}

struct Voice {
    r: Rack,
    knob: ModuleId,
    m: ModuleId,
    oscs: Vec<ModuleId>,
    drive: ModuleId,
    lad: ModuleId,
    fenv: ModuleId,
    aenv: ModuleId,
    mono: ModuleId,
    cur: Option<ModuleId>,
    n: usize,
}

impl Voice {
    fn new(c: Recipe) -> Self {
        let mut r = Rack::new();
        let (knob, m);
        let midi = |r: &mut Rack| {
            r.add(
                "midi.in",
                &[
                    ("mode", c.midi[0]),
                    ("glide", c.midi[1]),
                    ("glide_ms", c.midi[2]),
                    ("bend", 2.0),
                ],
            )
        };
        if c.bass {
            m = midi(&mut r);
            knob = macros(&mut r, c.macros);
        } else {
            knob = macros(&mut r, c.macros);
            m = midi(&mut r);
        }
        assert!(c.oscs.len() <= 4, "the mixer has four inputs");
        let oscs: Vec<ModuleId> = c
            .oscs
            .iter()
            .map(|o| {
                r.add(
                    "osc.va",
                    &[
                        ("base_hz", 261.63 * 2f32.powi(o.oct)),
                        ("waveform", o.wave),
                        ("pw", o.pw),
                        ("fine", o.fine),
                        ("unison", o.unison),
                        ("detune", o.detune),
                    ],
                )
            })
            .collect();
        r.next_row();
        let mix = r.add("mixer", &[]);
        for (i, o) in c.oscs.iter().enumerate() {
            r.e.set_param(mix, &format!("level{}", i + 1), o.level);
        }
        let drive = r.add(
            "drive",
            &[("drive_db", c.drive[0]), ("trim_db", c.drive[1])],
        );
        let lad = r.add(
            "filter.ladder",
            &[
                ("cutoff_hz", c.lad[0]),
                ("resonance", c.lad[1]),
                ("drive_db", c.lad[2]),
            ],
        );
        let fenv = r.add(
            "env.adsr",
            &adsr(c.fenv[0], c.fenv[1], c.fenv[2], c.fenv[3]),
        );
        let aenv = r.add(
            "env.adsr",
            &adsr(c.aenv[0], c.aenv[1], c.aenv[2], c.aenv[3]),
        );
        r.next_row();
        let vca = r.add("vca", &[("gain", 0.0)]);
        let trim = r.add("gain", &[("gain_db", c.trim_db)]);
        for (i, &o) in oscs.iter().enumerate() {
            r.wire(m, "pitch", o, "pitch");
            r.wire(o, "out", mix, &format!("in{}", i + 1));
        }
        r.wire(mix, "out", drive, "in");
        r.wire(drive, "out", lad, "in");
        r.wire(m, "gate", fenv, "gate");
        r.wire(m, "gate", aenv, "gate");
        r.wire(lad, "out", vca, "in");
        r.wire(aenv, "out", vca, "cv");
        r.wire(vca, "out", trim, "in");
        r.route(fenv, "out", lad, "cutoff_hz", c.fenv_amt);
        if c.wheel_amt > 0.0 {
            r.route(m, "wheel", lad, "cutoff_hz", c.wheel_amt);
        }
        Voice {
            r,
            knob,
            m,
            oscs,
            drive,
            lad,
            fenv,
            aenv,
            mono: trim,
            cur: None,
            n: c.macros.len(),
        }
    }

    /// A stereo stage (`in_l`/`in_r` to `left`/`right`) after the last one.
    fn stage(&mut self, kind: &str, params: &[(&str, f32)]) -> ModuleId {
        let s = self.r.add(kind, params);
        match self.cur {
            None => self.r.wire_both(self.mono, "out", s),
            Some(c) => self.r.stereo(c, s),
        }
        self.cur = Some(s);
        s
    }

    /// The echo and room on macro `mac`, then the limiter and the output.
    fn finish(
        mut self,
        mac: &str,
        echo: &[(&str, f32)],
        echo_max: f32,
        room: (f32, f32),
        clock: Option<ModuleId>,
    ) -> Rack {
        let src = self.cur.expect("a stereo stage");
        let tail = space_tail(
            &mut self.r,
            self.knob,
            src,
            echo,
            clock,
            echo_max,
            room.0,
            room.1,
            mac,
        );
        self.r.out_stereo(tail);
        perform(&mut self.r, self.knob, self.n, clock);
        self.r
    }

    /// A slow LFO on every oscillator's fine tune, its depth on macro `mac`.
    fn vibrato(&mut self, mac: &str, rate_hz: f32, depth: f32) {
        let lfo = self.r.add("lfo", &[("rate_hz", rate_hz)]);
        for o in self.oscs.clone() {
            self.r
                .depth_route(lfo, "out", o, "fine", self.knob, mac, depth);
        }
    }
}

fn compressor(
    threshold: f32,
    ratio: f32,
    attack: f32,
    release: f32,
    makeup: f32,
) -> [(&'static str, f32); 6] {
    [
        ("threshold_db", threshold),
        ("ratio", ratio),
        ("attack_ms", attack),
        ("release_ms", release),
        ("knee_db", 4.0),
        ("makeup_db", makeup),
    ]
}

fn echo(time_ms: f32, feedback: f32, tone_hz: f32) -> [(&'static str, f32); 4] {
    [
        ("time_ms", time_ms),
        ("feedback", feedback),
        ("mix", 0.0),
        ("tone_hz", tone_hz),
    ]
}

// ---- group 1 --------------------------------------------------------------------------------

/// A kick lane on the 16th grid: gates on `on` of the eight steps over two beats.
fn drum_lane(r: &mut Rack, on: &[usize]) -> ModuleId {
    let seq = r.add("seq", &[]);
    for k in 1..=8 {
        r.e.set_param(seq, &format!("g{k}"), f32::from(u8::from(on.contains(&k))));
        r.e.set_param(seq, &format!("v{k}"), 0.9);
    }
    seq
}

/// Steel Toe: a blunt compressed bass that a four-on-the-floor kick ducks through a sidechain.
fn steel_toe() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Bite", 0.45),
            ("Grit", 0.60),
            ("Pump", 0.60),
            ("Room", 0.30),
        ],
        midi: [1.0, 0.0, 120.0],
        oscs: vec![osc(2.0, 0, 0.55).unison(2.0, 8.0), osc(3.0, -1, 0.5)],
        drive: [18.0, -9.0],
        lad: [300.0, 0.28, 8.0],
        fenv: [1.0, 140.0, 0.12, 70.0],
        fenv_amt: 0.22,
        wheel_amt: 0.25,
        aenv: [1.0, 180.0, 0.8, 45.0],
        trim_db: 11.0,
    });
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.5);
    v.r.route(v.knob, "m2", v.drive, "drive_db", 0.35);
    v.stage("compressor", &compressor(-26.0, 6.0, 2.0, 90.0, 13.0));
    let clock = v.r.add("clock", &[("bpm", 124.0), ("swing", 25.0)]);
    let duck = v.stage(
        "compressor",
        &[
            ("threshold_db", -34.0),
            ("ratio", 1.0),
            ("attack_ms", 1.0),
            ("release_ms", 200.0),
            ("knee_db", 3.0),
        ],
    );
    v.r.route(v.knob, "m3", duck, "ratio", 0.95);
    // The kick on every beat; it feeds the duck and joins after the tail, so it is not ducked.
    let lane = drum_lane(&mut v.r, &[1, 5]);
    let kick = v.r.add(
        "drum.kick",
        &[("level_db", -3.0), ("decay_ms", 280.0), ("drive_db", 10.0)],
    );
    v.r.wire(clock, "gate", lane, "clock");
    v.r.wire(clock, "reset", lane, "reset");
    v.r.wire(lane, "gate", kick, "gate");
    v.r.wire(lane, "velocity", kick, "velocity");
    v.r.wire(kick, "out", duck, "sc");
    let tail = space_tail(
        &mut v.r,
        v.knob,
        duck,
        &[
            ("sync", 3.0),
            ("time_ms", 360.0),
            ("feedback", 30.0),
            ("mix", 0.0),
            ("tone_hz", 3500.0),
        ],
        Some(clock),
        0.22,
        0.45,
        0.16,
        "m4",
    );
    let (mix_l, mix_r) = (v.r.add("mixer", &[]), v.r.add("mixer", &[]));
    let beat = v.r.add("mixer", &[("level1", 0.45)]);
    v.r.wire(kick, "out", beat, "in1");
    for (mix, side) in [(mix_l, "left"), (mix_r, "right")] {
        v.r.wire(tail, side, mix, "in1");
        v.r.wire(beat, "out", mix, "in2");
    }
    let limiter =
        v.r.add("limiter", &[("ceiling_db", -1.5), ("release_ms", 150.0)]);
    let out = v.r.add("out", &[]);
    v.r.wire(mix_l, "out", limiter, "in_l");
    v.r.wire(mix_r, "out", limiter, "in_r");
    v.r.wire(limiter, "left", out, "left");
    v.r.wire(limiter, "right", out, "right");
    perform(&mut v.r, v.knob, v.n, Some(clock));
    v.r
}

/// Tar Pit: a trap 808, a sine with a pitch snap and a long body, sliding between notes.
fn tar_pit() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Tone", 0.50),
            ("Dirt", 0.35),
            ("Slide", 0.50),
            ("Room", 0.20),
        ],
        midi: [1.0, 1.0, 80.0],
        oscs: vec![osc(0.0, -1, 0.7), osc(1.0, -1, 0.15)],
        drive: [6.0, -3.0],
        lad: [400.0, 0.05, 0.0],
        fenv: [0.5, 40.0, 0.0, 25.0],
        fenv_amt: 0.12,
        wheel_amt: 0.0,
        aenv: [1.0, 2200.0, 0.75, 260.0],
        trim_db: 8.0,
    });
    // The thump: the fast envelope kicks the pitch up a little and lets it fall.
    v.r.route(v.fenv, "out", v.oscs[0], "fine", 0.5);
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.5);
    v.r.route(v.knob, "m2", v.drive, "drive_db", 0.4);
    v.r.route(v.knob, "m3", v.m, "glide_ms", 0.8);
    v.stage("compressor", &compressor(-24.0, 3.0, 8.0, 120.0, 3.0));
    v.finish("m4", &echo(90.0, 15.0, 3000.0), 0.12, (0.5, 0.14), None)
}

/// Acid Rain: a legato acid bass, a saw over a square sub, a resonant ladder with accents.
fn acid_rain() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Cutoff", 0.50),
            ("Squelch", 0.35),
            ("Space", 0.25),
            ("Drive", 0.40),
        ],
        midi: [2.0, 2.0, 55.0],
        oscs: vec![osc(2.0, 0, 0.6), osc(3.0, -1, 0.3)],
        drive: [10.0, -5.0],
        lad: [220.0, 0.40, 6.0],
        fenv: [1.0, 220.0, 0.0, 100.0],
        fenv_amt: 0.22,
        wheel_amt: 0.3,
        aenv: [1.0, 250.0, 0.7, 40.0],
        trim_db: 9.0,
    });
    v.r.route(v.m, "velocity", v.lad, "cutoff_hz", 0.06);
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.45);
    v.r.route(v.knob, "m2", v.lad, "resonance", 0.45);
    v.r.route(v.knob, "m4", v.drive, "drive_db", 0.5);
    v.stage("compressor", &compressor(-24.0, 3.0, 5.0, 100.0, 4.0));
    v.finish("m3", &echo(352.0, 45.0, 4000.0), 0.4, (0.9, 0.2), None)
}

/// Soft Shoe: a clean round pop bass, a triangle over a sine, a soft pluck in the filter.
fn soft_shoe() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Tone", 0.50),
            ("Pluck", 0.50),
            ("Grit", 0.20),
            ("Room", 0.25),
        ],
        midi: [1.0, 0.0, 120.0],
        oscs: vec![osc(1.0, 0, 0.5), osc(0.0, -1, 0.6)],
        drive: [3.0, -1.0],
        lad: [500.0, 0.10, 0.0],
        fenv: [2.0, 180.0, 0.3, 80.0],
        fenv_amt: 0.0,
        wheel_amt: 0.2,
        aenv: [3.0, 350.0, 0.7, 110.0],
        trim_db: 9.5,
    });
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.5);
    v.r.depth_route(v.fenv, "out", v.lad, "cutoff_hz", v.knob, "m2", 0.7);
    v.r.route(v.knob, "m3", v.drive, "drive_db", 0.5);
    v.stage("compressor", &compressor(-22.0, 3.0, 10.0, 120.0, 3.0));
    v.finish("m4", &echo(110.0, 20.0, 3500.0), 0.12, (0.6, 0.2), None)
}

/// Satin Ribbon: a smooth singing pop lead, a detuned saw stack over a triangle, chorus.
fn satin_ribbon() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Tone", 0.55),
            ("Edge", 0.20),
            ("Space", 0.50),
            ("Vibrato", 0.35),
        ],
        midi: [2.0, 2.0, 60.0],
        oscs: vec![
            osc(2.0, 0, 0.5).unison(3.0, 10.0),
            osc(1.0, 0, 0.35).fine(5.0),
        ],
        drive: [3.0, -1.0],
        lad: [1600.0, 0.15, 0.0],
        fenv: [5.0, 300.0, 0.5, 200.0],
        fenv_amt: 0.10,
        wheel_amt: 0.0,
        aenv: [8.0, 200.0, 0.8, 260.0],
        trim_db: 11.0,
    });
    v.r.route(v.m, "velocity", v.lad, "cutoff_hz", 0.10);
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.4);
    v.r.route(v.knob, "m2", v.drive, "drive_db", 0.45);
    v.vibrato("m4", 5.5, 0.1);
    v.stage(
        "chorus",
        &[("rate_hz", 0.5), ("depth", 40.0), ("mix", 30.0)],
    );
    v.stage("compressor", &compressor(-24.0, 2.0, 15.0, 150.0, 2.0));
    v.finish("m3", &echo(375.0, 38.0, 4500.0), 0.3, (2.2, 0.3), None)
}

/// Rust Belt: a hard, crushed, hard-retriggered lead, a pulse and a saw into heavy drive.
fn rust_belt() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Cutoff", 0.50),
            ("Grit", 0.60),
            ("Crush", 0.50),
            ("Room", 0.25),
        ],
        midi: [1.0, 0.0, 120.0],
        oscs: vec![osc(3.0, 0, 0.5).pw(35.0), osc(2.0, 0, 0.45).fine(7.0)],
        drive: [16.0, -8.0],
        lad: [900.0, 0.25, 6.0],
        fenv: [1.0, 120.0, 0.3, 60.0],
        fenv_amt: 0.15,
        wheel_amt: 0.0,
        aenv: [1.0, 150.0, 0.8, 50.0],
        trim_db: 8.0,
    });
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.5);
    v.r.route(v.knob, "m2", v.drive, "drive_db", 0.35);
    let crush = v.stage(
        "bitcrusher",
        &[("bits", 7.0), ("downsample", 3.0), ("mix", 0.0)],
    );
    v.r.route(v.knob, "m3", crush, "mix", 0.7);
    v.stage("compressor", &compressor(-26.0, 6.0, 2.0, 80.0, 7.0));
    v.finish("m4", &echo(110.0, 20.0, 3000.0), 0.25, (0.4, 0.15), None)
}

/// Hot Wire: a legato acid lead, a saw and a narrow pulse, a resonant ladder, a dotted echo.
fn hot_wire() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Cutoff", 0.50),
            ("Squelch", 0.35),
            ("Space", 0.40),
            ("Pulse", 0.30),
        ],
        midi: [2.0, 2.0, 45.0],
        oscs: vec![osc(2.0, 0, 0.6), osc(3.0, 0, 0.3).pw(40.0).fine(4.0)],
        drive: [8.0, -4.0],
        lad: [600.0, 0.38, 4.0],
        fenv: [1.0, 200.0, 0.25, 120.0],
        fenv_amt: 0.15,
        wheel_amt: 0.0,
        aenv: [2.0, 250.0, 0.75, 120.0],
        trim_db: 12.0,
    });
    v.r.route(v.m, "velocity", v.lad, "cutoff_hz", 0.05);
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.4);
    v.r.route(v.knob, "m2", v.lad, "resonance", 0.45);
    let pwm = v.r.add("lfo", &[("rate_hz", 0.7)]);
    v.r.depth_route(pwm, "out", v.oscs[1], "pw", v.knob, "m4", 0.35);
    v.stage("compressor", &compressor(-24.0, 3.0, 5.0, 100.0, 4.0));
    v.finish("m3", &echo(352.0, 45.0, 4500.0), 0.4, (1.0, 0.2), None)
}

/// Paper Crane: a soft ambient lead that swells in, a triangle stack and a sine an octave up.
fn paper_crane() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Tone", 0.50),
            ("Swell", 0.40),
            ("Space", 0.65),
            ("Vibrato", 0.30),
        ],
        midi: [2.0, 2.0, 140.0],
        oscs: vec![osc(1.0, 0, 0.45).unison(2.0, 6.0), osc(0.0, 1, 0.2)],
        drive: [2.0, -1.0],
        lad: [900.0, 0.10, 0.0],
        fenv: [200.0, 500.0, 0.4, 600.0],
        fenv_amt: 0.12,
        wheel_amt: 0.0,
        aenv: [180.0, 400.0, 0.8, 900.0],
        trim_db: 14.0,
    });
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.45);
    v.r.route(v.knob, "m2", v.aenv, "attack_ms", 0.45);
    v.vibrato("m4", 4.8, 0.08);
    v.stage(
        "chorus",
        &[("rate_hz", 0.3), ("depth", 35.0), ("mix", 40.0)],
    );
    v.stage("compressor", &compressor(-24.0, 2.0, 20.0, 200.0, 2.0));
    v.finish("m3", &echo(480.0, 50.0, 4000.0), 0.55, (6.0, 0.6), None)
}

// ---- phrases --------------------------------------------------------------------------------

/// The same pattern a bar later (16 steps), for phrases that repeat with a change.
fn bars(pattern: &[(u32, u8, f32)], per_bar: &[&[(usize, u8)]]) -> Vec<(u32, u8, f32)> {
    let mut out = Vec::new();
    for (b, swaps) in per_bar.iter().enumerate() {
        for (i, &(s, n, l)) in pattern.iter().enumerate() {
            let n = swaps.iter().find(|&&(k, _)| k == i).map_or(n, |&(_, m)| m);
            out.push((s + 16 * b as u32, n, l));
        }
    }
    out
}

/// Shifts a phrase later by `t` seconds.
fn later(t: f32, v: Vec<Hit>) -> Vec<Hit> {
    v.into_iter()
        .map(|Hit(at, n, vel)| Hit(at + t, n, vel))
        .collect()
}

fn steel_phrase() -> Vec<Hit> {
    // Off-beat stabs over the kick, which Run starts at 0.5 s.
    let bar = [
        (2, 40, 1.5),
        (5, 40, 1.0),
        (6, 43, 1.0),
        (10, 40, 1.5),
        (13, 38, 1.0),
        (14, 40, 1.5),
    ];
    later(0.5, steps(&bars(&bar, &[&[], &[(1, 43)], &[], &[(4, 36)]])))
}

fn tar_phrase() -> Vec<Hit> {
    steps(&[
        (0, 36, 6.0),
        (8, 36, 3.0),
        (12, 39, 3.0),
        (16, 34, 10.0),
        (28, 31, 12.0),
        (44, 36, 6.0),
    ])
}

fn acid_phrase() -> Vec<Hit> {
    // Sixteen steps of squelch; length 1.15 overlaps the next note, so it slides.
    let bar = [
        (0, 36, 2.0),
        (2, 36, 1.0),
        (3, 48, 1.15),
        (4, 36, 2.0),
        (6, 39, 1.15),
        (7, 36, 1.0),
        (8, 48, 2.0),
        (10, 36, 1.0),
        (11, 43, 1.15),
        (12, 36, 1.0),
        (13, 36, 1.0),
        (14, 39, 2.0),
    ];
    steps(&bars(&bar, &[&[], &[(8, 51)], &[], &[(8, 46)]]))
}

fn shoe_phrase() -> Vec<Hit> {
    // A pop groove: root, fifth and octave walk.
    let bar = [
        (0, 43, 2.0),
        (3, 43, 1.0),
        (4, 50, 1.0),
        (6, 48, 1.0),
        (8, 41, 2.0),
        (11, 41, 1.0),
        (12, 48, 1.0),
        (14, 46, 1.0),
    ];
    steps(&bars(
        &bar,
        &[
            &[],
            &[],
            &[(0, 38), (3, 38), (4, 45)],
            &[(0, 38), (3, 38), (4, 45)],
        ],
    ))
}

fn line(notes: &[(f32, u8, f32)]) -> Vec<Hit> {
    notes
        .iter()
        .flat_map(|&(t, n, len)| [on(t, n), off(t + len, n)])
        .collect()
}

fn ribbon_phrase() -> Vec<Hit> {
    line(&[
        (0.0, 69, 0.9),
        (0.8, 72, 0.5),
        (1.3, 76, 1.2),
        (2.4, 74, 0.5),
        (2.9, 72, 0.5),
        (3.4, 69, 1.4),
        (4.7, 67, 0.5),
        (5.2, 71, 0.5),
        (5.7, 74, 1.8),
    ])
}

fn rust_phrase() -> Vec<Hit> {
    steps(&bars(
        &[
            (0, 64, 1.0),
            (2, 64, 1.0),
            (3, 67, 1.0),
            (6, 71, 1.0),
            (8, 69, 1.0),
            (10, 67, 1.0),
            (12, 64, 1.0),
            (14, 62, 1.0),
        ],
        &[&[], &[(7, 59)], &[], &[(3, 64), (4, 59), (7, 55)]],
    ))
}

fn wire_phrase() -> Vec<Hit> {
    steps(&bars(
        &[
            (0, 60, 2.0),
            (2, 60, 1.0),
            (3, 72, 1.15),
            (4, 60, 2.0),
            (6, 63, 1.15),
            (7, 60, 1.0),
            (8, 72, 2.0),
            (10, 67, 1.15),
            (11, 70, 1.15),
            (12, 72, 3.0),
        ],
        &[&[], &[(4, 58)], &[], &[(9, 70)]],
    ))
}

fn crane_phrase() -> Vec<Hit> {
    line(&[
        (0.0, 72, 3.2),
        (3.0, 76, 3.4),
        (6.2, 74, 3.0),
        (8.8, 71, 5.0),
    ])
}

// ---- the bank entries -----------------------------------------------------------------------

fn presets() -> Vec<Preset> {
    vec![
        Preset {
            slug: "steel-toe",
            name: "Steel Toe",
            category: "Bass",
            tags: &["mono", "distorted", "compressed", "sidechain", "124bpm", "french touch", "signature"],
            description: "A blunt mono bass for French touch: two saws over a square sub, 18 dB of drive and a compressor squeezing it flat, then ducked hard by a four-on-the-floor kick through a sidechain. Press Run to start the kick, then play short off-beat notes; the mod wheel opens the filter. Bite opens the filter, Grit pushes the drive, Pump sets how hard the kick ducks the bass, Room adds a tempo-synced slapback and a tight room.",
            listen: "The bass snapping shut on every kick and swelling back between beats; Pump at 0 plays the bass flat, at 1 it pumps hard.",
            build: steel_toe,
            group: 1,
            phrase: steel_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "tar-pit",
            name: "Tar Pit",
            category: "Bass",
            tags: &["mono", "808", "sub", "glide", "trap", "hip-hop"],
            description: "A trap 808: a pure sine with a small pitch snap and a long, round body, sliding from note to note. Play long low notes and overlap them to slide. Tone opens the low-pass, Dirt drives it so it speaks on small speakers, Slide sets how long the slide takes, Room adds a short slapback and room.",
            listen: "The thump on each attack, the long sine body, and the slide between notes; Dirt up makes it grit on small speakers.",
            build: tar_pit,
            group: 1,
            phrase: tar_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "acid-rain",
            name: "Acid Rain",
            category: "Bass",
            tags: &["mono", "legato", "acid", "glide", "house", "techno"],
            description: "A legato acid bass: a saw over a square sub into a resonant ladder filter that snaps on every note, with a short slide between overlapped notes. Play a sixteenth-note line; velocity accents the filter. Cutoff opens the filter, Squelch raises the resonance, Space adds a dotted echo and a room, Drive pushes the drive stage.",
            listen: "The filter snapping on each note and the slides between overlapped ones; Squelch up for the acid bite.",
            build: acid_rain,
            group: 1,
            phrase: acid_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "soft-shoe",
            name: "Soft Shoe",
            category: "Bass",
            tags: &["mono", "clean", "round", "pop"],
            description: "A clean, round pop bass: a triangle over a sine an octave down, a soft pluck in the low-pass and a light compressor. It sits under anything. Tone opens the filter, Pluck deepens the filter snap on each note, Grit adds a little drive, Room adds a short room and slapback.",
            listen: "The round, clean low end; Pluck up adds a snap at the front of each note.",
            build: soft_shoe,
            group: 1,
            phrase: shoe_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "satin-ribbon",
            name: "Satin Ribbon",
            category: "Lead",
            tags: &["mono", "legato", "smooth", "chorus", "pop"],
            description: "A smooth, vocal pop lead: a detuned saw stack over a triangle, a ladder filter that opens with velocity, light chorus and a dotted echo. Play legato; notes glide. Tone brightens, Edge adds drive, Space adds the echo and a room, Vibrato fades in a gentle vibrato.",
            listen: "The even, vocal tone and the glide between overlapped notes; raise Vibrato for the long notes.",
            build: satin_ribbon,
            group: 1,
            phrase: ribbon_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "rust-belt",
            name: "Rust Belt",
            category: "Lead",
            tags: &["mono", "distorted", "bitcrushed", "compressed", "french touch", "signature"],
            description: "A hard, crushed lead for French touch: a narrow pulse and a saw into 16 dB of drive, a bitcrusher and a compressor, retriggered on every note. Play short syncopated lines. Cutoff opens the filter, Grit pushes the drive, Crush mixes in the bitcrusher, Room adds a slapback and a tight room.",
            listen: "The blunt attack and the crushed edge; Crush up for the digital grind, Crush down for a plain hard lead.",
            build: rust_belt,
            group: 1,
            phrase: rust_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "hot-wire",
            name: "Hot Wire",
            category: "Lead",
            tags: &["mono", "legato", "acid", "glide", "house", "techno"],
            description: "A legato acid lead: a saw and a narrow pulse into a resonant ladder filter with velocity accents and a dotted echo. Play a sixteenth-note line and overlap notes to slide. Cutoff opens the filter, Squelch raises the resonance, Space adds the echo and a room, Pulse sets a slow pulse-width sweep on the second oscillator.",
            listen: "The filter snap, the slides, and the dotted echo; Pulse up for a slowly moving thickness.",
            build: hot_wire,
            group: 1,
            phrase: wire_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "paper-crane",
            name: "Paper Crane",
            category: "Lead",
            tags: &["mono", "legato", "soft", "slow", "ambient", "cinematic"],
            description: "A soft ambient lead that swells in: a triangle stack with a sine an octave up, chorus, a long echo and a big room. Play long, slow notes. Tone opens the filter, Swell lengthens the fade-in of each note, Space is the echo and room, Vibrato adds a slow vibrato.",
            listen: "Each note blooming out of silence and the long tail carrying into the next.",
            build: paper_crane,
            group: 1,
            phrase: crane_phrase,
            secs: 14.0,
        },
    ]
}

/// Peak and loudness of a preset played the way the level test plays it.
fn level(p: &Preset, state: &kabl_core::PatchState) -> (f32, f32) {
    let note = if state.modules.get(&1).is_some_and(|m| m.kind == "midi.in") {
        48
    } else {
        64
    };
    let hits: Vec<Hit> = [1.0f32, 5.0, 13.0, 21.0]
        .iter()
        .flat_map(|&t| [Hit(t, note, Some(90)), off(t + 3.0, note)])
        .collect();
    let _ = p;
    let (l, r) = render(state, &hits, 24.0);
    let db = |v: f32| 20.0 * v.max(1e-9).log10();
    let peak = l.iter().chain(&r).fold(0.0f32, |m, s| m.max(s.abs()));
    let (mut sum, mut n) = (0.0f64, 0usize);
    for (cl, cr) in l.chunks(4800).zip(r.chunks(4800)) {
        let ms = cl
            .iter()
            .chain(cr)
            .map(|s| (*s as f64).powi(2))
            .sum::<f64>()
            / (cl.len() * 2) as f64;
        if ms > 1e-5 {
            sum += ms;
            n += 1;
        }
    }
    (
        db(peak),
        20.0 * (sum / n.max(1) as f64).sqrt().log10() as f32,
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let only = |p: &Preset| args.get(4).is_none_or(|g| g.parse().ok() == Some(p.group));
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
                std::fs::write(dir.join("sound.toml"), toml::to_string_pretty(&meta).unwrap())
                    .expect("sound.toml");
                println!("wrote {}", dir.display());
            }
        }
        Some("clips") => {
            // clips <patches dir> <out dir> [group]: one float WAV per preset
            let out = Path::new(&args[3]);
            std::fs::create_dir_all(out).unwrap();
            for p in presets().iter().filter(|p| only(p)) {
                let log = kabl_core::load(&Path::new(&args[2]).join("presets").join(p.slug))
                    .expect("load patch");
                let (l, r) = render(log.state(), &(p.phrase)(), p.secs);
                let (l2, _) = render(log.state(), &(p.phrase)(), p.secs);
                assert_eq!(l, l2, "{}: render is deterministic", p.slug);
                write_wav(&out.join(format!("{}.wav", p.slug)), &l, &r);
                let peak = l.iter().chain(&r).fold(0.0f32, |m, s| m.max(s.abs()));
                println!("{:14} peak {peak:.3} | {} | {}", p.slug, p.category, p.listen);
            }
        }
        Some("phrases") => {
            // For docs/presets/scripts/host.py: {slug: {secs, hits: [[t, note, velocity|0]], cues: []}}.
            let entries: Vec<String> = presets()
                .iter()
                .map(|p| {
                    let hits: Vec<String> = (p.phrase)()
                        .iter()
                        .map(|h| format!("[{:.3},{},{}]", h.0, h.1, h.2.unwrap_or(0)))
                        .collect();
                    format!("\"{}\":{{\"secs\":{},\"hits\":[{}],\"cues\":[]}}", p.slug, p.secs, hits.join(","))
                })
                .collect();
            std::fs::write(&args[2], format!("{{{}}}\n", entries.join(","))).unwrap();
        }
        Some("levels") => {
            // levels <patches dir>: peak and loudness (dBFS) as the level test measures them
            for p in presets() {
                let log = kabl_core::load(&Path::new(&args[2]).join("presets").join(p.slug)).unwrap();
                let (peak, loud) = level(&p, log.state());
                println!("LEVEL {:14} peak {peak:6.2}  loud {loud:6.2}", p.slug);
            }
        }
        Some("macros") => {
            // Peak and loudness (dBFS) of the clip phrase with every macro at 0 and at 1.
            let db = |v: f32| 20.0 * v.max(1e-9).log10();
            for p in presets() {
                let mut line = format!("{:14}", p.slug);
                for (label, value) in [("all 0", 0.0), ("all 1", 1.0)] {
                    let mut rack = (p.build)();
                    let knob = *rack.e.state().modules.iter().find(|(_, m)| m.kind == "macro").unwrap().0;
                    for k in 1..=8 {
                        if rack.e.state().modules[&knob].params.contains_key(&format!("pin.m{k}")) {
                            rack.e.set_param(knob, &format!("m{k}"), value);
                        }
                    }
                    let (l, r) = render(rack.e.state(), &(p.phrase)(), p.secs);
                    let peak = l.iter().chain(&r).fold(0.0f32, |m, s| m.max(s.abs()));
                    assert!(peak.is_finite(), "{}: finite", p.slug);
                    line += &format!("  {label}: peak {:6.1} |", db(peak));
                }
                // Each macro alone, 0 against 1: how far the render moves (dB under the render).
                let probe = |k: usize, v: f32| {
                    let mut rack = (p.build)();
                    let knob = *rack.e.state().modules.iter().find(|(_, m)| m.kind == "macro").unwrap().0;
                    rack.e.set_param(knob, &format!("m{k}"), v);
                    render(rack.e.state(), &(p.phrase)(), p.secs).0
                };
                let count = (1..=8)
                    .filter(|k| (p.build)().e.state().modules.values().any(|m| m.params.contains_key(&format!("pin.m{k}"))))
                    .count();
                for k in 1..=count {
                    let (a, b) = (probe(k, 0.0), probe(k, 1.0));
                    let rms = |f: &dyn Fn(usize) -> f32| (a.iter().enumerate().map(|(i, _)| f(i).powi(2)).sum::<f32>() / a.len() as f32).sqrt();
                    line += &format!("  m{k} {:6.1} dB", db(rms(&|i| a[i] - b[i]) / rms(&|i| a[i]).max(1e-9)));
                }
                println!("{line}");
            }
        }
        Some("library") => {
            // The entries for FACTORY in crates/ui/tests/library.rs.
            for p in presets() {
                println!("    (\n        \"presets/{}\",\n        {:?},\n        {:?},\n        &{:?},\n        {:?},\n    ),", p.slug, p.name, p.category, p.tags, p.description);
            }
        }
        _ => eprintln!("usage: presets_bass_lead write|clips|levels|macros|library <patches dir> [out dir] [group]"),
    }
}
