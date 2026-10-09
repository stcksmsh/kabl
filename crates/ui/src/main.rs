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
use kabl_engine::keyboard::{KeyEvent, MidiEvent, Source};
use kabl_engine::timeline::Timeline;

/// A key event and when the MIDI thread received it (docs/midi-timing/design.md, section 4).
type Note = (std::time::Instant, MidiEvent);

/// A controller key event received now.
fn note_now(event: KeyEvent) -> Note {
    (
        std::time::Instant::now(),
        MidiEvent::new(Source::Controller, 0, event),
    )
}
use kabl_engine::patch_engine::PatchEngine;
use kabl_engine::probe::ProbeReport;
use kabl_engine::runtime::{Feedback, ToAudio};
use kabl_modules::builtins::{DelayLock, LfoSync};
use kabl_standalone::{default_patch, RingBuffer, DEFAULT_VOICE_COUNT};
use kabl_ui::control::{self, Delivery};
use kabl_ui::kit::{self, Tip, Tone};
use kabl_ui::style::Role;
use kabl_ui::{record, show, PatchEditor, UiState};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[cfg(test)]
mod callback_allocations {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    use std::sync::atomic::{AtomicUsize, Ordering};

    thread_local! { static INSIDE: Cell<bool> = const { Cell::new(false) }; }
    pub static ALLOCS: AtomicUsize = AtomicUsize::new(0);
    pub static DEALLOCS: AtomicUsize = AtomicUsize::new(0);
    pub struct Counter;
    unsafe impl GlobalAlloc for Counter {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            if INSIDE.try_with(|v| v.get()).unwrap_or(false) {
                ALLOCS.fetch_add(1, Ordering::Relaxed);
            }
            System.alloc(layout)
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            if INSIDE.try_with(|v| v.get()).unwrap_or(false) {
                DEALLOCS.fetch_add(1, Ordering::Relaxed);
            }
            System.dealloc(ptr, layout)
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            if INSIDE.try_with(|v| v.get()).unwrap_or(false) {
                ALLOCS.fetch_add(1, Ordering::Relaxed);
            }
            System.realloc(ptr, layout, size)
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            if INSIDE.try_with(|v| v.get()).unwrap_or(false) {
                ALLOCS.fetch_add(1, Ordering::Relaxed);
            }
            System.alloc_zeroed(layout)
        }
    }
    pub struct Scope(bool);
    impl Scope {
        pub fn new(enabled: bool) -> Self {
            if enabled {
                INSIDE.with(|v| v.set(true));
            }
            Self(enabled)
        }
    }
    impl Drop for Scope {
        fn drop(&mut self) {
            if self.0 {
                INSIDE.with(|v| v.set(false));
            }
        }
    }
}
#[cfg(test)]
#[global_allocator]
static CALLBACK_ALLOCATOR: callback_allocations::Counter = callback_allocations::Counter;
#[cfg(test)]
use callback_allocations::Scope as CallbackAllocationScope;

const RING_CAPACITY: usize = BLOCK * 256;
/// Frames the callback renders through the timeline at a time (stack buffers).
const CHUNK: usize = 256;

/// (sequencer, step, playing bank, queued bank).
type SeqReport = (ModuleId, usize, usize, Option<usize>);

/// Owns everything audio-related: the swap queue's producer, the stream, the MIDI connection,
/// and the `basedrop` collector that frees retired graphs. Lives inside `App`.
struct AudioHost {
    /// Dropped first (fields drop in declaration order): the audio thread stops before the
    /// queues it feeds are torn down.
    _stream: Option<cpal::Stream>,
    gate: Arc<AtomicU64>,
    sample_rate: f32,
    settings: String,
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
    /// Revisions the callback applied and when (`KABL_LATENCY_FILE` only).
    applied_rx: Option<rtrb::Consumer<(u64, f64)>>,
}

#[derive(Clone)]
struct AudioRequest {
    rate: Option<u32>,
    frames: Option<u32>,
    realtime: bool,
    /// Exact enumerated output, with its name checked again before opening.
    device: Option<(usize, String)>,
}

fn output_names() -> Vec<(usize, String)> {
    cpal::default_host()
        .output_devices()
        .map(|ds| {
            ds.enumerate()
                .filter_map(|(i, d)| d.description().ok().map(|x| (i, x.name().to_string())))
                .collect()
        })
        .unwrap_or_default()
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
    /// MIDI key events scheduled, and those that arrived before the previous callback
    /// started or past its length (placed at an edge).
    midi_events: AtomicU64,
    midi_late: AtomicU64,
    /// Arrival to the scheduled frame of the callback that renders it (excluding the
    /// adapter's fixed 64 frames and the device's own latency), shortest and longest.
    midi_wait_min_ns: AtomicU64,
    midi_wait_max_ns: AtomicU64,
    /// Adapter queue: events dropped or forced to a release, and events moved in time.
    timeline_dropped: AtomicU64,
    timeline_moved: AtomicU64,
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
             (first second: worst {:.0} µs) · MIDI: {} events, {} late, arrival to frame \
             {:.2}–{:.2} ms · adapter: {} frames latency, {} dropped, {} moved",
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
            get(&self.midi_events),
            get(&self.midi_late),
            // No events yet: no range ("-"), not a zero delay.
            if get(&self.midi_events) == 0 { f64::NAN } else { get(&self.midi_wait_min_ns) as f64 / 1e6 },
            if get(&self.midi_events) == 0 { f64::NAN } else { get(&self.midi_wait_max_ns) as f64 / 1e6 },
            kabl_engine::timeline::LATENCY,
            get(&self.timeline_dropped),
            get(&self.timeline_moved),
        )
    }
}

