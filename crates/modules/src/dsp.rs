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
        let coeffs = SvfCoeffs::compute(cutoff_hz, resonance, sample_rate);
        self.process_with_coeffs(input, &coeffs)
    }

    /// Same math as `process_lowpass`, but with the coefficients precomputed by the caller —
    /// for when `cutoff_hz`/`resonance` are constant across many calls (e.g. a whole block, or
    /// several voices sharing one cutoff), so the `tan()` in `SvfCoeffs::compute` isn't paid
    /// redundantly every sample. Spike S3 needed this split to get a clean SIMD-vs-scalar
    /// comparison — see docs/decisions.md "Spike S3": recomputing coefficients per sample was
    /// masking the actual state-update speedup being measured.
    #[inline]
    pub fn process_with_coeffs(&mut self, input: f32, coeffs: &SvfCoeffs) -> f32 {
        let v3 = input - self.ic2eq;
        let v1 = coeffs.a1 * self.ic1eq + coeffs.a2 * v3;
        let v2 = self.ic2eq + coeffs.a2 * self.ic1eq + coeffs.a3 * v3;
        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;
        v2
    }

    /// Same recurrence as `process_with_coeffs`, but returning all three simultaneous outputs
    /// (`filter.svf`'s "LP/BP/HP", brief section 8) instead of just the lowpass. `v1`/`v2` below
    /// are the bandpass/lowpass outputs (Zavalishin/Cytomic naming); highpass is the standard
    /// identity `hp = input - k*bp - lp`.
    #[inline]
    pub fn process_with_coeffs_multi(&mut self, input: f32, coeffs: &SvfCoeffs) -> SvfOutputs {
        let v3 = input - self.ic2eq;
        let v1 = coeffs.a1 * self.ic1eq + coeffs.a2 * v3;
        let v2 = self.ic2eq + coeffs.a2 * self.ic1eq + coeffs.a3 * v3;
        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;
        let highpass = input - coeffs.k * v1 - v2;
        SvfOutputs {
            lowpass: v2,
            bandpass: v1,
            highpass,
        }
    }

    /// Convenience wrapper computing coefficients internally — for when `cutoff_hz`/`resonance`
    /// vary sample-to-sample (e.g. audio-rate CV), so there's no useful coefficient to cache.
    /// Mirrors `process_lowpass`'s relationship to `process_with_coeffs`.
    #[inline]
    pub fn process_multi(
        &mut self,
        input: f32,
        cutoff_hz: f32,
        resonance: f32,
        sample_rate: f32,
    ) -> SvfOutputs {
        let coeffs = SvfCoeffs::compute(cutoff_hz, resonance, sample_rate);
        self.process_with_coeffs_multi(input, &coeffs)
    }
}

/// TPT SVF coefficients, precomputable once for however long `cutoff_hz`/`resonance` hold
/// steady (see `Svf::process_with_coeffs`).
#[derive(Debug, Clone, Copy)]
pub struct SvfCoeffs {
    pub a1: f32,
    pub a2: f32,
    pub a3: f32,
    /// Damping term, needed only for the highpass identity in `SvfOutputs`
    /// (`hp = input - k*bp - lp`) — `process_with_coeffs`'s lowpass-only path never needed it.
    pub k: f32,
}

impl SvfCoeffs {
    #[inline]
    pub fn compute(cutoff_hz: f32, resonance: f32, sample_rate: f32) -> Self {
        let g = (PI * cutoff_hz / sample_rate).tan();
        let k = 2.0 - 2.0 * resonance.clamp(0.0, 0.95);
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        SvfCoeffs { a1, a2, a3, k }
    }
}

/// The SVF's three simultaneous outputs — brief section 8's `filter.svf` wants "LP/BP/HP" all
/// available, not a mode switch (the TPT/Zavalishin topology computes all three from the same
/// two integrator states each sample, so there's no extra cost to exposing all of them).
#[derive(Debug, Clone, Copy)]
pub struct SvfOutputs {
    pub lowpass: f32,
    pub bandpass: f32,
    pub highpass: f32,
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

/// Full 4-stage envelope (brief's `env.adsr`: "exponential segments, gate in") — `Adsr` above
/// only models attack-to-sustain (all S1/S2 needed, and their tests/ratios depend on its exact
/// behavior, so it's left alone rather than extended in place). Attack/decay/release are one-pole
/// exponential segments, matching the brief's wording exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdsrStage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

impl AdsrStage {
    pub fn to_u8(self) -> u8 {
        match self {
            AdsrStage::Idle => 0,
            AdsrStage::Attack => 1,
            AdsrStage::Decay => 2,
            AdsrStage::Sustain => 3,
            AdsrStage::Release => 4,
        }
    }

