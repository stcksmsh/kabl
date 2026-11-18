//! The factory basses and leads (docs/presets/BRIEF.md, first part of the bank): builds the
//! patches from the existing modules, writes them under `patches/presets/`, renders a clip per
//! preset by playing a short MIDI phrase through the real engine, and prints the level and the
//! library entries.
//!
//! ```sh
//! cargo run -p kabl-tools --example presets_bass_lead -- write patches
//! cargo run --release -p kabl-tools --example presets_bass_lead -- clips patches out_dir [group]
//! cargo run --release -p kabl-tools --example presets_bass_lead -- levels patches
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
    if mac.is_empty() {
        // A fixed wet amount: no macro drives the space.
        r.e.set_param(delay, "mix", echo_max);
        r.e.set_param(verb, "mix", room_max);
    } else {
        r.route(knob, mac, delay, "mix", echo_max);
        r.route(knob, mac, verb, "mix", room_max);
    }
    verb
}

fn rubber_phrase() -> Vec<Hit> {
    let bar = [
        (0, 40, 1.0),
        (2, 40, 0.6),
        (3, 43, 1.0),
        (6, 45, 1.0),
        (8, 40, 1.0),
        (10, 47, 0.7),
        (11, 45, 1.0),
        (14, 43, 1.0),
    ];
    steps(&bars(&bar, &[&[], &[(3, 47)], &[], &[(1, 52), (7, 38)]]))
}

fn slate_phrase() -> Vec<Hit> {
    steps(&[
        (0, 33, 7.0),
        (8, 33, 3.0),
        (12, 36, 3.0),
        (16, 31, 7.0),
        (24, 31, 3.0),
        (28, 34, 3.0),
        (32, 33, 7.0),
        (40, 36, 3.0),
        (44, 33, 3.0),
        (48, 29, 14.0),
    ])
}

fn gutter_phrase() -> Vec<Hit> {
    let bar = [
        (0, 40, 1.0),
        (1, 40, 1.0),
        (3, 40, 1.0),
        (4, 52, 1.0),
        (6, 40, 1.0),
        (8, 40, 1.0),
        (9, 40, 1.0),
        (11, 43, 1.0),
        (12, 40, 1.0),
        (14, 52, 1.0),
    ];
    steps(&bars(&bar, &[&[], &[(3, 43)], &[], &[(3, 38), (9, 38)]]))
}

fn tide_phrase() -> Vec<Hit> {
    line(&[(0.0, 36, 5.0), (4.8, 41, 5.0), (9.6, 34, 6.0)])
}

fn copper_phrase() -> Vec<Hit> {
    steps(&bars(
        &[(0, 40, 4.0), (6, 43, 2.0), (8, 46, 4.0), (14, 40, 2.0)],
        &[&[], &[(1, 45)], &[], &[(0, 38), (2, 41)]],
    ))
}

fn tail_phrase() -> Vec<Hit> {
    let bar: Vec<(u32, u8, f32)> = (0..8).map(|i| (i * 2, 40, 1.6)).collect();
    steps(&bars(
        &bar,
        &[
            &[],
            &[
                (0, 43),
                (1, 43),
                (2, 43),
                (3, 43),
                (4, 43),
                (5, 43),
                (6, 43),
                (7, 43),
            ],
            &[],
            &[
                (0, 38),
                (1, 38),
                (2, 38),
                (3, 38),
                (4, 38),
                (5, 38),
                (6, 38),
                (7, 38),
            ],
        ],
    ))
}

fn rail_phrase() -> Vec<Hit> {
    // Off-beat notes over the duck, which Run starts at 0.5 s.
    let bar = [(2, 40, 1.5), (6, 40, 1.5), (10, 40, 1.5), (14, 43, 1.5)];
    later(0.5, steps(&bars(&bar, &[&[], &[(3, 45)], &[], &[(3, 47)]])))
}

fn oak_phrase() -> Vec<Hit> {
    line(&[
        (0.0, 40, 0.8),
        (0.9, 43, 0.8),
        (1.8, 45, 0.8),
        (2.7, 47, 0.8),
        (3.6, 45, 0.8),
        (4.5, 43, 0.8),
        (5.4, 40, 1.4),
        (7.0, 38, 0.8),
        (7.9, 43, 1.6),
    ])
}

fn needle_phrase() -> Vec<Hit> {
    line(&[
        (0.0, 79, 0.8),
        (0.9, 83, 0.6),
        (1.6, 86, 1.4),
        (3.2, 83, 0.6),
        (3.9, 79, 0.6),
        (4.6, 76, 1.6),
        (6.4, 81, 0.6),
        (7.1, 84, 2.0),
    ])
}

fn mouth_phrase() -> Vec<Hit> {
    steps(&bars(
        &[
            (0, 60, 1.0),
            (2, 63, 1.0),
            (3, 60, 1.0),
            (6, 67, 1.0),
            (8, 65, 1.0),
            (10, 63, 1.0),
            (12, 60, 1.0),
            (14, 58, 1.0),
        ],
        &[&[], &[(5, 62), (7, 55)], &[], &[(3, 65)]],
    ))
}

fn devil_phrase() -> Vec<Hit> {
    line(&[
        (0.0, 72, 1.1),
        (1.0, 76, 1.1),
        (2.0, 74, 0.6),
        (2.5, 79, 1.6),
        (4.0, 76, 1.1),
        (5.0, 72, 1.1),
        (6.0, 67, 2.4),
    ])
}

fn afterglow_phrase() -> Vec<Hit> {
    line(&[
        (0.0, 64, 2.2),
        (2.0, 71, 2.2),
        (4.0, 69, 1.2),
        (5.0, 67, 1.2),
        (6.0, 64, 3.0),
    ])
}

fn horn_phrase() -> Vec<Hit> {
    steps(&bars(
        &[
            (0, 67, 1.5),
            (3, 67, 1.0),
            (4, 70, 1.5),
            (7, 72, 1.0),
            (8, 74, 1.5),
            (11, 72, 1.0),
            (12, 70, 1.5),
            (14, 67, 1.0),
        ],
        &[&[], &[(7, 74)], &[], &[(7, 65), (6, 62)]],
    ))
}

fn fire_phrase() -> Vec<Hit> {
    steps(&bars(
        &[
            (0, 64, 1.5),
            (3, 67, 1.5),
            (6, 64, 1.0),
            (8, 71, 1.5),
            (11, 67, 1.5),
            (14, 64, 1.0),
        ],
        &[&[], &[(4, 69)], &[], &[(0, 62), (1, 65)]],
    ))
}

