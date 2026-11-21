#![allow(dead_code)]
//! The factory Piece presets (docs/presets/PIECE.md): self-playing pieces with cues.
//!
//! ```sh
//! cargo run -p kabl-tools --example presets_piece -- write patches
//! cargo run --release -p kabl-tools --example presets_piece -- clips patches out_dir [slug ...]
//! cargo run --release -p kabl-tools --example presets_piece -- levels patches [slug ...]
//! ```
//!
//! Same recipe as `presets_pilot.rs` (helpers copied from it): every piece names its macros after
//! what they do (CC 20-23), the first cue is the stored macro values, the cues move macros only
//! and every sequencer keeps playing. No limiter: headroom comes from the patch gain.

use kabl_modules::builtins::{cell_index, make_code, with_code};

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

/// Extra parts a piece adds to the master sum: the sources to mix in.
type ExtraFn = fn(&mut Rack, ModuleId) -> Vec<(ModuleId, &'static str)>;

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
    /// A cable with a step pattern (`length`, `s1`.., `prob`): the cable's own params.
    fn pattern(&mut self, a: ModuleId, ap: &str, b: ModuleId, bp: &str, params: &[(&str, f32)]) {
        let cable = self.e.connect(jack(a, ap), jack(b, bp));
        for &(name, v) in params {
            self.e.set_param_gesture(
                kabl_core::ParamTarget::Cable {
                    id: cable,
                    param: name.into(),
                },
                v,
                true,
            );
        }
    }
    fn wire_both(&mut self, a: ModuleId, ap: &str, b: ModuleId) {
        self.wire(a, ap, b, "in");
    }
    /// A `split` of the stereo output of `a`: its `left` and `right` are the two sides.
    fn split(&mut self, a: ModuleId) -> ModuleId {
        let s = self.add("split", &[]);
        self.wire(a, "out", s, "in");
        s
    }
    /// A `join` of two mono sources into one stereo signal (its `out`).
    fn join(&mut self, l: ModuleId, r: ModuleId) -> ModuleId {
        let j = self.add("join", &[]);
        self.wire(l, "out", j, "left");
        self.wire(r, "out", j, "right");
        j
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

// ---- macro helpers ---------------------------------------------------------------------------

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

/// As `span`, with the value `hi` at macro 1 given and the value at 0 derived.
#[allow(clippy::too_many_arguments)]
fn span_hi(
    r: &mut Rack,
    knob: ModuleId,
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
        r,
        knob,
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
    r: &mut Rack,
    knob: ModuleId,
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
    span(r, knob, m, s, id, kind, param, lo, d);
    r.e.set_param(knob, &format!("m{m}"), s);
    s
}

/// Macro `m` at 0 leaves `param` at `d`; at 1 it is `hi`.
#[allow(clippy::too_many_arguments)]
fn rise(
    r: &mut Rack,
    knob: ModuleId,
    m: usize,
    id: ModuleId,
    kind: &str,
    param: &str,
    d: f32,
    hi: f32,
) {
    let i = param_of(kind, param);
    r.e.set_param(id, param, d);
    r.route(
        knob,
        &format!("m{m}"),
        id,
        param,
        i.to_norm(hi) - i.to_norm(d),
    );
}

/// What an old route of `amount` from a macro at `stored` leaves on a param with base `base`.
fn routed(kind: &str, param: &str, base: f32, amount: f32, stored: f32) -> f32 {
    let i = param_of(kind, param);
    i.from_norm((i.to_norm(base) + stored * amount).clamp(0.0, 1.0))
}

/// A route that sits at 0 until macro `mac` of `knob` opens it, up to `depth` of the knob's
/// travel: an LFO or envelope whose depth is a macro.
#[allow(clippy::too_many_arguments)]
fn depth_route(
    r: &mut Rack,
    a: ModuleId,
    ap: &str,
    b: ModuleId,
    param: &str,
    knob: ModuleId,
    mac: &str,
    depth: f32,
) {
    let cable = r.e.connect_route(jack(a, ap), b, param);
    r.e.set_route_amount(cable, 0.0, true);
    let d =
        r.e.connect_cable_route(jack(knob, mac), cable, "amount")
            .expect("a route has a depth");
    r.e.set_route_amount(d, depth, true);
}

/// Macro `m` (stored at `stored`) takes the gain stage `id` down by `slope_db` over its whole
/// travel, keeping the stored sound: level compensation for a macro that adds or removes level.
fn trim_by(r: &mut Rack, knob: ModuleId, m: usize, id: ModuleId, stored: f32, slope_db: f32) {
    let base = r.e.state().modules[&id].params["gain_db"];
    r.e.set_param(id, "gain_db", base + slope_db * stored);
    r.route(knob, &format!("m{m}"), id, "gain_db", -slope_db / 84.0);
}

// ---- shared stages ---------------------------------------------------------------------------

/// Where the echo and the room get their mix: a macro (`m3` at the old stored value, so its
/// sound is the old one) or a fixed fraction of the old maximum.
enum Space {
    Macro(ModuleId, &'static str),
    Fixed(f32),
}

/// The tail every preset shares: a short mono-in echo (a slapback, or a tempo-synced echo when
/// `clock` is given) and a tight room, both at mix 0 until the Space macro opens them. `src` is
/// a stereo module (`left`/`right`); returns the room, whose outputs go on to the output.
#[allow(clippy::too_many_arguments)]
fn space_tail(
    r: &mut Rack,
    space: Space,
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
    let split = r.split(src);
    r.wire(split, "left", delay, "in");
    if let Some(c) = clock {
        r.wire(c, "gate", delay, "clock");
    }
    r.stereo(src, verb);
    r.wire(delay, "out", verb, "in");
    match space {
        Space::Macro(knob, mac) => {
            r.route(knob, mac, delay, "mix", echo_max);
            r.route(knob, mac, verb, "mix", room_max);
        }
        Space::Fixed(p) => {
            r.e.set_param(delay, "mix", 100.0 * echo_max * p);
            r.e.set_param(verb, "mix", 100.0 * room_max * p);
        }
    }
    verb
}

// ---- the piece builder -----------------------------------------------------------------------

/// One synth part that plays itself. `steps` are semitones from the oscillator's base C, one per
/// step, `-` a rest, a trailing `!` an accent, a trailing `?` a step that plays half the time.
struct Line {
    steps: &'static str,
    len: f32,
    wave: f32,
    uni: f32,
    det: f32,
    pw: f32,
    oct: f32,
    /// Level of a sine an octave below (0 for none).
    sub: f32,
    /// Gate length in percent of a step.
    gate: f32,
    env: [f32; 4],
    db: f32,
    /// A low-pass on the part in Hz (0 for none).
    lp: f32,
    /// Clock division: 1 is a 16th per step, 16 a bar per step.
    div: f32,
    /// Joins after the master filter (it has its own tone filter, the master filter does not mask it).
    after: bool,
    /// Joins after the kick duck and the echo and room, next to the drums (a rumble the kick must not swallow).
    post: bool,
}

impl Line {
    fn new(steps: &'static str) -> Line {
        Line {
            steps,
            len: steps.split_whitespace().count() as f32,
            wave: 2.0,
            uni: 1.0,
            det: 10.0,
            pw: 50.0,
            oct: 0.0,
            sub: 0.0,
            gate: 60.0,
            env: [2.0, 150.0, 0.6, 80.0],
            db: 0.0,
            lp: 0.0,
            div: 1.0,
            after: false,
            post: false,
        }
    }
}

struct LineH {
    seq: ModuleId,
    osc: ModuleId,
    env: ModuleId,
    gain: ModuleId,
    /// The part's own low-pass, when it has one.
    tone: Option<ModuleId>,
    out: (ModuleId, &'static str),
    after: bool,
    post: bool,
}

/// A clock divided by `div` (cascaded: the module divides by 8 at most).
fn divided(
    r: &mut Rack,
    clock: ModuleId,
    div: f32,
) -> (ModuleId, &'static str, ModuleId, &'static str) {
    let mut src = (clock, "gate", clock, "reset");
    let mut left = div as u32;
    while left > 1 {
        let f = if left >= 8 && left.is_multiple_of(8) {
            8
        } else if left.is_multiple_of(4) {
            4
        } else if left.is_multiple_of(2) {
            2
        } else {
            panic!("div {div}")
        };
        let d = r.add("clock.div", &[("div", f as f32)]);
        r.wire(src.0, src.1, d, "clock");
        r.wire(src.2, src.3, d, "reset");
        src = (d, "gate", clock, "reset");
        left /= f;
    }
    src
}

fn add_line(r: &mut Rack, clock: ModuleId, l: &Line) -> LineH {
    let seq = r.add(
        "seq",
        &[("length", l.len), ("gate_len", l.gate), ("gate_mode", 1.0)],
    );
    let src = divided(r, clock, l.div);
    r.wire(src.0, src.1, seq, "clock");
    r.wire(clock, "reset", seq, "reset");
    for (k, tok) in l.steps.split_whitespace().enumerate() {
        let n = k + 1;
        if tok == "-" {
            r.e.set_param(seq, &format!("g{n}"), 0.0);
            continue;
        }
        let accent = tok.ends_with('!');
        let chance = tok.ends_with('?');
        let p: f32 = tok.trim_end_matches(['!', '?']).parse().expect(tok);
        r.e.set_param(seq, &format!("p{n}"), p);
        r.e.set_param(seq, &format!("g{n}"), 1.0);
        r.e.set_param(seq, &format!("v{n}"), if accent { 100.0 } else { 70.0 });
        if chance {
            r.e.set_param(seq, &format!("r{n}"), 50.0);
        }
    }
    let osc = r.add(
        "osc.va",
        &[
            ("base_hz", 261.63 * 2f32.powf(l.oct)),
            ("waveform", l.wave),
            ("unison", l.uni),
            ("detune", l.det),
            ("pw", l.pw),
        ],
    );
    r.wire(seq, "pitch", osc, "pitch");
    let env = r.add("env.adsr", &adsr(l.env[0], l.env[1], l.env[2], l.env[3]));
    r.wire(seq, "gate", env, "gate");
    let vca = r.add("vca", &[("gain", 0.0)]);
    r.wire(env, "out", vca, "cv");
    if l.sub > 0.0 {
        let sub = r.add(
            "osc.va",
            &[
                ("base_hz", 261.63 * 2f32.powf(l.oct - 1.0)),
                ("waveform", 0.0),
            ],
        );
        r.wire(seq, "pitch", sub, "pitch");
        let mix = r.add("mixer", &[("level1", 1.0), ("level2", l.sub)]);
        r.wire(osc, "out", mix, "in1");
        r.wire(sub, "out", mix, "in2");
        r.wire(mix, "out", vca, "in");
    } else {
        r.wire(osc, "out", vca, "in");
    }
    let gain = r.add("gain", &[("gain_db", l.db)]);
    r.wire(vca, "out", gain, "in");
    let mut out = (gain, "out");
    let mut tone = None;
    if l.lp > 0.0 {
        let t = r.add("filter.svf", &[("cutoff_hz", l.lp), ("resonance", 0.1)]);
        r.wire(gain, "out", t, "in");
        out = (t, "lp");
        tone = Some(t);
    }
    LineH {
        seq,
        osc,
        env,
        gain,
        tone,
        out,
        after: l.after,
        post: l.post,
    }
}

/// Mixes any number of sources into one: chained four-input mixers, every level 1.
fn sum(r: &mut Rack, srcs: &[(ModuleId, &str)]) -> ModuleId {
    let mut prev: Option<ModuleId> = None;
    let mut i = 0;
    loop {
        let m = r.add(
            "mixer",
            &[
                ("level1", 1.0),
                ("level2", 1.0),
                ("level3", 1.0),
                ("level4", 1.0),
            ],
        );
        let mut slot = 1;
        if let Some(p) = prev {
            r.wire(p, "out", m, "in1");
            slot = 2;
        }
        while slot <= 4 && i < srcs.len() {
            r.wire(srcs[i].0, srcs[i].1, m, &format!("in{slot}"));
            slot += 1;
            i += 1;
        }
        prev = Some(m);
        if i >= srcs.len() {
            return m;
        }
    }
}

/// One 16-step row of the drum grid from a pattern text (see `GROOVE.md`).
fn grid_row(r: &mut Rack, grid: ModuleId, row: usize, text: &str) {
    let mut chunks = [0.0f32; 4];
    let text: Vec<char> = text.chars().filter(|c| *c != ' ').collect();
    assert_eq!(text.len(), 16, "a grid row has 16 steps: {text:?}");
    for (step, c) in text.into_iter().enumerate() {
        let (vel, chance) = match c {
            '.' => continue,
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
        r.e.set_param(grid, params[cell_index(0, row, c)].name, v);
    }
}

/// A drum kit on the grid: rows are kick, snare, clap, closed hat, open hat, tom (16 steps each).
struct Kit {
    rows: [&'static str; 6],
    /// Kick level dB, decay ms, drive dB, tune Hz.
    kick: [f32; 4],
    /// Snare, clap, hat, tom levels dB.
    levels: [f32; 4],
    /// Mixer levels: kick, snare, clap, hats.
    mix: [f32; 4],
    snare_decay: f32,
    tom: [f32; 2],
    /// Level of the toms in the kit mixer.
    tom_mix: f32,
    /// Clock division in front of the grid (1: a 16th per step).
    div: f32,
    /// Kick tail: the `decay_ms` of the kick.
    lens: [f32; 6],
}

struct KitH {
    grid: ModuleId,
    kick: ModuleId,
    tom: ModuleId,
    beat: ModuleId,
    /// The ladder filter on clap and hats (open at 20 kHz until a macro closes it).
    top_lad: ModuleId,
    snare: ModuleId,
    /// Snare, clap and hat levels: in1 clap, in2 hat, in3 snare.
    top: ModuleId,
    out: ModuleId,
}

fn add_kit(r: &mut Rack, clock: ModuleId, k: &Kit, duck: Option<ModuleId>) -> KitH {
    let grid = r.add("drum.grid", &[]);
    let src = divided(r, clock, k.div);
    r.wire(src.0, src.1, grid, "clock");
    r.wire(clock, "reset", grid, "reset");
    for (row, text) in k.rows.iter().enumerate() {
        r.e.set_param(grid, &format!("len{}", row + 1), k.lens[row]);
        grid_row(r, grid, row, text);
    }
    let kick = r.add(
        "drum.kick",
        &[
            ("level_db", k.kick[0]),
            ("decay_ms", k.kick[1]),
            ("drive_db", k.kick[2]),
            ("tune_hz", k.kick[3]),
        ],
    );
    let snare = r.add(
        "drum.snare",
        &[("level_db", k.levels[0]), ("decay_ms", k.snare_decay)],
    );
    let clap = r.add("drum.clap", &[("level_db", k.levels[1])]);
    let hat = r.add("drum.hat", &[("level_db", k.levels[2])]);
    let tom = r.add(
        "drum.tom",
        &[
            ("level_db", k.levels[3]),
            ("tune_hz", k.tom[0]),
            ("decay_ms", k.tom[1]),
        ],
    );
    r.wire(grid, "kick", kick, "gate");
    r.wire(grid, "snare", snare, "gate");
    r.wire(grid, "clap", clap, "gate");
    r.wire(grid, "chat", hat, "closed");
    r.wire(grid, "ohat", hat, "open");
    r.wire(grid, "tom", tom, "gate");
    if let Some(d) = duck {
        r.wire(kick, "out", d, "sc");
    }
    let beat = r.add("mixer", &[("level1", k.mix[0])]);
    r.wire(kick, "out", beat, "in1");
    let top = r.add(
        "mixer",
        &[
            ("level1", k.mix[2]),
            ("level2", k.mix[3]),
            ("level3", k.mix[1]),
        ],
    );
    r.wire(clap, "out", top, "in1");
    r.wire(hat, "out", top, "in2");
    r.wire(snare, "out", top, "in3");
    let top_lad = r.add(
        "filter.ladder",
        &[("cutoff_hz", 20000.0), ("resonance", 0.2)],
    );
    r.wire(top, "out", top_lad, "in");
    let out = r.add(
        "mixer",
        &[("level1", 1.0), ("level2", k.tom_mix), ("level3", 1.0)],
    );
    r.wire(beat, "out", out, "in1");
    r.wire(tom, "out", out, "in2");
    r.wire(top_lad, "out", out, "in3");
    KitH {
        grid,
        kick,
        tom,
        beat,
        top_lad,
        snare,
        top,
        out,
    }
}

/// Echo and room: the clock-synced echo (`sync`, 4 = quarter), its feedback and tone, the
/// largest echo and room mixes, the room decay.
struct SpaceSpec {
    echo: [f32; 3],
    sync: f32,
    echo_max: f32,
    room_decay: f32,
    room_max: f32,
}

struct Spec {
    names: [&'static str; 4],
    stored: [f32; 4],
    bpm: f32,
    swing: f32,
    lines: Vec<Line>,
    kit: Option<Kit>,
    /// The voice MIDI plays: waveform, unison, detune, octave, low-pass Hz, level dB.
    play: [f32; 6],
    /// Bus drive dB and trim dB.
    drive: [f32; 2],
    /// Master filter: cutoff Hz, resonance, drive dB.
    lad: [f32; 3],
    trim_db: f32,
    /// Music compressor: threshold, ratio, release ms, makeup dB.
    comp: [f32; 4],
    /// The kick duck: release ms (None: no duck).
    duck: Option<f32>,
    riser: bool,
    space: Option<SpaceSpec>,
    /// Which macro is the space macro (1..=4).
    space_macro: usize,
    /// A bitcrusher after the bus drive (mix 0 until a macro opens it).
    crush: bool,
    /// A chorus before the echo and room (mix 0 until a macro opens it).
    chorus: bool,
    /// More sources for the bus, built by the piece (a random melody, say).
    extra: Option<ExtraFn>,
}

struct H {
    knob: ModuleId,
    cue: ModuleId,
    clock: ModuleId,
    lines: Vec<LineH>,
    drive: ModuleId,
    lad: ModuleId,
    crush: Option<ModuleId>,
    chorus: Option<ModuleId>,
    trim: ModuleId,
    comp: ModuleId,
    duck: Option<ModuleId>,
    /// The riser: a VCA on pink noise (gain 0 until a macro opens it).
    riser: Option<ModuleId>,
    noise: Option<ModuleId>,
    kit: Option<KitH>,
    play_gain: ModuleId,
    play_lad: ModuleId,
    after: ModuleId,
}

fn assemble(s: &Spec) -> (Rack, H) {
    let mut r = Rack::new();
    let knob = macros(&mut r, s.names, s.stored);
    let cue = r.add("cues", &[]);
    let clock = r.add("clock", &[("bpm", s.bpm), ("swing", s.swing)]);
    r.next_row();
    let lines: Vec<LineH> = s.lines.iter().map(|l| add_line(&mut r, clock, l)).collect();
    r.next_row();
    // The played voice.
    let m = r.add("midi.in", &[("mode", 1.0), ("bend", 2.0)]);
    let play = r.add(
        "osc.va",
        &[
            ("base_hz", 261.63 * 2f32.powf(s.play[3])),
            ("waveform", s.play[0]),
            ("unison", s.play[1]),
            ("detune", s.play[2]),
        ],
    );
    let play_env = r.add("env.adsr", &adsr(4.0, 250.0, 0.6, 120.0));
    let play_vca = r.add("vca", &[("gain", 0.0)]);
    let play_lad = r.add(
        "filter.ladder",
        &[("cutoff_hz", s.play[4]), ("resonance", 0.1)],
    );
    let play_gain = r.add("gain", &[("gain_db", s.play[5])]);
    r.wire(m, "pitch", play, "pitch");
    r.wire(m, "gate", play_env, "gate");
    r.wire(play, "out", play_vca, "in");
    r.wire(play_env, "out", play_vca, "cv");
    r.wire(play_vca, "out", play_lad, "in");
    r.wire(play_lad, "out", play_gain, "in");
    r.next_row();
    // Master chain.
    let mut pre_srcs: Vec<(ModuleId, &str)> = vec![(play_gain, "out")];
    let mut after_srcs: Vec<(ModuleId, &str)> = Vec::new();
    let mut dry_srcs: Vec<(ModuleId, &str)> = Vec::new();
    for l in &lines {
        if l.post {
            dry_srcs.push(l.out);
        } else if l.after {
            after_srcs.push(l.out);
        } else {
            pre_srcs.push(l.out);
        }
    }
    if let Some(f) = s.extra {
        pre_srcs.extend(f(&mut r, clock));
    }
    let pre = sum(&mut r, &pre_srcs);
    let drive = r.add(
        "drive",
        &[("drive_db", s.drive[0]), ("trim_db", s.drive[1])],
    );
    r.wire(pre, "out", drive, "in");
    let (noise, riser) = if s.riser {
        let n = r.add("noise", &[("color", 1.0)]);
        let v = r.add("vca", &[("gain", 0.0)]);
        r.wire(n, "out", v, "in");
        (Some(n), Some(v))
    } else {
        (None, None)
    };
    let crush = s.crush.then(|| {
        r.add(
            "bitcrusher",
            &[("bits", 7.0), ("downsample", 4.0), ("mix", 0.0)],
        )
    });
    let mut post_srcs = vec![(drive, "out")];
    if let Some(c) = crush {
        r.wire(drive, "out", c, "in");
        post_srcs = vec![(c, "out")];
    }
    if let Some(v) = riser {
        post_srcs.push((v, "out"));
    }
    let post = sum(&mut r, &post_srcs);
    let lad = r.add(
        "filter.ladder",
        &[
            ("cutoff_hz", s.lad[0]),
            ("resonance", s.lad[1]),
            ("drive_db", s.lad[2]),
        ],
    );
    r.wire(post, "out", lad, "in");
    let mut a = vec![(lad, "out")];
    a.extend(after_srcs);
    let after = sum(&mut r, &a);
    let trim = r.add("gain", &[("gain_db", s.trim_db)]);
    r.wire(after, "out", trim, "in");
    let comp = r.add(
        "compressor",
        &[
            ("threshold_db", s.comp[0]),
            ("ratio", s.comp[1]),
            ("attack_ms", 5.0),
            ("release_ms", s.comp[2]),
            ("makeup_db", s.comp[3]),
        ],
    );
    r.wire_both(trim, "out", comp);
    r.next_row();
    let duck = s.duck.map(|rel| {
        r.add(
            "compressor",
            &[
                ("threshold_db", 0.0),
                ("ratio", 10.0),
                ("attack_ms", 1.0),
                ("release_ms", rel),
                ("knee_db", 3.0),
            ],
        )
    });
    let kit = s.kit.as_ref().map(|k| add_kit(&mut r, clock, k, duck));
    let mut src = comp;
    if let Some(d) = duck {
        r.wire(comp, "out", d, "in");
        src = d;
    }
    let chorus = s.chorus.then(|| {
        let c = r.add("chorus", &[("rate_hz", 0.4), ("depth", 60.0), ("mix", 0.0)]);
        r.wire(src, "out", c, "in");
        c
    });
    if let Some(c) = chorus {
        src = c;
    }
    let tail = match &s.space {
        Some(sp) => {
            let t = space_tail(
                &mut r,
                Space::Macro(knob, ["m1", "m2", "m3", "m4"][s.space_macro - 1]),
                src,
                &[
                    ("sync", sp.sync),
                    ("feedback", sp.echo[0]),
                    ("mix", 0.0),
                    ("tone_hz", sp.echo[1]),
                ],
                Some(clock),
                sp.echo_max,
                sp.room_decay,
                sp.room_max,
            );
            r.e.set_param(t, "damp_hz", sp.echo[2]);
            t
        }
        None => src,
    };
    let split = r.split(tail);
    let l = r.add(
        "mixer",
        &[("level1", 1.0), ("level2", 1.0), ("level3", 1.0)],
    );
    let rr = r.add(
        "mixer",
        &[("level1", 1.0), ("level2", 1.0), ("level3", 1.0)],
    );
    r.wire(split, "left", l, "in1");
    r.wire(split, "right", rr, "in1");
    if let Some(k) = &kit {
        r.wire(k.out, "out", l, "in2");
        r.wire(k.out, "out", rr, "in2");
    }
    if !dry_srcs.is_empty() {
        let pm = sum(&mut r, &dry_srcs);
        r.wire(pm, "out", l, "in3");
        r.wire(pm, "out", rr, "in3");
    }
    let (mix_l, mix_r) = (l, rr);
    let master = r.join(mix_l, mix_r);
    r.out_stereo(master);
    let h = H {
        knob,
        cue,
        clock,
        lines,
        drive,
        lad,
        crush,
        chorus,
        trim,
        comp,
        duck,
        riser,
        noise,
        kit,
        play_gain,
        play_lad,
        after,
    };
    (r, h)
}

/// A pattern for a four-bar chord progression: `pat` per bar (None a rest, Some(n) a note
/// n semitones above that bar's root), `roots` one per bar. A leaked string for `Line::new`.
fn prog(roots: [i32; 4], pat: &[Option<i32>]) -> &'static str {
    let mut out = Vec::new();
    for root in roots {
        for n in pat {
            out.push(match n {
                Some(n) => (root + n).to_string(),
                None => "-".to_string(),
            });
        }
    }
    Box::leak(out.join(" ").into_boxed_str())
}

// ---- cues ------------------------------------------------------------------------------------

/// A section: name, the four macro targets, the morph in beats and the bar it lands on.
struct CueDef {
    name: &'static str,
    targets: [f32; 4],
    morph: f32,
    bar: f32,
}

/// Adds the cues (macros only: every sequencer keeps playing) and pins everything to Perform.
/// The first cue is the stored macro values.
fn finish(r: &mut Rack, h: &H, cues_def: &[CueDef]) {
    for (n, c) in cues_def.iter().enumerate() {
        let id = cues::add(&mut r.e, h.cue, &|_| 0).expect("cue");
        cues::rename(&mut r.e, h.cue, id, c.name);
        for l in &h.lines {
            cues::set_target(&mut r.e, h.cue, id, l.seq, None);
        }
        let t = if n == 0 {
            // The first cue is the stored sound.
            let st = r.e.state().modules[&h.knob].params.clone();
            [0, 1, 2, 3].map(|k| st[&format!("m{}", k + 1)])
        } else {
            c.targets
        };
        for (k, v) in t.into_iter().enumerate() {
            cues::set_macro(&mut r.e, h.cue, id, h.knob, k as u8, Some(v));
        }
        cues::set_morph(&mut r.e, h.cue, id, c.morph);
    }
    perform(r, h.knob, Some(h.clock), Some((h.cue, cues_def.len())));
}

/// Takes the stage's level down by `db` over the whole travel of macro `m` (it adds level when
/// `db` is negative) around its stored value.
fn level_comp(r: &mut Rack, h: &H, m: usize, id: ModuleId, db: f32) {
    let stored = r.e.state().modules[&h.knob].params[&format!("m{m}")];
    trim_by(r, h.knob, m, id, stored, db);
}

// ---- the bank entries and their clips --------------------------------------------------------

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
    tags: &'static [&'static str],
    description: &'static str,
    listen: &'static str,
    build: fn() -> (Rack, Vec<CueDef>),
    phrase: fn() -> Vec<Hit>,
    /// A chord held for four bars on repeat: what the macro tours play (steady, no sections).
    tour_chord: &'static [u8],
    bpm: f32,
    /// Bars of the clip.
    bars: f32,
    /// Level of the whole preset in dB (the generator's own trim over the design).
    trim: f32,
}

// ---- 1. Gantry: techno build and drop --------------------------------------------------------

fn gantry() -> (Rack, Vec<CueDef>) {
    let s = Spec {
        names: ["Climb", "Rumble", "Acid", "Pump"],
        stored: [0.20, 0.35, 0.15, 0.55],
        bpm: 132.0,
        swing: 0.0,
        lines: vec![
            // The rumble: off-beat A1s, low-passed, between the kicks.
            Line {
                wave: 2.0,
                oct: -2.0,
                gate: 70.0,
                env: [1.0, 160.0, 0.5, 60.0],
                lp: 300.0,
                div: 2.0,
                post: true,
                ..Line::new("- -3 - -3 - -3 - -3")
            },
            // The acid stab: A minor, off the beat.
            Line {
                wave: 3.0,
                pw: 30.0,
                uni: 2.0,
                det: 12.0,
                oct: -1.0,
                gate: 40.0,
                env: [1.0, 110.0, 0.0, 60.0],
                db: -6.0,
                lp: 900.0,
                after: true,
                ..Line::new("9! - - 9 - 12 - - 9! - 16 - 12 - 7 -")
            },
            // The pad: a bar per step, A, A, F, G.
            Line {
                wave: 2.0,
                uni: 5.0,
                det: 18.0,
                oct: -1.0,
                gate: 100.0,
                env: [400.0, 300.0, 0.9, 700.0],
                db: -14.0,
                div: 16.0,
                ..Line::new("9 9 5 7")
            },
            Line {
                wave: 2.0,
                uni: 5.0,
                det: 18.0,
                oct: -1.0,
                gate: 100.0,
                env: [400.0, 300.0, 0.9, 700.0],
                db: -14.0,
                div: 16.0,
                ..Line::new("16 16 12 14")
            },
        ],
        kit: Some(Kit {
            rows: [
                "X...X...X...X...",
                "................",
                "....X.......X...",
                "x?o?x?o?x?o?x?o?",
                "..X...X...X...X.",
                ".......?.......?",
            ],
            kick: [-7.0, 300.0, 10.0, 52.0],
            levels: [-6.0, -9.0, -12.0, -8.0],
            mix: [0.7, 0.0, 0.3, 0.22],
            snare_decay: 200.0,
            tom: [120.0, 300.0],
            tom_mix: 0.3,
            div: 1.0,
            lens: [16.0; 6],
        }),
        play: [2.0, 2.0, 8.0, 0.0, 3500.0, -12.0],
        drive: [10.0, -4.0],
        lad: [500.0, 0.5, 4.0],
        trim_db: -3.0,
        comp: [-18.0, 3.0, 120.0, 0.0],
        duck: Some(200.0),
        riser: true,
        space: None,
        space_macro: 3,
        crush: false,
        chorus: false,
        extra: None,
    };
    let (mut r, h) = assemble(&s);
    let k = h.knob;
    // Climb: the master filter opens and a pink-noise riser fades in over it.
    rise(
        &mut r,
        k,
        1,
        h.lad,
        "filter.ladder",
        "cutoff_hz",
        140.0,
        12000.0,
    );
    rise(&mut r, k, 1, h.lad, "filter.ladder", "resonance", 0.62, 0.3);
    r.route(k, "m1", h.riser.unwrap(), "gain", 0.5);
    rise(
        &mut r,
        k,
        1,
        h.kit.as_ref().unwrap().top_lad,
        "filter.ladder",
        "cutoff_hz",
        700.0,
        18000.0,
    );
    level_comp(&mut r, &h, 1, h.trim, 8.0);
    // Rumble: the bass part from out to full, and its notes lengthen.
    let rum = &h.lines[0];
    rise(&mut r, k, 2, rum.gain, "gain", "gain_db", -34.0, -8.0);
    rise(&mut r, k, 2, rum.seq, "seq", "gate_len", 25.0, 90.0);
    rise(
        &mut r,
        k,
        2,
        rum.tone.unwrap(),
        "filter.svf",
        "cutoff_hz",
        140.0,
        450.0,
    );
    // Acid: the stab's filter opens and squeals.
    let acid = &h.lines[1];
    r.e.set_param(acid.tone.unwrap(), "cutoff_hz", 1400.0);
    rise(&mut r, k, 3, acid.env, "env.adsr", "decay_ms", 40.0, 320.0);
    rise(
        &mut r,
        k,
        3,
        acid.tone.unwrap(),
        "filter.svf",
        "resonance",
        0.15,
        0.78,
    );
    rise(&mut r, k, 3, acid.gain, "gain", "gain_db", -18.0, -15.0);
    // Pump: the kick ducks the music harder.
    let duck = h.duck.unwrap();
    rise(&mut r, k, 4, duck, "compressor", "threshold_db", 0.0, -45.0);
    rise(&mut r, k, 4, h.comp, "compressor", "makeup_db", 0.0, 8.0);
    let cues = vec![
        CueDef {
            name: "Intro",
            targets: s.stored,
            morph: 0.0,
            bar: 0.0,
        },
        CueDef {
            name: "Build",
            targets: [0.92, 0.55, 0.45, 0.8],
            morph: 16.0,
            bar: 4.0,
        },
        CueDef {
            name: "Drop",
            targets: [0.75, 1.0, 0.6, 1.0],
            morph: 0.0,
            bar: 8.0,
        },
        CueDef {
            name: "Break",
            targets: [0.25, 0.1, 0.3, 0.3],
            morph: 4.0,
            bar: 16.0,
        },
        CueDef {
            name: "Peak",
            targets: [0.95, 1.0, 0.95, 0.9],
            morph: 8.0,
            bar: 20.0,
        },
    ];
    finish(&mut r, &h, &cues);
    (r, cues)
}

fn gantry_phrase() -> Vec<Hit> {
    // A held A minor chord over the first section, a lead line at the drop.
    let bar = 240.0 / 132.0;
    let mut v = chord(0.5, 3.0 * bar, &[57, 60, 64]);
    v.extend(chord(0.5 + 8.0 * bar, 4.0 * bar, &[69, 72, 76]));
    v.extend(chord(0.5 + 20.0 * bar, 4.0 * bar, &[69, 72, 76]));
    v
}

// ---- 2. Dead Calm: cinematic swell ------------------------------------------------------------

fn dead_calm() -> (Rack, Vec<CueDef>) {
    let slow = |steps: &'static str, uni: f32, det: f32, oct: f32, lp: f32, db: f32| Line {
        wave: 2.0,
        uni,
        det,
        oct,
        gate: 100.0,
        env: [1800.0, 800.0, 0.9, 3000.0],
        lp,
        db,
        div: 16.0,
        ..Line::new(steps)
    };
    let s = Spec {
        names: ["Swell", "Dread", "Pulse", "Hall"],
        stored: [0.15, 0.10, 0.20, 0.60],
        bpm: 66.0,
        swing: 0.0,
        lines: vec![
            slow("0 0 -4 -4 -7 -7 -2 -2", 6.0, 22.0, -2.0, 900.0, 0.0),
            slow("7 7 3 3 0 0 5 5", 4.0, 16.0, -1.0, 1800.0, -4.0),
            slow("3 3 0 0 8 8 14 14", 4.0, 14.0, -1.0, 2200.0, -8.0),
            // The dread: a tritone and a semitone above, a slow cluster.
            slow("6 6 1 1 6 6 1 1", 3.0, 45.0, 0.0, 1500.0, -40.0),
            // A sparse high bell, half the steps play half the time.
            Line {
                wave: 0.0,
                oct: 1.0,
                gate: 40.0,
                env: [2.0, 1500.0, 0.0, 1800.0],
                db: -20.0,
                div: 4.0,
                ..Line::new("19? - - - - - 12? - - - - - 15? - - -")
            },
        ],
        kit: Some(Kit {
            rows: [
                "X...............",
                "................",
                "................",
                "................",
                "................",
                "x.o.....x.o.....",
            ],
            kick: [-12.0, 1800.0, 6.0, 40.0],
            levels: [-6.0, -9.0, -12.0, -10.0],
            mix: [0.8, 0.0, 0.0, 0.0],
            snare_decay: 200.0,
            tom: [70.0, 600.0],
            tom_mix: 0.8,
            div: 2.0,
            lens: [16.0; 6],
        }),
        play: [2.0, 3.0, 12.0, 0.0, 1800.0, -12.0],
        drive: [3.0, -2.0],
        lad: [180.0, 0.3, 0.0],
        trim_db: 9.0,
        comp: [-26.0, 3.0, 300.0, 2.0],
        duck: None,
        riser: true,
        space: Some(SpaceSpec {
            echo: [45.0, 2500.0, 2500.0],
            sync: 4.0,
            echo_max: 0.4,
            room_decay: 9.0,
            room_max: 0.5,
        }),
        space_macro: 4,
        crush: false,
        chorus: false,
        extra: None,
    };
    let (mut r, h) = assemble(&s);
    let k = h.knob;
    // Swell: the master filter opens, a riser fades in, the drone gets a little louder.
    rise(
        &mut r,
        k,
        1,
        h.lad,
        "filter.ladder",
        "cutoff_hz",
        180.0,
        9000.0,
    );
    r.route(k, "m1", h.riser.unwrap(), "gain", 0.4);
    rise(&mut r, k, 1, h.lines[0].gain, "gain", "gain_db", -6.0, 0.0);
    level_comp(&mut r, &h, 1, h.trim, 6.0);
    // Dread: the cluster comes up out of nothing and opens.
    rise(
        &mut r,
        k,
        2,
        h.lines[3].gain,
        "gain",
        "gain_db",
        -30.0,
        -4.0,
    );
    rise(
        &mut r,
        k,
        2,
        h.lines[3].tone.unwrap(),
        "filter.svf",
        "cutoff_hz",
        400.0,
        4000.0,
    );
    // Pulse: the heartbeat toms and the low impact come up.
    // Hall: the echo and room add level; take the stage down by what they add.
    level_comp(&mut r, &h, 4, h.trim, 4.6);
    let kit = h.kit.as_ref().unwrap();
    rise(&mut r, k, 3, kit.out, "mixer", "level1", 0.0, 1.0);
    rise(&mut r, k, 3, kit.out, "mixer", "level2", 0.0, 0.8);
    let cues = vec![
        CueDef {
            name: "Hush",
            targets: s.stored,
            morph: 0.0,
            bar: 0.0,
        },
        CueDef {
            name: "Swell",
            targets: [0.85, 0.45, 0.35, 0.7],
            morph: 16.0,
            bar: 4.0,
        },
        CueDef {
            name: "Impact",
            targets: [1.0, 0.7, 1.0, 0.5],
            morph: 0.0,
            bar: 8.0,
        },
        CueDef {
            name: "Aftermath",
            targets: [0.15, 0.2, 0.15, 0.8],
            morph: 16.0,
            bar: 12.0,
        },
    ];
    finish(&mut r, &h, &cues);
    (r, cues)
}

fn dead_calm_phrase() -> Vec<Hit> {
    let bar = 240.0 / 66.0;
    let mut v = chord(0.5 + 2.0 * bar, 2.0 * bar, &[60, 67, 72]);
    v.extend(chord(0.5 + 8.0 * bar, 3.0 * bar, &[60, 63, 67, 72]));
    v.extend(chord(0.5 + 13.0 * bar, 4.0 * bar, &[55, 62, 67]));
    v
}

// ---- 3. Pawn Shop: boom-bap with sections ---------------------------------------------------

fn pawn_shop() -> (Rack, Vec<CueDef>) {
    let chord_line = |steps: &'static str| Line {
        wave: 2.0,
        uni: 2.0,
        det: 8.0,
        gate: 90.0,
        env: [8.0, 500.0, 0.4, 300.0],
        lp: 1100.0,
        db: -8.0,
        ..Line::new(steps)
    };
    let s = Spec {
        names: ["Swing", "Dust", "Tape", "Room"],
        stored: [0.50, 0.0, 0.85, 0.25],
        bpm: 88.0,
        swing: 50.0,
        lines: vec![
            Line {
                wave: 0.0,
                oct: -2.0,
                gate: 70.0,
                env: [5.0, 300.0, 0.7, 120.0],
                db: 2.0,
                ..Line::new("-7 - - -7 - - - - -4 - - -7 - 0 -")
            },
            chord_line("-7 - - - - - - - -7 - - - - - - -"),
            chord_line("0 - - - - - - - 0 - - - - - - -"),
            chord_line("3 - - - - - - - 3 - - - - - - -"),
        ],
        kit: Some(Kit {
            rows: [
                "X.....o...X..?..",
                "....X.......X...",
                "................",
                "x.x.x.x.x.x.x.x.",
                "..............o.",
                "................",
            ],
            kick: [-12.0, 350.0, 6.0, 55.0],
            levels: [-10.0, -14.0, -20.0, -10.0],
            mix: [0.7, 0.6, 0.15, 0.15],
            snare_decay: 240.0,
            tom: [120.0, 300.0],
            tom_mix: 0.3,
            div: 1.0,
            lens: [16.0; 6],
        }),
        play: [2.0, 2.0, 8.0, 0.0, 1400.0, -12.0],
        drive: [22.0, -12.0],
        lad: [9000.0, 0.15, 0.0],
        trim_db: 10.0,
        comp: [-24.0, 4.0, 150.0, 3.0],
        duck: None,
        riser: true,
        space: Some(SpaceSpec {
            echo: [30.0, 3000.0, 3500.0],
            sync: 4.0,
            echo_max: 0.4,
            room_decay: 1.4,
            room_max: 0.45,
        }),
        space_macro: 4,
        crush: false,
        chorus: false,
        extra: None,
    };
    let (mut r, h) = assemble(&s);
    let k = h.knob;
    // Swing: the off-beats get later; the tempo stays the host's.
    rise(&mut r, k, 1, h.clock, "clock", "swing", 20.0, 80.0);
    // Dust: a vinyl hiss comes in and the pitch of the chords wobbles like a worn tape.
    r.route(k, "m2", h.riser.unwrap(), "gain", 0.6);
    let lfo = r.add("lfo", &[("rate_hz", 0.55), ("waveform", 0.0)]);
    for l in &h.lines[1..] {
        depth_route(&mut r, lfo, "out", l.osc, "fine", k, "m2", 0.05);
    }
    // Tape: from muffled and saturated to clean and open.
    rise(
        &mut r,
        k,
        3,
        h.lad,
        "filter.ladder",
        "cutoff_hz",
        1200.0,
        9000.0,
    );
    rise(
        &mut r,
        k,
        3,
        h.kit.as_ref().unwrap().top_lad,
        "filter.ladder",
        "cutoff_hz",
        3000.0,
        16000.0,
    );
    rise(&mut r, k, 3, h.drive, "drive", "drive_db", 22.0, 2.0);
    rise(&mut r, k, 3, h.drive, "drive", "trim_db", -12.0, -2.0);
    let cues = vec![
        CueDef {
            name: "Verse",
            targets: s.stored,
            morph: 0.0,
            bar: 0.0,
        },
        CueDef {
            name: "Hook",
            targets: [0.5, 0.0, 1.0, 0.45],
            morph: 4.0,
            bar: 6.0,
        },
        CueDef {
            name: "Bridge",
            targets: [0.8, 0.5, 0.1, 0.35],
            morph: 4.0,
            bar: 12.0,
        },
        CueDef {
            name: "Outro",
            targets: [0.3, 0.9, 0.4, 0.6],
            morph: 8.0,
            bar: 18.0,
        },
    ];
    finish(&mut r, &h, &cues);
    (r, cues)
}

fn pawn_shop_phrase() -> Vec<Hit> {
    let bar = 240.0 / 88.0;
    let mut v = chord(0.5 + 6.0 * bar, 1.5 * bar, &[65, 68, 72, 75]);
    v.extend(chord(0.5 + 8.0 * bar, 1.5 * bar, &[63, 67, 70, 74]));
    v.extend(chord(0.5 + 10.0 * bar, 1.5 * bar, &[65, 68, 72, 75]));
    v
}

// ---- 4. Laundromat: filter-disco loop with cut-up sections (signature) ------------------------

fn laundromat() -> (Rack, Vec<CueDef>) {
    let stab = |steps: &'static str, db: f32| Line {
        wave: 3.0,
        pw: 35.0,
        uni: 2.0,
        det: 14.0,
        oct: -1.0,
        gate: 50.0,
        env: [1.0, 200.0, 0.7, 40.0],
        db,
        lp: 3000.0,
        ..Line::new(steps)
    };
    let s = Spec {
        names: ["Sweep", "Crunch", "Stutter", "Pump"],
        stored: [0.45, 0.20, 0.50, 0.60],
        bpm: 122.0,
        swing: 28.0,
        lines: vec![
            // The octave bass of a disco loop.
            Line {
                wave: 2.0,
                uni: 2.0,
                det: 8.0,
                oct: -2.0,
                gate: 55.0,
                env: [1.0, 140.0, 0.5, 50.0],
                lp: 800.0,
                div: 2.0,
                ..Line::new("-3 9 -3 9 -3 9 -3 12")
            },
            stab("- - 9 - - 9 - - - - 9 - 12 - - -", -6.0),
            stab("- - 16 - - 16 - - - - 16 - 19 - - -", -8.0),
        ],
        kit: Some(Kit {
            rows: [
                "X...X...X...X...",
                "................",
                "....X.......X...",
                "x.x.x.x.x.x.x.x.",
                "..o...o...o...o.",
                "................",
            ],
            kick: [-8.0, 250.0, 8.0, 55.0],
            levels: [-8.0, -10.0, -16.0, -10.0],
            mix: [0.7, 0.0, 0.5, 0.3],
            snare_decay: 200.0,
            tom: [120.0, 300.0],
            tom_mix: 0.3,
            div: 1.0,
            lens: [16.0; 6],
        }),
        play: [2.0, 2.0, 10.0, 0.0, 3000.0, -12.0],
        drive: [8.0, -6.0],
        lad: [800.0, 0.5, 3.0],
        trim_db: 0.0,
        comp: [-18.0, 3.0, 120.0, 0.0],
        duck: Some(150.0),
        riser: false,
        space: None,
        space_macro: 3,
        crush: true,
        chorus: false,
        extra: None,
    };
    let (mut r, h) = assemble(&s);
    let k = h.knob;
    // Sweep: a DJ filter over the loop, the hats and the clap follow it.
    rise(
        &mut r,
        k,
        1,
        h.lad,
        "filter.ladder",
        "cutoff_hz",
        200.0,
        14000.0,
    );
    rise(&mut r, k, 1, h.lad, "filter.ladder", "resonance", 0.6, 0.2);
    rise(
        &mut r,
        k,
        1,
        h.kit.as_ref().unwrap().top_lad,
        "filter.ladder",
        "cutoff_hz",
        1500.0,
        16000.0,
    );
    level_comp(&mut r, &h, 1, h.trim, 6.0);
    // Crunch: the bus is bitcrushed and driven harder.
    rise(
        &mut r,
        k,
        2,
        h.crush.unwrap(),
        "bitcrusher",
        "mix",
        0.0,
        70.0,
    );
    rise(&mut r, k, 2, h.drive, "drive", "drive_db", 4.0, 18.0);
    rise(&mut r, k, 2, h.drive, "drive", "trim_db", -4.0, -10.0);
    // Stutter: the stabs from clipped ticks to held chords.
    for l in &h.lines[1..] {
        rise(&mut r, k, 3, l.seq, "seq", "gate_len", 6.0, 95.0);
    }
    // Pump: the kick ducks the loop harder.
    rise(
        &mut r,
        k,
        4,
        h.duck.unwrap(),
        "compressor",
        "threshold_db",
        0.0,
        -40.0,
    );
    rise(&mut r, k, 4, h.comp, "compressor", "makeup_db", 0.0, 6.0);
    let cues = vec![
        CueDef {
            name: "Loop",
            targets: s.stored,
            morph: 0.0,
            bar: 0.0,
        },
        CueDef {
            name: "Cut-up",
            targets: [0.7, 0.5, 0.1, 0.8],
            morph: 0.0,
            bar: 4.0,
        },
        CueDef {
            name: "Break",
            targets: [0.15, 0.1, 0.95, 0.2],
            morph: 4.0,
            bar: 12.0,
        },
        CueDef {
            name: "Full",
            targets: [0.95, 0.75, 0.5, 1.0],
            morph: 8.0,
            bar: 16.0,
        },
    ];
    finish(&mut r, &h, &cues);
    (r, cues)
}

fn laundromat_phrase() -> Vec<Hit> {
    let bar = 240.0 / 122.0;
    let mut v = chord(0.5 + 8.0 * bar, 3.0 * bar, &[69, 72, 76]);
    v.extend(chord(0.5 + 16.0 * bar, 4.0 * bar, &[69, 72, 76, 79]));
    v
}

// ---- 5. Spillway: drum and bass drop ----------------------------------------------------------

fn spillway() -> (Rack, Vec<CueDef>) {
    let pad = |steps: &'static str, db: f32| Line {
        wave: 2.0,
        uni: 4.0,
        det: 14.0,
        oct: -1.0,
        gate: 100.0,
        env: [400.0, 300.0, 0.9, 700.0],
        lp: 1600.0,
        db,
        div: 16.0,
        ..Line::new(steps)
    };
    let s = Spec {
        names: ["Weight", "Reese", "Break", "Haze"],
        stored: [0.30, 0.30, 0.15, 0.50],
        bpm: 174.0,
        swing: 0.0,
        lines: vec![
            // The sub: a sine under the reese, F minor.
            Line {
                wave: 0.0,
                oct: -2.0,
                gate: 80.0,
                env: [3.0, 200.0, 0.8, 80.0],
                db: -20.0,
                ..Line::new("-7 - - -7 - - - - - 3 - -7 - - -")
            },
            // The reese: two saws beating, an octave above the sub.
            Line {
                wave: 2.0,
                uni: 2.0,
                det: 6.0,
                oct: -1.0,
                gate: 80.0,
                env: [3.0, 200.0, 0.8, 80.0],
                lp: 1200.0,
                db: -8.0,
                ..Line::new("-7 - - -7 - - - - - 3 - -7 - - -")
            },
            pad("8 8 5 3", -14.0),
            pad("12 12 8 7", -16.0),
        ],
        kit: Some(Kit {
            rows: [
                "X.........X.....",
                "....X.......X...",
                "..?...?.?...?.?.",
                "x.x.x.x.x.x.x.x.",
                "..............o.",
                "................",
            ],
            kick: [-8.0, 200.0, 8.0, 52.0],
            levels: [-8.0, -4.0, -10.0, -10.0],
            mix: [0.7, 0.7, 0.5, 0.3],
            snare_decay: 180.0,
            tom: [120.0, 300.0],
            tom_mix: 0.3,
            div: 1.0,
            lens: [16.0; 6],
        }),
        play: [2.0, 2.0, 10.0, 0.0, 2500.0, -12.0],
        drive: [4.0, -3.0],
        lad: [6000.0, 0.2, 0.0],
        trim_db: 0.0,
        comp: [-18.0, 3.0, 120.0, 0.0],
        duck: None,
        riser: false,
        space: Some(SpaceSpec {
            echo: [40.0, 3500.0, 3500.0],
            sync: 4.0,
            echo_max: 0.35,
            room_decay: 3.5,
            room_max: 0.5,
        }),
        space_macro: 4,
        crush: false,
        chorus: false,
        extra: None,
    };
    let (mut r, h) = assemble(&s);
    let k = h.knob;
    // Weight: the sub from nothing to a full floor shaker.
    rise(
        &mut r,
        k,
        1,
        h.lines[0].gain,
        "gain",
        "gain_db",
        -36.0,
        -4.0,
    );
    // Reese: the saws beat faster, the filter opens, the bus bites.
    rise(&mut r, k, 2, h.lines[1].osc, "osc.va", "detune", 4.0, 40.0);
    rise(
        &mut r,
        k,
        2,
        h.lines[1].tone.unwrap(),
        "filter.svf",
        "cutoff_hz",
        500.0,
        2500.0,
    );
    rise(&mut r, k, 2, h.drive, "drive", "drive_db", 4.0, 20.0);
    rise(&mut r, k, 2, h.drive, "drive", "trim_db", -3.0, -12.0);
    // Break: ghost snares and busy hats come in around the two-step.
    let kit = h.kit.as_ref().unwrap();
    rise(&mut r, k, 3, kit.top, "mixer", "level1", 0.0, 1.0);
    rise(&mut r, k, 3, kit.top, "mixer", "level2", 0.1, 0.7);
    let cues = vec![
        CueDef {
            name: "Intro",
            targets: s.stored,
            morph: 0.0,
            bar: 0.0,
        },
        CueDef {
            name: "Build",
            targets: [0.4, 0.3, 0.5, 0.5],
            morph: 16.0,
            bar: 8.0,
        },
        CueDef {
            name: "Drop",
            targets: [1.0, 0.7, 0.4, 0.3],
            morph: 0.0,
            bar: 16.0,
        },
        CueDef {
            name: "Rolldown",
            targets: [0.7, 0.3, 1.0, 0.5],
            morph: 8.0,
            bar: 32.0,
        },
    ];
    finish(&mut r, &h, &cues);
    (r, cues)
}

