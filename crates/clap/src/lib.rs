//! Production instrument. REAPER owns audio/MIDI; no device backend is started here.
mod sound_state;
mod state_bridge;
mod tail;

use basedrop::{Collector, Handle, Owned};
use kabl_engine::{
    keyboard::{MidiEvent, Source},
    patch_engine::PatchEngine,
    runtime::{Feedback, ToAudio},
    timeline::{Timeline, LATENCY},
};
use kabl_ui::{control::Delivery, PatchEditor, UiState};
use nice_plug::{context::gui::GuiContext, editor::dpi::LogicalSize, prelude::*};
use nice_plug_egui::{
    create_egui_editor, EguiEditor, EguiNiceSettings, EguiState, NiceEguiApp, RepaintNotifier,
};
use sound_state::SoundState;
use std::{
    num::NonZeroU32,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread::JoinHandle,
};

const VOICES: usize = 8;
type PreparedSession = (
    Owned<Session>,
    Delivery,
    rtrb::Consumer<(u8, u8, u8)>,
    rtrb::Consumer<RuntimeReport>,
);
// Fixed 256-slot queue (~140 KiB). Inline probe snapshots keep audio allocation-free.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Copy)]
enum RuntimeReport {
    Clock(u64, bool),
    Seq(u64, usize, usize, Option<usize>),
    Delay(u64, kabl_modules::builtins::DelayLock, f32),
    Lfo(u64, kabl_modules::builtins::LfoSync),
    Probe(kabl_engine::probe::ProbeReport),
}

#[derive(Params)]
struct InstrumentParams {
    #[id = "output_gain"]
    gain: FloatParam,
}

/// A state load replaces this entire session, including the old command queue. Destruction
/// of the old session (graphs, queue storage and feedback Arc) is deferred, never on audio.
struct Session {
    engine: PatchEngine,
    rx: rtrb::Consumer<ToAudio>,
    feedback: Arc<Feedback>,
    epoch: u64,
    gain: f32,
    cc: rtrb::Producer<(u8, u8, u8)>,
    reports: rtrb::Producer<RuntimeReport>,
}

struct Control {
    patch: kabl_core::PatchState,
    rejected: Option<kabl_core::PatchState>,
    editor: PatchEditor,
    view: UiState,
    delivery: Delivery,
    handle: Handle,
    rate: f32,
    tx: rtrb::Producer<Owned<Session>>,
    epoch: u64,
    cc: rtrb::Consumer<(u8, u8, u8)>,
    reports: rtrb::Consumer<RuntimeReport>,
    // Last: all queued Owned values and Handles must drop before collector storage.
    collector: CollectorGuard,
}

struct CollectorGuard(Option<Collector>);
impl CollectorGuard {
    fn collect(&mut self) {
        self.0.as_mut().unwrap().collect();
    }
    fn cleanup(&mut self) -> bool {
        let Some(mut collector) = self.0.take() else {
            return true;
        };
        collector.collect();
        match collector.try_cleanup() {
            Ok(()) => true,
            Err(collector) => {
                self.0 = Some(collector);
                false
            }
        }
    }
}
impl Drop for CollectorGuard {
    fn drop(&mut self) {
        if !self.cleanup() {
            eprintln!("kabl: collector still has outstanding ownership at shutdown");
        }
    }
}

