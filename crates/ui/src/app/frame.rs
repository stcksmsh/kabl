//! The app window: per-frame polling, the toolbar additions, the close request.

use super::*;

use kabl_engine::runtime::Feedback;
use kabl_ui::control::{self, Delivery};
use kabl_ui::kit::{self, Tip, Tone};
use kabl_ui::style::Role;
use kabl_ui::{record, PatchEditor, UiState};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub(crate) struct App {
    pub(crate) core: Arc<Mutex<Core>>,
    pub(crate) compile_notice: CompileNotice,
    pub(crate) latency_at: std::time::Instant,
    pub(crate) audio: AudioHost,
    pub(crate) midi: Midi,
    /// `KABL_HITS_FILE`: where to write the drawn target rects (for scripted real-input runs).
    pub(crate) hits_file: Option<String>,
    pub(crate) hits_written: String,
    /// `KABL_STATS_FILE`: callback timing, rewritten about once a second.
    pub(crate) stats_file: Option<(String, std::time::Instant)>,
    pub(crate) peaks: Arc<record::PeakTap>,
    pub(crate) meter_at: std::time::Instant,
    /// Callback count last seen and when it last moved: a stream that stops calling back
    /// (an unsupported buffer size, a device gone) is reported, not left silent.
    pub(crate) watchdog: (u64, std::time::Instant),
    pub(crate) log: Option<kabl_standalone::applog::LogHandle>,
    pub(crate) health: Health,
    pub(crate) request: AudioRequest,
    pub(crate) outputs: Vec<(usize, String)>,
    pub(crate) output_scanned: std::time::Instant,
    pub(crate) phase: AudioPhase,
    pub(crate) retry: Option<RetryTask>,
    pub(crate) session: u64,
    pub(crate) pending_notes: Option<rtrb::Producer<Note>>,
    pub(crate) pending_recorder: Option<record::Recorder>,
    pub(crate) record_finish: Option<std::thread::JoinHandle<record::Recorder>>,
    pub(crate) record_last: Option<record::Outcome>,
    pub(crate) control_stop: Arc<AtomicBool>,
    pub(crate) control_thread: Option<std::thread::JoinHandle<()>>,
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
        // Measurements for the inspector: only the newest few matter.
        let now = ui.input(|i| i.time);
        ui_state.inspect.audio = self.phase == AudioPhase::Running;
        self.read_audio_reports(ui_state, now);
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
        self.poll_midi(ui_state);
        let interrupt_take = self.watch_stream(ui, progress);
        self.show_status(ui, ui_state);
        let mut retry_clicked = false;
        kabl_ui::show_with(editor, ui_state, ui, |ui, ui_state| {
            retry_clicked = self.toolbar(ui, st, ui_state, delivery);
        });
        // The Sound-check strip's output choice applies at once: it is there to hear it.
        if let Some(dev) = ui_state.output_select.take() {
            kabl_ui::first_run::devices::remember_output(
                ui_state,
                dev.as_ref().map(|(_, n)| n.clone()),
            );
            self.request.device = dev;
            retry_clicked |= self.retry.is_none();
        }
        self.midi.send_keys(ui_state);
        if let Some(rec) = ui_state.recorder.as_ref() {
            // Keep the chosen folder even if the control pump takes this recorder on a
            // failure before the next UI frame.
            *record_dir = rec.dir.clone();
        }
        self.write_files(ui, ui_state, editor, delivery, latency, *stale_cc);
        // Document changes and queued commands to the audio thread (runtime values or a graph).
        control::deliver(editor, ui_state, delivery);
        self.advance_start(ui_state, delivery, ready_at_frame_start, start_rev);
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
        handle_close(ui, editor, ui_state);
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

impl App {
    /// Measurements and states the audio callback published since the last frame.
    fn read_audio_reports(&mut self, ui_state: &mut UiState, now: f64) {
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
        if let Some(rx) = self.audio.face_rx.as_mut() {
            while let Ok(r) = rx.pop() {
                ui_state.live.accept(&r, now);
            }
        }
        if let Some(rx) = self.audio.steps_rx.as_mut() {
            while let Ok((id, step, bank, queued)) = rx.pop() {
                ui_state.seq_steps.insert(id, step);
                ui_state.seq_banks.insert(id, (bank, queued));
            }
        }
        if let Some(rx) = self.audio.clocks_rx.as_mut() {
            while let Ok((id, running, pos)) = rx.pop() {
                ui_state.clock_running.insert(id, running);
                ui_state.clock_pos.insert(id, pos);
            }
        }
        if let Some(rx) = self.audio.delays_rx.as_mut() {
            while let Ok((id, lock, ms)) = rx.pop() {
                ui_state.delay_status.insert(id, (lock, ms));
            }
        }
        if let Some(rx) = self.audio.morphs_rx.as_mut() {
            while let Ok((id, k, value, to, progress)) = rx.pop() {
                let live = kabl_ui::MacroLive {
                    value,
                    to,
                    progress,
                };
                ui_state
                    .macro_live
                    .insert((id, k), (live, std::time::Instant::now()));
            }
        }
        if let Some(rx) = self.audio.lfos_rx.as_mut() {
            while let Ok((id, sync)) = rx.pop() {
                ui_state.lfo_status.insert(id, sync);
            }
        }
    }

    /// Port choice from the UI, rescans, unplug and replug, all notes off.
    fn poll_midi(&mut self, ui_state: &mut UiState) {
        if let Some(pick) = ui_state.midi_select.take() {
            self.midi.lost = None;
            ui_state.midi_note = None;
            reset_pickup(ui_state);
            let r = match &pick {
                Some(name) => self.midi.connect(name),
                None => {
                    self.midi.disconnect();
                    Ok(())
                }
            };
            match r {
                Ok(()) => kabl_ui::first_run::devices::remember_midi(ui_state, pick),
                Err(e) => ui_state.last_message = Some(e),
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
            // AllOff below ends them too: only forget what was held here.
            ui_state.keys.all_off();
            ui_state.keys.out.clear();
            self.midi.release_all();
            ui_state.last_message = Some("all MIDI voices released".into());
        }
    }

    /// Keeps frames coming while audio runs; returns whether a sticky callback failure was
    /// just seen (the take in progress is then interrupted).
    fn watch_stream(&mut self, ui: &egui::Ui, progress: &Option<Progress>) -> bool {
        let mut interrupt = false;
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
                    interrupt = true;
                }
            }
            if n != self.watchdog.0 {
                self.watchdog = (n, std::time::Instant::now());
            }
        }
        interrupt
    }

    /// The meter and the status bar's audio, CPU, buffer and voice readouts.
    fn show_status(&mut self, ui: &egui::Ui, ui_state: &mut UiState) {
        let dt = self.meter_at.elapsed().as_secs_f32();
        self.meter_at = std::time::Instant::now();
        let (p, bad) = self.peaks.take();
        ui_state.meter.update(p, bad, dt);
        ui_state.midi_inputs = self.midi.ports.clone();
        ui_state.midi_input = self.midi.port.clone();
        ui_state.audio_outputs.clone_from(&self.outputs);
        ui_state.audio_output.clone_from(&self.request.device);
        if !ui_state.seq_steps.is_empty() || !ui_state.delay_status.is_empty() {
            // egui only repaints on input; the step light and readouts need frames of their own.
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(30));
        }
        ui_state.status_meter = true;
        ui_state.status_audio.clone_from(&self.audio.status);
        if let Some(t) = self.audio.timing.as_ref() {
            ui_state.status_cpu = Some(t.load_permille.load(Ordering::Relaxed) as f32 / 10.0);
            let frames = t.frames_max.load(Ordering::Relaxed);
            ui_state.status_buffer = (frames > 0).then_some(frames as u32);
        }
        ui_state.status_voices = Some(self.midi.voices());
    }

    /// The status bar's right side: Diagnostics, Log, Retry audio and the output choice.
    /// Returns whether Retry was clicked.
    fn toolbar(
        &mut self,
        ui: &mut egui::Ui,
        st: &kabl_ui::style::Style,
        ui_state: &mut UiState,
        delivery: &Delivery,
    ) -> bool {
        // Requested but not yet with the audio thread (it is not taking messages).
        let waiting = delivery.pending();
        let diag = ui.add(
            kit::Button::new(
                st,
                if waiting > 0 && self.audio.timing.is_some() {
                    "Diagnostics •"
                } else {
                    "Diagnostics"
                },
            )
            .ghost()
            .small(),
        );
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
                row(
                    ui,
                    "Audio",
                    format!("{:?} · session {}", self.phase, self.session),
                );
                if waiting > 0 && self.audio.timing.is_some() {
                    row(ui, "Waiting for audio", format!("{waiting} change(s)"));
                }
                if !self.audio.settings.is_empty() {
                    kit::paragraph(ui, st, Role::Caption, Tone::Text2, &self.audio.settings);
                }
                if let Some(last) = ui_state
                    .recorder
                    .as_ref()
                    .and_then(|r| r.last.as_ref())
                    .or(self.record_last.as_ref())
                {
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
            let text = if problem.is_some() { "Log !" } else { "Log" };
            let r = ui.add(kit::Button::new(st, text).ghost().small()).tip(
                st,
                &format!(
                    "{}\nLevel {} (KABL_LOG or --log-level to change). Click to copy the path.",
                    problem.unwrap_or_else(|| log.path.display().to_string()),
                    log.level
                ),
            );
            ui_state.record("log-path".into(), r.rect);
            if r.clicked() {
                ui.ctx().copy_text(log.path.display().to_string());
                ui_state.last_message = Some(format!("log path copied: {}", log.path.display()));
            }
        }
        let r = ui.add(kit::Button::new(st, "Retry audio").small().enabled(self.retry.is_none()))
            .tip(st, "Restarts stream; live notes and tails end, clocks stop, and a recording becomes a partial take");
        ui_state.record("audio-retry".into(), r.rect);
        let retry_clicked = r.clicked();
        let chosen = self
            .request
            .device
            .as_ref()
            .map_or("System default".to_string(), |(i, name)| {
                format!("{i}: {name}")
            });
        let out = kit::dropdown(ui, st, "audio-output", &chosen, 190.0, |ui| {
            if kit::menu_item(ui, st, "System default", self.request.device.is_none()).clicked() {
                self.request.device = None;
                kabl_ui::first_run::devices::remember_output(ui_state, None);
            }
            for (i, name) in &self.outputs {
                let dev = Some((*i, name.clone()));
                if kit::menu_item(ui, st, &format!("{i}: {name}"), self.request.device == dev)
                    .clicked()
                {
                    self.request.device = dev;
                    kabl_ui::first_run::devices::remember_output(ui_state, Some(name.clone()));
                }
            }
        });
        ui_state.record("audio-output".into(), out.rect);
        retry_clicked
    }

    /// `KABL_HITS_FILE`, `KABL_LATENCY_FILE` and `KABL_STATS_FILE`.
    fn write_files(
        &mut self,
        ui: &egui::Ui,
        ui_state: &UiState,
        editor: &PatchEditor,
        delivery: &Delivery,
        latency: &Option<Latency>,
        stale_cc: u64,
    ) {
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
    }

    /// While starting: fail on a compile error, else open the gate once the current graph
    /// plays, hand back the notes and the recorder.
    fn advance_start(
        &mut self,
        ui_state: &mut UiState,
        delivery: &Delivery,
        ready_at_frame_start: bool,
        start_rev: u64,
    ) {
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
    }
}

/// Closing the window (or Ctrl+Q) with unsaved changes asks first; a finished quit closes.
fn handle_close(ui: &egui::Ui, editor: &mut PatchEditor, ui_state: &mut UiState) {
    let close = ui.ctx().input(|i| i.viewport().close_requested());
    if close && kabl_ui::browser::io_busy(ui_state) {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::CancelClose);
        ui_state.last_message = Some("wait for the file operation to finish before closing".into());
    } else if close && !ui_state.quit_now && kabl_ui::browser::is_modified(editor, ui_state) {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::CancelClose);
        if ui_state.browser.dialog.is_none() {
            kabl_ui::browser::request(editor, ui_state, kabl_ui::browser::Pending::Quit);
        } else {
            // A dialog (Save As, a question) is already open: it stays, and the close
            // waits for it rather than replacing it.
            ui_state.last_message = Some("close: finish or cancel the open dialog first".into());
        }
    }
    if ui_state.quit_now && !close {
        ui_state.launches.clear();
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
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
pub(crate) fn delivery_line(d: &Delivery) -> String {
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