fn spillway_phrase() -> Vec<Hit> {
    let bar = 240.0 / 174.0;
    let mut v = chord(0.5 + 16.0 * bar, 6.0 * bar, &[65, 68, 72]);
    v.extend(chord(0.5 + 32.0 * bar, 8.0 * bar, &[65, 68, 72, 75]));
    v
}

// ---- 6. Balcony: a pop loop with verse, pre-chorus, chorus and bridge --------------------------

fn balcony() -> (Rack, Vec<CueDef>) {
    let chord_line = |steps: &'static str| Line {
        wave: 2.0,
        uni: 3.0,
        det: 10.0,
        gate: 90.0,
        env: [20.0, 400.0, 0.6, 300.0],
        lp: 2500.0,
        db: -8.0,
        div: 16.0,
        ..Line::new(steps)
    };
    let bass = prog(
        [0, -5, -3, -7],
        &[Some(0), None, None, Some(0), None, Some(0), None, None],
    );
    let s = Spec {
        names: ["Lift", "Sparkle", "Bounce", "Room"],
        stored: [0.40, 0.15, 0.30, 0.30],
        bpm: 104.0,
        swing: 8.0,
        lines: vec![
            Line {
                wave: 2.0,
                oct: -2.0,
                sub: 0.5,
                gate: 70.0,
                env: [2.0, 200.0, 0.6, 80.0],
                lp: 600.0,
                div: 2.0,
                ..Line::new(bass)
            },
            chord_line("0 -1 -3 -3"),
            chord_line("4 2 0 0"),
            chord_line("7 7 4 5"),
            // The sparkle: a triangle arpeggio of each chord, eighth notes.
            Line {
                wave: 1.0,
                oct: 1.0,
                gate: 50.0,
                env: [1.0, 300.0, 0.0, 100.0],
                db: -30.0,
                lp: 6000.0,
                div: 2.0,
                ..Line::new(
                    "0 4 7 12 7 4 7 4 -1 2 7 14 7 2 7 2 -3 0 4 12 4 0 4 0 -3 0 5 12 5 0 5 0",
                )
            },
        ],
        kit: Some(Kit {
            rows: [
                "X.......X.X.....",
                "....X.......X...",
                "................",
                "x.x.x.x.x.x.x.x.",
                "..............o.",
                "................",
            ],
            kick: [-9.0, 280.0, 6.0, 55.0],
            levels: [-9.0, -12.0, -16.0, -10.0],
            mix: [0.7, 0.7, 0.0, 0.3],
            snare_decay: 220.0,
            tom: [120.0, 300.0],
            tom_mix: 0.3,
            div: 1.0,
            lens: [16.0; 6],
        }),
        play: [2.0, 2.0, 8.0, 0.0, 3000.0, -12.0],
        drive: [3.0, -2.0],
        lad: [800.0, 0.2, 0.0],
        trim_db: 0.0,
        comp: [-18.0, 3.0, 120.0, 0.0],
        duck: Some(220.0),
        riser: false,
        space: Some(SpaceSpec {
            echo: [30.0, 4000.0, 4500.0],
            sync: 4.0,
            echo_max: 0.3,
            room_decay: 2.2,
            room_max: 0.5,
        }),
        space_macro: 4,
        crush: false,
        chorus: true,
        extra: None,
    };
    let (mut r, h) = assemble(&s);
    let k = h.knob;
    // Lift: the filter opens and a chorus widens the chords.
    rise(
        &mut r,
        k,
        1,
        h.lad,
        "filter.ladder",
        "cutoff_hz",
        600.0,
        14000.0,
    );
    rise(
        &mut r,
        k,
        1,
        h.kit.as_ref().unwrap().top_lad,
        "filter.ladder",
        "cutoff_hz",
        2000.0,
        16000.0,
    );
    rise(&mut r, k, 1, h.chorus.unwrap(), "chorus", "mix", 0.0, 55.0);
    level_comp(&mut r, &h, 1, h.trim, 5.0);
    // Sparkle: the arpeggio comes up out of the chords.
    rise(
        &mut r,
        k,
        2,
        h.lines[4].gain,
        "gain",
        "gain_db",
        -40.0,
        -8.0,
    );
    // Bounce: the kick ducks the chords and the bass.
    rise(
        &mut r,
        k,
        3,
        h.duck.unwrap(),
        "compressor",
        "threshold_db",
        0.0,
        -30.0,
    );
    rise(&mut r, k, 3, h.comp, "compressor", "makeup_db", 0.0, 4.0);
    let cues = vec![
        CueDef {
            name: "Verse",
            targets: s.stored,
            morph: 0.0,
            bar: 0.0,
        },
        CueDef {
            name: "Pre",
            targets: [0.6, 0.35, 0.5, 0.35],
            morph: 8.0,
            bar: 8.0,
        },
        CueDef {
            name: "Chorus",
            targets: [0.95, 0.8, 0.8, 0.4],
            morph: 0.0,
            bar: 12.0,
        },
        CueDef {
            name: "Bridge",
            targets: [0.2, 0.45, 0.1, 0.6],
            morph: 4.0,
            bar: 20.0,
        },
        CueDef {
            name: "Last Chorus",
            targets: [1.0, 0.9, 0.9, 0.45],
            morph: 0.0,
            bar: 24.0,
        },
    ];
    finish(&mut r, &h, &cues);
    (r, cues)
}

