#![allow(dead_code)]
//! The factory grooves (docs/presets/BRIEF.md, docs/presets/GROOVE.md): builds the patches from the
//! existing modules, writes them under `patches/presets/`, renders a clip per preset by playing a
//! short chord progression through the real engine, and prints the level and the library entries.
//!
//! ```sh
//! cargo run -p kabl-ui --example presets_groove -- write patches
//! cargo run --release -p kabl-ui --example presets_groove -- clips patches out_dir [group]
//! cargo run --release -p kabl-ui --example presets_groove -- levels patches
//! ```
//!
//! Same recipe as `presets_bass_lead.rs`: every preset names its macros after what they do (CC 20
//! upward), echo and room mixes are 0 on the module and driven by the space macro, so the stored
//! value is the baked-in wet amount and that macro at 0 is dry. A macro that fades a modulator in
//! is a depth route (the macro scales the route's amount). Stereo is only what the engine has
//! today: a mono voice into stereo fx stages (`docs/stereo/DESIGN.md`, step 1).

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
    fn migrated(mut self) -> Self {
        self.e = self.e.migrated();
        self
    }

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
    /// The last stage: the output. No limiter; headroom comes from the patch gain.
    fn out_stereo(&mut self, from: ModuleId) {
        let out = self.add("out", &[]);
        self.wire(from, "left", out, "left");
        self.wire(from, "right", out, "right");
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
/// a stereo module (`left`/`right`); returns the room, whose outputs go on to the output.
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

impl Rack {
    /// Adds `db` to every gain stage's `gain_db`.
    fn trim(&mut self, db: f32) {
        let gains: Vec<_> = self
            .e
            .state()
            .modules
            .iter()
            .filter(|(_, m)| m.kind == "gain")
            .map(|(&id, m)| (id, m.params.get("gain_db").copied().unwrap_or(0.0)))
            .collect();
        for (id, g) in gains {
            self.e.set_param(id, "gain_db", g + db);
        }
    }
}

include!("preset_trims.inc");

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
            let (kind, param) = target.rsplit_once('.').expect("kind.param");
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

// ---- the voice recipe (as presets_bass_lead.rs) ----
struct Osc {
    wave: f32,
    oct: f32,
    fine: f32,
    unison: f32,
    detune: f32,
    pw: f32,
    level: f32,
}

fn osc(wave: f32, oct: f32, level: f32) -> Osc {
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
    mix: ModuleId,
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
                        ("base_hz", 261.63 * 2f32.powf(o.oct)),
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
            mix,
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

    /// A mono stage (`in` to `out`) after the amp: it becomes the module the stereo stages follow.
    fn mono_after(&mut self, kind: &str, params: &[(&str, f32)]) -> ModuleId {
        let m = self.r.add(kind, params);
        self.r.wire(self.mono, "out", m, "in");
        self.mono = m;
        m
    }

    /// The echo and room on macro `mac`, then the output.
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

// ---- the groove builder ---------------------------------------------------------------------

use kabl_modules::builtins::{cell_index, make_code, with_code};

/// A step pattern for one drum row: `.` off, `,` very soft, `o` soft, `x` medium, `X` loud, `?`
/// soft and played half the time, `!` loud and played three times in four. 16 characters.
fn grid_row(r: &mut Rack, grid: ModuleId, pattern: usize, row: usize, text: &str) {
    let mut chunks = [0.0f32; 4];
    assert_eq!(text.len(), 16, "a grid row has 16 steps: {text}");
    for (step, c) in text.chars().filter(|c| *c != ' ').enumerate() {
        let (vel, chance) = match c {
            '.' => continue,
            ',' => (0, 0),
            'o' => (1, 0),
            'x' => (2, 0),
            'X' => (3, 0),
            '?' => (1, 2),
            '!' => (3, 1),
            _ => panic!("pattern character {c}"),
        };
        chunks[step / 4] = with_code(chunks[step / 4], step % 4, make_code(vel, chance));
    }
    let params = kabl_modules::registry::info_for("drum.grid")
        .unwrap()
        .params;
    for (c, v) in chunks.into_iter().enumerate() {
        r.e.set_param(grid, params[cell_index(pattern, row, c)].name, v);
    }
}

/// The drum kit of a groove: one 16-step row per drum for pattern A (the groove) and B (a fill).
/// Rows: kick, snare, clap, closed hat, open hat, tom.
struct Kit {
    a: [&'static str; 6],
    b: [&'static str; 6],
    /// Kick level dB, kick decay ms, kick drive dB, snare/clap/hat/tom level dB.
    kick: [f32; 3],
    levels: [f32; 4],
    /// Mixer levels: kick, snare, clap, hats.
    mix: [f32; 4],
    snare_decay: f32,
    /// Steps per row (kick, snare, clap, closed hat, open hat, tom): 16 unless a row is polymetric.
    lens: [f32; 6],
}

/// One synth part that plays itself: a sequencer into an oscillator (and a sub) and an amp
/// envelope. `steps` are semitones from the oscillator's C (None is a rest), one per 16th.
struct Part {
    steps: [Option<f32>; 16],
    /// Waveform, unison voices, detune, pulse width, octave of the oscillator.
    osc: [f32; 5],
    /// Level of a square an octave below (0 for none).
    sub: f32,
    /// Gate length in percent of a step, and the amp envelope.
    gate: f32,
    env: [f32; 4],
    level: f32,
    /// Low-pass on the part in Hz (0 for none): it keeps the pulse's upper harmonics from ringing.
    lp: f32,
    /// A slew on the pitch (a glide between notes) and a formant filter on the part.
    glide: bool,
    formant: bool,
    /// Steps the sequencer plays before it wraps (a polymeter against the clock's bar).
    len: f32,
    /// Pitch of the second oscillator against the first, semitones (-12: the usual sub).
    sub_st: f32,
    /// Clock division in front of the sequencer (2: eighth-note steps, a bar in its 8 steps).
    div: f32,
}

/// Steps from a compact text: note numbers and `-` rests, space separated, 16 entries.
fn steps(text: &str) -> [Option<f32>; 16] {
    let mut out = [None; 16];
    for (i, t) in text.split_whitespace().enumerate() {
        out[i] = t.parse().ok();
    }
    out
}

/// What a macro does, besides the fourth (always the space macro).
#[derive(Clone, Copy)]
enum T {
    /// The ladder filter's cutoff, by this fraction of its travel.
    Cutoff(f32),
    /// Drive into the filter.
    Dirt(f32),
    /// How hard the kick ducks the music.
    Pump,
    /// The clock's swing.
    Swing(f32),
    /// The mix of the phaser or chorus on the music.
    Fx(f32),
    /// The bitcrusher mix.
    Crush(f32),
    /// A hiss layer in the music bus (the bitcrusher's output differs between host runs).
    Dust(f32),
    /// The level of the hats in the beat.
    Hats(f32),
    /// The level of the bass part in the bus.
    Bass(f32),
    /// The level of the chord part in the bus.
    Chords(f32),
    /// The glide time of the bass part (needs `Part::glide`).
    Glide(f32),
    /// The vowel of the formant filter on the chord part (needs `Part::formant`).
    Vowel(f32),
    /// The tempo of the clock, by this fraction of its travel.
    Tempo(f32),
    /// The pitch of both synth parts, fine tune by this fraction of its travel.
    Pitch(f32),
    /// The gate length of the chord part.
    Chop(f32),
    /// The mix of the chord part's own tempo echo (needs `Spec::dub`).
    Dub(f32),
}

struct Spec {
    macros: [(&'static str, f32); 4],
    targets: [T; 3],
    bpm: f32,
    swing: f32,
    kit: Kit,
    bass: Part,
    chords: Option<Part>,
    /// The voice MIDI plays: waveform, octave, level.
    play: [f32; 3],
    /// Bus drive dB and trim, filter cutoff Hz and resonance.
    drive: [f32; 2],
    filter: [f32; 2],
    /// Which character stage the `Fx` macro mixes: "phaser", "chorus" or none.
    fx: Option<&'static str>,
    /// Bitcrusher in the chain (mix 0 until the `Crush` macro).
    crush: bool,
    /// Compressor on the music: threshold, ratio, makeup.
    comp: [f32; 3],
    /// Echo time ms (0 for a tempo-synced eighth), feedback, tone; max echo and room mix, decay.
    echo: [f32; 3],
    space: [f32; 3],
    trim_db: f32,
    /// A tempo-synced dotted echo on the chord part alone, at mix 0 until the `Dub` macro.
    dub: bool,
    /// The played voice's ladder cutoff Hz, unison detune and amp envelope, when not the default
    /// (3500 Hz, 8 ct, 2/300/0.6/90): a darker, softer voice.
    voice: Option<(f32, f32, [f32; 4])>,
}

/// What `seq_part` built: where the audio comes out, and the stages a macro can reach.
struct Built {
    out: (ModuleId, &'static str),
    slew: Option<ModuleId>,
    formant: Option<ModuleId>,
    seq: ModuleId,
    osc: ModuleId,
}

fn seq_part(r: &mut Rack, clock: ModuleId, p: &Part) -> Built {
    let seq = r.add("seq", &[("length", p.len), ("gate_len", p.gate)]);
    if p.div > 1.0 {
        let d = r.add("clock.div", &[("div", p.div)]);
        r.wire(clock, "gate", d, "clock");
        r.wire(clock, "reset", d, "reset");
        r.wire(d, "gate", seq, "clock");
    } else {
        r.wire(clock, "gate", seq, "clock");
    }
    r.wire(clock, "reset", seq, "reset");
    for (k, s) in p.steps.iter().enumerate() {
        r.e.set_param(seq, &format!("p{}", k + 1), s.unwrap_or(0.0));
        r.e.set_param(
            seq,
            &format!("g{}", k + 1),
            f32::from(u8::from(s.is_some())),
        );
    }
    let main = r.add(
        "osc.va",
        &[
            ("base_hz", 261.63 * 2f32.powf(p.osc[4])),
            ("waveform", p.osc[0]),
            ("unison", p.osc[1]),
            ("detune", p.osc[2]),
            ("pw", p.osc[3]),
        ],
    );
    let sub = r.add(
        "osc.va",
        &[
            ("base_hz", 261.63 * 2f32.powf(p.osc[4] + p.sub_st / 12.0)),
            ("waveform", 3.0),
        ],
    );
    let env = r.add("env.adsr", &adsr(p.env[0], p.env[1], p.env[2], p.env[3]));
    let mix = r.add("mixer", &[("level1", 1.0), ("level2", p.sub)]);
    let vca = r.add("vca", &[("gain", 0.0)]);
    let lvl = r.add("gain", &[("gain_db", 20.0 * p.level.max(1e-3).log10())]);
    let slew = p
        .glide
        .then(|| r.add("slew", &[("rise_ms", 0.1), ("fall_ms", 0.1)]));
    match slew {
        Some(sl) => {
            r.wire(seq, "pitch", sl, "in");
            r.wire(sl, "out", main, "pitch");
            r.wire(sl, "out", sub, "pitch");
        }
        None => {
            r.wire(seq, "pitch", main, "pitch");
            r.wire(seq, "pitch", sub, "pitch");
        }
    }
    r.wire(seq, "gate", env, "gate");
    r.wire(main, "out", mix, "in1");
    r.wire(sub, "out", mix, "in2");
    r.wire(mix, "out", vca, "in");
    r.wire(env, "out", vca, "cv");
    r.wire(vca, "out", lvl, "in");
    let mut out = (lvl, "out");
    let mut formant = None;
    if p.formant {
        let f = r.add("filter.formant", &[("vowel", 0.0), ("q", 2.0)]);
        r.wire(out.0, out.1, f, "in");
        out = (f, "out");
        formant = Some(f);
    }
    if p.lp > 0.0 {
        let tone = r.add("filter.svf", &[("cutoff_hz", p.lp), ("resonance", 0.0)]);
        r.wire(out.0, out.1, tone, "in");
        out = (tone, "lp");
    }
    Built {
        out,
        slew,
        formant,
        seq,
        osc: main,
    }
}

fn groove(s: Spec) -> Rack {
    // The voice MIDI plays over the groove, with its own amp and a fixed open filter.
    let (cutoff, detune, aenv) = s.voice.unwrap_or((3500.0, 8.0, [2.0, 300.0, 0.6, 90.0]));
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &s.macros,
        midi: [1.0, 0.0, 120.0],
        oscs: vec![osc(s.play[0], s.play[1], 0.5).unison(2.0, detune)],
        drive: [0.0, 0.0],
        lad: [cutoff, 0.1, 0.0],
        fenv: [2.0, 200.0, 0.3, 100.0],
        fenv_amt: 0.0,
        wheel_amt: 0.0,
        aenv,
        trim_db: 20.0 * s.play[2].max(1e-3).log10(),
    });
    let clock = v.r.add("clock", &[("bpm", s.bpm), ("swing", s.swing)]);
    let bass = seq_part(&mut v.r, clock, &s.bass);
    let chords = s.chords.as_ref().map(|c| seq_part(&mut v.r, clock, c));
    v.r.next_row();
    let bus = v.r.add(
        "mixer",
        &[("level1", 1.0), ("level2", 1.0), ("level3", 1.0)],
    );
    v.r.wire(bass.out.0, bass.out.1, bus, "in1");
    let dub = match (&chords, s.dub) {
        (Some(c), true) => {
            let d = v.r.add(
                "delay",
                &[
                    ("sync", 3.0),
                    ("time_ms", 400.0),
                    ("feedback", 62.0),
                    ("mix", 0.0),
                    ("tone_hz", 1800.0),
                ],
            );
            v.r.wire(c.out.0, c.out.1, d, "in");
            v.r.wire(clock, "gate", d, "clock");
            v.r.wire(d, "left", bus, "in2");
            Some(d)
        }
        (Some(c), false) => {
            v.r.wire(c.out.0, c.out.1, bus, "in2");
            None
        }
        _ => None,
    };
    v.r.wire(v.mono, "out", bus, "in3");
    if s.targets.iter().any(|t| matches!(t, T::Dust(_))) {
        let hiss = v.r.add("noise", &[("color", 1.0)]);
        v.r.wire(hiss, "out", bus, "in4");
        v.r.e.set_param(bus, "level4", 0.0);
    }
    let drive = v.r.add(
        "drive",
        &[("drive_db", s.drive[0]), ("trim_db", s.drive[1])],
    );
    let lad = v.r.add(
        "filter.ladder",
        &[
            ("cutoff_hz", s.filter[0]),
            ("resonance", s.filter[1]),
            ("drive_db", 2.0),
        ],
    );
    v.r.wire(bus, "out", drive, "in");
    v.r.wire(drive, "out", lad, "in");
    let master = v.r.add("gain", &[("gain_db", s.trim_db)]);
    v.r.wire(lad, "out", master, "in");
    v.mono = master;
    let crush = s.crush.then(|| {
        v.mono_after(
            "bitcrusher",
            &[("bits", 4.0), ("downsample", 10.0), ("mix", 0.0)],
        )
    });
    let fx = s.fx.map(|kind| match kind {
        "phaser" => v.stage(
            "phaser",
            &[
                ("stages", 6.0),
                ("rate_hz", 0.4),
                ("depth", 80.0),
                ("feedback", 50.0),
                ("mix", 0.0),
            ],
        ),
        _ => v.stage("chorus", &[("rate_hz", 0.3), ("depth", 55.0), ("mix", 0.0)]),
    });
    v.stage(
        "compressor",
        &compressor(s.comp[0], s.comp[1], 4.0, 110.0, s.comp[2]),
    );
    let duck = v.stage(
        "compressor",
        &[
            ("threshold_db", -34.0),
            ("ratio", 1.0),
            ("attack_ms", 1.0),
            ("release_ms", 180.0),
            ("knee_db", 3.0),
        ],
    );
    // The kit.
    let k = &s.kit;
    let grid = v.r.add("drum.grid", &[]);
    v.r.wire(clock, "gate", grid, "clock");
    v.r.wire(clock, "reset", grid, "reset");
    for (row, (a, b)) in k.a.iter().zip(&k.b).enumerate() {
        v.r.e
            .set_param(grid, &format!("len{}", row + 1), k.lens[row]);
        grid_row(&mut v.r, grid, 0, row, a);
        grid_row(&mut v.r, grid, 1, row, b);
    }
    let kick = v.r.add(
        "drum.kick",
        &[
            ("level_db", k.kick[0]),
            ("decay_ms", k.kick[1]),
            ("drive_db", k.kick[2]),
        ],
    );
    let snare = v.r.add(
        "drum.snare",
        &[("level_db", k.levels[0]), ("decay_ms", k.snare_decay)],
    );
    let clap = v.r.add("drum.clap", &[("level_db", k.levels[1])]);
    let hat = v.r.add("drum.hat", &[("level_db", k.levels[2])]);
    let tom = v.r.add("drum.tom", &[("level_db", k.levels[3])]);
    v.r.wire(grid, "kick", kick, "gate");
    v.r.wire(grid, "snare", snare, "gate");
    v.r.wire(grid, "clap", clap, "gate");
    v.r.wire(grid, "chat", hat, "closed");
    v.r.wire(grid, "ohat", hat, "open");
    v.r.wire(grid, "tom", tom, "gate");
    v.r.wire(kick, "out", duck, "sc");
    let beat = v.r.add(
        "mixer",
        &[
            ("level1", k.mix[0]),
            ("level2", k.mix[1]),
            ("level3", k.mix[2]),
            ("level4", k.mix[3]),
        ],
    );
    v.r.wire(kick, "out", beat, "in1");
    v.r.wire(snare, "out", beat, "in2");
    v.r.wire(clap, "out", beat, "in3");
    v.r.wire(hat, "out", beat, "in4");
    let kit = v.r.add("mixer", &[("level1", 1.0), ("level2", 0.35)]);
    v.r.wire(beat, "out", kit, "in1");
    v.r.wire(tom, "out", kit, "in2");
    // The macros.
    for (i, t) in s.targets.iter().enumerate() {
        let m = format!("m{}", i + 1);
        let stored = s.macros[i].1;
        match *t {
            T::Cutoff(a) => v.r.route(v.knob, &m, lad, "cutoff_hz", a),
            T::Dirt(a) => v.r.route(v.knob, &m, drive, "drive_db", a),
            T::Pump => v.r.route(v.knob, &m, duck, "ratio", 0.95),
            T::Swing(a) => v.r.route(v.knob, &m, clock, "swing", a),
            T::Fx(a) => v.r.route(v.knob, &m, fx.expect("an fx stage"), "mix", a),
            T::Dust(a) => v.r.route(v.knob, &m, bus, "level4", a),
            T::Crush(a) => v.r.route(v.knob, &m, crush.expect("a crusher"), "mix", a),
            // The macro is the level: stored at 0.5 it gives the kit's own hat level.
            T::Hats(a) => {
                v.r.e.set_param(beat, "level4", 0.0);
                v.r.route(v.knob, &m, beat, "level4", (2.0 * k.mix[3]).min(1.0) * a)
            }
            // From 0.4 of the bass level to full.
            T::Bass(a) => {
                v.r.e.set_param(bus, "level1", 0.4);
                v.r.route(v.knob, &m, bus, "level1", 0.6 * a)
            }
            T::Chords(a) => {
                v.r.e.set_param(bus, "level2", 0.3);
                v.r.route(v.knob, &m, bus, "level2", 0.7 * a)
            }
            T::Glide(a) => {
                let sl = bass.slew.expect("a glide part");
                v.r.route(v.knob, &m, sl, "rise_ms", a);
                v.r.route(v.knob, &m, sl, "fall_ms", a)
            }
            T::Vowel(a) => {
                let f = chords
                    .as_ref()
                    .and_then(|c| c.formant)
                    .expect("a formant part");
                v.r.route(v.knob, &m, f, "vowel", a)
            }
            // The route adds `a * macro * travel` to the base, so the base is set that far below the
            // designed value and the stored macro lands on it (280 bpm, 200 ct and 99 % of travel).
            T::Tempo(a) => {
                v.r.e.set_param(clock, "bpm", s.bpm - a * stored * 280.0);
                v.r.route(v.knob, &m, clock, "bpm", a)
            }
            T::Pitch(a) => {
                for o in std::iter::once(bass.osc).chain(chords.as_ref().map(|c| c.osc)) {
                    v.r.e.set_param(o, "fine", -a * stored * 200.0);
                    v.r.route(v.knob, &m, o, "fine", a)
                }
            }
            T::Chop(a) => {
                let c = chords.as_ref().expect("a chord part");
                let gate = s.chords.as_ref().expect("a chord part").gate;
                v.r.e.set_param(c.seq, "gate_mode", 1.0);
                v.r.e.set_param(c.seq, "gate_len", gate - a * stored * 99.0);
                v.r.route(v.knob, &m, c.seq, "gate_len", a)
            }
            T::Dub(a) => v.r.route(v.knob, &m, dub.expect("a dub echo"), "mix", a),
        }
    }
    let echo = [
        ("sync", if s.echo[0] == 0.0 { 3.0 } else { 0.0 }),
        ("time_ms", s.echo[0].max(60.0)),
        ("feedback", s.echo[1]),
        ("mix", 0.0),
        ("tone_hz", s.echo[2]),
    ];
    finish_drums(
        v,
        duck,
        clock,
        kit,
        &echo,
        (s.space[0], s.space[1], s.space[2]),
    )
}

/// The tail of a groove: echo and room on macro 4, the kit joined after them, the output.
fn finish_drums(
    mut v: Voice,
    duck: ModuleId,
    clock: ModuleId,
    beat: ModuleId,
    echo: &[(&str, f32)],
    (echo_max, decay, room_max): (f32, f32, f32),
) -> Rack {
    let tail = space_tail(
        &mut v.r,
        v.knob,
        duck,
        echo,
        Some(clock),
        echo_max,
        decay,
        room_max,
        "m4",
    );
    let (mix_l, mix_r) = (v.r.add("mixer", &[]), v.r.add("mixer", &[]));
    for (mix, side) in [(mix_l, "left"), (mix_r, "right")] {
        v.r.wire(tail, side, mix, "in1");
        v.r.wire(beat, "out", mix, "in2");
    }
    let out = v.r.add("out", &[]);
    v.r.wire(mix_l, "out", out, "left");
    v.r.wire(mix_r, "out", out, "right");
    perform(&mut v.r, v.knob, v.n, Some(clock));
    v.r
}

fn kit(
    a: [&'static str; 6],
    b: [&'static str; 6],
    kick: [f32; 3],
    levels: [f32; 4],
    mix: [f32; 4],
) -> Kit {
    Kit {
        a,
        b,
        kick,
        levels,
        mix,
        snare_decay: 220.0,
        lens: [16.0; 6],
    }
}

fn part(text: &str, osc: [f32; 5], sub: f32, gate: f32, env: [f32; 4], level: f32) -> Part {
    Part {
        steps: steps(text),
        osc,
        sub,
        gate,
        env,
        level,
        lp: 0.0,
        glide: false,
        formant: false,
        len: 16.0,
        sub_st: -12.0,
        div: 1.0,
    }
}

impl Part {
    fn lp(mut self, hz: f32) -> Self {
        self.lp = hz;
        self
    }
    fn len(mut self, steps: f32) -> Self {
        self.len = steps;
        self
    }
    fn sub_at(mut self, semitones: f32) -> Self {
        self.sub_st = semitones;
        self
    }
    fn div(mut self, by: f32) -> Self {
        self.div = by;
        self
    }
    fn glide(mut self) -> Self {
        self.glide = true;
        self
    }
    fn formant(mut self) -> Self {
        self.formant = true;
        self
    }
}

// ---- group 1 --------------------------------------------------------------------------------

const FLOOR: &str = "X...X...X...X...";
const BACKBEAT: &str = "....X.......X...";
const NONE: &str = "................";

fn copper_mile() -> Rack {
    groove(Spec {
        macros: [
            ("Filter", 0.55),
            ("Phase", 0.50),
            ("Pump", 0.60),
            ("Room", 0.25),
        ],
        targets: [T::Cutoff(0.6), T::Fx(0.9), T::Pump],
        bpm: 123.0,
        swing: 28.0,
        kit: kit(
            [
                FLOOR,
                NONE,
                BACKBEAT,
                ".o.,.o.,.o.,.o.,",
                "..X...X...X...X.",
                NONE,
            ],
            [
                "X...X...X...X.X.",
                NONE,
                "....X.......XXXX",
                ".o.,.o.,.o.,.o.,",
                "..X...X...X.....",
                "..........X.X.XX",
            ],
            [-3.0, 240.0, 8.0],
            [-8.0, -9.0, -11.0, -6.0],
            [0.5, 0.3, 0.3, 0.18],
        ),
        bass: part(
            "4 - 16 - 4 - 16 - 7 - 19 - 2 - 14 -",
            [2.0, 1.0, 10.0, 50.0, -2.0],
            0.5,
            70.0,
            [2.0, 180.0, 0.7, 40.0],
            0.7,
        ),
        chords: Some(part(
            "- - 4 - - 7 - - 11 - - 7 - 14 - -",
            [2.0, 3.0, 14.0, 50.0, 0.0],
            0.0,
            35.0,
            [1.0, 140.0, 0.0, 50.0],
            0.5,
        )),
        play: [2.0, 0.0, 0.5],
        drive: [6.0, -3.0],
        filter: [1400.0, 0.25],
        fx: Some("phaser"),
        crush: false,
        comp: [-24.0, 4.0, 4.0],
        echo: [0.0, 25.0, 3000.0],
        space: [0.2, 0.4, 0.15],
        trim_db: 0.0,
        dub: false,
        voice: None,
    })
}

fn brick_dust() -> Rack {
    groove(Spec {
        macros: [
            ("Filter", 0.50),
            ("Dirt", 0.60),
            ("Pump", 0.65),
            ("Room", 0.20),
        ],
        targets: [T::Cutoff(0.6), T::Dirt(0.45), T::Pump],
        bpm: 126.0,
        swing: 25.0,
        kit: kit(
            [
                "X..X..X...X.X...",
                "....X.......X..!",
                NONE,
                "x.x.x.x.x.x.x.x.",
                NONE,
                NONE,
            ],
            [
                "X..X..X...X.X.XX",
                "....X.......XXXX",
                NONE,
                "x.x.x.x.x.x.x.x.",
                NONE,
                "..........X.X.XX",
            ],
            [-2.0, 200.0, 6.0],
            [-6.0, -9.0, -10.0, -6.0],
            [0.55, 0.35, 0.0, 0.2],
        ),
        bass: part(
            "4 - 4 4 - - 4 - 7 - 4 4 - 2 - 4",
            [2.0, 2.0, 8.0, 50.0, -2.0],
            0.55,
            80.0,
            [1.0, 160.0, 0.8, 40.0],
            0.8,
        ),
        chords: None,
        play: [2.0, -1.0, 0.5],
        drive: [20.0, -10.0],
        filter: [700.0, 0.30],
        fx: None,
        crush: false,
        comp: [-26.0, 6.0, 4.0],
        echo: [90.0, 15.0, 3000.0],
        space: [0.15, 0.25, 0.1],
        trim_db: -4.5,
        dub: false,
        voice: None,
    })
}

fn ore_cart() -> Rack {
    groove(Spec {
        macros: [
            ("Filter", 0.45),
            ("Rumble", 0.55),
            ("Pump", 0.55),
            ("Space", 0.30),
        ],
        targets: [T::Cutoff(0.55), T::Bass(0.6), T::Pump],
        bpm: 132.0,
        swing: 0.0,
        kit: kit(
            [
                FLOOR,
                NONE,
                BACKBEAT,
                "x.x.x.x.x.x.x.x.",
                "..X...X...X...X.",
                NONE,
            ],
            [
                "X...X...X...X.XX",
                NONE,
                BACKBEAT,
                "x.x.x.x.x.x.x.x.",
                "..X...X...X...X.",
                "............X.XX",
            ],
            [-2.0, 220.0, 8.0],
            [-8.0, -9.0, -10.0, -6.0],
            [0.5, 0.0, 0.25, 0.2],
        ),
        bass: part(
            "- 0 0 - - 0 0 - - 0 0 - - 0 0 -",
            [2.0, 1.0, 6.0, 50.0, -2.0],
            0.6,
            55.0,
            [1.0, 120.0, 0.6, 30.0],
            0.95,
        ),
        chords: Some(part(
            "- - - - 12 - - - - - - - 12 - 15 -",
            [3.0, 2.0, 10.0, 40.0, 0.0],
            0.0,
            30.0,
            [1.0, 100.0, 0.0, 40.0],
            0.4,
        )),
        play: [2.0, 0.0, 0.5],
        drive: [10.0, -5.0],
        filter: [500.0, 0.30],
        fx: None,
        crush: false,
        comp: [-24.0, 4.0, 4.0],
        echo: [0.0, 40.0, 2500.0],
        space: [0.3, 1.2, 0.2],
        trim_db: 0.0,
        dub: false,
        voice: None,
    })
}

fn stoop_sale() -> Rack {
    groove(Spec {
        macros: [
            ("Swing", 0.45),
            ("Dust", 0.30),
            ("Filter", 0.50),
            ("Space", 0.30),
        ],
        targets: [T::Swing(0.5), T::Dust(0.3), T::Cutoff(0.5)],
        bpm: 86.0,
        swing: 38.0,
        kit: kit(
            [
                "X.....x...X.X...",
                "....X.......X...",
                NONE,
                "x.o.x.o.x.o.x.o.",
                NONE,
                NONE,
            ],
            [
                "X.....x...X.X.X.",
                "....X.......X.XX",
                NONE,
                "x.o.x.o.x.o.xxxx",
                NONE,
                NONE,
            ],
            [-3.0, 330.0, 4.0],
            [-6.0, -9.0, -12.0, -6.0],
            [0.45, 0.34, 0.0, 0.2],
        ),
        bass: part(
            "0 - - - - - 0 - 7 - - - 5 - - 3",
            [0.0, 1.0, 8.0, 50.0, -2.0],
            0.0,
            90.0,
            [3.0, 300.0, 0.6, 80.0],
            0.8,
        ),
        chords: Some(part(
            "- - - - - - - - - - 3 - - - - -",
            [1.0, 1.0, 8.0, 50.0, 0.0],
            0.0,
            200.0,
            [3.0, 400.0, 0.5, 200.0],
            0.4,
        )),
        play: [1.0, 0.0, 0.5],
        drive: [3.0, -1.5],
        filter: [1800.0, 0.10],
        fx: None,
        crush: false,
        comp: [-22.0, 3.0, 3.0],
        echo: [0.0, 25.0, 2800.0],
        space: [0.3, 0.8, 0.2],
        trim_db: -4.0,
        dub: false,
        voice: None,
    })
}

fn fault_line() -> Rack {
    groove(Spec {
        macros: [
            ("Reese", 0.50),
            ("Filter", 0.50),
            ("Hats", 0.55),
            ("Space", 0.25),
        ],
        targets: [T::Dirt(0.5), T::Cutoff(0.55), T::Hats(1.0)],
        bpm: 174.0,
        swing: 0.0,
        kit: kit(
            [
                "X.........X.....",
                "....X.......X...",
                NONE,
                "x.x.x.x.x.x.x.x.",
                NONE,
                NONE,
            ],
            [
                "X.........X.X...",
                "....X.......X.?X",
                NONE,
                "x.x.x.xxx.x.x.x.",
                NONE,
                NONE,
            ],
            [-3.0, 160.0, 6.0],
            [-6.0, -9.0, -12.0, -6.0],
            [0.55, 0.45, 0.0, 0.2],
        ),
        bass: part(
            "0 - - - - - 0 - - - 3 - - - 0 -",
            [2.0, 2.0, 22.0, 50.0, -2.0],
            0.5,
            95.0,
            [3.0, 400.0, 0.9, 60.0],
            0.7,
        ),
        chords: None,
        play: [2.0, -1.0, 0.5],
        drive: [8.0, -4.0],
        filter: [900.0, 0.20],
        fx: None,
        crush: false,
        comp: [-24.0, 4.0, 4.0],
        echo: [0.0, 30.0, 3000.0],
        space: [0.25, 0.8, 0.2],
        trim_db: -2.0,
        dub: false,
        voice: None,
    })
}

fn corner_store() -> Rack {
    groove(Spec {
        macros: [
            ("Swing", 0.50),
            ("Filter", 0.55),
            ("Drive", 0.30),
            ("Room", 0.25),
        ],
        targets: [T::Swing(0.5), T::Cutoff(0.5), T::Dirt(0.4)],
        bpm: 108.0,
        swing: 40.0,
        kit: kit(
            [
                "X.....X..X.X....",
                "....X.......X..o",
                NONE,
                "x.x.x.x.x.x.x.x.",
                "..............X.",
                NONE,
            ],
            [
                "X.....X..X.X..XX",
                "....X.......X.XX",
                NONE,
                "x.x.x.x.x.x.x.x.",
                NONE,
                "..........X.X.XX",
            ],
            [-3.0, 200.0, 4.0],
            [-7.0, -9.0, -11.0, -6.0],
            [0.55, 0.4, 0.0, 0.25],
        ),
        bass: part(
            "0 - 12 0 - 3 - 5 7 - 5 - 3 - 0 -",
            [2.0, 1.0, 6.0, 50.0, -2.0],
            0.3,
            55.0,
            [1.0, 140.0, 0.4, 40.0],
            0.7,
        ),
        chords: Some(
            part(
                "- - 12 - - - 15 - - - 12 14 - - - -",
                [3.0, 1.0, 8.0, 30.0, -1.0],
                0.0,
                30.0,
                [1.0, 110.0, 0.0, 40.0],
                0.5,
            )
            .lp(1900.0),
        ),
        play: [3.0, -1.0, 0.5],
        drive: [4.0, -2.0],
        filter: [1600.0, 0.20],
        fx: None,
        crush: false,
        comp: [-24.0, 4.0, 4.0],
        echo: [0.0, 20.0, 3000.0],
        space: [0.15, 0.3, 0.12],
        trim_db: 0.0,
        dub: false,
        voice: None,
    })
}

fn glass_coast() -> Rack {
    groove(Spec {
        macros: [
            ("Filter", 0.55),
            ("Hats", 0.55),
            ("Chorus", 0.50),
            ("Space", 0.55),
        ],
        targets: [T::Cutoff(0.55), T::Hats(1.0), T::Fx(0.9)],
        bpm: 100.0,
        swing: 0.0,
        kit: Kit {
            snare_decay: 130.0,
            ..kit(
                [
                    "X.......X.X.....",
                    "....X.......X...",
                    "....X.......X...",
                    "x.x.x.x.x.x.x.x.",
                    NONE,
                    NONE,
                ],
                [
                    "X.......X.X...X.",
                    "....X.......X.XX",
                    "....X.......XXXX",
                    "x.x.x.x.x.x.x.x.",
                    NONE,
                    "..........X.XXXX",
                ],
                [-3.0, 260.0, 4.0],
                [-4.0, -8.0, -12.0, -5.0],
                [0.3, 0.22, 0.14, 0.5],
            )
        },
        bass: part(
            "0 0 12 0 0 12 0 0 -2 -2 10 -2 -2 10 -2 -2",
            [2.0, 2.0, 12.0, 50.0, -2.0],
            0.5,
            50.0,
            [1.0, 150.0, 0.6, 40.0],
            0.6,
        ),
        chords: Some(
            part(
                "7 12 16 19 16 12 7 12 5 9 12 17 12 9 5 9",
                [2.0, 4.0, 18.0, 50.0, -1.0],
                0.3,
                78.0,
                [3.0, 260.0, 0.35, 240.0],
                0.45,
            )
            .lp(2400.0),
        ),
        play: [2.0, -1.0, 0.5],
        drive: [2.0, -1.0],
        filter: [2500.0, 0.10],
        fx: Some("chorus"),
        crush: false,
        comp: [-22.0, 3.0, 3.0],
        echo: [0.0, 52.0, 2800.0],
        space: [0.55, 3.2, 0.42],
        trim_db: -1.0,
        dub: false,
        voice: None,
    })
}

fn still_water() -> Rack {
    groove(Spec {
        macros: [
            ("Bloom", 0.50),
            ("Pulse", 0.40),
            ("Weight", 0.50),
            ("Space", 0.60),
        ],
        targets: [T::Cutoff(0.5), T::Hats(1.0), T::Bass(0.7)],
        bpm: 68.0,
        swing: 0.0,
        kit: kit(
            [
                "X.......?.......",
                NONE,
                NONE,
                "x.?.x.?.x.?.x.?.",
                NONE,
                "........?...?...",
            ],
            [
                "X.......?.......",
                NONE,
                NONE,
                "..?...?...?...?.",
                NONE,
                "........?...?...",
            ],
            [-6.0, 500.0, 0.0],
            [-12.0, -14.0, -2.0, -8.0],
            [0.15, 0.0, 0.0, 0.4],
        ),
        bass: part(
            "0 - - - - - - - 5 - - - - - - -",
            [0.0, 1.0, 6.0, 50.0, -2.0],
            0.4,
            100.0,
            [200.0, 800.0, 0.7, 1500.0],
            1.0,
        ),
        chords: Some(part(
            "- - 7 - - - - - - - - - 10 - - -",
            [1.0, 1.0, 6.0, 50.0, 1.0],
            0.0,
            90.0,
            [20.0, 1200.0, 0.0, 1500.0],
            0.4,
        )),
        play: [1.0, 0.0, 0.5],
        drive: [0.0, 0.0],
        filter: [1800.0, 0.05],
        fx: None,
        crush: false,
        comp: [-22.0, 2.0, 2.0],
        echo: [0.0, 55.0, 2500.0],
        space: [0.5, 5.0, 0.45],
        trim_db: -11.0,
        dub: false,
        voice: None,
    })
}

// ---- group 2 --------------------------------------------------------------------------------

/// The variant under audition (`PP_VAR=a|b|c`); without it the committed sound, `default`.
fn variant(default: char) -> char {
    std::env::var("PP_VAR")
        .ok()
        .and_then(|v| v.chars().next())
        .unwrap_or(default)
}

fn slab_city() -> Rack {
    let v = variant('c');
    // 808 line (dark: root, fifth, minor third; long notes), hats (straight, sparse), pad, glide.
    let (line, hats, hats_b, pad, glide) = match v {
        'a' => (
            "0 - - - - - 7 -",
            "x.x.x.x.x.x.x.x.",
            "x.x.x.x.x.x.xxx.",
            (650.0, 12.0, [120.0, 1200.0, 0.5, 900.0], 0.35),
            0.0,
        ),
        'c' => (
            "0 - 0 - - - 3 -",
            "X.x.X.x.X.x.X.x.",
            "X.x.X.x.X.x.X.xx",
            (650.0, 14.0, [80.0, 900.0, 0.55, 800.0], 0.30),
            0.25,
        ),
        _ => (
            "0 - - - 0 - - 7",
            "X.x.X.x.X.x.X.x.",
            "X.x.X.x.X.x.X.xx",
            (500.0, 10.0, [150.0, 1400.0, 0.5, 1000.0], 0.25),
            0.12,
        ),
    };
    groove(Spec {
        macros: [
            ("Tone", 0.55),
            ("Glide", glide),
            ("Hats", 0.60),
            ("Room", 0.25),
        ],
        targets: [T::Cutoff(0.5), T::Glide(0.6), T::Hats(1.0)],
        bpm: 140.0,
        swing: 0.0,
        kit: kit(
            [
                "X.....x...X.....",
                NONE,
                "........X.......",
                hats,
                "..............o.",
                NONE,
            ],
            [
                "X.....x...X..X.X",
                "........X.......",
                "........X...XXXX",
                hats_b,
                NONE,
                "..........X.X.XX",
            ],
            [-4.0, 160.0, 4.0],
            [-8.0, -10.0, -11.0, 0.0],
            [0.32, 0.0, 0.4, 0.5],
        ),
        bass: part(
            line,
            [0.0, 1.0, 8.0, 50.0, -2.0],
            0.0,
            100.0,
            [2.0, 700.0, 0.35, 140.0],
            0.6,
        )
        .div(2.0)
        .glide(),
        chords: None,
        play: [2.0, 0.0, pad.3],
        drive: [6.0, -3.0],
        filter: [1000.0, 0.15],
        fx: None,
        crush: false,
        comp: [-24.0, 3.0, 3.0],
        echo: [0.0, 25.0, 2800.0],
        space: [0.15, 0.5, 0.15],
        trim_db: 0.0,
        dub: false,
        voice: Some((pad.0, pad.1, pad.2)),
    })
}

fn rope_walk() -> Rack {
    groove(Spec {
        macros: [
            ("Filter", 0.55),
            ("Phase", 0.55),
            ("Pump", 0.60),
            ("Room", 0.25),
        ],
        targets: [T::Cutoff(0.6), T::Fx(0.9), T::Pump],
        bpm: 118.0,
        swing: 30.0,
        kit: kit(
            [
                FLOOR,
                NONE,
                BACKBEAT,
                "x.o.x.o.x.o.x.o.",
                "..X...X...X...X.",
                NONE,
            ],
            [
                "X...X...X...X.X.",
                NONE,
                "....X.......XXXX",
                "x.o.x.o.x.o.x.o.",
                "..X...X...X.....",
                "..........X.X.XX",
            ],
            [-3.0, 230.0, 6.0],
            [-8.0, -9.0, -11.0, -6.0],
            [0.5, 0.3, 0.3, 0.18],
        ),
        bass: part(
            "0 - 12 12 0 - 12 - 3 - 15 15 3 - 15 -",
            [2.0, 1.0, 8.0, 50.0, -2.0],
            0.5,
            65.0,
            [2.0, 170.0, 0.7, 40.0],
            0.7,
        ),
        chords: Some(
            part(
                "- - 7 - - - 7 7 - - 10 - - - 12 -",
                [2.0, 3.0, 14.0, 50.0, -1.0],
                0.0,
                40.0,
                [1.0, 160.0, 0.0, 60.0],
                0.5,
            )
            .lp(2000.0),
        ),
        play: [2.0, 0.0, 0.5],
        drive: [5.0, -2.5],
        filter: [1500.0, 0.25],
        fx: Some("phaser"),
        crush: false,
        comp: [-24.0, 4.0, 5.5],
        echo: [0.0, 25.0, 3000.0],
        space: [0.2, 0.4, 0.15],
        trim_db: 1.0,
        dub: false,
        voice: None,
    })
}

fn lamp_oil() -> Rack {
    groove(Spec {
        macros: [
            ("Chord", 0.70),
            ("Dub", 0.50),
            ("Filter", 0.50),
            ("Space", 0.50),
        ],
        targets: [T::Chords(1.0), T::Dub(0.7), T::Cutoff(0.6)],
        bpm: 124.0,
        swing: 0.0,
        kit: Kit {
            snare_decay: 70.0,
            ..kit(
                [
                    "X...X...X...X...",
                    "..x.......x.....",
                    NONE,
                    "..o...o...o...o.",
                    NONE,
                    NONE,
                ],
                [
                    "X...X...X...X.X.",
                    "..x.......x...x.",
                    NONE,
                    "..o...o...o.o.o.",
                    NONE,
                    "..........X.....",
                ],
                [-4.0, 280.0, 3.0],
                [-10.0, -9.0, -13.0, -6.0],
                [0.45, 0.28, 0.0, 0.18],
            )
        },
        bass: part(
            "0 - - - - - - - - - - - 0 - 3 -",
            [0.0, 1.0, 6.0, 50.0, -2.0],
            0.0,
            95.0,
            [6.0, 500.0, 0.8, 200.0],
            0.9,
        ),
        chords: Some(
            part(
                "- - 0 - - - - - - - - - 3 - - -",
                [2.0, 3.0, 12.0, 50.0, -1.0],
                0.55,
                25.0,
                [3.0, 220.0, 0.0, 110.0],
                0.55,
            )
            .sub_at(7.0)
            .lp(1000.0),
        ),
        play: [1.0, 0.0, 0.5],
        drive: [2.0, -1.0],
        filter: [1400.0, 0.15],
        fx: None,
        crush: false,
        comp: [-24.0, 3.0, 3.0],
        echo: [0.0, 55.0, 2200.0],
        space: [0.45, 4.0, 0.32],
        trim_db: -2.0,
        dub: true,
        voice: None,
    })
}

fn pinball() -> Rack {
    // Hypnotic and serious: few pitches, narrow intervals, dark short notes.
    let (bass, chords, wave, lp) = match variant('a') {
        'a' => ("0 0 - 3 0", "7 - 7 - - 5 -", 1.0, (900.0, 800.0)),
        'c' => ("0 3 - 0 -", "5 - - 7 - 3 -", 0.0, (1000.0, 900.0)),
        _ => ("0 - 0 0 -", "7 - - 7 - - -", 1.0, (700.0, 650.0)),
    };
    groove(Spec {
        macros: [
            ("Pitch", 0.50),
            ("Tempo", 0.50),
            ("Bleep", 0.60),
            ("Space", 0.45),
        ],
        targets: [T::Pitch(1.0), T::Tempo(0.4), T::Chords(1.0)],
        bpm: 120.0,
        swing: 0.0,
        kit: Kit {
            lens: [16.0, 16.0, 13.0, 7.0, 16.0, 9.0],
            ..kit(
                [
                    NONE,
                    NONE,
                    "o...............",
                    "x..o............",
                    NONE,
                    "x....o..........",
                ],
                [
                    NONE,
                    NONE,
                    "o.....o.........",
                    "x.xo..x.........",
                    NONE,
                    "x..o..o.x.......",
                ],
                [-6.0, 200.0, 0.0],
                [-8.0, -9.0, -12.0, -5.0],
                [0.0, 0.0, 0.4, 0.3],
            )
        },
        bass: part(
            bass,
            [wave, 1.0, 8.0, 50.0, -1.0],
            0.0,
            45.0,
            [4.0, 80.0, 0.0, 40.0],
            0.8,
        )
        .len(5.0)
        .lp(lp.0),
        chords: Some(
            part(
                chords,
                [wave, 1.0, 8.0, 50.0, 0.0],
                0.0,
                45.0,
                [4.0, 100.0, 0.0, 50.0],
                0.7,
            )
            .len(7.0)
            .lp(lp.1),
        ),
        play: [1.0, 0.0, 0.5],
        drive: [2.0, -1.0],
        filter: [3000.0, 0.15],
        fx: None,
        crush: false,
        comp: [-22.0, 2.0, 2.0],
        echo: [0.0, 50.0, 2800.0],
        space: [0.5, 2.2, 0.35],
        trim_db: -1.5,
        dub: false,
        voice: Some((900.0, 8.0, [4.0, 200.0, 0.5, 80.0])),
    })
}

fn scrap_metal() -> Rack {
    groove(Spec {
        macros: [
            ("Vowel", 0.50),
            ("Crush", 0.30),
            ("Pump", 0.60),
            ("Room", 0.30),
        ],
        targets: [T::Vowel(1.0), T::Crush(0.8), T::Pump],
        bpm: 128.0,
        swing: 20.0,
        kit: kit(
            [
                "X.....X..X..X...",
                "....X.......X...",
                "....X.......X..!",
                "x.x.x.x.x.x.x.x.",
                "..X...X...X...X.",
                NONE,
            ],
            [
                "X.....X..X..X.XX",
                "....X.......XXXX",
                "....X.......X.X.",
                "x.x.x.x.xxx.xxxx",
                "..X...X...X.....",
                "..........X.X.XX",
            ],
            [-3.0, 190.0, 8.0],
            [-8.0, -9.0, -11.0, -6.0],
            [0.5, 0.3, 0.3, 0.2],
        ),
        bass: part(
            "0 - 0 - - 0 - - 5 - 5 - - 3 - -",
            [2.0, 2.0, 8.0, 50.0, -2.0],
            0.5,
            60.0,
            [1.0, 150.0, 0.7, 40.0],
            0.8,
        ),
        chords: Some(
            part(
                "- - 0 - - 0 - 0 - - 7 - - 3 - -",
                [2.0, 2.0, 12.0, 50.0, -1.0],
                0.0,
                50.0,
                [1.0, 130.0, 0.4, 40.0],
                0.7,
            )
            .formant()
            .lp(2400.0),
        ),
        play: [2.0, 0.0, 0.5],
        drive: [10.0, -6.0],
        filter: [1200.0, 0.30],
        fx: None,
        crush: true,
        comp: [-24.0, 6.0, 5.5],
        echo: [90.0, 20.0, 2800.0],
        space: [0.3, 0.5, 0.2],
        trim_db: -1.0,
        dub: false,
        voice: None,
    })
}

fn dune_buggy() -> Rack {
    groove(Spec {
        macros: [
            ("Swing", 0.50),
            ("Filter", 0.55),
            ("Chop", 0.50),
            ("Space", 0.30),
        ],
        targets: [T::Swing(0.5), T::Cutoff(0.55), T::Chop(0.8)],
        bpm: 130.0,
        swing: 33.0,
        kit: kit(
            [
                "X.......X.X.....",
                "....X.......X...",
                NONE,
                "x.x.x.x.x.x.x.x.",
                "......X.......X.",
                NONE,
            ],
            [
                "X.......X.X...XX",
                "....X.......X.XX",
                NONE,
                "x.x.x.xxx.x.x.x.",
                "......X.......X.",
                "..........X.X.XX",
            ],
            [-3.0, 200.0, 4.0],
            [-7.0, -9.0, -11.0, -6.0],
            [0.5, 0.4, 0.0, 0.2],
        ),
        bass: part(
            "0 - - - - - 0 - - - 5 - - - - 3",
            [0.0, 1.0, 8.0, 50.0, -2.0],
            0.0,
            90.0,
            [2.0, 300.0, 0.7, 60.0],
            0.9,
        ),
        chords: Some(
            part(
                "- - 0 - 3 - - 0 - - - 7 - - 10 -",
                [2.0, 2.0, 10.0, 50.0, -1.0],
                0.0,
                50.0,
                [1.0, 130.0, 0.4, 30.0],
                0.55,
            )
            .lp(2000.0),
        ),
        play: [2.0, 0.0, 0.5],
        drive: [3.0, -1.5],
        filter: [1700.0, 0.15],
        fx: None,
        crush: false,
        comp: [-22.0, 3.0, 3.0],
        echo: [0.0, 25.0, 2800.0],
        space: [0.3, 1.0, 0.2],
        trim_db: -1.0,
        dub: false,
        voice: None,
    })
}

/// A held riff on the played voice, repeated: `(beat, length in beats, note)`.
fn riff(bpm: f32, notes: &[(f32, f32, u8)], loops: u32) -> Vec<Hit> {
    let beat = 60.0 / bpm;
    let mut out = Vec::new();
    for l in 0..loops {
        for &(b, len, n) in notes {
            let t = 0.5 + (l as f32 * 8.0 + b) * beat;
            out.push(on(t, n));
            out.push(off(t + len * beat, n));
        }
    }
    out
}

macro_rules! phrase {
    ($name:ident, $bpm:expr, $notes:expr) => {
        fn $name() -> Vec<Hit> {
            riff($bpm, &$notes, 4)
        }
    };
}
phrase!(
    copper_phrase,
    123.0,
    [
        (1.0, 0.5, 64),
        (1.5, 0.5, 67),
        (3.0, 0.5, 71),
        (5.0, 1.0, 64),
        (7.0, 0.5, 62)
    ]
);
phrase!(
    brick_phrase,
    126.0,
    [
        (0.0, 0.75, 40),
        (1.5, 0.5, 40),
        (3.0, 1.0, 43),
        (5.0, 0.5, 40),
        (6.5, 1.0, 38)
    ]
);
phrase!(
    ore_phrase,
    132.0,
    [
        (1.5, 0.5, 64),
        (3.5, 0.5, 67),
        (5.5, 0.5, 64),
        (7.5, 0.5, 71)
    ]
);
phrase!(
    stoop_phrase,
    86.0,
    [
        (0.0, 3.0, 60),
        (0.0, 3.0, 63),
        (0.0, 3.0, 67),
        (4.0, 3.0, 58),
        (4.0, 3.0, 62),
        (4.0, 3.0, 65)
    ]
);
phrase!(
    fault_phrase,
    174.0,
    [
        (0.0, 1.5, 45),
        (2.0, 1.5, 48),
        (4.0, 1.5, 45),
        (6.0, 1.5, 43)
    ]
);
phrase!(
    corner_phrase,
    108.0,
    [
        (0.0, 0.5, 64),
        (1.0, 0.5, 67),
        (2.0, 0.5, 69),
        (3.5, 0.5, 67),
        (5.0, 1.0, 64)
    ]
);
phrase!(
    glass_phrase,
    100.0,
    [
        (0.0, 2.0, 71),
        (2.0, 2.0, 74),
        (4.0, 2.0, 76),
        (6.0, 2.0, 74)
    ]
);
phrase!(
    still_phrase,
    68.0,
    [
        (0.0, 4.0, 62),
        (0.0, 4.0, 69),
        (4.0, 4.0, 60),
        (4.0, 4.0, 67)
    ]
);

phrase!(
    slab_phrase,
    140.0,
    [
        (0.0, 3.5, 58),
        (0.0, 3.5, 63),
        (0.0, 3.5, 66),
        (4.0, 3.5, 56),
        (4.0, 3.5, 61),
        (4.0, 3.5, 65)
    ]
);
phrase!(
    rope_phrase,
    118.0,
    [
        (1.0, 0.5, 65),
        (1.5, 0.5, 68),
        (3.0, 0.5, 72),
        (5.0, 1.0, 65),
        (7.0, 0.5, 63)
    ]
);
phrase!(
    lamp_phrase,
    124.0,
    [
        (0.0, 3.0, 60),
        (0.0, 3.0, 63),
        (0.0, 3.0, 67),
        (4.0, 3.0, 58),
        (4.0, 3.0, 62),
        (4.0, 3.0, 65)
    ]
);
phrase!(
    pinball_phrase,
    120.0,
    [
        (0.0, 2.0, 60),
        (2.0, 2.0, 63),
        (4.0, 3.0, 60),
        (7.0, 1.0, 58)
    ]
);
phrase!(
    scrap_phrase,
    128.0,
    [
        (1.0, 0.5, 55),
        (2.0, 0.5, 58),
        (3.5, 0.5, 55),
        (5.0, 1.0, 62),
        (7.0, 0.5, 58)
    ]
);
phrase!(
    dune_phrase,
    130.0,
    [
        (0.0, 1.0, 60),
        (0.0, 1.0, 64),
        (0.0, 1.0, 67),
        (2.5, 0.5, 62),
        (2.5, 0.5, 65),
        (2.5, 0.5, 69),
        (4.0, 2.0, 59),
        (4.0, 2.0, 62),
        (4.0, 2.0, 67)
    ]
);

fn presets() -> Vec<Preset> {
    let p = |slug,
             name,
             tags,
             description,
             listen,
             build: fn() -> Rack,
             phrase: fn() -> Vec<Hit>,
             secs| Preset {
        slug,
        name,
        category: "Groove",
        tags,
        description,
        listen,
        build,
        group: 1,
        phrase,
        secs,
    };
    let q = |slug,
             name,
             tags,
             description,
             listen,
             build: fn() -> Rack,
             phrase: fn() -> Vec<Hit>,
             secs| Preset {
        group: 2,
        ..p(slug, name, tags, description, listen, build, phrase, secs)
    };
    vec![
        p("copper-mile", "Copper Mile", &["phased", "pumping", "french touch", "123bpm", "signature"],
            "A filtered disco loop for French touch: a soft octave bass, syncopated saw stabs and a hard kick and clap, phased and compressed, with the kick ducking the music. Press Run, then play stabs on top. Filter opens the loop like a DJ knob, Phase brings in the phaser, Pump sets how hard the kick ducks the music, Room adds a tempo-synced echo and a tight room.",
            "The whole loop pumping on the kick; Phase up for swirl on the stabs, Filter for the sweep.", copper_mile, copper_phrase, 16.0),
        p("brick-dust", "Brick Dust", &["mono", "distorted", "compressed", "french touch", "126bpm", "signature"],
            "A blunt, overdriven stomp: a hard-driven saw and square bass line under a short kick, snare and hats, squashed by a heavy compressor. Press Run, then play low notes over it. Filter opens the bass, Dirt adds drive, Pump sets how hard the kick ducks the music, Room adds a slapback and a tight room.",
            "Distorted bass stabs locked to the kick; Dirt up for more bite, Pump up for deeper ducking.", brick_dust, brick_phrase, 16.0),
        p("ore-cart", "Ore Cart", &["rumble", "techno", "132bpm"],
            "A rolling techno groove: a four-on-the-floor kick, off-beat hats, a rumbling bass between the kicks and a sparse stab. Press Run, then play stabs over it. Filter opens the music, Rumble raises the bass, Pump sets how hard the kick ducks it, Space adds the echo and the room.",
            "The bass rolling between the kicks; Rumble up for more low end, Pump for a deeper duck.", ore_cart, ore_phrase, 16.0),
        p("stoop-sale", "Stoop Sale", &["swung", "dusty", "hip hop", "86bpm"],
            "A boom-bap loop: a loose kick and snare, swung hats, a sine sub bass and a soft seventh chord. Press Run, then play chords over it. Swing pushes the off-beats late, Dust brings in a layer of vinyl hiss, Filter opens the music, Space adds the echo and the room.",
            "The late, loose off-beats; Swing up to drag them further, Dust up for a dusty hiss.", stoop_sale, stoop_phrase, 16.0),
        p("fault-line", "Fault Line", &["reese", "breaks", "drum and bass", "174bpm"],
            "A two-step drum and bass loop: a sharp kick and snare, ghost snares, fast hats and a detuned reese bass. Press Run, then play low notes over it. Reese adds drive to the bass, Filter opens it, Hats raises the hats, Space adds the echo and the room.",
            "The detuned bass growling under the break; Reese up for more teeth, Hats up for a busier top.", fault_line, fault_phrase, 16.0),
        p("corner-store", "Corner Store", &["swung", "funk", "108bpm"],
            "A funk groove: a soft plucked bass with octave jumps, a tight kick and snare, swung hats and clav-like stabs. Press Run, then play lines over it. Swing sets the shuffle of the 16ths, Filter opens the stabs and the bass, Drive adds bite, Room adds a short echo and a small room.",
            "The bass and kick locking on the swung 16ths; Swing up for more shuffle.", corner_store, corner_phrase, 16.0),
        p("glass-coast", "Glass Coast", &["arp", "chorus", "synthwave", "100bpm"],
            "A synthwave groove: a pulsing octave bass, a rolling arpeggio, a gated snare and clap and a long echo. Press Run, then play leads over it. Filter opens the music, Hats raises the hats, Chorus widens the whole mix, Space adds the long echo and the room.",
            "The arpeggio rolling over the pulsing bass; Chorus up for width, Space up for the long tail.", glass_coast, glass_phrase, 16.0),
        p("still-water", "Still Water", &["slow", "sparse", "ambient", "68bpm"],
            "A sparse ambient groove: a soft kick, a few random hats and toms, a slow bass and bell-like chords with long tails. Press Run, then play slow chords over it. Bloom opens the music, Pulse raises the hats and drops, Weight raises the bass, Space adds the long echo and room.",
            "Rare, soft hits that never repeat exactly; Pulse up for more of them, Space up for a long tail.", still_water, still_phrase, 24.0),
    ].into_iter().chain([
        q("slab-city", "Slab City", &["glide", "808", "trap", "140bpm"],
            "A half-time trap loop: a short kick and a clap on the third beat, rolling hats with some left to chance, and a sliding sine 808 under them. Press Run, then play slow minor chords over it; the played voice is a dark, soft saw pad. Tone opens the music, Glide sets how long the 808 slides between notes, Hats raises the hats, Room adds a short echo and a tight room.",
            "The 808 sliding between its notes; Glide up for longer slides, Hats up for busier rolls.", slab_city, slab_phrase, 16.0),
        q("rope-walk", "Rope Walk", &["phased", "pumping", "french touch", "118bpm", "signature"],
            "A disco loop for French touch: an octave-jumping bass walking up in steps, syncopated saw stabs through a phaser, a four-on-the-floor kick and a clap, all ducked by the kick. Press Run, then play stabs on top. Filter opens the loop like a DJ knob, Phase brings in the phaser, Pump sets how hard the kick ducks the music, Room adds a tempo-synced echo and a tight room.",
            "The octave bass walking under the stabs; Phase up for swirl, Pump up for a harder duck.", rope_walk, rope_phrase, 18.0),
        q("lamp-oil", "Lamp Oil", &["dub", "echo", "dub techno", "124bpm"],
            "A dub techno loop: a soft kick, a rim click, a deep sine sub and a muted chord stab thrown into a long dotted echo. Press Run, then play chords over it. Chord sets the level of the stab, Dub feeds more of it to its own tempo echo, Filter opens the music, Space adds the long echo and the room.",
            "The chord stab repeating into the dark; Dub up for a longer tail, Chord up for a louder stab.", lamp_oil, lamp_phrase, 16.0),
        q("pinball", "Pinball", &["polymeter", "bleeps", "modular", "120bpm"],
            "A hypnotic modular groove: two dark, low sequencer lines of five and seven steps drift against a sparse clap, hat and tom of thirteen, seven and nine steps, so the pattern only repeats after many bars. Press Run, then play notes over it. Pitch tunes the bleeps, Tempo speeds the whole machine up and down, Bleep raises the second bleep line, Space adds the echo and the room.",
            "The dark five- and seven-step lines drifting against sparse hits; Tempo up to speed them, Pitch to bend the bleeps.", pinball, pinball_phrase, 16.0),
        q("scrap-metal", "Scrap Metal", &["formant", "crushed", "electro", "french touch", "128bpm", "signature"],
            "A robot stomp: a hard-driven bass, a stab that talks through a vowel filter, a tight kick, snare and clap, crushed and compressed until it pumps. Press Run, then play lines on top. Vowel morphs the stab from a to u, Crush brings in the bitcrusher, Pump sets how hard the kick ducks the music, Room adds a slapback and a tight room.",
            "The stab changing vowel; Crush up for grit, Vowel to open and close the mouth.", scrap_metal, scrap_phrase, 16.0),
        q("dune-buggy", "Dune Buggy", &["swung", "chopped", "garage", "130bpm"],
            "A UK garage groove: a shuffled two-step kick and snare, ticking hats, a round sub and minor chords chopped into short stabs. Press Run, then play chords over it. Swing sets the shuffle of the 16ths, Filter opens the music, Chop sets how short the chord stabs are, Space adds the echo and the room.",
            "The shuffled hats and chopped chords; Swing up for more skip, Chop down for shorter stabs.", dune_buggy, dune_phrase, 16.0),
    ])
    .collect()
}

/// Peak and loudness (dBFS) of the clip phrase.
fn level(p: &Preset, state: &kabl_core::PatchState) -> (f32, f32) {
    let (l, r) = render(state, &(p.phrase)(), p.secs);
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
    let group: Option<u8> = args
        .get(3..)
        .and_then(|a| a.last())
        .and_then(|g| g.parse().ok());
    let only = |p: &Preset| group.is_none_or(|g| g == p.group);
    match args.get(1).map(String::as_str) {
        Some("write") => {
            for p in presets().into_iter().filter(|p| only(p)) {
                let mut rack = (p.build)().migrated();
                rack.trim(level_trim(p.slug));
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
                println!(
                    "{:14} peak {peak:.3} | {} | {}",
                    p.slug, p.category, p.listen
                );
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
                    format!(
                        "\"{}\":{{\"secs\":{},\"hits\":[{}],\"cues\":[]}}",
                        p.slug,
                        p.secs,
                        hits.join(",")
                    )
                })
                .collect();
            std::fs::write(&args[2], format!("{{{}}}\n", entries.join(","))).unwrap();
        }
        Some("levels") => {
            // levels <patches dir>: peak and loudness (dBFS) as the level test measures them
            for p in presets().into_iter().filter(|p| only(p)) {
                let log =
                    kabl_core::load(&Path::new(&args[2]).join("presets").join(p.slug)).unwrap();
                let (peak, loud) = level(&p, log.state());
                println!("LEVEL {:14} peak {peak:6.2}  loud {loud:6.2}", p.slug);
            }
        }
        Some("macros") => {
            // Peak and loudness (dBFS) of the clip phrase with every macro at 0 and at 1.
            let db = |v: f32| 20.0 * v.max(1e-9).log10();
            for p in presets().into_iter().filter(|p| only(p)) {
                let mut line = format!("{:14}", p.slug);
                for (label, value) in [("all 0", 0.0), ("all 1", 1.0)] {
                    let mut rack = (p.build)().migrated();
                    let knob = *rack
                        .e
                        .state()
                        .modules
                        .iter()
                        .find(|(_, m)| m.kind == "macro")
                        .unwrap()
                        .0;
                    for k in 1..=8 {
                        if rack.e.state().modules[&knob]
                            .params
                            .contains_key(&format!("pin.m{k}"))
                        {
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
                    let mut rack = (p.build)().migrated();
                    let knob = *rack
                        .e
                        .state()
                        .modules
                        .iter()
                        .find(|(_, m)| m.kind == "macro")
                        .unwrap()
                        .0;
                    rack.e.set_param(knob, &format!("m{k}"), v);
                    render(rack.e.state(), &(p.phrase)(), p.secs).0
                };
                let count = (1..=8)
                    .filter(|k| {
                        (p.build)()
                            .migrated()
                            .e
                            .state()
                            .modules
                            .values()
                            .any(|m| m.params.contains_key(&format!("pin.m{k}")))
                    })
                    .count();
                for k in 1..=count {
                    let (a, b) = (probe(k, 0.0), probe(k, 1.0));
                    let rms = |f: &dyn Fn(usize) -> f32| {
                        (a.iter().enumerate().map(|(i, _)| f(i).powi(2)).sum::<f32>()
                            / a.len() as f32)
                            .sqrt()
                    };
                    line += &format!(
                        "  m{k} {:6.1} dB",
                        db(rms(&|i| a[i] - b[i]) / rms(&|i| a[i]).max(1e-9))
                    );
                }
                println!("{line}");
            }
        }
        Some("library") => {
            // The entries for FACTORY in crates/ui/tests/library.rs.
            for p in presets().into_iter().filter(|p| only(p)) {
                println!("    (\n        \"presets/{}\",\n        {:?},\n        {:?},\n        &{:?},\n        {:?},\n    ),", p.slug, p.name, p.category, p.tags, p.description);
            }
        }
        _ => eprintln!(
            "usage: presets_groove write|clips|levels|macros|library <patches dir> [out dir] [group]"
        ),
    }
}
