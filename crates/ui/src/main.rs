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
use kabl_core::PatchState;
use kabl_engine::compile::compile;
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::{swap_channel, PatchEngine, SwapSender};
use kabl_standalone::{
    apply_voice_event, connect_midi, default_patch, RingBuffer, VoiceEvent, DEFAULT_VOICE_COUNT,
};
use kabl_ui::{show, PatchEditor, UiState};

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
    _stream: Option<cpal::Stream>,
    _midi_connection: Option<midir::MidiInputConnection<()>>,
}

impl AudioHost {
    fn offline(collector: Collector, status: String) -> Self {
        AudioHost {
            swap_tx: None,
            sample_rate: 48000.0,
            collector,
            status,
            _stream: None,
            _midi_connection: None,
        }
    }

    fn start(patch: &PatchState) -> Self {
        let collector = Collector::new();
        let handle = collector.handle();

        let host = cpal::default_host();
        let Some(device) = host.default_output_device() else {
            return Self::offline(
                collector,
                "no audio output device found -- editing works, playback won't".into(),
            );
        };
        let config = match device.default_output_config() {
            Ok(c) if c.sample_format() == cpal::SampleFormat::F32 => c,
            _ => {
                return Self::offline(
                    collector,
                    "no usable (f32) audio output config -- editing works, playback won't".into(),
                )
            }
        };

        let sample_rate = config.sample_rate() as f32;
        let channels = config.channels() as usize;

        let mut engine = match PatchEngine::new(&handle, patch, sample_rate, DEFAULT_VOICE_COUNT) {
            Ok(e) => e,
            Err(err) => return Self::offline(collector, format!("patch failed to compile: {err}")),
        };

        let (swap_tx, mut swap_rx) = swap_channel(SWAP_QUEUE);
        let (midi_producer, mut midi_consumer) = rtrb::RingBuffer::<VoiceEvent>::new(256);
        let midi_connection = connect_midi("kabl-ui", midi_producer, None);

        let mut left_ring = RingBuffer::new(RING_CAPACITY);
        let mut right_ring = RingBuffer::new(RING_CAPACITY);

        let stream = device.build_output_stream(
            config.config(),
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                // Audio thread. No allocation, no locks: install queued graphs (state carry is
                // allocation-free), apply MIDI to every running graph, render.
                engine.drain_swaps(&mut swap_rx);
                while let Ok(event) = midi_consumer.pop() {
                    apply_voice_event(&mut engine, event);
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

        let (stream, status) = match stream {
            Ok(s) => match s.play() {
                Ok(()) => (
                    Some(s),
                    format!("playing -- {sample_rate} Hz, {channels} ch"),
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
            _stream: stream,
            _midi_connection: midi_connection,
        }
    }

    /// UI-thread call: compiles `patch` and queues it for the audio thread.
    fn rebuild(&mut self, patch: &PatchState) {
        let new_patch = match compile(patch, self.sample_rate, DEFAULT_VOICE_COUNT) {
            Ok(p) => p,
            Err(err) => {
                self.status = format!("recompile failed: {err}");
                return;
            }
        };
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
    let audio = AudioHost::start(editor.state());

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
        Box::new(|_cc| {
            Ok(Box::new(App {
                editor,
                ui_state,
                audio,
            }))
        }),
    )
}
