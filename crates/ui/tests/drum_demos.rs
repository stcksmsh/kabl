//! The two drum demos: a four-on-the-floor groove played by five sequencers on one clock, and a
//! pad pumping against the kick through the compressor's sidechain. Each is built through the
//! real `PatchEditor`, so the committed patch is what the editor makes.
//!
//! Rewrite the patches with `cargo test -p kabl-ui --test drum_demos write_patches -- --ignored`
//! and the audio (`target/drum-demos/*.wav`) with `write_renders`.

use kabl_core::{ModuleId, PatchState, PortRef, Vec2};
use kabl_engine::graph::BLOCK;
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
        self.row += 380.0;
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
        self.col += 330.0;
        for &(name, v) in params {
            self.e.set_param(id, name, v);
        }
        id
    }
    fn wire(&mut self, a: ModuleId, ap: &str, b: ModuleId, bp: &str) {
        self.e.connect(port(a, ap), port(b, bp));
    }
}

/// A drum sequencer: eight steps, `steps` of them `(step 1..8, velocity)`, in bank A, and
/// `fill` in bank B (the bank the factory render launches into).
fn lane(r: &mut Rack, a: &[(usize, f32)], b: &[(usize, f32)]) -> ModuleId {
    let seq = r.add("seq", &[]);
    for (prefix, steps) in [("", a), ("b.", b)] {
        for k in 1..=8 {
            let v = steps.iter().find(|s| s.0 == k).map(|s| s.1);
            r.e.set_param(
                seq,
                &format!("{prefix}g{k}"),
                f32::from(u8::from(v.is_some())),
            );
            r.e.set_param(seq, &format!("{prefix}v{k}"), v.unwrap_or(0.8));
        }
    }
    seq
}

fn hit(r: &mut Rack, seq: ModuleId, voice: ModuleId, gate: &str) {
    r.wire(seq, "gate", voice, gate);
    r.wire(seq, "velocity", voice, "velocity");
}

/// A house groove at 124 bpm over two beats: kick on every beat, snare and clap on the second,
/// open hats on the off-beats cut by closed hats, a tom fill in the second bank.
fn four_floor() -> PatchEditor {
    let mut r = Rack::new();
    let clock = r.add("clock", &[("bpm", 124.0)]);
    let kick_seq = lane(&mut r, &[(1, 1.0), (5, 0.95)], &[(1, 1.0), (5, 0.95)]);
    let back_seq = lane(&mut r, &[(5, 1.0)], &[(5, 1.0), (8, 0.45)]);
    let closed_seq = lane(
        &mut r,
        &[(1, 0.7), (2, 0.35), (4, 0.6), (5, 0.7), (6, 0.35), (8, 0.6)],
        &[(1, 0.8), (2, 0.4), (4, 0.65), (5, 0.8), (6, 0.4), (8, 0.65)],
    );
    r.next_row();
    let open_seq = lane(&mut r, &[(3, 0.85), (7, 0.85)], &[(3, 0.9), (7, 0.9)]);
    let tom_seq = lane(&mut r, &[], &[(6, 0.7), (7, 0.8)]);
    let kick = r.add("drum.kick", &[("level_db", 0.0)]);
    let snare = r.add("drum.snare", &[("level_db", -6.0)]);
    r.next_row();
    let clap = r.add("drum.clap", &[("level_db", -6.0)]);
    let hat = r.add("drum.hat", &[("level_db", -6.0)]);
    let tom = r.add("drum.tom", &[("level_db", -5.0), ("tune_hz", 110.0)]);
    let kit = r.add(
        "mixer",
        &[
            ("level1", 0.9),
            ("level2", 0.55),
            ("level3", 0.5),
            ("level4", 0.55),
        ],
    );
    r.next_row();
    let fill = r.add("mixer", &[("level1", 1.0), ("level2", 0.7)]);
    let out = r.add("out", &[]);
    for seq in [kick_seq, back_seq, closed_seq, open_seq, tom_seq] {
        r.wire(clock, "gate", seq, "clock");
        r.wire(clock, "reset", seq, "reset");
    }
    hit(&mut r, kick_seq, kick, "gate");
    hit(&mut r, back_seq, snare, "gate");
    r.wire(back_seq, "gate", clap, "gate");
    r.wire(back_seq, "velocity", clap, "velocity");
    r.wire(closed_seq, "gate", hat, "closed");
    r.wire(closed_seq, "velocity", hat, "velocity");
    r.wire(open_seq, "gate", hat, "open");
    hit(&mut r, tom_seq, tom, "gate");
    r.wire(kick, "out", kit, "in1");
    r.wire(snare, "out", kit, "in2");
    r.wire(clap, "out", kit, "in3");
    r.wire(hat, "out", kit, "in4");
    r.wire(kit, "out", fill, "in1");
    r.wire(tom, "out", fill, "in2");
    r.wire(fill, "out", out, "left");
    r.wire(fill, "out", out, "right");
    r.e
}