fn circuit_phrase() -> Vec<Hit> {
    steps(&bars(
        &[
            (0, 64, 1.0),
            (1, 67, 1.0),
            (3, 71, 1.0),
            (6, 64, 1.0),
            (8, 76, 1.0),
            (10, 74, 1.0),
            (12, 71, 1.0),
            (14, 67, 1.0),
        ],
        &[&[], &[(4, 79)], &[], &[(4, 72), (7, 64)]],
    ))
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
    aenv: ModuleId,
    mono: ModuleId,
    cur: Option<ModuleId>,
    n: usize,
    /// The filter envelope's route into the cutoff (its amount is the recipe's `fenv_amt`).
    fenv_cable: kabl_core::CableId,
    /// The recipe's `fenv_amt`, for `snap`.
    fenv_amt: f32,
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
        let fenv_cable = r.e.connect_route(jack(fenv, "out"), lad, "cutoff_hz");
        r.e.set_route_amount(fenv_cable, c.fenv_amt, true);
        if c.wheel_amt > 0.0 {
            r.route(m, "wheel", lad, "cutoff_hz", c.wheel_amt);
        }
        Voice {
            fenv_cable,
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
            fenv_amt: c.fenv_amt,
        }
    }

    /// Macro `m` (stored at `stored`) scales how far the filter envelope sweeps the cutoff, by
    /// `depth` of the knob's travel from 0 to 1; the recipe's `fenv_amt` is what is left at the
    /// stored position, so the stored sound is unchanged.
    fn snap(&mut self, m: usize, stored: f32, depth: f32) {
        let base = self.fenv_amt - stored * depth;
        assert!((-1.0..=1.0).contains(&base) && (-1.0..=1.0).contains(&(base + depth)));
        self.r.e.set_route_amount(self.fenv_cable, base, true);
        let d = self
            .r
            .e
            .connect_cable_route(jack(self.knob, &format!("m{m}")), self.fenv_cable, "amount")
            .expect("a route has a depth");
        self.r.e.set_route_amount(d, depth, true);
    }

    /// As `depth_route`, for a source whose stored depth is `at_stored`: macro `m` scales the
    /// route from `at_stored - stored * depth` at 0 to that plus `depth` at 1.
    #[allow(clippy::too_many_arguments)]
    fn depth_keep(
        &mut self,
        a: ModuleId,
        ap: &str,
        b: ModuleId,
        param: &str,
        m: usize,
        stored: f32,
        depth: f32,
        at_stored: f32,
    ) {
        let base = at_stored - stored * depth;
        assert!((-1.0..=1.0).contains(&base) && (-1.0..=1.0).contains(&(base + depth)));
        let cable = self.r.e.connect_route(jack(a, ap), b, param);
        self.r.e.set_route_amount(cable, base, true);
        let d = self
            .r
            .e
            .connect_cable_route(jack(self.knob, &format!("m{m}")), cable, "amount")
            .expect("a route has a depth");
        self.r.e.set_route_amount(d, depth, true);
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
    fn vibrato(&mut self, mac: &str, rate_hz: f32, depth: f32) -> ModuleId {
        let lfo = self.r.add("lfo", &[("rate_hz", rate_hz)]);
        for o in self.oscs.clone() {
            self.r
                .depth_route(lfo, "out", o, "fine", self.knob, mac, depth);
        }
        lfo
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

fn param_of(kind: &str, param: &str) -> kabl_modules::ParamInfo {
    *kabl_modules::registry::info_for(kind)
        .and_then(|i| i.params.iter().find(|p| p.name == param))
        .unwrap_or_else(|| panic!("{kind}.{param}"))
}

/// What an old route of `amount` from a macro at `stored` leaves on a param with base `base`.
fn routed(kind: &str, param: &str, base: f32, amount: f32, stored: f32) -> f32 {
    let i = param_of(kind, param);
    i.from_norm(i.to_norm(base) + stored * amount)
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
    assert!(
        n0 + amount <= 1.0 + 1e-3 && n0 + amount >= -1e-3,
        "{kind}.{param}: macro {m} runs out of range ({})",
        n0 + amount
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
    assert!(
        (-1e-3..=1.0 + 1e-3).contains(&n0),
        "{kind}.{param}: macro {m} starts out of range ({n0})"
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

// ---- group 1 --------------------------------------------------------------------------------

/// A silent beat duck from the host clock: `clock.div` into a fast-decay `env.adsr` that pulls the
/// voice's gain stage down, depth on macro 3. No drum is built in; it stays still until Run.
fn clock_duck(v: &mut Voice, clock: ModuleId, decay_ms: f32, depth: f32, mac: &str) -> ModuleId {
    let beat = v.r.add("clock.div", &[("div", 4.0)]);
    v.r.wire(clock, "gate", beat, "clock");
    v.r.wire(clock, "reset", beat, "reset");
    let env = v.r.add("env.adsr", &adsr(3.0, decay_ms, 0.0, 50.0));
    v.r.wire(beat, "gate", env, "gate");
    v.r.depth_route(env, "out", v.mono, "gain_db", v.knob, mac, depth);
    env
}

/// Steel Toe: a blunt compressed bass that ducks on every beat of the host clock.
fn steel_toe() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Bite", 0.45),
            ("Weight", 0.60),
            ("Squash", 0.50),
            ("Pump", 0.30),
        ],
        midi: [1.0, 0.0, 120.0],
        oscs: vec![osc(2.0, 0.0, 0.55).unison(2.0, 8.0), osc(3.0, -1.0, 0.5)],
        drive: [18.0, -9.0],
        lad: [300.0, 0.28, 8.0],
        fenv: [1.0, 140.0, 0.12, 70.0],
        fenv_amt: 0.22,
        wheel_amt: 0.25,
        aenv: [6.0, 180.0, 0.8, 45.0],
        trim_db: 11.0,
    });
    let (lad, mix) = (v.lad, v.mix);
    // Bite: the filter opens and each note snaps harder (tone and articulation).
    let cut = routed("filter.ladder", "cutoff_hz", 300.0, 0.5, 0.45);
    span_hi(
        &mut v,
        1,
        0.45,
        lad,
        "filter.ladder",
        "cutoff_hz",
        cut,
        5000.0,
    );
    v.snap(1, 0.45, 0.15);
    // Weight: the square sub grows and the saw gives way (the low-end balance).
    span(&mut v, 2, 0.6, mix, "mixer", "level2", 0.15, 0.5);
    span(&mut v, 2, 0.6, mix, "mixer", "level1", 0.75, 0.55);
    // The old Grit stays where it was stored.
    let drive = routed("drive", "drive_db", 18.0, 0.35, 0.6);
    v.r.e.set_param(v.drive, "drive_db", drive);
    let comp = v.stage("compressor", &compressor(-26.0, 6.0, 2.0, 90.0, 13.0));
    // Squash: the compressor goes from a slow, punchy squeeze to a fast, flat clamp (attack,
    // ratio and threshold together, the makeup follows): density rather than a level drop.
    span_hi(
        &mut v,
        3,
        0.5,
        comp,
        "compressor",
        "threshold_db",
        -26.0,
        -34.0,
    );
    span(&mut v, 3, 0.5, comp, "compressor", "makeup_db", 6.0, 13.0);
    span(&mut v, 3, 0.5, comp, "compressor", "ratio", 2.0, 6.0);
    span(&mut v, 3, 0.5, comp, "compressor", "attack_ms", 25.0, 2.0);
    let clock = v.r.add("clock", &[("bpm", 124.0), ("swing", 25.0)]);
    // Pump: the beat ducks the bass, deeper, and the bass takes the whole beat to swell back (the
    // release of the duck envelope: the duck is a short gate, the breathing is its release).
    let env = clock_duck(&mut v, clock, 200.0, -0.60, "m4");
    span_hi(&mut v, 4, 0.30, env, "env.adsr", "release_ms", 50.0, 170.0);
    v.finish(
        "",
        &[
            ("sync", 3.0),
            ("time_ms", 360.0),
            ("feedback", 30.0),
            ("mix", 0.0),
            ("tone_hz", 3500.0),
        ],
        0.3 * 0.22 * 100.0,
        (0.45, 0.3 * 0.16 * 100.0),
        Some(clock),
    )
}

/// Tar Pit: a trap 808, a sine with a pitch snap and a long body, sliding between notes.
fn tar_pit() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Thump", 0.30),
            ("Body", 0.50),
            ("Dirt", 0.40),
            ("Slide", 0.80),
        ],
        midi: [1.0, 1.0, 80.0],
        oscs: vec![osc(0.0, -1.0, 0.7), osc(1.0, -1.0, 0.15)],
        drive: [6.0, -3.0],
        lad: [400.0, 0.05, 0.0],
        fenv: [3.0, 40.0, 0.0, 25.0],
        fenv_amt: 0.12,
        wheel_amt: 0.0,
        // A 6 ms attack: no click at the start of the note by default (Thump brings it in).
        aenv: [6.0, 2200.0, 0.75, 260.0],
        trim_db: 13.0,
    });
    let (fenv, aenv, mix, drive, m, o0) = (v.fenv, v.aenv, v.mix, v.drive, v.m, v.oscs[0]);
    // The cutoff stays where the old Tone was stored.
    let cut = routed("filter.ladder", "cutoff_hz", 400.0, 0.5, 0.5);
    v.r.e.set_param(v.lad, "cutoff_hz", cut);
    // Thump: the fast envelope kicks the pitch up and lets it fall (how far, how long), and the
    // note starts with a harder attack: a soft swell at 0, a click-and-punch at 1.
    v.depth_keep(fenv, "out", o0, "fine", 1, 0.3, 0.45, 0.5);
    span(&mut v, 1, 0.3, fenv, "env.adsr", "decay_ms", 15.0, 40.0);
    span_hi(&mut v, 1, 0.3, aenv, "env.adsr", "attack_ms", 6.0, 3.0);
    // Body: the round tail gets longer and the triangle under the sine fills it.
    span(&mut v, 2, 0.5, aenv, "env.adsr", "decay_ms", 550.0, 2200.0);
    span(&mut v, 2, 0.5, aenv, "env.adsr", "release_ms", 120.0, 260.0);
    v.r.e.set_param(mix, "level2", 0.3);
    // Dirt: from a clean sine to a gritty one that speaks on small speakers (default: warm).
    span_hi(&mut v, 3, 0.4, drive, "drive", "drive_db", 12.0, 24.0);
    // Slide: how long the glide between notes takes.
    let glide = routed("midi.in", "glide_ms", 80.0, 0.8, 0.5);
    span(&mut v, 4, 0.8, m, "midi.in", "glide_ms", 40.0, glide);
    v.stage("compressor", &compressor(-24.0, 3.0, 8.0, 120.0, 3.0));
    v.finish(
        "",
        &echo(90.0, 15.0, 3000.0),
        0.2 * 0.12 * 100.0,
        (0.5, 0.2 * 0.14 * 100.0),
        None,
    )
}

/// Acid Rain: a legato acid bass, a saw over a square sub, a resonant ladder with accents.
fn acid_rain() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Squelch", 0.50),
            ("Accent", 0.60),
            ("Slide", 0.40),
            ("Space", 0.70),
        ],
        midi: [2.0, 2.0, 55.0],
        oscs: vec![osc(2.0, 0.0, 0.6), osc(3.0, -1.0, 0.3)],
        drive: [7.0, -3.5],
        lad: [220.0, 0.28, 2.0],
        fenv: [6.0, 220.0, 0.0, 100.0],
        fenv_amt: 0.16,
        wheel_amt: 0.3,
        aenv: [3.0, 250.0, 0.7, 40.0],
        trim_db: 9.0,
    });
    let (lad, m) = (v.lad, v.m);
    v.r.route(v.m, "velocity", v.lad, "cutoff_hz", 0.03);
    // Cutoff: the filter opens from a dark growl to a bright snarl.
    let cut = routed("filter.ladder", "cutoff_hz", 220.0, 0.45, 0.5);
    span_hi(
        &mut v,
        1,
        0.5,
        lad,
        "filter.ladder",
        "cutoff_hz",
        cut,
        4000.0,
    );
    // The resonance rises with the cutoff: the squelch of the sweep.
    let res = routed("filter.ladder", "resonance", 0.28, 0.35, 0.35);
    span(&mut v, 1, 0.5, lad, "filter.ladder", "resonance", 0.2, res);
    // Accent: the notes go from short and clipped to long and held (envelope shape).
    let (aenv, fenv) = (v.aenv, v.fenv);
    span(&mut v, 2, 0.6, aenv, "env.adsr", "decay_ms", 90.0, 250.0);
    span_hi(&mut v, 2, 0.6, aenv, "env.adsr", "sustain", 0.7, 1.0);
    span(&mut v, 2, 0.6, fenv, "env.adsr", "decay_ms", 80.0, 220.0);
    // Slide: how long the glide between overlapped notes takes.
    span(&mut v, 3, 0.4, m, "midi.in", "glide_ms", 15.0, 55.0);
    // The old Drive stays where it was stored.
    let drive = routed("drive", "drive_db", 7.0, 0.5, 0.4);
    v.r.e.set_param(v.drive, "drive_db", drive);
    v.stage("compressor", &compressor(-24.0, 3.0, 5.0, 100.0, 4.0));
    // Space: the dotted echo repeats more and more (the echo and room stay at the old stored mix).
    let knob = v.knob;
    let mut r = v.finish(
        "",
        &echo(352.0, 45.0, 4000.0),
        0.25 * 0.4 * 100.0,
        (0.9, 0.25 * 0.2 * 100.0),
        None,
    );
    let delay =
        *r.e.state()
            .modules
            .iter()
            .find(|(_, m)| m.kind == "delay")
            .expect("the echo")
            .0;
    span_rack(&mut r, knob, 4, 0.7, delay, "delay", "feedback", 0.0, 45.0);
    r
}

/// Oatmeal: a clean round pop bass, a triangle over a sine, a soft pluck in the filter.
fn soft_shoe() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Warmth", 0.20),
            ("Soften", 0.0),
            ("Throb", 0.0),
            ("Squeeze", 0.40),
        ],
        midi: [1.0, 0.0, 120.0],
        oscs: vec![osc(1.0, 0.0, 0.5), osc(0.0, -1.0, 0.6)],
        drive: [3.0, -1.0],
        lad: [500.0, 0.10, 0.0],
        fenv: [2.0, 180.0, 0.3, 80.0],
        fenv_amt: 0.0,
        wheel_amt: 0.2,
        aenv: [3.0, 350.0, 0.7, 110.0],
        trim_db: 9.5,
    });
    let (lad, fenv, drive_id, aenv) = (v.lad, v.fenv, v.drive, v.aenv);
    // The low-pass stays where the old Tone was stored (a soft triangle has little above it).
    let cut = routed("filter.ladder", "cutoff_hz", 500.0, 0.5, 0.5);
    v.r.e.set_param(lad, "cutoff_hz", cut);
    // Warmth: a soft saturation adds harmonics to the round note, from clean to warm.
    let d = routed("drive", "drive_db", 3.0, 0.5, 0.2);
    span(&mut v, 1, 0.2, drive_id, "drive", "drive_db", 3.0, d);
    // The old Pluck stays where it was stored: a light filter snap on each note.
    v.r.route(fenv, "out", lad, "cutoff_hz", 0.35);
    // Soften: the pop of each note becomes a slower swell and a longer ring-out (articulation).
    rise(&mut v, 2, aenv, "env.adsr", "attack_ms", 3.0, 40.0);
    rise(&mut v, 2, aenv, "env.adsr", "release_ms", 110.0, 600.0);
    // The old Body stays where it was: the sine under the triangle at its stored level.
    // Throb: a slow tremolo fades in on the note (motion, off by default).
    let lfo = v.r.add("lfo", &[("rate_hz", 4.5)]);
    let mono = v.mono;
    v.r.depth_route(lfo, "out", mono, "gain_db", v.knob, "m3", 0.08);
    // Squeeze: the compressor clamps harder and the makeup follows.
    let comp = v.stage("compressor", &compressor(-22.0, 3.0, 10.0, 120.0, 3.0));
    span(
        &mut v,
        4,
        0.4,
        comp,
        "compressor",
        "threshold_db",
        -8.0,
        -22.0,
    );
    span(&mut v, 4, 0.4, comp, "compressor", "makeup_db", 0.0, 3.0);
    v.finish(
        "",
        &echo(110.0, 20.0, 3500.0),
        0.25 * 0.12 * 100.0,
        (0.6, 0.25 * 0.2 * 100.0),
        None,
    )
}

