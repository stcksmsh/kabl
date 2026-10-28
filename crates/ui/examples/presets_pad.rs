//! The factory pads (docs/presets/BRIEF.md, docs/presets/PAD.md): builds the patches from the
//! existing modules, writes them under `patches/presets/`, renders a clip per preset by playing a
//! short chord progression through the real engine, and prints the level and the library entries.
//!
//! ```sh
//! cargo run -p kabl-ui --example presets_pad -- write patches
//! cargo run --release -p kabl-ui --example presets_pad -- clips patches out_dir [group]
//! cargo run --release -p kabl-ui --example presets_pad -- levels patches
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

// ---- clip phrases ---------------------------------------------------------------------------

/// A chord held from `t` for `len` seconds.
fn chord(t: f32, notes: &[u8], len: f32) -> Vec<Hit> {
    notes
        .iter()
        .flat_map(|&n| [on(t, n), off(t + len, n)])
        .collect()
}

/// A chord progression: (start second, notes, length in seconds).
fn progression(parts: &[(f32, &[u8], f32)]) -> Vec<Hit> {
    parts
        .iter()
        .flat_map(|&(t, notes, len)| chord(t, notes, len))
        .collect()
}

/// Chord stabs on the 16th grid: (step, length in steps) per hit, the same chord each time.
fn stabs(notes: &[u8], grid: &[(u32, f32)]) -> Vec<Hit> {
    grid.iter()
        .flat_map(|&(s, len)| chord(s as f32 * S16, notes, len * S16 * 0.9))
        .collect()
}

const AM9: &[u8] = &[45, 57, 60, 64, 67];
const FMAJ7: &[u8] = &[41, 53, 57, 60, 64];
const CMAJ: &[u8] = &[48, 55, 60, 64, 67];
const GSUS: &[u8] = &[43, 55, 59, 62, 69];

fn slow_phrase() -> Vec<Hit> {
    progression(&[
        (0.5, AM9, 5.5),
        (6.5, FMAJ7, 5.5),
        (12.5, CMAJ, 5.5),
        (18.5, GSUS, 5.0),
    ])
}

fn pop_phrase() -> Vec<Hit> {
    progression(&[
        (0.3, CMAJ, 2.7),
        (3.2, &[43, 55, 59, 62, 67], 2.7),
        (6.1, &[45, 57, 60, 64, 69], 2.7),
        (9.0, FMAJ7, 3.0),
    ])
}

fn stab_phrase() -> Vec<Hit> {
    let grid = [(0, 3.0), (3, 1.0), (6, 2.0), (8, 3.0), (11, 1.0), (14, 1.0)];
    let mut v = stabs(&[57, 60, 64, 67], &grid);
    for (bar, notes) in [
        (1u32, &[53u8, 57, 60, 65][..]),
        (2, &[55, 59, 62, 67]),
        (3, &[57, 60, 64, 67]),
    ] {
        for h in stabs(notes, &grid) {
            v.push(Hit(h.0 + bar as f32 * 16.0 * S16, h.1, h.2));
        }
    }
    v
}

// ---- the pads -------------------------------------------------------------------------------

fn pad_recipe<'a>(
    macros: &'a [(&'a str, f32)],
    oscs: Vec<Osc>,
    lad: [f32; 3],
    fenv: [f32; 4],
    fenv_amt: f32,
    aenv: [f32; 4],
    trim_db: f32,
) -> Recipe<'a> {
    Recipe {
        bass: false,
        macros,
        midi: [0.0, 0.0, 120.0],
        oscs,
        drive: [0.0, 0.0],
        lad,
        fenv,
        fenv_amt,
        wheel_amt: 0.15,
        aenv,
        trim_db,
    }
}

