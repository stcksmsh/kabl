//! The factory voices (docs/presets/BRIEF.md, docs/presets/VOICE.md): vowel, formant, talkbox and
//! robot voices, metal, glass and noise, risers and impacts. Builds the patches from the existing
//! modules, writes them under `patches/presets/`, renders a clip per preset through the real engine
//! and prints the levels and the library entries.
//!
//! ```sh
//! cargo run -p kabl-tools --example presets_voice -- write patches
//! cargo run --release -p kabl-tools --example presets_voice -- clips patches out_dir [group]
//! cargo run --release -p kabl-tools --example presets_voice -- levels patches
//! ```
//!
//! Same recipe as `presets_pad.rs` (the helpers are copied): macros are named for what they do (CC 20
//! upward), a space macro drives echo and room (0 is dry), no limiter, no drums: a rhythm comes from
//! the host clock (press Run).

#![allow(dead_code)]

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
        self.wire(a, ap, b, "in");
    }
    /// A `split` of the stereo output of `a`: its `left` and `right` are the two sides.
    fn split(&mut self, a: ModuleId) -> ModuleId {
        let s = self.add("split", &[]);
        self.wire(a, "out", s, "in");
        s
    }
    fn stereo(&mut self, a: ModuleId, b: ModuleId) {
        self.wire(a, "out", b, "in");
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
        self.wire(from, "out", out, "in");
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
    let split = r.split(src);
    r.wire(split, "left", delay, "in");
    if let Some(c) = clock {
        r.wire(c, "gate", delay, "clock");
    }
    r.stereo(src, verb);
    r.wire(delay, "out", verb, "in");
    if let Some(stored) = mac.strip_prefix('=') {
        // No space macro: the echo and room sit at what the macro left them at.
        let stored: f32 = stored.parse().expect("stored");
        r.e.set_param(delay, "mix", 100.0 * echo_max * stored);
        r.e.set_param(verb, "mix", 100.0 * room_max * stored);
    } else {
        r.route(knob, mac, delay, "mix", echo_max);
        r.route(knob, mac, verb, "mix", room_max);
    }
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
    /// Stereo spread of the unison stack, percent; above 0 the stack leaves on `stereo`.
    width: f32,
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
        width: 0.0,
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
    fn wide(mut self, width: f32) -> Self {
        self.width = width;
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
    /// A stack leaves the oscillator stereo, so the mono part runs as a pair.
    wide: bool,
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
                        ("width", o.width),
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
            let port = if c.oscs[i].width > 0.0 {
                "stereo"
            } else {
                "out"
            };
            r.wire(o, port, mix, &format!("in{}", i + 1));
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
            wide: c.oscs.iter().any(|o| o.width > 0.0),
        }
    }

    /// A mono stage after the voice (before any stereo stage).
    fn mono_after(&mut self, kind: &str, params: &[(&str, f32)]) -> ModuleId {
        let m = self.r.add(kind, params);
        self.r.wire(self.mono, "out", m, "in");
        self.mono = m;
        m
    }

    /// A stereo stage (`in_l`/`in_r` to `left`/`right`) after the last one.
    fn stage(&mut self, kind: &str, params: &[(&str, f32)]) -> ModuleId {
        let s = self.r.add(kind, params);
        match self.cur {
            None if self.wide => self.r.wire(self.mono, "out", s, "in"),
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

// ---- macro helpers ----------------------------------------------------------------------------

/// A range check; with `PP_LAX` set it only prints, so one run lists every spot to fix.
fn check(ok: bool, msg: String) {
    if !ok {
        if std::env::var("PP_LAX").is_ok() {
            eprintln!("RANGE {msg}");
        } else {
            panic!("{msg}");
        }
    }
}

fn param_of(kind: &str, param: &str) -> kabl_modules::ParamInfo {
    *kabl_modules::registry::info_for(kind)
        .and_then(|i| i.params.iter().find(|p| p.name == param))
        .unwrap_or_else(|| panic!("{kind}.{param}"))
}

/// Macro `m` (1-based) takes `param` from `lo` at 0 to `d` at its stored position, and on in the
/// same line to 1. The stored sound is exactly `d`; panics when the line leaves the param's range.
#[allow(clippy::too_many_arguments)]
fn span(
    v: &mut Voice,
    m: usize,
    stored: f32,
    id: ModuleId,
    kind: &str,
    param: &str,
    lo: f32,
    d: f32,
) {
    let knob = v.knob;
    span_rack(&mut v.r, knob, m, stored, id, kind, param, lo, d);
}

/// `span` on a finished rack (for a module `finish` built, such as the echo).
#[allow(clippy::too_many_arguments)]
fn span_rack(
    r: &mut Rack,
    knob: ModuleId,
    m: usize,
    stored: f32,
    id: ModuleId,
    kind: &str,
    param: &str,
    lo: f32,
    d: f32,
) {
    let i = param_of(kind, param);
    let (n0, nd) = (i.to_norm(lo), i.to_norm(d));
    let amount = (nd - n0) / stored;
    check(
        n0 + amount <= 1.0 + 1e-3 && n0 + amount >= -1e-3,
        format!(
            "{kind}.{param}: macro {m} runs out of range ({})",
            n0 + amount
        ),
    );
    r.e.set_param(id, param, lo);
    r.route(knob, &format!("m{m}"), id, param, amount);
}

/// Macro `m` at stored position 0 leaves `param` at `d`; at 1 it is `hi`.
fn rise(v: &mut Voice, m: usize, id: ModuleId, kind: &str, param: &str, d: f32, hi: f32) {
    let i = param_of(kind, param);
    v.r.e.set_param(id, param, d);
    v.r.route(
        v.knob,
        &format!("m{m}"),
        id,
        param,
        i.to_norm(hi) - i.to_norm(d),
    );
}

/// As `span`, with the value `hi` at macro 1 given and the value at 0 derived.
#[allow(clippy::too_many_arguments)]
fn span_hi(
    v: &mut Voice,
    m: usize,
    stored: f32,
    id: ModuleId,
    kind: &str,
    param: &str,
    d: f32,
    hi: f32,
) {
    let i = param_of(kind, param);
    let (nd, n1) = (i.to_norm(d), i.to_norm(hi));
    let n0 = (nd - stored * n1) / (1.0 - stored);
    check(
        (-1e-3..=1.0 + 1e-3).contains(&n0),
        format!("{kind}.{param}: macro {m} starts out of range ({n0})"),
    );
    span(
        v,
        m,
        stored,
        id,
        kind,
        param,
        i.from_norm(n0.clamp(0.0, 1.0)),
        d,
    );
}

/// `span` with the three values given: `lo` at macro 0, `d` at the stored position, `hi` at 1.
/// Stores the macro at the position that fits and returns it.
#[allow(clippy::too_many_arguments)]
fn span_lh(
    v: &mut Voice,
    m: usize,
    id: ModuleId,
    kind: &str,
    param: &str,
    lo: f32,
    d: f32,
    hi: f32,
) -> f32 {
    let i = param_of(kind, param);
    let (n0, nd, n1) = (i.to_norm(lo), i.to_norm(d), i.to_norm(hi));
    let s = (nd - n0) / (n1 - n0);
    check(
        s > 0.02 && s < 0.98,
        format!("{kind}.{param}: stored position {s}"),
    );
    span(v, m, s, id, kind, param, lo, d);
    let knob = v.knob;
    v.r.e.set_param(knob, &format!("m{m}"), s);
    s
}

/// What an old route of `amount` from a macro at `stored` leaves on a param with base `base`.
fn routed(kind: &str, param: &str, base: f32, amount: f32, stored: f32) -> f32 {
    let i = param_of(kind, param);
    i.from_norm(i.to_norm(base) + stored * amount)
}

/// Macro `m` (stored at `stored`) takes the voice's output gain down by `slope_db` over its whole
/// travel (so `slope_db` x (1 - stored) from the stored position to 1), keeping the stored sound:
/// a macro that adds level pulls the voice back so the peaks hold. Stacks over several macros.
fn comp(v: &mut Voice, m: usize, stored: f32, slope_db: f32) {
    let id = v.mono;
    let base = v.r.e.state().modules[&id].params["gain_db"];
    v.r.e.set_param(id, "gain_db", base + slope_db * stored);
    v.r.route(v.knob, &format!("m{m}"), id, "gain_db", -slope_db / 84.0);
}

/// A fixed LFO on every oscillator's fine tune: a vibrato that no macro scales.
fn fixed_vibrato(v: &mut Voice, rate_hz: f32, amount: f32) {
    let lfo = v.r.add("lfo", &[("rate_hz", rate_hz)]);
    for o in v.oscs.clone() {
        v.r.route(lfo, "out", o, "fine", amount);
    }
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

// ---- level trims and the dry finish ----------------------------------------------------------

/// Per-preset level trims in dB, added to every `gain` stage when a preset is written. The voices
/// set their level in the recipe (`trim_db`), and the compensation stages (`comp_gain`) are gain
/// stages too, so a trim here would move them all: it stays 0. Patch gain only, no limiter.
fn level_trim(_slug: &str) -> f32 {
    0.0
}

impl Voice {
    /// No echo and no room: the voice goes straight to the output (the preset has no space macro).
    fn finish_dry(mut self, clock: Option<ModuleId>) -> Rack {
        let out = self.r.add("out", &[]);
        match self.cur {
            Some(c) => self.r.wire(c, "out", out, "in"),
            None => {
                let m = self.mono;
                self.r.wire(m, "out", out, "in")
            }
        }
        perform(&mut self.r, self.knob, self.n, clock);
        self.r
    }
}

/// A dedicated trim stage after the voice (its own `gain`, so two macros never share one param):
/// macro `m` takes the level down by `slope_db` over its travel, stored sound unchanged. A negative
/// slope raises the level as the macro goes up. Call before the first stereo stage.
fn comp_gain(v: &mut Voice, m: usize, stored: f32, slope_db: f32) {
    let g = v.mono_after("gain", &[("gain_db", slope_db * stored)]);
    v.r.route(v.knob, &format!("m{m}"), g, "gain_db", -slope_db / 84.0);
}

/// A line: (start second, note, length in seconds).
fn line(parts: &[(f32, u8, f32)]) -> Vec<Hit> {
    parts
        .iter()
        .flat_map(|&(t, n, len)| [on(t, n), off(t + len, n)])
        .collect()
}

/// A riff on the 16th grid: (step, note, length in steps), repeated `times` every `bar_steps`.
fn riff(steps: &[(u32, u8, f32)], bar_steps: u32, times: u32, start: f32) -> Vec<Hit> {
    let mut v = Vec::new();
    for k in 0..times {
        for &(s, n, len) in steps {
            let t = start + (k * bar_steps + s) as f32 * S16;
            v.push(on(t, n));
            v.push(off(t + len * S16 * 0.95, n));
        }
    }
    v
}

// ---- the voices -----------------------------------------------------------------------------

/// A voice-shaped modulator for the vocoder, built from the patch itself: a saw buzz and a little
/// noise through a formant filter (hold on, so every vowel is clear). Returns the formant filter.
fn mouth(r: &mut Rack, buzz_hz: f32, hiss_db: f32, q: f32) -> ModuleId {
    let buzz = r.add(
        "osc.va",
        &[("base_hz", buzz_hz), ("waveform", 2.0), ("unison", 1.0)],
    );
    let hiss = r.add("noise", &[("color", 0.0), ("level_db", hiss_db)]);
    let mix = r.add("mixer", &[("level1", 0.7), ("level2", 1.0)]);
    r.wire(buzz, "out", mix, "in1");
    r.wire(hiss, "out", mix, "in2");
    let f = r.add(
        "filter.formant",
        &[("vowel", 0.0), ("shift_st", 0.0), ("q", q), ("hold", 1.0)],
    );
    r.wire(mix, "out", f, "in");
    f
}

/// Sock Puppet: a talkbox for French touch. The played saw lead is the carrier of a vocoder; the
/// modulator is a buzz and a hiss through a formant "mouth" built into the patch. Mouth places the
/// vowel, Chatter makes the mouth hop on the host clock (press Run), Size moves the formants,
/// Bite drives and crushes the voice.
fn sock_puppet() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Mouth", 0.30),
            ("Chatter", 0.35),
            ("Weight", 0.40),
            ("Bite", 0.40),
        ],
        midi: [1.0, 0.0, 120.0],
        oscs: vec![
            osc(2.0, 0.0, 0.45).unison(3.0, 9.0),
            osc(3.0, 0.0, 0.25).pw(35.0),
            osc(2.0, -1.0, 0.0),
        ],
        drive: [6.0, -3.0],
        lad: [9000.0, 0.05, 0.0],
        fenv: [2.0, 100.0, 0.5, 60.0],
        fenv_amt: 0.0,
        wheel_amt: 0.0,
        aenv: [3.0, 200.0, 0.9, 80.0],
        trim_db: 21.0,
    });
    let formant = mouth(&mut v.r, 110.0, -14.0, 1.8);
    // Mouth: the vowels differ in level; Weight: the octave adds level. Both are pulled back.
    comp_gain(&mut v, 1, 0.30, 4.0);
    comp_gain(&mut v, 3, 0.40, 3.0);
    let voc = v.stage(
        "vocoder",
        &[
            ("bands", 20.0),
            ("hf", 0.35),
            ("drive_db", 30.0),
            ("shift_st", 0.0),
            ("mix", 1.0),
        ],
    );
    v.r.wire(formant, "out", voc, "mod");
    // Mouth: a, e, i, o, u in clear steps (the mod wheel moves it too).
    v.r.route(v.knob, "m1", formant, "vowel", 1.0);
    v.r.route(v.m, "wheel", formant, "vowel", 0.5);
    // Chatter: on the host clock the mouth hops to a new vowel every eighth note.
    let clock = v.r.add("clock", &[("bpm", 124.0), ("swing", 0.0)]);
    let hop =
        v.r.add("lfo", &[("waveform", 4.0), ("sync", 2.0), ("rate_hz", 4.0)]);
    v.r.wire(clock, "gate", hop, "clock");
    v.r.depth_route(hop, "out", formant, "vowel", v.knob, "m2", 0.75);
    // Weight: a saw an octave down joins under the voice.
    let mix = v.mix;
    span(&mut v, 3, 0.40, mix, "mixer", "level3", 0.0, 0.2);
    // Bite: clean to driven, a crusher and the consonant hiss come in.
    let drive = v.drive;
    let d = 6.0;
    let s = span_lh(&mut v, 4, drive, "drive", "drive_db", 0.0, d, 28.0);
    span_hi(&mut v, 4, s, drive, "drive", "trim_db", -3.0, -12.0);
    span_hi(&mut v, 4, s, voc, "vocoder", "hf", 0.35, 0.9);
    let crush = v.stage(
        "bitcrusher",
        &[("bits", 9.0), ("downsample", 3.0), ("mix", 0.0)],
    );
    span(&mut v, 4, s, crush, "bitcrusher", "mix", 0.0, 60.0 * s);
    v.stage("compressor", &compressor(-24.0, 5.0, 3.0, 120.0, 5.0));
    v.finish_dry(Some(clock))
}

