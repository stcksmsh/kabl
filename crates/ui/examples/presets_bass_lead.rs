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
    // Off-beat notes over the grid's kick, which Run starts at 0.5 s.
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
        oscs: vec![osc(2.0, 0.0, 0.55).unison(2.0, 8.0), osc(3.0, -1.0, 0.5)],
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
    let beat = v.r.add("mixer", &[("level1", 0.45)]);
    v.r.wire(kick, "out", beat, "in1");
    finish_drums(
        v,
        duck,
        clock,
        beat,
        &[
            ("sync", 3.0),
            ("time_ms", 360.0),
            ("feedback", 30.0),
            ("mix", 0.0),
            ("tone_hz", 3500.0),
        ],
        (0.22, 0.45, 0.16),
    )
}

/// The tail of a bass that plays with a drum bus: echo and room on macro 4, the drum bus joined
/// after them (so the drums are not ducked by their own kick), the output.
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
        oscs: vec![osc(0.0, -1.0, 0.7), osc(1.0, -1.0, 0.15)],
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
        oscs: vec![osc(2.0, 0.0, 0.6), osc(3.0, -1.0, 0.3)],
        drive: [7.0, -3.5],
        lad: [220.0, 0.28, 2.0],
        fenv: [6.0, 220.0, 0.0, 100.0],
        fenv_amt: 0.16,
        wheel_amt: 0.3,
        aenv: [3.0, 250.0, 0.7, 40.0],
        trim_db: 9.0,
    });
    v.r.route(v.m, "velocity", v.lad, "cutoff_hz", 0.03);
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.45);
    v.r.route(v.knob, "m2", v.lad, "resonance", 0.35);
    v.r.route(v.knob, "m4", v.drive, "drive_db", 0.5);
    v.stage("compressor", &compressor(-24.0, 3.0, 5.0, 100.0, 4.0));
    v.finish("m3", &echo(352.0, 45.0, 4000.0), 0.4, (0.9, 0.2), None)
}

/// Oatmeal: a clean round pop bass, a triangle over a sine, a soft pluck in the filter.
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
        oscs: vec![osc(1.0, 0.0, 0.5), osc(0.0, -1.0, 0.6)],
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

/// Spun Sugar: a smooth singing pop lead, a detuned saw stack over a triangle, chorus.
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

/// Pig Iron: a hard, crushed, hard-retriggered lead, a pulse and a saw into heavy drive.
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
        oscs: vec![osc(3.0, 0.0, 0.5).pw(35.0), osc(2.0, 0.0, 0.45).fine(7.0)],
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
        oscs: vec![osc(2.0, 0.0, 0.6), osc(3.0, 0.0, 0.3).pw(40.0).fine(4.0)],
        drive: [8.0, -4.0],
        lad: [600.0, 0.32, 2.0],
        fenv: [4.0, 200.0, 0.25, 120.0],
        fenv_amt: 0.15,
        wheel_amt: 0.0,
        aenv: [3.0, 250.0, 0.75, 120.0],
        trim_db: 12.0,
    });
    v.r.route(v.m, "velocity", v.lad, "cutoff_hz", 0.05);
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.4);
    v.r.route(v.knob, "m2", v.lad, "resonance", 0.38);
    let pwm = v.r.add("lfo", &[("rate_hz", 0.7)]);
    v.r.depth_route(pwm, "out", v.oscs[1], "pw", v.knob, "m4", 0.35);
    v.stage("compressor", &compressor(-24.0, 3.0, 5.0, 100.0, 4.0));
    v.finish("m3", &echo(352.0, 45.0, 4500.0), 0.4, (1.0, 0.2), None)
}

/// Lantern: a soft ambient lead that swells in, a triangle stack and a sine an octave up.
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
        oscs: vec![osc(1.0, 0.0, 0.45).unison(2.0, 6.0), osc(0.0, 1.0, 0.2)],
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

