//! Production instrument. REAPER owns audio/MIDI; no device backend is started here.
mod automation;
mod schedule;
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
    rtrb::Producer<(automation::Bank, automation::Bank)>,
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
struct SlotParam {
    #[id = "slot"]
    value: FloatParam,
}

#[derive(Params)]
struct InstrumentParams {
    #[id = "output_gain"]
    gain: FloatParam,
    #[nested(array, group = "Automation")]
    slots: [SlotParam; automation::SLOTS],
    #[id = "host_clock"]
    host_clock: BoolParam,
}

/// A state load replaces this entire session, including the old command queue. Destruction
/// of the old session (graphs, queue storage and feedback Arc) is deferred, never on audio.
struct Session {
    engine: PatchEngine,
    rx: rtrb::Consumer<ToAudio>,
    feedback: Arc<Feedback>,
    epoch: u64,
    gain: f32,
    host_clock: bool,
    slot_values: [f32; automation::SLOTS],
    cc: rtrb::Producer<(u8, u8, u8)>,
    reports: rtrb::Producer<RuntimeReport>,
    bank: automation::Bank,
    banks: rtrb::Consumer<(automation::Bank, automation::Bank)>,
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
    lanes: [automation::Lane; automation::SLOTS],
    banks: rtrb::Producer<(automation::Bank, automation::Bank)>,
    bank_dirty: bool,
    published_bank: automation::Bank,
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
        if self.view.loaded {
            // A browser document owns new queues too. Old bank/base updates must never
            // cross into reused module identities, even when audio was paused.
            if self.tx.slots() == 0 {
                self.view.last_message = Some("State queue full; previous sound retained".into());
                return false;
            }
            let prepared = prepare(
                self.editor.state(),
                self.rate,
                &self.handle,
                self.epoch,
                1.0,
            );
            let (mut session, delivery, cc, reports, banks) = match prepared {
                Ok(prepared) => prepared,
                Err(error) => {
                    self.view.last_message = Some(error);
                    return false;
                }
            };
            if self.view.load_stopped {
                for (&id, module) in &self.editor.state().modules {
                    if module.kind == "clock" {
                        session
                            .engine
                            .transport(id, kabl_modules::builtins::Transport::Stop);
                    }
                }
            }
            self.tx.push(session).expect("checked state queue capacity");
            self.delivery = delivery;
            self.cc = cc;
            self.reports = reports;
            self.banks = banks;
            self.bank_dirty = false;
            self.published_bank = [None; automation::SLOTS];
            for lane in &mut self.lanes {
                lane.retired |= lane.target.is_some();
                lane.target = None;
            }
            self.patch = self.editor.state().clone();
            self.editor.take_dirty();
            self.rejected = None;
            self.clear_reports();
            self.view.loaded = false;
            self.view.load_stopped = false;
            self.view.launches.clear();
            self.view.transport.clear();
            self.view.takeover.clear();
            self.view.button_rearm = true;
            self.view.learn = None;
            return true;
        }
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
            automation::retire_missing(&self.patch, &mut self.lanes);
            self.bank_dirty = true;
            self.rejected = None;
        }
        if self.bank_dirty {
            let next = automation::bank(&self.patch, &self.lanes);
            let restored = automation::release_bases(&self.patch, self.published_bank);
            if self.banks.push((next, restored)).is_ok() {
                self.bank_dirty = false;
                self.published_bank = next;
            }
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
    let (banks_tx, banks_rx) = rtrb::RingBuffer::new(4);
    Ok((
        Owned::new(
            handle,
            Session {
                engine,
                rx,
                feedback,
                epoch,
                gain,
                host_clock: false,
                slot_values: [0.0; automation::SLOTS],
                cc: cc_tx,
                reports: reports_tx,
                bank: [None; automation::SLOTS],
                banks: banks_rx,
            },
        ),
        delivery,
        cc_rx,
        reports_rx,
        banks_tx,
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
    event_drops: AtomicU64,
    gestures: AtomicU32,
    gesture_values: [AtomicU32; automation::SLOTS],
    flushed_values: [AtomicU32; automation::SLOTS],
    value_notify: AtomicU32,
    flushed_modulation: [AtomicU32; automation::SLOTS],
    modulation_notify: AtomicU32,
    cc_notify: AtomicU32,
    start_reset: AtomicBool,
}

impl Shared {
    fn gesture(&self, slot: usize, value: f32) {
        self.gesture_values[slot].store(value.to_bits(), Ordering::Relaxed);
        self.gestures.fetch_or(1 << slot, Ordering::Release);
    }
    fn snapshot(&self) -> SoundState {
        let c = self.control.lock().unwrap();
        SoundState {
            version: 2,
            patch: c.patch.clone(),
            output_gain: self.params.gain.unmodulated_plain_value(),
            lanes: c.lanes.clone(),
            slot_values: std::array::from_fn(|i| {
                self.params.slots[i].value.unmodulated_plain_value()
            }),
            host_clock: self.params.host_clock.value(),
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
        let (mut session, delivery, cc, reports, banks) =
            prepare(&state.patch, c.rate, &c.handle, epoch, state.output_gain)?;
        session.bank = automation::bank(&state.patch, &state.lanes);
        let published_bank = session.bank;
        session.host_clock = state.host_clock;
        session.slot_values = state.slot_values;
        let editor = PatchEditor::seed_from(&state.patch);
        c.tx.push(session)
            .map_err(|_| "State queue full".to_string())?;
        self.committed.store(epoch * 2 - 1, Ordering::SeqCst);
        c.delivery = delivery;
        c.cc = cc;
        c.reports = reports;
        c.banks = banks;
        c.lanes = state.lanes;
        c.bank_dirty = false;
        c.published_bank = published_bank;
        c.clear_reports();
        c.patch = state.patch;
        c.editor = editor;
        c.rejected = None;
        // Keep library and view preferences; discard commands, keys and CC pickup.
        c.view.loaded = false;
        c.view.load_stopped = false;
        c.view.launches.clear();
        c.view.transport.clear();
        c.view.takeover.clear();
        c.view.button_rearm = true;
        c.view.learn = None;
        c.view.doc = None;
        c.epoch = epoch;
        self.tail_samples
            .store(tail::samples(&c.patch, c.rate), Ordering::Relaxed);
        // No wrapper persistent-field loader or activation here. Pointer remains alive via Arc.
        unsafe {
            self.params.gain.as_ptr()._internal_modulate_value(0.0);
            self.params
                .gain
                .as_ptr()
                ._internal_set_normalized_value(state.output_gain);
        }
        for (i, value) in state.slot_values.iter().enumerate() {
            unsafe {
                self.params.slots[i]
                    .value
                    .as_ptr()
                    ._internal_modulate_value(0.0);
                self.params.slots[i]
                    .value
                    .as_ptr()
                    ._internal_set_normalized_value(*value);
            }
        }
        unsafe {
            self.params
                .host_clock
                .as_ptr()
                ._internal_modulate_value(0.0);
            self.params
                .host_clock
                .as_ptr()
                ._internal_set_normalized_value(state.host_clock as u8 as f32);
        }
        for (i, value) in state.slot_values.iter().enumerate() {
            self.gesture_values[i].store(value.to_bits(), Ordering::Relaxed);
            self.flushed_values[i].store(value.to_bits(), Ordering::Relaxed);
            self.flushed_modulation[i].store(0, Ordering::Relaxed);
        }
        self.value_notify.store(0, Ordering::Release);
        self.gestures.store(0, Ordering::Release);
        self.modulation_notify.store(0, Ordering::Release);
        self.cc_notify.store(0, Ordering::Release);
        self.committed.store(epoch * 2, Ordering::SeqCst);
        Ok(())
    }
}

pub struct RackApp {
    shared: Arc<Shared>,
    gui: Option<GuiContext>,
    active_gestures: [bool; automation::SLOTS],
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
        if let Some(gui) = &self.gui {
            for (i, active) in self.active_gestures.iter_mut().enumerate() {
                if *active {
                    gui.param_setter()
                        .end_set_parameter(&self.shared.params.slots[i].value);
                    *active = false;
                }
            }
        }
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
        let before: [Option<f32>; automation::SLOTS] = std::array::from_fn(|i| {
            c.lanes[i]
                .target
                .as_ref()
                .map(|t| automation::normalized(c.editor.state(), t))
        });
        c.view.host_clock = self.shared.params.host_clock.value();
        let candidates = automation::candidates(c.editor.state());
        ui.horizontal(|ui| {
            let old = self.shared.params.host_clock.value();
            let mut host = old;
            ui.checkbox(&mut host, "Host clock (off = Free)");
            if host != old {
                if let Some(gui) = &self.gui {
                    let setter = gui.param_setter();
                    setter.begin_set_parameter(&self.shared.params.host_clock);
                    setter.set_parameter(&self.shared.params.host_clock, host);
                    setter.end_set_parameter(&self.shared.params.host_clock);
                }
            }
            ui.menu_button("Automation slots", |ui| {
                ui.set_min_width(420.0);
                egui::ScrollArea::vertical()
                    .max_height(470.0)
                    .show(ui, |ui| {
                        for i in 0..automation::SLOTS {
                            let old_target = c.lanes[i].target.clone();
                            let label = old_target.as_ref().map_or_else(
                                || {
                                    if c.lanes[i].retired {
                                        "Unassigned (deleted)".into()
                                    } else {
                                        "Unassigned".into()
                                    }
                                },
                                |t| format!("{} #{} · {}", t.kind, t.module, t.param),
                            );
                            ui.horizontal(|ui| {
                                ui.label(format!("Slot {}", i + 1));
                                egui::ComboBox::from_id_salt(("host_slot", i))
                                    .selected_text(label)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut c.lanes[i].target,
                                            None,
                                            "Unassigned",
                                        );
                                        for target in &candidates {
                                            if c.lanes.iter().enumerate().any(|(j, l)| {
                                                j != i && l.target.as_ref() == Some(target)
                                            }) {
                                                continue;
                                            }
                                            ui.selectable_value(
                                                &mut c.lanes[i].target,
                                                Some(target.clone()),
                                                format!(
                                                    "{} #{} · {}",
                                                    target.kind, target.module, target.param
                                                ),
                                            );
                                        }
                                    });
                            });
                            if c.lanes[i].target.is_some() {
                                let mut value =
                                    self.shared.params.slots[i].value.unmodulated_plain_value();
                                let response = ui.add(
                                    egui::Slider::new(&mut value, 0.0..=1.0).text("Host value"),
                                );
                                if let Some(gui) = &self.gui {
                                    let setter = gui.param_setter();
                                    if response.drag_started() {
                                        self.active_gestures[i] = true;
                                        setter.begin_set_parameter(
                                            &self.shared.params.slots[i].value,
                                        );
                                    }
                                    if response.changed() {
                                        if !response.dragged() {
                                            setter.begin_set_parameter(
                                                &self.shared.params.slots[i].value,
                                            );
                                        }
                                        setter.set_parameter(
                                            &self.shared.params.slots[i].value,
                                            value,
                                        );
                                        self.shared.gesture(i, value);
                                        if !response.dragged() {
                                            setter.end_set_parameter(
                                                &self.shared.params.slots[i].value,
                                            );
                                        }
                                    }
                                    if response.drag_stopped() {
                                        self.active_gestures[i] = false;
                                        setter
                                            .end_set_parameter(&self.shared.params.slots[i].value);
                                    }
                                }
                            }
                            if c.lanes[i].target != old_target {
                                c.lanes[i].retired = false;
                                c.bank_dirty = true;
                                self.shared.dirty.store(true, Ordering::Release);
                                state_bridge::request_main(&self.shared);
                                if let Some(target) = &c.lanes[i].target {
                                    let value = automation::normalized(c.editor.state(), target);
                                    if let Some(gui) = &self.gui {
                                        let setter = gui.param_setter();
                                        setter.begin_set_parameter(
                                            &self.shared.params.slots[i].value,
                                        );
                                        setter.set_parameter(
                                            &self.shared.params.slots[i].value,
                                            value,
                                        );
                                        setter
                                            .end_set_parameter(&self.shared.params.slots[i].value);
                                        self.shared.gesture(i, value);
                                    }
                                }
                            }
                        }
                    });
            });
        });
        let Control { editor, view, .. } = &mut *c;
        kabl_ui::show(editor, view, ui);
        for (i, previous) in before.iter().enumerate() {
            let after = c.lanes[i]
                .target
                .as_ref()
                .map(|t| automation::normalized(c.editor.state(), t));
            if after != *previous {
                if let (Some(value), Some(gui)) = (after, &self.gui) {
                    let setter = gui.param_setter();
                    if !self.active_gestures[i] {
                        setter.begin_set_parameter(&self.shared.params.slots[i].value);
                        self.active_gestures[i] = true;
                    }
                    setter.set_parameter(&self.shared.params.slots[i].value, value);
                    self.shared.gesture(i, value);
                }
            }
            if self.active_gestures[i] && !ui.input(|input| input.pointer.any_down()) {
                if let Some(gui) = &self.gui {
                    gui.param_setter()
                        .end_set_parameter(&self.shared.params.slots[i].value);
                }
                self.active_gestures[i] = false;
            }
        }
        // Delivery/compile/retry and collection run on the control worker, also when closed.
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(16));
    }
}

