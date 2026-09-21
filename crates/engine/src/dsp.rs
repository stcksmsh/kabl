//! Minimal DSP blocks for spikes S1 and S2. Not the v1 module system (brief section 8) — that
//! needs the module registry / `ModuleInfo` machinery, which doesn't exist yet. This is just
//! enough real signal to make the spike tests meaningful instead of testing silence.

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

/// Sine LFO. brief section 7's control-rate tier names "LFOs below ~100 Hz" as a block-held-
/// scalar candidate explicitly — `next` is the naive per-sample path, `next_block_held` is the
/// optimized one (advances phase by a whole block in one step, returns the value once).
#[derive(Debug, Clone, Copy, Default)]
pub struct Lfo {
    pub phase: f32,
}

impl Lfo {
    pub fn new() -> Self {
        Lfo { phase: 0.0 }
    }

    #[inline]
    pub fn next(&mut self, rate_hz: f32, sample_rate: f32) -> f32 {
        let v = (2.0 * PI * self.phase).sin();
        self.phase += rate_hz / sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        v
    }

    #[inline]
    pub fn next_block_held(&mut self, rate_hz: f32, sample_rate: f32, block_len: usize) -> f32 {
        let v = (2.0 * PI * self.phase).sin();
        self.phase += rate_hz / sample_rate * block_len as f32;
        self.phase -= self.phase.floor();
        v
    }
}

/// One-pole attack/sustain envelope (brief's `env.adsr`, simplified: this spike only needs
/// "attack up to a sustained gate," since the patch holds its gate the whole render — decay/
/// release aren't exercised). brief section 7's control-rate tier also names "envelopes in
/// sustain" as a block-held-scalar candidate: once `level` has converged to `target`, further
/// per-sample recomputation of the one-pole recurrence produces the same value every time.
#[derive(Debug, Clone, Copy)]
pub struct Adsr {
    pub level: f32,
    attack_coeff: f32,
}

/// Settled once within this of the sustain target — small enough that holding the scalar
/// instead of recomputing is inaudible (the recurrence has converged to float precision).
const ADSR_SETTLE_EPSILON: f32 = 1e-5;

impl Adsr {
    pub fn new(attack_ms: f32, sample_rate: f32) -> Self {
        let attack_coeff = (-1.0 / (attack_ms * 0.001 * sample_rate)).exp();
        Adsr {
            level: 0.0,
            attack_coeff,
        }
    }

    #[inline]
    pub fn next(&mut self, gate: f32) -> f32 {
        self.level = gate + (self.level - gate) * self.attack_coeff;
        self.level
    }

    #[inline]
    pub fn is_settled(&self, gate: f32) -> bool {
        (self.level - gate).abs() < ADSR_SETTLE_EPSILON
    }
}