fn balcony_phrase() -> Vec<Hit> {
    let bar = 240.0 / 104.0;
    let mut v = chord(0.5 + 12.0 * bar, 2.0 * bar, &[72, 76, 79]);
    v.extend(chord(0.5 + 14.0 * bar, 2.0 * bar, &[71, 74, 79]));
    v.extend(chord(0.5 + 24.0 * bar, 2.0 * bar, &[72, 76, 79]));
    v.extend(chord(0.5 + 26.0 * bar, 2.0 * bar, &[71, 74, 79]));
    v
}

// ---- 7. Estuary: an ambient evolution in 5, 7 and 11 -----------------------------------------

fn estuary() -> (Rack, Vec<CueDef>) {
    let pad = |steps: &'static str, db: f32| Line {
        wave: 2.0,
        uni: 4.0,
        det: 12.0,
        oct: -1.0,
        gate: 100.0,
        env: [1500.0, 800.0, 0.9, 2500.0],
        lp: 1400.0,
        db,
        div: 16.0,
        ..Line::new(steps)
    };
    // Bells that run against the bar: 5 beats, 7 beats and 11 eighths, some steps by chance.
    let bell = |steps: &'static str, oct: f32, div: f32, db: f32| Line {
        wave: 0.0,
        oct,
        gate: 30.0,
        env: [2.0, 1800.0, 0.0, 2000.0],
        db,
        lp: 7000.0,
        div,
        after: true,
        ..Line::new(steps)
    };
    let s = Spec {
        names: ["Tide", "Glitter", "Depth", "Wash"],
        stored: [0.20, 0.30, 0.25, 0.60],
        bpm: 60.0,
        swing: 0.0,
        lines: vec![
            pad("2 2 -2 -2 0 0 -5 -5", -6.0),
            pad("9 9 5 5 7 7 2 2", -7.0),
            pad("5 5 2 2 4 4 -1 -1", -10.0),
            bell("2 - 9? - 5", 1.0, 4.0, -30.0),
            bell("14? - - 9 - 12? -", 1.0, 4.0, -30.0),
            bell("- 2 - - 9? - - 5 - - 12?", 2.0, 2.0, -30.0),
            // The depth: a sine drone three octaves down.
            Line {
                wave: 0.0,
                oct: -3.0,
                gate: 100.0,
                env: [3000.0, 800.0, 0.9, 3000.0],
                db: -40.0,
                div: 16.0,
                ..Line::new("2 2 -2 -2 0 0 -5 -5")
            },
            // The crest, silent until Tide and Glitter are both high: a shimmer two octaves over
            // the pads on the added ninths (Dm9, Bbadd9, Cadd9, Gadd9) ...
            Line {
                wave: 1.0,
                uni: 3.0,
                det: 8.0,
                gate: 100.0,
                env: [3000.0, 800.0, 0.9, 3000.0],
                db: -60.0,
                lp: 5000.0,
                div: 16.0,
                ..Line::new("16 16 12 12 14 14 9 9")
            },
            // ... a fourth bell, 13 sixteenths, above the others ...
            bell("21? - - 14 - 17? - - 26? - 19 - -", 1.0, 1.0, -60.0),
            // ... and, with Depth, the pad roots an octave down.
            Line {
                uni: 2.0,
                det: 6.0,
                oct: -2.0,
                gate: 100.0,
                env: [3000.0, 800.0, 0.9, 3000.0],
                db: -60.0,
                lp: 500.0,
                div: 16.0,
                ..Line::new("2 2 -2 -2 0 0 -5 -5")
            },
        ],
        kit: None,
        play: [2.0, 3.0, 12.0, 0.0, 1800.0, -12.0],
        drive: [2.0, -1.0],
        lad: [300.0, 0.25, 0.0],
        trim_db: 2.0,
        comp: [-24.0, 2.0, 300.0, 0.0],
        duck: None,
        riser: true,
        space: Some(SpaceSpec {
            echo: [55.0, 3000.0, 3000.0],
            sync: 4.0,
            echo_max: 0.45,
            room_decay: 12.0,
            room_max: 0.6,
        }),
        space_macro: 4,
        crush: false,
        chorus: true,
        extra: None,
    };
    let (mut r, h) = assemble(&s);
    let k = h.knob;
    // Tide: the pads open from dark and the sea noise rises under them.
    rise(
        &mut r,
        k,
        1,
        h.lad,
        "filter.ladder",
        "cutoff_hz",
        300.0,
        8000.0,
    );
    r.route(k, "m1", h.riser.unwrap(), "gain", 0.3);
    level_comp(&mut r, &h, 1, h.trim, 4.0);
    // Wash: the echo and the long room add level; take the stage down by what they add.
    level_comp(&mut r, &h, 4, h.trim, 2.5);
    // Glitter: the bells come up and ring longer.
    for l in &h.lines[3..6] {
        rise(&mut r, k, 2, l.gain, "gain", "gain_db", -40.0, -10.0);
        rise(&mut r, k, 2, l.seq, "seq", "gate_len", 15.0, 70.0);
    }
    // Depth: the drone rises from below hearing.
    rise(
        &mut r,
        k,
        3,
        h.lines[6].gain,
        "gain",
        "gain_db",
        -40.0,
        -4.0,
    );
    // The crest: what Tide brings in, scaled by how far a second macro is past `c`. It is nothing
    // until that macro passes `c` (Low Tide, Ebb and the stored sound do not hear it) and only
    // High Tide hears most of it. Size, not level: the master stage comes down by what it adds.
    // Every crest param starts at the end of its range (or a value that only gets darker or
    // drier below it), so the clamp holds it there while the second macro is under `c`.
    #[allow(clippy::too_many_arguments)]
    fn crest(
        r: &mut Rack,
        k: ModuleId,
        mac: &str,
        c: f32,
        id: ModuleId,
        kind: &str,
        param: &str,
        lo: f32,
        hi: f32,
    ) {
        let i = param_of(kind, param);
        let a = (i.to_norm(hi) - i.to_norm(lo)) / (1.0 - c);
        r.e.set_param(id, param, lo);
        depth_route(r, k, "m1", id, param, k, mac, a);
        r.route(k, "m1", id, param, -a * c);
    }
    crest(
        &mut r,
        k,
        "m2",
        0.4,
        h.lines[7].gain,
        "gain",
        "gain_db",
        -60.0,
        -2.0,
    );
    crest(
        &mut r,
        k,
        "m2",
        0.4,
        h.lines[8].gain,
        "gain",
        "gain_db",
        -60.0,
        -4.0,
    );
    crest(
        &mut r,
        k,
        "m3",
        0.2,
        h.lines[9].gain,
        "gain",
        "gain_db",
        -60.0,
        20.0,
    );
    let chorus = h.chorus.unwrap();
    r.e.set_param(chorus, "rate_hz", 0.15);
    let verb =
        *r.e.state()
            .modules
            .iter()
            .find(|(_, m)| m.kind == "reverb")
            .unwrap()
            .0;
    crest(&mut r, k, "m2", 0.4, chorus, "chorus", "mix", 0.0, 70.0);
    crest(&mut r, k, "m2", 0.4, verb, "reverb", "width", 60.0, 100.0);
    crest(
        &mut r, k, "m2", 0.4, verb, "reverb", "damp_hz", 3000.0, 6000.0,
    );
    crest(&mut r, k, "m2", 0.4, verb, "reverb", "decay_s", 12.0, 18.0);
    // A longer, wider room for less of the stage: the space grows, its energy does not.
    // (Wash sets the mix from 0 by a route, so this is a route of its own: 20 points down.)
    depth_route(&mut r, k, "m1", verb, "mix", k, "m2", -0.2 / 0.6);
    r.route(k, "m1", verb, "mix", 0.2 * 0.4 / 0.6);
    // The level: the master filter's output (pads, drone, sea, shimmer, low pad) into the sum
    // before the trim, from unity down. The bells after the filter keep theirs.
    let lad_sum =
        r.e.state()
            .cables
            .values()
            .find(|c| c.from.module_id() == Some(h.lad))
            .and_then(|c| c.to.module_id())
            .expect("the master filter feeds a sum");
    crest(&mut r, k, "m2", 0.4, lad_sum, "mixer", "level1", 1.0, 0.25);
    let cues = vec![
        CueDef {
            name: "Low Tide",
            targets: s.stored,
            morph: 0.0,
            bar: 0.0,
        },
        CueDef {
            name: "Rising",
            targets: [0.55, 0.55, 0.4, 0.65],
            morph: 16.0,
            bar: 4.0,
        },
        CueDef {
            name: "High Tide",
            targets: [0.95, 0.9, 0.7, 0.75],
            morph: 16.0,
            bar: 10.0,
        },
        CueDef {
            name: "Ebb",
            targets: [0.15, 0.2, 0.5, 0.9],
            morph: 16.0,
            bar: 16.0,
        },
    ];
    finish(&mut r, &h, &cues);
    (r, cues)
}

