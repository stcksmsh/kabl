//! The factory keys and plucks (docs/presets/BRIEF.md, second part of the bank): builds the
//! patches from the existing modules, writes them under `patches/presets/`, renders a clip per
//! preset by playing a short MIDI phrase through the real engine, and prints the level and the
//! library entries.
//!
//! ```sh
//! cargo run -p kabl-ui --example presets_keys_pluck -- write patches
//! cargo run --release -p kabl-ui --example presets_keys_pluck -- clips patches out_dir [group]
//! cargo run --release -p kabl-ui --example presets_keys_pluck -- levels patches [slug]
//! ```
//!
//! Same recipe as `presets_pilot.rs`: every preset names its macros after what they do (m1 the
//! tone, m2 the harmonic intensity, m3 the space, m4 what defines the sound, on CC 20-23);
//! echo and room mixes are 0 on the module and driven by m3, so the stored value is the baked-in
//! wet amount and m3 at 0 is dry.

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

const SR: f32 = 48000.0;

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
    /// An LFO whose depth is macro `m`: the LFO runs through a VCA the macro opens. Returns the
    /// VCA; route its `out` into whatever should move.
    fn motion(&mut self, knob: ModuleId, m: &str, rate_hz: f32) -> ModuleId {
        let lfo = self.add("lfo", &[("rate_hz", rate_hz)]);
        let depth = self.add("vca", &[("gain", 0.0)]);
        self.wire(lfo, "out", depth, "in");
        self.route(knob, m, depth, "gain", 1.0);
        depth
    }
    /// The last stage: a limiter just under the level target's ceiling, then the output.
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

/// A preset's macros: the names it gives them and their stored values, on CC 20 and up.
fn macros(r: &mut Rack, names: &[&str], values: &[f32]) -> ModuleId {
    let knob = r.add("macro", &[]);
    for (i, name) in names.iter().enumerate() {
        let key = format!("m{}", i + 1);
        r.e.set_label(knob, &format!("name.{key}"), Some((*name).into()));
        r.e.set_param(knob, &key, values[i]);
    }
    knob
}

/// Pins (Perform panel order) and CC numbers: the transport first when the patch has a clock,
/// then the macros.
fn perform(r: &mut Rack, knob: ModuleId, clock: Option<ModuleId>, n: usize) {
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

/// One FM operator row: ratio, fine (cents), level, attack, decay, sustain, release (ms),
/// velocity sensitivity.
type Op = [f32; 8];

/// An `osc.fm6` wired to a `midi.in`: algorithm, index, feedback and the six operator rows.
fn fm(r: &mut Rack, m: ModuleId, algorithm: f32, index: f32, feedback: f32, ops: &[Op; 6]) -> ModuleId {
    let voice = r.add(
        "osc.fm6",
        &[("algorithm", algorithm), ("index", index), ("feedback", feedback)],
    );
    let names = [
        "ratio", "fine", "level", "attack", "decay", "sustain", "release", "vel",
    ];
    for (k, op) in ops.iter().enumerate() {
        for (name, &v) in names.iter().zip(op) {
            r.e.set_param(voice, &format!("{name}{}", k + 1), v);
        }
    }
    r.wire(m, "pitch", voice, "pitch");
    r.wire(m, "gate", voice, "gate");
    r.wire(m, "velocity", voice, "velocity");
    voice
}

/// Everything after the voice, the same for every preset: drive, ladder filter, trim, then the
/// stereo stages (phaser, chorus, compressor), the echo and room, the limiter. m1 opens the
/// filter, m2 pushes the drive, m3 is the space; the caller routes m4.
struct Post<'a> {
    /// Drive stage: (drive dB stored, trim dB, m2 amount).
    drive: Option<(f32, f32, f32)>,
    cutoff: f32,
    resonance: f32,
    /// How far m1 sweeps the cutoff.
    cut_m1: f32,
    trim_db: f32,
    /// Phaser params, and an optional m4 route into one of its knobs (param, amount).
    phaser: Option<(&'a [(&'static str, f32)], Option<(&'static str, f32)>)>,
    /// Chorus params, and the share of m4 that opens its mix (None: the mix is as stored).
    chorus: Option<(&'a [(&'static str, f32)], Option<f32>)>,
    comp: Option<&'a [(&'static str, f32)]>,
    /// A module whose `out` feeds the compressor's sidechain, and an m4 route into one of its knobs.
    duck: Option<(ModuleId, Option<(&'static str, f32)>)>,
    echo: &'a [(&'static str, f32)],
    echo_max: f32,
    clock: Option<ModuleId>,
    room_decay: f32,
    room_max: f32,
}

