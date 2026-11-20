//! `kabl` command-line tool. It has no audio player of its own: live play (audio output, MIDI
//! input) is the `kabl-ui` app, the one live audio path.
//!
//! - `kabl cat|pack|unpack ...`: the .kabl file tools (`kabl_core::cli`).
//! - `kabl --save-default <file.kabl>`: writes `default_patch()` in the format `kabl-ui`'s Save
//!   writes, as a starting point to edit in `kabl-ui`.
//! - `kabl [--patch <file.kabl>]`: compiles the patch (or `default_patch()`) and renders a short
//!   chord to `target/spike-renders/standalone_selftest.wav`, proving the synthesis path works.
//!   `--midi` (the old live-play port choice) is ignored with a warning: use `kabl-ui`.

use std::path::Path;

use kabl_core::{Op, PatchLog, PatchState, Source};
use kabl_engine::graph::BLOCK;
use kabl_engine::voice_allocator::VoiceAllocator;
use kabl_standalone::{default_patch, DEFAULT_VOICE_COUNT};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // `kabl cat|pack|unpack`: the .kabl file tools, before audio and the logger start.
    if args
        .get(1)
        .is_some_and(|c| kabl_core::cli::COMMANDS.contains(&c.as_str()))
    {
        if let Err(msg) = kabl_core::cli::run(&args[1..], &mut std::io::stdout().lock()) {
            eprintln!("kabl: {msg}");
            std::process::exit(1);
        }
        return;
    }
    let (level, bad_level) = kabl_standalone::applog::level_from(&args);
    let log = kabl_standalone::applog::init("kabl", level, kabl_standalone::applog::log_dir());
    log::info!(
        target: "app",
        "start kabl {} level={level} log={}",
        kabl_standalone::applog::build_id(),
        log.as_ref().map_or("unavailable".into(), |l| l.path.display().to_string())
    );
    if let Some(w) = bad_level {
        log::warn!(target: "app", "unknown log level {w:?}: using info");
    }
    let code = run(&args);
    if let Some(mut l) = log {
        l.shutdown();
    }
    if code != 0 {
        std::process::exit(code);
    }
}

/// The program; returns the exit code (the logger flushes before exiting).
fn run(args: &[String]) -> i32 {
    let args = args.to_vec();

    if let Some(dir) = flag_value(&args, "--save-default") {
        match save_default_patch(Path::new(&dir)) {
            Ok(()) => log::info!(target: "app", "wrote default_patch() to {dir}"),
            Err(err) => log::warn!(target: "app", "failed to save to {dir}: {err:?}"),
        }
        return 0;
    }

    let patch = match flag_value(&args, "--patch") {
        Some(dir) => match kabl_core::format::load_kabl_full(Path::new(&dir)) {
            Ok((log, _, code)) => {
                // The user named this file on the command line, so its embedded code is
                // registered without the trust notice the UI shows.
                for w in kabl_standalone::code_load::register_for_cli(&code, log.state()) {
                    log::warn!(target: "app", "{w}");
                }
                log.state().clone()
            }
            Err(err) => {
                log::error!(target: "app", "failed to load patch from {dir}: {err:?}");
                return 1;
            }
        },
        None => default_patch(),
    };

    if args.iter().any(|a| a == "--midi") {
        log::warn!(target: "app", "--midi: live play moved to kabl-ui; this binary only renders");
    }
    render_self_test(&patch);
    0
}

