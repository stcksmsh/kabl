//! The factory pads (docs/presets/BRIEF.md, docs/presets/PAD.md): builds the patches from the
//! existing modules, writes them under `patches/presets/`, renders a clip per preset by playing a
//! short chord progression through the real engine, and prints the level and the library entries.
//!
//! ```sh
//! cargo run -p kabl-tools --example presets_pad -- write patches
//! cargo run --release -p kabl-tools --example presets_pad -- clips patches out_dir [group]
//! cargo run --release -p kabl-tools --example presets_pad -- levels patches
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
            ("Arrival", 0.50),
            ("Haze", 0.50),
            ("Drift", 0.08),
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
    // The octave sub stays where the old Depth left it.
    v.r.e.set_param(v.mix, "level3", 0.175);
    let ch = v.stage(
        "chorus",
        &[("rate_hz", 0.15), ("depth", 55.0), ("mix", 22.0)],
    );
    // Arrival: how slowly the cloud comes in and how long it hangs.
    let (fenv, aenv) = (v.fenv, v.aenv);
    let s = span_lh(
        &mut v,
        1,
        aenv,
        "env.adsr",
        "attack_ms",
        400.0,
        2600.0,
        4500.0,
    );
    span_hi(&mut v, 1, s, fenv, "env.adsr", "attack_ms", 3500.0, 6000.0);
    span_hi(&mut v, 1, s, aenv, "env.adsr", "release_ms", 5000.0, 9000.0);
    // Haze: the cloud thickens and brightens.
    let d = routed("filter.ladder", "cutoff_hz", 900.0, 0.5, 0.5);
    let lad = v.lad;
    let s = span_lh(
        &mut v,
        2,
        lad,
        "filter.ladder",
        "cutoff_hz",
        1300.0,
        d,
        12000.0,
    );
    span_hi(&mut v, 2, s, ch, "chorus", "mix", 22.0, 33.0);
    // The slow filter walk stays at what the old Drift left (stored 0.4 of 0.35). Drift is now the
    // pitch alone: a sway that fades in and speeds up.
    let drift = v.r.add("lfo", &[("rate_hz", 0.07)]);
    v.r.route(drift, "out", v.lad, "cutoff_hz", 0.14);
    let vib = v.r.add("lfo", &[("rate_hz", 0.13)]);
    for o in v.oscs.clone() {
        v.r.depth_route(vib, "out", o, "fine", v.knob, "m3", 0.3);
    }
    span_hi(&mut v, 3, 0.08, vib, "lfo", "rate_hz", 0.13, 0.9);
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
        15.0,
    ));
    let ch = v.stage(
        "chorus",
        &[("rate_hz", 0.10), ("depth", 45.0), ("mix", 15.0)],
    );
    // Swell: how far and how slowly each chord opens out of the dark.
    let fenv = v.fenv;
    let s = span_lh(
        &mut v,
        1,
        fenv,
        "env.adsr",
        "attack_ms",
        600.0,
        4500.0,
        9000.0,
    );
    v.r.depth_route(v.fenv, "out", v.lad, "cutoff_hz", v.knob, "m1", 0.3 / s);
    // Sub and Shimmer: an octave under and an octave over.
    let mix = v.mix;
    span(&mut v, 2, 0.5, mix, "mixer", "level4", 0.0, 0.3);
    comp(&mut v, 2, 0.5, 8.1);
    span(&mut v, 3, 0.35, mix, "mixer", "level3", 0.0, 0.175);
    comp(&mut v, 3, 0.35, 0.7);
    span_hi(&mut v, 3, 0.35, ch, "chorus", "depth", 45.0, 85.0);
    v.finish("m4", &echo(800.0, 50.0, 2500.0), 0.3, (6.5, 0.6), None)
}