struct Stages {
    drive: Option<ModuleId>,
    lad: ModuleId,
    chorus: Option<ModuleId>,
}

fn post(r: &mut Rack, knob: ModuleId, src: ModuleId, p: &Post) -> Stages {
    r.next_row();
    let drive = p.drive.map(|(db, trim, _)| {
        r.add("drive", &[("drive_db", db), ("trim_db", trim)])
    });
    let lad = r.add(
        "filter.ladder",
        &[("cutoff_hz", p.cutoff), ("resonance", p.resonance)],
    );
    let trim = r.add("gain", &[("gain_db", p.trim_db)]);
    match drive {
        Some(d) => {
            r.wire(src, "out", d, "in");
            r.wire(d, "out", lad, "in");
        }
        None => r.wire(src, "out", lad, "in"),
    }
    r.wire(lad, "out", trim, "in");
    if let (Some(d), Some((_, _, amt))) = (drive, p.drive) {
        r.route(knob, "m2", d, "drive_db", amt);
    }
    r.route(knob, "m1", lad, "cutoff_hz", p.cut_m1);
    // Stereo chain: each stage takes the previous one's left and right.
    let mut last = trim;
    let mut first = true;
    let mut chorus_id = None;
    let stage = |r: &mut Rack, last: &mut ModuleId, first: &mut bool, id: ModuleId| {
        if *first {
            r.wire_both(*last, "out", id);
            *first = false;
        } else {
            r.stereo(*last, id);
        }
        *last = id;
    };
    if let Some((ph, m4)) = p.phaser {
        let id = r.add("phaser", ph);
        stage(r, &mut last, &mut first, id);
        if let Some((param, amt)) = m4 {
            r.route(knob, "m4", id, param, amt);
        }
    }
    if let Some((ch, m4)) = p.chorus {
        let id = r.add("chorus", ch);
        stage(r, &mut last, &mut first, id);
        if let Some(amt) = m4 {
            r.route(knob, "m4", id, "mix", amt);
        }
        chorus_id = Some(id);
    }
    if let Some(cp) = p.comp {
        let id = r.add("compressor", cp);
        stage(r, &mut last, &mut first, id);
        if let Some((src, m4)) = p.duck {
            r.wire(src, "out", id, "sc");
            if let Some((param, amt)) = m4 {
                r.route(knob, "m4", id, param, amt);
            }
        }
    }
    if first {
        // No stereo stage: a plain width-less chorus-free path still needs left and right.
        let id = r.add("chorus", &[("mix", 0.0)]);
        stage(r, &mut last, &mut first, id);
    }
    let tail = space_tail(r, knob, last, p.echo, p.clock, p.echo_max, p.room_decay, p.room_max);
    r.out_stereo(tail);
    Stages {
        drive,
        lad,
        chorus: chorus_id,
    }
}

/// A short echo (a slapback, or tempo-synced when `clock` is given) and a room, both at mix 0
/// until m3 opens them. `src` is a stereo module; returns the room.
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
            ("damp_hz", 5000.0),
            ("mix", 0.0),
            ("predelay_ms", 0.0),
            ("width", 70.0),
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

// ---- clips ------------------------------------------------------------------------------------

/// One thing that happens in a clip: a key goes down (velocity) or up.
struct Hit(f32, u8, Option<u8>);

