//! Minimal DSP blocks for spike S1. Not the v1 module system (brief section 8) — that needs the
//! module registry / `ModuleInfo` machinery, which doesn't exist yet. This is just enough real
//! signal (an anti-aliased saw into a state-variable filter) to make the swap/crossfade spike
//! test meaningful instead of testing silence.

use std::f32::consts::PI;

/// PolyBLEP correction (Välimäki et al.), the Live-tier oscillator anti-aliasing method named
/// in brief section 7 (quality tiers table).
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        t + t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Saw {
    pub phase: f32,
}

impl Default for Saw {
    fn default() -> Self {
        Self::new()
    }
}

impl Saw {
    pub fn new() -> Self {
        Saw { phase: 0.0 }
    }

    #[inline]
    pub fn next(&mut self, freq_hz: f32, sample_rate: f32) -> f32 {
        let dt = freq_hz / sample_rate;
        let mut v = 2.0 * self.phase - 1.0;
        v -= poly_blep(self.phase, dt);
        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        v
    }
}

/// Topology-preserving-transform state-variable filter (Zavalishin, *The Art of VA Filter
/// Design*), lowpass output. Reference implementation named in brief section 8.
#[derive(Debug, Clone, Copy)]
pub struct Svf {
    pub ic1eq: f32,
    pub ic2eq: f32,
}

impl Default for Svf {
    fn default() -> Self {
        Self::new()
    }
}

impl Svf {
    pub fn new() -> Self {
        Svf {
            ic1eq: 0.0,
            ic2eq: 0.0,
        }
    }

    #[inline]
    pub fn process_lowpass(
        &mut self,
        input: f32,
        cutoff_hz: f32,
        resonance: f32,
        sample_rate: f32,
    ) -> f32 {
        let g = (PI * cutoff_hz / sample_rate).tan();
        let k = 2.0 - 2.0 * resonance.clamp(0.0, 0.95);
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;

        let v3 = input - self.ic2eq;
        let v1 = a1 * self.ic1eq + a2 * v3;
        let v2 = self.ic2eq + a2 * self.ic1eq + a3 * v3;
        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;
        v2
    }
}