/// Temple Bowl: a struck metal bowl. An FM voice with an inharmonic ratio and a ring modulated
/// partner, a mallet-length index envelope, a long decay, a hall.
fn temple_bowl() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Strike", 0.5),
            ("Metal", 0.4),
            ("Ring", 0.5),
            ("Space", 0.6),
        ],
        midi: [0.0, 0.0, 120.0],
        oscs: vec![osc(0.0, 0.0, 0.0)],
        drive: [0.0, 0.0],
        lad: [12000.0, 0.05, 0.0],
        fenv: [1.0, 300.0, 0.0, 200.0],
        fenv_amt: 0.0,
        wheel_amt: 0.0,
        aenv: [3.0, 2500.0, 0.0, 1200.0],
        trim_db: 6.5,
    });
    // The modulator (inharmonic ratio) phase-modulates the carrier; the carrier's index is how hard.
    let modulator = v.r.add(
        "osc.fm",
        &[
            ("base_hz", 261.63),
            ("ratio", 2.76),
            ("index", 0.0),
            ("level", 1.0),
        ],
    );
    v.r.wire(v.m, "pitch", modulator, "pitch");
    let fm = v.r.add(
        "osc.fm",
        &[
            ("base_hz", 261.63),
            ("ratio", 1.0),
            ("index", 1.2),
            ("level", 1.0),
        ],
    );
    v.r.wire(v.m, "pitch", fm, "pitch");
    v.r.wire(modulator, "out", fm, "pm");
    let partner = v.r.add(
        "osc.va",
        &[("base_hz", 369.99), ("waveform", 0.0), ("fine", 0.0)],
    );
    v.r.wire(v.m, "pitch", partner, "pitch");
    let ring = v.r.add("ringmod", &[]);
    v.r.wire(fm, "out", ring, "a");
    v.r.wire(partner, "out", ring, "b");
    let mix = v.mix;
    v.r.e.set_param(mix, "level2", 0.45);
    v.r.wire(fm, "out", mix, "in2");
    v.r.e.set_param(mix, "level3", 0.0);
    v.r.wire(ring, "out", mix, "in3");
    // The strike: an index envelope that hits hard and falls in a mallet's length.
    let hit = v.r.add("env.adsr", &adsr(2.0, 220.0, 0.0, 120.0));
    v.r.wire(v.m, "gate", hit, "gate");
    v.r.route(hit, "out", fm, "index", 0.18);
    // Strike: a soft mallet to a hard clang (the index of the FM voice). Level held by a trim.
    let s = span_lh(&mut v, 1, fm, "osc.fm", "index", 0.3, 1.2, 3.5);
    let _ = s;
    // Metal: a tuned bowl to an inharmonic clang (the ratio and the ring modulated partner).
    let s = span_lh(&mut v, 2, modulator, "osc.fm", "ratio", 1.0, 2.76, 3.9);
    span(&mut v, 2, s, mix, "mixer", "level3", 0.0, 0.2);
    // Ring: a short tap to a six second ring.
    let aenv = v.aenv;
    let s = span_lh(
        &mut v, 3, aenv, "env.adsr", "decay_ms", 350.0, 2500.0, 7000.0,
    );
    span_hi(&mut v, 3, s, aenv, "env.adsr", "release_ms", 300.0, 1200.0);
    comp_gain(&mut v, 3, s, 9.0);
    v.stage(
        "chorus",
        &[("rate_hz", 0.2), ("depth", 30.0), ("mix", 18.0)],
    );
    v.finish("m4", &echo(620.0, 40.0, 3500.0), 0.3, (5.0, 0.55), None)
}