/// Fog Bank: a very slow ambient cloud, saw stack and triangle, a long attack and a nine second room.
fn fog_bank() -> Rack {
    let mut v = Voice::new(pad_recipe(
        &[
            ("Bloom", 0.50),
            ("Drift", 0.40),
            ("Depth", 0.35),
            ("Space", 0.65),
        ],
        vec![
            osc(2.0, 0.0, 0.30).unison(4.0, 12.0),
            osc(1.0, 0.0, 0.30).fine(7.0),
            osc(0.0, -1.0, 0.0),
        ],
        [900.0, 0.10, 0.0],
        [3500.0, 3000.0, 0.7, 3500.0],
        0.25,
        [2600.0, 1500.0, 0.9, 5000.0],
        11.0,
    ));
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.5);
    v.r.route(v.knob, "m3", v.mix, "level3", 0.5);
    let drift = v.r.add("lfo", &[("rate_hz", 0.07)]);
    v.r.depth_route(drift, "out", v.lad, "cutoff_hz", v.knob, "m2", 0.35);
    v.vibrato("m2", 0.13, 0.06);
    v.stage(
        "chorus",
        &[("rate_hz", 0.15), ("depth", 55.0), ("mix", 22.0)],
    );
    v.finish("m4", &echo(700.0, 45.0, 3000.0), 0.3, (9.0, 0.55), None)
}

/// Deep Field: a cinematic swell, saws over a deep sub, a filter that opens over seconds.
fn deep_field() -> Rack {
    let mut v = Voice::new(pad_recipe(
        &[
            ("Swell", 0.50),
            ("Sub", 0.50),
            ("Shimmer", 0.35),
            ("Space", 0.60),
        ],
        vec![
            osc(2.0, 0.0, 0.28).unison(3.0, 10.0),
            osc(2.0, -1.0, 0.25).fine(5.0),
            osc(1.0, 1.0, 0.0),
            osc(0.0, -1.0, 0.0),
        ],
        [320.0, 0.10, 0.0],
        [4500.0, 2500.0, 0.5, 4000.0],
        0.0,
        [3200.0, 1500.0, 0.9, 5500.0],
        14.0,
    ));
    v.r.depth_route(v.fenv, "out", v.lad, "cutoff_hz", v.knob, "m1", 0.6);
    v.r.route(v.knob, "m2", v.mix, "level4", 0.6);
    v.r.route(v.knob, "m3", v.mix, "level3", 0.5);
    v.stage(
        "chorus",
        &[("rate_hz", 0.10), ("depth", 45.0), ("mix", 15.0)],
    );
    v.finish("m4", &echo(800.0, 50.0, 2500.0), 0.3, (6.5, 0.6), None)
}

/// Cotton Mill: an analog pop pad, a pulse with moving width and two saws, chorus.
fn cotton_mill() -> Rack {
    let mut v = Voice::new(pad_recipe(
        &[
            ("Cutoff", 0.50),
            ("Motion", 0.45),
            ("Chorus", 0.50),
            ("Space", 0.50),
        ],
        vec![
            osc(3.0, 0.0, 0.30).pw(35.0),
            osc(2.0, 0.0, 0.30).fine(6.0),
            osc(2.0, 0.0, 0.20).fine(-6.0),
        ],
        [1800.0, 0.12, 0.0],
        [200.0, 800.0, 0.5, 600.0],
        0.15,
        [90.0, 700.0, 0.8, 700.0],
        6.0,
    ));
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.5);
    let pwm = v.r.add("lfo", &[("rate_hz", 0.45)]);
    v.r.depth_route(pwm, "out", v.oscs[0], "pw", v.knob, "m2", 0.5);
    let ch = v.stage("chorus", &[("rate_hz", 0.4), ("depth", 50.0), ("mix", 0.0)]);
    v.r.route(v.knob, "m3", ch, "mix", 0.8);
    v.finish("m4", &echo(380.0, 30.0, 3500.0), 0.25, (3.0, 0.5), None)
}

/// Picture Window: a clean, bright seven-voice saw stack for pop.
fn picture_window() -> Rack {
    let mut v = Voice::new(pad_recipe(
        &[
            ("Cutoff", 0.55),
            ("Spread", 0.50),
            ("Sparkle", 0.40),
            ("Space", 0.50),
        ],
        vec![
            osc(2.0, 0.0, 0.28).unison(7.0, 8.0),
            osc(2.0, 1.0, 0.0).unison(3.0, 14.0),
        ],
        [3000.0, 0.10, 0.0],
        [250.0, 800.0, 0.85, 900.0],
        0.10,
        [220.0, 800.0, 0.85, 900.0],
        10.0,
    ));
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.5);
    v.r.route(v.knob, "m2", v.oscs[0], "detune", 0.5);
    v.r.route(v.knob, "m3", v.mix, "level2", 0.5);
    v.stage(
        "chorus",
        &[("rate_hz", 0.3), ("depth", 55.0), ("mix", 22.0)],
    );
    v.finish("m4", &echo(380.0, 30.0, 4000.0), 0.25, (4.0, 0.5), None)
}

