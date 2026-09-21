//! Spike S3 (brief section 11): "Does f32x4 voice batching beat scalar by >=2.5x on osc+filter?"
//!
//! Same osc+filter math as `dsp.rs`'s `Saw`/`Svf` (PolyBLEP saw into a TPT SVF lowpass), but
//! `SawX4`/`SvfX4` process all 4 voices' state in one `f32x4` lane per operation instead of
//! looping over 4 independent scalar instances (brief section 7: "process voices in lanes of 4
//! ... batch across voices, not across modules"). Filter cutoff and resonance are shared across
//! the patch's voices already (matching S1/S2's patches), so their coefficients (`a1`/`a2`/`a3`,
//! which need `tan`) stay plain `f32` scalars broadcast with `splat` — only oscillator phase and
//! filter state, which genuinely differ per voice, need to be SIMD lanes. That sidesteps needing
//! SIMD trig entirely for this spike.
//!
//! `wide` chosen per brief section 14 ("portable SIMD crate or std::arch behind a small
//! abstraction, document the choice"): stable-Rust compatible (MSRV 1.89; this workspace runs
//! stable, and `std::simd`/`core::simd` is nightly-only), picks SSE2/NEON/wasm128 automatically
//! per target behind a safe API — see docs/decisions.md "Spike S3" for the version and date
//! checked.
//!
//! Filter coefficients (`dsp::SvfCoeffs`, needing a `tan()` call) are computed once in `new()`
//! and reused every sample via `process_with_coeffs`/`SvfX4::process_with_coeffs`, not
//! recomputed per call — an earlier version recomputed them every sample in both the scalar and
//! SIMD paths, which buried the actual state-update speedup under an identical fixed `tan()`
//! cost paid by both sides. See docs/decisions.md "Spike S3" for the before/after numbers.

use wide::f32x4;

/// Branchless PolyBLEP (brief section 8's oscillator reference, Välimäki et al.) — the scalar
/// version in `dsp.rs` branches on `t`; SIMD has no per-lane branch, so this computes both
/// regions and selects with a mask instead.
#[inline]
fn poly_blep_x4(t: f32x4, dt: f32x4) -> f32x4 {
    let one = f32x4::splat(1.0);
    let zero = f32x4::splat(0.0);

    let t1 = t / dt;
    let region_lt = t1 + t1 - t1 * t1 - one;

    let t2 = (t - one) / dt;
    let region_gt = t2 * t2 + t2 + t2 + one;

    let gt_mask = t.simd_gt(one - dt);
    let region2_or_zero = gt_mask.select(region_gt, zero);

    let lt_mask = t.simd_lt(dt);
    lt_mask.select(region_lt, region2_or_zero)
}

/// 4 independent saw oscillators' phases, one per lane.
#[derive(Debug, Clone, Copy)]
pub struct SawX4 {
    pub phase: f32x4,
}

impl SawX4 {
    pub fn new() -> Self {
        SawX4 {
            phase: f32x4::splat(0.0),
        }
    }

    #[inline]
    pub fn next(&mut self, freq_hz: f32x4, sample_rate: f32) -> f32x4 {
        let dt = freq_hz / f32x4::splat(sample_rate);
        let mut v = f32x4::splat(2.0) * self.phase - f32x4::splat(1.0);
        v -= poly_blep_x4(self.phase, dt);
        self.phase += dt;
        let wrapped = self.phase - f32x4::splat(1.0);
        let wrap_mask = self.phase.simd_ge(f32x4::splat(1.0));
        self.phase = wrap_mask.select(wrapped, self.phase);
        v
    }
}

/// 4 independent TPT SVF lowpass filters' state, one per lane. Coefficients are shared scalars
/// (see module doc) — only `ic1eq`/`ic2eq` are per-lane.
#[derive(Debug, Clone, Copy)]
pub struct SvfX4 {
    pub ic1eq: f32x4,
    pub ic2eq: f32x4,
}

impl SvfX4 {
    pub fn new() -> Self {
        SvfX4 {
            ic1eq: f32x4::splat(0.0),
            ic2eq: f32x4::splat(0.0),
        }
    }