/// Depth Charge: an impact. A sine that falls in pitch, a noise crack, a long hall; every key is a
/// one-shot (the amp decays whatever the key does).
fn depth_charge() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[("Boom", 0.5), ("Crack", 0.4), ("Drop", 0.5), ("Space", 0.6)],
        midi: [0.0, 0.0, 120.0],
        oscs: vec![osc(0.0, 0.0, 0.8), osc(1.0, -1.0, 0.3)],
        drive: [8.0, -4.0],
        lad: [5000.0, 0.05, 0.0],
        fenv: [1.0, 200.0, 0.0, 100.0],
        fenv_amt: 0.0,
        wheel_amt: 0.0,
        aenv: [2.0, 1200.0, 0.0, 600.0],
        trim_db: 8.0,
    });
    let mix = v.mix;
    // The crack: a noise burst with its own short envelope.
    let noise = v.r.add("noise", &[("color", 0.0), ("level_db", -6.0)]);
    let burst = v.r.add("env.adsr", &adsr(1.0, 70.0, 0.0, 40.0));
    v.r.wire(v.m, "gate", burst, "gate");
    let vca = v.r.add("vca", &[("gain", 0.0)]);
    v.r.wire(noise, "out", vca, "in");
    v.r.wire(burst, "out", vca, "cv");
    v.r.wire(vca, "out", mix, "in3");
    // The fall: a fast envelope on the pitch of both oscillators.
    let fall = v.r.add("env.adsr", &adsr(1.0, 150.0, 0.0, 60.0));
    v.r.wire(v.m, "gate", fall, "gate");
    for o in v.oscs.clone() {
        v.r.depth_route(fall, "out", o, "base_hz", v.knob, "m3", 0.18);
    }
    // Boom: a short thud to a four second boom.
    let aenv = v.aenv;
    let s = span_lh(
        &mut v, 1, aenv, "env.adsr", "decay_ms", 250.0, 1200.0, 4500.0,
    );
    span_hi(&mut v, 1, s, aenv, "env.adsr", "release_ms", 150.0, 600.0);
    comp_gain(&mut v, 1, s, 6.0);
    // Crack: a dull thud to a hard crack on the front.
    let s = span_lh(&mut v, 2, mix, "mixer", "level3", 0.0, 0.35, 0.9);
    comp_gain(&mut v, 2, s, 1.0);
    v.stage("compressor", &compressor(-18.0, 3.0, 8.0, 150.0, 2.0));
    v.finish("m4", &echo(380.0, 30.0, 2500.0), 0.15, (7.5, 0.5), None)
}

// ---- phrases --------------------------------------------------------------------------------

