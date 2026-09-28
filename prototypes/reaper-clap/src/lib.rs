//! Isolated REAPER/CLAP integration proof; no device backend is used here.
use basedrop::{Collector, Handle, Owned};
use kabl_core::PatchState;
use kabl_engine::{
    compile::{compile, CompiledPatch},
    graph::BLOCK,
    keyboard::KeyEvent,
    patch_engine::PatchEngine,
    runtime::{ParamSet, RuntimeTarget},
};
use kabl_ui::{PatchEditor, UiState};
use nice_plug::{context::gui::GuiContext, editor::dpi::LogicalSize, prelude::*};
use nice_plug_egui::{
    create_egui_editor, EguiEditor, EguiNiceSettings, EguiState, NiceEguiApp, RepaintNotifier,
};
use std::{
    num::NonZeroU32,
    sync::{Arc, Mutex},
};

const VOICES: usize = 8;
struct Control {
    patch: PatchState,
    editor: PatchEditor,
    view: UiState,
    collector: Collector,
    handle: Handle,
    tx: rtrb::Producer<Owned<CompiledPatch>>,
    rate: f32,
    rev: u64,
    error: Option<String>,
}
impl Control {
    fn sync_cutoff_from_host(&mut self, value: f32) {
        let rack = self
            .editor
            .state()
            .modules
            .get(&3)
            .and_then(|m| m.params.get("cutoff_hz"))
            .copied();
        if rack.is_some_and(|v| v.to_bits() != value.to_bits()) {
            self.editor.set_param(3, "cutoff_hz", value);
            self.editor.take_dirty();
            if let Some(m) = self.patch.modules.get_mut(&3) {
                m.params.insert("cutoff_hz".into(), value);
            }
        }
    }
    fn submit_editor_snapshot(&mut self, snapshot: PatchState) -> bool {
        if snapshot != self.patch {
            self.queue(snapshot, false)
        } else {
            true
        }
    }
    fn queue(&mut self, patch: PatchState, fresh: bool) -> bool {
        let Ok(mut graph) = compile(&patch, self.rate, VOICES) else {
            self.error = Some("Patch did not compile; last valid sound retained".into());
            return false;
        };
        if self.tx.slots() == 0 {
            self.error = Some("Audio edit queue full; retry the edit".into());
            return false;
        }
        graph.rev = self.rev + 1;
        graph.fresh = fresh;
        let _ = self.tx.push(Owned::new(&self.handle, graph));
        self.rev += 1;
        self.patch = patch;
        self.error = None;
        true
    }
}
struct HostPatch(Arc<Mutex<Control>>);
impl<'a> nice_plug::params::persist::PersistentField<'a, PatchState> for HostPatch {
    fn set(&self, patch: PatchState) {
        let mut c = self.0.lock().unwrap();
        c.collector.collect();
        if c.queue(patch.clone(), true) {
            c.editor = PatchEditor::seed_from(&patch);
            c.view = UiState::default();
        }
    }
    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&PatchState) -> R,
    {
        f(&self.0.lock().unwrap().patch)
    }
}
#[derive(Params)]
struct ProofParams {
    #[id = "cutoff_hz"]
    cutoff: FloatParam,
    #[persist = "patch_v1"]
    patch: HostPatch,
}
pub struct RackApp {
    control: Arc<Mutex<Control>>,
    params: Arc<ProofParams>,
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
        let mut c = self.control.lock().unwrap();
        c.collector.collect();
        ui.label("kabl • REAPER integration proof");
        let old = self.params.cutoff.value();
        let mut cutoff = old;
        c.sync_cutoff_from_host(old);
        ui.add(egui::Slider::new(&mut cutoff, 50.0..=20000.0).text("Filter cutoff Hz"));
        if cutoff != old {
            c.editor.set_param(3, "cutoff_hz", cutoff);
        }
        if let Some(message) = &c.error {
            ui.colored_label(egui::Color32::RED, message);
        }
        let Control { editor, view, .. } = &mut *c;
        kabl_ui::show(editor, view, ui);
        let dirty = c.editor.take_dirty();
        let snapshot = c.editor.state().clone();
        let accepted_cutoff = snapshot
            .modules
            .get(&3)
            .and_then(|m| m.params.get("cutoff_hz"))
            .copied()
            .filter(|rack_cutoff| {
                let stored = c.patch.modules.get(&3).and_then(|m| m.params.get("cutoff_hz"));
                dirty && stored.is_none_or(|v| v.to_bits() != rack_cutoff.to_bits())
            });
        if c.submit_editor_snapshot(snapshot) {
            if let (Some(rack_cutoff), Some(gui)) = (accepted_cutoff, &self.gui) {
                let setter = gui.param_setter();
                setter.begin_set_parameter(&self.params.cutoff);
                setter.set_parameter(&self.params.cutoff, rack_cutoff);
                setter.end_set_parameter(&self.params.cutoff);
            }
        }
        c.collector.collect();
    }
}
pub struct Proof {
    params: Arc<ProofParams>,
    control: Arc<Mutex<Control>>,
    rx: rtrb::Consumer<Owned<CompiledPatch>>,
    engine: Option<PatchEngine>,
    left: [f32; BLOCK],
    right: [f32; BLOCK],
    cursor: usize,
    last_cutoff: f32,
    held: [u16; 128],
    editor_state: Arc<EguiState>,
    repaint: RepaintNotifier,
}
impl Default for Proof {
    fn default() -> Self {
        let patch = kabl_standalone::default_patch();
        let collector = Collector::new();
        let handle = collector.handle();
        let (tx, rx) = rtrb::RingBuffer::new(256);
        let control = Arc::new(Mutex::new(Control {
            editor: PatchEditor::seed_from(&patch),
            view: UiState::default(),
            patch,
            collector,
            handle,
            tx,
            rate: 48000.0,
            rev: 0,
            error: None,
        }));
        Self {
            params: Arc::new(ProofParams {
                cutoff: FloatParam::new(
                    "Filter cutoff",
                    3000.0,
                    FloatRange::Linear {
                        min: 50.0,
                        max: 20000.0,
                    },
                )
                .with_unit(" Hz"),
                patch: HostPatch(control.clone()),
            }),
            control,
            rx,
            engine: None,
            left: [0.0; BLOCK],
            right: [0.0; BLOCK],
            cursor: BLOCK,
            last_cutoff: f32::NAN,
            held: [0; 128],
            editor_state: EguiState::from_size(LogicalSize::new(1180.0, 740.0), 1.0),
            repaint: RepaintNotifier::new(),
        }
    }
}
impl Plugin for Proof {
    const NAME: &'static str = "kabl REAPER proof";
    const VENDOR: &'static str = "stcksmsh";
    const URL: &'static str = "https://github.com/stcksmsh/kabl";
    const EMAIL: &'static str = "kabl@example.invalid";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];
    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    type Editor = EguiEditor<RackApp>;
    type SysExMessage = ();
    type BackgroundTask = ();
    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }
    fn editor(&mut self, _executor: AsyncExecutor<Self>) -> Option<Self::Editor> {
        create_egui_editor(
            self.editor_state.clone(),
            self.repaint.clone(),
            EguiNiceSettings::new(),
            RackApp {
                control: self.control.clone(),
                params: self.params.clone(),
                gui: None,
            },
        )
    }
    fn activate(
        &mut self,
        _layout: &AudioIOLayout,
        config: &BufferConfig,
        _context: &mut impl ActivateContext<Self>,
    ) -> bool {
        if !(config.sample_rate.is_finite() && config.sample_rate > 0.0) {
            return false;
        }
        let mut c = self.control.lock().unwrap();
        c.rate = config.sample_rate;
        c.view.sample_rate = config.sample_rate;
        let Ok(engine) = PatchEngine::new(&c.handle, &c.patch, c.rate, VOICES) else {
            return false;
        };
        while let Ok(old) = self.rx.pop() {
            drop(old);
        }
        c.collector.collect();
        self.engine = Some(engine);
        self.cursor = BLOCK;
        self.last_cutoff = f32::NAN;
        self.held = [0; 128];
        true
    }
    fn reset(&mut self) {
        if let Some(engine) = &mut self.engine {
            engine.key(KeyEvent::AllOff);
        }
        self.cursor = BLOCK;
        self.held = [0; 128];
        self.last_cutoff = f32::NAN;
    }
    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let Some(engine) = &mut self.engine else {
            for samples in buffer.iter_samples() {
                for out in samples {
                    *out = 0.0;
                }
            }
            return ProcessStatus::Normal;
        };
        let cutoff = self.params.cutoff.value();
        if cutoff.to_bits() != self.last_cutoff.to_bits() {
            apply_cutoff(engine, cutoff);
            self.last_cutoff = cutoff;
        }
        let mut event = context.next_event();
        for (i, samples) in buffer.iter_samples().enumerate() {
            while event.as_ref().is_some_and(|e| e.timing() <= i as u32) {
                match event.unwrap() {
                    NoteEvent::NoteOn { note, velocity, .. } if note < 128 => {
                        let count = &mut self.held[note as usize];
                        if *count == 0 {
                            engine.key(KeyEvent::On {
                                note,
                                velocity: (velocity * 127.0).round().clamp(1.0, 127.0) as u8,
                            });
                        }
                        *count = count.saturating_add(1);
                    }
                    NoteEvent::NoteOff { note, .. } if note < 128 => {
                        let count = &mut self.held[note as usize];
                        *count = count.saturating_sub(1);
                        if *count == 0 {
                            engine.key(KeyEvent::Off { note });
                        }
                    }
                    NoteEvent::Choke { note, .. } if note < 128 => {
                        self.held[note as usize] = 0;
                        engine.key(KeyEvent::Off { note });
                    }
                    NoteEvent::Choke { note: 255, .. } => {
                        self.held = [0; 128];
                        engine.key(KeyEvent::AllOff);
                    }
                    _ => {} // Other note dialect features are outside the proof.
                }
                event = context.next_event();
            }
            if self.cursor == BLOCK {
                let swapped = if let Ok(graph) = self.rx.pop() {
                    engine.receive_swap(graph);
                    true
                } else {
                    false
                };
                if swapped {
                    apply_cutoff(engine, cutoff);
                }
                engine.process_block(&mut self.left, &mut self.right);
                self.cursor = 0;
            }
            let mut channels = samples.into_iter();
            if let Some(out) = channels.next() {
                *out = self.left[self.cursor];
            }
            if let Some(out) = channels.next() {
                *out = self.right[self.cursor];
            }
            self.cursor += 1;
        }
        ProcessStatus::Normal
    }
    fn deactivate(&mut self) {
        self.engine = None;
        let mut c = self.control.lock().unwrap();
        while let Ok(old) = self.rx.pop() {
            drop(old);
        }
        c.collector.collect();
    }
}
#[inline]
fn apply_cutoff(engine: &mut PatchEngine, cutoff: f32) {
    engine.set(&ParamSet {
        rev: u64::MAX,
        target: RuntimeTarget::Param {
            id: 3,
            kind: "filter.svf",
            index: 0,
        },
        value: cutoff,
    });
}
impl ClapPlugin for Proof {
    const CLAP_ID: &'static str = "dev.stcksmsh.kabl.reaper-proof";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Isolated kabl integration prototype");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::Instrument, ClapFeature::Stereo];
}
mod state_bridge;

