//! The cpal output callback: control queue, MIDI timing, rendering, reports to the UI, the
//! start-up gate, and the Sounds-preview mix on top of the rack.

use super::*;
use kabl_core::ModuleId;
use kabl_engine::facetap::FaceReport;
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::PatchEngine;
use kabl_engine::probe::ProbeReport;
use kabl_engine::runtime::{Feedback, ToAudio};
use kabl_engine::timeline::Timeline;
use kabl_modules::builtins::{DelayLock, LfoSync};
use kabl_standalone::RingBuffer;
use kabl_ui::{control, record};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[cfg(test)]
use super::alloc_count::Scope as CallbackAllocationScope;

pub(crate) const RING_CAPACITY: usize = BLOCK * 256;
/// Frames the callback renders through the timeline at a time (stack buffers).
pub(crate) const CHUNK: usize = 256;

/// Everything the output callback owns. Built by `AudioHost::start`, moved into the stream.
pub(crate) struct OutputCallback {
    pub(crate) engine: PatchEngine,
    pub(crate) control_rx: rtrb::Consumer<ToAudio>,
    pub(crate) fb: Arc<Feedback>,
    pub(crate) preview_voice: kabl_ui::sound_preview::Voice,
    /// Measurement only (`KABL_LATENCY_FILE`): each newly applied revision with its time.
    pub(crate) measure: bool,
    pub(crate) applied_tx: rtrb::Producer<(u64, f64)>,
    pub(crate) last_applied: u64,
    pub(crate) midi_consumer: rtrb::Consumer<Note>,
    pub(crate) last_start: Option<std::time::Instant>,
    pub(crate) t: Arc<CallbackTiming>,
    /// Boxed: its event queue is too big for the stack of the thread that builds it.
    pub(crate) timeline: Box<Timeline>,
    pub(crate) left_ring: RingBuffer,
    pub(crate) right_ring: RingBuffer,
    pub(crate) take_input: bool,
    pub(crate) input_rx: rtrb::Consumer<[f32; 2]>,
    pub(crate) steps_tx: rtrb::Producer<SeqReport>,
    pub(crate) clocks_tx: rtrb::Producer<(ModuleId, bool, f64)>,
    pub(crate) delays_tx: rtrb::Producer<(ModuleId, DelayLock, f32)>,
    pub(crate) lfos_tx: rtrb::Producer<(ModuleId, LfoSync)>,
    pub(crate) morphs_tx: rtrb::Producer<MorphReport>,
    pub(crate) keys: Arc<AtomicU64>,
    pub(crate) probe_tx: rtrb::Producer<ProbeReport>,
    pub(crate) face_tx: rtrb::Producer<FaceReport>,
    pub(crate) audio_gate: Arc<AtomicU64>,
    pub(crate) ready_for: u64,
    pub(crate) ready_frames: usize,
    pub(crate) tap: record::Tap,
    pub(crate) peaks: Arc<record::PeakTap>,
    pub(crate) fault: Option<String>,
    pub(crate) fault_after: u64,
    pub(crate) fault_sent: bool,
    pub(crate) sample_rate: f32,
    pub(crate) channels: usize,
    pub(crate) rt_tried: bool,
    /// Held while the stream lives: real-time priority ends when it drops.
    pub(crate) _rt_handle: Option<audio_thread_priority::RtPriorityHandle>,
    #[cfg(test)]
    pub(crate) warmups: u32,
}

