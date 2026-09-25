//! The standalone binary's testable core: a default patch, a non-allocating ring buffer for
//! bridging `cpal`'s arbitrary callback buffer sizes against the engine's fixed `BLOCK`, and pure
//! MIDI-byte-to-voice-event resolution. Kept separate from `main.rs` because `main.rs` itself
//! needs a real audio device and MIDI port to do anything — everything that *can* be tested
//! without hardware lives here instead, so "no `/dev/snd` in this container" doesn't mean "no
//! tests for the standalone crate" (see `tests/` and the module docs below for what's proven).

pub mod applog;

/// Stream errors from the error callback to another thread (`stream_errors`). This bounds the
/// errors whose ownership is outstanding at once; every error is also counted by kind.
pub const STREAM_ERROR_QUEUE: usize = 16;

/// The `cpal::ErrorKind`s counted apart; any other (the enum is non-exhaustive) counts in one
/// more slot, "unknown".
pub const STREAM_ERROR_KINDS: [cpal::ErrorKind; 14] = {
    use cpal::ErrorKind::*;
    [
        DeviceBusy,
        DeviceChanged,
        DeviceNotAvailable,
        HostUnavailable,
        InvalidInput,
        PermissionDenied,
        RealtimeDenied,
        ResourceExhausted,
        StreamInvalidated,
        UnsupportedConfig,
        UnsupportedOperation,
        Xrun,
        BackendError,
        Other,
    ]
};

/// Written by the error callback with relaxed atomics: every handed-over error by kind, and how
/// many found the queue full.
#[derive(Default)]
pub struct StreamErrorCounts {
    pub by_kind: [std::sync::atomic::AtomicU64; STREAM_ERROR_KINDS.len() + 1],
    pub undelivered: std::sync::atomic::AtomicU64,
}

impl StreamErrorCounts {
    /// Off the audio thread: the counts since `seen` as "Kind=n …" (empty when none) and the
    /// undelivered count since then; updates `seen`.
    pub fn since(&self, seen: &mut [u64; STREAM_ERROR_KINDS.len() + 2]) -> (String, u64) {
        use std::sync::atomic::Ordering::Relaxed;
        let mut text = String::new();
        for (i, c) in self.by_kind.iter().enumerate() {
            let n = c.load(Relaxed);
            if n != seen[i] {
                let name = STREAM_ERROR_KINDS
                    .get(i)
                    .map_or("unknown".to_string(), |k| format!("{k:?}"));
                text += &format!(
                    "{}{name}={}",
                    if text.is_empty() { "" } else { " " },
                    n - seen[i]
                );
                seen[i] = n;
            }
        }
        let u = self.undelivered.load(Relaxed);
        let lost = u - seen[STREAM_ERROR_KINDS.len() + 1];
        seen[STREAM_ERROR_KINDS.len() + 1] = u;
        (text, lost)
    }
}

/// For a stream's error callback, which runs on the audio thread (cpal 0.18.2 ALSA calls it
/// from the stream's worker thread, between data callbacks; see signal-inspection design.md):
/// counts `err` by kind and moves it into `tx` for another thread to log and drop. Never
/// allocates, formats, logs, locks or blocks.
///
/// When the queue is full the error is dropped here and counted in `undelivered`. That frees
/// its message if the backend allocated one (on ALSA `BackendError` and `RealtimeDenied` do,
/// allocated by cpal on this thread just before the call); keeping it instead would hold
/// memory without bound while the consumer is paused. At most `STREAM_ERROR_QUEUE` errors are
/// outstanding. This is a proposed contract adjustment: see design.md, "Stream-error
/// ownership".
pub fn hand_off_stream_error(
    err: cpal::Error,
    tx: &mut rtrb::Producer<cpal::Error>,
    counts: &StreamErrorCounts,
) {
    use std::sync::atomic::Ordering::Relaxed;
    // A bare xrun owns nothing (callers count xruns themselves): dropping it here frees
    // nothing, and it cannot crowd a real device error out of the queue.
    if err.kind() == cpal::ErrorKind::Xrun && err.message().is_none() {
        return;
    }
    let i = STREAM_ERROR_KINDS
        .iter()
        .position(|k| *k == err.kind())
        .unwrap_or(STREAM_ERROR_KINDS.len());
    counts.by_kind[i].fetch_add(1, Relaxed);
    if tx.push(err).is_err() {
        counts.undelivered.fetch_add(1, Relaxed);
    }
}