/// Drive-In: a lush synthwave pad, detuned saws over a square, slow vibrato, wide chorus.
fn drive_in() -> Rack {
    let mut v = Voice::new(pad_recipe(
        &[
            ("Cutoff", 0.50),
            ("Wobble", 0.40),
            ("Chorus", 0.60),
            ("Space", 0.60),
        ],
        vec![
            osc(2.0, 0.0, 0.28).unison(5.0, 16.0),
            osc(2.0, 0.0, 0.25).fine(-9.0),
            osc(3.0, -1.0, 0.15),
        ],
        [1500.0, 0.15, 0.0],
        [400.0, 1200.0, 0.6, 900.0],
        0.20,
        [300.0, 500.0, 0.85, 1200.0],
        8.0,
    ));
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.5);
    v.vibrato("m2", 0.6, 0.12);
    let ch = v.stage("chorus", &[("rate_hz", 0.6), ("depth", 70.0), ("mix", 0.0)]);
    v.r.route(v.knob, "m3", ch, "mix", 0.9);
    v.finish("m4", &echo(460.0, 40.0, 3200.0), 0.35, (5.0, 0.55), None)
}

/// Dry Cleaner: phased chords through a compressor, close and dry, a filter sweep on macro 1.
fn dry_cleaner() -> Rack {
    let mut c = pad_recipe(
        &[
            ("Sweep", 0.45),
            ("Phase", 0.50),
            ("Grit", 0.35),
            ("Room", 0.25),
        ],
        vec![
            osc(2.0, 0.0, 0.30).unison(2.0, 8.0),
            osc(2.0, 0.0, 0.25).fine(10.0),
            osc(3.0, 0.0, 0.15).pw(30.0),
        ],
        [1200.0, 0.20, 2.0],
        [4.0, 500.0, 0.3, 300.0],
        0.12,
        [4.0, 350.0, 0.6, 200.0],
        8.0,
    );
    c.drive = [8.0, -4.0];
    let mut v = Voice::new(c);
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.6);
    v.r.route(v.knob, "m3", v.drive, "drive_db", 0.5);
    let ph = v.stage(
        "phaser",
        &[
            ("stages", 6.0),
            ("rate_hz", 0.35),
            ("depth", 85.0),
            ("feedback", 55.0),
            ("mix", 0.0),
        ],
    );
    v.r.route(v.knob, "m2", ph, "mix", 0.9);
    v.stage("compressor", &compressor(-22.0, 4.0, 6.0, 120.0, 5.0));
    v.finish("m4", &echo(363.0, 25.0, 3000.0), 0.2, (1.2, 0.35), None)
}

/// Concrete Cloud: a thick saturated pad, drive into a hard compressor, a bitcrusher on request.
fn concrete_cloud() -> Rack {
    let mut c = pad_recipe(
        &[
            ("Weight", 0.55),
            ("Cutoff", 0.45),
            ("Crush", 0.25),
            ("Room", 0.25),
        ],
        vec![
            osc(2.0, 0.0, 0.30).unison(4.0, 14.0),
            osc(2.0, -1.0, 0.30),
            osc(3.0, 0.0, 0.15),
        ],
        [900.0, 0.15, 3.0],
        [60.0, 800.0, 0.5, 500.0],
        0.10,
        [60.0, 500.0, 0.9, 500.0],
        8.0,
    );
    c.drive = [16.0, -8.0];
    let mut v = Voice::new(c);
    v.r.route(v.knob, "m1", v.drive, "drive_db", 0.5);
    v.r.route(v.knob, "m2", v.lad, "cutoff_hz", 0.45);
    let crush = v.stage(
        "bitcrusher",
        &[("bits", 7.0), ("downsample", 3.0), ("mix", 0.0)],
    );
    v.r.route(v.knob, "m3", crush, "mix", 0.8);
    v.stage("compressor", &compressor(-24.0, 6.0, 8.0, 200.0, 6.0));
    v.finish("m4", &echo(300.0, 20.0, 2800.0), 0.2, (1.5, 0.35), None)
}