/// Spun Sugar: a smooth singing pop lead, a detuned saw stack over a triangle, chorus.
fn satin_ribbon() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Tone", 0.80),
            ("Vibrato", 0.35),
            ("Shimmer", 0.40),
            ("Space", 0.50),
        ],
        midi: [2.0, 2.0, 60.0],
        oscs: vec![
            osc(2.0, 0.0, 0.5).unison(3.0, 10.0),
            osc(1.0, 0.0, 0.35).fine(5.0),
        ],
        drive: [3.0, -1.0],
        lad: [1600.0, 0.15, 0.0],
        fenv: [5.0, 300.0, 0.5, 200.0],
        fenv_amt: 0.10,
        wheel_amt: 0.0,
        aenv: [8.0, 200.0, 0.8, 260.0],
        trim_db: 11.0,
    });
    let (lad, o0) = (v.lad, v.oscs[0]);
    v.r.route(v.m, "velocity", v.lad, "cutoff_hz", 0.10);
    // Tone: the filter opens from a soft vowel to a bright singing edge.
    let cut = routed("filter.ladder", "cutoff_hz", 1600.0, 0.4, 0.55);
    span_hi(
        &mut v,
        1,
        0.8,
        lad,
        "filter.ladder",
        "cutoff_hz",
        cut,
        12000.0,
    );
    // The old Edge stays where it was stored.
    let drive = routed("drive", "drive_db", 3.0, 0.45, 0.2);
    v.r.e.set_param(v.drive, "drive_db", drive);
    // Vibrato: a gentle vibrato fades in, and it speeds up as it deepens.
    let lfo = v.vibrato("m2", 5.5, 0.1);
    span(&mut v, 2, 0.35, lfo, "lfo", "rate_hz", 3.5, 5.5);
    // Shimmer: the chorus widens and the saw stack spreads (a lone voice to a choir).
    span(&mut v, 3, 0.4, o0, "osc.va", "detune", 4.0, 10.0);
    let chorus = v.stage(
        "chorus",
        &[("rate_hz", 0.5), ("depth", 40.0), ("mix", 30.0)],
    );
    span(&mut v, 3, 0.4, chorus, "chorus", "mix", 8.0, 30.0);
    span(&mut v, 3, 0.4, chorus, "chorus", "depth", 20.0, 40.0);
    v.stage("compressor", &compressor(-24.0, 2.0, 15.0, 150.0, 2.0));
    v.finish("m4", &echo(375.0, 38.0, 4500.0), 0.3, (2.2, 0.3), None)
}

/// Pig Iron: a hard, crushed, hard-retriggered lead, a pulse and a saw into heavy drive.
fn rust_belt() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Bite", 0.50),
            ("Weld", 0.50),
            ("Crush", 0.50),
            ("Length", 0.60),
        ],
        midi: [1.0, 0.0, 120.0],
        oscs: vec![osc(3.0, 0.0, 0.5).pw(35.0), osc(2.0, 0.0, 0.45).fine(7.0)],
        drive: [16.0, -8.0],
        lad: [900.0, 0.25, 6.0],
        fenv: [6.0, 120.0, 0.3, 60.0],
        fenv_amt: 0.15,
        wheel_amt: 0.0,
        aenv: [5.0, 150.0, 0.8, 50.0],
        trim_db: 8.0,
    });
    let (lad, aenv, (o0, o1)) = (v.lad, v.aenv, (v.oscs[0], v.oscs[1]));
    // Bite: the filter opens and each note snaps harder.
    let cut = routed("filter.ladder", "cutoff_hz", 900.0, 0.5, 0.5);
    span_hi(
        &mut v,
        1,
        0.5,
        lad,
        "filter.ladder",
        "cutoff_hz",
        cut,
        12000.0,
    );
    v.snap(1, 0.5, 0.2);
    // Weld: the saw drifts against the pulse and the pulse thins (a fat weld to a thin whine).
    span(&mut v, 2, 0.5, o1, "osc.va", "fine", 0.0, 7.0);
    span(&mut v, 2, 0.5, o0, "osc.va", "pw", 50.0, 35.0);
    // The old Grit stays where it was stored.
    let drive = routed("drive", "drive_db", 16.0, 0.35, 0.6);
    v.r.e.set_param(v.drive, "drive_db", drive);
    // Crush: the digital edge comes in, and the crush gets coarser as it does.
    let crush = v.stage(
        "bitcrusher",
        &[("bits", 7.0), ("downsample", 3.0), ("mix", 0.0)],
    );
    span(&mut v, 3, 0.5, crush, "bitcrusher", "mix", 0.0, 35.0);
    span_hi(&mut v, 3, 0.5, crush, "bitcrusher", "bits", 7.0, 5.0);
    span(&mut v, 3, 0.5, crush, "bitcrusher", "downsample", 1.5, 3.0);
    v.stage("compressor", &compressor(-26.0, 6.0, 2.0, 80.0, 7.0));
    // Length: the note holds its level longer (a stab to a held blare).
    span_hi(&mut v, 4, 0.6, aenv, "env.adsr", "sustain", 0.8, 1.0);
    span(&mut v, 4, 0.6, aenv, "env.adsr", "release_ms", 20.0, 50.0);
    v.finish(
        "",
        &echo(110.0, 20.0, 3000.0),
        0.25 * 0.25 * 100.0,
        (0.4, 0.25 * 0.15 * 100.0),
        None,
    )
}

/// Hot Wire: a legato acid lead, a saw and a narrow pulse, a resonant ladder, a dotted echo.
fn hot_wire() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Squelch", 0.50),
            ("Slide", 0.40),
            ("Wire", 0.30),
            ("Space", 0.40),
        ],
        midi: [2.0, 2.0, 45.0],
        oscs: vec![osc(2.0, 0.0, 0.6), osc(3.0, 0.0, 0.3).pw(40.0).fine(4.0)],
        drive: [8.0, -4.0],
        lad: [600.0, 0.32, 2.0],
        fenv: [4.0, 200.0, 0.25, 120.0],
        fenv_amt: 0.15,
        wheel_amt: 0.0,
        aenv: [3.0, 250.0, 0.75, 120.0],
        trim_db: 12.0,
    });
    let (lad, mix, o1, v_m) = (v.lad, v.mix, v.oscs[1], v.m);
    v.r.route(v.m, "velocity", v.lad, "cutoff_hz", 0.05);
    // Cutoff: the filter opens from a closed growl to a bright scream.
    let cut = routed("filter.ladder", "cutoff_hz", 600.0, 0.4, 0.5);
    span_hi(
        &mut v,
        1,
        0.5,
        lad,
        "filter.ladder",
        "cutoff_hz",
        cut,
        9000.0,
    );
    // The resonance rises with the cutoff: the squelch of the sweep.
    let res = routed("filter.ladder", "resonance", 0.32, 0.38, 0.35);
    span(&mut v, 1, 0.5, lad, "filter.ladder", "resonance", 0.2, res);
    // Slide: how long the glide between overlapped notes takes.
    span(&mut v, 2, 0.4, v_m, "midi.in", "glide_ms", 8.0, 45.0);
    // Wire: the pulse sweeps in, faster, and the pulse oscillator thickens under the saw.
    let pwm = v.r.add("lfo", &[("rate_hz", 0.7)]);
    v.depth_keep(pwm, "out", o1, "pw", 3, 0.3, 0.35, 0.105);
    span(&mut v, 3, 0.3, pwm, "lfo", "rate_hz", 0.3, 0.7);
    span(&mut v, 3, 0.3, mix, "mixer", "level2", 0.15, 0.3);
    v.stage("compressor", &compressor(-24.0, 3.0, 5.0, 100.0, 4.0));
    v.finish("m4", &echo(352.0, 45.0, 4500.0), 0.4, (1.0, 0.2), None)
}

/// Lantern: a soft ambient lead that swells in, a triangle stack and a sine an octave up.
fn paper_crane() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Glow", 0.50),
            ("Swell", 0.40),
            ("Drift", 0.30),
            ("Space", 0.65),
        ],
        midi: [2.0, 2.0, 140.0],
        oscs: vec![osc(1.0, 0.0, 0.45).unison(2.0, 6.0), osc(0.0, 1.0, 0.2)],
        drive: [2.0, -1.0],
        lad: [900.0, 0.10, 0.0],
        fenv: [200.0, 500.0, 0.4, 600.0],
        fenv_amt: 0.12,
        wheel_amt: 0.0,
        aenv: [180.0, 400.0, 0.8, 900.0],
        trim_db: 14.0,
    });
    let (lad, aenv, mix) = (v.lad, v.aenv, v.mix);
    // Glow: the filter opens and the sine an octave up comes forward.
    let cut = routed("filter.ladder", "cutoff_hz", 900.0, 0.45, 0.5);
    span_hi(
        &mut v,
        1,
        0.5,
        lad,
        "filter.ladder",
        "cutoff_hz",
        cut,
        12000.0,
    );
    span(&mut v, 1, 0.5, mix, "mixer", "level2", 0.05, 0.2);
    // Swell: each note takes longer to bloom and longer to fade.
    let attack = routed("env.adsr", "attack_ms", 180.0, 0.45, 0.4);
    span_hi(
        &mut v,
        2,
        0.4,
        aenv,
        "env.adsr",
        "attack_ms",
        attack,
        5000.0,
    );
    span(&mut v, 2, 0.4, aenv, "env.adsr", "release_ms", 500.0, 900.0);
    // Drift: a slow vibrato fades in, the chorus swims wider and slower.
    let lfo = v.vibrato("m3", 4.8, 0.08);
    span(&mut v, 3, 0.3, lfo, "lfo", "rate_hz", 3.2, 4.8);
    let chorus = v.stage(
        "chorus",
        &[("rate_hz", 0.3), ("depth", 35.0), ("mix", 40.0)],
    );
    span(&mut v, 3, 0.3, chorus, "chorus", "depth", 15.0, 35.0);
    span(&mut v, 3, 0.3, chorus, "chorus", "rate_hz", 0.1, 0.3);
    v.stage("compressor", &compressor(-24.0, 2.0, 20.0, 200.0, 2.0));
    v.finish("m4", &echo(480.0, 50.0, 4000.0), 0.55, (6.0, 0.6), None)
}

// ---- group 2 --------------------------------------------------------------------------------