    /// Any value outside 0..=4 maps to `Idle` — state loaded from a corrupt/foreign source
    /// should fail safe, not panic on the audio thread.
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => AdsrStage::Attack,
            2 => AdsrStage::Decay,
            3 => AdsrStage::Sustain,
            4 => AdsrStage::Release,
            _ => AdsrStage::Idle,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FullAdsr {
    pub stage: AdsrStage,
    pub level: f32,
    attack_coeff: f32,
    decay_coeff: f32,
    release_coeff: f32,
    sustain_level: f32,
    /// Whether `next()` saw the gate high last call — edge-triggers `Attack`/`Release` entry.
    /// `pub`, like `stage`/`level`: a module holding a `FullAdsr` needs to carry this across a
    /// recompile exactly like the other two, or a continuously-held gate looks like a fresh
    /// note-on every time state is reloaded into a freshly-constructed instance (whose `new()`
    /// always starts this `false`) — see `builtins::env_adsr`'s `save_state`/`load_state`.
    pub gate_was_high: bool,
}

/// Segment considered complete once within this of its target — same reasoning as
/// `ADSR_SETTLE_EPSILON` above (a one-pole recurrence only reaches its target asymptotically).
const FULL_ADSR_SEGMENT_EPSILON: f32 = 1e-4;

impl FullAdsr {
    pub fn new(
        attack_ms: f32,
        decay_ms: f32,
        sustain_level: f32,
        release_ms: f32,
        sample_rate: f32,
    ) -> Self {
        let coeff = |ms: f32| (-1.0 / (ms.max(0.01) * 0.001 * sample_rate)).exp();
        FullAdsr {
            stage: AdsrStage::Idle,
            level: 0.0,
            attack_coeff: coeff(attack_ms),
            decay_coeff: coeff(decay_ms),
            release_coeff: coeff(release_ms),
            sustain_level: sustain_level.clamp(0.0, 1.0),
            gate_was_high: false,
        }
    }

    /// Recompute coefficients from possibly-changed params without resetting `level`/`stage` —
    /// used when a module's params are read once per block rather than per sample (see
    /// `builtins::env_adsr`'s doc comment for why).
    pub fn set_params(
        &mut self,
        attack_ms: f32,
        decay_ms: f32,
        sustain_level: f32,
        release_ms: f32,
        sample_rate: f32,
    ) {
        let coeff = |ms: f32| (-1.0 / (ms.max(0.01) * 0.001 * sample_rate)).exp();
        self.attack_coeff = coeff(attack_ms);
        self.decay_coeff = coeff(decay_ms);
        self.release_coeff = coeff(release_ms);
        self.sustain_level = sustain_level.clamp(0.0, 1.0);
    }

    /// Back to `Idle` at `level` 0 — keeps the currently-set coefficients (`set_params`/`new`),
    /// only clears the envelope's run state.
    pub fn reset(&mut self) {
        self.stage = AdsrStage::Idle;
        self.level = 0.0;
        self.gate_was_high = false;
    }

    #[inline]
    pub fn next(&mut self, gate: f32) -> f32 {
        let gate_high = gate > 0.5;
        if gate_high && !self.gate_was_high {
            self.stage = AdsrStage::Attack;
        } else if !gate_high && self.gate_was_high {
            self.stage = AdsrStage::Release;
        }
        self.gate_was_high = gate_high;

        match self.stage {
            AdsrStage::Idle => self.level = 0.0,
            AdsrStage::Attack => {
                self.level += (1.0 - self.level) * (1.0 - self.attack_coeff);
                if self.level >= 1.0 - FULL_ADSR_SEGMENT_EPSILON {
                    self.level = 1.0;
                    self.stage = AdsrStage::Decay;
                }
            }
            AdsrStage::Decay => {
                self.level =
                    self.sustain_level + (self.level - self.sustain_level) * self.decay_coeff;
                if (self.level - self.sustain_level).abs() < FULL_ADSR_SEGMENT_EPSILON {
                    self.level = self.sustain_level;
                    self.stage = AdsrStage::Sustain;
                }
            }
            AdsrStage::Sustain => self.level = self.sustain_level,
            AdsrStage::Release => {
                self.level *= self.release_coeff;
                if self.level < FULL_ADSR_SEGMENT_EPSILON {
                    self.level = 0.0;
                    self.stage = AdsrStage::Idle;
                }
            }
        }
        self.level
    }

    #[inline]
    pub fn is_settled(&self) -> bool {
        matches!(self.stage, AdsrStage::Idle | AdsrStage::Sustain)
    }
}

/// brief's `lfo`: "sine/tri/saw/square/S&H, rate in Hz, sync later." All five waveforms —
/// unlike `osc.va`'s audio-rate saw, an LFO's rate is low enough (brief section 7 itself names
/// "~100 Hz" as the control-rate cutoff) that none of these need anti-aliasing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LfoWaveform {
    Sine,
    Triangle,
    Saw,
    Square,
    SampleAndHold,
}

#[derive(Debug, Clone, Copy)]
pub struct FullLfo {
    pub phase: f32,
    rng_state: u32,
    held_sample: f32,
}

impl Default for FullLfo {
    fn default() -> Self {
        Self::new()
    }
}

impl FullLfo {
    pub fn new() -> Self {
        FullLfo {
            phase: 0.0,
            // Fixed nonzero seed: xorshift32 is undefined at 0. Determinism (not
            // unpredictability) is what matters for an LFO's S&H — same seed every render.
            rng_state: 0x1234_5678,
            held_sample: 0.0,
        }
    }

