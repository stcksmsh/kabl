//! The audio output: device and config choice, the queues to and from the callback.

use super::*;
use basedrop::Collector;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use kabl_core::{ModuleId, PatchState};
use kabl_engine::timeline::Timeline;

use kabl_engine::facetap::FaceReport;
use kabl_engine::patch_engine::PatchEngine;
use kabl_engine::probe::ProbeReport;
use kabl_engine::runtime::{Feedback, ToAudio};
use kabl_modules::builtins::{DelayLock, LfoSync};
use kabl_standalone::{RingBuffer, DEFAULT_VOICE_COUNT};
use kabl_ui::control::{self, Delivery};
use kabl_ui::record;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Arc;

/// (sequencer, step, playing bank, queued bank).
pub(crate) type SeqReport = (ModuleId, usize, usize, Option<usize>);
/// Macro module, output, value, target, progress of a macro a cue drives.
pub(crate) type MorphReport = (ModuleId, u8, f32, f32, f32);

/// Owns everything audio-related: the swap queue's producer, the stream, the MIDI connection,
/// and the `basedrop` collector that frees retired graphs. Lives inside `App`.
pub(crate) struct AudioHost {
    /// Dropped first (fields drop in declaration order): the audio thread stops before the
    /// queues it feeds are torn down.
    pub(crate) _stream: Option<cpal::Stream>,
    /// The `--input` device, if asked for and open; dropped with the output stream.
    pub(crate) _input: Option<cpal::Stream>,
    /// The input device vanished while playing (its error callback fired).
    pub(crate) input_lost: Option<Arc<AtomicBool>>,
    pub(crate) gate: Arc<AtomicU64>,
    pub(crate) sample_rate: f32,
    pub(crate) settings: String,
    pub(crate) collector: Collector,
    pub(crate) status: String,
    /// Each sequencer's playing step, playing bank and queued bank, published by the audio
    /// callback.
    pub(crate) steps_rx: Option<rtrb::Consumer<SeqReport>>,
    /// Each clock's run state, published by the audio callback.
    pub(crate) clocks_rx: Option<rtrb::Consumer<(ModuleId, bool, f64)>>,
    /// Each LFO's sync state, published by the audio callback.
    pub(crate) lfos_rx: Option<rtrb::Consumer<(ModuleId, LfoSync)>>,
    pub(crate) morphs_rx: Option<rtrb::Consumer<MorphReport>>,
    /// Each delay's lock state and target time, published by the audio callback.
    pub(crate) delays_rx: Option<rtrb::Consumer<(ModuleId, DelayLock, f32)>>,
    pub(crate) recorder: Option<record::Recorder>,
    pub(crate) timing: Option<Arc<CallbackTiming>>,
    /// Selected-signal measurements from the audio callback (D03).
    pub(crate) probe_rx: Option<rtrb::Consumer<ProbeReport>>,
    pub(crate) face_rx: Option<rtrb::Consumer<FaceReport>>,
    /// Stream errors except bare xruns, moved out of the error callback unformatted (it runs
    /// on the audio thread); logged here. Declared after `_stream` so the stream (and the
    /// producer in its error callback) goes first on teardown: queued errors are then freed
    /// here, with this consumer, not on the exiting audio thread.
    pub(crate) faults_rx: Option<rtrb::Consumer<cpal::Error>>,
    /// Revisions the callback applied and when (`KABL_LATENCY_FILE` only).
    pub(crate) applied_rx: Option<rtrb::Consumer<(u64, f64)>>,
    /// The Sounds scene's preview player, handed to the UI state once (`None` when offline).
    pub(crate) preview: Option<kabl_ui::sound_preview::Player>,
}

#[derive(Clone)]
pub(crate) struct AudioRequest {
    pub(crate) rate: Option<u32>,
    pub(crate) frames: Option<u32>,
    pub(crate) realtime: bool,
    /// Exact enumerated output, with its name checked again before opening.
    pub(crate) device: Option<(usize, String)>,
    /// Open the default audio input for `audio.in` modules (`--input`).
    pub(crate) input: bool,
}

pub(crate) fn output_names() -> Vec<(usize, String)> {
    cpal::default_host()
        .output_devices()
        .map(|ds| {
            ds.enumerate()
                .filter_map(|(i, d)| d.description().ok().map(|x| (i, x.name().to_string())))
                .collect()
        })
        .unwrap_or_default()
}

impl AudioHost {
    pub(crate) fn offline(collector: Collector, status: String) -> Self {
        AudioHost {
            gate: Arc::new(AtomicU64::new(0)),
            sample_rate: 48000.0,
            settings: String::new(),
            collector,
            status,
            steps_rx: None,
            clocks_rx: None,
            delays_rx: None,
            lfos_rx: None,
            morphs_rx: None,
            recorder: None,
            timing: None,
            probe_rx: None,
            face_rx: None,
            faults_rx: None,
            applied_rx: None,
            preview: None,
            _stream: None,
            _input: None,
            input_lost: None,
        }
    }