fn estuary_phrase() -> Vec<Hit> {
    let bar = 4.0;
    let mut v = chord(0.5 + 6.0 * bar, 3.0 * bar, &[62, 69, 74]);
    v.extend(chord(0.5 + 12.0 * bar, 4.0 * bar, &[60, 67, 72]));
    v
}

// ---- 8. Lichen: a random melody that mutates, in polymeter -----------------------------------

/// The generative part: two random loops (7 and 5 values) quantized to D dorian, each through a
/// cable with its own step pattern, so the lines drift against the 8-step bar.
fn lichen_voices(r: &mut Rack, clock: ModuleId) -> Vec<(ModuleId, &'static str)> {
    let div = r.add("clock.div", &[("div", 2.0)]);
    r.wire(clock, "gate", div, "clock");
    r.wire(clock, "reset", div, "reset");
    let mixer = r.add("mixer", &[("level1", 1.0), ("level2", 0.6)]);
    for (len, wave, uni, db, tr, cutoff, pat, pos) in [
        (
            7.0,
            1.0,
            2.0,
            -6.0,
            -12.0,
            1800.0,
            [("length", 5.0), ("s2", 0.0), ("s4", 0.5)],
            1,
        ),
        (
            5.0,
            0.0,
            1.0,
            -12.0,
            12.0,
            6000.0,
            [("length", 3.0), ("s2", 0.0), ("s3", 0.4)],
            2,
        ),
    ] {
        let rnd = r.add(
            "random",
            &[("length", len), ("change", 0.0), ("range", 12.0)],
        );
        let quant = r.add(
            "quantizer",
            &[("scale", 4.0), ("root", 2.0), ("transpose", tr)],
        );
        let osc = r.add(
            "osc.va",
            &[("waveform", wave), ("unison", uni), ("detune", 8.0)],
        );
        let env = r.add("env.adsr", &adsr(2.0, 320.0, 0.0, 380.0));
        let lad = r.add(
            "filter.ladder",
            &[
                ("cutoff_hz", cutoff),
                ("resonance", 0.25),
                ("drive_db", 0.0),
            ],
        );
        let vca = r.add("vca", &[("gain", 0.0)]);
        let gain = r.add("gain", &[("gain_db", db)]);
        r.wire(div, "gate", rnd, "clock");
        r.wire(rnd, "out", quant, "in");
        r.wire(quant, "out", osc, "pitch");
        r.wire(quant, "trig", env, "gate");
        r.wire(osc, "out", lad, "in");
        r.wire(lad, "out", vca, "in");
        r.wire(env, "out", vca, "cv");
        r.wire(vca, "out", gain, "in");
        r.route(env, "out", lad, "cutoff_hz", 0.3);
        r.pattern(gain, "out", mixer, &format!("in{pos}"), &pat);
    }
    vec![(mixer, "out")]
}

