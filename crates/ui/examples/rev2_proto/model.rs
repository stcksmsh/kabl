//! Prototype patch model: module kinds, control values, jack cables, knob routes, undo.
//!
//! Deliberately small and local to the prototype. Nothing here is the production patch format;
//! values live as normalized knob travel (0..1 in the param's taper) because route amounts are
//! defined in travel space (revision-2 INTERACTIONS §2.2).

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Midi,
    Osc,
    Lfo,
    Filter,
    Adsr,
    Vca,
    Out,
    Ensemble,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Taper {
    Lin,
    Exp,
}

#[derive(Clone, Copy, Debug)]
pub enum Spec {
    Knob {
        min: f32,
        max: f32,
        taper: Taper,
        unit: &'static str,
        default: f32,
        large: bool,
    },
    Select {
        options: &'static [&'static str],
        default: usize,
    },
}

pub struct CtlDef {
    pub id: &'static str,
    pub label: &'static str,
    pub spec: Spec,
    /// Marked ◆: exists in the design, not in the engine.
    pub conceptual: bool,
    pub primary: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sig {
    Audio,
    Cv,
    Gate,
    Pitch,
}

pub struct JackDef {
    pub id: &'static str,
    pub label: &'static str,
    pub out: bool,
    pub sig: Sig,
    pub conceptual: bool,
    /// Local position; `x < 0` means "from the face's right edge".
    pub x: f32,
    pub y: f32,
    pub label_right: bool,
}

pub struct KindDef {
    pub name: &'static str,
    pub tag: &'static str,
    pub base_wu: u32,
    pub knob_y: f32,
    pub controls: &'static [CtlDef],
    pub jacks: &'static [JackDef],
    /// Bipolar source (±1) when true, unipolar (0..1) otherwise. Only meaningful for outputs.
    pub bipolar_out: bool,
}

const fn knob(
    id: &'static str,
    label: &'static str,
    min: f32,
    max: f32,
    taper: Taper,
    unit: &'static str,
    default: f32,
    large: bool,
    conceptual: bool,
    primary: bool,
) -> CtlDef {
    CtlDef {
        id,
        label,
        spec: Spec::Knob { min, max, taper, unit, default, large },
        conceptual,
        primary,
    }
}

const fn select(
    id: &'static str,
    label: &'static str,
    options: &'static [&'static str],
    default: usize,
    conceptual: bool,
    primary: bool,
) -> CtlDef {
    CtlDef { id, label, spec: Spec::Select { options, default }, conceptual, primary }
}

const fn jack(
    id: &'static str,
    label: &'static str,
    out: bool,
    sig: Sig,
    x: f32,
    y: f32,
) -> JackDef {
    JackDef { id, label, out, sig, conceptual: false, x, y, label_right: false }
}

const fn jack_r(id: &'static str, label: &'static str, out: bool, sig: Sig, y: f32) -> JackDef {
    JackDef { id, label, out, sig, conceptual: false, x: 40.0, y, label_right: true }
}

use Taper::{Exp, Lin};

static OSC: KindDef = KindDef {
    name: "VA Oscillator",
    tag: "osc.va",
    base_wu: 7,
    knob_y: 112.0,
    controls: &[
        knob("base_hz", "Frequency", 20.0, 20000.0, Exp, "Hz", 261.63, true, false, true),
        select("waveform", "Waveform", &["SIN", "TRI", "SAW", "SQR"], 2, false, true),
    ],
    jacks: &[
        jack("pitch", "Pitch", false, Sig::Pitch, 40.0, 272.0),
        jack("sync", "Sync", false, Sig::Gate, 0.0, 272.0),
        jack("out", "Out", true, Sig::Audio, -40.0, 272.0),
    ],
    bipolar_out: true,
};