fn puppet_phrase() -> Vec<Hit> {
    riff(
        &[
            (0, 57, 2.0),
            (2, 60, 2.0),
            (4, 64, 3.0),
            (8, 62, 2.0),
            (10, 60, 2.0),
            (12, 57, 4.0),
            (16, 55, 2.0),
            (18, 59, 2.0),
            (20, 62, 3.0),
            (24, 64, 2.0),
            (26, 62, 2.0),
            (28, 60, 4.0),
        ],
        32,
        3,
        0.7,
    )
}

fn bowl_phrase() -> Vec<Hit> {
    let mut v = line(&[
        (0.5, 57, 3.0),
        (4.0, 64, 3.0),
        (4.0, 69, 3.0),
        (8.0, 60, 3.0),
        (11.5, 67, 3.0),
        (11.5, 55, 3.0),
        (15.0, 57, 3.0),
    ]);
    v.extend(line(&[(15.0, 64, 3.0)]));
    v
}

fn charge_phrase() -> Vec<Hit> {
    line(&[
        (0.5, 31, 1.0),
        (3.5, 28, 1.0),
        (7.0, 33, 1.0),
        (10.0, 26, 1.0),
        (13.5, 31, 1.0),
        (16.5, 24, 1.0),
    ])
}

// ---- the list ---------------------------------------------------------------------------------

fn presets() -> Vec<Preset> {
    vec![
        Preset {
            slug: "sock-puppet",
            name: "Sock Puppet",
            category: "Voice",
            tags: &["mono", "vocoder", "talkbox", "124bpm", "french touch", "signature"],
            description: "A talkbox for French touch: your lead plays through a vocoder whose voice is a formant mouth built into the patch, close and dry. Press Run to start the clock for Chatter, then play a line and move the mod wheel. Mouth sets the vowel from a to u, Chatter makes the mouth hop to a new vowel every eighth note, Weight brings in a saw an octave down, Bite drives and crushes the voice.",
            listen: "A saw lead that talks; Mouth steps through the vowels, Chatter turns it into a chattering robot, Bite for grit.",
            build: sock_puppet,
            group: 1,
            phrase: puppet_phrase,
            secs: 13.0,
        },
        Preset {
            slug: "temple-bowl",
            name: "Temple Bowl",
            category: "Voice",
            tags: &["poly", "fm", "metal", "struck", "ambient", "experimental"],
            description: "A struck metal bowl: an FM voice with an inharmonic ratio and a ring modulated partner, a mallet strike and a long ring in a hall. Play single notes and open fifths and let them ring. Strike goes from a soft mallet to a hard clang, Metal from a tuned bowl to an inharmonic clang, Ring from a short tap to a seven second ring, Space adds the echo and the hall.",
            listen: "Bowls that ring for seconds; Strike up for the clang, Metal up for a harsher, clanging tone.",
            build: temple_bowl,
            group: 1,
            phrase: bowl_phrase,
            secs: 20.0,
        },
        Preset {
            slug: "depth-charge",
            name: "Depth Charge",
            category: "Voice",
            tags: &["poly", "impact", "sub", "cinematic", "hip-hop"],
            description: "An impact: a sine that falls in pitch under a noise crack, with a long hall. Every key is a one-shot, so play low single notes and let the tail run. Boom sets how long the thud rings, Crack adds the noise crack on the front, Drop sets how far the pitch falls, Space adds the echo and the hall.",
            listen: "A low hit with a crack and a falling pitch; Boom up for a long thud, Drop up for a bigger fall, Space for the cinematic tail.",
            build: depth_charge,
            group: 1,
            phrase: charge_phrase,
            secs: 20.0,
        },
        Preset {
            slug: "oracle",
            name: "Oracle",
            category: "Voice",
            tags: &["poly", "formant", "choir", "slow", "ambient", "cinematic"],
            description: "A choir of saws and a triangle through a formant filter that holds every vowel clear, with a slow bloom and a long room. Play slow chords and move the mod wheel. Vowel steps through a, e, i, o and u, Breath brings in a whispered layer through the same vowels, Choir spreads one voice into a crowd, Space adds the echo and the room.",
            listen: "A wordless choir; Vowel steps cleanly between the vowels, Breath for a whisper, Choir for one voice to a crowd.",
            build: oracle,
            group: 1,
            phrase: choir_phrase,
            secs: 20.0,
        },
        Preset {
            slug: "automaton",
            name: "Automaton",
            category: "Voice",
            tags: &["poly", "vocoder", "robot", "bitcrushed", "french touch", "signature"],
            description: "A robot choir for French touch: your chord is the carrier of a vocoder whose voice is a monotone buzz that speaks a looping sentence, built into the patch, close and dry (no echo or room). Play held chords. Speech goes from one held vowel to a sentence of vowels four seconds long, Detail from a coarse robot to a crisp one with consonants, Size from a small robot to a large one, Crush adds a digital grind.",
            listen: "Chords that talk in a loop; Speech at 0 is a held vowel, at 1 a sentence. Detail for crisp consonants, Crush for grit.",
            build: automaton,
            group: 1,
            phrase: robot_phrase,
            secs: 17.0,
        },
        Preset {
            slug: "vox-chop",
            name: "Vox Chop",
            category: "Voice",
            tags: &["poly", "formant", "chopped", "124bpm", "house"],
            description: "House chord stabs that talk: saws through a formant filter whose vowel hops on every sixteenth of the host clock, with a breathing chop on the level. Press Run to start the clock, then play short chords. Chop goes from a steady level to a deep, breathing pulse, Vowels from one vowel to a mouth that jumps all over, Shift from a small bright mouth to a large dark one, Space adds the echo and the room.",
            listen: "Stabs whose vowel jumps on every sixteenth; Vowels at 0 is one vowel, at 1 it chatters. Chop for the pulse.",
            build: vox_chop,
            group: 2,
            phrase: chop_phrase,
            secs: 10.5,
        },
        Preset {
            slug: "chandelier",
            name: "Chandelier",
            category: "Voice",
            tags: &["poly", "fm", "bowed", "glass", "pop", "ambient"],
            description: "Bowed crystal: a sine with a high FM partial and a rubbed rim, bowed in slowly and left in a hall. Play slow, high notes and fifths. Rub brings in the friction of a wet rim with a hint of vibrato, Overtone brings forward the glassy partial, Bow goes from a struck ping to a slow bow, Space adds the echo and the hall.",
            listen: "Pure, glassy notes that bloom; Rub up for the singing rim, Overtone for glitter, Bow for how slowly each note arrives.",
            build: chandelier,
            group: 2,
            phrase: glass_phrase,
            secs: 20.0,
        },
        Preset {
            slug: "whiteout",
            name: "Whiteout",
            category: "Voice",
            tags: &["poly", "noise", "wind", "comb", "experimental", "ambient"],
            description: "Wind you can play: noise through a comb filter that follows the keys, a filter that gusts and a long room. Play slow notes and fifths. Pitch goes from plain wind to a ringing note, Gust makes the filter open and close, deeper and faster, Grain brings in a bitcrusher, Space adds the echo and the room.",
            listen: "A wind that sings the notes you play; Pitch at 0 is a plain wash, at 1 it rings. Gust for slow breathing.",
            build: whiteout,
            group: 2,
            phrase: wind_phrase,
            secs: 20.0,
        },
        Preset {
            slug: "updraft",
            name: "Updraft",
            category: "Voice",
            tags: &["poly", "riser", "noise", "build", "techno", "house"],
            description: "A riser: hold a key and the sound builds, a closed filter opening over seconds under a saw stack and noise. Hold a note for the whole build and let go to end it. Build sets how long the rise takes, from one and a half to ten seconds, Lift opens the filter further and raises the resonance with a very small pitch rise, Heat adds saturation and noise, Space adds the echo and the room.",
            listen: "Hold a note: a dark rumble that opens into a roaring wash. Build for time, Lift for how high it gets.",
            build: updraft,
            group: 2,
            phrase: riser_phrase,
            secs: 17.5,
        },
        Preset {
            slug: "maw",
            name: "Maw",
            category: "Voice",
            tags: &["mono", "formant", "growl", "distorted", "drum and bass"],
            description: "A growling bass voice for drum and bass: a saw stack through a formant filter whose vowel a free LFO throws around, dry and heavy (no echo or room). Play low single notes. Vowel sets the mouth from a to u, Growl goes from still to a fast growl, Grit adds drive and a crusher, Weight brings in a sine sub.",
            listen: "A mouth in the bass: Vowel moves the mouth, Growl turns it into a snarl, Grit for bite.",
            build: maw,
            group: 2,
            phrase: maw_phrase,
            secs: 10.4,
        },
    ]
}