impl Control {
    /// Only a successfully prepared document becomes persistent sound state. Metadata-only
    /// edits also pass through sync; a failed document never becomes accepted on the next tick.
    fn receive_reports(&mut self, now: f64, sample_clock: u64) {
        self.view.inspect.audio = true;
        self.view.inspect.rebuilt(
            self.delivery.generation,
            self.delivery.compile_error.clone(),
            self.editor.state(),
            now,
        );
        for _ in 0..256 {
            let Ok(report) = self.reports.pop() else {
                break;
            };
            match report {
                RuntimeReport::Clock(id, running) => {
                    self.view.clock_running.insert(id, running);
                }
                RuntimeReport::Seq(id, step, bank, queued) => {
                    self.view.seq_steps.insert(id, step);
                    self.view.seq_banks.insert(id, (bank, queued));
                }
                RuntimeReport::Delay(id, lock, ms) => {
                    self.view.delay_status.insert(id, (lock, ms));
                }
                RuntimeReport::Lfo(id, sync) => {
                    self.view.lfo_status.insert(id, sync);
                }
                RuntimeReport::Probe(report) => {
                    let age =
                        sample_clock.saturating_sub(report.end_sample) as f64 / self.rate as f64;
                    self.view.inspect.accept(report, now, age);
                }
            }
        }
    }
    fn clear_reports(&mut self) {
        self.view.clock_running.clear();
        self.view.seq_steps.clear();
        self.view.seq_banks.clear();
        self.view.delay_status.clear();
        self.view.lfo_status.clear();
        self.view.inspect.reset_audio_session(0.0);
    }
    fn pump(&mut self) -> bool {
        let changed = self.patch != *self.editor.state();
        if changed && self.rejected.as_ref() != Some(self.editor.state()) {
            if let Err(error) = sound_state::validate_patch(self.editor.state()) {
                self.rejected = Some(self.editor.state().clone());
                self.editor.take_dirty();
                self.view.last_message = Some(error);
                self.delivery.flush();
                return false;
            }
            // Standalone deliberately permits existing runtime controls after a failed graph.
            // Plugin recall requires the entire candidate to compile before accepting it.
            if self.rejected.is_some() {
                if let Err(error) =
                    kabl_engine::compile::compile(self.editor.state(), self.rate, VOICES)
                {
                    self.rejected = Some(self.editor.state().clone());
                    self.editor.take_dirty();
                    self.view.last_message = Some(error.to_string());
                    self.delivery.flush();
                    return false;
                }
            }
            self.editor.mark_dirty();
        } else if changed {
            self.editor.take_dirty();
            self.delivery.flush();
            return false;
        }
        let out = kabl_ui::control::deliver(&mut self.editor, &mut self.view, &mut self.delivery);
        if out == kabl_ui::control::Outcome::Failed {
            self.rejected = Some(self.editor.state().clone());
            return false;
        }
        if changed {
            self.patch = self.editor.state().clone();
            self.rejected = None;
        }
        changed
    }
}

fn prepare(
    patch: &kabl_core::PatchState,
    rate: f32,
    handle: &Handle,
    epoch: u64,
    gain: f32,
) -> Result<PreparedSession, String> {
    sound_state::validate_patch(patch)?;
    let mut engine = PatchEngine::new(handle, patch, rate, VOICES).map_err(|e| e.to_string())?;
    engine.reset();
    let (tx, rx) = rtrb::RingBuffer::new(kabl_ui::control::QUEUE);
    let feedback = Arc::new(Feedback::default());
    let delivery = Delivery::new(
        Some(tx),
        handle.clone(),
        feedback.clone(),
        rate,
        VOICES,
        Some(patch),
    );
    let (cc_tx, cc_rx) = rtrb::RingBuffer::new(256);
    let (reports_tx, reports_rx) = rtrb::RingBuffer::new(256);
    Ok((
        Owned::new(
            handle,
            Session {
                engine,
                rx,
                feedback,
                epoch,
                gain,
                cc: cc_tx,
                reports: reports_tx,
            },
        ),
        delivery,
        cc_rx,
        reports_rx,
    ))
}

struct Shared {
    control: Mutex<Control>,
    params: Arc<InstrumentParams>,
    /// Published after the complete transaction (queue, document, parameter) is ready.
    committed: AtomicU64,
    stop: AtomicBool,
    host: AtomicU64,
    dirty: AtomicBool,
    audio_clock: AtomicU64,
    tail_samples: AtomicU64,
    host_frames: AtomicU32,
    midi_overflow: AtomicBool,
}

impl Shared {
    fn snapshot(&self) -> SoundState {
        let c = self.control.lock().unwrap();
        SoundState {
            version: 1,
            patch: c.patch.clone(),
            output_gain: self.params.gain.unmodulated_plain_value(),
        }
    }

