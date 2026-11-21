//! Audio retry: tearing the old stream down and opening a new one off the UI thread.

use super::*;
use basedrop::Collector;

use kabl_standalone::default_patch;
use kabl_ui::control::{self, Delivery};
use kabl_ui::record;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub(crate) struct RetryTask {
    pub(crate) handle: std::thread::JoinHandle<RetryResult>,
    pub(crate) started: std::time::Instant,
    pub(crate) session: u64,
}

pub(crate) struct RetryResult {
    pub(crate) audio: AudioHost,
    pub(crate) delivery: Delivery,
    pub(crate) notes: rtrb::Producer<Note>,
    pub(crate) retired_graphs: usize,
}

pub(crate) struct RetryFault {
    pub(crate) delay_ms: u64,
    pub(crate) fail_reopen: bool,
}

/// The old stream is dropped before its collector is drained or a new stream is opened.
pub(crate) fn retry_worker(
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

impl App {
    pub(crate) fn interrupt_record(&mut self, core: &mut Core) {
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

    pub(crate) fn poll_record_finish(&mut self, core: &mut Core) {
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

    pub(crate) fn begin_retry(&mut self, core: &mut Core) {
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
        core.ui.clock_pos.clear();
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

    pub(crate) fn poll_retry(&mut self, core: &mut Core, now: f64) {
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
        core.ui.browser.clips.attach(self.audio.preview.take());
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
}
