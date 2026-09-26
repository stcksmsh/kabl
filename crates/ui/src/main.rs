//! `kabl-ui` binary: the patchbay editor with live audio. Combines `PatchEditor` (patch editing
//! over the real op log) with the same `cpal`/`midir` wiring `kabl-standalone` uses, so editing a
//! patch here and hearing the result are the same window, not a separate player process.
//!
//! **Audio handoff is lock-free.** The audio callback owns the `PatchEngine` outright. Document
//! changes go through `kabl_ui::control::Delivery`: runtime values when only knob values or
//! route amounts changed, else a compiled graph wrapped for deferred drop (`basedrop::Owned`),
//! both through one bounded `rtrb` queue in revision order. The callback takes a bounded number
//! of messages per call (`PatchEngine::drain`); state carry and value changes are
//! allocation-free. No mutex on the audio thread. Retired graphs are freed on the UI thread by
//! `Collector::collect()` every frame.
//!
//! **Control without the editor (D04).** The document, the UI state and the delivery live in
//! one `Core` behind a mutex that only non-audio threads take: the UI for a frame, and the
//! control thread (`pump`) for MIDI CCs and retries. MIDI mappings, pickup and buttons run
//! there, so they reach the audio whether or not a frame is drawn.

use basedrop::Collector;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use kabl_core::{ModuleId, PatchState};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::{Command, PatchEngine};
use kabl_engine::probe::ProbeReport;
use kabl_engine::runtime::{Feedback, ToAudio};
use kabl_modules::builtins::{DelayLock, LfoSync, Transport};
use kabl_standalone::{default_patch, RingBuffer, DEFAULT_VOICE_COUNT};
use kabl_ui::control::{self, Delivery};
use kabl_ui::{record, show, PatchEditor, UiState};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const RING_CAPACITY: usize = BLOCK * 256;

/// (sequencer, step, playing bank, queued bank).
type SeqReport = (ModuleId, usize, usize, Option<usize>);

/// Owns everything audio-related: the swap queue's producer, the stream, the MIDI connection,
/// and the `basedrop` collector that frees retired graphs. Lives inside `App`.
struct AudioHost {
    /// Dropped first (fields drop in declaration order): the audio thread stops before the
    /// queues it feeds are torn down.
    _stream: Option<cpal::Stream>,
    sample_rate: f32,
    collector: Collector,
    status: String,
    /// Each sequencer's playing step, playing bank and queued bank, published by the audio
    /// callback.
    steps_rx: Option<rtrb::Consumer<SeqReport>>,
    /// Each clock's run state, published by the audio callback.
    clocks_rx: Option<rtrb::Consumer<(ModuleId, bool)>>,
    /// Each LFO's sync state, published by the audio callback.
    lfos_rx: Option<rtrb::Consumer<(ModuleId, LfoSync)>>,
    /// Each delay's lock state and target time, published by the audio callback.
    delays_rx: Option<rtrb::Consumer<(ModuleId, DelayLock, f32)>>,
    recorder: Option<record::Recorder>,
    timing: Option<Arc<CallbackTiming>>,
    /// Selected-signal measurements from the audio callback (D03).
    probe_rx: Option<rtrb::Consumer<ProbeReport>>,
    /// Stream errors except bare xruns, moved out of the error callback unformatted (it runs
    /// on the audio thread); logged here. Declared after `_stream` so the stream (and the
    /// producer in its error callback) goes first on teardown: queued errors are then freed
    /// here, with this consumer, not on the exiting audio thread.
    faults_rx: Option<rtrb::Consumer<cpal::Error>>,
}

/// Audio-callback telemetry, written by the callback (and the stream's error callback) with
/// relaxed atomics, read by the status bar and `KABL_STATS_FILE`. Kept apart:
/// - execution: how long the callback ran, against the audio it produced (its budget);
/// - arrival: the interval between callback starts, against the same period;
/// - xruns the backend reported (`cpal::ErrorKind::Xrun`);
/// - the frames each callback actually asked for.
///
/// The first second (stream start-up) is counted apart.
#[derive(Default)]
struct CallbackTiming {
    count: AtomicU64,
    worst_ns: AtomicU64,
    /// Seconds into the stream of the worst execution (to line it up with what happened).
    worst_at_ms: AtomicU64,
    /// Wall-clock time of the worst execution, ms since the Unix epoch.
    worst_wall_ms: AtomicU64,
    /// Execution longer than the callback's audio (after the first second).
    late: AtomicU64,
    /// Execution over half of it.
    over_half: AtomicU64,
    startup_worst_ns: AtomicU64,
    /// Longest interval between callback starts, and intervals over 1.5 × the period.
    arrival_worst_ns: AtomicU64,
    arrival_late: AtomicU64,
    frames_min: AtomicU64,
    frames_max: AtomicU64,
    xruns: AtomicU64,
    other_errors: AtomicU64,
    /// Stream errors the error callback handed over, by kind, and those that found the queue
    /// full (dropped there).
    errors: kabl_standalone::StreamErrorCounts,
    /// Frames delivered to the device: the audio clock probe reports are aged against.
    frames_total: AtomicU64,
    /// Execution times after the first second, in `HIST_BIN_US` bins (the last one open).
    hist: Hist,
    /// 0 not tried, 1 granted, 2 refused.
    rt: std::sync::atomic::AtomicU8,
    rt_error: std::sync::OnceLock<String>,
}

struct Hist([AtomicU64; HIST_BINS]);

impl Default for Hist {
    fn default() -> Self {
        Hist(std::array::from_fn(|_| AtomicU64::new(0)))
    }
}

/// Execution histogram: 64 bins of 50 µs (the last collects everything above 3.15 ms).
const HIST_BINS: usize = 64;
const HIST_BIN_US: u64 = 50;

impl CallbackTiming {
    /// `p` quantile (0..1) of the execution histogram, as the bin's upper edge in µs.
    fn quantile(&self, p: f64) -> Option<u64> {
        let counts: Vec<u64> = self
            .hist
            .0
            .iter()
            .map(|b| b.load(Ordering::Relaxed))
            .collect();
        let total: u64 = counts.iter().sum();
        if total == 0 {
            return None;
        }
        let want = (total as f64 * p).ceil() as u64;
        let mut seen = 0;
        for (i, c) in counts.iter().enumerate() {
            seen += c;
            if seen >= want {
                return Some((i as u64 + 1) * HIST_BIN_US);
            }
        }
        None
    }