fn lichen() -> (Rack, Vec<CueDef>) {
    let s = Spec {
        names: ["Mutate", "Reach", "Gate", "Echo"],
        stored: [0.05, 0.30, 0.40, 0.35],
        bpm: 112.0,
        swing: 0.0,
        lines: vec![
            // A 7-step bass against the 16-step bar.
            Line {
                wave: 2.0,
                oct: -2.0,
                gate: 60.0,
                env: [2.0, 160.0, 0.5, 60.0],
                lp: 600.0,
                db: 0.0,
                ..Line::new("2 - 9 - 5 - 0")
            },
            // A 5-step stab.
            Line {
                wave: 3.0,
                pw: 30.0,
                oct: 0.0,
                gate: 40.0,
                env: [1.0, 200.0, 0.5, 60.0],
                lp: 2500.0,
                db: -10.0,
                ..Line::new("2 - 9 5 -")
            },
            Line {
                wave: 2.0,
                uni: 4.0,
                det: 14.0,
                oct: -1.0,
                gate: 100.0,
                env: [800.0, 600.0, 0.9, 1500.0],
                lp: 1200.0,
                db: -14.0,
                div: 16.0,
                ..Line::new("2 2 0 0")
            },
        ],
        kit: Some(Kit {
            rows: [
                "X.......X.X.....",
                "....x.......x...",
                "................",
                "x.x.x...........",
                "..o....?........",
                "x..x..o.x.......",
            ],
            kick: [-10.0, 260.0, 8.0, 55.0],
            levels: [-10.0, -16.0, -16.0, -10.0],
            mix: [0.7, 0.5, 0.0, 0.3],
            snare_decay: 180.0,
            tom: [140.0, 260.0],
            tom_mix: 0.5,
            div: 1.0,
            // The hat loops in 5, the open hat in 7 and the toms in 9 against a bar of 16.
            lens: [16.0, 16.0, 16.0, 5.0, 7.0, 9.0],
        }),
        play: [1.0, 2.0, 8.0, 0.0, 2500.0, -12.0],
        drive: [3.0, -2.0],
        lad: [3000.0, 0.2, 0.0],
        trim_db: 0.0,
        comp: [-18.0, 3.0, 120.0, 0.0],
        duck: None,
        riser: false,
        space: Some(SpaceSpec {
            echo: [45.0, 3500.0, 3500.0],
            sync: 4.0,
            echo_max: 0.4,
            room_decay: 2.5,
            room_max: 0.4,
        }),
        space_macro: 4,
        crush: false,
        chorus: false,
        extra: Some(lichen_voices),
    };
    let (mut r, h) = assemble(&s);
    let k = h.knob;
    let randoms: Vec<ModuleId> =
        r.e.state()
            .modules
            .iter()
            .filter(|(_, m)| m.kind == "random")
            .map(|(&id, _)| id)
            .collect();
    // Mutate: the random loops stop repeating and start to change as they come round.
    for &id in &randoms {
        rise(&mut r, k, 1, id, "random", "change", 0.0, 70.0);
    }
    // Reach: the random melodies span a third, then two octaves.
    for &id in &randoms {
        rise(&mut r, k, 2, id, "random", "range", 4.0, 30.0);
    }
    // Gate: the bass and stab notes from ticks to held.
    for l in &h.lines[..2] {
        rise(&mut r, k, 3, l.seq, "seq", "gate_len", 12.0, 95.0);
    }
    let cues = vec![
        CueDef {
            name: "Spore",
            targets: s.stored,
            morph: 0.0,
            bar: 0.0,
        },
        CueDef {
            name: "Colony",
            targets: [0.4, 0.5, 0.6, 0.45],
            morph: 16.0,
            bar: 4.0,
        },
        CueDef {
            name: "Bloom",
            targets: [0.8, 0.9, 0.9, 0.6],
            morph: 8.0,
            bar: 12.0,
        },
        CueDef {
            name: "Die-back",
            targets: [0.0, 0.2, 0.2, 0.8],
            morph: 16.0,
            bar: 20.0,
        },
    ];
    finish(&mut r, &h, &cues);
    (r, cues)
}

