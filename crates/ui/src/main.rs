//! `kabl-ui` binary: the patchbay editor with live audio. Combines `PatchEditor` (patch editing
//! over the real op log) with the same `cpal`/`midir` wiring `kabl-standalone` uses, so editing a
//! patch here and hearing the result are the same window, not a separate player process.
//!
//! **Audio handoff is lock-free.** The audio callback owns the `PatchEngine` outright. The UI
//! thread compiles each edit, wraps it for deferred drop (`basedrop::Owned`) and pushes it into
//! a bounded `rtrb` queue (`SWAP_QUEUE`). The callback drains the queue and installs each graph
//! with `PatchEngine::receive_swap`, which carries state from the playing graph without
//! allocating. No mutex, so an edit can never silence a block. If the queue is full, `SwapSender`
//! keeps only the newest unsent graph and retries next frame (older unsent ones are superseded).
//! Retired graphs are freed on the UI thread by `Collector::collect()` every frame.

use basedrop::{Collector, Owned};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use kabl_core::{ModuleId, PatchState};
use kabl_engine::compile::compile;
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::{swap_channel, PatchEngine, SwapSender};
use kabl_engine::voice_allocator::VoiceAllocator;
use kabl_modules::builtins::{DelayLock, Transport};
use kabl_standalone::{
    apply_voice_event, default_patch, resolve_midi_message, RingBuffer, VoiceEvent,
    DEFAULT_VOICE_COUNT,
};
use kabl_ui::{record, show, PatchEditor, UiState};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const RING_CAPACITY: usize = BLOCK * 256;

/// Graphs in flight from the UI thread to the audio callback. The callback drains it every
/// callback, so it only fills if the audio thread stalls; see `AudioHost::flush`.
const SWAP_QUEUE: usize = 4;

/// Owns everything audio-related: the swap queue's producer, the stream, the MIDI connection,
/// and the `basedrop` collector that frees retired graphs. Lives inside `App`.
struct AudioHost {
    swap_tx: Option<SwapSender>,
    sample_rate: f32,
    collector: Collector,
    status: String,
    /// Each sequencer's playing step, published by the audio callback.
    steps_rx: Option<rtrb::Consumer<(ModuleId, usize)>>,
    /// Each clock's run state, published by the audio callback.
    clocks_rx: Option<rtrb::Consumer<(ModuleId, bool)>>,
    /// Each delay's lock state and target time, published by the audio callback.
    delays_rx: Option<rtrb::Consumer<(ModuleId, DelayLock, f32)>>,
    /// Transport commands to the audio callback (runtime only, never in the op log).
    transport_tx: Option<rtrb::Producer<(ModuleId, Transport)>>,
    recorder: Option<record::Recorder>,
    timing: Option<Arc<CallbackTiming>>,
    _stream: Option<cpal::Stream>,
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
    /// 0 not tried, 1 granted, 2 refused.
    rt: std::sync::atomic::AtomicU8,
    rt_error: std::sync::OnceLock<String>,
}

