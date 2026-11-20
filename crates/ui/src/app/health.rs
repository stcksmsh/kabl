//! Audio health logging, rate-limited.

use super::*;

use std::sync::atomic::{AtomicU64, Ordering};

/// Audio health as last logged: the log gets counts per interval, never a line per event.
pub(crate) struct Health {
    pub(crate) at: std::time::Instant,
    pub(crate) debug_at: std::time::Instant,
    /// (xruns, late executions, late arrivals, other errors, callbacks) at `at`.
    pub(crate) counts: [u64; 5],
    pub(crate) rt_logged: bool,
    pub(crate) stalled: bool,
    /// `errors` at the last WARN line.
    pub(crate) errors_seen: [u64; kabl_standalone::STREAM_ERROR_KINDS.len() + 2],
    /// The last error taken from the queue, and when (its age is logged: it may predate the
    /// interval whose counts it is printed with).
    pub(crate) fault_text: Option<(String, std::time::Instant)>,
    pub(crate) log_problem: Option<String>,
    pub(crate) log_checked: std::time::Instant,
}

impl Health {
    pub(crate) fn new() -> Self {
        let now = std::time::Instant::now();
        Health {
            at: now,
            debug_at: now,
            counts: [0; 5],
            rt_logged: false,
            stalled: false,
            errors_seen: [0; kabl_standalone::STREAM_ERROR_KINDS.len() + 2],
            fault_text: None,
            log_problem: None,
            log_checked: now,
        }
    }
}

impl App {
    /// UI thread, every frame: logs what the audio side counted, rate-limited.
    pub(crate) fn log_health(&mut self) {
        let h = &mut self.health;
        if self
            .audio
            .input_lost
            .as_ref()
            .is_some_and(|l| l.swap(false, Ordering::Relaxed))
        {
            log::warn!(target: "audio", "input device lost: audio.in is silent until Retry");
            self.audio.status = "input device lost: audio.in is silent until Retry".into();
        }
        if let Some(rx) = self.audio.faults_rx.as_mut() {
            while let Ok(e) = rx.pop() {
                h.fault_text = Some((e.to_string(), std::time::Instant::now()));
            }
        }
        let Some(t) = &self.audio.timing else {
            return;
        };
        let get = |a: &AtomicU64| a.load(Ordering::Relaxed);
        if !h.rt_logged && t.rt.load(Ordering::Relaxed) != 0 {
            h.rt_logged = true;
            match t.rt.load(Ordering::Relaxed) {
                1 => log::info!(target: "audio", "real-time priority granted"),
                _ => log::warn!(
                    target: "audio",
                    "real-time priority refused: {}",
                    t.rt_error.get().map_or("?", String::as_str)
                ),
            }
        }
        let now = [
            get(&t.xruns),
            get(&t.late),
            get(&t.arrival_late),
            get(&t.other_errors),
            get(&t.count),
        ];
        let secs = h.at.elapsed().as_secs_f64();
        if secs >= 10.0 {
            let d: Vec<u64> = now.iter().zip(h.counts).map(|(a, b)| a - b).collect();
            // Xruns and late executions are INFO; arrival lateness alone (common on a busy
            // machine, and not itself a dropout) only at DEBUG.
            if d[0] + d[1] > 0
                || (d[2] > 0 && log::log_enabled!(target: "audio.health", log::Level::Debug))
            {
                log::info!(
                    target: "audio.health",
                    "last {secs:.1}s: xruns={} late_executions={} late_arrivals={} callbacks={} worst_run_us={:.0}",
                    d[0], d[1], d[2], d[4], get(&t.worst_ns) as f64 / 1e3
                );
            }
            let (kinds, lost) = t.errors.since(&mut h.errors_seen);
            if !kinds.is_empty() {
                let last = h
                    .fault_text
                    .as_ref()
                    .map(|(t, at)| (t.as_str(), at.elapsed().as_secs_f64()));
                log::warn!(
                    target: "audio",
                    "{}",
                    kabl_standalone::stream_error_line(secs, &kinds, last, lost)
                );
            }
            h.counts = now;
            h.at = std::time::Instant::now();
        }
        if h.debug_at.elapsed().as_secs() >= 60 {
            h.debug_at = std::time::Instant::now();
            log::debug!(target: "audio.health", "{}", t.line(self.audio.sample_rate));
        }
        let stalled = self.audio.status.starts_with("audio stalled");
        if stalled != h.stalled {
            h.stalled = stalled;
            if stalled {
                log::warn!(target: "audio", "stalled: no callbacks for over 1.5 s after {}", now[4]);
            } else {
                log::info!(target: "audio", "callbacks resumed");
            }
        }
    }
}