// ---- group 2 --------------------------------------------------------------------------------

/// Rubber Band: a funky plucked bass, a narrow pulse over a saw, a quick filter slap.
fn rubber_band() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Tone", 0.50),
            ("Slap", 0.55),
            ("Grit", 0.20),
            ("Room", 0.20),
        ],
        midi: [1.0, 0.0, 120.0],
        oscs: vec![osc(3.0, 0.0, 0.45).pw(30.0), osc(2.0, -1.0, 0.45)],
        drive: [6.0, -3.0],
        lad: [250.0, 0.20, 2.0],
        fenv: [3.0, 90.0, 0.05, 60.0],
        fenv_amt: 0.12,
        wheel_amt: 0.2,
        aenv: [2.0, 260.0, 0.35, 70.0],
        trim_db: 9.0,
    });
    v.r.route(v.m, "velocity", v.lad, "cutoff_hz", 0.1);
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.45);
    v.r.depth_route(v.fenv, "out", v.lad, "cutoff_hz", v.knob, "m2", 0.6);
    v.r.route(v.knob, "m3", v.drive, "drive_db", 0.5);
    v.stage("compressor", &compressor(-22.0, 4.0, 6.0, 90.0, 5.0));
    v.finish("m4", &echo(120.0, 18.0, 3500.0), 0.12, (0.5, 0.15), None)
}

/// Slate Roof: a neuro bass, a unison saw stack against a detuned saw over a sub, with a growl.
fn slate_roof() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Cutoff", 0.45),
            ("Growl", 0.35),
            ("Crush", 0.20),
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
        fenv: [4.0, 300.0, 0.2, 80.0],
        fenv_amt: 0.2,
        wheel_amt: 0.25,
        aenv: [3.0, 200.0, 0.85, 50.0],
        trim_db: 9.0,
    });
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.45);
    let growl = v.r.add("lfo", &[("rate_hz", 3.2)]);
    v.r.depth_route(growl, "out", v.lad, "cutoff_hz", v.knob, "m2", 0.25);
    let crush = v.stage(
        "bitcrusher",
        &[("bits", 6.0), ("downsample", 4.0), ("mix", 0.0)],
    );
    v.r.route(v.knob, "m3", crush, "mix", 0.9);
    v.stage("compressor", &compressor(-24.0, 6.0, 3.0, 90.0, 6.0));
    v.finish("m4", &echo(100.0, 18.0, 3500.0), 0.12, (0.4, 0.12), None)
}

/// Gutter: a dirty mid bass for French touch, a saw and a sub into 24 dB of drive, an octave pulse.
fn gutter() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Bite", 0.45),
            ("Grit", 0.65),
            ("Octave", 0.30),
            ("Room", 0.25),
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
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.45);
    v.r.route(v.knob, "m2", v.drive, "drive_db", 0.3);
    v.r.route(v.knob, "m3", v.mix, "level3", 0.6);
    v.stage(
        "bitcrusher",
        &[("bits", 8.0), ("downsample", 2.0), ("mix", 10.0)],
    );
    v.stage("compressor", &compressor(-28.0, 8.0, 1.0, 80.0, 14.0));
    v.finish("m4", &echo(95.0, 20.0, 3500.0), 0.22, (0.4, 0.16), None)
}

/// Low Tide: an ambient sub, a sine over a triangle, a slow swell and a slow tide in the filter.
fn low_tide() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Tone", 0.45),
            ("Tide", 0.35),
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
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.45);
    let tide = v.r.add("lfo", &[("rate_hz", 0.15)]);
    v.r.depth_route(tide, "out", v.lad, "cutoff_hz", v.knob, "m2", 0.3);
    v.r.route(v.knob, "m3", v.aenv, "attack_ms", 0.4);
    v.stage(
        "chorus",
        &[("rate_hz", 0.2), ("depth", 30.0), ("mix", 25.0)],
    );
    v.stage("compressor", &compressor(-24.0, 2.0, 30.0, 300.0, 2.0));
    v.finish("m4", &echo(600.0, 45.0, 3000.0), 0.35, (7.0, 0.55), None)
}

