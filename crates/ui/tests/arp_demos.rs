//! The four rhythm demos: a held chord arpeggiated across octaves, one sequence straight and then
//! swung, a sequence with ratchets and probability, and the arpeggiator combined with a patterned
//! cable. Each is built through the real `PatchEditor`, so the committed patch is what the editor
//! makes.
//!
//! Rewrite the patches with `cargo test -p kabl-ui --test arp_demos write_patches -- --ignored`
//! and the audio (`target/arp-demos/*.wav`) with `write_renders`.

use kabl_core::{ModuleId, PatchState, PortRef, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::PatchEngine;
use kabl_ui::PatchEditor;

const SR: f32 = 48000.0;

fn port(id: ModuleId, port: &str) -> PortRef {
    PortRef::Module {
        id,
        port: port.into(),
    }
}

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
        self.e.connect(port(a, ap), port(b, bp));
    }
    /// A cable with a step pattern: `params` are the cable's own (`length`, `s2`, `prob`, ...).
    fn pattern(&mut self, a: ModuleId, ap: &str, b: ModuleId, bp: &str, params: &[(&str, f32)]) {
        let cable = self.e.connect(port(a, ap), port(b, bp));
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
    fn route(&mut self, a: ModuleId, ap: &str, b: ModuleId, param: &str, amount: f32) {
        let cable = self.e.connect_route(port(a, ap), b, param);
        self.e.set_route_amount(cable, amount, true);
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

/// A held chord, arpeggiated up and down over three octaves. Two chords, eight seconds each.
fn arp_chord() -> PatchEditor {
    let mut r = Rack::new();
    let clock = r.add("clock", &[("bpm", 112.0)]);
    let arp = r.add(
        "arp",
        &[("mode", 2.0), ("octaves", 3.0), ("gate_len", 55.0)],
    );
    let osc = r.add(
        "osc.va",
        &[("waveform", 2.0), ("unison", 2.0), ("detune", 9.0)],
    );
    r.next_row();
    let filter = r.add(
        "filter.ladder",
        &[("cutoff_hz", 1500.0), ("resonance", 0.3), ("drive_db", 0.0)],
    );
    let env = r.add("env.adsr", &adsr(2.0, 220.0, 0.15, 160.0));
    let vca = r.add("vca", &[("gain", 0.0)]);
    let trim = r.add("gain", &[("gain_db", -8.0)]);
    let delay = r.add(
        "delay",
        &[
            ("sync", 3.0),
            ("feedback", 40.0),
            ("mix", 30.0),
            ("mode", 1.0),
        ],
    );
    r.next_row();
    let out = r.add("out", &[]);
    r.wire(clock, "gate", arp, "clock");
    r.wire(clock, "reset", arp, "reset");
    r.wire(arp, "pitch", osc, "pitch");
    r.wire(arp, "gate", env, "gate");
    r.wire(osc, "out", filter, "in");
    r.wire(filter, "out", vca, "in");
    r.wire(env, "out", vca, "cv");
    r.wire(clock, "gate", delay, "clock");
    r.wire(vca, "out", trim, "in");
    r.wire(trim, "out", delay, "in");
    r.wire(delay, "left", out, "left");
    r.wire(delay, "right", out, "right");
    r.route(env, "out", filter, "cutoff_hz", 0.3);
    r.e
}

fn chord_phrase() -> Events {
    notes(&[
        (50, 0.3, 8.0),
        (53, 0.3, 8.0),
        (57, 0.3, 8.0),
        (60, 0.3, 8.0),
        (46, 8.3, 16.0),
        (50, 8.3, 16.0),
        (53, 8.3, 16.0),
        (57, 8.3, 16.0),
    ])
}

/// The swing percentage the saved patch carries; the straight half of the clip renders with 0.
const SWING: f32 = 58.0;

/// A bass line and a hat on one clock. The hat's envelope takes the clock's own pulse, so both
/// swing together by the clock's `swing` alone.
fn swing_seq() -> PatchEditor {
    let mut r = Rack::new();
    let clock = r.add("clock", &[("bpm", 116.0), ("swing", SWING)]);
    let seq = r.add("seq", &[]);
    for (k, p) in [0.0, 0.0, 7.0, 0.0, 10.0, 0.0, 12.0, 7.0]
        .into_iter()
        .enumerate()
    {
        r.e.set_param(seq, &format!("p{}", k + 1), p - 24.0);
    }
    let bass = r.add("osc.va", &[("waveform", 2.0)]);
    r.next_row();
    let ladder = r.add(
        "filter.ladder",
        &[("cutoff_hz", 380.0), ("resonance", 0.45), ("drive_db", 0.0)],
    );
    let env = r.add("env.adsr", &adsr(2.0, 170.0, 0.0, 60.0));
    let vca = r.add("vca", &[("gain", 0.0)]);
    let hat = r.add("noise", &[("color", 0.0)]);
    r.next_row();
    let hp = r.add("filter.svf", &[("cutoff_hz", 7000.0), ("resonance", 0.2)]);
    let henv = r.add("env.adsr", &adsr(0.5, 38.0, 0.0, 20.0));
    let hvca = r.add("vca", &[("gain", 0.0)]);
    let mixer = r.add("mixer", &[("level1", 0.7), ("level2", 0.22)]);
    r.next_row();
    let out = r.add("out", &[]);
    r.wire(clock, "gate", seq, "clock");
    r.wire(clock, "reset", seq, "reset");
    r.wire(seq, "pitch", bass, "pitch");
    r.wire(seq, "gate", env, "gate");
    r.wire(bass, "out", ladder, "in");
    r.wire(ladder, "out", vca, "in");
    r.wire(env, "out", vca, "cv");
    r.wire(vca, "out", mixer, "in1");
    r.wire(hat, "out", hp, "in");
    r.wire(hp, "hp", hvca, "in");
    r.wire(clock, "gate", henv, "gate");
    r.wire(henv, "out", hvca, "cv");
    r.wire(hvca, "out", mixer, "in2");
    r.wire(mixer, "out", out, "left");
    r.wire(mixer, "out", out, "right");
    r.route(env, "out", ladder, "cutoff_hz", 0.35);
    r.e
}

/// A lead whose steps 3, 6 and 8 are repeated two, three and four times, with some steps left to
/// chance. The ratchet of a step either plays whole or not at all.
fn ratchet_roll() -> PatchEditor {
    let mut r = Rack::new();
    let clock = r.add("clock", &[("bpm", 104.0), ("swing", 22.0)]);
    let seq = r.add(
        "seq",
        &[
            ("k3", 2.0),
            ("k6", 3.0),
            ("k8", 4.0),
            ("r2", 70.0),
            ("r5", 60.0),
            ("r7", 50.0),
        ],
    );
    for (k, p) in [0.0, 3.0, 7.0, 10.0, 12.0, 10.0, 7.0, 3.0]
        .into_iter()
        .enumerate()
    {
        r.e.set_param(seq, &format!("p{}", k + 1), p - 12.0);
    }
    let osc = r.add("osc.va", &[("waveform", 3.0), ("pw", 35.0)]);
    r.next_row();
    let filter = r.add("filter.svf", &[("cutoff_hz", 1800.0), ("resonance", 0.35)]);
    let env = r.add("env.adsr", &adsr(1.0, 110.0, 0.0, 70.0));
    let vca = r.add("vca", &[("gain", 0.0)]);
    let trim = r.add("gain", &[("gain_db", -13.0)]);
    let delay = r.add(
        "delay",
        &[
            ("sync", 2.0),
            ("feedback", 35.0),
            ("mix", 28.0),
            ("mode", 1.0),
        ],
    );
    r.next_row();
    let out = r.add("out", &[]);
    r.wire(clock, "gate", seq, "clock");
    r.wire(clock, "reset", seq, "reset");
    r.wire(seq, "pitch", osc, "pitch");
    r.wire(seq, "gate", env, "gate");
    r.wire(osc, "out", filter, "in");
    r.wire(filter, "lp", vca, "in");
    r.wire(env, "out", vca, "cv");
    r.wire(clock, "gate", delay, "clock");
    r.wire(vca, "out", trim, "in");
    r.wire(trim, "out", delay, "in");
    r.wire(delay, "left", out, "left");
    r.wire(delay, "right", out, "right");
    r.route(env, "out", filter, "cutoff_hz", 0.3);
    r.e
}

/// A random arpeggio of a held chord whose output is gated by a six-step cable against the
/// 16ths (a second pattern running against the first) and thrown to a delay by a four-step cable
/// that is left to chance.
fn arp_cable() -> PatchEditor {
    let mut r = Rack::new();
    let clock = r.add("clock", &[("bpm", 100.0), ("swing", 30.0)]);
    let arp = r.add(
        "arp",
        &[("mode", 4.0), ("octaves", 2.0), ("gate_len", 60.0)],
    );
    let osc = r.add("osc.va", &[("waveform", 3.0), ("pw", 40.0)]);
    r.next_row();
    let filter = r.add(
        "filter.ladder",
        &[("cutoff_hz", 1400.0), ("resonance", 0.3), ("drive_db", 0.0)],
    );
    let env = r.add("env.adsr", &adsr(1.0, 140.0, 0.1, 100.0));
    let vca = r.add("vca", &[("gain", 0.0)]);
    let delay = r.add(
        "delay",
        &[
            ("sync", 3.0),
            ("feedback", 55.0),
            ("mix", 100.0),
            ("mode", 1.0),
        ],
    );
    r.next_row();
    let mixer = r.add(
        "mixer",
        &[("level1", 0.6), ("level2", 0.3), ("level3", 0.3)],
    );
    let out = r.add("out", &[]);
    r.wire(clock, "gate", arp, "clock");
    r.wire(clock, "reset", arp, "reset");
    r.wire(arp, "pitch", osc, "pitch");
    r.wire(arp, "gate", env, "gate");
    r.wire(osc, "out", filter, "in");
    r.wire(filter, "out", vca, "in");
    r.wire(env, "out", vca, "cv");
    // The dry line passes a six-step cable: steps 2 and 5 are shut, step 4 is half level.
    r.pattern(
        vca,
        "out",
        mixer,
        "in1",
        &[("length", 6.0), ("s2", 0.0), ("s4", 0.5), ("s5", 0.0)],
    );
    // The throw to the delay: four steps, only the third open, happening 70 % of the time.
    r.pattern(
        vca,
        "out",
        delay,
        "in",
        &[
            ("length", 4.0),
            ("s1", 0.0),
            ("s2", 0.0),
            ("s4", 0.0),
            ("prob", 70.0),
        ],
    );
    r.wire(clock, "gate", delay, "clock");
    r.wire(delay, "left", mixer, "in2");
    r.wire(delay, "right", mixer, "in3");
    r.wire(mixer, "out", out, "left");
    r.wire(mixer, "out", out, "right");
    r.route(env, "out", filter, "cutoff_hz", 0.3);
    r.e
}

type Events = Vec<(f32, u8, bool)>;

fn notes(list: &[(u8, f32, f32)]) -> Events {
    list.iter()
        .flat_map(|&(n, a, b)| [(a, n, true), (b, n, false)])
        .collect()
}

fn no_keys() -> Events {
    Vec::new()
}

fn cable_phrase() -> Events {
    notes(&[
        (57, 0.3, 14.0),
        (60, 0.3, 14.0),
        (64, 0.3, 14.0),
        (67, 0.3, 14.0),
    ])
}

type Builder = fn() -> PatchEditor;
type Demo = (
    &'static str,
    Builder,
    fn() -> Events,
    f32,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static str,
);

const DEMOS: [Demo; 4] = [
    (
        "arp-chord",
        arp_chord,
        chord_phrase,
        16.0,
        "Arp Chord",
        "Keys",
        &["arpeggiator", "poly", "delay", "ambient"],
        "Hold a chord and the arpeggiator plays it up and down across three octaves, a note per clock tick, with a ping-pong delay. Change the mode, the octave range or the gate length; turn on Latch to take your hands off.",
    ),
    (
        "swing-seq",
        swing_seq,
        no_keys,
        16.0,
        "Swing Seq",
        "Sequence",
        &["swing", "clock", "bass", "self-playing", "groove"],
        "A bass line and a hi-hat on one clock. The clock's Swing delays every second 16th, so both swing together; set Swing to 0 to hear it straight.",
    ),
    (
        "ratchet-roll",
        ratchet_roll,
        no_keys,
        20.0,
        "Ratchet Roll",
        "Sequence",
        &["ratchet", "probability", "lead", "self-playing", "evolving"],
        "A lead whose third, sixth and eighth steps repeat two, three and four times inside their step, with some steps left to chance. A ratcheted step plays all its repeats or none. Open the sequencer's more controls to change the ratchets.",
    ),
    (
        "arp-cable",
        arp_cable,
        cable_phrase,
        20.0,
        "Arp Cable",
        "Keys",
        &["arpeggiator", "cable pattern", "random", "delay", "evolving"],
        "A random arpeggio of the chord you hold, gated by a six-step cable that runs against the 16ths and thrown to a delay by a four-step cable that happens seven times in ten. Hold a chord and let it run.",
    ),
];

fn patch_dir(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../patches/sound-engines/demos")
        .join(name)
}

fn sound_toml(name: &str, category: &str, tags: &[&str], description: &str, keys: bool) -> String {
    let tags: Vec<String> = tags.iter().map(|t| format!("    \"{t}\",\n")).collect();
    format!(
        "name = \"{name}\"\ncategory = \"{category}\"\ntags = [\n{}]\ndescription = \"{description}\"\nkeys = {keys}\nsequence = {}\n",
        tags.concat(),
        !keys
    )
}

/// Not a check: writes `patches/sound-engines/demos/*`.
#[test]
#[ignore]
fn write_patches() {
    for (dir, build, events, _, name, category, tags, description) in DEMOS {
        let path = patch_dir(dir);
        let _ = std::fs::remove_dir_all(&path);
        kabl_core::save(&path, build().log()).unwrap();
        std::fs::write(
            path.join("sound.toml"),
            sound_toml(name, category, tags, description, !events().is_empty()),
        )
        .unwrap();
    }
}

fn render(st: &PatchState, events: &Events, secs: f32) -> Vec<f32> {
    let collector = basedrop::Collector::new();
    let handle = collector.handle();
    let mut e = PatchEngine::new(&handle, st, SR, 8).unwrap();
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    let mut out = Vec::new();
    for b in 0..(secs * SR) as usize / BLOCK {
        let t0 = (b * BLOCK) as f32 / SR;
        let t1 = t0 + BLOCK as f32 / SR;
        for &(t, n, on) in events {
            if t >= t0 && t < t1 {
                e.key(if on {
                    KeyEvent::On {
                        note: n,
                        velocity: 100,
                    }
                } else {
                    KeyEvent::Off { note: n }
                });
            }
        }
        e.process_block(&mut l, &mut r);
        for (a, c) in l.iter().zip(&r) {
            out.push(*a);
            out.push(*c);
        }
    }
    out
}

/// The same patch with the clock's swing set to `swing`.
fn with_swing(st: &PatchState, swing: f32) -> PatchState {
    let mut st = st.clone();
    for m in st.modules.values_mut().filter(|m| m.kind == "clock") {
        m.params.insert("swing".into(), swing);
    }
    st
}

fn write_wav(path: &std::path::Path, audio: &[f32]) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for s in audio {
        w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16)
            .unwrap();
    }
    w.finalize().unwrap();
}