/// A saw pad ducked by the kick: the kick's output goes to the compressor's sidechain, the pad
/// to its input. Claps and hats ride on top.
fn pump() -> PatchEditor {
    let mut r = Rack::new();
    let clock = r.add("clock", &[("bpm", 122.0)]);
    let kick_seq = lane(&mut r, &[(1, 1.0), (5, 1.0)], &[(1, 1.0), (5, 1.0)]);
    let clap_seq = lane(&mut r, &[(5, 1.0)], &[(5, 1.0)]);
    let hat_seq = lane(
        &mut r,
        &[(3, 0.8), (7, 0.8)],
        &[(2, 0.4), (3, 0.8), (6, 0.4), (7, 0.8)],
    );
    r.next_row();
    let kick = r.add(
        "drum.kick",
        &[("level_db", -3.0), ("decay_ms", 300.0), ("drive_db", 10.0)],
    );
    let clap = r.add("drum.clap", &[("level_db", -9.0)]);
    let hat = r.add("drum.hat", &[("level_db", -10.0)]);
    r.next_row();
    let osc_a = r.add(
        "osc.va",
        &[
            ("waveform", 2.0),
            ("base_hz", 110.0),
            ("unison", 3.0),
            ("detune", 14.0),
        ],
    );
    let osc_b = r.add(
        "osc.va",
        &[
            ("waveform", 2.0),
            ("base_hz", 164.81),
            ("unison", 3.0),
            ("detune", 12.0),
        ],
    );
    let pad = r.add("mixer", &[("level1", 0.5), ("level2", 0.4)]);
    let tone = r.add(
        "filter.ladder",
        &[
            ("cutoff_hz", 1100.0),
            ("resonance", 0.15),
            ("drive_db", 0.0),
        ],
    );
    r.next_row();
    let duck = r.add(
        "compressor",
        &[
            ("threshold_db", -34.0),
            ("ratio", 12.0),
            ("attack_ms", 1.0),
            ("release_ms", 220.0),
            ("knee_db", 3.0),
            ("makeup_db", 4.0),
        ],
    );
    let mix = r.add(
        "mixer",
        &[
            ("level1", 0.45),
            ("level2", 0.22),
            ("level3", 0.2),
            ("level4", 0.38),
        ],
    );
    let out = r.add("out", &[]);
    for seq in [kick_seq, clap_seq, hat_seq] {
        r.wire(clock, "gate", seq, "clock");
        r.wire(clock, "reset", seq, "reset");
    }
    hit(&mut r, kick_seq, kick, "gate");
    hit(&mut r, clap_seq, clap, "gate");
    hit(&mut r, hat_seq, hat, "open");
    r.wire(osc_a, "out", pad, "in1");
    r.wire(osc_b, "out", pad, "in2");
    r.wire(pad, "out", tone, "in");
    r.wire(tone, "out", duck, "in_l");
    r.wire(tone, "out", duck, "in_r");
    r.wire(kick, "out", duck, "sc");
    r.wire(kick, "out", mix, "in1");
    r.wire(clap, "out", mix, "in2");
    r.wire(hat, "out", mix, "in3");
    r.wire(duck, "left", mix, "in4");
    r.wire(mix, "out", out, "left");
    r.wire(mix, "out", out, "right");
    r.e
}

type Demo = (
    &'static str,
    fn() -> PatchEditor,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static str,
);

const DEMOS: [Demo; 2] = [
    (
        "four-floor",
        four_floor,
        "Four On The Floor",
        "Sequence",
        &["drums", "house", "groove", "self-playing", "kick"],
        "A house groove on one clock: kick on every beat, snare and clap on the second, open hats on the off-beats cut off by closed hats. Launch the second bank for the snare ghost note and a tom fill.",
    ),
    (
        "pump",
        pump,
        "Kick Pump",
        "Sequence",
        &["drums", "sidechain", "compressor", "pad", "self-playing"],
        "A saw pad ducked by the kick: the kick's output goes to the compressor's sidechain input, so the pad breathes in time with every beat. Lower the threshold or raise the ratio for harder pumping.",
    ),
];

fn patch_dir(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../patches/sound-engines/demos")
        .join(name)
}

fn sound_toml(name: &str, category: &str, tags: &[&str], description: &str) -> String {
    let tags: Vec<String> = tags.iter().map(|t| format!("    \"{t}\",\n")).collect();
    format!(
        "name = \"{name}\"\ncategory = \"{category}\"\ntags = [\n{}]\ndescription = \"{description}\"\nkeys = false\nsequence = true\n",
        tags.concat(),
    )
}