static LFO: KindDef = KindDef {
    name: "LFO",
    tag: "lfo",
    base_wu: 7,
    knob_y: 112.0,
    controls: &[
        knob("rate_hz", "Rate", 0.01, 100.0, Exp, "Hz", 0.8, true, false, true),
        select("waveform", "Waveform", &["SIN", "TRI", "SAW", "SQR", "S&H"], 1, false, true),
        knob("fine", "Fine", -10.0, 10.0, Lin, "%", 0.0, false, true, false),
        knob("phase", "Phase", 0.0, 360.0, Lin, "°", 0.0, false, true, false),
        knob("fade_ms", "Fade-in", 1.0, 10000.0, Exp, "ms", 1200.0, false, true, false),
        knob("amp", "Amplitude", 0.0, 1.0, Lin, "", 1.0, false, true, false),
        knob("offset", "Offset", -1.0, 1.0, Lin, "±", 0.0, false, true, false),
        select("sync", "Sync", &["FREE", "TEMPO"], 0, true, false),
        select("polarity", "Polarity", &["BI", "UNI"], 0, true, false),
        select("trigger", "Trigger", &["FREE", "NOTE", "ONCE"], 1, true, false),
        select("scope", "Scope", &["VOICE", "GLOBAL"], 0, true, false),
    ],
    jacks: &[
        JackDef {
            id: "trig",
            label: "Trig",
            out: false,
            sig: Sig::Gate,
            conceptual: true,
            x: 40.0,
            y: 272.0,
            label_right: false,
        },
        jack("out", "Out", true, Sig::Cv, -44.0, 272.0),
    ],
    bipolar_out: true,
};

static FILTER: KindDef = KindDef {
    name: "SVF Filter",
    tag: "filter.svf",
    base_wu: 7,
    knob_y: 112.0,
    controls: &[
        knob("cutoff_hz", "Cutoff", 20.0, 20000.0, Exp, "Hz", 1200.0, true, false, true),
        knob("resonance", "Resonance", 0.0, 0.95, Lin, "", 0.35, false, false, true),
    ],
    jacks: &[
        jack("in", "In", false, Sig::Audio, 0.0, 212.0),
        jack("lp", "LP", true, Sig::Audio, 40.0, 290.0),
        jack("bp", "BP", true, Sig::Audio, 0.0, 290.0),
        jack("hp", "HP", true, Sig::Audio, -40.0, 290.0),
    ],
    bipolar_out: true,
};

static MIDI: KindDef = KindDef {
    name: "MIDI In",
    tag: "midi.in",
    base_wu: 5,
    knob_y: 112.0,
    controls: &[],
    jacks: &[
        jack_r("pitch", "Pitch", true, Sig::Pitch, 206.0),
        jack_r("velocity", "Velocity", true, Sig::Cv, 254.0),
        jack_r("gate", "Gate", true, Sig::Gate, 302.0),
    ],
    bipolar_out: false,
};

static ADSR: KindDef = KindDef {
    name: "ADSR Envelope",
    tag: "env.adsr",
    base_wu: 8,
    knob_y: 168.0,
    controls: &[
        knob("attack_ms", "Attack", 0.1, 10000.0, Exp, "ms", 8.0, false, false, true),
        knob("decay_ms", "Decay", 0.1, 10000.0, Exp, "ms", 240.0, false, false, true),
        knob("sustain", "Sustain", 0.0, 1.0, Lin, "", 0.6, false, false, true),
        knob("release_ms", "Release", 0.1, 10000.0, Exp, "ms", 420.0, false, false, true),
    ],
    jacks: &[
        jack("gate", "Gate", false, Sig::Gate, 60.0, 272.0),
        jack("out", "Out", true, Sig::Cv, -60.0, 272.0),
    ],
    bipolar_out: false,
};

static VCA: KindDef = KindDef {
    name: "VCA",
    tag: "vca",
    base_wu: 7,
    knob_y: 112.0,
    controls: &[
        knob("gain", "Gain", 0.0, 1.0, Lin, "", 0.85, true, false, true),
        select("response", "Response", &["LIN", "EXP"], 0, false, true),
    ],
    jacks: &[
        jack("in", "In", false, Sig::Audio, 34.0, 272.0),
        jack("cv", "CV", false, Sig::Cv, 0.0, 272.0),
        jack("out", "Out", true, Sig::Audio, -34.0, 272.0),
    ],
    bipolar_out: true,
};

static OUT: KindDef = KindDef {
    name: "Output",
    tag: "out",
    base_wu: 5,
    knob_y: 112.0,
    controls: &[],
    jacks: &[
        jack_r("left", "Left", false, Sig::Audio, 230.0),
        jack_r("right", "Right", false, Sig::Audio, 278.0),
    ],
    bipolar_out: true,
};

