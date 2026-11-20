//! Audio-callback telemetry, the start-up gate's progress watch and the CC latency probe.

use super::*;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Audio-callback telemetry, written by the callback (and the stream's error callback) with
/// relaxed atomics, read by the status bar and `KABL_STATS_FILE`. Kept apart:
/// - execution: how long the callback ran, against the audio it produced (its budget);
/// - arrival: the interval between callback starts, against the same period;
/// - xruns the backend reported (`cpal::ErrorKind::Xrun`);
/// - the frames each callback actually asked for.
///
/// The first second (stream start-up) is counted apart.
#[derive(Default)]
pub(crate) struct CallbackTiming {
    pub(crate) count: AtomicU64,
    pub(crate) worst_ns: AtomicU64,
    /// Seconds into the stream of the worst execution (to line it up with what happened).
    pub(crate) worst_at_ms: AtomicU64,
    /// Wall-clock time of the worst execution, ms since the Unix epoch.
    pub(crate) worst_wall_ms: AtomicU64,
    /// Execution longer than the callback's audio (after the first second).
    pub(crate) late: AtomicU64,
    /// Execution over half of it.
    pub(crate) over_half: AtomicU64,
    pub(crate) startup_worst_ns: AtomicU64,
    /// Longest interval between callback starts, and intervals over 1.5 × the period.
    pub(crate) arrival_worst_ns: AtomicU64,
    pub(crate) arrival_late: AtomicU64,
    pub(crate) frames_min: AtomicU64,
    pub(crate) frames_max: AtomicU64,
    pub(crate) xruns: AtomicU64,
    pub(crate) other_errors: AtomicU64,
    /// Stream errors the error callback handed over, by kind, and those that found the queue
    /// full (dropped there).
    pub(crate) errors: kabl_standalone::StreamErrorCounts,
    /// Frames delivered to the device: the audio clock probe reports are aged against.
    pub(crate) frames_total: AtomicU64,
    /// Execution times after the first second, in `HIST_BIN_US` bins (the last one open).
    pub(crate) hist: Hist,
    /// 0 not tried, 1 granted, 2 refused.
    pub(crate) rt: std::sync::atomic::AtomicU8,
    pub(crate) rt_error: std::sync::OnceLock<String>,
    /// MIDI key events scheduled, and those that arrived before the previous callback
    /// started or past its length (placed at an edge).
    pub(crate) midi_events: AtomicU64,
    pub(crate) midi_late: AtomicU64,
    /// Arrival to the scheduled frame of the callback that renders it (excluding the
    /// adapter's fixed 64 frames and the device's own latency), shortest and longest.
    pub(crate) midi_wait_min_ns: AtomicU64,
    pub(crate) midi_wait_max_ns: AtomicU64,
    /// Adapter queue: events dropped or forced to a release, and events moved in time.
    pub(crate) timeline_dropped: AtomicU64,
    pub(crate) timeline_moved: AtomicU64,
    /// Smoothed share of the callback's audio period spent executing, in permille (the
    /// status bar's CPU readout).
    pub(crate) load_permille: AtomicU64,
}

pub(crate) struct Hist([AtomicU64; HIST_BINS]);

impl Default for Hist {
    fn default() -> Self {
        Hist(std::array::from_fn(|_| AtomicU64::new(0)))
    }
}

/// Execution histogram: 64 bins of 50 µs (the last collects everything above 3.15 ms).
pub(crate) const HIST_BINS: usize = 64;
pub(crate) const HIST_BIN_US: u64 = 50;

impl CallbackTiming {
    /// `p` quantile (0..1) of the execution histogram, as the bin's upper edge in µs.
    pub(crate) fn quantile(&self, p: f64) -> Option<u64> {
        let counts: Vec<u64> = self
            .hist
            .0
            .iter()
            .map(|b| b.load(Ordering::Relaxed))
            .collect();
        let total: u64 = counts.iter().sum();
        if total == 0 {
            return None;
        }
        let want = (total as f64 * p).ceil() as u64;
        let mut seen = 0;
        for (i, c) in counts.iter().enumerate() {
            seen += c;
            if seen >= want {
                return Some((i as u64 + 1) * HIST_BIN_US);
            }
        }
        None
    }

    /// The histogram for `KABL_STATS_FILE`: nonzero bins as `upper_µs:count`.
    pub(crate) fn hist_line(&self) -> String {
        let open = HIST_BINS as u64 * HIST_BIN_US;
        let q = |p| {
            self.quantile(p).map_or("-".into(), |v| {
                if v == open {
                    format!(">{}", open - HIST_BIN_US)
                } else {
                    format!("≤{v}")
                }
            })
        };
        let bins: Vec<String> = self
            .hist
            .0
            .iter()
            .enumerate()
            .filter_map(|(i, b)| {
                let n = b.load(Ordering::Relaxed);
                (n > 0).then(|| {
                    if i + 1 == HIST_BINS {
                        format!(">{}:{n}", i as u64 * HIST_BIN_US)
                    } else {
                        format!("{}:{n}", (i as u64 + 1) * HIST_BIN_US)
                    }
                })
            })
            .collect();
        format!(
            "run histogram ({HIST_BIN_US} µs bins, after the first second): p50 {} p99 {} p99.9 {} µs · {}",
            q(0.5),
            q(0.99),
            q(0.999),
            bins.join(" ")
        )
    }

    pub(crate) fn record(
        &self,
        took: std::time::Duration,
        since_last: Option<std::time::Duration>,
        frames: usize,
        sample_rate: f32,
    ) {
        let ns = took.as_nanos() as u64;
        self.frames_total
            .fetch_add(frames as u64, Ordering::Relaxed);
        let n = self.count.fetch_add(1, Ordering::Relaxed);
        if n == 0 {
            self.frames_min.store(u64::MAX, Ordering::Relaxed);
        }
        self.frames_min.fetch_min(frames as u64, Ordering::Relaxed);
        self.frames_max.fetch_max(frames as u64, Ordering::Relaxed);
        // One writer (the audio thread): a plain read-modify-write is enough for the average.
        let share = ns as f64 / (frames as f64 / sample_rate as f64 * 1e9) * 1000.0;
        let old = self.load_permille.load(Ordering::Relaxed) as f64;
        self.load_permille
            .store((old * 0.95 + share * 0.05) as u64, Ordering::Relaxed);
        if (n * frames as u64) < sample_rate as u64 {
            self.startup_worst_ns.fetch_max(ns, Ordering::Relaxed);
            return;
        }
        let budget = frames as f64 / sample_rate as f64 * 1e9;
        let bin = ((ns / 1000 / HIST_BIN_US) as usize).min(HIST_BINS - 1);
        self.hist.0[bin].fetch_add(1, Ordering::Relaxed);
        if self.worst_ns.fetch_max(ns, Ordering::Relaxed) < ns {
            let ms = n as f64 * frames as f64 / sample_rate as f64 * 1e3;
            self.worst_at_ms.store(ms as u64, Ordering::Relaxed);
            let wall = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64);
            self.worst_wall_ms.store(wall, Ordering::Relaxed);
        }
        if ns as f64 > budget {
            self.late.fetch_add(1, Ordering::Relaxed);
        }
        if ns as f64 > budget / 2.0 {
            self.over_half.fetch_add(1, Ordering::Relaxed);
        }
        if let Some(gap) = since_last {
            let g = gap.as_nanos() as u64;
            self.arrival_worst_ns.fetch_max(g, Ordering::Relaxed);
            if g as f64 > 1.5 * budget {
                self.arrival_late.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    pub(crate) fn error(&self, err: &cpal::Error) {
        if err.kind() == cpal::ErrorKind::Xrun {
            self.xruns.fetch_add(1, Ordering::Relaxed);
        } else {
            self.other_errors.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// A short status-bar line and the full report.
    pub(crate) fn summary(&self, sample_rate: f32) -> (String, String) {
        let get = |a: &AtomicU64| a.load(Ordering::Relaxed);
        let short = format!(
            "callback worst {:.0}/{:.0} µs · {} late · {} xruns · RT {}",
            get(&self.worst_ns) as f64 / 1e3,
            get(&self.frames_max) as f64 / sample_rate as f64 * 1e6,
            get(&self.late) + get(&self.arrival_late),
            get(&self.xruns),
            match self.rt.load(Ordering::Relaxed) {
                1 => "on",
                2 => "refused",
                _ => "off",
            }
        );
        (short, self.line(sample_rate))
    }

    pub(crate) fn line(&self, sample_rate: f32) -> String {
        let get = |a: &AtomicU64| a.load(Ordering::Relaxed);
        let us = |a: &AtomicU64| get(a) as f64 / 1e3;
        let (fmin, fmax) = (get(&self.frames_min), get(&self.frames_max));
        let frames = if fmin == fmax {
            format!("{fmax}")
        } else {
            format!("{fmin}–{fmax}")
        };
        let rt = match self.rt.load(Ordering::Relaxed) {
            1 => "RT priority on".to_string(),
            2 => format!(
                "RT priority refused ({})",
                self.rt_error.get().map_or("?", String::as_str)
            ),
            _ => "RT priority not tried".to_string(),
        };
        format!(
            "{} callbacks × {frames} frames at {sample_rate} Hz · run: worst {:.0} of {:.0} µs \
             (at {:.1} s, wall {}), {} over half, {} late · arrival: worst {:.0} µs, {} late · {} xruns · {rt} \
             (first second: worst {:.0} µs) · MIDI: {} events, {} late, arrival to frame \
             {:.2}–{:.2} ms · adapter: {} frames latency, {} dropped, {} moved",
            get(&self.count),
            us(&self.worst_ns),
            fmax as f64 / sample_rate as f64 * 1e6,
            get(&self.worst_at_ms) as f64 / 1e3,
            get(&self.worst_wall_ms),
            get(&self.over_half),
            get(&self.late),
            us(&self.arrival_worst_ns),
            get(&self.arrival_late),
            get(&self.xruns),
            us(&self.startup_worst_ns),
            get(&self.midi_events),
            get(&self.midi_late),
            // No events yet: no range ("-"), not a zero delay.
            if get(&self.midi_events) == 0 { f64::NAN } else { get(&self.midi_wait_min_ns) as f64 / 1e6 },
            if get(&self.midi_events) == 0 { f64::NAN } else { get(&self.midi_wait_max_ns) as f64 / 1e6 },
            kabl_engine::timeline::LATENCY,
            get(&self.timeline_dropped),
            get(&self.timeline_moved),
        )
    }
}

/// Sticky detection also runs while the UI is suspended.
pub(crate) struct Progress {
    pub(crate) timing: Arc<CallbackTiming>,
    pub(crate) gate: Arc<AtomicU64>,
    pub(crate) last: u64,
    pub(crate) at: std::time::Instant,
    pub(crate) failure: Option<AudioPhase>,
}

pub(crate) fn unmute_if_current(gate: &AtomicU64, revision: u64) -> bool {
    gate.compare_exchange(revision, u64::MAX, Ordering::AcqRel, Ordering::Relaxed)
        .is_ok()
}

impl Progress {
    pub(crate) fn new(timing: Arc<CallbackTiming>, gate: Arc<AtomicU64>) -> Self {
        Self {
            timing,
            gate,
            last: 0,
            at: std::time::Instant::now(),
            failure: None,
        }
    }
    pub(crate) fn observe(&mut self) {
        if self.failure.is_some() {
            self.gate.store(0, Ordering::Release);
            return;
        }
        let n = self.timing.count.load(Ordering::Relaxed);
        if self.timing.other_errors.load(Ordering::Relaxed) > 0 {
            self.failure = Some(AudioPhase::Failed);
        } else if n != self.last {
            self.last = n;
            self.at = std::time::Instant::now();
        } else if self.at.elapsed().as_secs_f32() > if n == 0 { 3.0 } else { 1.5 } {
            self.failure = Some(AudioPhase::Stalled);
        }
        if self.failure.is_some() {
            self.gate.store(0, Ordering::Relaxed);
        }
    }
}

/// Measurement only (`KABL_LATENCY_FILE`): from a CC's arrival at the MIDI input to the audio
/// callback that took the change it made (a value, or a graph for a structural fallback).
#[derive(Default)]
pub(crate) struct Latency {
    /// (revision a CC batch ended on, the batch's earliest arrival), oldest first.
    pub(crate) waiting: std::collections::VecDeque<(u64, f64)>,
    /// Seconds.
    pub(crate) samples: Vec<f64>,
}

impl Latency {
    pub(crate) fn applied(&mut self, rev: u64, at: f64) {
        while self.waiting.front().is_some_and(|w| w.0 <= rev) {
            let (_, arrived) = self.waiting.pop_front().unwrap();
            self.samples.push(at - arrived);
        }
    }

    pub(crate) fn line(&self) -> String {
        let mut v = self.samples.clone();
        v.sort_by(f64::total_cmp);
        let q = |p: f64| {
            v.get(((v.len() as f64 * p).ceil() as usize).saturating_sub(1))
                .map_or("-".into(), |x| format!("{:.2}", x * 1e3))
        };
        format!(
            "CC to audio callback: {} batches · p50 {} ms · p90 {} ms · p99 {} ms · max {} ms · {} waiting",
            v.len(),
            q(0.5),
            q(0.9),
            q(0.99),
            q(1.0),
            self.waiting.len()
        )
    }
}

/// Seconds since the process started: the control layer's steady clock (CC gestures, the
/// button guard).
pub(crate) fn clock_s() -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_secs_f64()
}