/// Rubber Band: a funky plucked bass, a narrow pulse over a saw, a quick filter slap.
fn rubber_band() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Room", 0.20),
            ("Slap", 0.40),
            ("Stab", 0.50),
            ("Body", 0.50),
        ],
        midi: [1.0, 0.0, 120.0],
        oscs: vec![osc(3.0, 0.0, 0.45).pw(30.0), osc(2.0, -1.0, 0.45)],
        drive: [6.0, -3.0],
        lad: [250.0, 0.20, 2.0],
        fenv: [8.0, 90.0, 0.05, 60.0],
        fenv_amt: 0.12,
        wheel_amt: 0.2,
        aenv: [6.0, 260.0, 0.35, 70.0],
        trim_db: 9.0,
    });
    let (lad, fenv, aenv, mix) = (v.lad, v.fenv, v.aenv, v.mix);
    v.r.route(v.m, "velocity", v.lad, "cutoff_hz", 0.1);
    // The cutoff stays where the old Tone was stored (Slap opens it per note).
    let cut = routed("filter.ladder", "cutoff_hz", 250.0, 0.45, 0.5);
    v.r.e.set_param(lad, "cutoff_hz", cut);
    // Slap: the filter snap on every note gets a voice: more resonance on the pop (a plain pop to a wah).
    v.r.route(fenv, "out", lad, "cutoff_hz", 0.33);
    span(&mut v, 2, 0.4, lad, "filter.ladder", "resonance", 0.05, 0.2);
    // Stab: short and dry to long and ringing (decay, sustain and release together).
    span(&mut v, 3, 0.5, aenv, "env.adsr", "decay_ms", 120.0, 260.0);
    span(&mut v, 3, 0.5, aenv, "env.adsr", "sustain", 0.2, 0.35);
    span(&mut v, 3, 0.5, aenv, "env.adsr", "release_ms", 15.0, 70.0);
    // Body: the saw an octave down comes forward and the pulse steps back.
    span(&mut v, 4, 0.5, mix, "mixer", "level2", 0.15, 0.45);
    span(&mut v, 4, 0.5, mix, "mixer", "level1", 0.6, 0.45);
    // The old Grit stays where it was stored.
    let drive = routed("drive", "drive_db", 6.0, 0.5, 0.2);
    v.r.e.set_param(v.drive, "drive_db", drive);
    v.stage("compressor", &compressor(-22.0, 4.0, 6.0, 90.0, 5.0));
    // Room: a short slapback and room around the pluck.
    v.finish("m1", &echo(120.0, 18.0, 3500.0), 0.12, (0.5, 0.15), None)
}

/// Slate Roof: a neuro bass, a unison saw stack against a detuned saw over a sub, with a growl.
fn slate_roof() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Cutoff", 0.45),
            ("Growl", 0.44),
            ("Stack", 0.50),
            ("Room", 0.20),
        ],
        midi: [1.0, 0.0, 120.0],
        oscs: vec![
            osc(2.0, 0.0, 0.4).unison(4.0, 30.0),
            osc(2.0, -1.0, 0.4).fine(8.0),
            osc(3.0, -1.0, 0.3),
        ],
        drive: [14.0, -7.0],
        lad: [260.0, 0.22, 4.0],
        fenv: [8.0, 300.0, 0.2, 80.0],
        fenv_amt: 0.2,
        wheel_amt: 0.25,
        aenv: [6.0, 200.0, 0.85, 50.0],
        trim_db: 9.0,
    });
    let (lad, (o0, o1)) = (v.lad, (v.oscs[0], v.oscs[1]));
    // Cutoff: the filter opens from a closed rumble to a bright snarl.
    let cut = routed("filter.ladder", "cutoff_hz", 260.0, 0.45, 0.45);
    span_hi(
        &mut v,
        1,
        0.45,
        lad,
        "filter.ladder",
        "cutoff_hz",
        cut,
        4000.0,
    );
    // Growl: the filter rolls faster and deeper.
    let growl = v.r.add("lfo", &[("rate_hz", 3.2)]);
    v.depth_keep(growl, "out", lad, "cutoff_hz", 2, 0.44, 0.2, 0.0875);
    span(&mut v, 2, 0.44, growl, "lfo", "rate_hz", 1.2, 3.2);
    // Stack: the saw stack spreads and the second saw drifts (a tight stack to a wall).
    span(&mut v, 3, 0.5, o0, "osc.va", "detune", 8.0, 30.0);
    span(&mut v, 3, 0.5, o1, "osc.va", "fine", 2.0, 8.0);
    // The crusher stays at the mix the old Crush was stored at.
    v.stage(
        "bitcrusher",
        &[
            ("bits", 6.0),
            ("downsample", 4.0),
            ("mix", 0.2 * 0.9 * 100.0),
        ],
    );
    v.stage("compressor", &compressor(-24.0, 6.0, 3.0, 90.0, 6.0));
    // Room: a tight room and slapback.
    v.finish("m4", &echo(100.0, 18.0, 3500.0), 0.12, (0.4, 0.12), None)
}

/// Gutter: a dirty mid bass for French touch, a saw and a sub into 24 dB of drive, an octave pulse.
fn gutter() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Bite", 0.45),
            ("Crush", 0.25),
            ("Octave", 0.30),
            ("Squash", 0.50),
        ],
        midi: [1.0, 0.0, 120.0],
        oscs: vec![
            osc(2.0, 0.0, 0.5).unison(2.0, 10.0),
            osc(3.0, -1.0, 0.45),
            osc(3.0, 0.0, 0.0).pw(25.0),
        ],
        drive: [24.0, -12.0],
        lad: [320.0, 0.25, 10.0],
        fenv: [2.0, 110.0, 0.1, 60.0],
        fenv_amt: 0.25,
        wheel_amt: 0.25,
        aenv: [2.0, 150.0, 0.8, 40.0],
        trim_db: 7.5,
    });
    let (lad, drive) = (v.lad, v.drive);
    // Bite: the filter opens and each note snaps harder.
    let cut = routed("filter.ladder", "cutoff_hz", 320.0, 0.45, 0.45);
    span_hi(
        &mut v,
        1,
        0.45,
        lad,
        "filter.ladder",
        "cutoff_hz",
        cut,
        5000.0,
    );
    v.snap(1, 0.45, 0.15);
    // The drive stays where the old Grit was stored (it is saturated there: more drive adds little).
    let d = routed("drive", "drive_db", 24.0, 0.3, 0.65);
    v.r.e.set_param(drive, "drive_db", d);
    v.r.route(v.knob, "m3", v.mix, "level3", 0.6);
    // Crush: the crusher bites into the drive, coarser and coarser (digital grit).
    let crush = v.stage(
        "bitcrusher",
        &[("bits", 8.0), ("downsample", 2.0), ("mix", 10.0)],
    );
    span(&mut v, 2, 0.25, crush, "bitcrusher", "mix", 0.0, 10.0);
    span_hi(&mut v, 2, 0.25, crush, "bitcrusher", "bits", 8.0, 4.0);
    span_hi(&mut v, 2, 0.25, crush, "bitcrusher", "downsample", 2.0, 6.0);
    // Squash: the compressor clamps harder and harder and the makeup follows.
    let comp = v.stage("compressor", &compressor(-28.0, 8.0, 1.0, 80.0, 14.0));
    span(
        &mut v,
        4,
        0.5,
        comp,
        "compressor",
        "threshold_db",
        -12.0,
        -28.0,
    );
    span(&mut v, 4, 0.5, comp, "compressor", "ratio", 4.0, 8.0);
    span(&mut v, 4, 0.5, comp, "compressor", "makeup_db", 6.0, 14.0);
    v.finish(
        "",
        &echo(95.0, 20.0, 3500.0),
        0.25 * 0.22 * 100.0,
        (0.4, 0.25 * 0.16 * 100.0),
        None,
    )
}

/// Low Tide: an ambient sub, a sine over a triangle, a slow swell and a slow tide in the filter.
fn low_tide() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Tone", 0.45),
            ("Tide", 0.40),
            ("Swell", 0.30),
            ("Space", 0.60),
        ],
        midi: [2.0, 2.0, 160.0],
        oscs: vec![osc(0.0, -1.0, 0.7), osc(1.0, -1.0, 0.25).fine(4.0)],
        drive: [2.0, -1.0],
        lad: [350.0, 0.10, 0.0],
        fenv: [40.0, 500.0, 0.5, 500.0],
        fenv_amt: 0.10,
        wheel_amt: 0.0,
        aenv: [60.0, 500.0, 0.9, 1200.0],
        trim_db: 14.0,
    });
    let (lad, aenv, mix) = (v.lad, v.aenv, v.mix);
    // Tone: the filter opens and the triangle an octave down comes forward.
    let cut = routed("filter.ladder", "cutoff_hz", 350.0, 0.45, 0.45);
    span_hi(
        &mut v,
        1,
        0.45,
        lad,
        "filter.ladder",
        "cutoff_hz",
        cut,
        5000.0,
    );
    span(&mut v, 1, 0.45, mix, "mixer", "level2", 0.1, 0.25);
    // Tide: a slow filter tide comes in, and it gets faster as it deepens.
    let tide = v.r.add("lfo", &[("rate_hz", 0.15)]);
    v.depth_keep(tide, "out", lad, "cutoff_hz", 2, 0.4, 0.26, 0.105);
    span_hi(&mut v, 2, 0.4, tide, "lfo", "rate_hz", 0.15, 0.4);
    span(
        &mut v,
        2,
        0.4,
        lad,
        "filter.ladder",
        "resonance",
        0.02,
        0.10,
    );
    // Swell: each note takes longer to bloom and longer to fade.
    let attack = routed("env.adsr", "attack_ms", 60.0, 0.4, 0.3);
    span_hi(
        &mut v,
        3,
        0.3,
        aenv,
        "env.adsr",
        "attack_ms",
        attack,
        1500.0,
    );
    span(
        &mut v,
        3,
        0.3,
        aenv,
        "env.adsr",
        "release_ms",
        600.0,
        1200.0,
    );
    v.stage(
        "chorus",
        &[("rate_hz", 0.2), ("depth", 30.0), ("mix", 25.0)],
    );
    v.stage("compressor", &compressor(-24.0, 2.0, 30.0, 300.0, 2.0));
    v.finish("m4", &echo(600.0, 45.0, 3000.0), 0.35, (7.0, 0.55), None)
}

/// A pitch kick on each note: a fast envelope off the gate (it falls to 0, so the pitch settles
/// back) drives the fine tune of `targets`; its depth is on macro `mac` (0 at the stored position
/// 0, so the stored sound is unchanged). Returns the envelope for its decay.
fn kick(
    v: &mut Voice,
    mac: &str,
    param: &str,
    decay_ms: f32,
    depth: f32,
    targets: &[ModuleId],
) -> ModuleId {
    let env = v.r.add("env.adsr", &adsr(1.0, decay_ms, 0.0, 20.0));
    v.r.wire(v.m, "gate", env, "gate");
    for &o in targets {
        v.r.depth_route(env, "out", o, param, v.knob, mac, depth);
    }
    env
}

/// A fixed vibrato (the old macro's stored depth) on every oscillator.
fn fixed_vibrato(v: &mut Voice, rate_hz: f32, amount: f32) -> ModuleId {
    let lfo = v.r.add("lfo", &[("rate_hz", rate_hz)]);
    for o in v.oscs.clone() {
        v.r.route(lfo, "out", o, "fine", amount);
    }
    lfo
}

