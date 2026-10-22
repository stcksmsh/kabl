//! Curated-bank browser, bindings, recall and reproducible audio evidence.
#[path = "support/factory_bank.rs"]
mod bank;

use kabl_core::PatchState;
use kabl_engine::{
    graph::BLOCK,
    keyboard::KeyEvent,
    patch_engine::{Launch, PatchEngine, Timing},
};
use kabl_modules::{builtins::Transport, registry};
use kabl_ui::{
    browser,
    library::{Library, Meta},
    perform, PatchEditor, UiState,
};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches")
}

#[test]
#[ignore = "writes three original factory graphs and curated metadata"]
fn write_bank() {
    for (dir, name, category, mut editor) in [
        (
            "palette/keyboard-bass",
            "Round Keyboard Bass",
            "Bass",
            bank::keyboard(true),
        ),
        ("palette/keys", "Glass Keys", "Keys", bank::keyboard(false)),
        (
            "palette/progression",
            "Slow Horizons",
            "Pad",
            bank::progression(),
        ),
    ] {
        let path = root().join(dir);
        bank::attach_guide(&mut editor, dir);
        kabl_core::save(&path, editor.log()).unwrap();
        let (keys, sequence) = Meta::play_of(editor.state());
        let meta = Meta {
            name: name.into(),
            category: category.into(),
            keys,
            sequence,
            ..Meta::default()
        };
        std::fs::write(
            path.join("sound.toml"),
            toml::to_string_pretty(&meta).unwrap(),
        )
        .unwrap();
    }
    for (dir, _, _, _) in bank::INVENTORY {
        let path = root().join(dir);
        let mut editor = PatchEditor::from_log(kabl_core::load(&path).unwrap());
        bank::attach_guide(&mut editor, dir);
        kabl_core::save(&path, editor.log()).unwrap();
    }
    bank::write_metadata(&root());
}

#[test]
fn purpose_filters_preserve_legacy_metadata_and_user_saves() {
    let legacy: Meta =
        toml::from_str("name='Old sequence'\ncategory='Bass'\nsequence=true").unwrap();
    assert!(legacy.in_category("Sequences"));
    assert!(legacy.in_category("Bass"));
    assert!(!legacy.in_category("Sounds"));
    let tmp = tempfile::tempdir().unwrap();
    let mut lib = Library::open(Some(root()), tmp.path().into());
    let keys = lib.get("factory:palette/keys").unwrap().clone();
    assert!(keys.matches("sounds pluck bright"));
    assert!(!keys.matches("sequences"));
    let piece = lib.get("factory:sound-palette").unwrap();
    assert!(piece.matches("104bpm"));
    assert!(!piece.matches("112bpm"));
    let log = lib.read(&keys.id).unwrap();
    let user = lib
        .save(
            &log,
            Meta {
                name: "My keys".into(),
                ..keys.meta.clone()
            },
            None,
        )
        .unwrap();
    assert_eq!(
        lib.get(&user).unwrap().meta.description,
        keys.meta.description
    );
    for purpose in ["Sounds", "Sequences", "Performances"] {
        let entries: Vec<_> = lib
            .entries
            .iter()
            .filter(|e| {
                e.meta.tags.iter().any(|t| t == "curated")
                    && e.meta.in_category(purpose)
                    && e.origin == kabl_ui::library::Origin::Factory
            })
            .collect();
        assert_eq!(
            entries.len(),
            match purpose {
                "Sounds" => 6,
                "Sequences" => 4,
                _ => 2,
            }
        );
        assert!(entries.iter().all(|e| e.matches(purpose)));
    }
    assert_eq!(
        lib.entries
            .iter()
            .filter(|e| e.origin == kabl_ui::library::Origin::User
                && e.meta.in_category("Sounds")
                && e.matches("pluck"))
            .count(),
        1
    );
    let user_dir = lib.get(&user).unwrap().dir.clone();
    std::fs::write(
        user_dir.join("sound.toml"),
        "name='Legacy keys'\ncategory='Keys'",
    )
    .unwrap();
    lib.rescan();
    assert!(lib.get(&user).unwrap().meta.keys);
    assert!(lib.get(&user).unwrap().meta.in_category("Sounds"));
}