/// Not a check: `target/arp-demos/*.wav`. The swing clip is eight seconds straight, then eight
/// swung.
#[test]
#[ignore]
fn write_renders() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/arp-demos");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, build, events, secs, ..) in DEMOS {
        let st = build().state().clone();
        let audio = if name == "swing-seq" {
            let mut a = render(&with_swing(&st, 0.0), &events(), 8.0);
            a.extend(render(&st, &events(), 8.0));
            a
        } else {
            render(&st, &events(), secs)
        };
        write_wav(&dir.join(format!("{name}.wav")), &audio);
    }
}

fn peak_rms(v: &[f32]) -> (f32, f32) {
    (
        v.iter().fold(0.0, |m, x| f32::max(m, x.abs())),
        (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt(),
    )
}

#[test]
fn committed_demos_match_builders_and_play() {
    let mut bad = Vec::new();
    for (dir, build, events, secs, name, ..) in DEMOS {
        let saved = kabl_core::load(&patch_dir(dir)).unwrap();
        let built = build();
        assert_eq!(saved.state(), built.state(), "{dir}");
        let st = saved.state();
        for m in st.modules.values() {
            let info = kabl_modules::registry::info_for(&m.kind).unwrap();
            for p in info.params {
                if let Some(&v) = m.params.get(p.name) {
                    assert!(
                        (p.min..=p.max).contains(&v),
                        "{dir}: {} {} = {v}",
                        m.kind,
                        p.name
                    );
                }
            }
        }
        let meta = std::fs::read_to_string(patch_dir(dir).join("sound.toml")).unwrap();
        assert!(meta.contains(&format!("name = \"{name}\"")), "{dir}");
        kabl_engine::compile::compile(st, SR, 8).expect("compiles");
        let audio = render(st, &events(), secs);
        assert!(audio.iter().all(|v| v.is_finite()), "{dir}");
        let (peak, rms) = peak_rms(&audio);
        println!("{dir}: peak {peak:.3} rms {rms:.4}");
        if !(peak < 0.98 && peak > 0.1 && rms > 0.01) {
            bad.push(format!("{dir}: peak {peak}, rms {rms}"));
        }
    }
    assert!(bad.is_empty(), "levels: {bad:?}");
}

#[test]
fn the_demos_use_what_they_show() {
    let kinds = |d: &str| -> Vec<String> {
        let b = DEMOS.iter().find(|x| x.0 == d).unwrap().1();
        b.state().modules.values().map(|m| m.kind.clone()).collect()
    };
    assert!(kinds("arp-chord").contains(&"arp".into()));
    assert!(kinds("arp-cable").contains(&"arp".into()));
    let cable = arp_cable();
    assert!(cable.state().cables.values().any(|c| !c.params.is_empty()));
    let roll = ratchet_roll();
    let seq = roll
        .state()
        .modules
        .values()
        .find(|m| m.kind == "seq")
        .unwrap();
    assert!(seq.params.contains_key("k6"));
    assert!(seq.params.contains_key("r2"));
}

#[test]
fn swing_changes_the_swing_demo_and_nothing_else_does() {
    let st = swing_seq().state().clone();
    let straight = render(&with_swing(&st, 0.0), &no_keys(), 6.0);
    let swung = render(&st, &no_keys(), 6.0);
    let diff = straight
        .iter()
        .zip(&swung)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max);
    assert!(diff > 0.05, "swing does nothing: {diff}");
    // Same energy: it is the same notes, moved.
    let (_, a) = peak_rms(&straight);
    let (_, b) = peak_rms(&swung);
    assert!((a / b - 1.0).abs() < 0.15, "{a} vs {b}");
}