/// Copper Wire: an experimental metal bass, a sine ring-modulated by a triangle a fifth up.
fn copper_wire() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Metal", 0.45),
            ("Weight", 0.50),
            ("Hold", 0.50),
            ("Space", 0.25),
        ],
        midi: [1.0, 0.0, 120.0],
        oscs: vec![osc(0.0, -1.0, 0.5), osc(1.0, -1.0 + 7.0 / 12.0, 0.0)],
        drive: [8.0, -4.0],
        lad: [500.0, 0.15, 0.0],
        fenv: [3.0, 200.0, 0.15, 80.0],
        fenv_amt: 0.2,
        wheel_amt: 0.2,
        aenv: [2.0, 300.0, 0.7, 80.0],
        trim_db: 9.0,
    });
    // The ring modulator takes the first two oscillators and joins the mix on the third input.
    let ring = v.r.add("ringmod", &[]);
    v.r.wire(v.oscs[0], "out", ring, "a");
    v.r.wire(v.oscs[1], "out", ring, "b");
    v.r.wire(ring, "out", v.mix, "in3");
    let (lad, mix, aenv, drive) = (v.lad, v.mix, v.aenv, v.drive);
    // The old Tone and Grit stay where they were stored.
    let cut = routed("filter.ladder", "cutoff_hz", 500.0, 0.45, 0.5);
    v.r.e.set_param(lad, "cutoff_hz", cut);
    let d = routed("drive", "drive_db", 8.0, 0.5, 0.2);
    // Metal: the ring modulator comes in.
    span(&mut v, 1, 0.45, mix, "mixer", "level3", 0.0, 0.315);
    // Weight: the clean sine under the bells grows from thin to a heavy sub (the old Grit stays
    // at its stored drive).
    span(&mut v, 2, 0.5, mix, "mixer", "level1", 0.25, 0.5);
    v.r.e.set_param(drive, "drive_db", d);
    // Hold: a short, plucked sine to a long, ringing drone (decay, sustain, release together).
    span(&mut v, 3, 0.5, aenv, "env.adsr", "decay_ms", 90.0, 300.0);
    span_hi(&mut v, 3, 0.5, aenv, "env.adsr", "sustain", 0.7, 1.0);
    span(&mut v, 3, 0.5, aenv, "env.adsr", "release_ms", 25.0, 80.0);
    v.stage("compressor", &compressor(-24.0, 3.0, 8.0, 100.0, 4.0));
    // Space: an echo and a room that keep the bells ringing.
    v.finish("m4", &echo(210.0, 35.0, 4500.0), 0.3, (0.8, 0.25), None)
}

/// Tail Light: a synthwave bass, two saws and a sub with a medium pluck and a chorus.
fn tail_light() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Pluck", 0.50),
            ("Width", 0.40),
            ("Weight", 0.50),
            ("Space", 0.40),
        ],
        midi: [1.0, 0.0, 120.0],
        oscs: vec![
            osc(2.0, 0.0, 0.5).unison(2.0, 10.0),
            osc(2.0, -1.0, 0.4),
            osc(3.0, -1.0, 0.2),
        ],
        drive: [4.0, -2.0],
        lad: [300.0, 0.20, 2.0],
        fenv: [4.0, 250.0, 0.2, 100.0],
        fenv_amt: 0.2,
        wheel_amt: 0.2,
        aenv: [3.0, 300.0, 0.75, 90.0],
        trim_db: 9.0,
    });
    let (fenv, mix, o0) = (v.fenv, v.mix, v.oscs[0]);
    // The old Cutoff and Drive stay where they were stored.
    let cut = routed("filter.ladder", "cutoff_hz", 300.0, 0.45, 0.5);
    v.r.e.set_param(v.lad, "cutoff_hz", cut);
    let d = routed("drive", "drive_db", 4.0, 0.5, 0.3);
    v.r.e.set_param(v.drive, "drive_db", d);
    // Pluck: the filter snap of each note, deeper and longer: a tight pluck to a slow open.
    v.snap(1, 0.5, 0.25);
    span(&mut v, 1, 0.5, fenv, "env.adsr", "decay_ms", 60.0, 250.0);
    // Width: the chorus comes in and swims wider, and the saw stack spreads.
    let chorus = v.stage("chorus", &[("rate_hz", 0.5), ("depth", 40.0), ("mix", 0.0)]);
    let cmix = routed("chorus", "mix", 0.0, 0.5, 0.4);
    span(&mut v, 2, 0.4, chorus, "chorus", "mix", 0.0, cmix);
    span(&mut v, 2, 0.4, chorus, "chorus", "depth", 15.0, 40.0);
    span(&mut v, 2, 0.4, o0, "osc.va", "detune", 3.0, 10.0);
    // Weight: the square sub grows and the second saw steps back.
    span(&mut v, 3, 0.5, mix, "mixer", "level3", 0.05, 0.2);
    span(&mut v, 3, 0.5, mix, "mixer", "level2", 0.55, 0.4);
    v.stage("compressor", &compressor(-24.0, 3.0, 8.0, 100.0, 4.0));
    v.finish("m4", &echo(352.0, 38.0, 4000.0), 0.3, (1.8, 0.3), None)
}

/// Rail Yard: a house bass that ducks on every beat of the host clock; press Run.
fn rail_yard() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Pump", 0.275),
            ("Snap", 0.50),
            ("Thick", 0.30),
            ("Room", 0.25),
        ],
        midi: [1.0, 0.0, 120.0],
        oscs: vec![osc(2.0, 0.0, 0.5).unison(2.0, 6.0), osc(3.0, -1.0, 0.45)],
        drive: [8.0, -4.0],
        lad: [260.0, 0.20, 3.0],
        fenv: [3.0, 160.0, 0.1, 70.0],
        fenv_amt: 0.2,
        wheel_amt: 0.25,
        aenv: [2.0, 190.0, 0.8, 50.0],
        trim_db: 11.0,
    });
    let (fenv, o0, drive) = (v.fenv, v.oscs[0], v.drive);
    // The old Cutoff stays where it was stored.
    let cut = routed("filter.ladder", "cutoff_hz", 260.0, 0.45, 0.5);
    v.r.e.set_param(v.lad, "cutoff_hz", cut);
    // Snap: the filter bites each note and falls back, deeper and longer.
    v.snap(2, 0.5, 0.25);
    span(&mut v, 2, 0.5, fenv, "env.adsr", "decay_ms", 50.0, 160.0);
    // Thick: the saw stack spreads and the drive thickens it (the old Drive is the stored amount).
    span(&mut v, 3, 0.3, o0, "osc.va", "detune", 0.0, 6.0);
    let d = routed("drive", "drive_db", 8.0, 0.4, 0.3);
    span(&mut v, 3, 0.3, drive, "drive", "drive_db", 3.0, d);
    v.stage("compressor", &compressor(-24.0, 4.0, 3.0, 90.0, 6.0));
    let clock = v.r.add("clock", &[("bpm", 124.0), ("swing", 20.0)]);
    // Pump: the beat ducks the bass deeper, and it takes longer to swell back.
    let env = clock_duck(&mut v, clock, 200.0, -0.60, "m1");
    span_hi(&mut v, 1, 0.275, env, "env.adsr", "release_ms", 50.0, 110.0);
    v.finish(
        "m4",
        &[
            ("sync", 3.0),
            ("time_ms", 360.0),
            ("feedback", 30.0),
            ("mix", 0.0),
            ("tone_hz", 3500.0),
        ],
        0.2,
        (0.5, 0.16),
        Some(clock),
    )
}

/// Oak Stave: an upright-style plucked bass, a triangle with a sine body and a soft pitch snap.
fn oak_stave() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Pluck", 0.50),
            ("Thump", 0.30),
            ("Body", 0.40),
            ("Room", 0.30),
        ],
        midi: [1.0, 0.0, 120.0],
        oscs: vec![osc(1.0, 0.0, 0.5), osc(0.0, -1.0, 0.2)],
        drive: [2.0, -1.0],
        lad: [380.0, 0.10, 0.0],
        fenv: [2.0, 240.0, 0.1, 60.0],
        // The recipe's .25 plus the old Pluck's .5 at its stored position.
        fenv_amt: 0.75,
        wheel_amt: 0.0,
        aenv: [2.0, 700.0, 0.15, 160.0],
        trim_db: 16.0,
    });
    let (fenv, aenv, mix, o0) = (v.fenv, v.aenv, v.mix, v.oscs[0]);
    let cut = routed("filter.ladder", "cutoff_hz", 380.0, 0.4, 0.5);
    v.r.e.set_param(v.lad, "cutoff_hz", cut);
    // Pluck: how hard the filter snaps on each note and how long it takes to close again.
    v.snap(1, 0.5, 0.5);
    span(&mut v, 1, 0.5, fenv, "env.adsr", "decay_ms", 90.0, 240.0);
    // Thump: the soft pitch snap at the front of the note, from none to a plucked-string kick.
    v.depth_keep(fenv, "out", o0, "fine", 2, 0.3, 0.5, 0.15);
    // Body: the sine under the note comes forward and the note rings longer.
    // (The stored sine level is the recipe's .2 plus the old Body's .6 x .4.)
    span(&mut v, 3, 0.4, mix, "mixer", "level2", 0.15, 0.44);
    span(&mut v, 3, 0.4, aenv, "env.adsr", "decay_ms", 250.0, 700.0);
    v.stage("compressor", &compressor(-22.0, 3.0, 10.0, 120.0, 3.0));
    v.finish("m4", &echo(95.0, 18.0, 3500.0), 0.2, (0.8, 0.25), None)
}

/// Glass Needle: a pure modular lead, a sine with an octave of overtone and a ping-pong echo.
fn glass_needle() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Overtone", 0.30),
            ("Ping", 0.00),
            ("Vibrato", 0.10),
            ("Space", 0.55),
        ],
        midi: [2.0, 2.0, 70.0],
        oscs: vec![osc(0.0, 0.0, 0.6), osc(0.0, 1.0, 0.0)],
        drive: [0.0, 0.0],
        lad: [1800.0, 0.05, 0.0],
        fenv: [3.0, 150.0, 0.5, 100.0],
        fenv_amt: 0.08,
        wheel_amt: 0.0,
        aenv: [4.0, 300.0, 0.8, 320.0],
        trim_db: 14.0,
    });
    let (mix, oscs) = (v.mix, v.oscs.clone());
    let cut = routed("filter.ladder", "cutoff_hz", 1800.0, 0.4, 0.55);
    v.r.e.set_param(v.lad, "cutoff_hz", cut);
    // Overtone: the octave above comes in (a thin sine to a glassy ring).
    span(&mut v, 1, 0.3, mix, "mixer", "level2", 0.03, 0.15);
    // Ping: a pitch kick at the front of each note, like a needle touching glass.
    kick(&mut v, "m2", "fine", 200.0, 0.5, &oscs);
    // Vibrato: fades in and speeds up as it deepens.
    let lfo = v.r.add("lfo", &[("rate_hz", 5.0)]);
    for &o in &oscs {
        v.r.depth_route(lfo, "out", o, "fine", v.knob, "m3", 0.15);
    }
    span_hi(&mut v, 3, 0.1, lfo, "lfo", "rate_hz", 5.0, 7.5);
    v.stage("compressor", &compressor(-24.0, 2.0, 15.0, 150.0, 2.0));
    v.finish(
        "m4",
        &[
            ("time_ms", 420.0),
            ("feedback", 50.0),
            ("mix", 0.0),
            ("tone_hz", 7000.0),
            ("mode", 1.0),
        ],
        0.4,
        (3.5, 0.4),
        None,
    )
}