impl CallbackTiming {
    fn record(
        &self,
        took: std::time::Duration,
        since_last: Option<std::time::Duration>,
        frames: usize,
        sample_rate: f32,
    ) {
        let ns = took.as_nanos() as u64;
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
        self.worst_ns.fetch_max(ns, Ordering::Relaxed);
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
            "{} callbacks × {frames} frames at {sample_rate} Hz · run: worst {:.0} of {:.0} µs, \
             {} over half, {} late · arrival: worst {:.0} µs, {} late · {} xruns · {rt} \
             (first second: worst {:.0} µs)",
            get(&self.count),
            us(&self.worst_ns),
            fmax as f64 / sample_rate as f64 * 1e6,
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
            swap_tx: None,
            sample_rate: 48000.0,
            collector,
            status,
            steps_rx: None,
            clocks_rx: None,
            delays_rx: None,
            transport_tx: None,
            recorder: None,
            timing: None,
            _stream: None,
        }
    }

    /// `rate`/`frames`: a sample rate and a fixed callback size to ask the device for.
    fn start(
        patch: &PatchState,
        notes: rtrb::Consumer<VoiceEvent>,
        rate: Option<u32>,
        frames: Option<u32>,
        realtime: bool,
        peaks: Arc<record::PeakTap>,
    ) -> Self {
        let mut midi_consumer = notes;
        let collector = Collector::new();
        let handle = collector.handle();

        let host = cpal::default_host();
        let Some(device) = host.default_output_device() else {
            return Self::offline(
                collector,
                "no audio output device found -- editing works, playback won't".into(),
            );
        };
        let f32_at = |r: u32| {
            device.supported_output_configs().ok()?.find_map(|c| {
                (c.sample_format() == cpal::SampleFormat::F32)
                    .then(|| c.try_with_sample_rate(r))
                    .flatten()
            })
        };
        let config = match rate {
            Some(r) => f32_at(r),
            None => device
                .default_output_config()
                .ok()
                .filter(|c| c.sample_format() == cpal::SampleFormat::F32),
        };
        let Some(config) = config else {
            return Self::offline(
                collector,
                format!(
                    "no usable f32 audio output config{} -- editing works, playback won't",
                    rate.map_or(String::new(), |r| format!(" at {r} Hz"))
                ),
            );
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
            Err(err) => return Self::offline(collector, format!("patch failed to compile: {err}")),
        };

        let (swap_tx, mut swap_rx) = swap_channel(SWAP_QUEUE);
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

        let (mut steps_tx, steps_rx) = rtrb::RingBuffer::<(ModuleId, usize)>::new(256);
        let (mut clocks_tx, clocks_rx) = rtrb::RingBuffer::<(ModuleId, bool)>::new(64);
        let (mut delays_tx, delays_rx) = rtrb::RingBuffer::<(ModuleId, DelayLock, f32)>::new(64);
        let (transport_tx, mut transport_rx) = rtrb::RingBuffer::<(ModuleId, Transport)>::new(64);

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
                // Audio thread. No allocation, no locks: install queued graphs (state carry is
                // allocation-free), apply MIDI to every running graph, render.
                engine.drain_swaps(&mut swap_rx);
                while let Ok(event) = midi_consumer.pop() {
                    apply_voice_event(&mut engine, event);
                }
                while let Ok((id, t)) = transport_rx.pop() {
                    engine.transport(id, t);
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
                engine.seq_steps(|id, step| {
                    let _ = steps_tx.push((id, step));
                });
                engine.clocks(|id, running| {
                    let _ = clocks_tx.push((id, running));
                });
                engine.delays(|id, lock, ms| {
                    let _ = delays_tx.push((id, lock, ms));
                });

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
                t_err.error(&err);
                eprintln!("kabl-ui: audio stream error: {err}");
            },
            None,
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
        AudioHost {
            swap_tx: Some(swap_tx),
            sample_rate,
            collector,
            status,
            steps_rx: Some(steps_rx),
            clocks_rx: Some(clocks_rx),
            delays_rx: Some(delays_rx),
            transport_tx: Some(transport_tx),
            recorder: stream.is_some().then_some(recorder),
            timing: stream.is_some().then_some(timing),
            _stream: stream,
        }
    }

    /// UI-thread call: compiles `patch` and queues it for the audio thread. A `fresh` graph (a
    /// Load) carries no state from the playing one.
    fn rebuild(&mut self, patch: &PatchState, fresh: bool) {
        let mut new_patch = match compile(patch, self.sample_rate, DEFAULT_VOICE_COUNT) {
            Ok(p) => p,
            Err(err) => {
                self.status = format!("recompile failed: {err}");
                return;
            }
        };
        new_patch.fresh = fresh;
        if let Some(tx) = self.swap_tx.as_mut() {
            tx.send(Owned::new(&self.collector.handle(), new_patch));
        }
    }

    /// UI-thread, once per frame: retry a waiting graph and free retired ones.
    fn tick(&mut self) -> bool {
        let waiting = self.swap_tx.as_mut().is_some_and(|tx| tx.flush());
        self.collector.collect();
        waiting
    }
}

/// What the MIDI callback owns: note events to the audio thread, CC to the UI.
struct MidiSink {
    notes: rtrb::Producer<VoiceEvent>,
    cc: rtrb::Producer<(u8, u8, u8)>,
    allocator: VoiceAllocator,
    ctx: Option<egui::Context>,
}

