//! The shared document core, the audio phase, the control thread (`pump`) and the compile notice.

use super::*;

use kabl_engine::runtime::Feedback;
use kabl_ui::control::{self, Delivery};
use kabl_ui::{record, PatchEditor, UiState};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// The document, the UI state and the control side of the audio queues: one lock, taken by
/// the UI for a frame and by the control thread for MIDI CCs and retries, never by audio.
pub(crate) struct Core {
    pub(crate) editor: PatchEditor,
    pub(crate) ui: UiState,
    pub(crate) delivery: Delivery,
    /// `KABL_LATENCY_FILE` only.
    pub(crate) latency: Option<Latency>,
    pub(crate) progress: Option<Progress>,
    pub(crate) record_finish: Option<std::thread::JoinHandle<record::Recorder>>,
    pub(crate) record_dir: String,
    pub(crate) record_last: Option<record::Outcome>,
    /// CCs queued before the most recent retry boundary must not apply afterward.
    pub(crate) cc_cutoff: f64,
    pub(crate) stale_cc: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum AudioPhase {
    Starting,
    Running,
    Stalled,
    Failed,
    Restarting,
    Unavailable,
}

/// The control thread: applies MIDI CCs (mappings, pickup, buttons) and retries whatever waits
/// for the audio queue, without an editor frame. Woken by the MIDI input for each CC, and every
/// few milliseconds while something waits.
pub(crate) fn pump(
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

pub(crate) fn fresh_cc(
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

/// Scheduled, compiled and acknowledged graphs are distinct UI observations.
pub(crate) struct CompileNotice {
    pub(crate) generation: u64,
    pub(crate) error: Option<String>,
    pub(crate) graphs: u64,
    pub(crate) awaiting_rev: Option<u64>,
}

impl CompileNotice {
    pub(crate) fn new(delivery: &Delivery) -> Self {
        Self {
            generation: delivery.generation,
            error: delivery.compile_error.clone(),
            graphs: delivery.counts.graphs,
            awaiting_rev: None,
        }
    }

    /// Returns whether inspection needs rebuilding. A failed audio phase owns its actionable
    /// status; a successful compile is not described as playing until the callback accepts it.
    pub(crate) fn observe(
        &mut self,
        phase: AudioPhase,
        status: &mut String,
        delivery: &Delivery,
    ) -> bool {
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