/// Loud Mouth: a talking lead for French touch, a saw and a pulse driven hard through a formant filter.
fn loud_mouth() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Vowel", 0.35),
            ("Talk", 0.35),
            ("Shift", 0.50),
            ("Shout", 0.50),
        ],
        midi: [1.0, 0.0, 120.0],
        oscs: vec![
            osc(2.0, 0.0, 0.55).unison(2.0, 8.0),
            osc(3.0, 0.0, 0.35).pw(40.0),
        ],
        drive: [14.0, -7.0],
        lad: [3000.0, 0.10, 0.0],
        fenv: [2.0, 100.0, 0.5, 60.0],
        fenv_amt: 0.0,
        wheel_amt: 0.0,
        aenv: [2.0, 200.0, 0.85, 60.0],
        trim_db: 21.0,
    });
    let drive = v.drive;
    let formant = v.mono_after(
        "filter.formant",
        &[("vowel", 0.0), ("shift_st", -6.0), ("q", 1.5)],
    );
    // Every vowel is held clear (`hold`): a sweep steps between a, e, i, o, u with short crossings,
    // no blends in the middle.
    v.r.e.set_param(formant, "hold", 1.0);
    // Vowel: a to u (the mod wheel moves it too).
    v.r.route(v.knob, "m1", formant, "vowel", 1.0);
    v.r.route(v.m, "wheel", formant, "vowel", 0.5);
    // Talk: on the host clock, the mouth jumps to a new vowel every eighth note (a talkbox), from
    // still to chattering. Press Run.
    let clock = v.r.add("clock", &[("bpm", 124.0), ("swing", 0.0)]);
    let talk =
        v.r.add("lfo", &[("waveform", 4.0), ("sync", 2.0), ("rate_hz", 4.0)]);
    v.r.wire(clock, "gate", talk, "clock");
    v.r.depth_route(talk, "out", formant, "vowel", v.knob, "m2", 0.5);
    // Shift: a smaller or a larger mouth.
    let shift = routed("filter.formant", "shift_st", -6.0, 0.5, 0.5);
    span_hi(
        &mut v,
        3,
        0.5,
        formant,
        "filter.formant",
        "shift_st",
        shift,
        12.0,
    );
    // Shout: the drive pushes hard, the formants sharpen and the level comes up into the
    // compressor (a mumble to a shout).
    let d = routed("drive", "drive_db", 14.0, 0.3, 0.5);
    span_hi(&mut v, 4, 0.5, drive, "drive", "drive_db", d, 32.0);
    span(&mut v, 4, 0.5, formant, "filter.formant", "q", 0.7, 1.5);
    let comp = v.stage("compressor", &compressor(-26.0, 6.0, 2.0, 80.0, 6.0));
    span(&mut v, 4, 0.5, comp, "compressor", "makeup_db", 0.0, 6.0);
    // The old Room stays at its stored level.
    let mut e = echo(110.0, 20.0, 3000.0).to_vec();
    e.push(("sync", 0.0));
    v.finish(
        "",
        &e,
        0.25 * 0.25 * 100.0,
        (0.4, 0.25 * 0.15 * 100.0),
        Some(clock),
    )
}

/// Velvet Sine: a soft, round lead, a sine with a hint of triangle, a short legato glide, vibrato
/// and a dotted echo. (Formerly Dust Devil: a long whistle-like slide was its sound.)
fn velvet_sine() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Glide", 0.50),
            ("Body", 0.35),
            ("Bloom", 0.00),
            ("Vibrato", 0.40),
        ],
        midi: [2.0, 1.0, 40.0],
        oscs: vec![osc(0.0, 0.0, 0.6), osc(1.0, 0.0, 0.0).fine(3.0)],
        drive: [2.0, -1.0],
        lad: [1400.0, 0.10, 0.0],
        fenv: [4.0, 200.0, 0.5, 100.0],
        fenv_amt: 0.05,
        wheel_amt: 0.0,
        aenv: [6.0, 200.0, 0.85, 220.0],
        trim_db: 13.0,
    });
    let (m, lad, aenv, mix) = (v.m, v.lad, v.aenv, v.mix);
    // Glide: how long the legato glide between notes takes, from a step to a short, singing slide.
    span(&mut v, 1, 0.5, m, "midi.in", "glide_ms", 5.0, 40.0);
    // Body: a pure sine to a rounder, hollow tone (the triangle comes in as the filter opens).
    span(&mut v, 2, 0.35, mix, "mixer", "level2", 0.0, 0.2);
    span(
        &mut v,
        2,
        0.35,
        lad,
        "filter.ladder",
        "cutoff_hz",
        700.0,
        1400.0,
    );
    // Bloom: each note swells in and fades out slower (a soft pluck at 0, a pad-like lead at 1).
    rise(&mut v, 3, aenv, "env.adsr", "attack_ms", 6.0, 500.0);
    span(
        &mut v,
        3,
        1.0,
        aenv,
        "env.adsr",
        "release_ms",
        220.0,
        1100.0,
    );
    // Vibrato: fades in and speeds up.
    let lfo = v.vibrato("m4", 5.6, 0.08);
    span_hi(&mut v, 4, 0.4, lfo, "lfo", "rate_hz", 5.6, 8.5);
    v.stage("compressor", &compressor(-24.0, 2.0, 15.0, 150.0, 2.0));
    v.finish(
        "",
        &echo(310.0, 40.0, 3500.0),
        0.4 * 0.3 * 100.0,
        (1.4, 0.4 * 0.25 * 100.0),
        None,
    )
}

/// Afterglow: a big synthwave lead, a seven-voice saw stack with a pulse, chorus and a long echo.
fn afterglow() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Spread", 0.50),
            ("Rise", 0.00),
            ("Octave", 0.20),
            ("Space", 0.60),
            ("Chop", 0.00),
        ],
        midi: [2.0, 2.0, 90.0],
        oscs: vec![
            osc(2.0, 0.0, 0.5).unison(7.0, 22.0),
            osc(2.0, 1.0, 0.12).unison(3.0, 18.0),
            osc(3.0, 0.0, 0.2).pw(45.0),
        ],
        drive: [4.0, -2.0],
        lad: [1800.0, 0.12, 0.0],
        fenv: [10.0, 500.0, 0.5, 250.0],
        fenv_amt: 0.10,
        wheel_amt: 0.0,
        aenv: [10.0, 300.0, 0.8, 350.0],
        trim_db: 16.0,
    });
    let (fenv, mix, o0, o1) = (v.fenv, v.mix, v.oscs[0], v.oscs[1]);
    let cut = routed("filter.ladder", "cutoff_hz", 1800.0, 0.4, 0.55);
    v.r.e.set_param(v.lad, "cutoff_hz", cut);
    // The old Vibrato stays at its stored depth.
    fixed_vibrato(&mut v, 5.2, 0.3 * 0.08);
    // Spread: both saw stacks open from a tight stack to a loose, huge one.
    let wide = routed("osc.va", "detune", 22.0, 0.35, 0.5);
    span(&mut v, 1, 0.5, o0, "osc.va", "detune", 6.0, wide);
    span(&mut v, 1, 0.5, o1, "osc.va", "detune", 6.0, 18.0);
    // Rise: the filter starts closed and opens into each note slowly, from a flat start to a long swell.
    v.snap(2, 0.0, 0.6);
    let lad = v.lad;
    rise(&mut v, 2, lad, "filter.ladder", "cutoff_hz", cut, 500.0);
    rise(&mut v, 2, fenv, "env.adsr", "attack_ms", 10.0, 1200.0);
    // Octave: the stack an octave up comes forward.
    span(&mut v, 3, 0.2, mix, "mixer", "level2", 0.0, 0.12);
    v.stage(
        "chorus",
        &[("rate_hz", 0.4), ("depth", 40.0), ("mix", 35.0)],
    );
    v.stage("compressor", &compressor(-24.0, 2.0, 15.0, 150.0, 2.0));
    // Chop: a gate on the host clock, off at 0, up to hard sixteenth-note chops at 1 (the echo and
    // room still ring through the gaps). The edges are 3 ms and 12 ms envelopes: no clicks.
    // Press Run; with Chop up and the clock stopped the gate stays shut.
    let clock = v.r.add("clock", &[("bpm", 124.0), ("swing", 0.0)]);
    let gate = v.r.add("env.adsr", &adsr(3.0, 100.0, 1.0, 12.0));
    v.r.wire(clock, "gate", gate, "gate");
    v.r.depth_route(gate, "out", v.mono, "gain_db", v.knob, "m5", 0.5);
    v.r.route(v.knob, "m5", v.mono, "gain_db", -0.5);
    // The echo stays free-running (the clock is patched in for the gate only).
    let mut e = echo(375.0, 42.0, 4500.0).to_vec();
    e.push(("sync", 0.0));
    v.finish("m4", &e, 0.4, (3.2, 0.35), Some(clock))
}

/// Horn Section: a synth brass stab for funk and disco, a saw stack with a swelling filter.
fn horn_section() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Blat", 0.50),
            ("Section", 0.40),
            ("Blare", 0.30),
            ("Vibrato", 0.10),
        ],
        midi: [1.0, 0.0, 120.0],
        oscs: vec![
            osc(2.0, 0.0, 0.5).unison(3.0, 12.0),
            osc(2.0, 0.0, 0.35).fine(-7.0),
        ],
        drive: [6.0, -3.0],
        lad: [500.0, 0.15, 2.0],
        fenv: [60.0, 260.0, 0.55, 120.0],
        // The recipe's .15 plus the old Blat's .4 x .5 at its stored position.
        fenv_amt: 0.35,
        wheel_amt: 0.0,
        aenv: [18.0, 220.0, 0.85, 100.0],
        trim_db: 13.0,
    });
    let (fenv, o0, o1, drive) = (v.fenv, v.oscs[0], v.oscs[1], v.drive);
    v.r.route(v.m, "velocity", v.lad, "cutoff_hz", 0.12);
    let cut = routed("filter.ladder", "cutoff_hz", 500.0, 0.4, 0.5);
    v.r.e.set_param(v.lad, "cutoff_hz", cut);
    // Blat: the swell at the front of each note gets deeper and slower (a pip to a blare).
    v.snap(1, 0.5, 0.5);
    span(&mut v, 1, 0.5, fenv, "env.adsr", "attack_ms", 20.0, 60.0);
    // Section: several players, from one tight horn to a loose section.
    span(&mut v, 2, 0.4, o0, "osc.va", "detune", 2.0, 12.0);
    span(&mut v, 2, 0.4, o1, "osc.va", "fine", -1.0, -7.0);
    // Blare: the drive adds the brassy edge.
    span(&mut v, 3, 0.3, drive, "drive", "drive_db", 0.0, 6.0);
    // Vibrato: fades in and speeds up.
    let lfo = v.vibrato("m4", 5.0, 0.14);
    span_hi(&mut v, 4, 0.1, lfo, "lfo", "rate_hz", 5.0, 7.0);
    v.stage("compressor", &compressor(-24.0, 3.0, 8.0, 100.0, 3.0));
    // The old Space stays at its stored level.
    v.finish(
        "",
        &echo(210.0, 25.0, 4000.0),
        0.25 * 0.2 * 100.0,
        (0.7, 0.25 * 0.2 * 100.0),
        None,
    )
}