/// The MIDI input connection. The sink outlives connections, so switching ports keeps the
/// queues; a switch releases every voice first so no note hangs.
struct Midi {
    sink: Arc<Mutex<MidiSink>>,
    conn: Option<midir::MidiInputConnection<Arc<Mutex<MidiSink>>>>,
    port: Option<String>,
    cc_rx: rtrb::Consumer<(u8, u8, u8)>,
    ports: Vec<String>,
    scanned: Option<std::time::Instant>,
    /// The port that disappeared while connected; reconnected when it comes back.
    lost: Option<String>,
}

fn input_names() -> Vec<String> {
    let Ok(m) = midir::MidiInput::new("kabl-ui-scan") else {
        return Vec::new();
    };
    m.ports()
        .iter()
        .filter_map(|p| m.port_name(p).ok())
        .collect()
}

impl Midi {
    fn new(notes: rtrb::Producer<VoiceEvent>) -> Self {
        let (cc, cc_rx) = rtrb::RingBuffer::new(1024);
        Midi {
            sink: Arc::new(Mutex::new(MidiSink {
                notes,
                cc,
                allocator: VoiceAllocator::new(DEFAULT_VOICE_COUNT),
                ctx: None,
            })),
            conn: None,
            port: None,
            cc_rx,
            ports: Vec::new(),
            scanned: None,
            lost: None,
        }
    }

    fn disconnect(&mut self) {
        if let Some(c) = self.conn.take() {
            c.close();
        }
        self.port = None;
        self.release_all();
    }

    /// Note-off to every MIDI voice, and a fresh allocator.
    fn release_all(&self) {
        let mut s = self.sink.lock().unwrap();
        for voice in 0..DEFAULT_VOICE_COUNT {
            let _ = s.notes.push(VoiceEvent::NoteOff { voice });
        }
        s.allocator = VoiceAllocator::new(DEFAULT_VOICE_COUNT);
    }

    fn connect(&mut self, name: &str) -> Result<(), String> {
        self.disconnect();
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
                    let Ok(mut s) = sink.lock() else { return };
                    let s = &mut *s;
                    if data.len() >= 3 && data[0] & 0xF0 == 0xB0 {
                        let _ = s.cc.push((data[0] & 0x0F, data[1], data[2]));
                        if let Some(ctx) = &s.ctx {
                            ctx.request_repaint();
                        }
                    } else if let Some(e) = resolve_midi_message(&mut s.allocator, data) {
                        let _ = s.notes.push(e);
                    }
                },
                self.sink.clone(),
            )
            .map_err(|e| e.to_string())?;
        eprintln!("kabl-ui: listening for MIDI on \"{name}\"");
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
                    eprintln!("kabl-ui: {e}");
                }
            }
            None => eprintln!("kabl-ui: no MIDI input matched {filter:?}"),
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
    for t in ui.takeover.values_mut() {
        *t = Default::default();
    }
}