impl OutputCallback {
    /// One cpal output callback.
    pub(crate) fn process(&mut self, data: &mut [f32]) {
        #[cfg(test)]
        let _allocation_scope = {
            self.warmups += 1;
            CallbackAllocationScope::new(self.warmups > 2)
        };
        let (sample_rate, channels) = (self.sample_rate, self.channels);
        let t = &*self.t;
        // Flush subnormals to zero for the whole callback; restored when it returns.
        let _ftz = kabl_engine::ftz::FtzGuard::new();
        // Once, on the first callback: ask for real-time priority for this thread (rtkit
        // over D-Bus, else the rlimit). It may allocate and block briefly; the stream
        // is starting and silent. Refusal leaves the thread as it was.
        if !self.rt_tried {
            self.rt_tried = true;
            let frames = (data.len() / channels) as u32;
            match audio_thread_priority::promote_current_thread_to_real_time(
                frames,
                sample_rate as u32,
            ) {
                Ok(h) => {
                    self._rt_handle = Some(h);
                    t.rt.store(1, Ordering::Relaxed);
                }
                Err(e) => {
                    let _ = t.rt_error.set(e.to_string());
                    t.rt.store(2, Ordering::Relaxed);
                }
            }
        }
        let started = std::time::Instant::now();
        let previous = self.last_start;
        let since_last = previous.map(|l| started - l);
        self.last_start = Some(started);
        // Audio thread. No allocation, no locks: install queued graphs, runtime values,
        // commands and transport in request order (allocation-free, at most one queue's
        // worth per callback), apply MIDI to every running graph, render.
        let engine = &mut self.engine;
        let fb = &*self.fb;
        engine.drain(&mut self.control_rx, control::QUEUE, fb);
        self.preview_voice.poll();
        if self.measure {
            let a = Feedback::get(&fb.applied_rev);
            if a != self.last_applied {
                self.last_applied = a;
                let _ = self.applied_tx.push((a, clock_s()));
            }
        }
        let frames_needed = data.len() / channels;
        // Each message keeps its place inside the previous callback period, one
        // period later (docs/midi-timing/design.md, section 4).
        while let Ok((at, event)) = self.midi_consumer.pop() {
            let offset = match previous {
                Some(p) if at >= p => ((at - p).as_secs_f64() * sample_rate as f64) as usize,
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
            self.timeline.push(offset, frames_needed, event);
        }

        // The shared timing path: any callback size, one 64-sample engine grid.
        let (take_input, input_rx) = (self.take_input, &mut self.input_rx);
        while self.left_ring.available() < frames_needed {
            let n = (frames_needed - self.left_ring.available()).min(CHUNK);
            let (mut l, mut r) = ([0f32; CHUNK], [0f32; CHUNK]);
            self.timeline
                .render(engine, &mut l[..n], &mut r[..n], |e, _| {
                    if take_input {
                        let (mut il, mut ir) = ([0f32; BLOCK], [0f32; BLOCK]);
                        kabl_standalone::input::take_block(input_rx, &mut il, &mut ir);
                        e.set_audio_in(&il, &ir);
                    }
                });
            self.left_ring.push_slice(&l[..n]);
            self.right_ring.push_slice(&r[..n]);
        }
        let q = self.timeline.stats();
        t.timeline_dropped
            .store(q.dropped + q.forced, Ordering::Relaxed);
        t.timeline_moved
            .store(q.clamped + q.reordered, Ordering::Relaxed);

        // Full queue (UI not drawing): the UI just misses these, nothing waits.
        engine.seqs(|id, step, bank, queued| {
            let _ = self.steps_tx.push((id, step, bank, queued));
        });
        engine.clocks(|id, running, pos| {
            let _ = self.clocks_tx.push((id, running, pos));
        });
        engine.delays(|id, lock, ms| {
            let _ = self.delays_tx.push((id, lock, ms));
        });
        engine.lfos(|id, sync| {
            let _ = self.lfos_tx.push((id, sync));
        });
        engine.morphs(|id, k, v, to, p| {
            let _ = self.morphs_tx.push((id, k, v, to, p));
        });
        let (held, sounding) = engine.keys();
        // Voices sounding: the keyboards' own, or the envelopes not idle when a
        // sequencer or an arpeggiator plays the patch.
        let sounding = sounding.max(engine.active_envelopes());
        self.keys
            .store(pack_keys(held, sounding), Ordering::Relaxed);
        // Full (UI not drawing): this window is dropped; the UI counts the gap.
        if let Some(r) = engine.take_probe_report() {
            let _ = self.probe_tx.push(r);
        }
        if let Some(r) = engine.take_face_report() {
            let _ = self.face_tx.push(r);
        }

        let audio_gate = &*self.audio_gate;
        let want = audio_gate.load(Ordering::Relaxed);
        if want != 0 && want != u64::MAX {
            if self.ready_for != want {
                self.ready_for = want;
                self.ready_frames = 0;
            }
            if Feedback::get(&fb.applied_rev) >= want && !engine.is_swapping() {
                self.ready_frames += frames_needed;
            } else {
                self.ready_frames = 0;
            }
            if self.ready_frames >= sample_rate as usize / 40 {
                // A failure or a newer edit may have closed/revised the gate since
                // this callback read `want`; never overwrite that decision.
                let _ = unmute_if_current(audio_gate, want);
            }
        }
        let muted = audio_gate.load(Ordering::Relaxed) != u64::MAX;
        let rec = self.tap.begin(frames_needed) && !muted;
        for frame in data.chunks_mut(channels) {
            let l = self.left_ring.pop().unwrap_or(0.0);
            let r = self.right_ring.pop().unwrap_or(0.0);
            let (l, r) = if muted { (0.0, 0.0) } else { (l, r) };
            // The preview sounds on top of the rack, through the same output, meter
            // and recorder.
            let (pl, pr) = self.preview_voice.frame();
            let (l, r) = (l + pl, r + pr);
            if rec {
                self.tap.frame(l, r);
            }
            self.peaks.feed(l, r);
            frame[0] = l;
            for s in frame.iter_mut().skip(1) {
                *s = r;
            }
        }
        self.preview_voice.report();
        if t.count.load(Ordering::Relaxed) >= self.fault_after {
            match self.fault.as_deref() {
                Some("stall") => return,
                Some("error") if !self.fault_sent => {
                    self.fault_sent = true;
                    t.other_errors.fetch_add(1, Ordering::Relaxed);
                }
                _ => {}
            }
        }
        t.record(started.elapsed(), since_last, frames_needed, sample_rate);
    }
}