use kabl_core::{CableState, ModuleId, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::patch_engine::PatchEngine;
use std::collections::BTreeMap;

/// How many notes can sound at once. Chosen as a reasonable default for a first-light polysynth,
/// not derived from anything — the brief doesn't mandate a number and nothing yet lets a person
/// change it at runtime.
pub const DEFAULT_VOICE_COUNT: usize = 8;

/// `osc.va`'s `base_hz` default is C4 (261.63 Hz) — MIDI note 60 is also C4, so a MIDI note
/// number converts to `pitch` semitones as `note - MIDI_NOTE_FOR_BASE_HZ`.
pub const MIDI_NOTE_FOR_BASE_HZ: i32 = 60;

/// The default patch a freshly-started standalone binary plays: one voice-rate chain
/// (`midi.in -> osc.va -> filter.svf -> env.adsr/vca -> out`), the same 5-stage shape
/// `patch_demo.rs`/`compile.rs`'s tests already proved correct — the compiler instances it once
/// per voice automatically (`Rate::Voice`), so this patch alone is already a full polysynth once
/// paired with a `VoiceAllocator` routing MIDI notes across `DEFAULT_VOICE_COUNT` instances.
pub fn default_patch() -> PatchState {
    let mut patch = PatchState::new();
    // Staggered left-to-right in signal-flow order, one per column -- not just cosmetic: a UI
    // that lays modules out by their stored `pos` (see `kabl-ui`) needs them to not all collide
    // at the same point, which an earlier all-(0,0) version of this patch did (caught visually
    // running `kabl-ui` under Xvfb -- see decisions.md "kabl-ui: the patchbay").
    patch
        .modules
        .insert(1, positioned_module("midi.in", &[], 40.0, 40.0));
    patch.modules.insert(
        2,
        positioned_module(
            "osc.va",
            &[("base_hz", 261.63), ("waveform", 2.0)],
            240.0,
            40.0,
        ),
    );
    patch.modules.insert(
        3,
        positioned_module(
            "filter.svf",
            &[("cutoff_hz", 3000.0), ("resonance", 0.2)],
            440.0,
            40.0,
        ),
    );
    patch.modules.insert(
        4,
        positioned_module(
            "env.adsr",
            &[
                ("attack_ms", 5.0),
                ("decay_ms", 120.0),
                ("sustain", 0.6),
                ("release_ms", 250.0),
            ],
            240.0,
            220.0,
        ),
    );
    patch.modules.insert(
        5,
        positioned_module("vca", &[("gain", 0.0), ("exponential", 0.0)], 640.0, 40.0),
    );
    patch
        .modules
        .insert(6, positioned_module("out", &[], 840.0, 40.0));

    patch.cables.insert(1, cable(1, "pitch", 2, "pitch"));
    patch.cables.insert(2, cable(2, "out", 3, "in"));
    patch.cables.insert(3, cable(1, "gate", 4, "gate"));
    patch.cables.insert(4, cable(3, "lp", 5, "in"));
    patch.cables.insert(5, cable(4, "out", 5, "cv"));
    patch.cables.insert(6, cable(5, "out", 6, "left"));
    patch.cables.insert(7, cable(5, "out", 6, "right"));
    patch
}

/// The `midi.in` module's id in `default_patch()`. MIDI routing does not depend on it (notes
/// reach every `midi.in`, see `CompiledPatch::note_on`); tests use it to inspect that module.
pub const MIDI_IN_ID: ModuleId = 1;

fn positioned_module(kind: &str, params: &[(&str, f32)], x: f32, y: f32) -> ModuleState {
    ModuleState {
        kind: kind.to_string(),
        pos: Vec2 { x, y },
        params: params.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
    }
}

fn cable(from_id: u64, from_port: &str, to_id: u64, to_port: &str) -> CableState {
    CableState {
        from: PortRef::Module {
            id: from_id,
            port: from_port.to_string(),
        },
        to: PortRef::Module {
            id: to_id,
            port: to_port.to_string(),
        },
        params: BTreeMap::new(),
        steps: Vec::new(),
    }
}

/// A voice-level event, already resolved (MIDI note number -> voice index, done on the control
/// thread by `resolve_midi_message`) — cheap enough to pass through an `rtrb` channel and apply
/// on the audio thread with no allocation (`apply_voice_event`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VoiceEvent {
    NoteOn {
        voice: usize,
        semitones: f32,
        velocity: f32,
    },
    NoteOff {
        voice: usize,
    },
}