fn on(t: f32, n: u8, v: u8) -> Hit {
    Hit(t, n, Some(v))
}
fn off(t: f32, n: u8) -> Hit {
    Hit(t, n, None)
}
/// A chord struck at `t` and released `len` later.
fn chord(t: f32, len: f32, vel: u8, notes: &[u8]) -> Vec<Hit> {
    notes
        .iter()
        .flat_map(|&n| [on(t, n, vel), off(t + len, n)])
        .collect()
}
/// A line: (time, length, note, velocity).
fn line(notes: &[(f32, f32, u8, u8)]) -> Vec<Hit> {
    notes
        .iter()
        .flat_map(|&(t, len, n, v)| [on(t, n, v), off(t + len, n)])
        .collect()
}
/// Chords at beat positions of a tempo: (beat, beats long, notes), every chord at `vel`.
fn comp(bpm: f32, vel: u8, chords: &[(f32, f32, &[u8])]) -> Vec<Hit> {
    let beat = 60.0 / bpm;
    chords
        .iter()
        .flat_map(|&(b, len, notes)| chord(b * beat, len * beat * 0.92, vel, notes))
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
    /// 1 = first delivery group, 2 = second.
    group: u8,
    build: fn() -> Rack,
    phrase: fn() -> Vec<Hit>,
    /// Seconds of the clip.
    secs: f32,
}

#[path = "presets_keys_pluck/bank.rs"]
mod bank;
use bank::presets;

fn write_wav(path: &Path, left: &[f32], right: &[f32]) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR as u32,
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