struct App {
    editor: PatchEditor,
    ui_state: UiState,
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
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
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
                kabl_ui::record::meter_ui(ui, &mut self.ui_state.meter);
                ui.separator();
                ui.label(&self.audio.status);
                if let Some(t) = &self.audio.timing {
                    ui.separator();
                    let (short, full) = t.summary(self.audio.sample_rate);
                    ui.add(egui::Label::new(egui::RichText::new(short).small()).truncate())
                        .on_hover_text(full);
                }
            });
        });
        if let Some(rx) = self.audio.steps_rx.as_mut() {
            while let Ok((id, step)) = rx.pop() {
                self.ui_state.seq_steps.insert(id, step);
            }
        }
        if let Some(rx) = self.audio.clocks_rx.as_mut() {
            while let Ok((id, running)) = rx.pop() {
                self.ui_state.clock_running.insert(id, running);
            }
        }
        while let Ok(cc) = self.midi.cc_rx.pop() {
            self.ui_state.midi_cc.push(cc);
        }
        if let Some(pick) = self.ui_state.midi_select.take() {
            self.midi.lost = None;
            self.ui_state.midi_note = None;
            reset_pickup(&mut self.ui_state);
            let r = match pick {
                Some(name) => self.midi.connect(&name),
                None => {
                    self.midi.disconnect();
                    Ok(())
                }
            };
            if let Err(e) = r {
                self.ui_state.last_message = Some(e);
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
                    self.ui_state.midi_note =
                        Some(format!("\"{port}\" disconnected: its notes were released"));
                    reset_pickup(&mut self.ui_state);
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
                    self.ui_state.midi_note = Some(format!("\"{now}\" reconnected"));
                    reset_pickup(&mut self.ui_state);
                }
            }
        }
        if std::mem::take(&mut self.ui_state.all_notes_off) {
            self.midi.release_all();
            self.ui_state.last_message = Some("all MIDI voices released".into());
        }
        if let Some(t) = &self.audio.timing {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
            let n = t.count.load(Ordering::Relaxed);
            if n != self.watchdog.0 {
                self.watchdog = (n, std::time::Instant::now());
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
        self.ui_state.meter.update(p, bad, dt);
        self.ui_state.midi_inputs = self.midi.ports.clone();
        self.ui_state.midi_input = self.midi.port.clone();
        if let Some(rx) = self.audio.delays_rx.as_mut() {
            while let Ok((id, lock, ms)) = rx.pop() {
                self.ui_state.delay_status.insert(id, (lock, ms));
            }
        }
        if !self.ui_state.seq_steps.is_empty() || !self.ui_state.delay_status.is_empty() {
            // egui only repaints on input; the step light and readouts need frames of their own.
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(30));
        }
        show(&mut self.editor, &mut self.ui_state, ui);
        // Scripted real-input runs (xdotool) read the drawn targets from here.
        if let Some(path) = &self.hits_file {
            let text: String = self
                .ui_state
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
        for cmd in self.ui_state.transport.drain(..) {
            if let Some(tx) = self.audio.transport_tx.as_mut() {
                // Full only if the audio thread stalls; the click is then lost, not queued.
                let _ = tx.push(cmd);
            }
        }
        if let (Some((path, at)), Some(t)) = (self.stats_file.as_mut(), &self.audio.timing) {
            if at.elapsed().as_secs_f32() > 1.0 {
                *at = std::time::Instant::now();
                let _ = std::fs::write(
                    path,
                    format!(
                        "{} · {} live graph allocations · {} undo entries · {} MIDI notes held\n",
                        t.line(self.audio.sample_rate),
                        self.audio.collector.alloc_count(),
                        self.editor.log().entries().len(),
                        self.midi.sink.lock().map_or(0, |s| s.allocator.held())
                    ),
                );
            }
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(500));
        }
        if self.editor.take_dirty() {
            let fresh = std::mem::take(&mut self.ui_state.loaded);
            self.audio.rebuild(self.editor.state(), fresh);
        }
        if self.audio.tick() {
            ui.ctx().request_repaint();
        }
    }
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
    let mut ui_state = UiState::default();
    let editor = match flag("--patch") {
        Some(dir) => match kabl_core::load(std::path::Path::new(&dir)) {
            Ok(log) => {
                ui_state.patch_path = dir;
                PatchEditor::from_log(log)
            }
            Err(err) => {
                eprintln!("kabl-ui: failed to load patch from {dir}: {err:?}");
                std::process::exit(1);
            }
        },
        None => PatchEditor::seed_from(&default_patch()),
    };
    let size = flag("--size")
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some([w.parse().ok()?, h.parse().ok()?])
        })
        .unwrap_or([1440.0, 900.0]);
    let (notes_tx, notes_rx) = rtrb::RingBuffer::<VoiceEvent>::new(256);
    let mut midi = Midi::new(notes_tx);
    midi.connect_default(flag("--midi").as_deref());
    let num = |name: &str| flag(name).and_then(|v| v.parse::<u32>().ok());
    let peaks = Arc::new(record::PeakTap::default());
    let mut audio = AudioHost::start(
        editor.state(),
        notes_rx,
        num("--rate"),
        num("--frames"),
        !args.iter().any(|a| a == "--no-rt"),
        peaks.clone(),
    );
    ui_state.recorder = audio.recorder.take();
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
    eframe::run_native(
        "kabl",
        options,
        Box::new(|cc| {
            kabl_ui::theme::install_fonts(&cc.egui_ctx);
            midi.sink.lock().unwrap().ctx = Some(cc.egui_ctx.clone());
            Ok(Box::new(App {
                editor,
                ui_state,
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
            }))
        }),
    )
}
