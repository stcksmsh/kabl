//! `kabl` standalone binary: opens the default audio output device and a MIDI input port,
//! compiles `default_patch()`, and plays it live. This is brief section 12's `standalone` crate
//! (cpal + midir) — the "something a person can open and hear" milestone.
//!
//! No UI yet (that's `kabl-ui`, separate) — this binary alone gets you a playable polysynth from
//! any class-compliant MIDI controller, nothing to patch or configure yet beyond `default_patch()`.
//!
//! If no audio output device is found (or none of its supported configs can do `f32` samples),
//! falls back to rendering a short self-test WAV instead of exiting silently — proves the
//! synthesis path is intact even on a machine (or container) with no audio hardware, rather than
//! just failing with no evidence either way.
//!
//! `--patch <dir>` loads a patch saved by `kabl_core::save` (the same format `kabl-ui`'s "Save"
//! button writes) instead of playing `default_patch()`. `--save-default <dir>` writes
//! `default_patch()` out in that format and exits, as a starting point to then edit with
//! `kabl-ui` — neither binary shipped any way to get a save file onto disk before this existed.
//! `--midi <substring>` connects to the first port whose name contains it (case-insensitive);
//! without it, the first port that isn't ALSA's own "Midi Through" virtual loopback (confirmed
//! live: a real controller plugged in still lost to it before this existed).

use std::path::Path;

use basedrop::Collector;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use kabl_core::{Op, PatchLog, PatchState, Source};
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::PatchEngine;
use kabl_engine::voice_allocator::VoiceAllocator;
use kabl_standalone::{
    apply_voice_event, connect_midi, default_patch, RingBuffer, VoiceEvent,
    DEFAULT_VOICE_COUNT, MIDI_IN_ID,
};

/// Samples of headroom each ring buffer keeps between the engine's `BLOCK`-sized output and
/// `cpal`'s host-chosen callback buffer size (which can be much larger than `BLOCK` and isn't
/// known until the stream is built). 256 blocks =~ 340ms at 48kHz — comfortably more than any
/// reasonable host buffer setting, so `RingBuffer::push_slice`'s overflow assert should never
/// fire in practice; if it ever does, that's a real host buffer size this needs to be raised for,
/// not a bug to silently work around.
const RING_CAPACITY: usize = BLOCK * 256;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if let Some(dir) = flag_value(&args, "--save-default") {
        match save_default_patch(Path::new(&dir)) {
            Ok(()) => eprintln!("kabl: wrote default_patch() to {dir}"),
            Err(err) => eprintln!("kabl: failed to save to {dir}: {err:?}"),
        }
        return;
    }

    let patch = match flag_value(&args, "--patch") {
        Some(dir) => match kabl_core::load(Path::new(&dir)) {
            Ok(log) => log.state().clone(),
            Err(err) => {
                eprintln!("kabl: failed to load patch from {dir}: {err:?}");
                std::process::exit(1);
            }
        },
        None => default_patch(),
    };

    let host = cpal::default_host();
    let Some(device) = host.default_output_device() else {
        eprintln!("kabl: no audio output device found on this system.");
        render_self_test(&patch);
        return;
    };

    let Some(config) = pick_output_config(&device) else {
        eprintln!("kabl: no usable (f32) output config found for the default audio device.");
        render_self_test(&patch);
        return;
    };

    let sample_rate = config.sample_rate() as f32;
    let channels = config.channels() as usize;
    eprintln!(
        "kabl: output device ready, {sample_rate} Hz, {channels} channel(s), {DEFAULT_VOICE_COUNT} voices"
    );

    let collector = Collector::new();
    let handle = collector.handle();
    let mut engine = match PatchEngine::new(&handle, &patch, sample_rate, DEFAULT_VOICE_COUNT) {
        Ok(e) => e,
        Err(err) => {
            eprintln!("kabl: failed to compile the default patch: {err}");
            std::process::exit(1);
        }
    };

    let (midi_producer, mut midi_consumer) = rtrb::RingBuffer::<VoiceEvent>::new(256);
    let _midi_connection = connect_midi("kabl", midi_producer, flag_value(&args, "--midi"));

    let mut left_ring = RingBuffer::new(RING_CAPACITY);
    let mut right_ring = RingBuffer::new(RING_CAPACITY);

    let stream = device.build_output_stream(
        config.config(),
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            // Audio thread: apply any MIDI events queued since the last callback, then produce
            // and interleave samples. No allocation anywhere in this closure body.
            while let Ok(event) = midi_consumer.pop() {
                apply_voice_event(&mut engine, MIDI_IN_ID, event);
            }

            let frames_needed = data.len() / channels;
            while left_ring.available() < frames_needed {
                let mut l = [0f32; BLOCK];
                let mut r = [0f32; BLOCK];
                engine.process_block(&mut l, &mut r);
                left_ring.push_slice(&l);
                right_ring.push_slice(&r);
            }

            for frame in data.chunks_mut(channels) {
                let l = left_ring.pop().unwrap_or(0.0);
                let r = right_ring.pop().unwrap_or(0.0);
                frame[0] = l;
                for s in frame.iter_mut().skip(1) {
                    *s = r;
                }
            }
        },
        |err| eprintln!("kabl: audio stream error: {err}"),
        None,
    );

    let stream = match stream {
        Ok(s) => s,
        Err(err) => {
            eprintln!("kabl: failed to build audio output stream: {err}");
            render_self_test(&patch);
            return;
        }
    };

    if let Err(err) = stream.play() {
        eprintln!("kabl: failed to start audio stream: {err}");
        render_self_test(&patch);
        return;
    }

    eprintln!("kabl: playing. Ctrl+C to quit.");
    // `collector`/`stream` just need to stay alive for the process's lifetime -- an infinite
    // loop here keeps both in scope without an explicit leak. Nothing to swap yet (no UI/
    // repatching), so no periodic `collector.collect()` is needed either: deferred-drop nodes
    // only queue up on a completed swap, and none happen in this v1 binary.
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}