fn lichen_phrase() -> Vec<Hit> {
    let bar = 240.0 / 112.0;
    let mut v = chord(0.5 + 12.0 * bar, 3.0 * bar, &[62, 69, 74]);
    v.extend(chord(0.5 + 20.0 * bar, 4.0 * bar, &[57, 62, 65]));
    v
}

// ---- 9. Roller Rink: synthwave with an intro, verse, chorus and break --------------------------

fn roller_rink() -> (Rack, Vec<CueDef>) {
    let pad = |steps: &'static str, db: f32| Line {
        wave: 2.0,
        uni: 5.0,
        det: 18.0,
        gate: 100.0,
        env: [80.0, 400.0, 0.8, 500.0],
        lp: 3000.0,
        db,
        div: 16.0,
        ..Line::new(steps)
    };
    let bass = prog(
        [-3, -7, 0, -5],
        &[
            Some(0),
            Some(0),
            Some(12),
            Some(0),
            Some(0),
            Some(12),
            Some(0),
            Some(12),
        ],
    );
    let s = Spec {
        names: ["Glow", "Arp", "Gated", "Space"],
        stored: [0.20, 0.20, 0.20, 0.45],
        bpm: 100.0,
        swing: 0.0,
        lines: vec![
            Line {
                wave: 2.0,
                uni: 2.0,
                det: 8.0,
                oct: -2.0,
                gate: 80.0,
                env: [2.0, 150.0, 0.7, 60.0],
                lp: 900.0,
                div: 2.0,
                ..Line::new(bass)
            },
            pad("-3 -3 -5 -5", -9.0),
            pad("0 0 0 -1", -10.0),
            pad("4 5 4 2", -11.0),
            // The arpeggio: pulse notes through the chords, eighths.
            Line {
                wave: 3.0,
                pw: 40.0,
                gate: 60.0,
                env: [1.0, 160.0, 0.3, 60.0],
                lp: 5000.0,
                db: -10.0,
                div: 2.0,
                ..Line::new("-3 0 4 9 4 0 4 9 -3 0 5 9 5 0 5 9 -5 0 4 7 4 0 4 7 -5 -1 2 7 2 -1 2 7")
            },
        ],
        kit: Some(Kit {
            rows: [
                "X.......X.......",
                "....X.......X...",
                "................",
                "x.x.x.x.x.x.x.x.",
                "................",
                "...............?",
            ],
            kick: [-8.0, 280.0, 6.0, 52.0],
            levels: [-8.0, -12.0, -16.0, -10.0],
            mix: [0.7, 0.8, 0.0, 0.3],
            snare_decay: 350.0,
            tom: [110.0, 300.0],
            tom_mix: 0.4,
            div: 1.0,
            lens: [16.0; 6],
        }),
        play: [2.0, 3.0, 12.0, 0.0, 3500.0, -12.0],
        drive: [3.0, -2.0],
        lad: [1000.0, 0.2, 0.0],
        trim_db: 0.0,
        comp: [-18.0, 3.0, 120.0, 0.0],
        duck: None,
        riser: false,
        space: Some(SpaceSpec {
            echo: [50.0, 3500.0, 4500.0],
            sync: 4.0,
            echo_max: 0.4,
            room_decay: 3.5,
            room_max: 0.5,
        }),
        space_macro: 4,
        crush: false,
        chorus: true,
        extra: None,
    };
    let (mut r, h) = assemble(&s);
    let k = h.knob;
    // Glow: the filter opens and a deep chorus spreads the chords.
    rise(
        &mut r,
        k,
        1,
        h.lad,
        "filter.ladder",
        "cutoff_hz",
        400.0,
        12000.0,
    );
    rise(
        &mut r,
        k,
        1,
        h.kit.as_ref().unwrap().top_lad,
        "filter.ladder",
        "cutoff_hz",
        1500.0,
        16000.0,
    );
    rise(&mut r, k, 1, h.chorus.unwrap(), "chorus", "mix", 0.0, 60.0);
    level_comp(&mut r, &h, 1, h.trim, 5.0);
    // Arp: the arpeggio comes up and its notes lengthen.
    rise(
        &mut r,
        k,
        2,
        h.lines[4].gain,
        "gain",
        "gain_db",
        -40.0,
        -6.0,
    );
    rise(&mut r, k, 2, h.lines[4].seq, "seq", "gate_len", 15.0, 95.0);
    // Gated: the snare gets louder and rings into the big gated tail.
    let kit = h.kit.as_ref().unwrap();
    rise(&mut r, k, 3, kit.snare, "drum.snare", "drive_db", 0.0, 18.0);
    rise(
        &mut r,
        k,
        3,
        kit.snare,
        "drum.snare",
        "decay_ms",
        150.0,
        900.0,
    );
    let cues = vec![
        CueDef {
            name: "Intro",
            targets: [0.2, 0.2, 0.2, 0.45],
            morph: 0.0,
            bar: 0.0,
        },
        CueDef {
            name: "Verse",
            targets: [0.5, 0.45, 0.4, 0.45],
            morph: 0.0,
            bar: 4.0,
        },
        CueDef {
            name: "Chorus",
            targets: [0.95, 0.9, 0.9, 0.6],
            morph: 0.0,
            bar: 12.0,
        },
        CueDef {
            name: "Break",
            targets: [0.15, 0.1, 0.6, 0.7],
            morph: 4.0,
            bar: 20.0,
        },
    ];
    finish(&mut r, &h, &cues);
    (r, cues)
}