pub struct Instrument {
    shared: Arc<Shared>,
    loads: rtrb::Consumer<Owned<Session>>,
    midi: rtrb::Consumer<(u32, [u8; 3])>,
    events: rtrb::Consumer<(u32, schedule::Event)>,
    schedule: schedule::Schedule,
    raw_position: Option<(schedule::Position, usize)>,
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
        let candidates = automation::candidates(&patch);
        let lanes: [automation::Lane; automation::SLOTS] =
            std::array::from_fn(|i| automation::Lane {
                target: candidates.get(i).cloned(),
                retired: false,
            });
        let published_bank = automation::bank(&patch, &lanes);
        let initial_values = std::array::from_fn(|i| {
            lanes[i]
                .target
                .as_ref()
                .map_or(0.0, |t| automation::normalized(&patch, t))
        });
        let collector = Collector::new();
        let handle = collector.handle();
        let (_, delivery, cc, reports, banks) =
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
                lanes,
                banks,
                bank_dirty: true,
                published_bank,
            }),
            params: Arc::new(InstrumentParams {
                gain: FloatParam::new(
                    "Output gain",
                    1.0,
                    FloatRange::Linear { min: 0.0, max: 1.0 },
                ),
                slots: std::array::from_fn(|i| SlotParam {
                    value: FloatParam::new(
                        format!("Slot {}", i + 1),
                        initial_values[i],
                        FloatRange::Linear { min: 0.0, max: 1.0 },
                    ),
                }),
                host_clock: BoolParam::new("Host clock", false),
            }),
            committed: AtomicU64::new(0),
            stop: AtomicBool::new(false),
            host: AtomicU64::new(0),
            dirty: AtomicBool::new(false),
            audio_clock: AtomicU64::new(0),
            host_frames: AtomicU32::new(0),
            midi_overflow: AtomicBool::new(false),
            event_drops: AtomicU64::new(0),
            gestures: AtomicU32::new(0),
            gesture_values: std::array::from_fn(|i| AtomicU32::new(initial_values[i].to_bits())),
            flushed_values: std::array::from_fn(|i| AtomicU32::new(initial_values[i].to_bits())),
            value_notify: AtomicU32::new(0),
            flushed_modulation: std::array::from_fn(|_| AtomicU32::new(0)),
            modulation_notify: AtomicU32::new(0),
            cc_notify: AtomicU32::new(0),
            start_reset: AtomicBool::new(false),
            tail_samples: AtomicU64::new(tail::samples(&kabl_standalone::default_patch(), 48000.0)),
        });
        let (event_tx, events) = rtrb::RingBuffer::new(4096);
        state_bridge::capture(&shared, midi_tx, event_tx);
        let worker_shared = shared.clone();
        let worker = std::thread::Builder::new()
            .name("kabl-plugin-control".into())
            .spawn(move || {
                let start = std::time::Instant::now();
                while !worker_shared.stop.load(Ordering::Acquire) {
                    {
                        let mut c = worker_shared.control.lock().unwrap();
                        let dropped = worker_shared.event_drops.swap(0, Ordering::Relaxed);
                        if dropped > 0 {
                            c.view.last_message =
                                Some(format!("Host event limit: {dropped} events dropped"));
                        }
                        let mut events = Vec::new();
                        for _ in 0..256 {
                            if let Ok(cc) = c.cc.pop() {
                                events.push(cc);
                            } else {
                                break;
                            }
                        }
                        let before: [Option<f32>; automation::SLOTS] = std::array::from_fn(|i| {
                            c.lanes[i]
                                .target
                                .as_ref()
                                .map(|t| automation::normalized(c.editor.state(), t))
                        });
                        c.view.host_clock = worker_shared.params.host_clock.value();
                        let Control { editor, view, .. } = &mut *c;
                        kabl_ui::perform::apply_cc(
                            editor,
                            view,
                            &events,
                            start.elapsed().as_secs_f64(),
                        );
                        for (i, previous) in before.iter().enumerate() {
                            let after = c.lanes[i]
                                .target
                                .as_ref()
                                .map(|t| automation::normalized(c.editor.state(), t));
                            if after != *previous {
                                if let Some(value) = after {
                                    unsafe {
                                        worker_shared.params.slots[i]
                                            .value
                                            .as_ptr()
                                            ._internal_set_normalized_value(value);
                                    }
                                    worker_shared.gesture(i, value);
                                    worker_shared.cc_notify.fetch_or(1 << i, Ordering::Release);
                                }
                            }
                        }
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
            events,
            schedule: schedule::Schedule::new(initial_values),
            raw_position: None,
            pending: None,
            session: None,
            timeline: Timeline::new(),
            rate: 48000.0,
            worker: Some(worker),
            editor_state: EguiState::from_size(LogicalSize::new(1180.0, 680.0), 1.0),
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
            self.raw_position = None;
            self.schedule = schedule::Schedule::new(self.session.as_ref().unwrap().slot_values);
        }
    }
    fn live_params(&mut self) -> (u32, [f32; automation::SLOTS]) {
        let gestures = self.shared.gestures.swap(0, Ordering::Acquire);
        let mut values = [0.0; automation::SLOTS];
        let Some(session) = &mut self.session else {
            return (0, values);
        };
        let expected = session.epoch * 2;
        if self.shared.committed.load(Ordering::SeqCst) == expected {
            let mode = self.shared.params.host_clock.value();
            for (i, value) in values.iter_mut().enumerate() {
                if gestures & (1 << i) != 0 {
                    *value = f32::from_bits(self.shared.gesture_values[i].load(Ordering::Acquire));
                }
            }
            if self.shared.committed.load(Ordering::SeqCst) == expected {
                session.host_clock = mode;
                return (gestures, values);
            }
        }
        self.shared.gestures.fetch_or(gestures, Ordering::Release);
        (0, [0.0; automation::SLOTS])
    }
    fn live_flush(&mut self, modulation: bool) -> (u32, [f32; automation::SLOTS]) {
        let (notify, source) = if modulation {
            (
                &self.shared.modulation_notify,
                &self.shared.flushed_modulation,
            )
        } else {
            (&self.shared.value_notify, &self.shared.flushed_values)
        };
        let mask = notify.swap(0, Ordering::Acquire);
        let expected = self.session.as_ref().map(|s| s.epoch * 2);
        let epoch = self.shared.committed.load(Ordering::SeqCst);
        let values = std::array::from_fn(|i| f32::from_bits(source[i].load(Ordering::Acquire)));
        if expected == Some(epoch) && self.shared.committed.load(Ordering::SeqCst) == epoch {
            return (mask, values);
        }
        notify.fetch_or(mask, Ordering::Release);
        (0, [0.0; automation::SLOTS])
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
        for _ in 0..4 {
            if let Ok((bank, restored)) = s.banks.pop() {
                automation::replace_bank(&mut s.engine, &mut s.bank, bank, restored);
            } else {
                break;
            }
        }
        // Lane removal restores old bases first; later document edits/graphs retain their
        // normal FIFO precedence. Mapped overlays are reapplied after document delivery.
        s.engine
            .drain(&mut s.rx, kabl_ui::control::QUEUE, &s.feedback);
        self.schedule.mode(s.host_clock);
        let schedule = &mut self.schedule;
        let bank = &s.bank;
        let rate = self.rate;
        self.timeline
            .render(&mut s.engine, left, right, |engine, start| {
                schedule.block(engine, start, bank, rate);
            });
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
                active_gestures: [false; automation::SLOTS],
            },
        )
    }
    fn activate(
        &mut self,
        _layout: &AudioIOLayout,
        config: &BufferConfig,
        context: &mut impl ActivateContext<Self>,
    ) -> bool {
        if !config.sample_rate.is_finite() || !(1000.0..=768000.0).contains(&config.sample_rate) {
            return false;
        }
        let mut c = self.shared.control.lock().unwrap();
        let epoch = c.epoch;
        let gain = self.shared.params.gain.unmodulated_plain_value();
        let Ok((mut session, delivery, cc, reports, banks)) =
            prepare(&c.patch, config.sample_rate, &c.handle, epoch, gain)
        else {
            return false;
        };
        session.bank = automation::bank(&c.patch, &c.lanes);
        let published_bank = session.bank;
        session.host_clock = self.shared.params.host_clock.value();
        session.slot_values =
            std::array::from_fn(|i| self.shared.params.slots[i].value.unmodulated_plain_value());
        c.banks = banks;
        c.bank_dirty = false;
        c.published_bank = published_bank;
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
        self.schedule = schedule::Schedule::new(std::array::from_fn(|i| {
            self.shared.params.slots[i].value.unmodulated_plain_value()
        }));
        self.raw_position = None;
        context.set_latency_samples(LATENCY as u32);
        true
    }
    fn reset(&mut self) {
        // The framework calls reset on every start_processing, including restarts during
        // REAPER's render pre-roll. Transport, not a thread restart, defines a Host epoch.
        // Dedicated CLAP reset still always clears DSP and transient ownership.
        if self.shared.start_reset.load(Ordering::Relaxed)
            && self.session.as_ref().is_some_and(|s| s.host_clock)
            && self
                .raw_position
                .is_some_and(|(position, _)| position.playing)
        {
            return;
        }
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
        self.schedule.clear_time();
        self.raw_position = None;
        for _ in 0..4096 {
            if self.events.pop().is_err() {
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
        let (gestures, gesture_values) = self.live_params();
        let host_clock = self.session.as_ref().is_some_and(|s| s.host_clock);
        let mut raw_values = 0;
        let mut raw_modulation = 0;
        for _ in 0..4096 {
            let Ok((offset, event)) = self.events.pop() else {
                break;
            };
            if offset == 0 {
                if let schedule::Event::Value {
                    slot, modulation, ..
                } = event
                {
                    if modulation {
                        raw_modulation |= 1 << slot;
                    } else {
                        raw_values |= 1 << slot;
                    }
                }
                if let schedule::Event::Position(position) = event {
                    if host_clock && position.playing {
                        let discontinuity =
                            self.raw_position.is_none_or(|(previous, previous_frames)| {
                                !previous.playing
                                    || ((position.seconds - previous.seconds) * self.rate as f64
                                        - previous_frames as f64)
                                        .abs()
                                        > 2.0
                            });
                        if discontinuity {
                            if let Some(session) = &mut self.session {
                                session.engine.reset();
                            }
                            self.timeline.reset();
                            self.schedule.clear_time();
                        }
                    }
                    self.raw_position = Some((position, frames));
                }
            }
            self.schedule
                .push(self.timeline.now() + offset as u64, event);
        }
        for (modulation, raw) in [(false, raw_values), (true, raw_modulation)] {
            let (mask, values) = self.live_flush(modulation);
            for (i, value) in values.iter().enumerate() {
                if mask & !raw & (1 << i) != 0 {
                    self.schedule.push(
                        self.timeline.now(),
                        schedule::Event::Value {
                            slot: i,
                            value: *value,
                            modulation,
                        },
                    );
                }
            }
        }
        for (i, value) in gesture_values.iter().enumerate() {
            if gestures & (1 << i) != 0 {
                self.schedule.push(
                    self.timeline.now(),
                    schedule::Event::Value {
                        slot: i,
                        value: *value,
                        modulation: false,
                    },
                );
            }
        }
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
        let dropped_before = self.schedule.dropped;
        let channels = buffer.as_slice();
        if channels.len() >= 2 {
            let (left, right) = channels.split_at_mut(1);
            self.render(left[0], right[0]);
        }
        self.shared
            .event_drops
            .fetch_add(self.schedule.dropped - dropped_before, Ordering::Relaxed);
        // Host lanes may open a silent route or extend a release/effect beyond document bases.
        // A mapped instrument stays active; legacy unassigned projects keep finite-tail logic.
        if self
            .session
            .as_ref()
            .is_some_and(|s| s.bank.iter().any(Option::is_some))
        {
            return ProcessStatus::KeepAlive;
        }
        match self.shared.tail_samples.load(Ordering::Relaxed) {
            samples if samples >= u32::MAX as u64 => ProcessStatus::KeepAlive,
            samples => ProcessStatus::Tail(samples as u32),
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

    pub(super) fn instrument() -> Instrument {
        let mut p = Instrument::default();
        p.shared.stop.store(true, Ordering::Release);
        p.worker.take().unwrap().join().unwrap();
        let mut c = p.shared.control.lock().unwrap();
        let (session, delivery, cc, reports, banks) =
            prepare(&c.patch, 48000.0, &c.handle, 0, 1.0).unwrap();
        c.banks = banks;
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
        state.version = 3;
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

#[cfg(test)]
mod host_tests;