#[test]
fn every_entry_opens_stopped_has_real_controls_and_recalls_complete_state() {
    let tmp = tempfile::tempdir().unwrap();
    let mut ui = UiState::default();
    ui.library = Some(Library::open(Some(root()), tmp.path().into()));
    let mut editor = PatchEditor::new();
    for (dir, _, _, _) in bank::INVENTORY {
        browser::perform(
            &mut editor,
            &mut ui,
            browser::Pending::Open(format!("factory:{dir}")),
        );
        assert_eq!(
            ui.doc.as_ref().unwrap().name,
            ui.library
                .as_ref()
                .unwrap()
                .get(&format!("factory:{dir}"))
                .unwrap()
                .meta
                .name
        );
        let state = editor.state();
        let pins = perform::pins(state);
        assert!(pins.len() >= 6, "{dir}: prepared controls");
        let macros: Vec<_> = state
            .modules
            .iter()
            .filter(|(_, m)| m.kind == "macro")
            .collect();
        assert!(!macros.is_empty(), "{dir}: musical macros");
        for (&id, _) in macros {
            for n in 1..=4 {
                let key = format!("m{n}");
                assert!(
                    kabl_ui::macro_name(state, id, &key).is_some(),
                    "{dir}: macro name"
                );
                assert!(
                    state.cables.values().any(|c| c.from
                        == kabl_core::PortRef::Module {
                            id,
                            port: key.clone()
                        }
                        && c.to
                            .module_id()
                            .is_some_and(|m| state.modules.contains_key(&m))),
                    "{dir}: macro route"
                );
            }
        }
        for pin in &pins {
            if !["transport", "banks", "cues"].contains(&pin.key.as_str()) {
                let info = registry::info_for(&state.modules[&pin.id].kind).unwrap();
                assert!(info.params.iter().any(|p| p.name == pin.key));
            }
        }
        let (_, seq) = Meta::play_of(state);
        assert_eq!(ui.load_stopped, seq);
        if seq {
            assert!(pins.iter().any(|p| p.key == "transport"));
            assert!(
                pins.iter().any(|p| p.key == "banks" || p.key == "cues"),
                "{dir}: launch workflow"
            );
        }
        if ui.doc.as_ref().unwrap().meta.content_type() == "Performances" {
            assert!(pins.iter().any(|p| p.key == "cues"));
        }
        let macro_pin = pins
            .iter()
            .find(|p| state.modules[&p.id].kind == "macro")
            .unwrap();
        editor.set_param(macro_pin.id, &macro_pin.key, 0.6);
        let saved = tmp.path().join("recalled");
        kabl_core::save(&saved, editor.log()).unwrap();
        assert_eq!(kabl_core::load(&saved).unwrap().state(), editor.state());
    }
}

/// `render_raw` plus the check that a stopped load is silent for the first second.
fn render(state: &PatchState, keys: bool, secs: usize, hot: bool) -> (Vec<f32>, serde_json::Value) {
    let (samples, metrics) = render_raw(state, keys, secs, hot);
    assert!(
        rms(&samples[..96000]) < 0.000001,
        "Stopped load must remain silent"
    );
    (samples, metrics)
}

fn rms(v: &[f32]) -> f64 {
    (v.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / v.len() as f64).sqrt()
}

/// A recorded voice for patches with `audio.in`: `docs/vocoder/audio/voice-espeak-16k.wav` (made
/// with espeak-ng, see docs/vocoder/README.md), brought to 48 kHz by linear interpolation and
/// scaled to -20 dBFS RMS over the samples that carry sound, then repeated with half a second
/// between. It starts at two seconds, so the stopped-load check still holds.
struct SyntheticVoice {
    samples: Vec<f32>,
}

impl SyntheticVoice {
    fn new() -> Self {
        let path = root().join("../docs/vocoder/audio/voice-espeak-16k.wav");
        let mut r = hound::WavReader::open(path).unwrap();
        let src: Vec<f32> = r
            .samples::<i16>()
            .map(|s| s.unwrap() as f32 / 32768.0)
            .collect();
        let mut samples = Vec::with_capacity(src.len() * 3 + 24000);
        for w in src.windows(2) {
            for k in 0..3 {
                samples.push(w[0] + (w[1] - w[0]) * k as f32 / 3.0);
            }
        }
        samples.extend(std::iter::repeat_n(0.0, 24000));
        let (sum, n) = samples
            .iter()
            .filter(|s| s.abs() > 0.002)
            .fold((0.0f64, 0usize), |(a, n), s| {
                (a + (*s as f64).powi(2), n + 1)
            });
        let gain = 0.1 / (sum / n as f64).sqrt() as f32;
        samples.iter_mut().for_each(|s| *s *= gain);
        SyntheticVoice { samples }
    }