/// Copper Wire: an experimental metal bass, a sine ring-modulated by a triangle a fifth up.
fn copper_wire() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Tone", 0.50),
            ("Metal", 0.45),
            ("Grit", 0.20),
            ("Room", 0.25),
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
    v.r.e.set_param(v.mix, "level3", 0.0);
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.45);
    v.r.route(v.knob, "m2", v.mix, "level3", 0.7);
    v.r.route(v.knob, "m3", v.drive, "drive_db", 0.5);
    v.stage("compressor", &compressor(-24.0, 3.0, 8.0, 100.0, 4.0));
    v.finish("m4", &echo(210.0, 35.0, 4500.0), 0.3, (0.8, 0.25), None)
}

/// Tail Light: a synthwave bass, two saws and a sub with a medium pluck and a chorus.
fn tail_light() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Cutoff", 0.50),
            ("Drive", 0.30),
            ("Chorus", 0.40),
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
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.45);
    v.r.route(v.knob, "m2", v.drive, "drive_db", 0.5);
    let chorus = v.stage("chorus", &[("rate_hz", 0.5), ("depth", 40.0), ("mix", 0.0)]);
    v.r.route(v.knob, "m3", chorus, "mix", 0.5);
    v.stage("compressor", &compressor(-24.0, 3.0, 8.0, 100.0, 4.0));
    v.finish("m4", &echo(352.0, 38.0, 4000.0), 0.3, (1.8, 0.3), None)
}

/// Rail Yard: a house bass ducked by its own drum grid (kick, clap, hats); press Run.
fn rail_yard() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Cutoff", 0.50),
            ("Drive", 0.30),
            ("Pump", 0.55),
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
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.45);
    v.r.route(v.knob, "m2", v.drive, "drive_db", 0.4);
    v.stage("compressor", &compressor(-24.0, 4.0, 3.0, 90.0, 6.0));
    let clock = v.r.add("clock", &[("bpm", 124.0), ("swing", 20.0)]);
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
    // The drum grid plays its default beat: kick, clap and closed hat.
    let grid = v.r.add("drum.grid", &[]);
    v.r.wire(clock, "gate", grid, "clock");
    v.r.wire(clock, "reset", grid, "reset");
    let kick = v.r.add(
        "drum.kick",
        &[("level_db", -3.0), ("decay_ms", 260.0), ("drive_db", 6.0)],
    );
    let clap = v.r.add("drum.clap", &[("level_db", -9.0)]);
    let hat = v.r.add("drum.hat", &[("level_db", -11.0)]);
    v.r.wire(grid, "kick", kick, "gate");
    v.r.wire(grid, "clap", clap, "gate");
    v.r.wire(grid, "chat", hat, "closed");
    v.r.wire(kick, "out", duck, "sc");
    let beat = v.r.add(
        "mixer",
        &[("level1", 0.45), ("level2", 0.2), ("level3", 0.15)],
    );
    v.r.wire(kick, "out", beat, "in1");
    v.r.wire(clap, "out", beat, "in2");
    v.r.wire(hat, "out", beat, "in3");
    finish_drums(
        v,
        duck,
        clock,
        beat,
        &[
            ("sync", 3.0),
            ("time_ms", 360.0),
            ("feedback", 30.0),
            ("mix", 0.0),
            ("tone_hz", 3500.0),
        ],
        (0.2, 0.5, 0.16),
    )
}