fn roller_rink_phrase() -> Vec<Hit> {
    let bar = 2.4;
    let mut v = chord(0.5 + 12.0 * bar, 2.0 * bar, &[69, 72, 76]);
    v.extend(chord(0.5 + 14.0 * bar, 2.0 * bar, &[67, 71, 74]));
    v.extend(chord(0.5 + 4.0 * bar, 4.0 * bar, &[76, 72]));
    v
}

// ---- the list --------------------------------------------------------------------------------

fn presets() -> Vec<Preset> {
    vec![Preset {
        slug: "gantry",
        name: "Gantry",
        tags: &["sequenced", "cues", "drums", "drop", "techno", "132bpm"],
        description: "A self-playing techno build and drop at 132 BPM: a four-on-the-floor kick with a clap, closed and open hats, a low-passed rumble between the kicks, an acid stab line and a slow pad, ducking to the kick. Press Run, then launch the five cues with the pads or CC 40-44: Intro, Build, Drop, Break, Peak. Climb opens the master filter and brings in a noise riser, Rumble brings the off-beat bass up and lengthens it, Acid makes the stab line squeal and ring longer, Pump sets how hard the kick ducks the music.",
        listen: "The filter and riser rising through the Build, the Drop landing with the rumble in, the Break pulling back.",
        build: gantry,
        phrase: gantry_phrase,
        tour_chord: &[57, 60, 64],
        bpm: 132.0,
        bars: 36.0,
        trim: 0.0,
    },
    Preset {
        slug: "dead-calm",
        name: "Dead Calm",
        tags: &["sequenced", "cues", "cinematic", "drone", "swell", "66bpm"],
        description: "A self-playing cinematic piece at 66 BPM: a minor drone and pad that change chord every bar pair, a dissonant cluster waiting underneath, a sparse high bell that plays half its notes by chance, and a low impact with a heartbeat of toms every two bars. Press Run, then launch the four cues with the pads or CC 40-43: Hush, Swell, Impact, Aftermath. Swell opens the filter and fades in a noise riser, Dread brings in a dissonant cluster, Pulse brings in the low impact and heartbeat, Hall sets the echo and the long room.",
        listen: "The swell building to the impact, the heartbeat toms, then the long room tail of the Aftermath.",
        build: dead_calm,
        phrase: dead_calm_phrase,
        tour_chord: &[48, 55, 63],
        bpm: 66.0,
        bars: 20.0,
        trim: 0.0,
    },
    Preset {
        slug: "pawn-shop",
        name: "Pawn Shop",
        tags: &["sequenced", "cues", "drums", "boom bap", "hip-hop", "88bpm"],
        description: "A self-playing boom-bap loop at 88 BPM with swing: a dusty kick and snare, off-beat hats, a sine sub and a minor-ninth chord through a hot tape-style filter. Press Run, then launch the four cues with the pads or CC 40-43: Verse, Hook, Bridge, Outro. Swing pushes the off-beats later, Dust brings in vinyl hiss and a worn-tape wobble, Tape opens the loop from muffled and saturated to clean, Room adds a slapback echo and a small room.",
        listen: "The loop opening up at the Hook, the muffled dusty Bridge, and the swing dragging in the Outro.",
        build: pawn_shop,
        phrase: pawn_shop_phrase,
        tour_chord: &[53, 56, 60],
        bpm: 88.0,
        bars: 24.0,
        trim: 0.0,
    },
    Preset {
        slug: "laundromat",
        name: "Laundromat",
        tags: &["sequenced", "cues", "drums", "disco", "french touch", "122bpm", "signature"],
        description: "A self-playing filtered disco loop at 122 BPM with swing: an octave bass, two off-beat stab lines, a kick, clap and hats, all ducking to the kick. It is dry on purpose: no echo, no room. Press Run, then launch the four cues with the pads or CC 40-43: Loop, Cut-up, Break, Full. Sweep is a DJ filter over the loop, Crunch bitcrushes and drives the bus, Stutter turns the stabs from clipped ticks into held chords, Pump sets how hard the kick ducks the loop.",
        listen: "The cut-up stabs at the second section, the dark break with held chords, and the full loop with the crusher in.",
        build: laundromat,
        phrase: laundromat_phrase,
        tour_chord: &[57, 60, 64],
        bpm: 122.0,
        bars: 30.0,
        trim: 0.0,
    },
    Preset {
        slug: "spillway",
        name: "Spillway",
        tags: &["sequenced", "cues", "drums", "two-step", "reese", "drum and bass", "174bpm"],
        description: "A self-playing drum and bass piece at 174 BPM: a two-step kick and snare, a sine sub and a beating reese in F minor, and a slow pad bed. Press Run, then launch the four cues with the pads or CC 40-43: Intro, Build, Drop, Rolldown. Weight brings in the sub, Reese makes the saws beat faster and bite, Break adds ghost snares and busy hats, Haze sets the echo and the room.",
        listen: "The sub and the reese landing at the Drop, then ghost snares and hats filling the Rolldown.",
        build: spillway,
        phrase: spillway_phrase,
        tour_chord: &[53, 56, 60],
        bpm: 174.0,
        bars: 48.0,
        trim: 0.0,
    },
    Preset {
        slug: "balcony",
        name: "Balcony",
        tags: &["sequenced", "cues", "drums", "chords", "pop", "104bpm"],
        description: "A self-playing pop loop at 104 BPM: a four-chord progression in C, a round bass, a kick and snare, a triangle arpeggio hiding under the chords and a chorus. Press Run, then launch the five cues with the pads or CC 40-44: Verse, Pre, Chorus, Bridge, Last Chorus. Lift opens the filter and widens the chords, Sparkle brings the arpeggio up, Bounce sets how hard the kick ducks the chords, Room adds echo and a small room.",
        listen: "The Pre building into the Chorus with the arpeggio and chorus in, then the Bridge pulling everything back.",
        build: balcony,
        phrase: balcony_phrase,
        tour_chord: &[60, 64, 67],
        bpm: 104.0,
        bars: 32.0,
        trim: 0.0,
    },
    Preset {
        slug: "estuary",
        name: "Estuary",
        tags: &["sequenced", "cues", "polymeter", "probability", "ambient", "60bpm"],
        description: "A self-playing ambient piece at 60 BPM, with no drums: slow pads that change chord every bar, three bells that cycle in 5, 7 and 11 steps against it with some notes played by chance, a sine drone and a sea of noise. Press Run, then launch the four cues with the pads or CC 40-43: Low Tide, Rising, High Tide, Ebb. Tide opens the pads and lets the sea in, Glitter brings the bells up and lets them ring, Depth raises the drone from below, Wash sets the echo and the long room. High Tide is louder than Low Tide.",
        listen: "The bells drifting against the pads, the pads opening towards High Tide and the long wash of the Ebb.",
        build: estuary,
        phrase: estuary_phrase,
        tour_chord: &[50, 57, 62],
        bpm: 60.0,
        bars: 22.0,
        trim: 0.0,
    },
    Preset {
        slug: "lichen",
        name: "Lichen",
        tags: &["sequenced", "cues", "drums", "polymeter", "random", "experimental", "112bpm"],
        description: "A self-playing experimental piece at 112 BPM: two random melodies held to D dorian that loop in 7 and 5 steps, each through a cable with its own step pattern, over a bass in 7, a stab in 5 and hats, open hats and toms looping in 5, 7 and 9 against a bar of 16. Press Run, then launch the four cues with the pads or CC 40-43: Spore, Colony, Bloom, Die-back. Mutate lets the random loops change as they come round, Reach widens their range from a third to two octaves, Gate turns the bass and stab from ticks into held notes, Echo sets the echo and the room.",
        listen: "The loops that repeat at Spore, then mutate and spread through Colony and Bloom; the Die-back settling into a wet echo.",
        build: lichen,
        phrase: lichen_phrase,
        tour_chord: &[50, 57, 62],
        bpm: 112.0,
        bars: 28.0,
        trim: 0.0,
    },
    Preset {
        slug: "roller-rink",
        name: "Roller Rink",
        tags: &["sequenced", "cues", "drums", "arp", "synthwave", "100bpm", "80s"],
        description: "A self-playing synthwave piece at 100 BPM: a running eighth-note bass, lush saw chords through a chorus, a pulse arpeggio, a kick and a big gated snare, in A minor. Press Run, then launch the four cues with the pads or CC 40-43: Intro, Verse, Chorus, Break. Glow opens the filter and deepens the chorus, Arp brings the arpeggio up and lengthens its notes, Gated makes the snare louder and its gated tail longer, Space sets the echo and the room.",
        listen: "The chorus opening up with the arpeggio and gated snare, then the Break closing the filter.",
        build: roller_rink,
        phrase: roller_rink_phrase,
        tour_chord: &[57, 60, 64],
        bpm: 100.0,
        bars: 28.0,
        trim: 0.0,
    },
    ]
}