    /// Main-thread CLAP state load. All validation, compilation and queue checks precede
    /// mutation. Session publication is the single audible transaction boundary.
    fn load(&self, state: SoundState) -> Result<(), String> {
        state.validate()?;
        let mut c = self.control.lock().unwrap();
        if c.tx.slots() == 0 {
            return Err("State queue full; previous sound retained".into());
        }
        let epoch = c.epoch + 1;
        let (session, delivery, cc, reports) =
            prepare(&state.patch, c.rate, &c.handle, epoch, state.output_gain)?;
        let editor = PatchEditor::seed_from(&state.patch);
        c.tx.push(session)
            .map_err(|_| "State queue full".to_string())?;
        self.committed.store(epoch * 2 - 1, Ordering::SeqCst);
        c.delivery = delivery;
        c.cc = cc;
        c.reports = reports;
        c.clear_reports();
        c.patch = state.patch;
        c.editor = editor;
        c.rejected = None;
        // Keep library and view preferences; discard commands, keys and CC pickup.
        c.view.loaded = false;
        c.view.launches.clear();
        c.view.transport.clear();
        c.view.takeover.clear();
        c.view.doc = None;
        c.epoch = epoch;
        self.tail_samples
            .store(tail::samples(&c.patch, c.rate), Ordering::Relaxed);
        // No wrapper persistent-field loader or activation here. Pointer remains alive via Arc.
        unsafe {
            self.params
                .gain
                .as_ptr()
                ._internal_set_normalized_value(state.output_gain);
        }
        self.committed.store(epoch * 2, Ordering::SeqCst);
        Ok(())
    }
}

pub struct RackApp {
    shared: Arc<Shared>,
    gui: Option<GuiContext>,
}
impl NiceEguiApp for RackApp {
    fn build(
        &mut self,
        _ctx: egui::Context,
        gui: GuiContext,
        _frame: &mut nice_plug_egui::Frame,
    ) -> Result<(), nice_plug_egui::baseview::HandlerError> {
        self.gui = Some(gui);
        Ok(())
    }
    fn editor_closed(&mut self) {
        self.gui = None;
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut nice_plug_egui::Frame) {
        let old = self.shared.params.gain.unmodulated_plain_value();
        let mut gain = old;
        ui.horizontal(|ui| {
            ui.label("kabl");
            ui.add(egui::Slider::new(&mut gain, 0.0..=1.0).text("Output gain"));
        });
        if gain != old {
            if let Some(gui) = &self.gui {
                let setter = gui.param_setter();
                setter.begin_set_parameter(&self.shared.params.gain);
                setter.set_parameter(&self.shared.params.gain, gain);
                setter.end_set_parameter(&self.shared.params.gain);
            }
        }
        let mut c = self.shared.control.lock().unwrap();
        if c.view.library.is_none() {
            c.view.library = Some(kabl_ui::browser::open_library());
        }
        let Control { editor, view, .. } = &mut *c;
        kabl_ui::show(editor, view, ui);
        // Delivery/compile/retry and collection run on the control worker, also when closed.
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(16));
    }
}