/// Plays `hits` through the engine for `secs` seconds. A clocked patch runs from 0.5 s.
fn render(
    state: &kabl_core::PatchState,
    hits: &[Hit],
    secs: f32,
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
    let mut engine = PatchEngine::new(&collector.handle(), state, SR, 8).expect("compiles");
    let clocks: Vec<ModuleId> = state
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "clock")
        .map(|(&id, _)| id)
        .collect();
    let (mut left, mut right) = (Vec::new(), Vec::new());
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    for b in 0..(secs * SR) as usize / BLOCK {
        let t0 = (b * BLOCK) as f32 / SR;
        let t1 = t0 + BLOCK as f32 / SR;
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

/// The level test's measurement (`factory_bank::level`): a held chord (E3 B3 E4 G4, velocity
/// 90) struck at 1, 5, 13 ... s, released 3 s later, over 24 s. Peak and loudness in dBFS.
fn level(state: &kabl_core::PatchState) -> (f64, f64) {
    let mut hits = Vec::new();
    for t in [1.0, 5.0, 13.0, 21.0] {
        hits.extend(chord(t, 3.0, 90, &[52, 59, 64, 67]));
    }
    let (l, r) = render(state, &hits, 24.0);
    let mut inter = Vec::with_capacity(l.len() * 2);
    for (a, b) in l.iter().zip(&r) {
        inter.push(*a);
        inter.push(*b);
    }
    let peak = inter.iter().fold(0.0f32, |a, b| a.max(b.abs())) as f64;
    let (mut sum, mut n) = (0.0f64, 0usize);
    for block in inter.chunks(9600) {
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
            // clips <patches> <out> [group]: float WAVs; the encoder script does the rest.
            let out = Path::new(&args[3]);
            let only: Option<u8> = args.get(4).and_then(|g| g.parse().ok());
            std::fs::create_dir_all(out).unwrap();
            for p in presets().into_iter().filter(|p| only.is_none_or(|g| p.group == g)) {
                let log = kabl_core::load(&Path::new(&args[2]).join("presets").join(p.slug))
                    .expect("load patch");
                let (l, r) = render(log.state(), &(p.phrase)(), p.secs);
                let (l2, _) = render(log.state(), &(p.phrase)(), p.secs);
                assert_eq!(l, l2, "{}: render is deterministic", p.slug);
                write_wav(&out.join(format!("{}.wav", p.slug)), &l, &r);
                let peak = l.iter().chain(&r).fold(0.0f32, |m, s| m.max(s.abs()));
                println!("{:18} peak {peak:.3}  | {}", p.slug, p.listen);
            }
        }
        Some("levels") => {
            let only = args.get(3);
            for p in presets().into_iter().filter(|p| only.is_none_or(|s| s == p.slug)) {
                let log = kabl_core::load(&Path::new(&args[2]).join("presets").join(p.slug))
                    .expect("load patch");
                let (peak, loud) = level(log.state());
                println!("LEVEL {:18} peak {peak:6.2}  loud {loud:6.2}", p.slug);
            }
        }
        Some("macros") => {
            // Peak and loudness with every macro at 0, at 1, and with only m3 (space) at 0.
            let db = |v: f32| 20.0 * v.max(1e-9).log10();
            for p in presets() {
                let mut line = format!("{:16}", p.slug);
                for (label, set) in [("all 0", Some(0.0)), ("all 1", Some(1.0)), ("no Space", None)] {
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
                        let key = format!("m{k}");
                        if rack.e.state().labels[&knob].contains_key(&format!("name.{key}")) {
                            match set {
                                Some(v) => rack.e.set_param(knob, &key, v),
                                None if k == 3 => rack.e.set_param(knob, &key, 0.0),
                                None => {}
                            }
                        }
                    }
                    let (l, r) = render(rack.e.state(), &(p.phrase)(), p.secs);
                    let peak = l.iter().chain(&r).fold(0.0f32, |m, s| m.max(s.abs()));
                    let ms = l.iter().chain(&r).map(|s| s * s).sum::<f32>() / (2 * l.len()) as f32;
                    assert!(peak.is_finite(), "{}: finite", p.slug);
                    line += &format!("  {label}: peak {:6.1} rms {:6.1} |", db(peak), db(ms.sqrt()));
                }
                println!("{line}");
            }
        }
        Some("moves") => {
            // Each named macro alone at 0 and at 1 (the others as stored): the renders must differ.
            // Prints the difference as dB below the louder render's RMS; under -40 dB is flagged.
            let only = args.get(2);
            for p in presets().into_iter().filter(|p| only.is_none_or(|s| s == p.slug)) {
                let rack = (p.build)();
                let knob = *rack.e.state().modules.iter().find(|(_, m)| m.kind == "macro").unwrap().0;
                let mut line = format!("MOVES {:16}", p.slug);
                for k in 1..=8 {
                    let key = format!("m{k}");
                    let Some(name) = rack.e.state().labels[&knob].get(&format!("name.{key}")).cloned() else {
                        continue;
                    };
                    let at = |v: f32| {
                        let mut r = (p.build)();
                        r.e.set_param(knob, &key, v);
                        render(r.e.state(), &(p.phrase)(), p.secs).0
                    };
                    let (a, b) = (at(0.0), at(1.0));
                    let rms = |v: &[f32]| (v.iter().map(|s| s * s).sum::<f32>() / v.len().max(1) as f32).sqrt();
                    let d: Vec<f32> = a.iter().zip(&b).map(|(x, y)| x - y).collect();
                    let db = 20.0 * (rms(&d) / rms(&a).max(rms(&b)).max(1e-9)).max(1e-9).log10();
                    let flag = if db < -40.0 { " FLAT" } else { "" };
                    line += &format!(" | {key} {name} {db:.1}{flag}");
                }
                println!("{line}");
            }
        }
        Some("library") => {
            // The FACTORY entries for crates/ui/tests/library.rs, one block, to stdout.
            for p in presets() {
                let tags: Vec<String> = p.tags.iter().map(|t| format!("{t:?}")).collect();
                println!(
                    "    (\n        \"presets/{}\",\n        {:?},\n        {:?},\n        &[{}],\n        {:?},\n    ),",
                    p.slug,
                    p.name,
                    p.category,
                    tags.join(", "),
                    p.description
                );
            }
        }
        Some("phrases") => {
            // For docs/presets/scripts/host.py: {slug: {secs, hits: [[t, note, velocity|0]]}}.
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
        Some("names") => {
            for p in presets() {
                println!("{}\t{}", p.slug, p.name);
            }
        }
        Some("info") => {
            let info = kabl_modules::registry::info_for(&args[2]).expect("kind");
            for p in info.params {
                println!("{:14} {:>9} .. {:<9} default {}", p.name, p.min, p.max, p.default);
            }
        }
        _ => eprintln!("usage: presets_keys_pluck write|clips|levels|macros|library|phrases|info"),
    }
}