// ---- rendering -------------------------------------------------------------------------------

fn write_wav(path: &Path, left: &[f32], right: &[f32]) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: sr() as u32,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for (l, r) in left.iter().zip(right) {
        w.write_sample(*l).unwrap();
        w.write_sample(*r).unwrap();
    }
    w.finalize().unwrap();
}

/// Plays `hits` through the engine for `secs` seconds. The clock runs at 0.5 s; with `cues` the
/// piece launches its sections, each half a bar early so the cue lands on its bar line.
fn render(
    state: &kabl_core::PatchState,
    hits: &[Hit],
    secs: f32,
    cues_def: Option<(&[CueDef], f32)>,
) -> (Vec<f32>, Vec<f32>) {
    let collector = Collector::new();
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
        if let (Some((defs, bpm)), Some(id)) = (cues_def, cue_id) {
            let bar_s = 240.0 / bpm;
            if let Some(c) = defs.get(next_cue) {
                let at = 0.5 + c.bar * bar_s - bar_s * 0.5;
                if t1 > at {
                    let mut cmds = Vec::new();
                    cues::launch(&mut cmds, state, id, next_cue + 1).expect("launch");
                    for mut cmd in cmds {
                        if let Command::Launch(la) = &mut cmd {
                            if next_cue == 0 {
                                la.timing = Timing::Now;
                            }
                        }
                        engine.command(&cmd);
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

/// Peak and loudness (dBFS, 100 ms blocks above -50 dBFS) the way the factory level test
/// renders: 24 s, Run at 1 s, every macro at 0.65 from 3 s and at 0.15 from 7 s, or at 1 for
/// the macros in `hot` (bit k is macro k+1); a held chord at 1, 5 and 13 s.
fn level_of(state: &kabl_core::PatchState, hot: u8) -> (f64, f64) {
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
    let collector = Collector::new();
    let mut engine = PatchEngine::new(&collector.handle(), state, 48000.0, 8).expect("compiles");
    let clocks: Vec<ModuleId> = state
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "clock")
        .map(|(&id, _)| id)
        .collect();
    let macros: Vec<ModuleId> = state
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "macro")
        .map(|(&id, _)| id)
        .collect();
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
        if t.is_multiple_of(48000) && [3, 7, 19].contains(&(t / 48000)) {
            for &id in &macros {
                for n in 1..=4u16 {
                    let v = if hot & (1 << (n - 1)) != 0 {
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
                        value: v,
                    });
                }
            }
        }
        if t.is_multiple_of(48000) && [1, 5, 13].contains(&(t / 48000)) {
            for n in [52, 59, 64, 67] {
                engine.key(KeyEvent::On {
                    note: n,
                    velocity: 100,
                });
            }
        }
        if t.is_multiple_of(48000) && [4, 8, 16].contains(&(t / 48000)) {
            engine.key(KeyEvent::AllOff);
        }
        if t == 16 * 48000 {
            for &c in &clocks {
                engine.transport(c, Transport::Stop);
            }
        }
        engine.process_block(&mut l, &mut r);
        for (a, c) in l.iter().zip(&r) {
            samples.extend_from_slice(&[*a, *c]);
        }
    }
    let peak = samples.iter().fold(0.0_f32, |a, b| a.max(b.abs())) as f64;
    let (mut sum, mut n) = (0.0_f64, 0usize);
    for block in samples.chunks(9600) {
        let ms = block.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / block.len() as f64;
        if ms > 1e-5 {
            sum += ms;
            n += 1;
        }
    }
    let db = |v: f64| 20.0 * v.max(1e-12).log10();
    (db(peak), db((sum / n.max(1) as f64).sqrt()))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("write") => {
            for p in presets() {
                let (mut rack, _) = (p.build)();
                rack.trim(p.trim);
                let dir = Path::new(&args[2]).join("presets").join(p.slug);
                let _ = std::fs::remove_dir_all(&dir);
                kabl_core::save(&dir, rack.e.log()).expect("save");
                let (keys, sequence) = Meta::play_of(rack.e.state());
                let meta = Meta {
                    name: p.name.into(),
                    category: "Piece".into(),
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
            // clips <patches> <out dir> [slug ...]
            let out = Path::new(&args[3]);
            std::fs::create_dir_all(out).unwrap();
            for p in presets() {
                if args.len() > 4 && !args[4..].iter().any(|a| a == p.slug) {
                    continue;
                }
                let log = kabl_core::load(&Path::new(&args[2]).join("presets").join(p.slug))
                    .expect("load patch");
                let (_, defs) = (p.build)();
                if std::env::var("PP_DEBUG").is_ok() {
                    let id = *log
                        .state()
                        .modules
                        .iter()
                        .find(|(_, m)| m.kind == "cues")
                        .unwrap()
                        .0;
                    for c in cues::cues(log.state(), id) {
                        eprintln!("{:?}", c);
                    }
                }
                let secs = 0.5 + p.bars * 240.0 / p.bpm + 3.0;
                let (l, r) = render(log.state(), &(p.phrase)(), secs, Some((&defs, p.bpm)));
                write_wav(&out.join(format!("{}.wav", p.slug)), &l, &r);
                let peak = l.iter().chain(&r).fold(0.0f32, |m, s| m.max(s.abs()));
                println!(
                    "{:16} {secs:.1} s peak {:.1} dBFS | {}",
                    p.slug,
                    20.0 * peak.max(1e-9).log10(),
                    p.listen
                );
            }
        }
        Some("phrases") => {
            // For macro_audit and host.py: {slug: {secs, hits: [[t, note, velocity|0]]}}.
            let entries: Vec<String> = presets()
                .iter()
                .map(|p| {
                    // The phrase is the steady tour chord (4 bars, looped by macro_audit), so the
                    // tour hears the macro and not a phrase that changes with time. `PP_RUN=1`
                    // writes the cue-run phrase instead.
                    let bar = 240.0 / p.bpm;
                    let hits_v = if std::env::var("PP_RUN").is_ok() {
                        (p.phrase)()
                    } else {
                        chord(0.5, 3.8 * bar, p.tour_chord)
                    };
                    let hits: Vec<String> = hits_v
                        .iter()
                        .map(|h| format!("[{:.3},{},{}]", h.0, h.1, h.2.unwrap_or(0)))
                        .collect();
                    // For host.py: the cue pads (CC 40 up) launched half a bar before their bar.
                    let (_, defs) = (p.build)();
                    let cues: Vec<String> = defs
                        .iter()
                        .zip(40..)
                        .map(|(c, cc)| {
                            format!(
                                "[{cc},{:.3}]",
                                (0.5 + (c.bar - 0.5).max(0.0) * bar).max(0.6)
                            )
                        })
                        .collect();
                    format!(
                        "\"{}\":{{\"secs\":{},\"hits\":[{}],\"cues\":[{}]}}",
                        p.slug,
                        0.5 + p.bars * 240.0 / p.bpm,
                        hits.join(","),
                        cues.join(",")
                    )
                })
                .collect();
            std::fs::write(&args[2], format!("{{{}}}\n", entries.join(","))).unwrap();
        }
        Some("library") => {
            // The entries for FACTORY in crates/ui/tests/library.rs.
            for p in presets() {
                println!(
                    "    (\n        \"presets/{}\",\n        {:?},\n        \"Piece\",\n        &{:?},\n        {:?},\n    ),",
                    p.slug, p.name, p.tags, p.description
                );
            }
        }
        Some("levels") => {
            // levels <patches> [slug ...]: the factory level test's cases for the pieces.
            for p in presets() {
                if args.len() > 3 && !args[3..].iter().any(|a| a == p.slug) {
                    continue;
                }
                let log = kabl_core::load(&Path::new(&args[2]).join("presets").join(p.slug))
                    .expect("load patch");
                let mut line = format!("{:14}", p.slug);
                for (label, hot) in [
                    ("normal", 0u8),
                    ("all", 0b1111),
                    ("m1", 1),
                    ("m2", 2),
                    ("m3", 4),
                    ("m4", 8),
                ] {
                    let (peak, loud) = level_of(log.state(), hot);
                    line += &format!("  {label}: {peak:6.1}/{loud:6.1}");
                }
                println!("{line}");
            }
        }
        _ => eprintln!("usage: presets_piece write|clips|phrases|levels <patches dir> ..."),
    }
}

impl Rack {
    /// Adds `db` to every gain stage's `gain_db`... only the trim after the master filter.
    fn trim(&mut self, db: f32) {
        if db == 0.0 {
            return;
        }
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