/// Oak Stave: an upright-style plucked bass, a triangle with a sine body and a soft pitch snap.
fn oak_stave() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: true,
        macros: &[
            ("Tone", 0.50),
            ("Pluck", 0.50),
            ("Body", 0.40),
            ("Room", 0.30),
        ],
        midi: [1.0, 0.0, 120.0],
        oscs: vec![osc(1.0, 0.0, 0.5), osc(0.0, -1.0, 0.2)],
        drive: [2.0, -1.0],
        lad: [380.0, 0.10, 0.0],
        fenv: [2.0, 240.0, 0.1, 60.0],
        fenv_amt: 0.25,
        wheel_amt: 0.0,
        aenv: [2.0, 700.0, 0.15, 160.0],
        trim_db: 16.0,
    });
    v.r.route(v.fenv, "out", v.oscs[0], "fine", 0.15);
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.4);
    v.r.depth_route(v.fenv, "out", v.lad, "cutoff_hz", v.knob, "m2", 1.0);
    v.r.route(v.knob, "m3", v.mix, "level2", 0.6);
    v.stage("compressor", &compressor(-22.0, 3.0, 10.0, 120.0, 3.0));
    v.finish("m4", &echo(95.0, 18.0, 3500.0), 0.2, (0.8, 0.25), None)
}

/// Glass Needle: a pure modular lead, a sine with an octave of overtone and a ping-pong echo.
fn glass_needle() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Tone", 0.55),
            ("Overtone", 0.30),
            ("Space", 0.55),
            ("Vibrato", 0.25),
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
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.4);
    v.r.route(v.knob, "m2", v.mix, "level2", 0.5);
    v.vibrato("m4", 5.0, 0.06);
    v.stage("compressor", &compressor(-24.0, 2.0, 15.0, 150.0, 2.0));
    v.finish(
        "m3",
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
            ("Grit", 0.50),
            ("Shift", 0.50),
            ("Room", 0.25),
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
        trim_db: 17.0,
    });
    let formant = v.mono_after(
        "filter.formant",
        &[("vowel", 0.0), ("shift_st", -6.0), ("q", 1.5)],
    );
    v.r.route(v.knob, "m1", formant, "vowel", 0.9);
    v.r.route(v.m, "wheel", formant, "vowel", 0.5);
    v.r.route(v.knob, "m2", v.drive, "drive_db", 0.3);
    v.r.route(v.knob, "m3", formant, "shift_st", 0.5);
    // A gentle, fixed vowel drift so a held note keeps talking.
    let talk = v.r.add("lfo", &[("rate_hz", 2.2)]);
    v.r.route(talk, "out", formant, "vowel", 0.08);
    v.stage("compressor", &compressor(-26.0, 6.0, 2.0, 80.0, 6.0));
    v.finish("m4", &echo(110.0, 20.0, 3000.0), 0.25, (0.4, 0.15), None)
}

/// Dust Devil: a trap whistle lead, a sine with a hint of triangle, sliding between notes.
fn dust_devil() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Tone", 0.55),
            ("Slide", 0.45),
            ("Space", 0.40),
            ("Vibrato", 0.40),
        ],
        midi: [2.0, 1.0, 110.0],
        oscs: vec![osc(0.0, 0.0, 0.6), osc(1.0, 0.0, 0.2).fine(3.0)],
        drive: [2.0, -1.0],
        lad: [1400.0, 0.10, 0.0],
        fenv: [4.0, 200.0, 0.5, 100.0],
        fenv_amt: 0.05,
        wheel_amt: 0.0,
        aenv: [6.0, 200.0, 0.85, 220.0],
        trim_db: 10.0,
    });
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.4);
    v.r.route(v.knob, "m2", v.m, "glide_ms", 0.8);
    v.vibrato("m4", 5.6, 0.08);
    v.stage("compressor", &compressor(-24.0, 2.0, 15.0, 150.0, 2.0));
    v.finish("m3", &echo(310.0, 40.0, 3500.0), 0.3, (1.4, 0.25), None)
}