/// Parses one raw MIDI message (as delivered by `midir`'s callback) into a `VoiceEvent`, using
/// `allocator` to turn the note number into a voice index. Runs on the MIDI thread (not the
/// audio thread) — `VoiceAllocator` uses a `HashMap` internally and is not RT-safe to call from
/// the audio callback, which is exactly why voice resolution happens here and only the resolved
/// `VoiceEvent` crosses to the audio thread.
///
/// Handles Note On (0x9n) and Note Off (0x8n); a Note On with velocity 0 is the standard MIDI
/// convention for Note Off (lets a device use running status without an explicit 0x8n). Anything
/// else (CC, pitch bend, sysex, ...) returns `None` — real, just not wired to anything yet.
pub fn resolve_midi_message(
    allocator: &mut kabl_engine::voice_allocator::VoiceAllocator,
    data: &[u8],
) -> Option<VoiceEvent> {
    if data.len() < 3 {
        return None;
    }
    let status = data[0] & 0xF0;
    let note = data[1] as u32;
    match status {
        0x90 if data[2] > 0 => {
            let voice = allocator.note_on(note);
            let semitones = note as f32 - MIDI_NOTE_FOR_BASE_HZ as f32;
            let velocity = data[2] as f32 / 127.0;
            Some(VoiceEvent::NoteOn {
                voice,
                semitones,
                velocity,
            })
        }
        0x90 | 0x80 => allocator
            .note_off(note)
            .map(|voice| VoiceEvent::NoteOff { voice }),
        _ => None,
    }
}

/// Applies a resolved `VoiceEvent` to every graph `engine` is running (active and, during a
/// crossfade, incoming), reaching every `midi.in` instance in the patch — the audio-thread half
/// of MIDI handling. No allocation.
pub fn apply_voice_event(engine: &mut PatchEngine, event: VoiceEvent) {
    match event {
        VoiceEvent::NoteOn {
            voice,
            semitones,
            velocity,
        } => engine.note_on(voice, semitones, velocity),
        VoiceEvent::NoteOff { voice } => engine.note_off(voice),
    }
}

/// A fixed-capacity, non-allocating single-producer/single-consumer sample queue — bridges
/// `PatchEngine::process_block`'s fixed `BLOCK`-sized output against `cpal`'s callback, which can
/// ask for any number of frames per call (varies by host/device/buffer-size setting, not a
/// multiple of `BLOCK` in general). `push`/`pop` never allocate once constructed — the backing
/// `Vec` is sized once at construction (control thread) and never grows after, so this is safe to
/// drive entirely from the audio thread.
pub struct RingBuffer {
    buf: Vec<f32>,
    read: usize,
    write: usize,
    len: usize,
}