/// Static Garden: a triangle bed with a noise layer, a ring modulated layer and a wandering filter.
fn static_garden() -> Rack {
    let mut v = Voice::new(pad_recipe(
        &[
            ("Texture", 0.40),
            ("Motion", 0.50),
            ("Metal", 0.30),
            ("Space", 0.55),
        ],
        vec![
            osc(1.0, 0.0, 0.35).unison(3.0, 9.0),
            osc(0.0, 0.0, 0.20).fine(4.0),
        ],
        [1100.0, 0.12, 0.0],
        [1200.0, 2000.0, 0.6, 2500.0],
        0.15,
        [900.0, 1500.0, 0.9, 3500.0],
        8.0,
    ));
    let noise = v.r.add("noise", &[("color", 1.0), ("level_db", -8.0)]);
    v.r.wire(noise, "out", v.mix, "in3");
    v.r.e.set_param(v.mix, "level3", 0.0);
    v.r.route(v.knob, "m1", v.mix, "level3", 0.8);
    let partner = v.r.add(
        "osc.va",
        &[("base_hz", 392.0), ("waveform", 0.0), ("fine", 3.0)],
    );
    v.r.wire(v.m, "pitch", partner, "pitch");
    let ring = v.r.add("ringmod", &[]);
    v.r.wire(v.oscs[0], "out", ring, "a");
    v.r.wire(partner, "out", ring, "b");
    v.r.wire(ring, "out", v.mix, "in4");
    v.r.e.set_param(v.mix, "level4", 0.0);
    v.r.route(v.knob, "m3", v.mix, "level4", 0.5);
    let walk1 = v.r.add("lfo", &[("rate_hz", 0.11)]);
    let walk2 = v.r.add("lfo", &[("rate_hz", 0.29)]);
    v.r.depth_route(walk1, "out", v.lad, "cutoff_hz", v.knob, "m2", 0.4);
    v.r.depth_route(walk2, "out", v.lad, "cutoff_hz", v.knob, "m2", 0.2);
    v.stage(
        "chorus",
        &[("rate_hz", 0.2), ("depth", 50.0), ("mix", 18.0)],
    );
    v.finish("m4", &echo(620.0, 45.0, 2800.0), 0.3, (7.0, 0.55), None)
}