    /// The control side for an offline host: nothing is sent.
    pub(crate) fn offline_delivery(&self, patch: &PatchState) -> Delivery {
        Delivery::new(
            None,
            self.collector.handle(),
            Arc::new(Feedback::default()),
            self.sample_rate,
            DEFAULT_VOICE_COUNT,
            Some(patch),
        )
    }

    /// `rate`/`frames`: a sample rate and a fixed callback size to ask the device for.
    /// Returns the host and the control side of its queues.
    pub(crate) fn start(
        patch: &PatchState,
        notes: rtrb::Consumer<Note>,
        keys: Arc<AtomicU64>,
        request: &AudioRequest,
        restarting: bool,
        peaks: Arc<record::PeakTap>,
    ) -> (Self, Delivery) {
        let (rate, frames, realtime) = (request.rate, request.frames, request.realtime);
        let midi_consumer = notes;
        let collector = Collector::new();
        let handle = collector.handle();

        let host = cpal::default_host();
        log::info!(target: "audio", "backend={:?} requested rate={rate:?} frames={frames:?} rt={realtime}", host.id());
        let device = match &request.device {
            Some((index, name)) => host
                .output_devices()
                .ok()
                .and_then(|mut ds| ds.nth(*index))
                .filter(|d| d.description().is_ok_and(|desc| desc.name() == name)),
            None => host.default_output_device(),
        };
        let Some(device) = device else {
            log::warn!(target: "audio", "no output device: editing only, no playback");
            let a = Self::offline(
                collector,
                match &request.device {
                    Some((_, name)) => {
                        format!("output \"{name}\" unavailable; choose another output or Retry")
                    }
                    None => "no default audio output; choose an output or Retry".into(),
                },
            );
            let d = a.offline_delivery(patch);
            return (a, d);
        };
        // Stereo first: the first matching config can be a mono one, which would fold the mix.
        let f32_at = |r: u32| {
            device
                .supported_output_configs()
                .ok()?
                .filter(|c| c.sample_format() == cpal::SampleFormat::F32)
                .filter_map(|c| c.try_with_sample_rate(r))
                .max_by_key(|c| (c.channels() == 2, c.channels()))
        };
        let config = match rate {
            Some(r) => f32_at(r),
            None => device
                .default_output_config()
                .ok()
                .filter(|c| c.sample_format() == cpal::SampleFormat::F32),
        };
        let Some(config) = config else {
            log::warn!(target: "audio", "no usable f32 output config (rate {rate:?}): no playback");
            let a = Self::offline(
                collector,
                format!(
                    "no usable f32 audio output config{} -- editing works, playback won't",
                    rate.map_or(String::new(), |r| format!(" at {r} Hz"))
                ),
            );
            let d = a.offline_delivery(patch);
            return (a, d);
        };
        let mut stream_config = config.config();
        if let Some(n) = frames {
            stream_config.buffer_size = cpal::BufferSize::Fixed(n);
        }

        let sample_rate = config.sample_rate() as f32;
        // Startup and every device change: code-module budgets follow the new period and machine.
        kabl_modules::code::host::device_changed(sample_rate, kabl_engine::graph::BLOCK as u32);
        let channels = config.channels() as usize;
        let timing = Arc::new(CallbackTiming::default());
        let t_err = timing.clone();
        let fault = if restarting {
            None
        } else {
            std::env::var("KABL_AUDIO_FAULT").ok()
        };
        let fault_after = std::env::var("KABL_AUDIO_FAULT_AFTER")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(30);

        let mut engine = match PatchEngine::new(&handle, patch, sample_rate, DEFAULT_VOICE_COUNT) {
            Ok(e) => e,
            Err(err) => {
                log::error!(target: "audio", "startup patch failed to compile: {err}");
                let a = Self::offline(collector, format!("patch failed to compile: {err}"));
                // Nothing plays: the next edit compiles from scratch.
                let mut d = a.offline_delivery(patch);
                d.compile_error = Some(err.to_string());
                return (a, d);
            }
        };
        for n in kabl_modules::code::host::active_scope().take_notes() {
            log::warn!(target: "app", "{n}");
        }
        engine.active_mut().generation = 1;
        engine.active_mut().rev = 1;
        let (probe_tx, probe_rx) = rtrb::RingBuffer::<ProbeReport>::new(8);
        let (face_tx, face_rx) = rtrb::RingBuffer::<FaceReport>::new(4);
        let (mut faults_tx, faults_rx) =
            rtrb::RingBuffer::<cpal::Error>::new(kabl_standalone::STREAM_ERROR_QUEUE);

        let (control_tx, control_rx) = rtrb::RingBuffer::<ToAudio>::new(control::QUEUE);
        // Sound previews: a clip the UI rendered, mixed on top of the rack below.
        let (preview_player, preview_voice) = kabl_ui::sound_preview::channel(collector.handle());
        let feedback = Arc::new(Feedback::default());
        let gate = Arc::new(AtomicU64::new(if restarting { 0 } else { u64::MAX }));
        // Measurement only (`KABL_LATENCY_FILE`): the callback reports each newly applied
        // revision with its time.
        let (applied_tx, applied_rx) = rtrb::RingBuffer::<(u64, f64)>::new(1024);
        let measure = std::env::var_os("KABL_LATENCY_FILE").is_some();
        // Test hooks: a small ring forces lost frames, a failure point forces a write error.
        let ring_frames = std::env::var("KABL_RECORD_RING_FRAMES")
            .ok()
            .and_then(|v| v.parse().ok());
        let (mut recorder, tap) = match ring_frames {
            Some(n) => record::pair_sized(sample_rate as u32, n),
            None => record::pair(sample_rate as u32),
        };
        recorder.fail_after = std::env::var("KABL_RECORD_FAIL_AFTER")
            .ok()
            .and_then(|v| v.parse().ok());

        let (steps_tx, steps_rx) = rtrb::RingBuffer::<SeqReport>::new(256);
        let (clocks_tx, clocks_rx) = rtrb::RingBuffer::<(ModuleId, bool, f64)>::new(64);
        let (delays_tx, delays_rx) = rtrb::RingBuffer::<(ModuleId, DelayLock, f32)>::new(64);
        let (lfos_tx, lfos_rx) = rtrb::RingBuffer::<(ModuleId, LfoSync)>::new(128);
        let (morphs_tx, morphs_rx) = rtrb::RingBuffer::<MorphReport>::new(64);

        let mut input = if request.input {
            kabl_standalone::input::AudioInput::open(sample_rate as u32)
        } else {
            kabl_standalone::input::AudioInput::closed(String::new())
        };
        let input_stream = input.stream.take();
        let input_lost = input.lost.clone();
        let input_status = input.status.clone();
        let input_rx = input.rx;
        let take_input = request.input;

        let mut callback = OutputCallback {
            engine,
            control_rx,
            fb: feedback.clone(),
            preview_voice,
            measure,
            applied_tx,
            last_applied: 0,
            midi_consumer,
            last_start: None,
            t: timing.clone(),
            timeline: Box::new(Timeline::new()),
            left_ring: RingBuffer::new(RING_CAPACITY),
            right_ring: RingBuffer::new(RING_CAPACITY),
            take_input,
            input_rx,
            steps_tx,
            clocks_tx,
            delays_tx,
            lfos_tx,
            morphs_tx,
            keys,
            probe_tx,
            face_tx,
            audio_gate: gate.clone(),
            ready_for: 0,
            ready_frames: 0,
            tap,
            peaks,
            fault,
            fault_after,
            fault_sent: false,
            sample_rate,
            channels,
            rt_tried: !realtime,
            _rt_handle: None,
            #[cfg(test)]
            warmups: 0,
        };
        let stream = device.build_output_stream(
            stream_config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| callback.process(data),
            move |err| {
                // On the audio thread: count, and hand the error over unformatted to the UI
                // thread, which logs and drops it (`hand_off_stream_error`).
                t_err.error(&err);
                kabl_standalone::hand_off_stream_error(err, &mut faults_tx, &t_err.errors);
            },
            None,
        );

        log::info!(
            target: "audio",
            "device={:?} rate={sample_rate} channels={channels} buffer={:?}",
            device.description().map(|d| d.name().to_string()).ok(),
            stream_config.buffer_size
        );
        let (stream, status) = match stream {
            Ok(s) => match s.play() {
                Ok(()) => (
                    Some(s),
                    format!(
                        "playing {sample_rate} Hz, {channels} ch{}",
                        frames.map_or(String::new(), |n| format!(", {n} frames asked"))
                    ),
                ),
                Err(err) => (None, format!("failed to start audio stream: {err}")),
            },
            Err(err) => (None, format!("failed to build audio stream: {err}")),
        };
        let delivery = Delivery::new(
            stream.is_some().then_some(control_tx),
            collector.handle(),
            feedback,
            sample_rate,
            DEFAULT_VOICE_COUNT,
            Some(patch),
        );
        let host = AudioHost {
            gate,
            sample_rate,
            settings: format!(
                "{} · {sample_rate} Hz · {channels} channels · requested buffer {:?}{}",
                device
                    .description()
                    .map_or("unknown output".into(), |d| d.name().to_string()),
                stream_config.buffer_size,
                if take_input {
                    format!(" · {input_status}")
                } else {
                    String::new()
                }
            ),
            collector,
            status,
            steps_rx: Some(steps_rx),
            clocks_rx: Some(clocks_rx),
            delays_rx: Some(delays_rx),
            lfos_rx: Some(lfos_rx),
            morphs_rx: Some(morphs_rx),
            recorder: stream.is_some().then_some(recorder),
            timing: stream.is_some().then_some(timing),
            probe_rx: stream.is_some().then_some(probe_rx),
            face_rx: stream.is_some().then_some(face_rx),
            faults_rx: stream.is_some().then_some(faults_rx),
            applied_rx: (stream.is_some() && measure).then_some(applied_rx),
            preview: stream.is_some().then_some(preview_player),
            _stream: stream,
            _input: input_stream,
            input_lost: take_input.then_some(input_lost),
        };
        (host, delivery)
    }
}