impl RingBuffer {
    pub fn new(capacity: usize) -> Self {
        RingBuffer {
            buf: vec![0.0; capacity],
            read: 0,
            write: 0,
            len: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.buf.len()
    }

    pub fn available(&self) -> usize {
        self.len
    }

    pub fn free(&self) -> usize {
        self.buf.len() - self.len
    }

    /// Pushes `data`. Panics if `data.len() > self.free()` — a genuine logic error (the caller
    /// should always keep the buffer topped up before it runs dry, never blindly overfill it),
    /// not a runtime condition to silently truncate.
    #[inline]
    pub fn push_slice(&mut self, data: &[f32]) {
        assert!(
            data.len() <= self.free(),
            "RingBuffer overflow: pushing {} samples with only {} free",
            data.len(),
            self.free()
        );
        let cap = self.buf.len();
        for &s in data {
            self.buf[self.write] = s;
            self.write = (self.write + 1) % cap;
        }
        self.len += data.len();
    }

    #[inline]
    pub fn pop(&mut self) -> Option<f32> {
        if self.len == 0 {
            return None;
        }
        let cap = self.buf.len();
        let v = self.buf[self.read];
        self.read = (self.read + 1) % cap;
        self.len -= 1;
        Some(v)
    }
}

/// Connects to a MIDI input port (see `name_filter` handling below), if any. Voice allocation happens here, on
/// the MIDI callback thread (not the audio thread) -- see `resolve_midi_message`'s doc comment
/// for why (`VoiceAllocator` isn't RT-safe to call from the audio callback).
pub fn connect_midi(
    client_name: &str,
    mut producer: rtrb::Producer<VoiceEvent>,
    name_filter: Option<String>,
) -> Option<midir::MidiInputConnection<()>> {
    let midi_in = match midir::MidiInput::new(client_name) {
        Ok(m) => m,
        Err(err) => {
            log::warn!(target: "midi", "MIDI input unavailable on this system: {err}");
            return None;
        }
    };
    let ports = midi_in.ports();
    if ports.is_empty() {
        log::warn!(target: "midi", "no MIDI input ports found -- connect a controller and restart to play.");
        return None;
    }
    // Default (no --midi): first port whose name doesn't look like ALSA's own virtual "Midi
    // Through" loopback, since that's never the instrument a person actually wants to play --
    // falls back to the first port if every port looks like that (still better than silently
    // connecting to nothing).
    let port = match &name_filter {
        Some(filter) => ports.iter().find(|p| {
            midi_in
                .port_name(p)
                .is_ok_and(|n| n.to_lowercase().contains(&filter.to_lowercase()))
        }),
        None => ports
            .iter()
            .find(|p| midi_in.port_name(p).is_ok_and(|n| !n.contains("Through"))),
    }
    .or(ports.first());
    let Some(port) = port else {
        log::warn!(target: "midi", "no MIDI port matched --midi {name_filter:?}");
        return None;
    };
    let port_name = midi_in
        .port_name(port)
        .unwrap_or_else(|_| "unknown".to_string());

    let mut allocator = kabl_engine::voice_allocator::VoiceAllocator::new(DEFAULT_VOICE_COUNT);
    let connection = midi_in.connect(
        port,
        "kabl-input",
        move |_stamp_us, data, ()| {
            if let Some(event) = resolve_midi_message(&mut allocator, data) {
                // A full queue means events are arriving faster than the audio thread drains
                // them (shouldn't happen at 256 slots' depth for note on/off traffic) -- drop
                // rather than block, since blocking the MIDI thread is harmless but blocking
                // would be the wrong failure mode to invite here.
                let _ = producer.push(event);
            }
        },
        (),
    );
    match connection {
        Ok(conn) => {
            log::info!(target: "midi", "listening for MIDI on \"{port_name}\"");
            Some(conn)
        }
        Err(err) => {
            log::warn!(target: "midi", "failed to connect to MIDI port \"{port_name}\": {err}");
            None
        }
    }
}