    /// The histogram for `KABL_STATS_FILE`: nonzero bins as `upper_µs:count`.
    fn hist_line(&self) -> String {
        let open = HIST_BINS as u64 * HIST_BIN_US;
        let q = |p| {
            self.quantile(p).map_or("-".into(), |v| {
                if v == open {
                    format!(">{}", open - HIST_BIN_US)
                } else {
                    format!("≤{v}")
                }
            })
        };
        let bins: Vec<String> = self
            .hist
            .0
            .iter()
            .enumerate()
            .filter_map(|(i, b)| {
                let n = b.load(Ordering::Relaxed);
                (n > 0).then(|| {
                    if i + 1 == HIST_BINS {
                        format!(">{}:{n}", i as u64 * HIST_BIN_US)
                    } else {
                        format!("{}:{n}", (i as u64 + 1) * HIST_BIN_US)
                    }
                })
            })
            .collect();
        format!(
            "run histogram ({HIST_BIN_US} µs bins, after the first second): p50 {} p99 {} p99.9 {} µs · {}",
            q(0.5),
            q(0.99),
            q(0.999),
            bins.join(" ")
        )
    }

    fn record(
        &self,
        took: std::time::Duration,
        since_last: Option<std::time::Duration>,
        frames: usize,
        sample_rate: f32,
    ) {
        let ns = took.as_nanos() as u64;
        self.frames_total
            .fetch_add(frames as u64, Ordering::Relaxed);
        let n = self.count.fetch_add(1, Ordering::Relaxed);
        if n == 0 {
            self.frames_min.store(u64::MAX, Ordering::Relaxed);
        }
        self.frames_min.fetch_min(frames as u64, Ordering::Relaxed);
        self.frames_max.fetch_max(frames as u64, Ordering::Relaxed);
        if (n * frames as u64) < sample_rate as u64 {
            self.startup_worst_ns.fetch_max(ns, Ordering::Relaxed);
            return;
        }
        let budget = frames as f64 / sample_rate as f64 * 1e9;
        let bin = ((ns / 1000 / HIST_BIN_US) as usize).min(HIST_BINS - 1);
        self.hist.0[bin].fetch_add(1, Ordering::Relaxed);
        if self.worst_ns.fetch_max(ns, Ordering::Relaxed) < ns {
            let ms = n as f64 * frames as f64 / sample_rate as f64 * 1e3;
            self.worst_at_ms.store(ms as u64, Ordering::Relaxed);
            let wall = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64);
            self.worst_wall_ms.store(wall, Ordering::Relaxed);
        }
        if ns as f64 > budget {
            self.late.fetch_add(1, Ordering::Relaxed);
        }
        if ns as f64 > budget / 2.0 {
            self.over_half.fetch_add(1, Ordering::Relaxed);
        }
        if let Some(gap) = since_last {
            let g = gap.as_nanos() as u64;
            self.arrival_worst_ns.fetch_max(g, Ordering::Relaxed);
            if g as f64 > 1.5 * budget {
                self.arrival_late.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn error(&self, err: &cpal::Error) {
        if err.kind() == cpal::ErrorKind::Xrun {
            self.xruns.fetch_add(1, Ordering::Relaxed);
        } else {
            self.other_errors.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// A short status-bar line and the full report.
    fn summary(&self, sample_rate: f32) -> (String, String) {
        let get = |a: &AtomicU64| a.load(Ordering::Relaxed);
        let short = format!(
            "callback worst {:.0}/{:.0} µs · {} late · {} xruns · RT {}",
            get(&self.worst_ns) as f64 / 1e3,
            get(&self.frames_max) as f64 / sample_rate as f64 * 1e6,
            get(&self.late) + get(&self.arrival_late),
            get(&self.xruns),
            match self.rt.load(Ordering::Relaxed) {
                1 => "on",
                2 => "refused",
                _ => "off",
            }
        );
        (short, self.line(sample_rate))
    }

    fn line(&self, sample_rate: f32) -> String {
        let get = |a: &AtomicU64| a.load(Ordering::Relaxed);
        let us = |a: &AtomicU64| get(a) as f64 / 1e3;
        let (fmin, fmax) = (get(&self.frames_min), get(&self.frames_max));
        let frames = if fmin == fmax {
            format!("{fmax}")
        } else {
            format!("{fmin}–{fmax}")
        };
        let rt = match self.rt.load(Ordering::Relaxed) {
            1 => "RT priority on".to_string(),
            2 => format!(
                "RT priority refused ({})",
                self.rt_error.get().map_or("?", String::as_str)
            ),
            _ => "RT priority not tried".to_string(),
        };
        format!(
            "{} callbacks × {frames} frames at {sample_rate} Hz · run: worst {:.0} of {:.0} µs \
             (at {:.1} s, wall {}), {} over half, {} late · arrival: worst {:.0} µs, {} late · {} xruns · {rt} \
             (first second: worst {:.0} µs)",
            get(&self.count),
            us(&self.worst_ns),
            fmax as f64 / sample_rate as f64 * 1e6,
            get(&self.worst_at_ms) as f64 / 1e3,
            get(&self.worst_wall_ms),
            get(&self.over_half),
            get(&self.late),
            us(&self.arrival_worst_ns),
            get(&self.arrival_late),
            get(&self.xruns),
            us(&self.startup_worst_ns),
        )
    }
}

impl AudioHost {
    fn offline(collector: Collector, status: String) -> Self {
        AudioHost {
            sample_rate: 48000.0,
            collector,
            status,
            steps_rx: None,
            clocks_rx: None,
            delays_rx: None,
            lfos_rx: None,
            recorder: None,
            timing: None,
            probe_rx: None,
            faults_rx: None,
            _stream: None,
        }
    }

    /// The control side for an offline host: nothing is sent.
    fn offline_delivery(&self, patch: &PatchState) -> Delivery {
        Delivery::new(
            None,
            None,
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
    fn start(
        patch: &PatchState,
        notes: rtrb::Consumer<KeyEvent>,
        keys: Arc<AtomicU64>,
        rate: Option<u32>,
        frames: Option<u32>,
        realtime: bool,
        peaks: Arc<record::PeakTap>,
    ) -> (Self, Delivery) {
        let mut midi_consumer = notes;
        let collector = Collector::new();
        let handle = collector.handle();

        let host = cpal::default_host();
        log::info!(target: "audio", "backend={:?} requested rate={rate:?} frames={frames:?} rt={realtime}", host.id());
        let Some(device) = host.default_output_device() else {
            log::warn!(target: "audio", "no output device: editing only, no playback");
            let a = Self::offline(
                collector,
                "no audio output device found -- editing works, playback won't".into(),
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
        let channels = config.channels() as usize;
        let timing = Arc::new(CallbackTiming::default());
        let t = timing.clone();
        let t_err = timing.clone();
        let mut last_start: Option<std::time::Instant> = None;
        let mut rt_handle = None;
        let mut rt_tried = !realtime;

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
        engine.active_mut().generation = 1;
        engine.active_mut().rev = 1;
        let (mut probe_tx, probe_rx) = rtrb::RingBuffer::<ProbeReport>::new(8);
        let (mut faults_tx, faults_rx) =
            rtrb::RingBuffer::<cpal::Error>::new(kabl_standalone::STREAM_ERROR_QUEUE);

        let (control_tx, mut control_rx) = rtrb::RingBuffer::<ToAudio>::new(control::QUEUE);
        let feedback = Arc::new(Feedback::default());
        let fb = feedback.clone();
        // Test hooks: a small ring forces lost frames, a failure point forces a write error.
        let ring_frames = std::env::var("KABL_RECORD_RING_FRAMES")
            .ok()
            .and_then(|v| v.parse().ok());
        let (mut recorder, mut tap) = match ring_frames {
            Some(n) => record::pair_sized(sample_rate as u32, n),
            None => record::pair(sample_rate as u32),
        };
        recorder.fail_after = std::env::var("KABL_RECORD_FAIL_AFTER")
            .ok()
            .and_then(|v| v.parse().ok());

        let (mut steps_tx, steps_rx) = rtrb::RingBuffer::<SeqReport>::new(256);
        let (mut clocks_tx, clocks_rx) = rtrb::RingBuffer::<(ModuleId, bool)>::new(64);
        let (mut delays_tx, delays_rx) = rtrb::RingBuffer::<(ModuleId, DelayLock, f32)>::new(64);
        let (mut lfos_tx, lfos_rx) = rtrb::RingBuffer::<(ModuleId, LfoSync)>::new(128);
        let (transport_tx, mut transport_rx) = rtrb::RingBuffer::<(ModuleId, Transport)>::new(64);
        let (command_tx, mut command_rx) = rtrb::RingBuffer::<Command>::new(64);

        let mut left_ring = RingBuffer::new(RING_CAPACITY);
        let mut right_ring = RingBuffer::new(RING_CAPACITY);

        let stream = device.build_output_stream(
            stream_config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                // Once, on the first callback: ask for real-time priority for this thread (rtkit
                // over D-Bus, else the rlimit). It may allocate and block briefly; the stream
                // is starting and silent. Refusal leaves the thread as it was.
                if !rt_tried {
                    rt_tried = true;
                    let frames = (data.len() / channels) as u32;
                    match audio_thread_priority::promote_current_thread_to_real_time(
                        frames,
                        sample_rate as u32,
                    ) {
                        Ok(h) => {
                            rt_handle = Some(h);
                            t.rt.store(1, Ordering::Relaxed);
                        }
                        Err(e) => {
                            let _ = t.rt_error.set(e.to_string());
                            t.rt.store(2, Ordering::Relaxed);
                        }
                    }
                }
                let _ = &rt_handle;
                let started = std::time::Instant::now();
                let since_last = last_start.map(|l| started - l);
                last_start = Some(started);
                // Audio thread. No allocation, no locks: install queued graphs and runtime
                // values in order (state carry and values are allocation-free, at most one
                // queue's worth per callback), apply MIDI to every running graph, render.
                engine.drain(&mut control_rx, control::QUEUE, &fb);
                while let Ok(event) = midi_consumer.pop() {
                    engine.key(event);
                }
                while let Ok((id, t)) = transport_rx.pop() {
                    engine.transport(id, t);
                }
                while let Ok(c) = command_rx.pop() {
                    engine.command(&c);
                }

                let frames_needed = data.len() / channels;
                while left_ring.available() < frames_needed {
                    let mut l = [0f32; BLOCK];
                    let mut r = [0f32; BLOCK];
                    engine.process_block(&mut l, &mut r);
                    left_ring.push_slice(&l);
                    right_ring.push_slice(&r);
                }

                // Full queue (UI not drawing): the UI just misses these, nothing waits.
                engine.seqs(|id, step, bank, queued| {
                    let _ = steps_tx.push((id, step, bank, queued));
                });
                engine.clocks(|id, running| {
                    let _ = clocks_tx.push((id, running));
                });
                engine.delays(|id, lock, ms| {
                    let _ = delays_tx.push((id, lock, ms));
                });
                engine.lfos(|id, sync| {
                    let _ = lfos_tx.push((id, sync));
                });
                let (held, sounding) = engine.keys();
                keys.store(pack_keys(held, sounding), Ordering::Relaxed);
                // Full (UI not drawing): this window is dropped; the UI counts the gap.
                if let Some(r) = engine.take_probe_report() {
                    let _ = probe_tx.push(r);
                }

                let rec = tap.begin(frames_needed);
                for frame in data.chunks_mut(channels) {
                    let l = left_ring.pop().unwrap_or(0.0);
                    let r = right_ring.pop().unwrap_or(0.0);
                    if rec {
                        tap.frame(l, r);
                    }
                    peaks.feed(l, r);
                    frame[0] = l;
                    for s in frame.iter_mut().skip(1) {
                        *s = r;
                    }
                }
                t.record(started.elapsed(), since_last, frames_needed, sample_rate);
            },
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
            stream.is_some().then_some(command_tx),
            stream.is_some().then_some(transport_tx),
            collector.handle(),
            feedback,
            sample_rate,
            DEFAULT_VOICE_COUNT,
            Some(patch),
        );
        let host = AudioHost {
            sample_rate,
            collector,
            status,
            steps_rx: Some(steps_rx),
            clocks_rx: Some(clocks_rx),
            delays_rx: Some(delays_rx),
            lfos_rx: Some(lfos_rx),
            recorder: stream.is_some().then_some(recorder),
            timing: stream.is_some().then_some(timing),
            probe_rx: stream.is_some().then_some(probe_rx),
            faults_rx: stream.is_some().then_some(faults_rx),
            _stream: stream,
        };
        (host, delivery)
    }
}

/// The document, the UI state and the control side of the audio queues: one lock, taken by
/// the UI for a frame and by the control thread for MIDI CCs and retries, never by audio.
struct Core {
    editor: PatchEditor,
    ui: UiState,
    delivery: Delivery,
}

/// Seconds since the process started: the control layer's steady clock (CC gestures, the
/// button guard).
fn clock_s() -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_secs_f64()
}

/// The control thread: applies MIDI CCs (mappings, pickup, buttons) and retries whatever waits
/// for the audio queue, without an editor frame. Woken by the MIDI input for each CC, and every
/// few milliseconds while something waits.
fn pump(core: Arc<Mutex<Core>>, mut cc: rtrb::Consumer<(u8, u8, u8)>) {
    let mut events = Vec::with_capacity(64);
    loop {
        std::thread::park_timeout(std::time::Duration::from_millis(5));
        events.clear();
        while let Ok(e) = cc.pop() {
            events.push(e);
        }
        let Ok(mut c) = core.lock() else {
            return;
        };
        let Core {
            editor,
            ui,
            delivery,
        } = &mut *c;
        kabl_ui::perform::rearm(ui, clock_s());
        if !events.is_empty() {
            control::midi(editor, ui, delivery, &events, clock_s());
        } else if delivery.waiting() > 0 {
            delivery.flush();
        }
    }
}

/// What the MIDI callback owns: key events (notes, sustain pedal, All Notes Off / All Sound
/// Off) to the audio thread, where the engine's keyboards assign voices; every other CC to
/// the UI (learn, mappings, buttons).
struct MidiSink {
    notes: rtrb::Producer<KeyEvent>,
    cc: rtrb::Producer<(u8, u8, u8)>,
    ctx: Option<egui::Context>,
    /// The control thread (`pump`), woken for each CC.
    pump: Option<std::thread::Thread>,
}

/// CCs that are key events, never learnable: sustain pedal, All Sound Off, All Notes Off.
fn is_key_cc(cc: u8) -> bool {
    matches!(cc, 64 | 120 | 123)
}

/// Keys held and voices sounding, as the audio thread publishes them.
fn pack_keys(held: usize, sounding: usize) -> u64 {
    ((held as u64) << 32) | sounding as u64
}

fn unpack_keys(v: u64) -> (u64, u64) {
    (v >> 32, v & 0xFFFF_FFFF)
}

/// One incoming MIDI message: a key event to the audio thread, any other CC to the UI.
fn on_message(s: &mut MidiSink, data: &[u8]) {
    if let Some(e) = KeyEvent::from_midi(data) {
        let _ = s.notes.push(e);
    } else if data.len() >= 3 && data[0] & 0xF0 == 0xB0 && !is_key_cc(data[1]) {
        let _ = s.cc.push((data[0] & 0x0F, data[1], data[2]));
        if let Some(p) = &s.pump {
            p.unpark();
        }
        if let Some(ctx) = &s.ctx {
            ctx.request_repaint();
        }
    }
}

/// Test hook for machines without an ALSA sequencer (a container): `KABL_MIDI_PIPE=PATH`
/// lists the input `kabl-pipe` while the fifo PATH exists (`examples/midi_player` makes it
/// and writes hex bytes, one message per line). Connecting reads it on a thread through the
/// same `on_message` as a real port; removing the fifo is an unplug.
const PIPE_PORT: &str = "kabl-pipe";

fn pipe_path() -> Option<String> {
    std::env::var("KABL_MIDI_PIPE").ok()
}

fn read_pipe(path: String, sink: Arc<Mutex<MidiSink>>, stop: Arc<std::sync::atomic::AtomicBool>) {
    use std::io::BufRead;
    while !stop.load(Ordering::Acquire) {
        let Ok(f) = std::fs::File::open(&path) else {
            return;
        };
        for line in std::io::BufReader::new(f).lines() {
            let Ok(line) = line else { break };
            if stop.load(Ordering::Acquire) {
                return;
            }
            let data: Vec<u8> = line
                .split_whitespace()
                .filter_map(|b| u8::from_str_radix(b, 16).ok())
                .collect();
            if let Ok(mut s) = sink.lock() {
                on_message(&mut s, &data);
            }
        }
    }
}

/// The MIDI input connection. The sink outlives connections, so switching ports keeps the
/// queues; a switch releases every voice first so no note hangs.
struct Midi {
    sink: Arc<Mutex<MidiSink>>,
    conn: Option<midir::MidiInputConnection<Arc<Mutex<MidiSink>>>>,
    /// Stop flag of the `KABL_MIDI_PIPE` reader, when that is the input.
    pipe: Option<Arc<std::sync::atomic::AtomicBool>>,
    port: Option<String>,
    /// Keys held and voices sounding (`pack_keys`), from the audio thread.
    keys: Arc<AtomicU64>,
    ports: Vec<String>,
    scanned: Option<std::time::Instant>,
    /// The port that disappeared while connected; reconnected when it comes back.
    lost: Option<String>,
}

fn input_names() -> Vec<String> {
    let mut names: Vec<String> = match midir::MidiInput::new("kabl-ui-scan") {
        Ok(m) => m
            .ports()
            .iter()
            .filter_map(|p| m.port_name(p).ok())
            .collect(),
        Err(_) => Vec::new(),
    };
    if pipe_path().is_some_and(|p| std::path::Path::new(&p).exists()) {
        names.push(PIPE_PORT.into());
    }
    names
}

impl Midi {
    /// Also returns the CC queue's consumer, for the control thread.
    fn new(notes: rtrb::Producer<KeyEvent>) -> (Self, rtrb::Consumer<(u8, u8, u8)>) {
        let (cc, cc_rx) = rtrb::RingBuffer::new(1024);
        let m = Midi {
            sink: Arc::new(Mutex::new(MidiSink {
                notes,
                cc,
                ctx: None,
                pump: None,
            })),
            conn: None,
            pipe: None,
            port: None,
            keys: Arc::new(AtomicU64::new(0)),
            ports: Vec::new(),
            scanned: None,
            lost: None,
        };
        (m, cc_rx)
    }

    fn disconnect(&mut self) {
        if let Some(p) = &self.port {
            log::info!(target: "midi", "disconnect \"{p}\"");
        }
        if let Some(c) = self.conn.take() {
            c.close();
        }
        if let Some(stop) = self.pipe.take() {
            stop.store(true, Ordering::Release);
        }
        self.port = None;
        self.release_all();
    }

    /// Every keyboard releases every voice and forgets its keys and the pedal
    /// (docs/sound-palette-batch/keyboard.md, "Other events").
    fn release_all(&self) {
        let mut s = self.sink.lock().unwrap();
        let _ = s.notes.push(KeyEvent::AllOff);
    }

    fn connect(&mut self, name: &str) -> Result<(), String> {
        self.disconnect();
        if let Some(path) = pipe_path().filter(|_| name == PIPE_PORT) {
            let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let (sink, flag) = (self.sink.clone(), stop.clone());
            std::thread::spawn(move || read_pipe(path, sink, flag));
            log::info!(target: "midi", "listening for MIDI on \"{name}\" (test pipe)");
            self.pipe = Some(stop);
            self.port = Some(name.to_string());
            return Ok(());
        }
        let midi_in = midir::MidiInput::new("kabl-ui").map_err(|e| e.to_string())?;
        let port = midi_in
            .ports()
            .into_iter()
            .find(|p| midi_in.port_name(p).is_ok_and(|n| n == name))
            .ok_or_else(|| format!("MIDI input \"{name}\" is gone"))?;
        let conn = midi_in
            .connect(
                &port,
                "kabl-input",
                |_stamp, data, sink: &mut Arc<Mutex<MidiSink>>| {
                    if let Ok(mut s) = sink.lock() {
                        on_message(&mut s, data);
                    }
                },
                self.sink.clone(),
            )
            .map_err(|e| e.to_string())?;
        log::info!(target: "midi", "listening for MIDI on \"{name}\"");
        self.conn = Some(conn);
        self.port = Some(name.to_string());
        Ok(())
    }

    /// The first port whose name contains `filter` (case-insensitive), else the first that isn't
    /// ALSA's own loopback.
    fn connect_default(&mut self, filter: Option<&str>) {
        let names = input_names();
        let pick = match filter {
            Some(f) => names
                .iter()
                .find(|n| n.to_lowercase().contains(&f.to_lowercase())),
            None => names.iter().find(|n| !n.contains("Through")),
        };
        match pick {
            Some(n) => {
                if let Err(e) = self.connect(&n.clone()) {
                    log::warn!(target: "midi", "{e}");
                }
            }
            None => log::info!(target: "midi", "no MIDI input matched {filter:?}"),
        }
        self.ports = names;
    }
}

/// A MIDI port name without ALSA's trailing `client:port` numbers.
fn base_name(name: &str) -> &str {
    match name.rsplit_once(' ') {
        Some((head, tail))
            if tail.split(':').count() == 2
                && tail
                    .split(':')
                    .all(|p| p.chars().all(|c| c.is_ascii_digit())) =>
        {
            head
        }
        _ => name,
    }
}

/// Every MIDI mapping waits for pickup again (new or reconnected hardware).
fn reset_pickup(ui: &mut UiState) {
    ui.button_rearm = true;
    for t in ui.takeover.values_mut() {
        *t = Default::default();
    }
}

struct App {
    core: Arc<Mutex<Core>>,
    /// `Delivery::generation` the inspector last heard about.
    seen_generation: u64,
    audio: AudioHost,
    midi: Midi,
    /// `KABL_HITS_FILE`: where to write the drawn target rects (for scripted real-input runs).
    hits_file: Option<String>,
    hits_written: String,
    /// `KABL_STATS_FILE`: callback timing, rewritten about once a second.
    stats_file: Option<(String, std::time::Instant)>,
    peaks: Arc<record::PeakTap>,
    meter_at: std::time::Instant,
    /// Callback count last seen and when it last moved: a stream that stops calling back
    /// (an unsupported buffer size, a device gone) is reported, not left silent.
    watchdog: (u64, std::time::Instant),
    log: Option<kabl_standalone::applog::LogHandle>,
    health: Health,
}

/// Audio health as last logged: the log gets counts per interval, never a line per event.
struct Health {
    at: std::time::Instant,
    debug_at: std::time::Instant,
    /// (xruns, late executions, late arrivals, other errors, callbacks) at `at`.
    counts: [u64; 5],
    rt_logged: bool,
    stalled: bool,
    /// `errors` at the last WARN line.
    errors_seen: [u64; kabl_standalone::STREAM_ERROR_KINDS.len() + 2],
    /// The last error taken from the queue, and when (its age is logged: it may predate the
    /// interval whose counts it is printed with).
    fault_text: Option<(String, std::time::Instant)>,
    log_problem: Option<String>,
    log_checked: std::time::Instant,
}

impl Health {
    fn new() -> Self {
        let now = std::time::Instant::now();
        Health {
            at: now,
            debug_at: now,
            counts: [0; 5],
            rt_logged: false,
            stalled: false,
            errors_seen: [0; kabl_standalone::STREAM_ERROR_KINDS.len() + 2],
            fault_text: None,
            log_problem: None,
            log_checked: now,
        }
    }
}

impl App {
    /// UI thread, every frame: logs what the audio side counted, rate-limited.
    fn log_health(&mut self) {
        let h = &mut self.health;
        if let Some(rx) = self.audio.faults_rx.as_mut() {
            while let Ok(e) = rx.pop() {
                h.fault_text = Some((e.to_string(), std::time::Instant::now()));
            }
        }
        let Some(t) = &self.audio.timing else {
            return;
        };
        let get = |a: &AtomicU64| a.load(Ordering::Relaxed);
        if !h.rt_logged && t.rt.load(Ordering::Relaxed) != 0 {
            h.rt_logged = true;
            match t.rt.load(Ordering::Relaxed) {
                1 => log::info!(target: "audio", "real-time priority granted"),
                _ => log::warn!(
                    target: "audio",
                    "real-time priority refused: {}",
                    t.rt_error.get().map_or("?", String::as_str)
                ),
            }
        }
        let now = [
            get(&t.xruns),
            get(&t.late),
            get(&t.arrival_late),
            get(&t.other_errors),
            get(&t.count),
        ];
        let secs = h.at.elapsed().as_secs_f64();
        if secs >= 10.0 {
            let d: Vec<u64> = now.iter().zip(h.counts).map(|(a, b)| a - b).collect();
            // Xruns and late executions are INFO; arrival lateness alone (common on a busy
            // machine, and not itself a dropout) only at DEBUG.
            if d[0] + d[1] > 0
                || (d[2] > 0 && log::log_enabled!(target: "audio.health", log::Level::Debug))
            {
                log::info!(
                    target: "audio.health",
                    "last {secs:.1}s: xruns={} late_executions={} late_arrivals={} callbacks={} worst_run_us={:.0}",
                    d[0], d[1], d[2], d[4], get(&t.worst_ns) as f64 / 1e3
                );
            }
            let (kinds, lost) = t.errors.since(&mut h.errors_seen);
            if !kinds.is_empty() {
                let last = h
                    .fault_text
                    .as_ref()
                    .map(|(t, at)| (t.as_str(), at.elapsed().as_secs_f64()));
                log::warn!(
                    target: "audio",
                    "{}",
                    kabl_standalone::stream_error_line(secs, &kinds, last, lost)
                );
            }
            h.counts = now;
            h.at = std::time::Instant::now();
        }
        if h.debug_at.elapsed().as_secs() >= 60 {
            h.debug_at = std::time::Instant::now();
            log::debug!(target: "audio.health", "{}", t.line(self.audio.sample_rate));
        }
        let stalled = self.audio.status.starts_with("audio stalled");
        if stalled != h.stalled {
            h.stalled = stalled;
            if stalled {
                log::warn!(target: "audio", "stalled: no callbacks for over 1.5 s after {}", now[4]);
            } else {
                log::info!(target: "audio", "callbacks resumed");
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // The whole frame edits under the core lock; the control thread waits meanwhile.
        let core = self.core.clone();
        let Ok(mut core) = core.lock() else {
            return;
        };
        let Core {
            editor,
            ui: ui_state,
            delivery,
        } = &mut *core;
        // Ctrl+Q quits the normal way (the recorder finalizes its take on the way out).
        if ui.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::Q,
            ))
        }) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        egui::Panel::bottom("kabl-status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Out");
                kabl_ui::record::meter_ui(ui, &mut ui_state.meter);
                ui.separator();
                ui.label(&self.audio.status);
                if let Some(log) = &self.log {
                    ui.separator();
                    let problem = self.health.log_problem.clone();
                    let text = if problem.is_some() { "Log ⚠" } else { "Log" };
                    let r = ui.small_button(text).on_hover_text(format!(
                        "{}\nLevel {} (KABL_LOG or --log-level to change). Click to copy the path.",
                        problem.unwrap_or_else(|| log.path.display().to_string()),
                        log.level
                    ));
                    ui_state.record("log-path".into(), r.rect);
                    if r.clicked() {
                        ui.ctx().copy_text(log.path.display().to_string());
                        ui_state.last_message =
                            Some(format!("log path copied: {}", log.path.display()));
                    }
                }
                if let Some(t) = &self.audio.timing {
                    ui.separator();
                    let (short, full) = t.summary(self.audio.sample_rate);
                    ui.add(egui::Label::new(egui::RichText::new(short).small()).truncate())
                        .on_hover_text(full);
                }
            });
        });
        // Measurements for the inspector: only the newest few matter.
        let now = ui.input(|i| i.time);
        ui_state.inspect.audio = self.audio.timing.is_some();
        if let Some(rx) = self.audio.probe_rx.as_mut() {
            // Aged on the audio clock: a report that waited in the queue (the UI stalled) is
            // not fresh just because it was read now.
            let played = self
                .audio
                .timing
                .as_ref()
                .map_or(0, |t| t.frames_total.load(Ordering::Relaxed));
            while let Ok(r) = rx.pop() {
                let age =
                    played.saturating_sub(r.end_sample) as f64 / self.audio.sample_rate as f64;
                ui_state.inspect.accept(r, now, age);
            }
        }
        self.log_health();
        if self.health.log_checked.elapsed().as_secs() >= 2 {
            self.health.log_checked = std::time::Instant::now();
            let p = self.log.as_ref().and_then(|l| l.problem());
            if p.is_some() && p != self.health.log_problem {
                // Shown once per change, never per line.
                ui_state.last_message = p.clone();
            }
            self.health.log_problem = p;
        }
        if let Some(rx) = self.audio.steps_rx.as_mut() {
            while let Ok((id, step, bank, queued)) = rx.pop() {
                ui_state.seq_steps.insert(id, step);
                ui_state.seq_banks.insert(id, (bank, queued));
            }
        }
        if let Some(rx) = self.audio.clocks_rx.as_mut() {
            while let Ok((id, running)) = rx.pop() {
                ui_state.clock_running.insert(id, running);
            }
        }
        if let Some(pick) = ui_state.midi_select.take() {
            self.midi.lost = None;
            ui_state.midi_note = None;
            reset_pickup(ui_state);
            let r = match pick {
                Some(name) => self.midi.connect(&name),
                None => {
                    self.midi.disconnect();
                    Ok(())
                }
            };
            if let Err(e) = r {
                ui_state.last_message = Some(e);
            }
        }
        if self
            .midi
            .scanned
            .is_none_or(|t| t.elapsed().as_secs_f32() > 2.0)
        {
            self.midi.scanned = Some(std::time::Instant::now());
            self.midi.ports = input_names();
            // The connected input vanished (unplugged): release its notes, keep its name, and
            // reconnect when it comes back. Hardware positions are unknown either way, so every
            // mapping waits for pickup again.
            if let Some(port) = self.midi.port.clone() {
                if !self.midi.ports.contains(&port) {
                    self.midi.disconnect();
                    self.midi.lost = Some(port.clone());
                    log::warn!(target: "midi", "input \"{port}\" disappeared: its notes were released");
                    ui_state.midi_note =
                        Some(format!("\"{port}\" disconnected: its notes were released"));
                    reset_pickup(ui_state);
                }
            } else if let Some(port) = self.midi.lost.clone() {
                // ALSA renumbers a replugged device's client (`… 24:0` → `… 28:0`).
                let back = self
                    .midi
                    .ports
                    .iter()
                    .find(|n| base_name(n) == base_name(&port))
                    .cloned();
                if let Some(now) = back.filter(|n| self.midi.connect(n).is_ok()) {
                    self.midi.lost = None;
                    ui_state.midi_note = Some(format!("\"{now}\" reconnected"));
                    log::info!(target: "midi", "input \"{now}\" reconnected");
                    reset_pickup(ui_state);
                }
            }
        }
        if std::mem::take(&mut ui_state.all_notes_off) {
            self.midi.release_all();
            ui_state.last_message = Some("all MIDI voices released".into());
        }
        if let Some(t) = &self.audio.timing {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
            let n = t.count.load(Ordering::Relaxed);
            if n != self.watchdog.0 {
                self.watchdog = (n, std::time::Instant::now());
                if self.audio.status.starts_with("audio stalled") {
                    self.audio.status = format!("audio resumed after a stall (callback {n})");
                }
            } else if self.watchdog.1.elapsed().as_secs_f32() > 1.5
                && !self.audio.status.starts_with("audio stalled")
            {
                self.audio.status = format!(
                    "audio stalled: no callbacks for over a second after {n} \
                     (the device may not support these settings, or it went away)"
                );
            }
        }
        let dt = self.meter_at.elapsed().as_secs_f32();
        self.meter_at = std::time::Instant::now();
        let (p, bad) = self.peaks.take();
        ui_state.meter.update(p, bad, dt);
        ui_state.midi_inputs = self.midi.ports.clone();
        ui_state.midi_input = self.midi.port.clone();
        if let Some(rx) = self.audio.delays_rx.as_mut() {
            while let Ok((id, lock, ms)) = rx.pop() {
                ui_state.delay_status.insert(id, (lock, ms));
            }
        }
        if let Some(rx) = self.audio.lfos_rx.as_mut() {
            while let Ok((id, sync)) = rx.pop() {
                ui_state.lfo_status.insert(id, sync);
            }
        }
        if !ui_state.seq_steps.is_empty() || !ui_state.delay_status.is_empty() {
            // egui only repaints on input; the step light and readouts need frames of their own.
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(30));
        }
        show(editor, ui_state, ui);
        // Scripted real-input runs (xdotool) read the drawn targets from here.
        if let Some(path) = &self.hits_file {
            let text: String = ui_state
                .hits
                .iter()
                .map(|(k, r)| {
                    format!(
                        "{k} {:.0} {:.0} {:.0} {:.0}\n",
                        r.min.x, r.min.y, r.max.x, r.max.y
                    )
                })
                .collect();
            if text != self.hits_written {
                let _ = std::fs::write(path, &text);
                self.hits_written = text;
            }
        }
        if let (Some((path, at)), Some(t)) = (self.stats_file.as_mut(), &self.audio.timing) {
            if at.elapsed().as_secs_f32() > 1.0 {
                *at = std::time::Instant::now();
                let (held, sounding) = unpack_keys(self.midi.keys.load(Ordering::Relaxed));
                let _ = std::fs::write(
                    path,
                    format!(
                        "{} · {} live graph allocations · {} undo entries · {} MIDI notes held · {} voices sounding\n{}\n{}\n",
                        t.line(self.audio.sample_rate),
                        self.audio.collector.alloc_count(),
                        editor.log().entries().len(),
                        held,
                        sounding,
                        t.hist_line(),
                        delivery_line(delivery),
                    ),
                );
            }
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(500));
        }
        // Document changes and queued commands to the audio thread (runtime values or a graph).
        control::deliver(editor, ui_state, delivery);
        // A compile happened (here or on the control thread): the inspector checks whether
        // its readings still describe the patch.
        if delivery.generation != self.seen_generation {
            self.seen_generation = delivery.generation;
            if let Some(e) = &delivery.compile_error {
                self.audio.status = format!("recompile failed: {e}");
            }
            ui_state.inspect.rebuilt(
                delivery.generation,
                delivery.compile_error.clone(),
                editor.state(),
                ui.input(|i| i.time),
            );
        }
        // Closing the window (or Ctrl+Q) with unsaved changes asks first.
        let close = ui.ctx().input(|i| i.viewport().close_requested());
        if close && !ui_state.quit_now && kabl_ui::browser::is_modified(editor, ui_state) {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if ui_state.browser.dialog.is_none() {
                kabl_ui::browser::request(editor, ui_state, kabl_ui::browser::Pending::Quit);
            } else {
                // A dialog (Save As, a question) is already open: it stays, and the close
                // waits for it rather than replacing it.
                ui_state.last_message =
                    Some("close: finish or cancel the open dialog first".into());
            }
        }
        if ui_state.quit_now && !close {
            ui_state.launches.clear();
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.audio.collector.collect();
        if delivery.waiting() > 0 {
            ui.ctx().request_repaint();
        }
    }
}