    fn block(&mut self, t: usize) -> ([f32; BLOCK], [f32; BLOCK]) {
        let mut out = [0.0; BLOCK];
        for (i, o) in out.iter_mut().enumerate() {
            if let Some(n) = (t + i).checked_sub(2 * 48000) {
                *o = self.samples[n % self.samples.len()];
            }
        }
        (out, out)
    }
}

fn render_raw(
    state: &PatchState,
    keys: bool,
    secs: usize,
    hot: bool,
) -> (Vec<f32>, serde_json::Value) {
    let collector = basedrop::Collector::new();
    let mut engine = PatchEngine::new(&collector.handle(), state, 48000.0, 8).unwrap();
    let clocks: Vec<_> = state
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "clock")
        .map(|(&id, _)| id)
        .collect();
    let seqs: Vec<_> = state
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "seq")
        .map(|(&id, _)| (id, 1u8))
        .collect();
    let stop = secs - 8;
    // Patches that take sound from outside are measured with a stand-in voice.
    let mut voice = state
        .modules
        .values()
        .any(|m| m.kind == "audio.in")
        .then(SyntheticVoice::new);
    let mut samples = Vec::with_capacity(secs * 96000);
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    let mut active_windows = 0;
    let mut macro_changes = 0;
    let mut layer_changes = 0;
    for b in 0..secs * 48000 / BLOCK {
        let t = b * BLOCK;
        if t == 0 {
            for id in &clocks {
                engine.transport(*id, Transport::Stop);
            }
        }
        if t == 48000 {
            for id in &clocks {
                engine.transport(*id, Transport::Run);
            }
        }
        if keys
            && t < stop * 48000
            && [1, 5, 13, 21, 29, 37, 45, 53].contains(&(t / 48000))
            && t.is_multiple_of(48000)
        {
            for note in if state
                .modules
                .values()
                .any(|m| m.kind == "midi.in" && m.params.get("mode").is_some_and(|v| *v > 0.0))
            {
                vec![
                    if state.modules.get(&1).is_some_and(|m| m.kind == "midi.in") {
                        48
                    } else {
                        64
                    },
                ]
            } else {
                vec![52, 59, 64, 67]
            } {
                engine.key(KeyEvent::On { note, velocity: 90 });
            }
        }
        if keys && [4, 9, 17, 25, 33, 41, 49, 57].contains(&(t / 48000)) && t.is_multiple_of(48000)
        {
            engine.key(KeyEvent::AllOff);
        }
        if t == 6 * 48000 && !seqs.is_empty() {
            engine.launch(&Launch::new(clocks[0], Timing::NextBar, &seqs));
        }
        if secs > 24 && t.is_multiple_of(48000) && [2, 12, 24, 36, 48].contains(&(t / 48000)) {
            let section = [2, 12, 24, 36, 48]
                .iter()
                .position(|s| *s == t / 48000)
                .unwrap();
            let levels = [
                [0.8, 0.0, 0.0, 0.0],
                [0.3, 0.55, 0.0, 0.0],
                [0.15, 0.3, 0.55, 0.0],
                [0.0, 0.25, 0.35, 0.5],
                [0.5, 0.2, 0.3, 0.0],
            ][section];
            assert_eq!(state.modules[&81].kind, "mixer");
            for (index, value) in levels.into_iter().enumerate() {
                assert!(engine.set(&kabl_engine::runtime::ParamSet {
                    rev: t as u64 + 1,
                    target: kabl_engine::runtime::RuntimeTarget::Param {
                        id: 81,
                        kind: "mixer",
                        index: index as u16,
                    },
                    value,
                }));
                layer_changes += 1;
            }
            assert!(engine.set(&kabl_engine::runtime::ParamSet {
                rev: t as u64 + 1,
                target: kabl_engine::runtime::RuntimeTarget::Param {
                    id: 82,
                    kind: "mixer",
                    index: 0,
                },
                value: if section == 3 { 0.2 } else { 0.0 },
            }));
            layer_changes += 1;
            if let Some((&id, _)) = state.modules.iter().find(|(_, m)| m.kind == "cues") {
                let cue = [2, 12, 24, 36, 48]
                    .iter()
                    .position(|s| *s == t / 48000)
                    .unwrap()
                    + 1;
                let mut commands = Vec::new();
                kabl_ui::cues::launch(&mut commands, state, id, cue).unwrap();
                for command in commands {
                    engine.command(&command);
                }
            }
        }
        if t.is_multiple_of(48000) && [3, 7, 19, 31, 43, 55].contains(&(t / 48000)) {
            for (&id, m) in &state.modules {
                if m.kind == "macro" {
                    for n in 1..=4 {
                        let target = kabl_engine::runtime::RuntimeTarget::Param {
                            id,
                            kind: "macro",
                            index: n - 1,
                        };
                        let batch = kabl_engine::runtime::ParamSet {
                            rev: t as u64 + 1,
                            target,
                            value: if hot {
                                1.0
                            } else if t == 3 * 48000 {
                                0.65
                            } else {
                                0.15
                            },
                        };
                        assert!(engine.set(&batch));
                        macro_changes += 1;
                    }
                }
            }
        }
        if t == stop * 48000 {
            engine.key(KeyEvent::AllOff);
            for id in &clocks {
                engine.transport(*id, Transport::Stop);
            }
        }
        if let Some(v) = voice.as_mut() {
            let (il, ir) = v.block(t);
            engine.set_audio_in(&il, &ir);
        }
        engine.process_block(&mut l, &mut r);
        let mut sum = 0.0;
        for (a, c) in l.iter().zip(&r) {
            assert!(a.is_finite() && c.is_finite());
            samples.extend_from_slice(&[*a, *c]);
            sum += a * a + c * c;
        }
        if sum / (BLOCK * 2) as f32 > 0.00001 {
            active_windows += 1;
        }
    }
    let db = |v: f64| 20.0 * v.max(1e-12).log10();
    let peak = samples.iter().fold(0.0_f32, |a, b| a.max(b.abs()));
    let metrics = serde_json::json!({"peak_dbfs":db(peak as f64), "rms_dbfs":db(rms(&samples)), "last_second_rms_dbfs":db(rms(&samples[samples.len()-96000..])), "active_seconds_above_minus50_dbfs": active_windows as f64 * BLOCK as f64 / 48000.0, "stop_second":stop,"duration_seconds":secs,"macro_changes":macro_changes,"layer_changes":layer_changes});
    (samples, metrics)
}