pub struct Instrument {
    shared: Arc<Shared>,
    loads: rtrb::Consumer<Owned<Session>>,
    midi: rtrb::Consumer<(u32, [u8; 3])>,
    pending: Option<Owned<Session>>,
    session: Option<Owned<Session>>,
    timeline: Timeline,
    rate: f32,
    worker: Option<JoinHandle<()>>,
    editor_state: Arc<EguiState>,
    repaint: RepaintNotifier,
}
impl Default for Instrument {
    fn default() -> Self {
        let patch = kabl_standalone::default_patch();
        let collector = Collector::new();
        let handle = collector.handle();
        let (_, delivery, cc, reports) =
            prepare(&patch, 48000.0, &handle, 0, 1.0).expect("factory patch");
        let (tx, loads) = rtrb::RingBuffer::new(2);
        let (midi_tx, midi) = rtrb::RingBuffer::new(1024);
        let shared = Arc::new(Shared {
            control: Mutex::new(Control {
                editor: PatchEditor::seed_from(&patch),
                view: UiState::default(),
                patch,
                rejected: None,
                delivery,
                collector: CollectorGuard(Some(collector)),
                handle,
                rate: 48000.0,
                tx,
                epoch: 0,
                cc,
                reports,
            }),
            params: Arc::new(InstrumentParams {
                gain: FloatParam::new(
                    "Output gain",
                    1.0,
                    FloatRange::Linear { min: 0.0, max: 1.0 },
                ),
            }),
            committed: AtomicU64::new(0),
            stop: AtomicBool::new(false),
            host: AtomicU64::new(0),
            dirty: AtomicBool::new(false),
            audio_clock: AtomicU64::new(0),
            host_frames: AtomicU32::new(0),
            midi_overflow: AtomicBool::new(false),
            tail_samples: AtomicU64::new(tail::samples(&kabl_standalone::default_patch(), 48000.0)),
        });
        state_bridge::capture(&shared, midi_tx);
        let worker_shared = shared.clone();
        let worker = std::thread::Builder::new()
            .name("kabl-plugin-control".into())
            .spawn(move || {
                let start = std::time::Instant::now();
                while !worker_shared.stop.load(Ordering::Acquire) {
                    {
                        let mut c = worker_shared.control.lock().unwrap();
                        let mut events = Vec::new();
                        for _ in 0..256 {
                            if let Ok(cc) = c.cc.pop() {
                                events.push(cc);
                            } else {
                                break;
                            }
                        }
                        let Control { editor, view, .. } = &mut *c;
                        kabl_ui::perform::apply_cc(
                            editor,
                            view,
                            &events,
                            start.elapsed().as_secs_f64(),
                        );
                        c.receive_reports(
                            start.elapsed().as_secs_f64(),
                            worker_shared.audio_clock.load(Ordering::Relaxed),
                        );
                        if c.pump() {
                            worker_shared
                                .tail_samples
                                .store(tail::samples(&c.patch, c.rate), Ordering::Relaxed);
                            worker_shared.dirty.store(true, Ordering::Release);
                            state_bridge::request_main(&worker_shared);
                        }
                        c.collector.collect();
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            })
            .expect("plugin control worker");
        Self {
            shared,
            loads,
            midi,
            pending: None,
            session: None,
            timeline: Timeline::new(),
            rate: 48000.0,
            worker: Some(worker),
            editor_state: EguiState::from_size(LogicalSize::new(1180.0, 740.0), 1.0),
            repaint: RepaintNotifier::new(),
        }
    }
}

impl Instrument {
    /// Bounded loads. An unpublished session waits without changing graph or gain.
    fn accept_loads(&mut self) {
        for _ in 0..2 {
            if self.pending.is_none() {
                self.pending = self.loads.pop().ok();
            }
            let ready = self.pending.as_ref().is_some_and(|s| {
                self.shared.committed.load(Ordering::SeqCst) >= s.epoch * 2
                    && self
                        .shared
                        .committed
                        .load(Ordering::SeqCst)
                        .is_multiple_of(2)
            });
            if !ready {
                break;
            }
            self.session = self.pending.take();
            self.timeline.reset();
        }
    }
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let Some(s) = &mut self.session else {
            left.fill(0.0);
            right.fill(0.0);
            return;
        };
        let s: &mut Session = s;
        // A state transaction in flight must not apply its new parameter to the old graph.
        let epoch = self.shared.committed.load(Ordering::SeqCst);
        if epoch == s.epoch * 2 {
            let gain = self.shared.params.gain.value();
            if self.shared.committed.load(Ordering::SeqCst) == epoch {
                s.gain = gain;
            }
        }
        s.engine
            .drain(&mut s.rx, kabl_ui::control::QUEUE, &s.feedback);
        self.timeline.render(&mut s.engine, left, right, |_, _| {});
        self.shared
            .audio_clock
            .store(s.engine.rendered_samples(), Ordering::Relaxed);
        let reports = &mut s.reports;
        s.engine.clocks(|id, running| {
            let _ = reports.push(RuntimeReport::Clock(id, running));
        });
        s.engine.seqs(|id, step, bank, queued| {
            let _ = reports.push(RuntimeReport::Seq(id, step, bank, queued));
        });
        s.engine.delays(|id, lock, ms| {
            let _ = reports.push(RuntimeReport::Delay(id, lock, ms));
        });
        s.engine.lfos(|id, sync| {
            let _ = reports.push(RuntimeReport::Lfo(id, sync));
        });
        if let Some(report) = s.engine.take_probe_report() {
            let _ = reports.push(RuntimeReport::Probe(report));
        }
        for (l, r) in left.iter_mut().zip(right) {
            *l *= s.gain;
            *r *= s.gain;
        }
    }
}
impl Plugin for Instrument {
    const NAME: &'static str = "kabl";
    const VENDOR: &'static str = "stcksmsh";
    const URL: &'static str = "https://github.com/stcksmsh/kabl";
    const EMAIL: &'static str = "kabl@example.invalid";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];
    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    type Editor = EguiEditor<RackApp>;
    type SysExMessage = ();
    type BackgroundTask = ();
    fn params(&self) -> Arc<dyn Params> {
        self.shared.params.clone()
    }
    fn editor(&mut self, _executor: AsyncExecutor<Self>) -> Option<Self::Editor> {
        create_egui_editor(
            self.editor_state.clone(),
            self.repaint.clone(),
            EguiNiceSettings::new(),
            RackApp {
                shared: self.shared.clone(),
                gui: None,
            },
        )
    }
    fn activate(
        &mut self,
        _layout: &AudioIOLayout,
        config: &BufferConfig,
        context: &mut impl ActivateContext<Self>,
    ) -> bool {
        if !config.sample_rate.is_finite() || !(8000.0..=192000.0).contains(&config.sample_rate) {
            return false;
        }
        let mut c = self.shared.control.lock().unwrap();
        let epoch = c.epoch;
        let gain = self.shared.params.gain.unmodulated_plain_value();
        let Ok((session, delivery, cc, reports)) =
            prepare(&c.patch, config.sample_rate, &c.handle, epoch, gain)
        else {
            return false;
        };
        c.rate = config.sample_rate;
        self.shared
            .tail_samples
            .store(tail::samples(&c.patch, c.rate), Ordering::Relaxed);
        self.rate = config.sample_rate;
        c.view.sample_rate = c.rate;
        c.delivery = delivery;
        c.cc = cc;
        c.reports = reports;
        c.clear_reports();
        self.session = Some(session);
        self.pending = None;
        while let Ok(s) = self.loads.pop() {
            drop(s);
        }
        c.collector.collect();
        self.timeline.reset();
        context.set_latency_samples(LATENCY as u32);
        true
    }
    fn reset(&mut self) {
        self.accept_loads();
        if let Some(s) = &mut self.session {
            let s: &mut Session = s;
            s.engine
                .drain(&mut s.rx, kabl_ui::control::QUEUE, &s.feedback);
            s.engine.reset();
        }
        self.timeline.reset();
        for _ in 0..1024 {
            if self.midi.pop().is_err() {
                break;
            }
        }
        self.shared.midi_overflow.store(false, Ordering::Relaxed);
    }
    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.accept_loads();
        // Raw CLAP MIDI bypasses the framework's unbounded VecDeque. The outer callback
        // gives full host-buffer offsets; Timeline retains them across automation subblocks.
        let frames = self.shared.host_frames.load(Ordering::Relaxed) as usize;
        for _ in 0..1024 {
            let Ok((offset, bytes)) = self.midi.pop() else {
                break;
            };
            if bytes[0] & 0xF0 == 0xB0 && !matches!(bytes[1], 64 | 120 | 121 | 123) {
                if let Some(s) = &mut self.session {
                    let _ = s.cc.push((bytes[0] & 15, bytes[1], bytes[2]));
                }
            }
            if let Some(event) = MidiEvent::parse(Source::Host, &bytes) {
                self.timeline.push(offset as usize, frames, event);
            }
        }
        if self.shared.midi_overflow.swap(false, Ordering::Relaxed) {
            self.timeline.push(
                frames.saturating_sub(1),
                frames,
                MidiEvent::new(Source::Host, 0, kabl_engine::keyboard::KeyEvent::SourceLost),
            );
        }
        // Only bounded parameter events reach the wrapper. No native-note dialect advertised.
        while context.next_event().is_some() {}
        let channels = buffer.as_slice();
        if channels.len() >= 2 {
            let (left, right) = channels.split_at_mut(1);
            self.render(left[0], right[0]);
        }
        match self.shared.tail_samples.load(Ordering::Relaxed) {
            u64::MAX => ProcessStatus::KeepAlive,
            samples => ProcessStatus::Tail(samples.min(u32::MAX as u64 - 1) as u32),
        }
    }
    fn deactivate(&mut self) {
        self.session = None;
        self.pending = None;
        while let Ok(s) = self.loads.pop() {
            drop(s);
        }
        self.shared.control.lock().unwrap().collector.collect();
        self.timeline.reset();
    }
}
impl Drop for Instrument {
    fn drop(&mut self) {
        self.session = None;
        self.pending = None;
        while let Ok(s) = self.loads.pop() {
            drop(s);
        }
        self.shared.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        let mut c = self
            .shared
            .control
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        c.delivery = Delivery::new(
            None,
            c.handle.clone(),
            Arc::new(Feedback::default()),
            c.rate,
            VOICES,
            None,
        );
        c.collector.collect();
    }
}
#[cfg(test)]
fn midi_event(event: NoteEvent<()>) -> Option<MidiEvent> {
    let bytes = match event {
        NoteEvent::NoteOn {
            channel,
            note,
            velocity,
            ..
        } if channel < 16 && note < 128 => [
            0x90 | channel,
            note,
            (velocity * 127.0).round().clamp(0.0, 127.0) as u8,
        ],
        NoteEvent::NoteOff { channel, note, .. } if channel < 16 && note < 128 => {
            [0x80 | channel, note, 0]
        }
        NoteEvent::MidiCC {
            channel, cc, value, ..
        } if channel < 16 && cc < 128 => [
            0xB0 | channel,
            cc,
            (value * 127.0).round().clamp(0.0, 127.0) as u8,
        ],
        NoteEvent::MidiPitchBend { channel, value, .. } if channel < 16 => {
            let bend = (value * 16383.0).round().clamp(0.0, 16383.0) as u16;
            [0xE0 | channel, (bend & 127) as u8, (bend >> 7) as u8]
        }
        _ => return None,
    };
    MidiEvent::parse(Source::Host, &bytes)
}
impl ClapPlugin for Instrument {
    const CLAP_ID: &'static str = "dev.stcksmsh.kabl";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("kabl modular instrument");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::Instrument, ClapFeature::Stereo];
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_no_alloc::{assert_no_alloc, AllocDisabler};
    #[global_allocator]
    static ALLOC: AllocDisabler = AllocDisabler;