/// The control counters for `KABL_STATS_FILE`.
fn delivery_line(d: &Delivery) -> String {
    let c = d.counts;
    let fb = &d.feedback;
    format!(
        "control: {} graphs compiled ({} failed, {} as fallback) · {} runtime values ({} coalesced) \
         · audio took {} graphs ({} refused as stale), {} values ({} unresolved) · {} waiting · \
         {} revisions behind · {} actions not delivered",
        c.graphs,
        c.failed,
        c.fallbacks,
        c.values,
        c.coalesced,
        Feedback::get(&fb.graphs_taken),
        Feedback::get(&fb.stale_graphs),
        Feedback::get(&fb.sets_taken),
        Feedback::get(&fb.unresolved),
        d.waiting(),
        d.behind(),
        c.dropped_actions,
    )
}

/// `kabl-ui [--patch <dir>] [--size <W>x<H>]`. Without `--patch`, starts from
/// `default_patch()`. Default window 1440×900; 1280×800 is the supported minimum.
fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1).cloned())
    };
    use kabl_standalone::applog;
    let (level, bad_level) = applog::level_from(&args);
    let mut log = applog::init("kabl-ui", level, applog::log_dir());
    log::info!(
        target: "app",
        "start kabl-ui {} level={level} log={}",
        applog::build_id(),
        log.as_ref().map_or("unavailable".into(), |l| l.path.display().to_string())
    );
    if let Some(w) = bad_level {
        log::warn!(target: "app", "unknown log level {w:?}: using info");
    }
    let mut ui_state = UiState::default();
    let library = kabl_ui::browser::open_library();
    log::info!(
        target: "library",
        "factory sounds: {}; your sounds: {}",
        library
            .factory_dir
            .as_ref()
            .map_or("not found".into(), |d| d.display().to_string()),
        library.sounds_dir().display()
    );
    for note in &library.notes {
        log::warn!(target: "library", "{note}");
    }
    let editor = match flag("--patch") {
        Some(dir) => match kabl_core::load(std::path::Path::new(&dir)) {
            Ok(log) => {
                let editor = PatchEditor::from_log(log);
                // A library sound opened by path keeps its library identity (factory
                // protection, name); anything else is a plain folder.
                let canonical = std::path::Path::new(&dir).canonicalize().ok();
                let doc = match library
                    .entries
                    .iter()
                    .find(|e| e.dir.canonicalize().ok() == canonical)
                {
                    Some(e) => kabl_ui::browser::Doc {
                        name: e.meta.name.clone(),
                        origin: kabl_ui::browser::DocOrigin::Library(e.id.clone()),
                        meta: e.meta.clone(),
                        saved: editor.state().clone(),
                    },
                    None => {
                        let name = std::path::Path::new(&dir)
                            .file_name()
                            .map_or(dir.clone(), |n| n.to_string_lossy().to_string());
                        kabl_ui::browser::Doc::new(
                            &name,
                            kabl_ui::browser::DocOrigin::Folder(dir.clone()),
                            editor.state(),
                        )
                    }
                };
                ui_state.browser.folder = dir;
                ui_state.doc = Some(doc);
                editor
            }
            Err(err) => {
                log::error!(target: "doc", "failed to load patch from {dir}: {err:?}");
                if let Some(l) = log.as_mut() {
                    l.shutdown();
                }
                std::process::exit(1);
            }
        },
        None => {
            // No patch named: start with the browser open on the plain start patch.
            ui_state.browser_open = true;
            PatchEditor::seed_from(&default_patch())
        }
    };
    ui_state.library = Some(library);
    if args.iter().any(|a| a == "--browser") {
        ui_state.browser_open = true;
    }
    let size = flag("--size")
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some([w.parse().ok()?, h.parse().ok()?])
        })
        .unwrap_or([1440.0, 900.0]);
    let (notes_tx, notes_rx) = rtrb::RingBuffer::<KeyEvent>::new(1024);
    let (mut midi, cc_rx) = Midi::new(notes_tx);
    midi.connect_default(flag("--midi").as_deref());
    let num = |name: &str| flag(name).and_then(|v| v.parse::<u32>().ok());
    let peaks = Arc::new(record::PeakTap::default());
    let (mut audio, delivery) = AudioHost::start(
        editor.state(),
        notes_rx,
        midi.keys.clone(),
        num("--rate"),
        num("--frames"),
        !args.iter().any(|a| a == "--no-rt"),
        peaks.clone(),
    );
    ui_state.inspect.rebuilt(1, None, editor.state(), 0.0);
    ui_state.recorder = audio.recorder.take();
    ui_state.sample_rate = audio.sample_rate;
    if let (Some(rec), Some(dir)) = (ui_state.recorder.as_mut(), flag("--record-dir")) {
        rec.dir = dir;
    }
    if args.iter().any(|a| a == "--perform") {
        ui_state.perform_open = true;
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("kabl")
            .with_inner_size(size)
            .with_min_inner_size([1280.0, 800.0]),
        ..Default::default()
    };
    let result = eframe::run_native(
        "kabl",
        options,
        Box::new(|cc| {
            kabl_ui::theme::install_fonts(&cc.egui_ctx);
            let core = Arc::new(Mutex::new(Core {
                editor,
                ui: ui_state,
                delivery,
            }));
            let c = core.clone();
            let pump = std::thread::Builder::new()
                .name("kabl-control".into())
                .spawn(move || pump(c, cc_rx))
                .expect("spawn the control thread");
            {
                let mut sink = midi.sink.lock().unwrap();
                sink.ctx = Some(cc.egui_ctx.clone());
                sink.pump = Some(pump.thread().clone());
            }
            Ok(Box::new(App {
                core,
                seen_generation: 1,
                audio,
                midi,
                hits_file: std::env::var("KABL_HITS_FILE").ok(),
                peaks,
                meter_at: std::time::Instant::now(),
                watchdog: (0, std::time::Instant::now()),
                hits_written: String::new(),
                stats_file: std::env::var("KABL_STATS_FILE")
                    .ok()
                    .map(|f| (f, std::time::Instant::now())),
                log: log.as_ref().map(|l| l.view()),
                health: Health::new(),
            }))
        }),
    );
    log::info!(target: "app", "shutdown result={}", if result.is_ok() { "ok" } else { "error" });
    if let Some(l) = log.as_mut() {
        l.shutdown();
    }
    result
}