/// Cotton Mill: an analog pop pad, a pulse with moving width and two saws, chorus.
fn cotton_mill() -> Rack {
    let mut v = Voice::new(pad_recipe(
        &[
            ("Motion", 0.25),
            ("Chorus", 0.50),
            ("Warmth", 0.60),
            ("Beat", 0.40),
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
    let d = routed("filter.ladder", "cutoff_hz", 1800.0, 0.5, 0.5);
    v.r.e.set_param(v.lad, "cutoff_hz", d);
    let ch = v.stage("chorus", &[("rate_hz", 0.4), ("depth", 50.0), ("mix", 0.0)]);
    // Motion: the pulse width starts to move, faster and wider.
    let pwm = v.r.add("lfo", &[("rate_hz", 0.45)]);
    v.r.depth_route(pwm, "out", v.oscs[0], "pw", v.knob, "m1", 0.9);
    span_hi(&mut v, 1, 0.25, pwm, "lfo", "rate_hz", 0.45, 3.0);
    comp(&mut v, 1, 0.25, -2.1);
    // Chorus: nothing to a deep, fast wash.
    span(&mut v, 2, 0.5, ch, "chorus", "mix", 0.0, 40.0);
    span_hi(&mut v, 2, 0.5, ch, "chorus", "depth", 50.0, 90.0);
    span_hi(&mut v, 2, 0.5, ch, "chorus", "rate_hz", 0.4, 1.4);
    // Warmth: a dark, round pad opens to a bright, glassy one.
    let lad = v.lad;
    span_lh(
        &mut v,
        3,
        lad,
        "filter.ladder",
        "cutoff_hz",
        700.0,
        d,
        20000.0,
    );
    // Beat: the two saws start locked to the pulse and drift apart into a slow beating.
    let (o1, o2) = (v.oscs[1], v.oscs[2]);
    span(&mut v, 4, 0.4, o1, "osc.va", "fine", 0.0, 6.0);
    span(&mut v, 4, 0.4, o2, "osc.va", "fine", 0.0, -6.0);
    comp(&mut v, 4, 0.4, 5.0);
    v.finish("=0.5", &echo(380.0, 30.0, 3500.0), 0.25, (3.0, 0.5), None)
}

/// Picture Window: a clean, bright seven-voice saw stack for pop.
fn picture_window() -> Rack {
    let mut v = Voice::new(pad_recipe(
        &[
            ("Spread", 0.50),
            ("Sparkle", 0.40),
            ("Chorus", 0.45),
            ("Air", 0.50),
        ],
        vec![
            osc(2.0, 0.0, 0.28).unison(7.0, 8.0),
            osc(2.0, 1.0, 0.0).unison(3.0, 14.0),
        ],
        [3000.0, 0.10, 0.0],
        [250.0, 800.0, 0.85, 900.0],
        0.10,
        [220.0, 800.0, 0.85, 900.0],
        11.0,
    ));
    let d = routed("filter.ladder", "cutoff_hz", 3000.0, 0.5, 0.55);
    v.r.e.set_param(v.lad, "cutoff_hz", d);
    let ch = v.stage(
        "chorus",
        &[("rate_hz", 0.3), ("depth", 55.0), ("mix", 22.0)],
    );
    let eq = v.stage("eq", &[("high_hz", 6000.0), ("high_db", 0.0)]);
    let (o0, o1, mix) = (v.oscs[0], v.oscs[1], v.mix);
    // Spread: a tight stack to a wide, beating one.
    let d = routed("osc.va", "detune", 8.0, 0.5, 0.5);
    let s = span_lh(&mut v, 1, o0, "osc.va", "detune", 2.0, d, 60.0);
    span_hi(&mut v, 1, s, o1, "osc.va", "detune", 14.0, 25.0);
    // Sparkle: the octave of airy saw above.
    span(&mut v, 2, 0.4, mix, "mixer", "level2", 0.0, 0.2);
    comp(&mut v, 2, 0.4, 5.4);
    // Chorus: dry to a deep wash.
    span(&mut v, 3, 0.45, ch, "chorus", "mix", 0.0, 22.0);
    span_hi(&mut v, 3, 0.45, ch, "chorus", "depth", 55.0, 95.0);
    span_hi(&mut v, 3, 0.45, ch, "chorus", "rate_hz", 0.3, 1.0);
    // Air: a top shelf from soft and close to open and glassy (flat at the stored position).
    span_lh(&mut v, 4, eq, "eq", "high_db", -10.0, 0.0, 10.0);
    v.finish("=0.5", &echo(380.0, 30.0, 4000.0), 0.25, (4.0, 0.5), None)
}

/// Drive-In: a lush synthwave pad, detuned saws over a square, slow vibrato, wide chorus.
fn drive_in() -> Rack {
    let mut v = Voice::new(pad_recipe(
        &[
            ("Sway", 0.10),
            ("Chorus", 0.60),
            ("Weight", 0.30),
            ("Pulse", 0.00),
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
    let d = routed("filter.ladder", "cutoff_hz", 1500.0, 0.5, 0.5);
    v.r.e.set_param(v.lad, "cutoff_hz", d);
    let ch = v.stage("chorus", &[("rate_hz", 0.6), ("depth", 70.0), ("mix", 0.0)]);
    // Sway: a slow pitch sway fades in (depths doubled, stored at half).
    v.vibrato("m1", 0.6, 0.5);
    // Chorus: a plain stack to a wide, fast chorus.
    span(&mut v, 2, 0.6, ch, "chorus", "mix", 0.0, 54.0);
    span_hi(&mut v, 2, 0.6, ch, "chorus", "depth", 70.0, 100.0);
    span_hi(&mut v, 2, 0.6, ch, "chorus", "rate_hz", 0.6, 2.2);
    // Weight: the square an octave down comes forward.
    let mix = v.mix;
    let s = span_lh(&mut v, 3, mix, "mixer", "level3", 0.0, 0.15, 0.75);
    comp(&mut v, 3, s, 10.0);
    // Pulse: a trance gate on the host clock, off at 0, up to hard sixteenth-note chops at 1 (the
    // echo and room ring through the gaps). Edges of 3 ms and 12 ms: no clicks. Press Run; with
    // Pulse up and the clock stopped the gate stays shut.
    let clock = v.r.add("clock", &[("bpm", 124.0), ("swing", 0.0)]);
    let gate = v.r.add("env.adsr", &adsr(3.0, 100.0, 1.0, 12.0));
    v.r.wire(clock, "gate", gate, "gate");
    v.r.depth_route(gate, "out", v.mono, "gain_db", v.knob, "m4", 0.5);
    v.r.route(v.knob, "m4", v.mono, "gain_db", -0.5);
    // The echo stays free-running (the clock is patched in for the gate only).
    let mut e = echo(460.0, 40.0, 3200.0).to_vec();
    e.push(("sync", 0.0));
    v.finish("=0.6", &e, 0.35, (5.0, 0.55), Some(clock))
}

/// Dry Cleaner: phased chords through a compressor, close and dry, a filter sweep on macro 1.
fn dry_cleaner() -> Rack {
    let mut c = pad_recipe(
        &[
            ("Sweep", 0.75),
            ("Phase", 0.50),
            ("Grit", 0.35),
            ("Snap", 0.50),
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
        10.0,
    );
    c.drive = [8.0, -4.0];
    let mut v = Voice::new(c);
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
    v.stage("compressor", &compressor(-22.0, 4.0, 6.0, 120.0, 5.0));
    let (lad, drive, aenv) = (v.lad, v.drive, v.aenv);
    // Sweep: a muffled, resonant chord opens to bright.
    let d = routed("filter.ladder", "cutoff_hz", 1200.0, 0.6, 0.45);
    let s = span_lh(
        &mut v,
        1,
        lad,
        "filter.ladder",
        "cutoff_hz",
        1500.0,
        d,
        20000.0,
    );
    span_hi(&mut v, 1, s, lad, "filter.ladder", "resonance", 0.20, 0.08);
    // Phase: a plain chord to a fast, deep, ringing phaser.
    span(&mut v, 2, 0.5, ph, "phaser", "mix", 0.0, 45.0);
    span_hi(&mut v, 2, 0.5, ph, "phaser", "rate_hz", 0.35, 2.2);
    span_hi(&mut v, 2, 0.5, ph, "phaser", "depth", 85.0, 100.0);
    span_hi(&mut v, 2, 0.5, ph, "phaser", "feedback", 55.0, 90.0);
    // Grit: clean to driven hard.
    let d = routed("drive", "drive_db", 8.0, 0.5, 0.35);
    let s = span_lh(&mut v, 3, drive, "drive", "drive_db", 0.0, d, 36.0);
    span_hi(&mut v, 3, s, drive, "drive", "trim_db", -4.0, -14.0);
    comp(&mut v, 3, s, -1.5);
    // Snap: a short tick to a held, ringing chord.
    comp(&mut v, 4, 0.5, 3.1);
    span_hi(&mut v, 4, 0.5, aenv, "env.adsr", "sustain", 0.6, 0.85);
    span_hi(
        &mut v,
        4,
        0.5,
        aenv,
        "env.adsr",
        "release_ms",
        200.0,
        1200.0,
    );
    v.finish("=0.25", &echo(363.0, 25.0, 3000.0), 0.2, (1.2, 0.35), None)
}

/// Concrete Cloud: a thick saturated pad, drive into a hard compressor, a bitcrusher on request.
fn concrete_cloud() -> Rack {
    let mut c = pad_recipe(
        &[
            ("Weight", 0.55),
            ("Squash", 0.50),
            ("Crush", 0.25),
            ("Tail", 0.50),
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
    let d = routed("filter.ladder", "cutoff_hz", 900.0, 0.45, 0.45);
    v.r.e.set_param(v.lad, "cutoff_hz", d);
    let crush = v.stage(
        "bitcrusher",
        &[("bits", 7.0), ("downsample", 3.0), ("mix", 0.0)],
    );
    let cp = v.stage("compressor", &compressor(-24.0, 6.0, 8.0, 200.0, 6.0));
    let (drive, mix, aenv) = (v.drive, v.mix, v.aenv);
    // Weight: saturation and the octave down.
    let d = routed("drive", "drive_db", 16.0, 0.5, 0.55);
    let s = span_lh(&mut v, 1, drive, "drive", "drive_db", 6.0, d, 34.0);
    span_hi(&mut v, 1, s, mix, "mixer", "level2", 0.30, 0.40);
    comp(&mut v, 1, s, 2.8);
    // Squash: the compressor clamps from a loose hold to a flat slab.
    let s = span_lh(
        &mut v,
        2,
        cp,
        "compressor",
        "threshold_db",
        -14.0,
        -24.0,
        -34.0,
    );
    span_hi(&mut v, 2, s, cp, "compressor", "ratio", 6.0, 10.0);
    span_hi(&mut v, 2, s, cp, "compressor", "makeup_db", 6.0, 11.5);
    comp(&mut v, 2, s, -11.8);
    // Crush: clean to a coarse digital grind.
    span(&mut v, 3, 0.25, crush, "bitcrusher", "mix", 0.0, 20.0);
    span_hi(&mut v, 3, 0.25, crush, "bitcrusher", "bits", 7.0, 4.0);
    span_hi(&mut v, 3, 0.25, crush, "bitcrusher", "downsample", 3.0, 8.0);
    // Tail: struck and cut at 0, a slow swell that hangs at 1.
    let s = span_lh(
        &mut v,
        4,
        aenv,
        "env.adsr",
        "release_ms",
        100.0,
        500.0,
        6000.0,
    );
    span_hi(&mut v, 4, s, aenv, "env.adsr", "attack_ms", 60.0, 900.0);
    v.finish("=0.25", &echo(300.0, 20.0, 2800.0), 0.2, (1.5, 0.35), None)
}

/// Static Garden: a triangle bed with a noise layer, a ring modulated layer and a wandering filter.
fn static_garden() -> Rack {
    let mut v = Voice::new(pad_recipe(
        &[
            ("Static", 0.50),
            ("Wander", 0.00),
            ("Metal", 0.50),
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
        7.5,
    ));
    let noise = v.r.add("noise", &[("color", 1.0), ("level_db", -8.0)]);
    v.r.wire(noise, "out", v.mix, "in3");
    let partner = v.r.add(
        "osc.va",
        &[("base_hz", 392.0), ("waveform", 0.0), ("fine", 3.0)],
    );
    v.r.wire(v.m, "pitch", partner, "pitch");
    let ring = v.r.add("ringmod", &[]);
    v.r.wire(v.oscs[0], "out", ring, "a");
    v.r.wire(partner, "out", ring, "b");
    v.r.wire(ring, "out", v.mix, "in4");
    let mix = v.mix;
    // Static: the hiss comes in.
    span(&mut v, 1, 0.5, mix, "mixer", "level3", 0.0, 0.16);
    // The filter walk stays at what the old Motion left (stored 0.5 of 0.4 and 0.2).
    let walk1 = v.r.add("lfo", &[("rate_hz", 0.11)]);
    let walk2 = v.r.add("lfo", &[("rate_hz", 0.29)]);
    v.r.route(walk1, "out", v.lad, "cutoff_hz", 0.2);
    v.r.route(walk2, "out", v.lad, "cutoff_hz", 0.1);
    // Wander: the pitch starts to drift on its own, two slow waves that never line up (off by
    // default).
    let sway1 = v.r.add("lfo", &[("rate_hz", 0.17)]);
    let sway2 = v.r.add("lfo", &[("rate_hz", 0.43)]);
    for o in v.oscs.clone() {
        v.r.depth_route(sway1, "out", o, "fine", v.knob, "m2", 0.14);
        v.r.depth_route(sway2, "out", o, "fine", v.knob, "m2", 0.07);
    }
    // Metal: bell-like overtones come in and the partner detunes.
    span(&mut v, 3, 0.5, mix, "mixer", "level4", 0.0, 0.15);
    comp(&mut v, 3, 0.5, 0.6);
    span_hi(&mut v, 3, 0.5, partner, "osc.va", "fine", 3.0, 40.0);
    v.stage(
        "chorus",
        &[("rate_hz", 0.2), ("depth", 50.0), ("mix", 18.0)],
    );
    v.finish("m4", &echo(620.0, 45.0, 2800.0), 0.3, (7.0, 0.55), None)
}

// ---- group 2 --------------------------------------------------------------------------------

/// Glacier: a glassy, slow pad: a wide sine and triangle stack with a high glass partial, a slow
/// swell and a long echo.
fn glacier() -> Rack {
    let mut v = Voice::new(pad_recipe(
        &[
            ("Frost", 0.45),
            ("Width", 0.70),
            ("Swell", 0.50),
            ("Space", 0.70),
        ],
        vec![
            osc(0.0, 0.0, 0.30).unison(4.0, 12.0).wide(70.0),
            osc(1.0, 0.0, 0.20).fine(5.0),
            osc(0.0, 2.0, 0.0),
        ],
        [1600.0, 0.10, 0.0],
        [1500.0, 2500.0, 0.8, 3000.0],
        0.0,
        [1800.0, 1500.0, 0.9, 3500.0],
        12.0,
    ));
    let d = routed("filter.ladder", "cutoff_hz", 1600.0, 0.35, 0.45);
    v.r.e.set_param(v.lad, "cutoff_hz", d);
    fixed_vibrato(&mut v, 0.35, 0.03);
    v.stage(
        "chorus",
        &[("rate_hz", 0.25), ("depth", 45.0), ("mix", 16.0)],
    );
    let (o0, mix, aenv) = (v.oscs[0], v.mix, v.aenv);
    // Frost: the glass partial two octaves up comes in.
    span(&mut v, 1, 0.45, mix, "mixer", "level3", 0.0, 0.1575);
    comp(&mut v, 1, 0.45, 3.0);
    // Width: the stack from narrow to wide.
    span_lh(&mut v, 2, o0, "osc.va", "width", 0.0, 70.0, 100.0);
    // Swell: chords arrive and die slowly.
    let s = span_lh(
        &mut v,
        3,
        aenv,
        "env.adsr",
        "attack_ms",
        300.0,
        1800.0,
        6000.0,
    );
    span_hi(&mut v, 3, s, aenv, "env.adsr", "release_ms", 3500.0, 9000.0);
    v.finish(
        "m4",
        &echo(700.0, 48.0, 3200.0),
        0.30,
        (8.0, 0.55 * 0.6 / 0.7),
        None,
    )
}

/// Rooftop: a brassy analog pad, detuned saws with a slow filter envelope that swells every chord.
fn rooftop() -> Rack {
    let mut v = Voice::new(pad_recipe(
        &[
            ("Swell", 0.55),
            ("Section", 0.50),
            ("Octave", 0.00),
            ("Chorus", 0.50),
        ],
        vec![
            osc(2.0, 0.0, 0.28).unison(3.0, 14.0).wide(60.0),
            osc(2.0, 0.0, 0.22).fine(-7.0),
            osc(2.0, 1.0, 0.0).unison(3.0, 14.0),
        ],
        [1000.0, 0.18, 0.0],
        [650.0, 900.0, 0.45, 700.0],
        0.10,
        [180.0, 700.0, 0.85, 800.0],
        11.0,
    ));
    let d = routed("filter.ladder", "cutoff_hz", 1000.0, 0.5, 0.45);
    v.r.e.set_param(v.lad, "cutoff_hz", d);
    let ch = v.stage("chorus", &[("rate_hz", 0.5), ("depth", 60.0), ("mix", 0.0)]);
    let (fenv, o0, o1) = (v.fenv, v.oscs[0], v.oscs[1]);
    // Swell: each chord leans into the filter, further and more slowly.
    let s = span_lh(
        &mut v,
        1,
        fenv,
        "env.adsr",
        "attack_ms",
        150.0,
        650.0,
        3000.0,
    );
    v.r.depth_route(v.fenv, "out", v.lad, "cutoff_hz", v.knob, "m1", 0.33 / s);
    let aenv = v.aenv;
    span_hi(&mut v, 1, s, aenv, "env.adsr", "attack_ms", 180.0, 1400.0);
    // Section: one tight horn to a loose section.
    let s = span_lh(&mut v, 2, o0, "osc.va", "detune", 3.0, 14.0, 50.0);
    span_hi(&mut v, 2, s, o1, "osc.va", "fine", -7.0, -40.0);
    // Octave: a second section an octave up joins, the brass gets a bright, soaring top.
    let mix = v.mix;
    rise(&mut v, 3, mix, "mixer", "level3", 0.0, 0.2);
    comp(&mut v, 3, 0.0, 1.3);
    // Chorus: dry to a wide wash.
    span(&mut v, 4, 0.5, ch, "chorus", "mix", 0.0, 40.0);
    span_hi(&mut v, 4, 0.5, ch, "chorus", "depth", 60.0, 100.0);
    span_hi(&mut v, 4, 0.5, ch, "chorus", "rate_hz", 0.5, 2.0);
    v.finish("=0.45", &echo(340.0, 30.0, 3500.0), 0.25, (3.5, 0.45), None)
}

/// Tidepool: a glassy pad that never repeats, slow random steps in pitch and filter.
fn tidepool() -> Rack {
    let mut v = Voice::new(pad_recipe(
        &[
            ("Drift", 0.45),
            ("Steps", 0.00),
            ("Tone", 0.50),
            ("Space", 0.60),
        ],
        vec![
            osc(1.0, 0.0, 0.32).unison(3.0, 18.0).wide(80.0),
            osc(0.0, 1.0, 0.12).fine(3.0),
        ],
        [1400.0, 0.14, 0.0],
        [1500.0, 2000.0, 0.6, 2500.0],
        0.12,
        [1200.0, 1500.0, 0.9, 3000.0],
        11.0,
    ));
    v.vibrato("m1", 0.18, 0.25);
    // The old filter steps stay, fixed at what Motion left (stored 0.55).
    let step1 = v.r.add("lfo", &[("rate_hz", 0.13), ("waveform", 4.0)]);
    let step2 = v.r.add("lfo", &[("rate_hz", 0.07), ("waveform", 1.0)]);
    v.r.route(step1, "out", v.lad, "cutoff_hz", 0.45 * 0.55);
    v.r.route(step2, "out", v.lad, "cutoff_hz", 0.25 * 0.55);
    // Steps: random jumps of pitch, from still to a burbling run.
    let hop = v.r.add("lfo", &[("rate_hz", 0.35), ("waveform", 4.0)]);
    for o in v.oscs.clone() {
        v.r.depth_route(hop, "out", o, "fine", v.knob, "m2", 0.12);
    }
    rise(&mut v, 2, hop, "lfo", "rate_hz", 0.35, 1.6);
    // Tone: dark and plain to bright with the octave sine.
    let d = routed("filter.ladder", "cutoff_hz", 1400.0, 0.5, 0.5);
    let (lad, mix) = (v.lad, v.mix);
    let s = span_lh(
        &mut v,
        3,
        lad,
        "filter.ladder",
        "cutoff_hz",
        400.0,
        d,
        20000.0,
    );
    span(&mut v, 3, s, mix, "mixer", "level2", 0.0, 0.12);
    comp(&mut v, 3, s, 2.2);
    v.stage(
        "chorus",
        &[("rate_hz", 0.15), ("depth", 55.0), ("mix", 20.0)],
    );
    v.finish("m4", &echo(540.0, 42.0, 3000.0), 0.35, (6.5, 0.55), None)
}

/// Pump House: a chord pad that ducks on every beat of the host clock: a silent envelope, no drum,
/// dips the pad's gain like a sidechain would. Depth on macro 1; inert until Run starts the clock.
fn pump_house() -> Rack {
    let mut v = Voice::new(pad_recipe(
        &[
            ("Pump", 0.35),
            ("Weight", 0.40),
            ("Wide", 0.30),
            ("Open", 0.50),
        ],
        vec![osc(2.0, 0.0, 0.30).unison(3.0, 12.0), osc(2.0, -1.0, 0.25)],
        [1600.0, 0.15, 0.0],
        [200.0, 700.0, 0.7, 500.0],
        0.12,
        [60.0, 400.0, 0.9, 400.0],
        9.0,
    ));
    let clock = v.r.add("clock", &[("bpm", 124.0), ("swing", 0.0)]);
    let beat = v.r.add("clock.div", &[("div", 4.0)]);
    v.r.wire(clock, "gate", beat, "clock");
    v.r.wire(clock, "reset", beat, "reset");
    let duck = v.r.add("env.adsr", &adsr(1.0, 280.0, 0.0, 50.0));
    v.r.wire(beat, "gate", duck, "gate");
    // The envelope pulls the voice's gain stage down (84 dB travel: 0.30 is about 25 dB).
    let gain = v.mono;
    v.r.depth_route(duck, "out", gain, "gain_db", v.knob, "m1", -0.60);
    span_hi(&mut v, 1, 0.35, duck, "env.adsr", "decay_ms", 280.0, 800.0);
    let ch = v.stage(
        "chorus",
        &[("rate_hz", 0.3), ("depth", 40.0), ("mix", 14.0)],
    );
    let (mix, lad) = (v.mix, v.lad);
    // The old Drive stays where it left the saturation (stored 0.35 of 0.2).
    let d = routed("drive", "drive_db", 0.0, 0.2, 0.35);
    v.r.e.set_param(v.drive, "drive_db", d);
    // Weight: the octave down comes forward and the saws step back.
    let s = span_lh(&mut v, 2, mix, "mixer", "level2", 0.05, 0.25, 0.5);
    span_hi(&mut v, 2, s, mix, "mixer", "level1", 0.30, 0.15);
    comp(&mut v, 2, s, 2.7);
    // Wide: a narrow pad to a deep, slow chorus that moves around the duck.
    span_hi(&mut v, 3, 0.3, ch, "chorus", "mix", 14.0, 38.0);
    span_hi(&mut v, 3, 0.3, ch, "chorus", "depth", 40.0, 90.0);
    span_hi(&mut v, 3, 0.3, ch, "chorus", "rate_hz", 0.3, 1.2);
    // Open: the filter, muffled to bright.
    let d = routed("filter.ladder", "cutoff_hz", 1600.0, 0.5, 0.55);
    span_lh(
        &mut v,
        4,
        lad,
        "filter.ladder",
        "cutoff_hz",
        500.0,
        d,
        20000.0,
    );
    v.finish(
        "=0.3",
        &[
            ("sync", 3.0),
            ("time_ms", 360.0),
            ("feedback", 30.0),
            ("mix", 0.0),
            ("tone_hz", 3500.0),
        ],
        0.2,
        (3.0, 0.35),
        Some(clock),
    )
}

/// Tin Voice: robot vowel chords, saws and a pulse driven through a formant filter, dry.
fn tin_voice() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Vowel", 0.35),
            ("Chatter", 0.00),
            ("Shift", 0.58),
            ("Grit", 0.45),
        ],
        midi: [0.0, 0.0, 120.0],
        oscs: vec![
            osc(2.0, 0.0, 0.30).unison(3.0, 10.0),
            osc(3.0, 0.0, 0.20).pw(40.0),
        ],
        drive: [10.0, -5.0],
        lad: [3500.0, 0.10, 0.0],
        fenv: [2.0, 100.0, 0.5, 60.0],
        fenv_amt: 0.0,
        wheel_amt: 0.0,
        aenv: [8.0, 300.0, 0.85, 200.0],
        trim_db: 26.0,
    });
    let formant = v.mono_after(
        "filter.formant",
        &[
            ("vowel", 0.0),
            ("shift_st", -4.0),
            ("q", 1.5),
            ("hold", 1.0),
        ],
    );
    v.r.route(v.knob, "m1", formant, "vowel", 0.9);
    v.r.route(v.m, "wheel", formant, "vowel", 0.5);
    let talk = v.r.add("lfo", &[("rate_hz", 0.9)]);
    v.r.route(talk, "out", formant, "vowel", 0.08);
    // Chatter: the mouth jumps to a new vowel at random, faster and wider.
    let hop = v.r.add("lfo", &[("rate_hz", 0.9), ("waveform", 4.0)]);
    v.r.depth_route(hop, "out", formant, "vowel", v.knob, "m2", 0.35);
    rise(&mut v, 2, hop, "lfo", "rate_hz", 0.9, 7.0);
    // Shift: a small mouth to a large one.
    let d = routed("filter.formant", "shift_st", -4.0, 0.5, 0.5);
    let shift_s = span_lh(
        &mut v,
        3,
        formant,
        "filter.formant",
        "shift_st",
        -12.0,
        d,
        12.0,
    );
    // Grit: clean and round to driven and sharp.
    let drive = v.drive;
    let d = routed("drive", "drive_db", 10.0, 0.3, 0.45);
    let s = span_lh(&mut v, 4, drive, "drive", "drive_db", 2.0, d, 30.0);
    span_hi(&mut v, 4, s, formant, "filter.formant", "q", 1.5, 3.0);
    span_hi(&mut v, 4, s, drive, "drive", "trim_db", -5.0, -12.0);
    // The crusher after the formant: its mix, bit depth and sample-rate drop come in with Grit.
    let crush = v.stage(
        "bitcrusher",
        &[("bits", 10.0), ("downsample", 3.0), ("mix", 0.0)],
    );
    span(&mut v, 4, s, crush, "bitcrusher", "mix", 0.0, 25.0);
    span_hi(&mut v, 4, s, crush, "bitcrusher", "bits", 10.0, 4.0);
    span_hi(&mut v, 4, s, crush, "bitcrusher", "downsample", 3.0, 8.0);
    let cp = v.stage("compressor", &compressor(-26.0, 5.0, 3.0, 120.0, 6.0));
    // Shift: a bigger mouth is quieter, so the makeup follows (stored sound unchanged).
    span(
        &mut v,
        3,
        shift_s,
        cp,
        "compressor",
        "makeup_db",
        6.0 - 3.4 * shift_s,
        6.0,
    );
    v.finish("=0.25", &echo(180.0, 25.0, 3200.0), 0.25, (0.8, 0.2), None)
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
        Preset {
            slug: "glacier",
            name: "Glacier",
            category: "Pad",
            tags: &["poly", "slow", "wide", "ambient", "cinematic"],
            description: "A glassy, slow pad: a wide sine and triangle stack with a high glass partial, a slow swell and a long echo. Play long, open chords. Frost opens the filter and brings in the glass partial, Swell opens the filter over each held chord, Drift adds a slow pitch sway, Space adds the echo and the room.",
            listen: "A cold, wide shimmer that opens over each held chord; Frost up for the glassy octaves, Swell for the slow opening.",
            build: glacier,
            group: 2,
            phrase: slow_phrase,
            secs: 26.0,
        },
        Preset {
            slug: "pump-house",
            name: "Pump House",
            category: "Pad",
            tags: &["poly", "sidechain", "124bpm", "french touch", "signature"],
            description: "A chord pad that pumps with the beat for French touch: saws and an octave down, ducked on every beat of the host tempo by a silent envelope, no drum. Press Run to start the clock, then hold chords. Cutoff opens the filter, Drive pushes the saturation, Pump sets how deep the duck goes, Room adds a tempo-synced echo and a tight room.",
            listen: "The chord breathing on every beat (silent, no drum); Pump at 0 is a steady pad, at 1 it ducks deep on every beat.",
            build: pump_house,
            group: 2,
            phrase: pop_phrase,
            secs: 14.0,
        },
        Preset {
            slug: "tin-voice",
            name: "Tin Voice",
            category: "Pad",
            tags: &["poly", "formant", "distorted", "robot", "french touch", "signature"],
            description: "Robot vowel chords for French touch: saws and a pulse driven into a formant filter, close and dry. Play held chords and move the mod wheel. Vowel sweeps the mouth shape, Grit pushes the drive, Shift moves the formants up or down, Room adds a short echo and a small room.",
            listen: "A chord that talks; Vowel moves through the shapes, Shift turns the voice small or large, Grit adds bite.",
            build: tin_voice,
            group: 2,
            phrase: pop_phrase,
            secs: 14.0,
        },
        Preset {
            slug: "rooftop",
            name: "Rooftop",
            category: "Pad",
            tags: &["poly", "analog", "wide", "brass", "pop"],
            description: "A brassy analog pad: detuned saws with a slow filter envelope that swells every chord. Play open triads and hold them. Cutoff sets the filter, Swell sets how far each chord opens, Chorus widens the sound, Space adds the echo and the room.",
            listen: "Each chord leaning in like a brass section; Swell at 0 is a steady pad, at 1 each chord blooms open.",
            build: rooftop,
            group: 2,
            phrase: pop_phrase,
            secs: 14.0,
        },
        Preset {
            slug: "tidepool",
            name: "Tidepool",
            category: "Pad",
            tags: &["poly", "evolving", "wide", "ambient", "glass"],
            description: "An evolving glassy pad that never repeats: slow random steps in pitch and filter over a wide triangle stack. Play slow chords and let it move. Drift sets the pitch sway, Motion sets how far the filter wanders, Tone sets the brightness, Space adds the echo and the room.",
            listen: "Held chords that keep shifting colour; Motion up for a filter that never settles, Drift for a gentle pitch sway.",
            build: tidepool,
            group: 2,
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
            "usage: presets_pad write|clips|levels|macros|library <patches dir> [out dir] [group]"
        ),
    }
}