/// Picks the default output config if it supports `f32` samples, else scans every supported
/// config range for one that does (preferring the device's own default sample rate when it falls
/// within a candidate range). Real audio devices overwhelmingly support f32 directly via cpal's
/// resampling-free path; this only matters on the minority that don't advertise it as default.
fn pick_output_config(device: &cpal::Device) -> Option<cpal::SupportedStreamConfig> {
    if let Ok(default) = device.default_output_config() {
        if default.sample_format() == cpal::SampleFormat::F32 {
            return Some(default);
        }
    }
    let default_rate = device.default_output_config().ok().map(|c| c.sample_rate());
    let mut configs: Vec<_> = device
        .supported_output_configs()
        .ok()?
        .filter(|c| c.sample_format() == cpal::SampleFormat::F32)
        .collect();
    configs.sort_by_key(|c| c.channels());
    let range = configs.into_iter().next()?;
    let rate = default_rate
        .filter(|&r| r >= range.min_sample_rate() && r <= range.max_sample_rate())
        .unwrap_or_else(|| range.max_sample_rate());
    Some(range.with_sample_rate(rate))
}

/// No audio device (or no MIDI, separately handled) — proves the synthesis path still works by
/// rendering a short chord progression straight through the compiler (bypassing `PatchEngine`/
/// `cpal` entirely) to a WAV file, rather than the binary just doing nothing observable.
fn render_self_test(patch: &kabl_core::PatchState) {
    let sample_rate = 48000.0;
    let mut compiled = match kabl_engine::compile::compile(patch, sample_rate, DEFAULT_VOICE_COUNT)
    {
        Ok(c) => c,
        Err(err) => {
            eprintln!("kabl: self-test patch failed to compile: {err}");
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
            eprintln!(
                "kabl: no audio device -- wrote a self-test render proving the synth path works: {}",
                path.display()
            );
        }
        Err(err) => eprintln!("kabl: couldn't write self-test render: {err}"),
    }
}

fn apply_note_on_direct(
    compiled: &mut kabl_engine::compile::CompiledPatch,
    voice: usize,
    semitones: f32,
    velocity: f32,
) {
    if let Some(m) = compiled.module_mut(MIDI_IN_ID, Some(voice)) {
        if let Some(midi) = m
            .as_any_mut()
            .downcast_mut::<kabl_modules::builtins::MidiIn>()
        {
            midi.note_on(semitones, velocity);
        }
    }
}

fn apply_note_off_direct(compiled: &mut kabl_engine::compile::CompiledPatch, voice: usize) {
    if let Some(m) = compiled.module_mut(MIDI_IN_ID, Some(voice)) {
        if let Some(midi) = m
            .as_any_mut()
            .downcast_mut::<kabl_modules::builtins::MidiIn>()
        {
            midi.note_off();
        }
    }
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
    kabl_core::save(dir, &log)
}