/// Proves the synthesis path works by rendering a short chord progression straight through the compiler (bypassing `PatchEngine`/
/// `cpal` entirely) to a WAV file, rather than the binary just doing nothing observable.
fn render_self_test(patch: &kabl_core::PatchState) {
    let sample_rate = 48000.0;
    let mut compiled = match kabl_engine::compile::compile(patch, sample_rate, DEFAULT_VOICE_COUNT)
    {
        Ok(c) => c,
        Err(err) => {
            log::warn!(target: "app", "self-test patch failed to compile: {err}");
            return;
        }
    };

    let mut allocator = VoiceAllocator::new(DEFAULT_VOICE_COUNT);
    let chord = [60u32, 64, 67, 72]; // C major
    let mut voices = Vec::new();
    for (i, &note) in chord.iter().enumerate() {
        let voice = allocator.note_on(note);
        apply_note_on_direct(&mut compiled, voice, note as f32 - 60.0, 0.8);
        voices.push(voice);
        let _ = i;
    }

    let block = BLOCK;
    let sustain_blocks = (sample_rate / block as f32) as usize; // ~1s
    let release_blocks = (sample_rate / block as f32) as usize; // ~1s
    let mut rendered = Vec::with_capacity((sustain_blocks + release_blocks) * block);
    for _ in 0..sustain_blocks {
        compiled.process_block();
        rendered.extend_from_slice(&compiled.left()[..]);
    }
    for &voice in &voices {
        apply_note_off_direct(&mut compiled, voice);
    }
    for _ in 0..release_blocks {
        compiled.process_block();
        rendered.extend_from_slice(&compiled.left()[..]);
    }

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/spike-renders/standalone_selftest.wav");
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: sample_rate as u32,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    match hound::WavWriter::create(&path, spec) {
        Ok(mut writer) => {
            for &s in &rendered {
                let _ = writer.write_sample(s);
            }
            let _ = writer.finalize();
            log::info!(target: "app", "wrote a self-test render proving the synth path works: {}",
                path.display()
            );
        }
        Err(err) => log::warn!(target: "app", "couldn't write self-test render: {err}"),
    }
}

fn apply_note_on_direct(
    compiled: &mut kabl_engine::compile::CompiledPatch,
    voice: usize,
    semitones: f32,
    velocity: f32,
) {
    compiled.note_on(voice, semitones, velocity);
}

fn apply_note_off_direct(compiled: &mut kabl_engine::compile::CompiledPatch, voice: usize) {
    compiled.note_off(voice);
}

/// `--flag value` (two separate argv entries) — the simplest possible parser for two optional
/// flags. Not using a CLI-parsing crate here: two flags each taking one path argument doesn't
/// justify a new dependency (matches this project's "no new dependency without a decisions.md
/// line" rule), and this scales fine until a third flag actually needs it.
fn flag_value(args: &[String], flag: &str) -> Option<String> {
    let i = args.iter().position(|a| a == flag)?;
    args.get(i + 1).cloned()
}

/// Writes `default_patch()` out via `kabl_core::save`, replaying it as real `AddModule`/
/// `Connect`/`SetParam` ops (not just a bare state dump) so the saved file's history is genuine
/// and undoable back to empty when opened in `kabl-ui` — same reasoning as `PatchEditor::
/// seed_from`, duplicated here in miniature rather than depending on `kabl-ui` from `standalone`
/// (wrong direction: `kabl-ui` already depends on `kabl-standalone`, not the other way around).
fn save_default_patch(dir: &Path) -> Result<(), kabl_core::FormatError> {
    let patch: PatchState = default_patch();
    let mut log = PatchLog::new();
    for (&id, m) in &patch.modules {
        log.append(
            Op::AddModule {
                id,
                kind: m.kind.clone(),
                pos: m.pos,
            },
            0,
            Source::User,
        );
        for (name, &value) in &m.params {
            log.append(
                Op::SetParam {
                    target: kabl_core::ParamTarget::Module {
                        id,
                        param: name.clone(),
                    },
                    value,
                },
                0,
                Source::User,
            );
        }
    }
    for (cable_id, c) in (1u64..).zip(patch.cables.values()) {
        log.append(
            Op::Connect {
                id: cable_id,
                from: c.from.clone(),
                to: c.to.clone(),
            },
            0,
            Source::User,
        );
    }
    kabl_core::format::save_kabl(dir, &log)
}
