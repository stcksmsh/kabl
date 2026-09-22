//! `kabl-ui` binary: the patchbay editor with live audio. Combines `PatchEditor` (patch editing
//! over the real op log) with the same `cpal`/`midir` wiring `kabl-standalone` uses, so editing a
//! patch here and hearing the result are the same window, not a separate player process.
//!
//! **Known, flagged compromise, not textbook RT-safe**: the audio callback and the UI/control
//! thread share one `PatchEngine` behind a `Mutex`. The audio callback only ever `try_lock`s (and
//! outputs silence for a block on contention, never blocks) — a real but rare glitch risk, not a
//! deadlock risk. The control thread takes a real blocking lock only when an edit actually
//! happened (`PatchEditor::take_dirty()`), for the duration of `PatchEngine::build_swap` (which
//! calls `compile()` — real work, not instant). Any audio block that lands inside that window
//! gets silence instead of underrunning or corrupting anything. The textbook-correct fix is
//! restructuring `PatchEngine` so the control thread reads a lock-free-published state snapshot
//! instead of sharing the live instance directly — real follow-up work, not attempted here (see
//! decisions.md "kabl-ui: the patchbay").
//!
//! Neither the audio path nor the window itself could be run in this container (no audio device,
//! no display server) — built and logic-tested, not seen or heard. Run on a real machine to
//! verify.

use std::sync::{Arc, Mutex};

use basedrop::Collector;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use kabl_core::PatchState;
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::PatchEngine;
use kabl_standalone::{
    apply_voice_event, connect_midi, default_patch, RingBuffer, VoiceEvent,
    DEFAULT_VOICE_COUNT, MIDI_IN_ID,
};
use kabl_ui::{show, PatchEditor, UiState};

const RING_CAPACITY: usize = BLOCK * 256;

/// Owns everything audio-related: the shared engine, the stream, the MIDI connection, and the
/// `basedrop` handle the UI thread needs to build swaps. Kept alive for the app's lifetime by
/// living inside `App`.
struct AudioHost {
    engine: Arc<Mutex<PatchEngine>>,
    handle: basedrop::Handle,
    status: String,
    _stream: Option<cpal::Stream>,
    _midi_connection: Option<midir::MidiInputConnection<()>>,
    _collector: Collector,
}

impl AudioHost {
    fn start(patch: &PatchState) -> Self {
        let collector = Collector::new();
        let handle = collector.handle();

        let host = cpal::default_host();
        let Some(device) = host.default_output_device() else {
            return AudioHost {
                engine: Arc::new(Mutex::new(
                    PatchEngine::new(&handle, patch, 48000.0, DEFAULT_VOICE_COUNT)
                        .expect("default_patch should compile"),
                )),
                handle,
                status: "no audio output device found -- editing works, playback won't".into(),
                _stream: None,
                _midi_connection: None,
                _collector: collector,
            };
        };

        let config = match device.default_output_config() {
            Ok(c) if c.sample_format() == cpal::SampleFormat::F32 => c,
            _ => {
                return AudioHost {
                    engine: Arc::new(Mutex::new(
                        PatchEngine::new(&handle, patch, 48000.0, DEFAULT_VOICE_COUNT)
                            .expect("default_patch should compile"),
                    )),
                    handle,
                    status: "no usable (f32) audio output config -- editing works, playback won't"
                        .into(),
                    _stream: None,
                    _midi_connection: None,
                    _collector: collector,
                };
            }
        };

        let sample_rate = config.sample_rate() as f32;
        let channels = config.channels() as usize;

        let engine = Arc::new(Mutex::new(
            PatchEngine::new(&handle, patch, sample_rate, DEFAULT_VOICE_COUNT)
                .expect("default_patch should compile"),
        ));

        let (midi_producer, mut midi_consumer) = rtrb::RingBuffer::<VoiceEvent>::new(256);
        let midi_connection = connect_midi("kabl-ui", midi_producer, None);

        let engine_for_stream = engine.clone();
        let mut left_ring = RingBuffer::new(RING_CAPACITY);
        let mut right_ring = RingBuffer::new(RING_CAPACITY);

        let stream = device.build_output_stream(
            config.config(),
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                // See this module's doc comment: try_lock, never block. Contention should be
                // rare (only while an edit's build_swap is in flight) and results in a silent
                // block, not a glitch that corrupts state or panics.
                let Ok(mut engine) = engine_for_stream.try_lock() else {
                    data.fill(0.0);
                    return;
                };

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
            |err| eprintln!("kabl-ui: audio stream error: {err}"),
            None,
        );

        match stream {
            Ok(s) => match s.play() {
                Ok(()) => AudioHost {
                    engine,
                    handle,
                    status: format!("playing -- {sample_rate} Hz, {channels} ch"),
                    _stream: Some(s),
                    _midi_connection: midi_connection,
                    _collector: collector,
                },
                Err(err) => AudioHost {
                    engine,
                    handle,
                    status: format!("failed to start audio stream: {err}"),
                    _stream: None,
                    _midi_connection: midi_connection,
                    _collector: collector,
                },
            },
            Err(err) => AudioHost {
                engine,
                handle,
                status: format!("failed to build audio stream: {err}"),
                _stream: None,
                _midi_connection: midi_connection,
                _collector: collector,
            },
        }
    }

    /// Control-thread call: recompiles `patch` and installs it. Blocks briefly on the shared
    /// lock -- see this module's doc comment for why that's an accepted, flagged tradeoff here.
    fn rebuild(&mut self, patch: &PatchState) {
        let Ok(mut engine) = self.engine.lock() else {
            return;
        };
        match engine.build_swap(&self.handle, patch) {
            Ok(new_graph) => engine.receive_swap(new_graph),
            Err(err) => self.status = format!("recompile failed: {err}"),
        }
    }
}


struct App {
    editor: PatchEditor,
    ui_state: UiState,
    audio: AudioHost,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::bottom("kabl-status").show(ui, |ui| {
            ui.label(&self.audio.status);
        });
        show(&mut self.editor, &mut self.ui_state, ui);
        if self.editor.take_dirty() {
            self.audio.rebuild(self.editor.state());
        }
    }
}

fn main() -> eframe::Result<()> {
    let patch = default_patch();
    let editor = PatchEditor::seed_from(&patch);
    let audio = AudioHost::start(&patch);

    eframe::run_native(
        "kabl",
        eframe::NativeOptions::default(),
        Box::new(|_cc| {
            Ok(Box::new(App {
                editor,
                ui_state: UiState::default(),
                audio,
            }))
        }),
    )
}