#[test]
#[ignore = "renders every entry, checks headroom, writes metrics and WAV previews"]
fn render_bank() {
    let out = std::env::var_os("KABL_BANK_EVIDENCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| root().parent().unwrap().join("scratch/factory-bank/audio"));
    std::fs::create_dir_all(&out).unwrap();
    let mut metrics = serde_json::Map::new();
    for (dir, _, _, _) in bank::INVENTORY {
        let log = kabl_core::load(&root().join(dir)).unwrap();
        let (keys, _) = Meta::play_of(log.state());
        let (samples, m) = render(log.state(), keys, 24, false);
        let (_, hot) = render(log.state(), keys, 24, true);
        println!("{dir}: {m}; hot {hot}");
        assert!(m["peak_dbfs"].as_f64().unwrap() < -1.0, "{dir}: headroom");
        assert!(
            hot["peak_dbfs"].as_f64().unwrap() < -0.5,
            "{dir}: hot headroom"
        );
        assert!(
            m["active_seconds_above_minus50_dbfs"].as_f64().unwrap() > 1.0,
            "{dir}: active"
        );
        let mut w = hound::WavWriter::create(
            out.join(format!("{}.wav", dir.replace('/', "-"))),
            hound::WavSpec {
                channels: 2,
                sample_rate: 48000,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .unwrap();
        for s in samples {
            w.write_sample(s).unwrap();
        }
        w.finalize().unwrap();
        metrics.insert(
            (*dir).into(),
            serde_json::json!({"normal":m,"all_macros_one":hot}),
        );
    }
    std::fs::write(
        out.join("metrics.json"),
        serde_json::to_string_pretty(&metrics).unwrap(),
    )
    .unwrap();
    let project = kabl_core::load(&root().join("sound-palette")).unwrap();
    kabl_core::save(&out.parent().unwrap().join("performance-project"), &project).unwrap();
    let (samples, metrics) = render(project.state(), true, 72, false);
    assert!(metrics["peak_dbfs"].as_f64().unwrap() < -1.0);
    let mut w = hound::WavWriter::create(
        out.join("editable-performance.wav"),
        hound::WavSpec {
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .unwrap();
    for sample in samples {
        w.write_sample(sample).unwrap();
    }
    w.finalize().unwrap();
    std::fs::write(
        out.join("performance-metrics.json"),
        serde_json::to_string_pretty(&metrics).unwrap(),
    )
    .unwrap();
}

/// Every factory patch directory (one holding a `checkpoint.json`), relative to `patches/`.
fn all_factory_dirs() -> Vec<String> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
        if dir.join("checkpoint.json").is_file() {
            out.push(
                dir.strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        let mut subs: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        subs.sort();
        for s in subs {
            walk(base, &s, out);
        }
    }
    let mut out = Vec::new();
    walk(&root(), &root(), &mut out);
    out
}

/// Peak (dBFS) and loudness (dBFS RMS over the 100 ms blocks above -50 dBFS, so silent gaps
/// and the stopped lead-in do not count) of one factory patch played its own demo way.
fn level(dir: &str) -> (f64, f64) {
    let log = kabl_core::load(&root().join(dir)).unwrap();
    let meta: Meta =
        toml::from_str(&std::fs::read_to_string(root().join(dir).join("sound.toml")).unwrap())
            .unwrap();
    let keys = Meta::play_of(log.state()).0 || meta.keys;
    let (samples, _) = render_raw(log.state(), keys, 24, false);
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

/// Levels of every factory patch, rendered on a thread each.
fn levels() -> Vec<(String, f64, f64)> {
    std::thread::scope(|s| {
        let jobs: Vec<_> = all_factory_dirs()
            .into_iter()
            .map(|dir| {
                s.spawn(move || {
                    let (peak, loud) = level(&dir);
                    (dir, peak, loud)
                })
            })
            .collect();
        jobs.into_iter().map(|j| j.join().unwrap()).collect()
    })
}

#[test]
#[ignore = "prints the level table of the whole factory bank"]
fn level_table() {
    for (dir, peak, loud) in levels() {
        println!("LEVEL {dir:36} peak {peak:7.2}  loud {loud:7.2}");
    }
}

/// The bank's level target (docs/factory-levels/README.md): no sample above `PEAK_CEILING`,
/// loudness within `LOUD_WINDOW` of `LOUD_TARGET`.
const PEAK_CEILING: f64 = -1.0;
const LOUD_TARGET: f64 = -19.0;
const LOUD_WINDOW: f64 = 5.5;

#[test]
fn factory_levels_hold_the_target() {
    let mut bad = Vec::new();
    for (dir, peak, loud) in levels() {
        // The arpeggiator demos are silent without a held chord: nothing to measure.
        if loud < -100.0 {
            bad.push(format!("{dir}: silent"));
        } else if peak > PEAK_CEILING {
            bad.push(format!(
                "{dir}: peak {peak:.2} dBFS clips (ceiling {PEAK_CEILING})"
            ));
        } else if (loud - LOUD_TARGET).abs() > LOUD_WINDOW {
            bad.push(format!(
                "{dir}: loudness {loud:.2} dBFS is outside {LOUD_TARGET} +- {LOUD_WINDOW}"
            ));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// The stand-in voice speaks at -20 dBFS RMS while it sounds (about the level of a close-miked
/// voice), so `audio.in` patches are levelled for a typical input.
#[test]
fn synthetic_voice_is_about_minus_20_dbfs() {
    let mut v = SyntheticVoice::new();
    let mut sum = 0.0f64;
    let mut n = 0usize;
    for b in 0..(6 * 48000 / BLOCK) {
        let (l, _) = v.block(b * BLOCK);
        for s in l {
            if s != 0.0 {
                sum += (s as f64).powi(2);
                n += 1;
            }
        }
    }
    let db = 20.0 * (sum / n as f64).sqrt().log10();
    println!(
        "synthetic voice {db:.1} dBFS, peak {:.3}",
        v.samples.iter().fold(0.0f32, |a, s| a.max(s.abs()))
    );
    assert!((db + 20.0).abs() < 2.0, "{db:.1}");
}