    fn instrument() -> Instrument {
        let mut p = Instrument::default();
        p.shared.stop.store(true, Ordering::Release);
        p.worker.take().unwrap().join().unwrap();
        let mut c = p.shared.control.lock().unwrap();
        let (session, delivery, cc, reports) =
            prepare(&c.patch, 48000.0, &c.handle, 0, 1.0).unwrap();
        c.delivery = delivery;
        c.cc = cc;
        c.reports = reports;
        c.clear_reports();
        p.session = Some(session);
        drop(c);
        p
    }
    #[test]
    fn state_rejection_is_atomic_even_when_queue_is_full() {
        let mut p = instrument();
        for gain in [0.2, 0.4] {
            let mut state = p.shared.snapshot();
            state.output_gain = gain;
            state
                .patch
                .modules
                .get_mut(&3)
                .unwrap()
                .params
                .insert("cutoff_hz".into(), gain * 10000.0);
            p.shared.load(state).unwrap();
        }
        let original = serde_json::to_vec(&p.shared.snapshot()).unwrap();
        let mut state = p.shared.snapshot();
        state.output_gain = 0.9;
        assert!(p.shared.load(state).is_err());
        assert_eq!(serde_json::to_vec(&p.shared.snapshot()).unwrap(), original);
        p.accept_loads();
        let mut bad = p.shared.snapshot();
        bad.output_gain = 0.9;
        bad.patch.modules.get_mut(&2).unwrap().kind = "bad.module".into();
        assert!(p.shared.load(bad).is_err());
        assert_eq!(serde_json::to_vec(&p.shared.snapshot()).unwrap(), original);
        assert_eq!(p.session.as_ref().unwrap().gain, 0.4);
        let patch = p.shared.snapshot().patch;
        assert_eq!(p.shared.control.lock().unwrap().editor.state(), &patch);
    }
    #[test]
    fn edits_preserve_metadata_and_reject_invalid_documents_until_repaired() {
        let p = instrument();
        let mut c = p.shared.control.lock().unwrap();
        c.editor.set_label(2, "title", Some("Recall me".into()));
        assert!(c.pump());
        assert_eq!(c.patch.label(2, "title"), Some("Recall me"));
        let accepted = c.patch.clone();
        let id = c
            .editor
            .add_module("bad.module", kabl_core::Vec2::default());
        assert!(!c.pump());
        assert!(!c.pump());
        assert_eq!(c.patch, accepted);
        c.editor.set_param(2, "base_hz", 880.0);
        assert!(
            !c.pump(),
            "a knob edit must not accept the same failed topology"
        );
        assert_eq!(c.patch, accepted);
        c.editor.remove_module(id);
        c.editor.set_label(2, "title", Some("Repaired".into()));
        assert!(c.pump());
        assert_eq!(c.patch.label(2, "title"), Some("Repaired"));
        let accepted = c.patch.clone();
        for _ in 0..9 {
            c.editor.add_module("delay", kabl_core::Vec2::default());
        }
        assert!(!c.pump());
        assert!(!c.pump());
        assert_eq!(c.patch, accepted);
    }
    #[test]
    fn collector_storage_is_released_after_retired_values_and_handles() {
        let mut guard = CollectorGuard(Some(Collector::new()));
        let handle = guard.0.as_ref().unwrap().handle();
        let value = Owned::new(&handle, vec![1u8; 4096]);
        assert!(!guard.cleanup());
        drop(value);
        drop(handle);
        assert!(guard.cleanup());
        assert!(guard.0.is_none());
    }
    #[test]
    fn runtime_reports_update_closed_editor_and_are_cleared_on_load() {
        let mut p = instrument();
        let mut state = p.shared.snapshot();
        let mut editor = PatchEditor::seed_from(&state.patch);
        let clock = editor.add_module("clock", kabl_core::Vec2::default());
        state.patch = editor.state().clone();
        p.shared.load(state).unwrap();
        p.accept_loads();
        audio(&mut p, 256, false);
        {
            let mut c = p.shared.control.lock().unwrap();
            c.receive_reports(0.0, p.shared.audio_clock.load(Ordering::Relaxed));
            assert!(c.view.clock_running.contains_key(&clock));
            c.delivery
                .transport((clock, kabl_modules::builtins::Transport::Stop));
            c.delivery.flush();
        }
        audio(&mut p, 256, false);
        {
            let mut c = p.shared.control.lock().unwrap();
            c.receive_reports(0.01, p.shared.audio_clock.load(Ordering::Relaxed));
            assert!(!c.view.clock_running[&clock]);
        }
        let state = p.shared.snapshot();
        p.shared.load(state).unwrap();
        assert!(p
            .shared
            .control
            .lock()
            .unwrap()
            .view
            .clock_running
            .is_empty());
    }
    #[test]
    fn tail_covers_long_effects_and_keeps_ungated_racks_alive() {
        let patch = kabl_standalone::default_patch();
        assert!(tail::samples(&patch, 48000.0) < 8 * 48000);
        let mut editor = PatchEditor::seed_from(&patch);
        editor.add_module("reverb", kabl_core::Vec2::default());
        let mut long = editor.state().clone();
        long.modules
            .values_mut()
            .find(|m| m.kind == "reverb")
            .unwrap()
            .params
            .insert("decay_s".into(), 30.0);
        assert!(tail::samples(&long, 48000.0) > 120 * 48000);
        let mut latched = patch.clone();
        latched
            .modules
            .get_mut(&4)
            .unwrap()
            .params
            .insert("timing".into(), 1.0);
        latched
            .modules
            .get_mut(&4)
            .unwrap()
            .params
            .insert("release_ms".into(), 0.1);
        assert!(tail::samples(&latched, 48000.0) > 100 * 48000);
        let mut ungated = patch;
        ungated
            .modules
            .get_mut(&5)
            .unwrap()
            .params
            .insert("gain".into(), 1.0);
        assert_eq!(tail::samples(&ungated, 48000.0), u64::MAX);
        let mut modulation = kabl_standalone::default_patch();
        modulation.cables.retain(|_, c| c.to.module_id() != 6);
        modulation.cables.insert(
            100,
            kabl_core::CableState {
                from: kabl_core::PortRef::Module {
                    id: 5,
                    port: "out".into(),
                },
                to: kabl_core::PortRef::Param {
                    id: 2,
                    param: "base_hz".into(),
                },
                params: Default::default(),
                steps: vec![],
            },
        );
        modulation.cables.insert(
            101,
            kabl_core::CableState {
                from: kabl_core::PortRef::Module {
                    id: 2,
                    port: "out".into(),
                },
                to: kabl_core::PortRef::Module {
                    id: 6,
                    port: "left".into(),
                },
                params: Default::default(),
                steps: vec![],
            },
        );
        assert_eq!(tail::samples(&modulation, 48000.0), u64::MAX);
    }
    #[test]
    fn identity_overflow_is_rejected_before_state_mutation() {
        let p = instrument();
        let before = serde_json::to_vec(&p.shared.snapshot()).unwrap();
        let mut state = p.shared.snapshot();
        let module = state.patch.modules.remove(&2).unwrap();
        state.patch.modules.insert(u64::MAX, module);
        assert!(p.shared.load(state).is_err());
        assert_eq!(serde_json::to_vec(&p.shared.snapshot()).unwrap(), before);
    }
    #[test]
    fn state_decode_bounds_version_truncation_and_nonfinite() {
        let p = instrument();
        let bytes = serde_json::to_vec(&p.shared.snapshot()).unwrap();
        assert!(SoundState::decode(&bytes).is_ok());
        for data in [&b"invalid"[..], &bytes[..bytes.len() - 1], &[]] {
            assert!(SoundState::decode(data).is_err());
        }
        assert!(SoundState::decode(&vec![b' '; sound_state::MAX_STATE_BYTES + 1]).is_err());
        let mut state = p.shared.snapshot();
        state.version = 2;
        assert!(p.shared.load(state).is_err());
        let mut state = p.shared.snapshot();
        state
            .patch
            .modules
            .get_mut(&2)
            .unwrap()
            .params
            .insert("base_hz".into(), f32::INFINITY);
        assert!(p.shared.load(state).is_err());
    }
    fn audio(p: &mut Instrument, frames: usize, note: bool) -> Vec<f32> {
        let mut left = vec![0.0; frames];
        let mut right = vec![0.0; frames];
        if note {
            p.timeline.push(
                17,
                frames,
                MidiEvent::parse(Source::Host, &[0x91, 60, 100]).unwrap(),
            );
        }
        assert_no_alloc(|| p.render(&mut left, &mut right));
        left
    }
    #[test]
    fn reset_repeats_sound_clears_sustain_and_is_allocation_free() {
        let mut p = instrument();
        let first = audio(&mut p, 48000, true);
        p.timeline.push(
            0,
            256,
            MidiEvent::parse(Source::Host, &[0xB1, 64, 127]).unwrap(),
        );
        audio(&mut p, 256, false);
        assert_no_alloc(|| p.reset());
        assert_eq!(audio(&mut p, 48000, true), first);
        assert_no_alloc(|| p.reset());
        assert!(audio(&mut p, 48000, false).iter().all(|&s| s == 0.0));
    }
    #[test]
    fn state_load_replaces_old_commands_and_callback_ownership_without_allocations() {
        let mut p = instrument();
        p.shared.control.lock().unwrap().delivery.command(
            kabl_engine::patch_engine::Command::Preview {
                notes: [60, 64, 67, 0],
                count: 3,
                velocity: 100,
                blocks: 750,
            },
        );
        let state = p.shared.snapshot();
        p.shared.load(state).unwrap();
        assert_no_alloc(|| p.accept_loads());
        assert!(audio(&mut p, 48000, false).iter().all(|&s| s == 0.0));
        assert_no_alloc(|| p.reset());
    }
    #[test]
    fn host_midi_keeps_channel_source_bend_wheel_and_release() {
        for bytes in [
            [0x92, 60, 100],
            [0x82, 60, 0],
            [0xB2, 1, 127],
            [0xB2, 64, 127],
            [0xB2, 64, 0],
            [0xB2, 121, 0],
            [0xB2, 123, 0],
            [0xE2, 0, 64],
            [0xE2, 127, 127],
        ] {
            let expected = MidiEvent::parse(Source::Host, &bytes).unwrap();
            let event = match bytes[0] & 0xF0 {
                0x90 => NoteEvent::NoteOn {
                    timing: 63,
                    voice_id: None,
                    channel: 2,
                    note: bytes[1],
                    velocity: bytes[2] as f32 / 127.0,
                },
                0x80 => NoteEvent::NoteOff {
                    timing: 63,
                    voice_id: None,
                    channel: 2,
                    note: bytes[1],
                    velocity: 0.0,
                },
                0xB0 => NoteEvent::MidiCC {
                    timing: 63,
                    channel: 2,
                    cc: bytes[1],
                    value: bytes[2] as f32 / 127.0,
                },
                _ => NoteEvent::MidiPitchBend {
                    timing: 63,
                    channel: 2,
                    value: (bytes[1] as u16 + ((bytes[2] as u16) << 7)) as f32 / 16383.0,
                },
            };
            assert_eq!(midi_event(event), Some(expected));
        }
    }
}