static ENSEMBLE: KindDef = KindDef {
    name: "Night Ensemble",
    tag: "user.ensemble · skin",
    base_wu: 7,
    knob_y: 120.0,
    controls: &[
        knob("rate", "Rate", 0.05, 10.0, Exp, "Hz", 0.6, false, false, true),
        knob("depth", "Depth", 0.0, 1.0, Lin, "", 0.45, false, false, true),
        knob("mix", "Mix", 0.0, 1.0, Lin, "", 0.7, false, false, true),
        select("mode", "Mode", &["I", "II", "I+II"], 2, false, true),
    ],
    jacks: &[
        jack("in", "In", false, Sig::Audio, 38.0, 290.0),
        jack("l", "L", true, Sig::Audio, -88.0, 290.0),
        jack("r", "R", true, Sig::Audio, -38.0, 290.0),
    ],
    bipolar_out: true,
};

pub fn def(kind: Kind) -> &'static KindDef {
    match kind {
        Kind::Midi => &MIDI,
        Kind::Osc => &OSC,
        Kind::Lfo => &LFO,
        Kind::Filter => &FILTER,
        Kind::Adsr => &ADSR,
        Kind::Vca => &VCA,
        Kind::Out => &OUT,
        Kind::Ensemble => &ENSEMBLE,
    }
}

// ------------------------------------------------------------------------------ values

pub fn to_value(spec: &Spec, t: f32) -> f32 {
    match *spec {
        Spec::Knob { min, max, taper: Taper::Exp, .. } => min * (max / min).powf(t.clamp(0.0, 1.0)),
        Spec::Knob { min, max, .. } => min + (max - min) * t.clamp(0.0, 1.0),
        Spec::Select { options, .. } => (t.round() as usize).min(options.len() - 1) as f32,
    }
}

pub fn to_t(spec: &Spec, v: f32) -> f32 {
    match *spec {
        Spec::Knob { min, max, taper: Taper::Exp, .. } => {
            ((v.max(min) / min).ln() / (max / min).ln()).clamp(0.0, 1.0)
        }
        Spec::Knob { min, max, .. } => ((v - min) / (max - min)).clamp(0.0, 1.0),
        Spec::Select { .. } => v,
    }
}

pub fn fmt_value(spec: &Spec, t: f32) -> String {
    let v = to_value(spec, t);
    match *spec {
        Spec::Select { options, .. } => options[v as usize].to_string(),
        Spec::Knob { unit: "ms", .. } => {
            if v >= 1000.0 {
                format!("{:.2} s", v / 1000.0)
            } else if v >= 100.0 {
                format!("{v:.0} ms")
            } else if v >= 10.0 {
                format!("{v:.1} ms")
            } else {
                format!("{v:.2} ms")
            }
        }
        Spec::Knob { unit: "Hz", .. } => {
            if v >= 1000.0 {
                format!("{:.2} kHz", v / 1000.0)
            } else if v >= 100.0 {
                format!("{v:.1} Hz")
            } else {
                format!("{v:.2} Hz")
            }
        }
        Spec::Knob { unit: "%", .. } => format!("{v:+.1} %"),
        Spec::Knob { unit: "°", .. } => format!("{v:.0}°"),
        Spec::Knob { unit: "±", .. } => format!("{v:+.2}"),
        Spec::Knob { .. } => format!("{v:.2}"),
    }
}

/// Parses typed entry: `8`, `8 ms`, `1.2k`, `1.2 kHz`, `2 s`, `0.35`. Returns travel `t`.
pub fn parse_value(spec: &Spec, text: &str) -> Option<f32> {
    let s = text.trim().to_lowercase();
    if let Spec::Select { options, .. } = spec {
        return options
            .iter()
            .position(|o| o.to_lowercase() == s)
            .map(|i| i as f32);
    }
    let Spec::Knob { unit, min, max, .. } = *spec else { unreachable!() };
    let num_end = s
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e'))
        .unwrap_or(s.len());
    let mut v: f32 = s[..num_end].parse().ok()?;
    let suffix = s[num_end..].trim();
    v *= match (unit, suffix) {
        (_, "") => 1.0,
        ("Hz", "k" | "khz") => 1000.0,
        ("Hz", "hz") => 1.0,
        ("ms", "s") => 1000.0,
        ("ms", "ms") => 1.0,
        (u, x) if x == u.to_lowercase() => 1.0,
        _ => return None,
    };
    if !v.is_finite() {
        return None;
    }
    Some(to_t(spec, v.clamp(min.min(max), max.max(min))))
}