/// Fire Escape: a house stab lead, two unison saw stacks with a pitch dive on every note.
fn fire_escape() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Dive", 0.50),
            ("Stack", 0.50),
            ("Length", 0.50),
            ("Space", 0.40),
        ],
        midi: [1.0, 0.0, 120.0],
        oscs: vec![
            osc(2.0, 0.0, 0.45).unison(8.0, 36.0),
            osc(2.0, 0.0, 0.35).unison(4.0, 22.0).fine(5.0),
        ],
        drive: [6.0, -3.0],
        lad: [1200.0, 0.15, 2.0],
        fenv: [4.0, 180.0, 0.3, 100.0],
        fenv_amt: 0.12,
        wheel_amt: 0.0,
        aenv: [3.0, 260.0, 0.8, 110.0],
        trim_db: 13.0,
    });
    let (fenv, aenv, o0, o1) = (v.fenv, v.aenv, v.oscs[0], v.oscs[1]);
    // The old Cutoff and Edge stay where they were stored.
    let cut = routed("filter.ladder", "cutoff_hz", 1200.0, 0.4, 0.5);
    v.r.e.set_param(v.lad, "cutoff_hz", cut);
    let d = routed("drive", "drive_db", 6.0, 0.5, 0.3);
    v.r.e.set_param(v.drive, "drive_db", d);
    // Dive: how far each note falls into its pitch, and for how long.
    for o in [o0, o1] {
        v.r.depth_route(fenv, "out", o, "fine", v.knob, "m1", 0.6);
    }
    span(&mut v, 1, 0.5, fenv, "env.adsr", "decay_ms", 40.0, 180.0);
    // Stack: the two unison stacks spread from a tight chord to a wall.
    span(&mut v, 2, 0.5, o0, "osc.va", "detune", 6.0, 36.0);
    span(&mut v, 2, 0.5, o1, "osc.va", "detune", 4.0, 22.0);
    // Length: a short stab to a held one.
    span(&mut v, 3, 0.5, aenv, "env.adsr", "decay_ms", 60.0, 260.0);
    span(&mut v, 3, 0.5, aenv, "env.adsr", "release_ms", 25.0, 110.0);
    v.stage(
        "chorus",
        &[("rate_hz", 0.4), ("depth", 35.0), ("mix", 25.0)],
    );
    v.stage("compressor", &compressor(-24.0, 3.0, 8.0, 100.0, 3.0));
    v.finish("m4", &echo(352.0, 40.0, 4500.0), 0.35, (1.2, 0.2), None)
}

