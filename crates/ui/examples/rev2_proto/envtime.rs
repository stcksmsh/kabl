//! Envelope-time modulation A/B: the same notes and the same LFO → Attack route, rendered
//! offline under two timing policies. Only the policy differs.
//!
//! - **Continuous:** the modulated Attack is re-read every 64-sample block, as the engine's
//!   `env.adsr` reads its params today, so an attack in progress follows the LFO.
//! - **Stage start:** the modulated Attack is sampled once at note-on and held for that stage.
//!
//! Uses the production `kabl_modules::dsp` primitives (`FullAdsr`, `FullLfo`, `FullOsc`,
//! `SvfCoeffs`) unmodified. The route summing is this harness's own (the compiler has no knob
//! routes yet), so this is a simulation of the proposed behaviour, not engine output.

use kabl_modules::dsp::{FullAdsr, FullLfo, FullOsc, LfoWaveform, OscWaveform, Svf, SvfCoeffs};

pub const SR: f32 = 48_000.0;
pub const BLOCK: usize = 64;
const ATTACK_MIN: f32 = 0.1;
const ATTACK_MAX: f32 = 10_000.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Policy {
    Continuous,
    StageStart,
}

#[derive(Clone, Debug)]
pub struct Scenario {
    pub name: &'static str,
    pub attack_ms: f32,
    pub decay_ms: f32,
    pub sustain: f32,
    pub release_ms: f32,
    pub lfo_hz: f32,
    pub lfo_wave: LfoWaveform,
    /// Route amount in knob travel (−1..1); bipolar LFO.
    pub amount: f32,
    /// (start s, length s, MIDI note)
    pub notes: Vec<(f32, f32, u8)>,
    pub seconds: f32,
}

pub fn lfo_wave(idx: usize) -> LfoWaveform {
    [LfoWaveform::Sine, LfoWaveform::Triangle, LfoWaveform::Saw, LfoWaveform::Square, LfoWaveform::SampleAndHold]
        [idx.min(4)]
}

impl Scenario {
    /// The reference patch's numbers (Attack 8 ms, LFO 0.8 Hz TRI, +25 %).
    pub fn from_patch(attack_ms: f32, lfo_hz: f32, wave: usize, amount: f32) -> Self {
        Scenario {
            name: "patch",
            attack_ms,
            decay_ms: 240.0,
            sustain: 0.6,
            release_ms: 420.0,
            lfo_hz,
            lfo_wave: lfo_wave(wave),
            amount,
            notes: (0..8).map(|i| (0.1 + i as f32 * 0.5, 0.35, [45, 52, 57, 60, 64, 60, 57, 52][i])).collect(),
            seconds: 4.4,
        }
    }

    /// Slow pad: long attack, faster LFO. Where the two policies should differ most.
    pub fn slow_pad() -> Self {
        Scenario {
            name: "slow-pad",
            attack_ms: 400.0,
            decay_ms: 600.0,
            sustain: 0.7,
            release_ms: 900.0,
            lfo_hz: 2.3,
            lfo_wave: LfoWaveform::Sine,
            amount: 0.3,
            notes: (0..4).map(|i| (0.1 + i as f32 * 2.0, 1.5, [45, 48, 52, 43][i])).collect(),
            seconds: 8.6,
        }
    }
}

/// Modulated Attack in ms: base travel + amount × LFO, clamped to the param range.
pub fn attack_at(base_ms: f32, amount: f32, lfo: f32) -> f32 {
    let span = (ATTACK_MAX / ATTACK_MIN).ln();
    let t = ((base_ms / ATTACK_MIN).ln() / span + amount * lfo).clamp(0.0, 1.0);
    ATTACK_MIN * (span * t).exp()
}

pub struct Render {
    pub env: Vec<f32>,
    pub audio: Vec<f32>,
    /// Effective Attack (ms) actually in use at each block, for plotting.
    pub attack_used: Vec<f32>,
    /// Per note: seconds from note-on until the envelope first reaches 0.9.
    pub rise_90: Vec<Option<f32>>,
}