    #[inline]
    pub fn next(&mut self, rate_hz: f32, sample_rate: f32, waveform: LfoWaveform) -> f32 {
        let v = match waveform {
            LfoWaveform::Sine => (2.0 * PI * self.phase).sin(),
            LfoWaveform::Triangle => 4.0 * (self.phase - 0.5).abs() - 1.0,
            LfoWaveform::Saw => 2.0 * self.phase - 1.0,
            LfoWaveform::Square => {
                if self.phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            LfoWaveform::SampleAndHold => self.held_sample,
        };

        self.phase += rate_hz / sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
            // xorshift32 (Marsaglia): no allocation, no syscall, RT-safe. Not a new dependency —
            // pulling in `rand` for one PRNG call would be disproportionate (brief section 2:
            // no new dependency without a decisions.md line, and this doesn't need one).
            self.rng_state ^= self.rng_state << 13;
            self.rng_state ^= self.rng_state >> 17;
            self.rng_state ^= self.rng_state << 5;
            self.held_sample = (self.rng_state as f32 / u32::MAX as f32) * 2.0 - 1.0;
        }
        v
    }

    /// The sample-and-hold waveform's currently-held value — exposed for state carry-over
    /// (brief section 7.5) so a repatch doesn't reset it. `rng_state` itself isn't carried over
    /// (a repatch getting a fresh S&H sequence rather than continuing the exact same one is a
    /// minor, acceptable simplification — the held *value* surviving is what a listener notices).
    pub fn held_sample(&self) -> f32 {
        self.held_sample
    }

    pub fn set_held_sample(&mut self, value: f32) {
        self.held_sample = value;
    }
}

/// `osc.va`'s waveform selection (brief section 8's v1 table: "sine/square/tri/saw, PolyBLEP,
/// hard sync input").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OscWaveform {
    Sine,
    Triangle,
    Saw,
    Square,
}

/// Virtual-analog oscillator with all four brief-section-8 waveforms plus hard sync — the full
/// `osc.va` table entry (`Saw` above only ever did the saw case, kept as-is since S1-S3's spikes
/// depend on its exact shape).
///
/// **Saw and square are PolyBLEP-corrected** (`poly_blep`, same function `Saw` uses — square
/// applies it at both of its discontinuities: phase 0's rising edge and phase 0.5's falling
/// edge, the standard two-correction extension of the same technique). **Triangle is naive, not
/// band-limited** — a real PolyBLEP triangle needs "polyBLAMP" (an integrated correction at the
/// two corners, not the single discontinuity BLEP handles), which is a distinct algorithm this
/// pass didn't implement. Triangle's corners alias at high fundamental frequencies the way an
/// uncorrected square would; documented here rather than silently claiming full anti-aliasing
/// for all four waveforms. Sine needs no correction (it has no discontinuity to alias).
#[derive(Debug, Clone, Copy)]
pub struct FullOsc {
    pub phase: f32,
}

impl Default for FullOsc {
    fn default() -> Self {
        Self::new()
    }
}

impl FullOsc {
    pub fn new() -> Self {
        FullOsc { phase: 0.0 }
    }

    /// Hard sync (brief section 8): snap phase back to 0 immediately. The caller (`builtins::
    /// osc_va`) edge-detects the sync input and calls this once per rising edge — the resulting
    /// phase discontinuity is what gives hard sync its characteristic sound, so this
    /// deliberately does *not* try to smooth or band-limit the snap itself.
    pub fn hard_sync(&mut self) {
        self.phase = 0.0;
    }

    #[inline]
    pub fn next(&mut self, freq_hz: f32, sample_rate: f32, waveform: OscWaveform) -> f32 {
        let dt = freq_hz / sample_rate;
        let v = match waveform {
            OscWaveform::Saw => {
                let mut v = 2.0 * self.phase - 1.0;
                v -= poly_blep(self.phase, dt);
                v
            }
            OscWaveform::Square => {
                let mut v = if self.phase < 0.5 { 1.0 } else { -1.0 };
                v += poly_blep(self.phase, dt);
                v -= poly_blep((self.phase + 0.5).fract(), dt);
                v
            }
            OscWaveform::Triangle => {
                // Naive -- see the struct doc for why this one isn't PolyBLEP/BLAMP-corrected.
                // Standard piecewise-linear triangle: 0 at phase 0, +1 at phase 0.25, -1 at
                // phase 0.75, back to 0 at phase 1.
                if self.phase < 0.25 {
                    4.0 * self.phase
                } else if self.phase < 0.75 {
                    2.0 - 4.0 * self.phase
                } else {
                    4.0 * self.phase - 4.0
                }
            }
            OscWaveform::Sine => (2.0 * PI * self.phase).sin(),
        };
        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        v
    }
}