/// Short Circuit: a metallic lead for French touch, a saw ring-modulated by a square a fifth up.
fn short_circuit() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Metal", 0.50),
            ("Crush", 0.35),
            ("Snap", 0.50),
            ("Siren", 0.00),
        ],
        midi: [1.0, 0.0, 120.0],
        oscs: vec![
            osc(2.0, 0.0, 0.45),
            osc(3.0, 0.0, 0.0).pw(50.0),
            osc(2.0, 7.0 / 12.0, 0.0),
        ],
        drive: [14.0, -7.0],
        lad: [1800.0, 0.20, 4.0],
        fenv: [2.0, 120.0, 0.2, 60.0],
        fenv_amt: 0.15,
        wheel_amt: 0.0,
        aenv: [2.0, 170.0, 0.8, 60.0],
        trim_db: 12.0,
    });
    let ring = v.r.add("ringmod", &[]);
    v.r.wire(v.oscs[1], "out", ring, "a");
    v.r.wire(v.oscs[2], "out", ring, "b");
    v.r.wire(ring, "out", v.mix, "in4");
    let (mix, fenv, partner) = (v.mix, v.fenv, v.oscs[2]);
    let cut = routed("filter.ladder", "cutoff_hz", 1800.0, 0.4, 0.5);
    v.r.e.set_param(v.lad, "cutoff_hz", cut);
    // Metal: the ring modulator comes in on the saw.
    span(&mut v, 1, 0.5, mix, "mixer", "level4", 0.0, 0.35);
    // Crush: the bitcrusher mixes in and gets coarser.
    let crush = v.stage(
        "bitcrusher",
        &[("bits", 6.0), ("downsample", 3.0), ("mix", 0.0)],
    );
    let cmix = routed("bitcrusher", "mix", 0.0, 0.7, 0.35);
    span(&mut v, 2, 0.35, crush, "bitcrusher", "mix", 0.0, cmix);
    span_hi(&mut v, 2, 0.35, crush, "bitcrusher", "bits", 6.0, 5.0);
    span_hi(&mut v, 2, 0.35, crush, "bitcrusher", "downsample", 3.0, 5.0);
    // Snap: the filter snap of each note, a soft click to a zap.
    v.snap(3, 0.5, 0.25);
    span(&mut v, 3, 0.5, fenv, "env.adsr", "decay_ms", 40.0, 120.0);
    // Siren: the ring partner wobbles in pitch (0 at the stored position), from a drift to a siren.
    let lfo = v.r.add("lfo", &[("rate_hz", 0.5)]);
    v.r.depth_route(lfo, "out", partner, "fine", v.knob, "m4", 0.5);
    rise(&mut v, 4, lfo, "lfo", "rate_hz", 0.5, 9.0);
    v.stage("compressor", &compressor(-26.0, 6.0, 2.0, 80.0, 6.0));
    // The old Room stays at its stored level.
    v.finish(
        "",
        &echo(110.0, 20.0, 3000.0),
        0.25 * 0.25 * 100.0,
        (0.4, 0.25 * 0.15 * 100.0),
        None,
    )
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
    // Off-beat stabs over the duck, which Run starts at 0.5 s.
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
            description: "A blunt mono bass for French touch: two saws over a square sub, 18 dB of drive and a compressor squeezing it flat, then ducked on every beat by a silent envelope on the host clock (no drum is built in). Press Run to start the clock, then play short off-beat notes; the mod wheel opens the filter. Bite opens the filter and sharpens the snap of every note, Weight trades the saw for the square sub, Squash clamps the compressor harder, Pump sets how deep the beat ducks the bass and how long it takes to swell back.",
            listen: "The bass snapping shut on every beat and swelling back between beats; Pump at 0 plays the bass flat, at 1 it pumps hard. Weight up for a heavier sub, Squash up for a flatter, denser bass. Silent until Run.",
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
            description: "A trap 808: a pure sine with a small pitch snap and a long, round body, sliding from note to note. Play long low notes and overlap them to slide. Thump sets how hard the pitch snaps and how punchy the attack is at the front of each note (soft at 0, a click at 1), Body lengthens the round tail, Dirt drives it so it speaks on small speakers, Slide sets how long the slide takes.",
            listen: "The thump on each attack, the long sine body, and the slide between notes; Thump up for a harder kick, Dirt up makes it grit on small speakers.",
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
            description: "A legato acid bass: a saw over a square sub into a resonant ladder filter that snaps on every note, with a short slide between overlapped notes. Play a sixteenth-note line; velocity accents the filter. Squelch opens the filter and raises the resonance, Accent goes from short clipped notes to long held ones, Slide sets how long the slide between notes takes, Space makes the dotted echo repeat more and more.",
            listen: "The filter snapping on each note and the slides between overlapped ones; Squelch up for the acid bite, Accent up for held notes, Slide up for long swoops.",
            build: acid_rain,
            group: 1,
            phrase: acid_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "oatmeal",
            name: "Oatmeal",
            category: "Bass",
            tags: &["mono", "clean", "round", "pop"],
            description: "A clean, round pop bass: a triangle over a sine an octave down, a soft pluck in the low-pass and a light compressor. It sits under anything. Warmth adds a soft saturation, Soften turns the pop at the start of each note into a slow swell, Throb fades in a slow tremolo, Squeeze clamps the compressor harder.",
            listen: "The round, clean low end; Warmth up for harmonics, Soften up for a slow swell on each note, Throb up for a pulsing level.",
            build: soft_shoe,
            group: 1,
            phrase: shoe_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "spun-sugar",
            name: "Spun Sugar",
            category: "Lead",
            tags: &["mono", "legato", "smooth", "chorus", "pop"],
            description: "A smooth, vocal pop lead: a detuned saw stack over a triangle, a ladder filter that opens with velocity, light chorus and a dotted echo. Play legato; notes glide. Tone brightens, Vibrato fades in a vibrato that speeds up as it deepens, Shimmer widens the chorus and spreads the saw stack, Space adds the echo and a room.",
            listen: "The even, vocal tone and the glide between overlapped notes; raise Vibrato for the long notes, Shimmer for a choir.",
            build: satin_ribbon,
            group: 1,
            phrase: ribbon_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "pig-iron",
            name: "Pig Iron",
            category: "Lead",
            tags: &["mono", "distorted", "bitcrushed", "compressed", "french touch", "signature"],
            description: "A hard, crushed lead for French touch: a narrow pulse and a saw into 16 dB of drive, a bitcrusher and a compressor, retriggered on every note. Play short syncopated lines. Bite opens the filter and sharpens the snap of every note, Weld detunes the saw against the pulse and thins the pulse, Crush mixes in the bitcrusher and makes it coarser, Length holds each note longer.",
            listen: "The blunt attack and the crushed edge; Crush up for the digital grind, Crush down for a plain hard lead, Length up for a held blare.",
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
            description: "A legato acid lead: a saw and a narrow pulse into a resonant ladder filter with velocity accents and a dotted echo. Play a sixteenth-note line and overlap notes to slide. Squelch opens the filter and raises the resonance, Slide sets how long the slide between notes takes, Wire sets a pulse-width sweep on the second oscillator, speeds it up and thickens the pulse, Space adds the echo and a room.",
            listen: "The filter snap, the slides, and the dotted echo; Slide up for long swoops, Wire up for a moving, thicker pulse.",
            build: hot_wire,
            group: 1,
            phrase: wire_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "lantern",
            name: "Lantern",
            category: "Lead",
            tags: &["mono", "legato", "soft", "slow", "ambient", "cinematic"],
            description: "A soft ambient lead that swells in: a triangle stack with a sine an octave up, chorus, a long echo and a big room. Play long, slow notes. Glow opens the filter and brings in the octave of sine, Swell lengthens the fade-in and the fade-out of each note, Drift adds a slow vibrato and lets the chorus swim, Space is the echo and room.",
            listen: "Each note blooming out of silence and the long tail carrying into the next; Swell up for a slower bloom.",
            build: paper_crane,
            group: 1,
            phrase: crane_phrase,
            secs: 14.0,
        },
        Preset {
            slug: "rubber-band",
            name: "Rubber Band",
            category: "Bass",
            tags: &["mono", "plucked", "slap", "funk", "disco"],
            description: "A funky plucked bass: a narrow pulse over a saw an octave down, a fast filter slap on every note and a light compressor. Play 16th-note grooves; harder notes open the filter more. Room adds a short slapback and room, Slap puts resonance on the filter snap of each note, from a plain pop to a wah, Stab goes from short and dry to long and ringing, Body brings the saw an octave down forward of the pulse.",
            listen: "The rubbery snap on each note and the ghost notes in the groove; Slap up for a wah on each note, Stab down for staccato.",
            build: rubber_band,
            group: 2,
            phrase: rubber_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "slate-roof",
            name: "Slate Roof",
            category: "Bass",
            tags: &["mono", "neuro", "unison", "distorted", "drum and bass"],
            description: "A neuro bass for drum and bass: a four-voice saw stack against a detuned saw over a square sub, 14 dB of drive and a compressor. Play long half-time notes; the mod wheel opens the filter. Cutoff opens the filter, Growl fades in a rolling filter movement and speeds it up, Stack spreads the saw stack, Room adds a tight room and slapback.",
            listen: "The thick, dark stack; Growl up for a rolling filter, Stack up for a wall.",
            build: slate_roof,
            group: 2,
            phrase: slate_phrase,
            secs: 12.0,
        },
        Preset {
            slug: "gutter",
            name: "Gutter",
            category: "Bass",
            tags: &["mono", "distorted", "bitcrushed", "compressed", "french touch", "signature"],
            description: "A dirty mid-range bass for French touch: a saw and a square sub into 24 dB of drive, a light bitcrusher and an 8:1 compressor, with a pulse an octave up to bring in. Play short, chugging notes. Bite opens the filter and sharpens the snap of every note, Crush bites the bitcrusher into the drive and makes it coarser, Octave brings in the pulse an octave up, Squash clamps the compressor harder.",
            listen: "The compressed, flattened chug; Octave up for a buzzing top that cuts through a mix, Squash up for a flatter chug.",
            build: gutter,
            group: 2,
            phrase: gutter_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "low-tide",
            name: "Low Tide",
            category: "Bass",
            tags: &["mono", "legato", "sub", "slow", "ambient", "cinematic"],
            description: "An ambient sub bass that swells in: a sine over a triangle an octave down, a slow filter tide, chorus, a long echo and a big room. Play long, slow notes; they glide into each other. Tone opens the filter and brings in the triangle, Tide fades in a slow filter movement and speeds it up, Swell lengthens the fade-in and the fade-out of each note, Space is the echo and the room.",
            listen: "Each note rising out of silence, the slow filter tide and the long tail under the next note.",
            build: low_tide,
            group: 2,
            phrase: tide_phrase,
            secs: 15.0,
        },
        Preset {
            slug: "copper-wire",
            name: "Copper Wire",
            category: "Bass",
            tags: &["mono", "ring mod", "metallic", "experimental", "modular"],
            description: "An experimental metal bass: a sine ring-modulated by a triangle a fifth up, which adds inharmonic bell tones above a clean sub. Play slow, low notes. Metal brings in the ring modulator, Weight thickens the clean sine under the bells, Hold goes from a short plucked sine to a long ringing drone, Space adds an echo and a room that keep the bells ringing.",
            listen: "The bell-like metal appearing around a clean sub; Metal at 0 is a plain sine bass, Hold up for a long drone under the bells, Weight up for a heavier sub.",
            build: copper_wire,
            group: 2,
            phrase: copper_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "tail-light",
            name: "Tail Light",
            category: "Bass",
            tags: &["mono", "chorus", "pulsing", "synthwave", "80s"],
            description: "A synthwave bass: two saws and a square sub with a medium pluck and a wide chorus. Play steady eighth notes. Pluck deepens and lengthens the filter snap of each note, Width brings in the chorus and spreads the saws, Weight trades a saw for the square sub, Space adds a dotted echo and a room.",
            listen: "The pluck on each eighth note and the chorus widening it; Width at 0 is a plain saw bass, Pluck up for a slow filter open, Weight up for a heavier sub.",
            build: tail_light,
            group: 2,
            phrase: tail_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "rail-yard",
            name: "Rail Yard",
            category: "Bass",
            tags: &["mono", "sidechain", "124bpm", "house"],
            description: "A house bass that ducks on every beat: a saw and a square sub through a compressor, pulled down by a silent envelope on the host clock (no drum is built in). Press Run for the pump, then play off-beat notes; the mod wheel opens the filter. Pump sets how deep the beat ducks the bass and how long it takes to swell back, Snap deepens and lengthens the filter bite of each note, Thick spreads the saw and adds drive, Room adds a synced echo and a room.",
            listen: "The bass breathing on every beat; Pump at 0 plays flat, at 1 it pumps hard and swells back over the whole beat. Silent until Run.",
            build: rail_yard,
            group: 2,
            phrase: rail_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "oak-stave",
            name: "Oak Stave",
            category: "Bass",
            tags: &["mono", "plucked", "upright", "acoustic", "world"],
            description: "An upright-style plucked bass: a triangle with a sine body, a soft pitch snap and a filter that closes after each pluck. Play walking lines. Pluck deepens and lengthens the filter snap, Thump adds a pitch kick at the front of the note, Body brings in the sine and lets the note ring, Room adds a short room.",
            listen: "The woody pluck and the short ring of each note; Thump up for a plucked-string kick, Body up for a rounder, longer note.",
            build: oak_stave,
            group: 2,
            phrase: oak_phrase,
            secs: 10.0,
        },
        Preset {
            slug: "glass-needle",
            name: "Glass Needle",
            category: "Lead",
            tags: &["mono", "legato", "pure", "sine", "modular", "experimental"],
            description: "A pure modular lead: a sine with an octave of overtone and a ping-pong echo. Play slow, high lines; notes glide. Overtone brings in the octave above, Ping adds a pitch kick at the front of each note, Vibrato fades in a vibrato that speeds up as it deepens, Space adds the echo and the room.",
            listen: "The thin, clear sine and the ping-pong echo trailing each note; Overtone up for a glassy ring, Ping up for a needle-tap on every note.",
            build: glass_needle,
            group: 2,
            phrase: needle_phrase,
            secs: 11.0,
        },
        Preset {
            slug: "loud-mouth",
            name: "Loud Mouth",
            category: "Lead",
            tags: &["mono", "formant", "talking", "distorted", "french touch", "signature"],
            description: "A talking lead for French touch: a saw and a pulse driven hard into a formant filter that sounds like a mouth. Play short riffs; the mod wheel moves the vowel. Vowel sets the vowel from a to u, Talk sets how much and how fast the vowel drifts under a held note, Shift makes the mouth smaller or larger, Shout pushes the drive and sharpens the formants.",
            listen: "The vowel moving under the riff; Talk up for a fast babble, Shout up for a louder, nastier mouth.",
            build: loud_mouth,
            group: 2,
            phrase: mouth_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "velvet-sine",
            name: "Velvet Sine",
            category: "Lead",
            tags: &["mono", "legato", "soft", "sine", "vibrato", "echo"],
            description: "A soft, round lead: a sine with a hint of triangle, a short legato glide between overlapped notes, a vibrato and a dotted echo. Play simple, smooth melodies. Glide sets how long the glide takes, from a step to a short slide, Body turns the pure sine into a rounder, hollow tone, Bloom makes each note swell in and fade out slowly, Vibrato fades in a vibrato that speeds up.",
            listen: "The pure, soft tone and the short glide between notes; Body up for a hollow warmth, Bloom up for a swelling, pad-like lead.",
            build: velvet_sine,
            group: 2,
            phrase: devil_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "afterglow",
            name: "Afterglow",
            category: "Lead",
            tags: &["mono", "legato", "unison", "chorus", "synthwave", "80s"],
            description: "A big synthwave lead: a seven-voice saw stack with an octave stack and a pulse, chorus, a long echo and a big room. Play slow, soaring lines; notes glide. Spread opens the saw stacks from tight to huge, Rise opens the filter slowly into each note, Octave brings in the stack an octave up, Space adds the echo and the room, Chop gates the lead on the host clock from off to hard sixteenth-note chops (press Run).",
            listen: "The wide stack and the long echo; Spread up for a bigger, looser sound, Rise up for a slow swell into each note, Chop up for a trance gate once Run is on.",
            build: afterglow,
            group: 2,
            phrase: afterglow_phrase,
            secs: 10.0,
        },
        Preset {
            slug: "horn-section",
            name: "Horn Section",
            category: "Lead",
            tags: &["mono", "brass", "stabs", "funk", "disco"],
            description: "A synth brass lead for funk and disco: a three-voice saw stack with a second saw, a filter that swells open on every note and a light compressor. Play short stabs; harder notes are brighter. Blat deepens and slows the filter swell on each note, Section loosens the players from one tight horn to a section, Blare adds the brassy drive, Vibrato fades in a vibrato that speeds up.",
            listen: "The swell at the start of each stab; Blat up for a bigger blare, Section up for a loose horn section.",
            build: horn_section,
            group: 2,
            phrase: horn_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "fire-escape",
            name: "Fire Escape",
            category: "Lead",
            tags: &["mono", "unison", "stabs", "dive", "house"],
            description: "A house stab lead: two unison saw stacks with a pitch dive on every note, chorus and a dotted echo. Play short, bright stabs. Dive sets how far each note falls into its pitch and for how long, Stack spreads the two saw stacks from a tight chord to a wall, Length goes from a short stab to a held one, Space adds the echo and the room.",
            listen: "The pitch falling into each stab; Dive at 0 is a plain unison stab, Stack up for a wall, Length up for held chords.",
            build: fire_escape,
            group: 2,
            phrase: fire_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "short-circuit",
            name: "Short Circuit",
            category: "Lead",
            tags: &["mono", "ring mod", "bitcrushed", "distorted", "french touch", "signature"],
            description: "A metallic lead for French touch: a saw ring-modulated by a square a fifth up, 14 dB of drive, a bitcrusher and a heavy compressor. Play short, jagged lines. Metal brings in the ring modulator, Crush mixes in the bitcrusher and makes it coarser, Snap sets the filter snap of each note from a soft click to a zap, Siren wobbles the pitch of the ringed square.",
            listen: "The metal on top of the saw and the crushed edge; Metal at 0 is a plain hard saw lead, Siren up for a wobbling alarm.",
            build: short_circuit,
            group: 2,
            phrase: circuit_phrase,
            secs: 9.0,
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
            for p in presets().into_iter().filter(|p| only(p)) {
                let log = kabl_core::load(&Path::new(&args[2]).join("presets").join(p.slug)).unwrap();
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
            for p in presets().into_iter().filter(|p| only(p)) {
                println!("    (\n        \"presets/{}\",\n        {:?},\n        {:?},\n        &{:?},\n        {:?},\n    ),", p.slug, p.name, p.category, p.tags, p.description);
            }
        }
        _ => eprintln!("usage: presets_bass_lead write|clips|levels|macros|library <patches dir> [out dir] [group]"),
    }
}