    /// Coefficients precomputed by the caller — see `dsp::Svf::process_with_coeffs`. All 4
    /// voices share one cutoff/resonance (same as S1/S2's patches), so this is a genuine
    /// broadcast, not an approximation: every lane gets the identical `a1`/`a2`/`a3`.
    #[inline]
    pub fn process_with_coeffs(&mut self, input: f32x4, coeffs: &crate::dsp::SvfCoeffs) -> f32x4 {
        let a1 = f32x4::splat(coeffs.a1);
        let a2 = f32x4::splat(coeffs.a2);
        let a3 = f32x4::splat(coeffs.a3);

        let v3 = input - self.ic2eq;
        let v1 = a1 * self.ic1eq + a2 * v3;
        let v2 = self.ic2eq + a2 * self.ic1eq + a3 * v3;
        self.ic1eq = f32x4::splat(2.0) * v1 - self.ic1eq;
        self.ic2eq = f32x4::splat(2.0) * v2 - self.ic2eq;
        v2
    }
}

/// Same 4-voice chord as S1/S2 (`graph::CHORD_HZ`, `potato::VOICE_HZ`), reused here so S3's
/// comparison is against the same signal the other spikes already established.
pub const VOICE_HZ: [f32; 4] = [110.0, 130.81, 164.81, 220.0];
pub const CUTOFF_HZ: f32 = 1500.0;
pub const RESONANCE: f32 = 0.3;

/// Scalar baseline: 4 independent `Saw`/`Svf` instances, looped. This is exactly what S1's
/// `CompiledGraph` and S2's `PotatoPatch` already do per-voice — pulled out on its own here so
/// the comparison isolates osc+filter cost, with nothing else (no cable depth, no mixing) to
/// muddy the measurement.
pub struct ScalarVoices {
    voices: [(crate::dsp::Saw, crate::dsp::Svf); 4],
    coeffs: crate::dsp::SvfCoeffs,
}

impl ScalarVoices {
    pub fn new(sample_rate: f32) -> Self {
        ScalarVoices {
            voices: std::array::from_fn(|_| (crate::dsp::Saw::new(), crate::dsp::Svf::new())),
            coeffs: crate::dsp::SvfCoeffs::compute(CUTOFF_HZ, RESONANCE, sample_rate),
        }
    }

    #[inline]
    pub fn next(&mut self, sample_rate: f32) -> [f32; 4] {
        std::array::from_fn(|i| {
            let (osc, filter) = &mut self.voices[i];
            let osc_v = osc.next(VOICE_HZ[i], sample_rate);
            filter.process_with_coeffs(osc_v, &self.coeffs)
        })
    }

    /// Block-at-a-time, matching how the real engine actually calls into voice processing (one
    /// call per `BLOCK` samples, per brief section 7's flat schedule — not one call per sample
    /// from outside). Added to check whether per-call overhead was skewing the per-sample
    /// numbers; it gives the same ratio, so it wasn't — see docs/decisions.md "Spike S3".
    #[inline]
    pub fn process_block(&mut self, sample_rate: f32, out: &mut [[f32; 4]; crate::graph::BLOCK]) {
        for sample in out.iter_mut() {
            *sample = self.next(sample_rate);
        }
    }
}

/// SIMD version of the same 4 voices: one `SawX4` + one `SvfX4` instead of 4 scalar pairs.
pub struct SimdVoices {
    osc: SawX4,
    filter: SvfX4,
    freq: f32x4,
    coeffs: crate::dsp::SvfCoeffs,
}

impl SimdVoices {
    pub fn new(sample_rate: f32) -> Self {
        SimdVoices {
            osc: SawX4::new(),
            filter: SvfX4::new(),
            freq: f32x4::new(VOICE_HZ),
            coeffs: crate::dsp::SvfCoeffs::compute(CUTOFF_HZ, RESONANCE, sample_rate),
        }
    }

    #[inline]
    pub fn next(&mut self, sample_rate: f32) -> f32x4 {
        let osc_v = self.osc.next(self.freq, sample_rate);
        self.filter.process_with_coeffs(osc_v, &self.coeffs)
    }

    /// Block-at-a-time — see `ScalarVoices::process_block`.
    #[inline]
    pub fn process_block(&mut self, sample_rate: f32, out: &mut [f32x4; crate::graph::BLOCK]) {
        for sample in out.iter_mut() {
            *sample = self.next(sample_rate);
        }
    }
}

impl Default for SawX4 {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for SvfX4 {
    fn default() -> Self {
        Self::new()
    }
}