/// Afterglow: a big synthwave lead, a seven-voice saw stack with a pulse, chorus and a long echo.
fn afterglow() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Cutoff", 0.55),
            ("Spread", 0.50),
            ("Space", 0.60),
            ("Vibrato", 0.30),
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
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.4);
    v.r.route(v.knob, "m2", v.oscs[0], "detune", 0.35);
    v.vibrato("m4", 5.2, 0.08);
    v.stage(
        "chorus",
        &[("rate_hz", 0.4), ("depth", 40.0), ("mix", 35.0)],
    );
    v.stage("compressor", &compressor(-24.0, 2.0, 15.0, 150.0, 2.0));
    v.finish("m3", &echo(375.0, 42.0, 4500.0), 0.4, (3.2, 0.35), None)
}

/// Horn Section: a synth brass stab for funk and disco, a saw stack with a swelling filter.
fn horn_section() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Tone", 0.50),
            ("Blat", 0.50),
            ("Space", 0.25),
            ("Vibrato", 0.20),
        ],
        midi: [1.0, 0.0, 120.0],
        oscs: vec![
            osc(2.0, 0.0, 0.5).unison(3.0, 12.0),
            osc(2.0, 0.0, 0.35).fine(-7.0),
        ],
        drive: [6.0, -3.0],
        lad: [500.0, 0.15, 2.0],
        fenv: [60.0, 260.0, 0.55, 120.0],
        fenv_amt: 0.15,
        wheel_amt: 0.0,
        aenv: [18.0, 220.0, 0.85, 100.0],
        trim_db: 13.0,
    });
    v.r.route(v.m, "velocity", v.lad, "cutoff_hz", 0.12);
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.4);
    v.r.depth_route(v.fenv, "out", v.lad, "cutoff_hz", v.knob, "m2", 0.4);
    v.vibrato("m4", 5.0, 0.07);
    v.stage("compressor", &compressor(-24.0, 3.0, 8.0, 100.0, 3.0));
    v.finish("m3", &echo(210.0, 25.0, 4000.0), 0.2, (0.7, 0.2), None)
}

/// Fire Escape: a house stab lead, two unison saw stacks with a pitch dive on every note.
fn fire_escape() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Cutoff", 0.50),
            ("Dive", 0.50),
            ("Space", 0.40),
            ("Edge", 0.30),
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
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.4);
    for o in v.oscs.clone() {
        v.r.depth_route(v.fenv, "out", o, "fine", v.knob, "m2", 0.6);
    }
    v.r.route(v.knob, "m4", v.drive, "drive_db", 0.5);
    v.stage(
        "chorus",
        &[("rate_hz", 0.4), ("depth", 35.0), ("mix", 25.0)],
    );
    v.stage("compressor", &compressor(-24.0, 3.0, 8.0, 100.0, 3.0));
    v.finish("m3", &echo(352.0, 40.0, 4500.0), 0.35, (1.2, 0.2), None)
}