fn presets() -> Vec<Preset> {
    vec![
        Preset {
            slug: "fog-bank",
            name: "Fog Bank",
            category: "Pad",
            tags: &["poly", "slow", "wide", "ambient", "cinematic"],
            description: "A very slow ambient cloud: a saw stack and a triangle with a long attack and a nine second room. Play long chords and let them overlap. Bloom opens the filter, Drift adds slow drift to the filter and the pitch, Depth brings in an octave of sub, Space adds the echo and the room.",
            listen: "Chords that take two seconds to arrive and several to die away; Drift up for a slow shimmer, Depth for weight underneath.",
            build: fog_bank,
            group: 1,
            phrase: slow_phrase,
            secs: 26.0,
        },
        Preset {
            slug: "deep-field",
            name: "Deep Field",
            category: "Pad",
            tags: &["poly", "swell", "sub", "cinematic", "ambient"],
            description: "A cinematic swell: saws over a deep sub with a filter that opens over several seconds. Play low, wide chords and hold them. Swell sets how far the filter opens, Sub adds an octave below, Shimmer adds an octave above, Space adds the echo and the room.",
            listen: "The slow opening of each chord and the weight under it; Swell at 0 stays closed and dark, at 1 opens wide.",
            build: deep_field,
            group: 1,
            phrase: slow_phrase,
            secs: 26.0,
        },
        Preset {
            slug: "cotton-mill",
            name: "Cotton Mill",
            category: "Pad",
            tags: &["poly", "analog", "pulse width", "chorus", "pop"],
            description: "An analog-style pad for pop: a pulse with moving width and two detuned saws, soft chorus and a mid-sized room. Play open chords. Cutoff opens the filter, Motion sets how far the pulse width moves, Chorus brings in the chorus, Space adds the echo and the room.",
            listen: "The pulse width breathing inside each held chord; Motion at 0 is still, at 1 swirls. Chorus at 0 is dry and narrow.",
            build: cotton_mill,
            group: 1,
            phrase: pop_phrase,
            secs: 14.0,
        },
        Preset {
            slug: "picture-window",
            name: "Picture Window",
            category: "Pad",
            tags: &["poly", "supersaw", "bright", "wide", "pop"],
            description: "A clean, bright pad for pop: a seven-voice saw stack with a light chorus and a medium room. Play simple triads. Cutoff opens the filter, Spread widens the detune of the stack, Sparkle adds an octave of airy saw above, Space adds the echo and the room.",
            listen: "The stack widening as Spread goes up and the high shimmer Sparkle adds; the chords stay clean and sit behind a vocal.",
            build: picture_window,
            group: 1,
            phrase: pop_phrase,
            secs: 14.0,
        },
        Preset {
            slug: "drive-in",
            name: "Drive-In",
            category: "Pad",
            tags: &["poly", "supersaw", "chorus", "vibrato", "synthwave", "80s"],
            description: "A lush synthwave pad: detuned saws over a square, a slow vibrato, wide chorus and a long echo. Play slow minor chords. Cutoff opens the filter, Wobble sets the depth of the vibrato, Chorus brings in the chorus, Space adds the echo and the room.",
            listen: "The wide chorused saws with a slow pitch sway; Chorus at 0 is a plain stack, at 1 a wide wash.",
            build: drive_in,
            group: 1,
            phrase: slow_phrase,
            secs: 26.0,
        },
        Preset {
            slug: "dry-cleaner",
            name: "Dry Cleaner",
            category: "Pad",
            tags: &["poly", "phased", "compressed", "stabs", "french touch", "signature"],
            description: "Phased chord stabs for French touch: two detuned saws and a pulse through a phaser, a compressor and a little drive, in a close, dry room. Play short, syncopated chords. Sweep opens the filter like a DJ, Phase brings in the phaser, Grit adds drive, Room adds the echo and the room.",
            listen: "The phaser sweeping through short chord stabs; Phase at 0 is a plain saw chord, at 1 fully swirling. Sweep moves the whole chord like a filter knob.",
            build: dry_cleaner,
            group: 1,
            phrase: stab_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "concrete-cloud",
            name: "Concrete Cloud",
            category: "Pad",
            tags: &["poly", "distorted", "compressed", "bitcrushed", "french touch", "signature"],
            description: "A thick, saturated pad for French touch: saws and an octave down driven hard into a compressor, with a bitcrusher on request. Play fat, held chords. Weight sets the amount of drive, Cutoff opens the filter, Crush brings in the bitcrusher, Room adds the echo and the room.",
            listen: "A heavy, squashed chord that stays loud and dense while it is held; Crush up for lo-fi grit, Weight up for more saturation.",
            build: concrete_cloud,
            group: 1,
            phrase: pop_phrase,
            secs: 14.0,
        },
        Preset {
            slug: "static-garden",
            name: "Static Garden",
            category: "Pad",
            tags: &["poly", "evolving", "noise", "ring mod", "ambient"],
            description: "An evolving, textured pad: a triangle bed with a noise layer, a ring modulated layer and a filter that wanders on its own. Play slow chords. Texture brings in the noise, Motion sets how far the filter wanders, Metal adds the ring modulated layer, Space adds the echo and the room.",
            listen: "Hiss and grain around the chord; Texture up for static, Metal up for bell-like inharmonic overtones, Motion for a filter that never repeats.",
            build: static_garden,
            group: 1,
            phrase: slow_phrase,
            secs: 26.0,
        },
    ]
}

/// Peak and loudness (dBFS) the way `factory_levels_hold_the_target` measures a poly keys patch:
/// the chord E3 B3 E4 G4 at velocity 90, down at 1, 5 and 13 s, up at 4, 9 and 17 s, 24 s render.
fn level(p: &Preset, state: &kabl_core::PatchState) -> (f32, f32) {
    let _ = p;
    let notes = [52u8, 59, 64, 67];
    let mut hits = Vec::new();
    for (down, up) in [(1.0f32, 4.0f32), (5.0, 9.0), (13.0, 17.0)] {
        for n in notes {
            hits.push(Hit(down, n, Some(90)));
            hits.push(off(up, n));
        }
    }
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
    let group: Option<u8> = args
        .get(3..)
        .and_then(|a| a.last())
        .and_then(|g| g.parse().ok());
    let only = |p: &Preset| group.is_none_or(|g| g == p.group);
    match args.get(1).map(String::as_str) {
        Some("write") => {
            for p in presets().into_iter().filter(|p| only(p)) {
                let mut rack = (p.build)();
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
                    let mut rack = (p.build)();
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
                    let mut rack = (p.build)();
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
            "usage: presets_pad write|clips|levels|macros|library <patches dir> [out dir] [group]"
        ),
    }
}