#[cfg(test)]
mod tests {
    use super::*;
    use nice_plug::params::persist::PersistentField;
    #[test]
    fn host_patch_is_complete_and_instance_local() {
        let a = Proof::default();
        let b = Proof::default();
        let mut patch = a.control.lock().unwrap().patch.clone();
        patch
            .modules
            .get_mut(&3)
            .unwrap()
            .params
            .insert("cutoff_hz".into(), 1700.0);
        patch.cables.remove(&4);
        let saved = serde_json::to_vec(&patch).unwrap();
        let restored: PatchState = serde_json::from_slice(&saved).unwrap();
        a.params.patch.set(restored.clone());
        assert_eq!(a.control.lock().unwrap().patch, restored);
        assert_ne!(b.control.lock().unwrap().patch, restored);
        assert_eq!(a.control.lock().unwrap().editor.state(), &restored);
    }
    #[test]
    fn corrupt_compilation_preserves_working_state() {
        let a = Proof::default();
        let original = a.control.lock().unwrap().patch.clone();
        let mut bad = original.clone();
        bad.modules.get_mut(&2).unwrap().kind = "unknown.module".into();
        a.params.patch.set(bad);
        let c = a.control.lock().unwrap();
        assert_eq!(c.patch, original);
        assert_eq!(c.editor.state(), &original);
        assert!(c.error.is_some());
    }
    #[test]
    fn rejected_editor_snapshot_never_becomes_host_state() {
        let a = Proof::default();
        let mut c = a.control.lock().unwrap();
        let original = c.patch.clone();
        let mut bad = original.clone();
        bad.modules.get_mut(&2).unwrap().kind = "unknown.module".into();
        c.submit_editor_snapshot(bad.clone());
        c.submit_editor_snapshot(bad);
        assert_eq!(c.patch, original);
        assert!(c.error.is_some());
    }
    #[test]
    fn host_cutoff_syncs_the_reopened_rack_without_audio_lock() {
        let a = Proof::default();
        let mut c = a.control.lock().unwrap();
        c.sync_cutoff_from_host(1800.0);
        assert_eq!(c.patch.modules[&3].params["cutoff_hz"], 1800.0);
        assert_eq!(c.editor.state().modules[&3].params["cutoff_hz"], 1800.0);
    }
}