impl AudioHost {
    fn offline(collector: Collector, status: String) -> Self {
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
            recorder: None,
            timing: None,
            probe_rx: None,
            faults_rx: None,
            applied_rx: None,
            _stream: None,
        }
    }

    /// The control side for an offline host: nothing is sent.
    fn offline_delivery(&self, patch: &PatchState) -> Delivery {
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
    fn start(
        patch: &PatchState,
        notes: rtrb::Consumer<Note>,
        keys: Arc<AtomicU64>,
        request: &AudioRequest,
        restarting: bool,
        peaks: Arc<record::PeakTap>,
    ) -> (Self, Delivery) {
        let (rate, frames, realtime) = (request.rate, request.frames, request.realtime);
        let mut midi_consumer = notes;
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
        let channels = config.channels() as usize;
        let timing = Arc::new(CallbackTiming::default());
        let t = timing.clone();
        let t_err = timing.clone();
        let mut last_start: Option<std::time::Instant> = None;
        let mut rt_handle = None;
        let mut rt_tried = !realtime;
        let fault = if restarting {
            None
        } else {
            std::env::var("KABL_AUDIO_FAULT").ok()
        };
        let fault_after = std::env::var("KABL_AUDIO_FAULT_AFTER")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(30);
        let mut fault_sent = false;

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
        let gate = Arc::new(AtomicU64::new(if restarting { 0 } else { u64::MAX }));
        let audio_gate = gate.clone();
        let mut ready_frames = 0usize;
        let mut ready_for = 0u64;
        // Measurement only (`KABL_LATENCY_FILE`): the callback reports each newly applied
        // revision with its time.
        let (mut applied_tx, applied_rx) = rtrb::RingBuffer::<(u64, f64)>::new(1024);
        let measure = std::env::var_os("KABL_LATENCY_FILE").is_some();
        let mut last_applied = 0;
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

        let mut left_ring = RingBuffer::new(RING_CAPACITY);
        let mut right_ring = RingBuffer::new(RING_CAPACITY);
        // Boxed: its event queue is too big for the stack of the thread that builds it.
        let mut timeline = Box::new(Timeline::new());

        #[cfg(test)]
        let mut callback_warmups = 0u32;

        let stream = device.build_output_stream(
            stream_config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                #[cfg(test)]
                let _allocation_scope = {
                    callback_warmups += 1;
                    CallbackAllocationScope::new(callback_warmups > 2)
                };
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
                let previous = last_start;
                let since_last = previous.map(|l| started - l);
                last_start = Some(started);
                // Audio thread. No allocation, no locks: install queued graphs, runtime values,
                // commands and transport in request order (allocation-free, at most one queue's
                // worth per callback), apply MIDI to every running graph, render.
                engine.drain(&mut control_rx, control::QUEUE, &fb);
                if measure {
                    let a = Feedback::get(&fb.applied_rev);
                    if a != last_applied {
                        last_applied = a;
                        let _ = applied_tx.push((a, clock_s()));
                    }
                }
                let frames_needed = data.len() / channels;
                // Each message keeps its place inside the previous callback period, one
                // period later (docs/midi-timing/design.md, section 4).
                while let Ok((at, event)) = midi_consumer.pop() {
                    let offset = match previous {
                        Some(p) if at >= p => {
                            ((at - p).as_secs_f64() * sample_rate as f64) as usize
                        }
                        _ => {
                            t.midi_late.fetch_add(1, Ordering::Relaxed);
                            0
                        }
                    };
                    if offset >= frames_needed {
                        t.midi_late.fetch_add(1, Ordering::Relaxed);
                    }
                    let offset = offset.min(frames_needed.saturating_sub(1));
                    let waited = started.saturating_duration_since(at).as_nanos() as u64
                        + (offset as f64 / sample_rate as f64 * 1e9) as u64;
                    if t.midi_events.fetch_add(1, Ordering::Relaxed) == 0 {
                        t.midi_wait_min_ns.store(waited, Ordering::Relaxed);
                    }
                    t.midi_wait_max_ns.fetch_max(waited, Ordering::Relaxed);
                    t.midi_wait_min_ns.fetch_min(waited, Ordering::Relaxed);
                    timeline.push(offset, frames_needed, event);
                }

                // The shared timing path: any callback size, one 64-sample engine grid.
                while left_ring.available() < frames_needed {
                    let n = (frames_needed - left_ring.available()).min(CHUNK);
                    let (mut l, mut r) = ([0f32; CHUNK], [0f32; CHUNK]);
                    timeline.render(&mut engine, &mut l[..n], &mut r[..n], |_, _| {});
                    left_ring.push_slice(&l[..n]);
                    right_ring.push_slice(&r[..n]);
                }
                let q = timeline.stats();
                t.timeline_dropped
                    .store(q.dropped + q.forced, Ordering::Relaxed);
                t.timeline_moved
                    .store(q.clamped + q.reordered, Ordering::Relaxed);

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

                let want = audio_gate.load(Ordering::Relaxed);
                if want != 0 && want != u64::MAX {
                    if ready_for != want {
                        ready_for = want;
                        ready_frames = 0;
                    }
                    if Feedback::get(&fb.applied_rev) >= want && !engine.is_swapping() {
                        ready_frames += frames_needed;
                    } else {
                        ready_frames = 0;
                    }
                    if ready_frames >= sample_rate as usize / 40 {
                        // A failure or a newer edit may have closed/revised the gate since
                        // this callback read `want`; never overwrite that decision.
                        let _ = unmute_if_current(&audio_gate, want);
                    }
                }
                let muted = audio_gate.load(Ordering::Relaxed) != u64::MAX;
                let rec = tap.begin(frames_needed) && !muted;
                for frame in data.chunks_mut(channels) {
                    let l = left_ring.pop().unwrap_or(0.0);
                    let r = right_ring.pop().unwrap_or(0.0);
                    let (l, r) = if muted { (0.0, 0.0) } else { (l, r) };
                    if rec {
                        tap.frame(l, r);
                    }
                    peaks.feed(l, r);
                    frame[0] = l;
                    for s in frame.iter_mut().skip(1) {
                        *s = r;
                    }
                }
                if t.count.load(Ordering::Relaxed) >= fault_after {
                    match fault.as_deref() {
                        Some("stall") => return,
                        Some("error") if !fault_sent => {
                            fault_sent = true;
                            t.other_errors.fetch_add(1, Ordering::Relaxed);
                        }
                        _ => {}
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
                "{} · {sample_rate} Hz · {channels} channels · requested buffer {:?}",
                device
                    .description()
                    .map_or("unknown output".into(), |d| d.name().to_string()),
                stream_config.buffer_size
            ),
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
            applied_rx: (stream.is_some() && measure).then_some(applied_rx),
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
    /// `KABL_LATENCY_FILE` only.
    latency: Option<Latency>,
    progress: Option<Progress>,
    record_finish: Option<std::thread::JoinHandle<record::Recorder>>,
    record_dir: String,
    record_last: Option<record::Outcome>,
    /// CCs queued before the most recent retry boundary must not apply afterward.
    cc_cutoff: f64,
    stale_cc: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum AudioPhase {
    Starting,
    Running,
    Stalled,
    Failed,
    Restarting,
    Unavailable,
}

/// Sticky detection also runs while the UI is suspended.
struct Progress {
    timing: Arc<CallbackTiming>,
    gate: Arc<AtomicU64>,
    last: u64,
    at: std::time::Instant,
    failure: Option<AudioPhase>,
}

fn unmute_if_current(gate: &AtomicU64, revision: u64) -> bool {
    gate.compare_exchange(revision, u64::MAX, Ordering::AcqRel, Ordering::Relaxed)
        .is_ok()
}

impl Progress {
    fn new(timing: Arc<CallbackTiming>, gate: Arc<AtomicU64>) -> Self {
        Self {
            timing,
            gate,
            last: 0,
            at: std::time::Instant::now(),
            failure: None,
        }
    }
    fn observe(&mut self) {
        if self.failure.is_some() {
            self.gate.store(0, Ordering::Release);
            return;
        }
        let n = self.timing.count.load(Ordering::Relaxed);
        if self.timing.other_errors.load(Ordering::Relaxed) > 0 {
            self.failure = Some(AudioPhase::Failed);
        } else if n != self.last {
            self.last = n;
            self.at = std::time::Instant::now();
        } else if self.at.elapsed().as_secs_f32() > if n == 0 { 3.0 } else { 1.5 } {
            self.failure = Some(AudioPhase::Stalled);
        }
        if self.failure.is_some() {
            self.gate.store(0, Ordering::Relaxed);
        }
    }
}

/// Measurement only (`KABL_LATENCY_FILE`): from a CC's arrival at the MIDI input to the audio
/// callback that took the change it made (a value, or a graph for a structural fallback).
#[derive(Default)]
struct Latency {
    /// (revision a CC batch ended on, the batch's earliest arrival), oldest first.
    waiting: std::collections::VecDeque<(u64, f64)>,
    /// Seconds.
    samples: Vec<f64>,
}

impl Latency {
    fn applied(&mut self, rev: u64, at: f64) {
        while self.waiting.front().is_some_and(|w| w.0 <= rev) {
            let (_, arrived) = self.waiting.pop_front().unwrap();
            self.samples.push(at - arrived);
        }
    }

    fn line(&self) -> String {
        let mut v = self.samples.clone();
        v.sort_by(f64::total_cmp);
        let q = |p: f64| {
            v.get(((v.len() as f64 * p).ceil() as usize).saturating_sub(1))
                .map_or("-".into(), |x| format!("{:.2}", x * 1e3))
        };
        format!(
            "CC to audio callback: {} batches · p50 {} ms · p90 {} ms · p99 {} ms · max {} ms · {} waiting",
            v.len(),
            q(0.5),
            q(0.9),
            q(0.99),
            q(1.0),
            self.waiting.len()
        )
    }
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
fn pump(
    core: Arc<Mutex<Core>>,
    mut cc: rtrb::Consumer<((u8, u8, u8), f64)>,
    stop: Arc<AtomicBool>,
) {
    let mut events = Vec::with_capacity(64);
    let mut fresh = Vec::with_capacity(64);
    while !stop.load(Ordering::Acquire) {
        std::thread::park_timeout(std::time::Duration::from_millis(5));
        events.clear();
        while let Ok((e, t)) = cc.pop() {
            events.push((e, t));
        }
        let Ok(mut c) = core.lock() else {
            return;
        };
        let (discarded, arrived) = fresh_cc(&events, c.cc_cutoff, &mut fresh);
        c.stale_cc += discarded as u64;
        if discarded > 0 {
            log::info!(target: "midi", "discarded {discarded} CC events queued before audio retry");
        }
        if let Some(p) = c.progress.as_mut() {
            p.observe();
            if p.failure.is_some() && c.record_finish.is_none() {
                if let Some(mut rec) = c.ui.recorder.take() {
                    c.record_dir = rec.dir.clone();
                    if rec.recording() {
                        c.record_finish = Some(std::thread::spawn(move || {
                            rec.stop_interrupted();
                            rec
                        }));
                    } else {
                        c.record_last = rec.last.take();
                    }
                }
            }
        }
        let Core {
            editor,
            ui,
            delivery,
            latency,
            progress,
            record_finish: _,
            record_dir: _,
            record_last: _,
            cc_cutoff: _,
            stale_cc: _,
        } = &mut *c;
        kabl_ui::perform::rearm(ui, clock_s());
        if !fresh.is_empty() {
            let starting = progress
                .as_ref()
                .is_some_and(|p| p.failure.is_none() && p.gate.load(Ordering::Relaxed) != u64::MAX);
            if starting {
                progress.as_ref().unwrap().gate.store(0, Ordering::Relaxed);
            }
            let before = delivery.rev();
            control::midi(editor, ui, delivery, &fresh, clock_s());
            if starting {
                progress.as_ref().unwrap().gate.store(
                    if delivery.compile_error.is_none() {
                        delivery.rev()
                    } else {
                        0
                    },
                    Ordering::Relaxed,
                );
            }
            if let Some(l) = latency.as_mut().filter(|_| delivery.rev() > before) {
                l.waiting.push_back((delivery.rev(), arrived));
            }
        } else if delivery.waiting() > 0 {
            delivery.flush();
        }
    }
}

fn fresh_cc(
    events: &[((u8, u8, u8), f64)],
    cutoff: f64,
    output: &mut Vec<(u8, u8, u8)>,
) -> (usize, f64) {
    output.clear();
    let mut arrived = f64::MAX;
    for &(event, at) in events {
        if at > cutoff {
            output.push(event);
            arrived = arrived.min(at);
        }
    }
    (events.len() - output.len(), arrived)
}

/// What the MIDI callback owns: key events (notes, sustain pedal, All Notes Off / All Sound
/// Off) to the audio thread, where the engine's keyboards assign voices; every other CC to
/// the UI (learn, mappings, buttons).
struct MidiSink {
    notes: Option<rtrb::Producer<Note>>,
    /// CCs with their arrival time (`clock_s`).
    cc: rtrb::Producer<((u8, u8, u8), f64)>,
    ctx: Option<egui::Context>,
    /// The control thread (`pump`), woken for each CC.
    pump: Option<std::thread::Thread>,
}

/// CCs that are key events, never learnable: sustain pedal, All Sound Off, Reset All
/// Controllers, All Notes Off. CC 1 is both the wheel and learnable (design.md section 7).
fn is_key_cc(cc: u8) -> bool {
    matches!(cc, 64 | 120 | 121 | 123)
}

/// Keys held and voices sounding, as the audio thread publishes them.
fn pack_keys(held: usize, sounding: usize) -> u64 {
    ((held as u64) << 32) | sounding as u64
}

fn unpack_keys(v: u64) -> (u64, u64) {
    (v >> 32, v & 0xFFFF_FFFF)
}

/// One incoming MIDI message: a key event (notes, bend, wheel, pedal, notes off, reset) to the
/// audio thread with its arrival time; any other CC, and CC 1 as well, to the UI.
fn on_message(s: &mut MidiSink, data: &[u8]) {
    if let Some(e) = MidiEvent::parse(Source::Controller, data) {
        if let Some(notes) = s.notes.as_mut() {
            let _ = notes.push((std::time::Instant::now(), e));
        }
    }
    if data.len() >= 3 && data[0] & 0xF0 == 0xB0 && !is_key_cc(data[1]) {
        let _ = s.cc.push(((data[0] & 0x0F, data[1], data[2]), clock_s()));
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

/// Incoming CC messages `((channel, controller, value), arrival time)` for the control thread.
type CcQueue = rtrb::Consumer<((u8, u8, u8), f64)>;

impl Midi {
    /// Also returns the CC queue's consumer, for the control thread.
    fn new(notes: rtrb::Producer<Note>) -> (Self, CcQueue) {
        let (cc, cc_rx) = rtrb::RingBuffer::new(1024);
        let m = Midi {
            sink: Arc::new(Mutex::new(MidiSink {
                notes: Some(notes),
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
        // Only a controller that was connected leaves notes to end.
        if self.port.take().is_some() {
            self.send(KeyEvent::SourceLost);
        }
    }

    /// The controller's notes, pedals and expression end; a running preview does not
    /// (docs/midi-timing/design.md, "Cleanup").
    fn send(&self, e: KeyEvent) {
        let mut s = self.sink.lock().unwrap();
        if let Some(notes) = s.notes.as_mut() {
            let _ = notes.push(note_now(e));
        }
    }

    /// Every keyboard releases every voice and forgets its keys and the pedal
    /// (docs/sound-palette-batch/keyboard.md, "Other events").
    fn release_all(&self) {
        self.send(KeyEvent::AllOff);
    }

    fn suspend_notes(&self) {
        self.sink.lock().unwrap().notes = None;
        self.keys.store(0, Ordering::Relaxed);
    }

    fn resume_notes(&self, notes: rtrb::Producer<Note>) {
        self.sink.lock().unwrap().notes = Some(notes);
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

/// Scheduled, compiled and acknowledged graphs are distinct UI observations.
struct CompileNotice {
    generation: u64,
    error: Option<String>,
    graphs: u64,
    awaiting_rev: Option<u64>,
}

impl CompileNotice {
    fn new(delivery: &Delivery) -> Self {
        Self {
            generation: delivery.generation,
            error: delivery.compile_error.clone(),
            graphs: delivery.counts.graphs,
            awaiting_rev: None,
        }
    }

    /// Returns whether inspection needs rebuilding. A failed audio phase owns its actionable
    /// status; a successful compile is not described as playing until the callback accepts it.
    fn observe(&mut self, phase: AudioPhase, status: &mut String, delivery: &Delivery) -> bool {
        let generation_changed = self.generation != delivery.generation;
        let error_changed = self.error != delivery.compile_error;
        let graphs_changed = self.graphs != delivery.counts.graphs;
        let changed = generation_changed || error_changed || graphs_changed;
        if changed {
            self.generation = delivery.generation;
            self.error = delivery.compile_error.clone();
            self.graphs = delivery.counts.graphs;
            if phase == AudioPhase::Running {
                if let Some(error) = &delivery.compile_error {
                    *status = format!("recompile failed: {error}");
                    self.awaiting_rev = None;
                } else if graphs_changed {
                    *status = "audio running · current graph queued".into();
                    self.awaiting_rev = Some(delivery.rev());
                } else if generation_changed {
                    *status = "audio running · compiling current document".into();
                    self.awaiting_rev = None;
                }
            } else {
                self.awaiting_rev = None;
            }
        }
        if phase == AudioPhase::Running
            && self
                .awaiting_rev
                .is_some_and(|rev| Feedback::get(&delivery.feedback.applied_rev) >= rev)
        {
            *status = "audio running · current graph installed".into();
            self.awaiting_rev = None;
        }
        changed
    }
}

struct App {
    core: Arc<Mutex<Core>>,
    compile_notice: CompileNotice,
    latency_at: std::time::Instant,
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
    request: AudioRequest,
    outputs: Vec<(usize, String)>,
    output_scanned: std::time::Instant,
    phase: AudioPhase,
    retry: Option<RetryTask>,
    session: u64,
    pending_notes: Option<rtrb::Producer<Note>>,
    pending_recorder: Option<record::Recorder>,
    record_finish: Option<std::thread::JoinHandle<record::Recorder>>,
    record_last: Option<record::Outcome>,
    control_stop: Arc<AtomicBool>,
    control_thread: Option<std::thread::JoinHandle<()>>,
}

struct RetryTask {
    handle: std::thread::JoinHandle<RetryResult>,
    started: std::time::Instant,
    session: u64,
}

struct RetryResult {
    audio: AudioHost,
    delivery: Delivery,
    notes: rtrb::Producer<Note>,
    retired_graphs: usize,
}

struct RetryFault {
    delay_ms: u64,
    fail_reopen: bool,
}

/// The old stream is dropped before its collector is drained or a new stream is opened.
fn retry_worker(
    old_session: (AudioHost, Delivery),
    notes: rtrb::Producer<Note>,
    notes_rx: rtrb::Consumer<Note>,
    keys: Arc<AtomicU64>,
    request: AudioRequest,
    peaks: Arc<record::PeakTap>,
    fault: RetryFault,
) -> RetryResult {
    let (mut old, old_delivery) = old_session;
    drop(old._stream.take());
    drop(old_delivery);
    old.collector.collect();
    let retired_graphs = old.collector.alloc_count();
    drop(old);
    if fault.delay_ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(fault.delay_ms.min(5000)));
    }
    let (audio, delivery) = if fault.fail_reopen {
        let a = AudioHost::offline(
            Collector::new(),
            "injected reopen failure; choose output and Retry".into(),
        );
        let d = a.offline_delivery(&default_patch());
        (a, d)
    } else {
        // Always muted; the current editor state is installed before the gate opens.
        AudioHost::start(&default_patch(), notes_rx, keys, &request, true, peaks)
    };
    RetryResult {
        audio,
        delivery,
        notes,
        retired_graphs,
    }
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
    fn interrupt_record(&mut self, core: &mut Core) {
        let Some(mut rec) = core.ui.recorder.take() else {
            return;
        };
        core.record_dir = rec.dir.clone();
        if rec.recording() {
            self.record_finish = Some(std::thread::spawn(move || {
                rec.stop_interrupted();
                rec
            }));
        } else {
            self.record_last = rec.last.take();
        }
    }

    fn poll_record_finish(&mut self, core: &mut Core) {
        if self.record_finish.as_ref().is_some_and(|h| h.is_finished()) {
            if let Ok(rec) = self.record_finish.take().unwrap().join() {
                self.record_last = rec.last.clone();
                if let Some(last) = &self.record_last {
                    core.ui.last_message = Some(record::describe(last));
                    if let Some(r) = core.ui.recorder.as_mut() {
                        r.last = Some(last.clone());
                    }
                    if let Some(r) = self.pending_recorder.as_mut() {
                        r.last = Some(last.clone());
                    }
                }
            }
        }
    }

    fn begin_retry(&mut self, core: &mut Core) {
        if self.retry.is_some() {
            return;
        }
        self.session += 1;
        core.cc_cutoff = clock_s();
        let session = self.session;
        self.midi.suspend_notes();
        reset_pickup(&mut core.ui);
        core.ui.launches.clear();
        core.ui.transport.clear();
        core.ui.clock_running.clear();
        core.ui.seq_steps.clear();
        core.ui.seq_banks.clear();
        core.ui.delay_status.clear();
        core.ui.lfo_status.clear();
        core.ui.last_message =
            Some("Audio restart: live notes and tails end; clocks stay stopped until Start".into());
        self.interrupt_record(core);
        self.audio.gate.store(0, Ordering::Relaxed);
        let old = std::mem::replace(
            &mut self.audio,
            AudioHost::offline(
                Collector::new(),
                "restarting audio; editing and Save remain available".into(),
            ),
        );
        let old_delivery = std::mem::replace(
            &mut core.delivery,
            self.audio.offline_delivery(core.editor.state()),
        );
        core.progress = None;
        if let Some(l) = core.latency.as_mut() {
            l.waiting.clear();
        }
        let request = self.request.clone();
        let peaks = self.peaks.clone();
        let keys = self.midi.keys.clone();
        let (notes, notes_rx) = rtrb::RingBuffer::new(1024);
        self.phase = AudioPhase::Restarting;
        let delay = std::env::var("KABL_AUDIO_RETRY_DELAY_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let fail = std::env::var("KABL_AUDIO_FAIL_REOPEN_ATTEMPTS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .is_some_and(|n| session <= n + 1);
        let handle = std::thread::Builder::new()
            .name("kabl-audio-retry".into())
            .spawn(move || {
                retry_worker(
                    (old, old_delivery),
                    notes,
                    notes_rx,
                    keys,
                    request,
                    peaks,
                    RetryFault {
                        delay_ms: delay,
                        fail_reopen: fail,
                    },
                )
            })
            .expect("spawn one audio retry worker");
        self.retry = Some(RetryTask {
            handle,
            started: std::time::Instant::now(),
            session,
        });
    }

    fn poll_retry(&mut self, core: &mut Core, now: f64) {
        if !self.retry.as_ref().is_some_and(|r| r.handle.is_finished()) {
            if self
                .retry
                .as_ref()
                .is_some_and(|r| r.started.elapsed().as_secs() >= 10)
            {
                self.audio.status = "backend close/open still blocked; editing and Save work; wait or quit (Retry remains disabled while the worker owns the stream)".into();
            }
            return;
        }
        let task = self.retry.take().unwrap();
        let Ok(mut done) = task.handle.join() else {
            self.phase = AudioPhase::Failed;
            self.audio.status = "audio retry worker failed; choose output and Retry".into();
            return;
        };
        log::info!(target:"audio", "session={} retry completed in {:.2}s; retired graphs remaining={}",
            task.session, task.started.elapsed().as_secs_f32(), done.retired_graphs);
        if done.audio._stream.is_some() {
            done.delivery.async_compile = true;
            done.delivery.compile_delay_ms = std::env::var("KABL_COMPILE_DELAY_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let outcome = done.delivery.sync(core.editor.state(), true, true);
            if outcome != control::Outcome::Failed {
                done.audio
                    .gate
                    .store(done.delivery.rev(), Ordering::Relaxed);
                self.phase = AudioPhase::Starting;
                self.pending_notes = Some(done.notes);
                if let Some(mut rec) = done.audio.recorder.take() {
                    rec.dir = core.record_dir.clone();
                    rec.last = self.record_last.clone();
                    self.pending_recorder = Some(rec);
                }
            } else {
                done.audio.gate.store(0, Ordering::Relaxed);
                done.audio.status = format!(
                    "document cannot play: {}; edit to repair, then Retry",
                    done.delivery
                        .compile_error
                        .as_deref()
                        .unwrap_or("compile failed")
                );
                self.phase = AudioPhase::Failed;
            }
        } else {
            self.phase = AudioPhase::Unavailable;
        }
        self.audio = done.audio;
        core.delivery = done.delivery;
        core.progress = self
            .audio
            .timing
            .as_ref()
            .map(|t| Progress::new(t.clone(), self.audio.gate.clone()));
        self.watchdog = (0, std::time::Instant::now());
        self.health = Health::new();
        core.ui.sample_rate = self.audio.sample_rate;
        core.ui.inspect.reset_audio_session(now);
        core.ui.inspect.rebuilt(
            core.delivery.generation,
            core.delivery.compile_error.clone(),
            core.editor.state(),
            now,
        );
        self.compile_notice = CompileNotice::new(&core.delivery);
    }

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
        if self.output_scanned.elapsed().as_secs_f32() >= 2.0 {
            self.outputs = output_names();
            self.output_scanned = std::time::Instant::now();
        }
        // The whole frame edits under the core lock; the control thread waits meanwhile.
        let core = self.core.clone();
        let Ok(mut core) = core.lock() else {
            return;
        };
        let Core {
            editor,
            ui: ui_state,
            ..
        } = &mut *core;
        kabl_ui::browser::poll_io(editor, ui_state);
        if kabl_ui::browser::io_busy(&core.ui) {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(20));
        }
        if self.record_finish.is_none() {
            self.record_finish = core.record_finish.take();
        }
        if let Some(last) = core.record_last.take() {
            self.record_last = Some(last);
        }
        self.poll_record_finish(&mut core);
        self.poll_retry(&mut core, ui.input(|i| i.time));
        let ready_at_frame_start = self.phase == AudioPhase::Starting
            && self.audio.gate.load(Ordering::Relaxed) == u64::MAX;
        let start_rev = core.delivery.rev();
        if self.phase == AudioPhase::Starting && self.session > 1 {
            self.audio.gate.store(0, Ordering::Relaxed);
        }
        let mut retry_clicked = false;
        let mut interrupt_take = false;
        let Core {
            editor,
            ui: ui_state,
            delivery,
            latency,
            progress,
            record_finish: _,
            record_dir,
            record_last: _,
            cc_cutoff: _,
            stale_cc,
        } = &mut *core;
        if let (Some(rx), Some(l)) = (self.audio.applied_rx.as_mut(), latency.as_mut()) {
            while let Ok((rev, at)) = rx.pop() {
                l.applied(rev, at);
            }
        }
        // Ctrl+Q quits the normal way (the recorder finalizes its take on the way out).
        if ui.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::Q,
            ))
        }) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let st = ui_state.style.clone();
        let st = &*st;
        egui::Panel::bottom("kabl-status")
            .exact_size(st.metrics.status_h + 2.0 * st.sp(1))
            .frame(kit::bar_frame(st))
            .show(ui, |ui| {
            ui.horizontal(|ui| {
                kit::label(ui, st, Role::Label, Tone::Text2, "Out");
                kabl_ui::record::meter_ui(ui, st, &mut ui_state.meter);
                kit::label_truncated(ui, st, Role::Caption, Tone::Text2, &self.audio.status)
                    .on_hover_text(&self.audio.status);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Requested but not yet with the audio thread (it is not taking messages).
                    let waiting = delivery.pending();
                    let diag = ui.add(kit::Button::new(st, if waiting > 0 && self.audio.timing.is_some() { "Diagnostics •" } else { "Diagnostics" }).ghost().small());
                    ui_state.record("diagnostics".into(), diag.rect);
                    egui::Popup::from_toggle_button_response(&diag)
                        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                        .frame(kit::popover_frame(st))
                        .width(320.0)
                        .show(|ui| {
                            let row = |ui: &mut egui::Ui, k: &str, v: String| {
                                ui.horizontal(|ui| {
                                    kit::label(ui, st, Role::Caption, Tone::Text2, k);
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        kit::label(ui, st, Role::Value, Tone::Text, v);
                                    });
                                });
                            };
                            kit::section(ui, st, "Diagnostics");
                            row(ui, "Audio", format!("{:?} · session {}", self.phase, self.session));
                            if waiting > 0 && self.audio.timing.is_some() {
                                row(ui, "Waiting for audio", format!("{waiting} change(s)"));
                            }
                            if !self.audio.settings.is_empty() {
                                kit::paragraph(ui, st, Role::Caption, Tone::Text2, &self.audio.settings);
                            }
                            if let Some(last) = ui_state.recorder.as_ref().and_then(|r| r.last.as_ref())
                                .or(self.record_last.as_ref()) {
                                row(ui, "Last take", record::describe(last));
                            }
                            if let Some(t) = &self.audio.timing {
                                let (short, full) = t.summary(self.audio.sample_rate);
                                kit::gap(ui, st, 1);
                                kit::rule(ui, st);
                                kit::gap(ui, st, 1);
                                kit::paragraph(ui, st, Role::Value, Tone::Text, short);
                                kit::paragraph(ui, st, Role::Caption, Tone::Text3, full);
                            }
                        });
                    if let Some(log) = &self.log {
                        let problem = self.health.log_problem.clone();
                        let text = if problem.is_some() { "Log ⚠" } else { "Log" };
                        let r = ui.add(kit::Button::new(st, text).ghost().small()).tip(st, &format!(
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
                    let r = ui.add(kit::Button::new(st, "Retry audio").small().enabled(self.retry.is_none()))
                        .tip(st, "Restarts stream; live notes and tails end, clocks stop, and a recording becomes a partial take");
                    ui_state.record("audio-retry".into(), r.rect);
                    retry_clicked = r.clicked();
                    let chosen = self.request.device.as_ref().map_or("System default".to_string(),
                        |(i, name)| format!("{i}: {name}"));
                    kit::dropdown(ui, st, "audio-output", &chosen, 190.0, |ui| {
                        if kit::menu_item(ui, st, "System default", self.request.device.is_none()).clicked() {
                            self.request.device = None;
                        }
                        for (i, name) in &self.outputs {
                            let dev = Some((*i, name.clone()));
                            if kit::menu_item(ui, st, &format!("{i}: {name}"), self.request.device == dev).clicked() {
                                self.request.device = dev;
                            }
                        }
                    });
                });
            });
        });
        // Measurements for the inspector: only the newest few matter.
        let now = ui.input(|i| i.time);
        ui_state.inspect.audio = self.phase == AudioPhase::Running;
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
            if let Some(failure) = progress.as_ref().and_then(|p| p.failure) {
                if matches!(self.phase, AudioPhase::Running | AudioPhase::Starting) {
                    self.phase = failure;
                    self.audio.status = format!(
                        "audio {:?} after callback {n}; choose an output and Retry",
                        failure
                    );
                    interrupt_take = true;
                }
            }
            if n != self.watchdog.0 {
                self.watchdog = (n, std::time::Instant::now());
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
        if let Some(rec) = ui_state.recorder.as_ref() {
            // Keep the chosen folder even if the control pump takes this recorder on a
            // failure before the next UI frame.
            *record_dir = rec.dir.clone();
        }
        // Scripted real-input runs (xdotool) read the drawn targets from here.
        if let Some(path) = &self.hits_file {
            let mut text: String = ui_state
                .hits
                .iter()
                .map(|(k, r)| {
                    format!(
                        "{k} {:.0} {:.0} {:.0} {:.0}\n",
                        r.min.x, r.min.y, r.max.x, r.max.y
                    )
                })
                .collect();
            text.insert_str(
                0,
                &format!("# pixels_per_point {}\n", ui.ctx().pixels_per_point()),
            );
            if text != self.hits_written {
                let _ = std::fs::write(path, &text);
                self.hits_written = text;
            }
        }
        if let (Some(l), Ok(path)) = (latency.as_ref(), std::env::var("KABL_LATENCY_FILE")) {
            if self.latency_at.elapsed().as_secs_f32() > 1.0 {
                self.latency_at = std::time::Instant::now();
                let _ = std::fs::write(path, l.line() + "\n");
            }
        }
        if let (Some((path, at)), Some(t)) = (self.stats_file.as_mut(), &self.audio.timing) {
            if at.elapsed().as_secs_f32() > 1.0 {
                *at = std::time::Instant::now();
                let (held, sounding) = unpack_keys(self.midi.keys.load(Ordering::Relaxed));
                let _ = std::fs::write(
                    path,
                    format!(
                        "{} · {} live graph allocations · {} undo entries · {} MIDI notes held · {} voices sounding\n{}\n{} · {} stale CC events discarded\n",
                        t.line(self.audio.sample_rate),
                        self.audio.collector.alloc_count(),
                        editor.log().entries().len(),
                        held,
                        sounding,
                        t.hist_line(),
                        delivery_line(delivery),
                        stale_cc,
                    ),
                );
            }
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(500));
        }
        // Document changes and queued commands to the audio thread (runtime values or a graph).
        control::deliver(editor, ui_state, delivery);
        if self.phase == AudioPhase::Starting {
            if let Some(err) = delivery.compile_error.as_deref() {
                self.audio.gate.store(0, Ordering::Relaxed);
                self.phase = AudioPhase::Failed;
                self.audio.status =
                    format!("document cannot play: {err}; edit to repair, then Retry");
            } else if ready_at_frame_start
                && delivery.rev() == start_rev
                && self
                    .audio
                    .timing
                    .as_ref()
                    .is_some_and(|t| t.count.load(Ordering::Relaxed) > 0)
            {
                self.audio.gate.store(u64::MAX, Ordering::Relaxed);
                self.phase = AudioPhase::Running;
                self.audio.status = format!(
                    "audio running · session {} · current graph installed",
                    self.session
                );
                if let Some(notes) = self.pending_notes.take() {
                    self.midi.resume_notes(notes);
                }
                if let Some(rec) = self.pending_recorder.take() {
                    ui_state.recorder = Some(rec);
                }
            } else if self.session > 1 {
                self.audio.gate.store(delivery.rev(), Ordering::Relaxed);
            }
        }
        // A compile happened (here or on the control thread): the inspector checks whether
        // its readings still describe the patch.
        if self
            .compile_notice
            .observe(self.phase, &mut self.audio.status, delivery)
        {
            ui_state.inspect.rebuilt(
                delivery.generation,
                delivery.compile_error.clone(),
                editor.state(),
                ui.input(|i| i.time),
            );
        }
        // Closing the window (or Ctrl+Q) with unsaved changes asks first.
        let close = ui.ctx().input(|i| i.viewport().close_requested());
        if close && kabl_ui::browser::io_busy(ui_state) {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ui_state.last_message =
                Some("wait for the file operation to finish before closing".into());
        } else if close && !ui_state.quit_now && kabl_ui::browser::is_modified(editor, ui_state) {
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
        if self.retry.is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
        }
        if retry_clicked {
            self.begin_retry(&mut core);
        } else if interrupt_take {
            self.interrupt_record(&mut core);
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.control_stop.store(true, Ordering::Release);
        if let Some(thread) = self.control_thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
        if let Some(task) = self.retry.take() {
            let _ = task.handle.join();
        }
        if let Some(writer) = self.record_finish.take() {
            let _ = writer.join();
        }
        if let Ok(mut core) = self.core.lock() {
            if let Some(writer) = core.record_finish.take() {
                let _ = writer.join();
            }
        }
    }
}

/// The control counters for `KABL_STATS_FILE`.
fn delivery_line(d: &Delivery) -> String {
    let c = d.counts;
    let fb = &d.feedback;
    format!(
        "control: {} graphs compiled ({} failed, {} as fallback) · {} runtime values ({} coalesced) \
         · audio took {} graphs ({} refused as stale), {} values ({} unresolved) · {} pending \
         · {} actions not delivered",
        c.graphs,
        c.failed,
        c.fallbacks,
        c.values,
        c.coalesced,
        Feedback::get(&fb.graphs_taken),
        Feedback::get(&fb.stale_graphs),
        Feedback::get(&fb.sets_taken),
        Feedback::get(&fb.unresolved),
        d.pending(),
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
    ui_state.browser.async_io = true;
    ui_state.browser.io_delay_ms = std::env::var("KABL_BROWSER_IO_DELAY_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if args.iter().any(|a| a == "--browser") {
        ui_state.browser_open = true;
    }
    let size = flag("--size")
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some([w.parse().ok()?, h.parse().ok()?])
        })
        .unwrap_or([1440.0, 900.0]);
    let (notes_tx, notes_rx) = rtrb::RingBuffer::<Note>::new(1024);
    let (mut midi, cc_rx) = Midi::new(notes_tx);
    midi.connect_default(flag("--midi").as_deref());
    let num = |name: &str| flag(name).and_then(|v| v.parse::<u32>().ok());
    let peaks = Arc::new(record::PeakTap::default());
    let request = AudioRequest {
        rate: num("--rate"),
        frames: num("--frames"),
        realtime: !args.iter().any(|a| a == "--no-rt"),
        device: None,
    };
    let (mut audio, mut delivery) = AudioHost::start(
        editor.state(),
        notes_rx,
        midi.keys.clone(),
        &request,
        false,
        peaks.clone(),
    );
    delivery.async_compile = true;
    delivery.compile_delay_ms = std::env::var("KABL_COMPILE_DELAY_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let phase = if audio._stream.is_some() {
        AudioPhase::Starting
    } else {
        AudioPhase::Unavailable
    };
    ui_state.inspect.rebuilt(1, None, editor.state(), 0.0);
    ui_state.recorder = audio.recorder.take();
    ui_state.sample_rate = audio.sample_rate;
    if let (Some(rec), Some(dir)) = (ui_state.recorder.as_mut(), flag("--record-dir")) {
        rec.dir = dir;
    }
    if args.iter().any(|a| a == "--perform") {
        ui_state.perform_open = true;
    }
    let record_dir = ui_state
        .recorder
        .as_ref()
        .map_or_else(|| "Recordings".into(), |r| r.dir.clone());

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
            let compile_notice = CompileNotice::new(&delivery);
            let core = Arc::new(Mutex::new(Core {
                editor,
                ui: ui_state,
                delivery,
                latency: std::env::var_os("KABL_LATENCY_FILE").map(|_| Latency::default()),
                progress: audio
                    .timing
                    .as_ref()
                    .map(|t| Progress::new(t.clone(), audio.gate.clone())),
                record_finish: None,
                record_dir: record_dir.clone(),
                record_last: None,
                cc_cutoff: f64::NEG_INFINITY,
                stale_cc: 0,
            }));
            let c = core.clone();
            let control_stop = Arc::new(AtomicBool::new(false));
            let stop = control_stop.clone();
            let pump = std::thread::Builder::new()
                .name("kabl-control".into())
                .spawn(move || pump(c, cc_rx, stop))
                .expect("spawn the control thread");
            {
                let mut sink = midi.sink.lock().unwrap();
                sink.ctx = Some(cc.egui_ctx.clone());
                sink.pump = Some(pump.thread().clone());
            }
            Ok(Box::new(App {
                core,
                compile_notice,
                latency_at: std::time::Instant::now(),
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
                request,
                outputs: output_names(),
                output_scanned: std::time::Instant::now(),
                phase,
                retry: None,
                session: 1,
                pending_notes: None,
                pending_recorder: None,
                record_finish: None,
                record_last: None,
                control_stop,
                control_thread: Some(pump),
            }))
        }),
    );
    log::info!(target: "app", "shutdown result={}", if result.is_ok() { "ok" } else { "error" });
    if let Some(l) = log.as_mut() {
        l.shutdown();
    }
    result
}

#[cfg(test)]
mod recovery_tests {
    use super::*;

    /// D06: CC 1 is the wheel for the audio thread and still a learnable CC for mappings
    /// (soft takeover unchanged); bend and CC 121 go only to the audio thread; other CCs
    /// only to the control thread; a disconnect ends the controller source, not everything.
    #[test]
    fn midi_messages_reach_the_wheel_and_the_mappings() {
        let (notes, mut notes_rx) = rtrb::RingBuffer::<Note>::new(64);
        let (mut midi, mut cc_rx) = Midi::new(notes);
        let send = |d: &[u8]| on_message(&mut midi.sink.lock().unwrap(), d);
        send(&[0xB2, 1, 99]);
        send(&[0xE2, 0, 0x60]);
        send(&[0xB2, 121, 0]);
        send(&[0xB2, 74, 10]);
        send(&[0x92, 60, 0]);
        let got: Vec<MidiEvent> = std::iter::from_fn(|| notes_rx.pop().ok().map(|n| n.1)).collect();
        let ev = |e| MidiEvent::new(Source::Controller, 2, e);
        assert_eq!(
            got,
            [
                ev(KeyEvent::Wheel(99)),
                ev(KeyEvent::Bend(0x60 << 7)),
                ev(KeyEvent::ResetControllers),
                ev(KeyEvent::Off { note: 60 }),
            ]
        );
        let ccs: Vec<(u8, u8, u8)> = std::iter::from_fn(|| cc_rx.pop().ok().map(|c| c.0)).collect();
        assert_eq!(ccs, [(2, 1, 99), (2, 74, 10)]);
        midi.disconnect();
        assert!(
            notes_rx.pop().is_err(),
            "nothing was connected: nothing to end"
        );
        midi.port = Some("kabl-player".into());
        midi.disconnect();
        assert_eq!(
            notes_rx.pop().unwrap().1,
            MidiEvent::new(Source::Controller, 0, KeyEvent::SourceLost)
        );
    }

    #[test]
    fn delayed_compile_failure_and_repair_update_the_visible_notice() {
        let patch = default_patch();
        let collector = basedrop::Collector::new();
        let (tx, _rx) = rtrb::RingBuffer::new(control::QUEUE);
        let mut delivery = Delivery::new(
            Some(tx),
            collector.handle(),
            Arc::new(Feedback::default()),
            48000.0,
            8,
            Some(&patch),
        );
        delivery.async_compile = true;
        delivery.compile_delay_ms = 80;
        let mut editor = PatchEditor::seed_from(&patch);
        let mut notice = CompileNotice::new(&delivery);
        let mut status = "audio running · current graph installed".to_string();
        editor.add_module("no-such-module", kabl_core::Vec2 { x: 0.0, y: 0.0 });
        assert_eq!(
            delivery.sync(editor.state(), false, false),
            control::Outcome::Compiled
        );
        assert!(notice.observe(AudioPhase::Running, &mut status, &delivery));
        assert!(status.contains("compiling"));
        assert!(notice.error.is_none(), "the worker has not finished yet");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while delivery.compile_error.is_none() {
            delivery.flush();
            assert!(
                std::time::Instant::now() < deadline,
                "compile failure timed out"
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(notice.observe(AudioPhase::Running, &mut status, &delivery));
        assert!(status.contains("recompile failed"));
        assert!(notice.error.as_deref().unwrap().contains("no-such-module"));
        assert!(!notice.observe(AudioPhase::Running, &mut status, &delivery));

        editor.undo();
        assert_eq!(
            delivery.sync(editor.state(), false, false),
            control::Outcome::Compiled
        );
        assert!(
            delivery.compile_error.is_some(),
            "failure remains visible while compiling"
        );
        assert!(notice.observe(AudioPhase::Running, &mut status, &delivery));
        assert!(status.contains("recompile failed"));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while delivery.compile_error.is_some() {
            delivery.flush();
            assert!(
                std::time::Instant::now() < deadline,
                "compile repair timed out"
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(notice.observe(AudioPhase::Running, &mut status, &delivery));
        assert!(notice.error.is_none());
        assert!(
            status.contains("queued"),
            "must not claim playback before callback ack"
        );
        assert!(!notice.observe(AudioPhase::Running, &mut status, &delivery));
        assert!(status.contains("queued"));
        delivery
            .feedback
            .applied_rev
            .store(delivery.rev(), Ordering::Relaxed);
        assert!(!notice.observe(AudioPhase::Running, &mut status, &delivery));
        assert!(status.contains("installed"));

        // A failed retry leaves an actionable status even when a later edit compiles.
        editor.add_module("no-such-module", kabl_core::Vec2 { x: 0.0, y: 0.0 });
        assert_eq!(
            delivery.sync(editor.state(), false, false),
            control::Outcome::Compiled
        );
        status = "document cannot play: invalid graph; edit to repair, then Retry".into();
        assert!(notice.observe(AudioPhase::Failed, &mut status, &delivery));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while delivery.compile_error.is_none() {
            delivery.flush();
            assert!(
                std::time::Instant::now() < deadline,
                "retry compile failure timed out"
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(notice.observe(AudioPhase::Failed, &mut status, &delivery));
        editor.undo();
        assert_eq!(
            delivery.sync(editor.state(), false, false),
            control::Outcome::Compiled
        );
        notice.observe(AudioPhase::Failed, &mut status, &delivery);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while delivery.compile_error.is_some() {
            delivery.flush();
            assert!(
                std::time::Instant::now() < deadline,
                "retry repair timed out"
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(notice.observe(AudioPhase::Failed, &mut status, &delivery));
        assert!(status.contains("Retry"));
        assert!(!status.contains("playing"));
    }

    /// The real control pump and cpal callback keep processing a mapped CC while a 500 ms
    /// Save or Open worker runs. The environment has a software PCM, not a controller.
    #[test]
    #[ignore = "set ALSA_CONFIG_PATH to an ALSA null PCM and run explicitly"]
    fn cc_to_callback_during_delayed_save_and_open() {
        use kabl_ui::{
            browser,
            library::{Library, Meta},
            routing,
        };
        let dir = tempfile::tempdir().unwrap();
        let factory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches");
        let log = kabl_core::load(&factory.join("composition")).unwrap();
        let mut lib = Library::open(Some(factory), dir.path().to_path_buf());
        let meta = Meta {
            name: "CC latency piece".into(),
            ..Meta::default()
        };
        let id = lib.save(&log, meta, None).unwrap();
        let editor = PatchEditor::from_log(log);
        let mut ui = UiState::default();
        ui.library = Some(lib);
        ui.doc = Some(browser::Doc::new(
            "CC latency piece",
            browser::DocOrigin::Library(id),
            editor.state(),
        ));
        ui.browser.async_io = true;
        ui.browser.io_delay_ms = 500;
        let (_, notes_rx) = rtrb::RingBuffer::new(1024);
        let request = AudioRequest {
            rate: Some(48000),
            frames: Some(256),
            realtime: false,
            device: None,
        };
        let (mut host, mut delivery) = AudioHost::start(
            editor.state(),
            notes_rx,
            Arc::new(AtomicU64::new(0)),
            &request,
            false,
            Arc::new(record::PeakTap::default()),
        );
        assert!(host._stream.is_some(), "{}", host.status);
        delivery.async_compile = true;
        let feedback = delivery.feedback.clone();
        let core = Arc::new(Mutex::new(Core {
            editor,
            ui,
            delivery,
            latency: None,
            progress: None,
            record_finish: None,
            record_dir: String::new(),
            record_last: None,
            cc_cutoff: f64::NEG_INFINITY,
            stale_cc: 0,
        }));
        let (mut cc_tx, cc_rx) = rtrb::RingBuffer::new(128);
        let stop = Arc::new(AtomicBool::new(false));
        let pump_core = core.clone();
        let pump_stop = stop.clone();
        let pump_thread = std::thread::spawn(move || pump(pump_core, cc_rx, pump_stop));

        let send_and_measure =
            |core: &Arc<Mutex<Core>>, tx: &mut rtrb::Producer<((u8, u8, u8), f64)>| {
                let c = core.lock().unwrap();
                let before = c.delivery.rev();
                let state = c.editor.state();
                let module = &state.modules[&3];
                let info = kabl_modules::registry::info_for(&module.kind).unwrap();
                let p = info.params.iter().find(|p| p.name == "m1").unwrap();
                let at = (routing::base_value(state, 3, p) * 127.0).round() as u8;
                drop(c);
                let moved = if at < 107 { at + 20 } else { at - 20 };
                let start = std::time::Instant::now();
                tx.push(((0, 20, at), clock_s())).unwrap();
                tx.push(((0, 20, moved), clock_s())).unwrap();
                let deadline = start + std::time::Duration::from_millis(350);
                loop {
                    let rev = core.lock().unwrap().delivery.rev();
                    if rev > before && Feedback::get(&feedback.applied_rev) >= rev {
                        return start.elapsed();
                    }
                    assert!(
                        std::time::Instant::now() < deadline,
                        "CC did not reach callback within 350 ms during file I/O"
                    );
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
            };
        {
            let mut c = core.lock().unwrap();
            let Core { editor, ui, .. } = &mut *c;
            assert!(browser::save(editor, ui, None).is_none());
            assert!(browser::io_busy(ui));
        }
        let save_latency = send_and_measure(&core, &mut cc_tx);
        assert!(core.lock().unwrap().ui.browser.async_io);
        let save_deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            let mut c = core.lock().unwrap();
            if !browser::io_busy(&c.ui) {
                break;
            }
            assert!(
                std::time::Instant::now() < save_deadline,
                "Save worker did not finish"
            );
            let Core { editor, ui, .. } = &mut *c;
            browser::poll_io(editor, ui);
            drop(c);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        {
            let mut c = core.lock().unwrap();
            let Core { editor, ui, .. } = &mut *c;
            browser::perform(
                editor,
                ui,
                browser::Pending::Open("factory:palette/pad".into()),
            );
            assert!(browser::io_busy(ui));
            ui.takeover.clear();
        }
        let open_latency = send_and_measure(&core, &mut cc_tx);
        eprintln!(
            "CC to callback during 500 ms worker: Save {:.1} ms, Open {:.1} ms",
            save_latency.as_secs_f64() * 1e3,
            open_latency.as_secs_f64() * 1e3
        );
        assert!(save_latency.as_millis() < 350 && open_latency.as_millis() < 350);
        let open_deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            let mut c = core.lock().unwrap();
            if !browser::io_busy(&c.ui) {
                assert!(matches!(
                    c.ui.browser.dialog,
                    Some(browser::Dialog::Unsaved { .. })
                ));
                break;
            }
            assert!(
                std::time::Instant::now() < open_deadline,
                "Open worker did not finish"
            );
            let Core { editor, ui, .. } = &mut *c;
            browser::poll_io(editor, ui);
            drop(c);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        stop.store(true, Ordering::Release);
        pump_thread.thread().unpark();
        pump_thread.join().unwrap();
        drop(host._stream.take());
        drop(core);
        host.collector.collect();
    }

    #[test]
    fn callback_failure_sticks_and_xrun_alone_does_not_fail() {
        let timing = Arc::new(CallbackTiming::default());
        let gate = Arc::new(AtomicU64::new(u64::MAX));
        let mut p = Progress::new(timing.clone(), gate.clone());
        timing.xruns.store(3, Ordering::Relaxed);
        timing.count.store(8, Ordering::Relaxed);
        p.observe();
        assert_eq!(p.failure, None);
        p.at = std::time::Instant::now() - std::time::Duration::from_secs(2);
        p.observe();
        assert_eq!(p.failure, Some(AudioPhase::Stalled));
        assert_eq!(gate.load(Ordering::Relaxed), 0);
        timing.count.store(9, Ordering::Relaxed);
        p.observe();
        assert_eq!(p.failure, Some(AudioPhase::Stalled));
        gate.store(2, Ordering::Relaxed);
        p.observe();
        assert_eq!(
            gate.load(Ordering::Relaxed),
            0,
            "sticky failure closes a later gate too"
        );
    }

    #[test]
    fn stale_callback_cannot_reopen_a_failed_or_revised_gate() {
        let gate = AtomicU64::new(4);
        // Callback read revision 4; the detector failed or an edit became revision 5.
        gate.store(0, Ordering::Release);
        assert!(!unmute_if_current(&gate, 4));
        assert_eq!(gate.load(Ordering::Acquire), 0);
        gate.store(5, Ordering::Release);
        assert!(!unmute_if_current(&gate, 4));
        assert_eq!(gate.load(Ordering::Acquire), 5);
        assert!(unmute_if_current(&gate, 5));
        assert_eq!(gate.load(Ordering::Acquire), u64::MAX);
    }

    #[test]
    fn retry_boundary_discards_only_earlier_cc_arrivals() {
        let events = [((0, 24, 50), 1.0), ((0, 46, 127), 2.0), ((0, 24, 60), 3.0)];
        let mut fresh = Vec::new();
        let (dropped, earliest) = fresh_cc(&events, 2.0, &mut fresh);
        assert_eq!(dropped, 2);
        assert_eq!(earliest, 3.0);
        assert_eq!(fresh, vec![(0, 24, 60)]);
    }

    /// Opt-in cpal stream and callback test against an ALSA null PCM. No physical device.
    #[test]
    #[ignore = "set ALSA_CONFIG_PATH to an ALSA null PCM and run explicitly"]
    fn actual_backend_repeated_reopen_and_fault() {
        let patch = default_patch();
        let request = AudioRequest {
            rate: Some(48000),
            frames: Some(256),
            realtime: false,
            device: None,
        };
        let keys = Arc::new(AtomicU64::new(0));
        let peaks = Arc::new(record::PeakTap::default());
        let (_, rx) = rtrb::RingBuffer::new(1024);
        let mut invalid = request.clone();
        invalid.device = Some((usize::MAX, "missing".into()));
        let (unavailable, _) =
            AudioHost::start(&patch, rx, keys.clone(), &invalid, true, peaks.clone());
        assert!(unavailable._stream.is_none());
        let mut old = AudioHost::offline(Collector::new(), "offline".into());
        for (fail, delay) in [(true, 20), (false, 0), (false, 0)] {
            let (notes, rx) = rtrb::RingBuffer::new(1024);
            let at = std::time::Instant::now();
            let old_delivery = old.offline_delivery(&patch);
            let mut done = retry_worker(
                (old, old_delivery),
                notes,
                rx,
                keys.clone(),
                request.clone(),
                peaks.clone(),
                RetryFault {
                    delay_ms: delay,
                    fail_reopen: fail,
                },
            );
            assert_eq!(done.retired_graphs, 0);
            assert!(at.elapsed().as_millis() >= delay as u128);
            assert_eq!(done.audio._stream.is_some(), !fail);
            if !fail {
                assert_eq!(
                    done.delivery.sync(&patch, true, true),
                    control::Outcome::Compiled
                );
                done.audio
                    .gate
                    .store(done.delivery.rev(), Ordering::Relaxed);
                std::thread::sleep(std::time::Duration::from_millis(120));
                assert!(
                    done.audio
                        .timing
                        .as_ref()
                        .unwrap()
                        .count
                        .load(Ordering::Relaxed)
                        > 0
                );
                assert_eq!(done.audio.gate.load(Ordering::Relaxed), u64::MAX);
            }
            drop(done.delivery);
            drop(done.notes);
            old = done.audio;
        }
        drop(old._stream.take());
        old.collector.collect();
        assert_eq!(old.collector.alloc_count(), 0);
    }

    /// Exercises the production cpal closure, including queue drain, MIDI, reports, gate,
    /// recording tap and timing, after two startup callbacks. Requires ALSA null PCM.
    #[test]
    #[ignore = "set ALSA_CONFIG_PATH to an ALSA null PCM and run explicitly"]
    fn production_callback_warmed_path_has_no_alloc_or_dealloc() {
        let patch = default_patch();
        let (mut notes, rx) = rtrb::RingBuffer::new(1024);
        let request = AudioRequest {
            rate: Some(48000),
            frames: Some(256),
            realtime: false,
            device: None,
        };
        let (mut host, mut delivery) = AudioHost::start(
            &patch,
            rx,
            Arc::new(AtomicU64::new(0)),
            &request,
            true,
            Arc::new(record::PeakTap::default()),
        );
        assert!(host._stream.is_some(), "{}", host.status);
        assert_eq!(
            delivery.sync(&patch, true, true),
            control::Outcome::Compiled
        );
        host.gate.store(delivery.rev(), Ordering::Relaxed);
        let dir = tempfile::tempdir().unwrap();
        host.recorder
            .as_mut()
            .unwrap()
            .start_at(dir.path().join("callback.wav"))
            .unwrap();
        notes
            .push(note_now(KeyEvent::On {
                note: 60,
                velocity: 96,
            }))
            .unwrap();
        // D06: expression through the timeline's queue in the same warmed callbacks.
        for e in [
            KeyEvent::Bend(12000),
            KeyEvent::Wheel(90),
            KeyEvent::Sustain(true),
        ] {
            notes.push(note_now(e)).unwrap();
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert_eq!(host.gate.load(Ordering::Relaxed), u64::MAX);
        drop(host._stream.take());
        assert!(host.timing.as_ref().unwrap().count.load(Ordering::Relaxed) > 2);
        assert_eq!(callback_allocations::ALLOCS.load(Ordering::Relaxed), 0);
        assert_eq!(callback_allocations::DEALLOCS.load(Ordering::Relaxed), 0);
        assert!(host.recorder.as_mut().unwrap().stop().is_some());
        drop(delivery);
        host.collector.collect();
    }

    #[test]
    #[ignore = "set ALSA_CONFIG_PATH to an ALSA null PCM and run explicitly"]
    fn actual_callback_fault_sticks_and_mutes() {
        std::env::set_var("KABL_AUDIO_FAULT", "stall");
        std::env::set_var("KABL_AUDIO_FAULT_AFTER", "3");
        let patch = default_patch();
        let (_, rx) = rtrb::RingBuffer::new(1024);
        let request = AudioRequest {
            rate: Some(48000),
            frames: Some(256),
            realtime: false,
            device: None,
        };
        let (mut host, delivery) = AudioHost::start(
            &patch,
            rx,
            Arc::new(AtomicU64::new(0)),
            &request,
            false,
            Arc::new(record::PeakTap::default()),
        );
        assert!(host._stream.is_some(), "{}", host.status);
        let mut progress = Progress::new(host.timing.as_ref().unwrap().clone(), host.gate.clone());
        std::thread::sleep(std::time::Duration::from_millis(120));
        progress.observe();
        assert!(progress.last >= 3);
        std::thread::sleep(std::time::Duration::from_millis(1600));
        progress.observe();
        assert_eq!(progress.failure, Some(AudioPhase::Stalled));
        assert_eq!(host.gate.load(Ordering::Relaxed), 0);
        drop(delivery);
        drop(host._stream.take());
        host.collector.collect();
        assert_eq!(host.collector.alloc_count(), 0);
        std::env::remove_var("KABL_AUDIO_FAULT");
        std::env::remove_var("KABL_AUDIO_FAULT_AFTER");
    }
}