// ---- the second seven -----------------------------------------------------------------------

/// Oracle: a choir of saws and a triangle through a formant filter that holds every vowel clear.
fn oracle() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Vowel", 0.10),
            ("Breath", 0.00),
            ("Choir", 0.30),
            ("Space", 0.65),
        ],
        midi: [0.0, 0.0, 120.0],
        oscs: vec![
            osc(2.0, 0.0, 0.32).unison(3.0, 10.0),
            osc(1.0, 0.0, 0.28).fine(5.0),
        ],
        drive: [3.0, -1.0],
        lad: [7000.0, 0.08, 0.0],
        fenv: [2.0, 100.0, 0.5, 60.0],
        fenv_amt: 0.0,
        wheel_amt: 0.0,
        aenv: [380.0, 600.0, 0.9, 1100.0],
        trim_db: 16.0,
    });
    let mix = v.mix;
    let noise = v.r.add("noise", &[("color", 0.3), ("level_db", 0.0)]);
    v.r.wire(noise, "out", mix, "in3");
    // Breath: a whispered layer through the same vowels (texture on a macro, off by default).
    v.r.e.set_param(mix, "level3", 0.0);
    v.r.route(v.knob, "m2", mix, "level3", 1.0);
    comp_gain(&mut v, 1, 0.10, 3.0);
    let formant = v.mono_after(
        "filter.formant",
        &[("vowel", 0.0), ("shift_st", 0.0), ("q", 1.6), ("hold", 1.0)],
    );
    // Vowel: a, e, i, o, u in clear steps (the mod wheel moves it too).
    v.r.route(v.knob, "m1", formant, "vowel", 1.0);
    v.r.route(v.m, "wheel", formant, "vowel", 0.5);
    // Choir: one voice to a crowd (the saw stack spreads, the chorus widens). Level held by a trim.
    let o0 = v.oscs[0];
    let s = span_lh(&mut v, 3, o0, "osc.va", "detune", 3.0, 10.0, 34.0);
    let chorus = v.stage(
        "chorus",
        &[("rate_hz", 0.2), ("depth", 45.0), ("mix", 28.0)],
    );
    span(&mut v, 3, s, chorus, "chorus", "mix", 10.0, 28.0);
    span(&mut v, 3, s, chorus, "chorus", "depth", 35.0, 45.0);
    v.finish("m4", &echo(700.0, 45.0, 3000.0), 0.3, (8.0, 0.6), None)
}

/// Automaton: a vocoder robot choir. The played chord is the carrier; the modulator is a monotone
/// buzz that speaks a looping sentence (a free LFO walks the vowels), built into the patch.
fn automaton() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Speech", 0.50),
            ("Detail", 0.45),
            ("Size", 0.50),
            ("Crush", 0.00),
        ],
        midi: [0.0, 0.0, 120.0],
        oscs: vec![
            osc(2.0, 0.0, 0.34).unison(3.0, 9.0),
            osc(3.0, 0.0, 0.2).pw(40.0),
        ],
        drive: [4.0, -2.0],
        lad: [9000.0, 0.05, 0.0],
        fenv: [2.0, 100.0, 0.5, 60.0],
        fenv_amt: 0.0,
        wheel_amt: 0.0,
        aenv: [6.0, 300.0, 0.9, 250.0],
        trim_db: 15.0,
    });
    let formant = mouth(&mut v.r, 98.0, -12.0, 1.8);
    v.r.e.set_param(formant, "vowel", 1.0);
    // Speech: from one held vowel to a sentence that walks a, e, i, o, u and back, four seconds long.
    let sentence = v.r.add("lfo", &[("rate_hz", 0.25), ("waveform", 2.0)]);
    v.r.depth_route(sentence, "out", formant, "vowel", v.knob, "m1", 0.5);
    let voc = v.stage(
        "vocoder",
        &[
            ("bands", 16.0),
            ("hf", 0.2),
            ("drive_db", 30.0),
            ("shift_st", 0.0),
            ("mix", 1.0),
        ],
    );
    v.r.wire(formant, "out", voc, "mod");
    // Detail: a smeared, vowel-like robot to a crisp, articulate one (fast followers, consonant hiss).
    let s = span_lh(&mut v, 2, voc, "vocoder", "release_ms", 160.0, 50.0, 12.0);
    span(&mut v, 2, s, voc, "vocoder", "hf", 0.0, 0.25);
    // Size: a small robot to a large one.
    span_lh(&mut v, 3, voc, "vocoder", "shift_st", 6.0, 0.0, -6.0);
    // Crush: clean to a coarse digital grind.
    let crush = v.stage(
        "bitcrusher",
        &[("bits", 5.0), ("downsample", 6.0), ("mix", 0.0)],
    );
    v.r.route(v.knob, "m4", crush, "mix", 0.8);
    v.stage("compressor", &compressor(-24.0, 5.0, 3.0, 120.0, 5.0));
    v.finish_dry(None)
}

