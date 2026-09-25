//! Selected-signal inspection (D03, docs/signal-inspection/design.md): one output jack of one
//! module, measured inside the schedule right after that module wrote it, before the buffer
//! pool can reuse the slot. Everything here is fixed-size and `Copy`: the audio thread never
//! allocates, formats or waits for it.

use kabl_core::ModuleId;

use crate::graph::BLOCK;

/// Most voice lanes one report describes; lanes beyond are not measured.
pub const PROBE_LANES: usize = 16;
/// The engine's gate threshold (a gate input is high above this).
pub const GATE_THRESHOLD: f32 = 0.5;
/// Report window, seconds (rounded up to whole blocks).
pub const WINDOW_SECS: f32 = 0.05;

/// What the UI asked to measure. `token` changes with every selection, so reports for an older
/// one are recognizable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProbeTarget {
    pub token: u64,
    pub module: ModuleId,
    /// Index among the module's output ports.
    pub port: u8,
    /// The module kind the UI saw: a different kind under the same id is not measured.
    pub kind: &'static str,
}

/// One lane's statistics over a window.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LaneStats {
    pub min: f32,
    pub max: f32,
    /// Largest |x|.
    pub peak: f32,
    pub sum_sq: f32,
    /// Rising crossings of `GATE_THRESHOLD` (the first sample counts against the last value
    /// of the previous block; a new tap's first sample is never an edge).
    pub rises: u32,
    /// Samples above `GATE_THRESHOLD`.
    pub high: u32,
    pub last: f32,
    /// NaN or infinite samples (left out of the other figures).
    pub nonfinite: u32,
}

impl LaneStats {
    fn empty() -> Self {
        LaneStats {
            min: f32::INFINITY,
            max: f32::NEG_INFINITY,
            ..Default::default()
        }
    }

    /// RMS over `samples`.
    pub fn rms(&self, samples: u32) -> f32 {
        if samples == 0 {
            0.0
        } else {
            (self.sum_sq / samples as f32).sqrt()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeStatus {
    Measured,
    /// No instance with this id, kind and port in the measured graph.
    NotInGraph,
}

/// One window of measurement, published by the audio thread.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProbeReport {
    pub token: u64,
    /// `CompiledPatch::generation` of the graph measured.
    pub generation: u64,
    /// Counts every report the engine produced (gaps = reports dropped on the way).
    pub seq: u64,
    pub status: ProbeStatus,
    /// Some block of this window was inside a crossfade (the measured graph is the new one).
    pub fading: bool,
    /// Samples per lane in this window.
    pub samples: u32,
    /// Lanes measured (1 = one shared instance).
    pub lanes: u8,
    /// Lanes the module has (more than `lanes` when it has over `PROBE_LANES` voices).
    pub lanes_total: u16,
    /// Compiled per voice.
    pub voiced: bool,
    pub lane: [LaneStats; PROBE_LANES],
}

/// The tap inside one graph: which instances it reads and what it has gathered.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tap {
    pub target: Option<ProbeTarget>,
    /// Module index per lane, in lane order.
    pub modules: [u32; PROBE_LANES],
    pub n: usize,
    pub total: usize,
    pub voiced: bool,
    pub port: usize,
    pub found: bool,
    pub acc: [LaneStats; PROBE_LANES],
    pub prev: [f32; PROBE_LANES],
    pub samples: u32,
    pub blocks: u32,
    pub fading: bool,
}

impl Default for Tap {
    fn default() -> Self {
        Tap {
            target: None,
            modules: [u32::MAX; PROBE_LANES],
            n: 0,
            total: 0,
            voiced: false,
            port: 0,
            found: false,
            acc: [LaneStats::empty(); PROBE_LANES],
            // Unknown until the first block: a signal already high then is not an edge.
            prev: [f32::NAN; PROBE_LANES],
            samples: 0,
            blocks: 0,
            fading: false,
        }
    }
}

impl Tap {
    pub fn reset_window(&mut self) {
        self.acc = [LaneStats::empty(); PROBE_LANES];
        self.samples = 0;
        self.blocks = 0;
        self.fading = false;
    }

    /// Lane of module `index`, if the tap reads it.
    #[inline]
    pub fn lane_of(&self, index: usize) -> Option<usize> {
        self.modules[..self.n]
            .iter()
            .position(|&m| m as usize == index)
    }

    /// Adds one block of lane `lane`.
    #[inline]
    pub fn feed(&mut self, lane: usize, buf: &[f32; BLOCK]) {
        let s = &mut self.acc[lane];
        let mut prev = self.prev[lane];
        for &x in buf {
            if !x.is_finite() {
                s.nonfinite += 1;
                continue;
            }
            s.min = s.min.min(x);
            s.max = s.max.max(x);
            s.peak = s.peak.max(x.abs());
            s.sum_sq += x * x;
            if x > GATE_THRESHOLD {
                s.high += 1;
                // NaN (nothing seen yet) compares false.
                if prev <= GATE_THRESHOLD {
                    s.rises += 1;
                }
            }
            prev = x;
            s.last = x;
        }
        self.prev[lane] = prev;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_sample_pulses_count_and_a_held_gate_is_not_an_edge() {
        let mut t = Tap {
            n: 1,
            ..Tap::default()
        };
        let mut b = [0.0; BLOCK];
        b[10] = 1.0;
        b[11] = 0.2;
        b[BLOCK - 1] = 1.0;
        t.feed(0, &b);
        assert_eq!((t.acc[0].rises, t.acc[0].high), (2, 2));
        // Next block starts high: continues the pulse from the last sample, no new edge.
        t.feed(0, &[1.0; BLOCK]);
        assert_eq!(t.acc[0].rises, 2);
        // A new tap on a gate that is already high: no edge either.
        let mut fresh = Tap {
            n: 1,
            ..Tap::default()
        };
        fresh.feed(0, &[1.0; BLOCK]);
        assert_eq!((fresh.acc[0].rises, fresh.acc[0].high), (0, BLOCK as u32));
        // NaN is counted apart and does not break the figures.
        let mut b = [0.25; BLOCK];
        b[3] = f32::NAN;
        fresh.feed(0, &b);
        assert_eq!(fresh.acc[0].nonfinite, 1);
        assert_eq!(fresh.acc[0].min, 0.25);
    }
}