pub fn render(sc: &Scenario, policy: Policy) -> Render {
    let n = (sc.seconds * SR) as usize;
    let mut env = FullAdsr::new(sc.attack_ms, sc.decay_ms, sc.sustain, sc.release_ms, SR);
    let mut lfo = FullLfo::new();
    let mut osc = FullOsc::new();
    let mut svf = Svf::new();
    let coeffs = SvfCoeffs::compute(1500.0, 0.3, SR);
    let (mut out_env, mut out_audio) = (Vec::with_capacity(n), Vec::with_capacity(n));
    let mut attack_used = Vec::with_capacity(n / BLOCK + 1);
    let mut latched = sc.attack_ms;
    let mut lfo_now = 0.0;
    for i in 0..n {
        let t = i as f32 / SR;
        let note = sc.notes.iter().find(|(s, l, _)| t >= *s && t < s + l);
        let gate = if note.is_some() { 1.0 } else { 0.0 };
        if i % BLOCK == 0 {
            // The engine reads params once per block; the LFO value at block start is what a
            // block-rate route would see.
            let modulated = attack_at(sc.attack_ms, sc.amount, lfo_now);
            let attack = match policy {
                Policy::Continuous => modulated,
                Policy::StageStart => latched,
            };
            env.set_params(attack, sc.decay_ms, sc.sustain, sc.release_ms, SR);
            attack_used.push(attack);
        }
        if policy == Policy::StageStart && gate > 0.5 && !env.gate_was_high {
            // Note-on: sample the modulated Attack once for this stage.
            latched = attack_at(sc.attack_ms, sc.amount, lfo_now);
            env.set_params(latched, sc.decay_ms, sc.sustain, sc.release_ms, SR);
        }
        lfo_now = lfo.next(sc.lfo_hz, SR, sc.lfo_wave);
        let e = env.next(gate);
        let hz = note.map(|(_, _, m)| 440.0 * 2f32.powf((*m as f32 - 69.0) / 12.0)).unwrap_or(110.0);
        let s = svf.process_with_coeffs(osc.next(hz, SR, OscWaveform::Saw), &coeffs);
        out_env.push(e);
        out_audio.push(0.5 * s * e);
    }
    let rise_90 = sc
        .notes
        .iter()
        .map(|(s, l, _)| {
            let (a, b) = ((s * SR) as usize, (((s + l) * SR) as usize).min(n));
            out_env[a..b].iter().position(|&e| e >= 0.9).map(|k| k as f32 / SR)
        })
        .collect();
    Render { env: out_env, audio: out_audio, attack_used, rise_90 }
}

pub fn rms_diff(a: &[f32], b: &[f32]) -> f32 {
    let s: f32 = a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum();
    (s / a.len().max(1) as f32).sqrt()
}

/// 16-bit mono PCM WAV.
pub fn write_wav(path: &std::path::Path, samples: &[f32]) -> std::io::Result<()> {
    let mut b = Vec::with_capacity(44 + samples.len() * 2);
    let data_len = (samples.len() * 2) as u32;
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&(SR as u32).to_le_bytes());
    b.extend_from_slice(&(SR as u32 * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    std::fs::write(path, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_only_policy_differs() {
        let sc = Scenario::slow_pad();
        let a1 = render(&sc, Policy::Continuous);
        let a2 = render(&sc, Policy::Continuous);
        assert_eq!(a1.audio, a2.audio, "render must be deterministic");
        let b = render(&sc, Policy::StageStart);
        assert!(rms_diff(&a1.env, &b.env) > 1e-3, "policies should differ on the slow pad");
        // Zero amount: policies must be identical.
        let mut z = sc.clone();
        z.amount = 0.0;
        assert_eq!(render(&z, Policy::Continuous).env, render(&z, Policy::StageStart).env);
    }

    #[test]
    fn stage_start_holds_attack_within_a_note() {
        let sc = Scenario::slow_pad();
        let r = render(&sc, Policy::StageStart);
        // Inside the first note's attack (blocks 20..40 after note-on) the attack never changes.
        let first = (0.1 * SR) as usize / BLOCK + 2;
        let w = &r.attack_used[first..first + 20];
        assert!(w.iter().all(|a| (a - w[0]).abs() < 1e-3), "{w:?}");
    }
}