/// Vox Chop: house chord stabs through a formant filter whose vowel hops every sixteenth of the host
/// clock, with a clock-driven chop on the level. Press Run.
fn vox_chop() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Chop", 0.50),
            ("Vowels", 0.50),
            ("Shift", 0.50),
            ("Space", 0.30),
        ],
        midi: [0.0, 0.0, 120.0],
        oscs: vec![
            osc(2.0, 0.0, 0.34).unison(3.0, 10.0),
            osc(3.0, 0.0, 0.2).pw(40.0),
        ],
        drive: [8.0, -4.0],
        lad: [6000.0, 0.08, 0.0],
        fenv: [2.0, 100.0, 0.5, 60.0],
        fenv_amt: 0.0,
        wheel_amt: 0.0,
        aenv: [4.0, 300.0, 0.9, 140.0],
        trim_db: 23.5,
    });
    let clock = v.r.add("clock", &[("bpm", 124.0), ("swing", 0.0)]);
    let formant = v.mono_after(
        "filter.formant",
        &[("vowel", 1.0), ("shift_st", 0.0), ("q", 1.6), ("hold", 1.0)],
    );
    // Vowels: the mouth hops to a new vowel on every sixteenth, from still to jumping all over.
    let hop =
        v.r.add("lfo", &[("waveform", 4.0), ("sync", 1.0), ("rate_hz", 8.0)]);
    v.r.wire(clock, "gate", hop, "clock");
    v.r.depth_route(hop, "out", formant, "vowel", v.knob, "m2", 0.5);
    // Chop: a silent envelope on every eighth of the clock ducks the level (no drum), shallow to deep.
    let eighth = v.r.add("clock.div", &[("div", 2.0)]);
    v.r.wire(clock, "gate", eighth, "clock");
    v.r.wire(clock, "reset", eighth, "reset");
    let duck = v.r.add("env.adsr", &adsr(2.0, 110.0, 0.0, 30.0));
    v.r.wire(eighth, "gate", duck, "gate");
    let g = v.mono_after("gain", &[("gain_db", 0.0)]);
    v.r.depth_route(duck, "out", g, "gain_db", v.knob, "m1", -0.5);
    comp_gain(&mut v, 1, 0.5, -3.0);
    // Shift: a small, bright mouth to a large, dark one.
    span_lh(
        &mut v,
        3,
        formant,
        "filter.formant",
        "shift_st",
        7.0,
        0.0,
        -7.0,
    );
    comp_gain(&mut v, 3, 0.5, 3.5);
    v.stage("compressor", &compressor(-22.0, 4.0, 3.0, 100.0, 4.0));
    let mut e = echo(250.0, 30.0, 3500.0).to_vec();
    e.push(("sync", 0.0));
    v.finish("m4", &e, 0.2, (2.5, 0.3), Some(clock))
}

/// Chandelier: glass. A sine, a high FM partial and a rubbed rim, bowed in slowly.
fn chandelier() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Rub", 0.00),
            ("Overtone", 0.35),
            ("Bow", 0.40),
            ("Space", 0.60),
        ],
        midi: [0.0, 0.0, 120.0],
        oscs: vec![osc(0.0, 0.0, 0.5), osc(1.0, 1.0, 0.0)],
        drive: [0.0, 0.0],
        lad: [11000.0, 0.05, 0.0],
        fenv: [2.0, 100.0, 0.5, 60.0],
        fenv_amt: 0.0,
        wheel_amt: 0.0,
        aenv: [300.0, 600.0, 0.9, 1400.0],
        trim_db: 2.5,
    });
    let mix = v.mix;
    // The glass partial: a modulator and a carrier a few times the pitch, a thin ring on top.
    let modulator = v.r.add(
        "osc.fm",
        &[
            ("base_hz", 261.63),
            ("ratio", 3.5),
            ("index", 0.0),
            ("level", 1.0),
        ],
    );
    v.r.wire(v.m, "pitch", modulator, "pitch");
    let top = v.r.add(
        "osc.fm",
        &[
            ("base_hz", 261.63),
            ("ratio", 2.0),
            ("index", 0.9),
            ("level", 1.0),
        ],
    );
    v.r.wire(v.m, "pitch", top, "pitch");
    v.r.wire(modulator, "out", top, "pm");
    v.r.wire(top, "out", mix, "in3");
    v.r.e.set_param(mix, "level3", 0.0);
    // The rim: noise through a narrow band around 1.1 kHz (a wet-finger hum, under the glass partial).
    let noise = v.r.add("noise", &[("color", 0.0), ("level_db", -6.0)]);
    let rim =
        v.r.add("filter.svf", &[("cutoff_hz", 1100.0), ("resonance", 0.9)]);
    v.r.wire(noise, "out", rim, "in");
    v.r.wire(rim, "bp", mix, "in4");
    v.r.e.set_param(mix, "level4", 0.0);
    // Rub: the rim's friction fades in, with a very small vibrato (the motion is the friction, not pitch).
    v.r.route(v.knob, "m1", mix, "level4", 0.45);
    let lfo = v.r.add("lfo", &[("rate_hz", 5.5)]);
    for o in v.oscs.clone() {
        v.r.depth_route(lfo, "out", o, "fine", v.knob, "m1", 0.02);
    }
    // Overtone: the glass partial comes forward.
    span(&mut v, 2, 0.35, mix, "mixer", "level3", 0.0, 0.1);
    comp_gain(&mut v, 2, 0.35, 2.0);
    // Bow: from a struck ping to a slow bow.
    let aenv = v.aenv;
    span_lh(&mut v, 3, aenv, "env.adsr", "attack_ms", 8.0, 300.0, 2600.0);
    v.stage(
        "chorus",
        &[("rate_hz", 0.3), ("depth", 30.0), ("mix", 20.0)],
    );
    v.finish("m4", &echo(540.0, 42.0, 3500.0), 0.35, (6.5, 0.6), None)
}

/// Whiteout: wind you can play. Noise through a comb that follows the key, a ladder that gusts.
fn whiteout() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Pitch", 0.50),
            ("Gust", 0.35),
            ("Grain", 0.00),
            ("Space", 0.50),
        ],
        midi: [0.0, 0.0, 120.0],
        oscs: vec![osc(0.0, 0.0, 0.0)],
        drive: [0.0, 0.0],
        lad: [3500.0, 0.2, 0.0],
        fenv: [2.0, 100.0, 0.5, 60.0],
        fenv_amt: 0.0,
        wheel_amt: 0.0,
        aenv: [900.0, 600.0, 0.9, 1800.0],
        trim_db: 17.7,
    });
    let mix = v.mix;
    let noise = v.r.add("noise", &[("color", 0.4), ("level_db", 0.0)]);
    let comb = v.r.add(
        "filter.comb",
        &[("freq_hz", 261.63), ("feedback", 70.0), ("damp", 45.0)],
    );
    v.r.wire(noise, "out", comb, "in");
    v.r.wire(v.m, "pitch", comb, "pitch");
    v.r.wire(comb, "out", mix, "in2");
    v.r.wire(noise, "out", mix, "in3");
    v.r.e.set_param(mix, "level2", 0.6);
    v.r.e.set_param(mix, "level3", 0.35);
    // Pitch: plain wind to a ringing note (the comb's feedback up, the raw noise down).
    let s = span_lh(&mut v, 1, comb, "filter.comb", "feedback", 0.0, 70.0, 94.0);
    span(&mut v, 1, s, mix, "mixer", "level3", 0.75, 0.3);
    span(&mut v, 1, s, mix, "mixer", "level2", 0.3, 0.75);
    // Gust: a slow LFO opens and closes the filter, deeper and faster.
    let lfo = v.r.add("lfo", &[("rate_hz", 0.3)]);
    let lad = v.lad;
    v.r.depth_route(lfo, "out", lad, "cutoff_hz", v.knob, "m2", 0.3);
    span(&mut v, 2, 0.35, lfo, "lfo", "rate_hz", 0.1, 0.3);
    comp_gain(&mut v, 2, 0.35, 4.0);
    // Grain: a bitcrusher on the wind, from clean to gritty.
    let crush = v.stage(
        "bitcrusher",
        &[("bits", 5.0), ("downsample", 6.0), ("mix", 0.0)],
    );
    v.r.route(v.knob, "m3", crush, "mix", 0.8);
    v.finish("m4", &echo(620.0, 45.0, 3000.0), 0.3, (7.0, 0.55), None)
}