/// Not a check: writes `patches/sound-engines/demos/*`.
#[test]
#[ignore]
fn write_patches() {
    for (dir, build, name, category, tags, description) in DEMOS {
        let path = patch_dir(dir);
        let _ = std::fs::remove_dir_all(&path);
        kabl_core::save(&path, build().log()).unwrap();
        std::fs::write(
            path.join("sound.toml"),
            sound_toml(name, category, tags, description),
        )
        .unwrap();
    }
}

fn render(st: &PatchState, secs: f32) -> Vec<f32> {
    let collector = basedrop::Collector::new();
    let mut e = PatchEngine::new(&collector.handle(), st, SR, 8).unwrap();
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    let mut out = Vec::new();
    for _ in 0..(secs * SR) as usize / BLOCK {
        e.process_block(&mut l, &mut r);
        for (a, c) in l.iter().zip(&r) {
            out.push(*a);
            out.push(*c);
        }
    }
    out
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

/// Not a check: `target/drum-demos/*.wav`, 16 seconds each.
#[test]
#[ignore]
fn write_renders() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/drum-demos");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, build, ..) in DEMOS {
        let audio = render(build().state(), 16.0);
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
    for (dir, build, name, ..) in DEMOS {
        let saved = kabl_core::load(&patch_dir(dir)).unwrap();
        assert_eq!(saved.state(), build().state(), "{dir}");
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
        let audio = render(st, 8.0);
        assert!(audio.iter().all(|v| v.is_finite()), "{dir}");
        let (peak, rms) = peak_rms(&audio);
        println!("{dir}: peak {peak:.3} rms {rms:.4}");
        assert!(peak < 0.98 && peak > 0.1 && rms > 0.02, "{dir}");
    }
}

/// RMS in 20 ms windows over `x` (stereo interleaved, left only).
fn windows(x: &[f32]) -> Vec<f32> {
    x.chunks(960 * 2)
        .map(|c| (c.iter().step_by(2).map(|v| v * v).sum::<f32>() / (c.len() / 2) as f32).sqrt())
        .collect()
}

#[test]
fn the_pump_patch_ducks_its_pad_on_the_kick() {
    let mut st = pump().state().clone();
    // Hear the pad alone: every other input of the final mixer down, the sidechain left alone.
    let (mix, duck) = {
        let find = |k: &str| *st.modules.iter().find(|(_, m)| m.kind == k).unwrap().0;
        (
            *st.modules
                .iter()
                .filter(|(_, m)| m.kind == "mixer")
                .map(|(i, _)| i)
                .max()
                .unwrap(),
            find("compressor"),
        )
    };
    for n in ["level1", "level2", "level3"] {
        st.modules
            .get_mut(&mix)
            .unwrap()
            .params
            .insert(n.into(), 0.0);
    }
    let depth = |st: &PatchState| {
        let w = windows(&render(st, 8.0)[2 * 48000..]);
        let (lo, hi) = w
            .iter()
            .fold((f32::MAX, 0f32), |(lo, hi), &v| (lo.min(v), hi.max(v)));
        20.0 * (hi / lo.max(1e-6)).log10()
    };
    let ducked = depth(&st);
    // The same patch with the compressor out of the way (ratio 1): the pad's own movement.
    st.modules
        .get_mut(&duck)
        .unwrap()
        .params
        .insert("ratio".into(), 1.0);
    let flat = depth(&st);
    println!("pad level swing: with the sidechain {ducked:.1} dB, ratio 1 {flat:.1} dB");
    assert!(ducked > flat + 8.0, "{ducked} {flat}");
}

#[test]
fn the_groove_hits_on_the_beat_and_the_second_bank_adds_the_fill() {
    let st = four_floor().state().clone();
    let beat = (60.0 / 124.0 * SR) as usize;
    let audio = render(&st, 6.0);
    let left: Vec<f32> = audio.iter().step_by(2).copied().collect();
    // The clock starts at once: the first kick is on sample 0 and then every beat.
    let early = |start: usize| {
        left[start..start + 480]
            .iter()
            .fold(0f32, |m, v| m.max(v.abs()))
    };
    for k in 0..6 {
        assert!(early(k * beat) > 0.2, "no hit on beat {k}");
    }
    let kinds: Vec<_> = st.modules.values().map(|m| m.kind.as_str()).collect();
    for k in [
        "drum.kick",
        "drum.snare",
        "drum.clap",
        "drum.hat",
        "drum.tom",
    ] {
        assert!(kinds.contains(&k), "{k}");
    }
}