/// Short Circuit: a metallic lead for French touch, a saw ring-modulated by a square a fifth up.
fn short_circuit() -> Rack {
    let mut v = Voice::new(Recipe {
        bass: false,
        macros: &[
            ("Cutoff", 0.50),
            ("Metal", 0.50),
            ("Crush", 0.35),
            ("Room", 0.25),
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
        trim_db: 10.0,
    });
    let ring = v.r.add("ringmod", &[]);
    v.r.wire(v.oscs[1], "out", ring, "a");
    v.r.wire(v.oscs[2], "out", ring, "b");
    v.r.wire(ring, "out", v.mix, "in4");
    v.r.e.set_param(v.mix, "level4", 0.0);
    v.r.route(v.knob, "m1", v.lad, "cutoff_hz", 0.4);
    v.r.route(v.knob, "m2", v.mix, "level4", 0.7);
    let crush = v.stage(
        "bitcrusher",
        &[("bits", 6.0), ("downsample", 3.0), ("mix", 0.0)],
    );
    v.r.route(v.knob, "m3", crush, "mix", 0.7);
    v.stage("compressor", &compressor(-26.0, 6.0, 2.0, 80.0, 6.0));
    v.finish("m4", &echo(110.0, 20.0, 3000.0), 0.25, (0.4, 0.15), None)
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
            slug: "oatmeal",
            name: "Oatmeal",
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
            slug: "spun-sugar",
            name: "Spun Sugar",
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
            slug: "pig-iron",
            name: "Pig Iron",
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
            slug: "lantern",
            name: "Lantern",
            category: "Lead",
            tags: &["mono", "legato", "soft", "slow", "ambient", "cinematic"],
            description: "A soft ambient lead that swells in: a triangle stack with a sine an octave up, chorus, a long echo and a big room. Play long, slow notes. Tone opens the filter, Swell lengthens the fade-in of each note, Space is the echo and room, Vibrato adds a slow vibrato.",
            listen: "Each note blooming out of silence and the long tail carrying into the next.",
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
            description: "A funky plucked bass: a narrow pulse over a saw an octave down, a fast filter slap on every note and a light compressor. Play 16th-note grooves; harder notes open the filter more. Tone opens the filter, Slap deepens the filter snap on each note, Grit adds drive, Room adds a short slapback and room.",
            listen: "The rubbery snap on each note and the ghost notes in the groove; Slap up for a harder pop.",
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
            description: "A neuro bass for drum and bass: a four-voice saw stack against a detuned saw over a square sub, 14 dB of drive and a compressor. Play long half-time notes; the mod wheel opens the filter. Cutoff opens the filter, Growl fades in a rolling filter movement, Crush mixes in a bitcrusher, Room adds a tight room and slapback.",
            listen: "The thick, dark stack; Growl up for a rolling filter, Crush up for a digital edge.",
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
            description: "A dirty mid-range bass for French touch: a saw and a square sub into 24 dB of drive, a light bitcrusher and an 8:1 compressor, with a pulse an octave up to bring in. Play short, chugging notes. Bite opens the filter, Grit pushes the drive, Octave brings in the pulse an octave up, Room adds a slapback and a tight room.",
            listen: "The compressed, flattened chug; Octave up for a buzzing top that cuts through a mix.",
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
            description: "An ambient sub bass that swells in: a sine over a triangle an octave down, a slow filter tide, chorus, a long echo and a big room. Play long, slow notes; they glide into each other. Tone opens the filter, Tide fades in a slow filter movement, Swell lengthens the fade-in of each note, Space is the echo and the room.",
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
            description: "An experimental metal bass: a sine ring-modulated by a triangle a fifth up, which adds inharmonic bell tones above a clean sub. Play slow, low notes. Tone opens the filter, Metal brings in the ring modulator, Grit adds drive, Room adds an echo and a room.",
            listen: "The bell-like metal appearing around a clean sub; Metal at 0 is a plain sine bass.",
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
            description: "A synthwave bass: two saws and a square sub with a medium pluck and a wide chorus. Play steady eighth notes. Cutoff opens the filter, Drive adds drive, Chorus brings in the chorus, Space adds a dotted echo and a room.",
            listen: "The pluck on each eighth note and the chorus widening it; Chorus at 0 is a plain saw bass.",
            build: tail_light,
            group: 2,
            phrase: tail_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "rail-yard",
            name: "Rail Yard",
            category: "Bass",
            tags: &["mono", "sidechain", "drums", "124bpm", "house"],
            description: "A house bass ducked by its own drum grid: a saw and a square sub through a compressor, with a kick, clap and hats playing from the grid. Press Run for the beat, then play off-beat notes; the mod wheel opens the filter. Cutoff opens the filter, Drive adds drive, Pump sets how hard the kick ducks the bass, Room adds a synced echo and a room.",
            listen: "The bass breathing around the kick; Pump at 0 plays flat, at 1 it pumps hard.",
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
            description: "An upright-style plucked bass: a triangle with a sine body, a soft pitch snap and a filter that closes after each pluck. Play walking lines. Tone opens the filter, Pluck deepens the filter snap, Body adds the sine body under the note, Room adds a short room.",
            listen: "The woody pluck and the short ring of each note; Body up for a rounder low end.",
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
            description: "A pure modular lead: a sine with an octave of overtone and a ping-pong echo. Play slow, high lines; notes glide. Tone opens the filter, Overtone brings in the octave above, Space adds the echo and the room, Vibrato fades in a gentle vibrato.",
            listen: "The thin, clear sine and the ping-pong echo trailing each note; Overtone up for a glassy ring.",
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
            description: "A talking lead for French touch: a saw and a pulse driven hard into a formant filter that sounds like a mouth, with a gentle vowel drift. Play short riffs; the mod wheel moves the vowel. Vowel sets the vowel from a to u, Grit pushes the drive, Shift makes the mouth smaller or larger, Room adds a slapback and a tight room.",
            listen: "The vowel moving under the riff; push Vowel and Shift to change how it speaks.",
            build: loud_mouth,
            group: 2,
            phrase: mouth_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "dust-devil",
            name: "Dust Devil",
            category: "Lead",
            tags: &["mono", "glide", "whistle", "trap", "hip-hop"],
            description: "A trap whistle lead: a sine with a touch of triangle that slides from note to note, with a dotted echo and a vibrato. Play simple melodies. Tone opens the filter, Slide sets how long the slide takes, Space adds the echo and the room, Vibrato fades in a vibrato.",
            listen: "The slides between notes and the echo; Slide up for long swoops.",
            build: dust_devil,
            group: 2,
            phrase: devil_phrase,
            secs: 9.0,
        },
        Preset {
            slug: "afterglow",
            name: "Afterglow",
            category: "Lead",
            tags: &["mono", "legato", "unison", "chorus", "synthwave", "80s"],
            description: "A big synthwave lead: a seven-voice saw stack with an octave stack and a pulse, chorus, a long echo and a big room. Play slow, soaring lines; notes glide. Cutoff brightens, Spread widens the detune of the main stack, Space adds the echo and the room, Vibrato adds vibrato.",
            listen: "The wide stack and the long echo; Spread up for a bigger, looser sound.",
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
            description: "A synth brass lead for funk and disco: a three-voice saw stack with a second saw, a filter that swells open on every note and a light compressor. Play short stabs; harder notes are brighter. Tone opens the filter, Blat deepens the filter swell on each note, Space adds a short echo and a room, Vibrato adds a light vibrato.",
            listen: "The swell at the start of each stab; Blat up for a bigger blare.",
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
            description: "A house stab lead: two unison saw stacks with a pitch dive on every note, chorus and a dotted echo. Play short, bright stabs. Cutoff opens the filter, Dive sets how far each note falls into its pitch, Space adds the echo and the room, Edge adds drive.",
            listen: "The pitch falling into each stab; Dive at 0 is a plain unison stab.",
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
            description: "A metallic lead for French touch: a saw ring-modulated by a square a fifth up, 14 dB of drive, a bitcrusher and a heavy compressor. Play short, jagged lines. Cutoff opens the filter, Metal brings in the ring modulator, Crush mixes in the bitcrusher, Room adds a slapback and a tight room.",
            listen: "The metal on top of the saw and the crushed edge; Metal at 0 is a plain hard saw lead.",
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
                    let mut rack = (p.build)().migrated();
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
                    let mut rack = (p.build)().migrated();
                    let knob = *rack.e.state().modules.iter().find(|(_, m)| m.kind == "macro").unwrap().0;
                    rack.e.set_param(knob, &format!("m{k}"), v);
                    render(rack.e.state(), &(p.phrase)(), p.secs).0
                };
                let count = (1..=8)
                    .filter(|k| (p.build)().migrated().e.state().modules.values().any(|m| m.params.contains_key(&format!("pin.m{k}"))))
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