/// Updraft: a riser. Hold a key and the sound builds: a filter that opens over seconds, noise and
/// a saw stack, resonance and heat.
fn updraft() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Build", 0.45),
            ("Lift", 0.25),
            ("Heat", 0.30),
            ("Space", 0.40),
        ],
        midi: [0.0, 0.0, 120.0],
        oscs: vec![
            osc(2.0, 0.0, 0.3).unison(5.0, 16.0),
            osc(2.0, 1.0, 0.12).unison(3.0, 14.0),
        ],
        drive: [6.0, -3.0],
        lad: [250.0, 0.2, 0.0],
        fenv: [5000.0, 200.0, 1.0, 400.0],
        fenv_amt: 0.0,
        wheel_amt: 0.0,
        aenv: [200.0, 400.0, 1.0, 500.0],
        trim_db: 14.0,
    });
    let mix = v.mix;
    let noise = v.r.add("noise", &[("color", 0.2), ("level_db", -10.0)]);
    v.r.wire(noise, "out", mix, "in3");
    v.r.e.set_param(mix, "level3", 0.25);
    // Lift: how far the filter sweeps open (and the resonance with it), with a very small pitch rise.
    let (fenv, lad) = (v.fenv, v.lad);
    v.r.depth_route(fenv, "out", lad, "cutoff_hz", v.knob, "m2", 0.9);
    span(&mut v, 2, 0.25, lad, "filter.ladder", "resonance", 0.1, 0.2);
    for o in v.oscs.clone() {
        v.r.depth_route(fenv, "out", o, "fine", v.knob, "m2", 0.3);
    }
    // Build: how long the rise takes, 1.5 s to 10 s.
    span_lh(
        &mut v,
        1,
        fenv,
        "env.adsr",
        "attack_ms",
        1500.0,
        5000.0,
        10000.0,
    );
    // Heat: the saturation and the noise grow.
    let drive = v.drive;
    let s = span_lh(&mut v, 3, drive, "drive", "drive_db", 0.0, 6.0, 20.0);
    comp_gain(&mut v, 3, s, 3.0);
    span_hi(&mut v, 3, s, drive, "drive", "trim_db", -3.0, -10.0);
    span(&mut v, 3, s, mix, "mixer", "level3", 0.15, 0.25);
    v.stage("compressor", &compressor(-20.0, 3.0, 8.0, 150.0, 3.0));
    v.finish("m4", &echo(480.0, 40.0, 3000.0), 0.3, (4.0, 0.45), None)
}

/// Maw: a growling bass voice for drum and bass. A saw stack through a formant filter whose vowel a
/// free LFO moves, a sub, and drive.
fn maw() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Vowel", 0.30),
            ("Growl", 0.40),
            ("Grit", 0.30),
            ("Weight", 0.40),
        ],
        midi: [1.0, 0.0, 60.0],
        oscs: vec![
            osc(2.0, -1.0, 0.4).unison(2.0, 14.0),
            osc(3.0, -1.0, 0.2).pw(40.0),
            osc(0.0, -2.0, 0.0),
        ],
        drive: [12.0, -6.0],
        lad: [5000.0, 0.1, 0.0],
        fenv: [2.0, 100.0, 0.5, 60.0],
        fenv_amt: 0.0,
        wheel_amt: 0.0,
        aenv: [3.0, 200.0, 0.9, 80.0],
        trim_db: 24.0,
    });
    let mix = v.mix;
    // Weight: the sub comes in under the voice.
    span(&mut v, 4, 0.4, mix, "mixer", "level3", 0.0, 0.3);
    comp_gain(&mut v, 4, 0.4, -2.3);
    let formant = v.mono_after(
        "filter.formant",
        &[
            ("vowel", 0.0),
            ("shift_st", -2.0),
            ("q", 2.0),
            ("hold", 0.0),
        ],
    );
    v.r.route(v.knob, "m1", formant, "vowel", 1.0);
    // Growl: a free LFO throws the mouth between vowels, from still to a fast growl.
    let lfo = v.r.add("lfo", &[("rate_hz", 2.0)]);
    v.r.depth_route(lfo, "out", formant, "vowel", v.knob, "m2", 0.3);
    span_hi(&mut v, 2, 0.4, lfo, "lfo", "rate_hz", 1.2, 6.0);
    // Grit: drive and a crusher.
    let drive = v.drive;
    let s = span_lh(&mut v, 3, drive, "drive", "drive_db", 4.0, 12.0, 30.0);
    span_hi(&mut v, 3, s, drive, "drive", "trim_db", -0.5, -10.0);
    let crush = v.stage(
        "bitcrusher",
        &[("bits", 8.0), ("downsample", 3.0), ("mix", 0.0)],
    );
    span(&mut v, 3, s, crush, "bitcrusher", "mix", 0.0, 50.0 * s);
    v.stage("compressor", &compressor(-22.0, 4.0, 3.0, 100.0, 4.0));
    v.finish_dry(None)
}

// ---- phrases of the second seven -----------------------------------------------------------

fn choir_phrase() -> Vec<Hit> {
    let mut v = Vec::new();
    for (t, notes) in [
        (0.5, [57u8, 60, 64]),
        (5.5, [53, 57, 60]),
        (10.5, [48, 55, 60]),
        (15.5, [55, 59, 62]),
    ] {
        v.extend(chord(t, &notes, 5.0));
    }
    v
}

fn robot_phrase() -> Vec<Hit> {
    let mut v = Vec::new();
    for (t, notes) in [
        (0.5, [57u8, 64, 69]),
        (4.5, [53, 60, 65]),
        (8.5, [48, 55, 64]),
        (12.5, [55, 62, 67]),
    ] {
        v.extend(chord(t, &notes, 3.8));
    }
    v
}

fn chop_phrase() -> Vec<Hit> {
    let grid = [(0, 3.0), (3, 1.0), (6, 2.0), (8, 3.0), (11, 1.0), (14, 2.0)];
    let mut v = Vec::new();
    for (bar, notes) in [
        (0u32, &[57u8, 60, 64][..]),
        (1, &[53, 57, 60]),
        (2, &[55, 59, 62]),
        (3, &[57, 60, 64]),
        (4, &[53, 57, 60]),
    ] {
        for h in stabs(notes, &grid) {
            v.push(Hit(h.0 + 0.5 + bar as f32 * 16.0 * S16, h.1, h.2));
        }
    }
    v
}

fn glass_phrase() -> Vec<Hit> {
    line(&[
        (0.5, 76, 4.0),
        (3.5, 79, 4.0),
        (3.5, 72, 4.0),
        (7.0, 81, 4.0),
        (7.0, 69, 4.0),
        (10.5, 84, 4.5),
        (10.5, 76, 4.5),
        (14.5, 79, 4.5),
        (14.5, 72, 4.5),
    ])
}