/// Parses a route amount: `+25 %`, `-40`, `0.3` (values with |v| <= 1 and no `%` are fractions).
pub fn parse_amount(text: &str) -> Option<f32> {
    let s = text.trim();
    let (num, pct) = match s.strip_suffix('%') {
        Some(n) => (n.trim(), true),
        None => (s, false),
    };
    let v: f32 = num.parse().ok()?;
    if !v.is_finite() {
        return None;
    }
    let v = if pct || v.abs() > 1.0 { v / 100.0 } else { v };
    Some(v.clamp(-1.0, 1.0))
}

// ------------------------------------------------------------------------------ patch

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct JackRef {
    pub m: usize,
    pub j: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct CtlRef {
    pub m: usize,
    pub c: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    pub kind: Kind,
    pub row: usize,
    /// Knob: travel 0..1. Selector: option index.
    pub values: Vec<f32>,
    pub primary: Vec<bool>,
}

impl Module {
    pub fn new(kind: Kind, row: usize) -> Self {
        let d = def(kind);
        Module {
            kind,
            row,
            values: d
                .controls
                .iter()
                .map(|c| match c.spec {
                    Spec::Knob { default, .. } => to_t(&c.spec, default),
                    Spec::Select { default, .. } => default as f32,
                })
                .collect(),
            primary: d.controls.iter().map(|c| c.primary).collect(),
        }
    }
    pub fn def(&self) -> &'static KindDef {
        def(self.kind)
    }
    pub fn hidden_count(&self) -> usize {
        self.primary.iter().filter(|p| !**p).count()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cable {
    pub from: JackRef,
    pub to: JackRef,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Route {
    pub src: JackRef,
    pub dst: CtlRef,
    /// Fraction of knob travel, −1..+1. Sign = polarity.
    pub amount: f32,
    pub bypass: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Patch {
    pub modules: Vec<Module>,
    pub cables: Vec<Cable>,
    pub routes: Vec<Route>,
}

pub const DEFAULT_DROP_AMOUNT: f32 = 0.25;

impl Patch {
    pub fn find(&self, kind: Kind, nth: usize) -> usize {
        self.modules
            .iter()
            .enumerate()
            .filter(|(_, m)| m.kind == kind)
            .nth(nth)
            .map(|(i, _)| i)
            .expect("module kind present")
    }

    pub fn jack(&self, m: usize, id: &str) -> JackRef {
        let j = self.modules[m].def().jacks.iter().position(|j| j.id == id).expect("jack");
        JackRef { m, j }
    }

    pub fn ctl(&self, m: usize, id: &str) -> CtlRef {
        let c = self.modules[m].def().controls.iter().position(|c| c.id == id).expect("control");
        CtlRef { m, c }
    }

    pub fn jack_def(&self, r: JackRef) -> &'static JackDef {
        &self.modules[r.m].def().jacks[r.j]
    }

    pub fn ctl_def(&self, r: CtlRef) -> &'static CtlDef {
        &self.modules[r.m].def().controls[r.c]
    }

    pub fn routes_to(&self, dst: CtlRef) -> impl Iterator<Item = (usize, &Route)> {
        self.routes.iter().enumerate().filter(move |(_, r)| r.dst == dst)
    }

    pub fn source_bipolar(&self, src: JackRef) -> bool {
        let m = &self.modules[src.m];
        if m.kind == Kind::Lfo {
            // Polarity ◆ (BI/UNI) decides it.
            let c = m.def().controls.iter().position(|c| c.id == "polarity").unwrap();
            return m.values[c] < 0.5;
        }
        m.def().bipolar_out
    }

    /// Extent in travel space that every active route adds to the base: (lo, hi) before clamp.
    pub fn mod_span(&self, dst: CtlRef, include_bypassed: bool) -> (f32, f32) {
        let (mut lo, mut hi) = (0.0, 0.0);
        for (_, r) in self.routes_to(dst) {
            if r.bypass && !include_bypassed {
                continue;
            }
            let (a, b) = self.route_span(r);
            lo += a;
            hi += b;
        }
        (lo, hi)
    }

    pub fn route_span(&self, r: &Route) -> (f32, f32) {
        if self.source_bipolar(r.src) {
            (-r.amount.abs(), r.amount.abs())
        } else {
            (r.amount.min(0.0), r.amount.max(0.0))
        }
    }

    pub fn has_input_cable(&self, to: JackRef) -> Option<usize> {
        self.cables.iter().position(|c| c.to == to)
    }

    pub fn module_label(&self, m: usize) -> String {
        let kind = self.modules[m].kind;
        let n = self.modules[..m].iter().filter(|x| x.kind == kind).count();
        let total = self.modules.iter().filter(|x| x.kind == kind).count();
        let short = match kind {
            Kind::Midi => "MIDI",
            Kind::Osc => "Osc",
            Kind::Lfo => "LFO",
            Kind::Filter => "Filter",
            Kind::Adsr => "ADSR",
            Kind::Vca => "VCA",
            Kind::Out => "Out",
            Kind::Ensemble => "Ensemble",
        };
        if total > 1 {
            format!("{short} {}", n + 1)
        } else {
            short.to_string()
        }
    }

    pub fn route_label(&self, r: &Route) -> String {
        format!(
            "{} {} → {} {}",
            self.module_label(r.src.m),
            self.jack_def(r.src).label.to_lowercase(),
            self.module_label(r.dst.m),
            self.ctl_def(r.dst).label
        )
    }

    pub fn cable_label(&self, c: &Cable) -> String {
        format!(
            "{} {} → {} {}",
            self.module_label(c.from.m),
            self.jack_def(c.from).label.to_lowercase(),
            self.module_label(c.to.m),
            self.jack_def(c.to).label.to_lowercase()
        )
    }
}

pub fn reference_patch() -> Patch {
    let mods = vec![
        Module::new(Kind::Osc, 0),
        Module::new(Kind::Lfo, 0),
        Module::new(Kind::Filter, 0),
        Module::new(Kind::Ensemble, 0),
        Module::new(Kind::Midi, 1),
        Module::new(Kind::Adsr, 1),
        Module::new(Kind::Vca, 1),
        Module::new(Kind::Out, 1),
    ];
    let mut p = Patch { modules: mods, cables: vec![], routes: vec![] };
    let (osc, lfo, fil, midi, env, vca, out) = (
        p.find(Kind::Osc, 0),
        p.find(Kind::Lfo, 0),
        p.find(Kind::Filter, 0),
        p.find(Kind::Midi, 0),
        p.find(Kind::Adsr, 0),
        p.find(Kind::Vca, 0),
        p.find(Kind::Out, 0),
    );
    let c = |p: &Patch, a: (usize, &str), b: (usize, &str)| Cable {
        from: p.jack(a.0, a.1),
        to: p.jack(b.0, b.1),
    };
    p.cables = vec![
        c(&p, (midi, "gate"), (env, "gate")),
        c(&p, (midi, "pitch"), (osc, "pitch")),
        c(&p, (osc, "out"), (fil, "in")),
        c(&p, (fil, "lp"), (vca, "in")),
        c(&p, (env, "out"), (vca, "cv")),
        c(&p, (vca, "out"), (out, "left")),
        c(&p, (vca, "out"), (out, "right")),
    ];
    // LFO → Filter Cutoff is part of the reference; LFO → Attack is the walkthrough task.
    p.routes = vec![Route {
        src: p.jack(lfo, "out"),
        dst: p.ctl(fil, "cutoff_hz"),
        amount: 0.2,
        bypass: false,
    }];
    p
}

/// Reference patch plus a second voice path and more modulation: 14 modules, many routes.
pub fn crowded_patch() -> Patch {
    let mut p = reference_patch();
    for (kind, row) in [
        (Kind::Lfo, 0),
        (Kind::Osc, 0),
        (Kind::Filter, 1),
        (Kind::Adsr, 1),
        (Kind::Vca, 1),
        (Kind::Lfo, 1),
    ] {
        p.modules.push(Module::new(kind, row));
    }
    let (lfo1, lfo2, lfo3) = (p.find(Kind::Lfo, 0), p.find(Kind::Lfo, 1), p.find(Kind::Lfo, 2));
    let (osc1, osc2) = (p.find(Kind::Osc, 0), p.find(Kind::Osc, 1));
    let (f1, f2) = (p.find(Kind::Filter, 0), p.find(Kind::Filter, 1));
    let (e1, e2) = (p.find(Kind::Adsr, 0), p.find(Kind::Adsr, 1));
    let (v2, midi, ens) = (p.find(Kind::Vca, 1), p.find(Kind::Midi, 0), p.find(Kind::Ensemble, 0));
    let out = p.find(Kind::Out, 0);
    let add = |p: &mut Patch, a: (usize, &str), b: (usize, &str)| {
        let cable = Cable { from: p.jack(a.0, a.1), to: p.jack(b.0, b.1) };
        p.cables.push(cable);
    };
    add(&mut p, (midi, "pitch"), (osc2, "pitch"));
    add(&mut p, (osc2, "out"), (f2, "in"));
    add(&mut p, (f2, "bp"), (v2, "in"));
    add(&mut p, (e2, "out"), (v2, "cv"));
    add(&mut p, (midi, "gate"), (e2, "gate"));
    add(&mut p, (v2, "out"), (ens, "in"));
    let _ = out;
    let route = |p: &mut Patch, s: (usize, &str), d: (usize, &str), amount: f32| {
        let r = Route { src: p.jack(s.0, s.1), dst: p.ctl(d.0, d.1), amount, bypass: false };
        p.routes.push(r);
    };
    route(&mut p, (lfo1, "out"), (e1, "attack_ms"), 0.25);
    route(&mut p, (lfo2, "out"), (f1, "cutoff_hz"), -0.15);
    route(&mut p, (lfo2, "out"), (f2, "cutoff_hz"), 0.3);
    route(&mut p, (e2, "out"), (f2, "resonance"), 0.2);
    route(&mut p, (lfo3, "out"), (lfo2, "rate_hz"), 0.12);
    route(&mut p, (lfo3, "out"), (osc1, "waveform"), 0.34);
    route(&mut p, (midi, "velocity"), (f1, "cutoff_hz"), 0.2);
    route(&mut p, (e1, "out"), (lfo1, "amp"), 0.5);
    route(&mut p, (lfo1, "out"), (ens, "depth"), 0.2);
    route(&mut p, (e2, "out"), (osc2, "base_hz"), 0.05);
    p
}

// ------------------------------------------------------------------------------ undo

/// Snapshot undo: the prototype patch is small, so each edit stores the whole patch.
#[derive(Default)]
pub struct History {
    undo: Vec<(Patch, String)>,
    redo: Vec<(Patch, String)>,
}

impl History {
    /// Records `before` as the state an undo of `label` returns to.
    pub fn push(&mut self, before: Patch, label: String) {
        self.undo.push((before, label));
        self.redo.clear();
    }
    pub fn undo(&mut self, cur: &mut Patch) -> Option<String> {
        let (p, label) = self.undo.pop()?;
        self.redo.push((std::mem::replace(cur, p), label.clone()));
        Some(label)
    }
    pub fn redo(&mut self, cur: &mut Patch) -> Option<String> {
        let (p, label) = self.redo.pop()?;
        self.undo.push((std::mem::replace(cur, p), label.clone()));
        Some(label)
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exp_roundtrip_and_parse() {
        let spec = def(Kind::Adsr).controls[0].spec;
        let t = to_t(&spec, 8.0);
        assert!((to_value(&spec, t) - 8.0).abs() < 1e-3);
        assert!((to_value(&spec, parse_value(&spec, "2 s").unwrap()) - 2000.0).abs() < 0.5);
        let cut = def(Kind::Filter).controls[0].spec;
        assert!((to_value(&cut, parse_value(&cut, "1.2k").unwrap()) - 1200.0).abs() < 0.5);
        assert!(parse_value(&cut, "12 parsecs").is_none());
        assert_eq!(parse_amount("+25 %"), Some(0.25));
        assert_eq!(parse_amount("-40"), Some(-0.4));
    }

    #[test]
    fn storyboard_numbers() {
        // revision-2 storyboard: Attack 8 ms, +25 % bipolar swings 0.45 – 142 ms
        let spec = def(Kind::Adsr).controls[0].spec;
        let t = to_t(&spec, 8.0);
        let (lo, hi) = (to_value(&spec, t - 0.25), to_value(&spec, t + 0.25));
        assert!((lo - 0.45).abs() < 0.01, "{lo}");
        assert!((hi - 142.0).abs() < 1.0, "{hi}");
    }

    #[test]
    fn undo_redo_roundtrip() {
        let mut p = reference_patch();
        let mut h = History::default();
        let before = p.clone();
        p.modules[0].values[0] = 0.9;
        h.push(before.clone(), "Set".into());
        assert_eq!(h.undo(&mut p).as_deref(), Some("Set"));
        assert_eq!(p, before);
        h.redo(&mut p);
        assert_eq!(p.modules[0].values[0], 0.9);
    }
}