fn wind_phrase() -> Vec<Hit> {
    line(&[
        (0.5, 48, 4.0),
        (3.5, 55, 4.0),
        (7.0, 53, 4.5),
        (7.0, 60, 4.5),
        (11.0, 50, 5.0),
        (14.0, 57, 5.0),
    ])
}

fn riser_phrase() -> Vec<Hit> {
    line(&[(0.5, 45, 7.5), (9.0, 48, 7.5)])
}

fn maw_phrase() -> Vec<Hit> {
    riff(
        &[
            (0, 31, 3.0),
            (4, 31, 2.0),
            (6, 34, 2.0),
            (8, 36, 3.0),
            (12, 33, 2.0),
            (14, 31, 2.0),
        ],
        16,
        5,
        0.5,
    )
}

/// The render of `factory_levels_hold_the_target` (crates/ui/tests/factory_bank.rs `render_raw`):
/// 24 s, keys down at 1, 5 and 13 s (a chord, or one note for a legato patch) and up at 4, 9 and
/// 17 s, the clock run at 1 s, and every macro set to 0.65 at 3 s and to 0.15 later, or to 1 for the
/// macros in `hot` (bit n-1 is macro n). Returns (peak dBFS, loudness dBFS over blocks above -50).
fn test_level(state: &kabl_core::PatchState, hot: u8) -> (f32, f32) {
    let collector = Collector::new();
    let mut engine = PatchEngine::new(&collector.handle(), state, 48000.0, 8).expect("compiles");
    let clocks: Vec<ModuleId> = state
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "clock")
        .map(|(&id, _)| id)
        .collect();
    let legato = state
        .modules
        .values()
        .any(|m| m.kind == "midi.in" && m.params.get("mode").is_some_and(|v| *v > 0.0));
    let first_is_midi = state.modules.get(&1).is_some_and(|m| m.kind == "midi.in");
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    let mut samples = Vec::new();
    for b in 0..24 * 48000 / BLOCK {
        let t = b * BLOCK;
        if t == 0 {
            for &c in &clocks {
                engine.transport(c, Transport::Stop);
            }
        }
        if t == 48000 {
            for &c in &clocks {
                engine.transport(c, Transport::Run);
            }
        }
        if t.is_multiple_of(48000) && [1, 5, 13].contains(&(t / 48000)) {
            let notes: Vec<u8> = if legato {
                vec![if first_is_midi { 48 } else { 64 }]
            } else {
                vec![52, 59, 64, 67]
            };
            for note in notes {
                engine.key(KeyEvent::On { note, velocity: 90 });
            }
        }
        if t.is_multiple_of(48000) && [4, 9, 17].contains(&(t / 48000)) {
            engine.key(KeyEvent::AllOff);
        }
        if t.is_multiple_of(48000) && [3, 7, 19].contains(&(t / 48000)) {
            for (&id, m) in &state.modules {
                if m.kind == "macro" {
                    for n in 1..=4u16 {
                        let value = if hot & (1 << (n - 1)) != 0 {
                            1.0
                        } else if t == 3 * 48000 {
                            0.65
                        } else {
                            0.15
                        };
                        engine.set(&kabl_engine::runtime::ParamSet {
                            rev: t as u64 + 1,
                            target: kabl_engine::runtime::RuntimeTarget::Param {
                                id,
                                kind: "macro",
                                index: n - 1,
                            },
                            value,
                        });
                    }
                }
            }
        }
        if t == 16 * 48000 {
            engine.key(KeyEvent::AllOff);
            for &c in &clocks {
                engine.transport(c, Transport::Stop);
            }
        }
        engine.process_block(&mut l, &mut r);
        for (a, c) in l.iter().zip(&r) {
            samples.push(*a);
            samples.push(*c);
        }
    }
    let db = |v: f64| 20.0 * v.max(1e-12).log10();
    let peak = samples.iter().fold(0.0f32, |a, b| a.max(b.abs()));
    let (mut sum, mut n) = (0.0f64, 0usize);
    for block in samples.chunks(9600) {
        let ms = block.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / block.len() as f64;
        if ms > 1e-5 {
            sum += ms;
            n += 1;
        }
    }
    (
        db(peak as f64) as f32,
        db((sum / n.max(1) as f64).sqrt()) as f32,
    )
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
        Some("testlevel") => {
            // testlevel <patches dir> [slug ...]: what the factory level test measures, with
            // every macro at 0.65 then 0.15, and each macro alone at 1, and all four at 1
            let want: Vec<&String> = args.iter().skip(3).collect();
            for p in presets()
                .into_iter()
                .filter(|p| want.is_empty() || want.iter().any(|w| **w == p.slug))
            {
                let log =
                    kabl_core::load(&Path::new(&args[2]).join("presets").join(p.slug)).unwrap();
                let (peak0, loud) = test_level(log.state(), 0);
                let mut line = format!(
                    "TEST {:13} stored: peak {peak0:6.2} loud {loud:6.2} |",
                    p.slug
                );
                let mut worst = f32::MIN;
                for hot in [1u8, 2, 4, 8, 15] {
                    let (pk, _) = test_level(log.state(), hot);
                    worst = worst.max(pk);
                    line += &format!(" {hot}:{pk:6.2}");
                }
                println!("{line} | worst {:6.2}", worst.max(peak0));
            }
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
        Some("fold") => {
            // fold <patches dir>: mono-fold level against the stereo level (dB), level-test chord
            for p in presets().into_iter().filter(|p| only(p)) {
                let log =
                    kabl_core::load(&Path::new(&args[2]).join("presets").join(p.slug)).unwrap();
                let notes = [52u8, 59, 64, 67];
                let hits: Vec<Hit> = notes
                    .iter()
                    .flat_map(|&n| [Hit(1.0, n, Some(90)), off(4.0, n)])
                    .collect();
                let (l, r) = render(log.state(), &hits, 6.0);
                let (mut st, mut mo) = (0.0f64, 0.0f64);
                for (a, b) in l.iter().zip(&r).skip(48000) {
                    st += ((*a as f64).powi(2) + (*b as f64).powi(2)) / 2.0;
                    mo += (((*a + *b) / 2.0) as f64).powi(2);
                }
                println!(
                    "FOLD {:14} mono {:6.2} dB",
                    p.slug,
                    10.0 * (mo / st.max(1e-18)).log10()
                );
            }
        }
        Some("macros") => {
            // Peak and loudness (dBFS) of the clip phrase with every macro at 0 and at 1.
            let db = |v: f32| 20.0 * v.max(1e-9).log10();
            for p in presets().into_iter().filter(|p| only(p)) {
                let mut line = format!("{:14}", p.slug);
                for (label, value) in [("all 0", 0.0), ("all 1", 1.0)] {
                    let mut rack = (p.build)();
                    rack.trim(level_trim(p.slug));
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
                    rack.trim(level_trim(p.slug));
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
            "usage: presets_voice write|clips|levels|macros|library <patches dir> [out dir] [group]"
        ),
    }
}
