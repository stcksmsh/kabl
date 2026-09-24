//! Shared measurement helpers for the sound palette tests: run a module block by block, and a
//! small radix-2 FFT for spectra (no new dependency for one test file).
#![allow(dead_code)]

use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{registry, Module, ProcessIo, Signal};

pub const SR: f32 = 48000.0;
pub const BLOCK: usize = 64;

pub fn quality() -> QualityConfig {
    QualityConfig {
        tier: QualityTier::Live,
    }
}

/// A module of `kind` with every param at its default except `set`, prepared at `sr`.
pub struct Rig {
    pub m: Box<dyn Module>,
    pub params: Vec<f32>,
    pub n_in: usize,
    pub n_out: usize,
}

impl Rig {
    pub fn new(kind: &str, set: &[(&str, f32)], sr: f32) -> Rig {
        let mut m = registry::create(kind).unwrap();
        m.prepare(sr, BLOCK, &quality());
        let info = m.info();
        let mut params: Vec<f32> = info.params.iter().map(|p| p.default).collect();
        for &(name, v) in set {
            let i = info.params.iter().position(|p| p.name == name).unwrap();
            params[i] = v;
        }
        let n_in = info
            .ports
            .iter()
            .filter(|p| p.direction == kabl_modules::PortDirection::Input)
            .count();
        let n_out = info.ports.len() - n_in;
        Rig {
            m,
            params,
            n_in,
            n_out,
        }
    }

    pub fn set(&mut self, name: &str, v: f32) {
        let i = self
            .m
            .info()
            .params
            .iter()
            .position(|p| p.name == name)
            .unwrap();
        self.params[i] = v;
    }

    /// One block: `inputs[k]` per input port (missing = silence). Returns every output.
    pub fn block(&mut self, inputs: &[&[f32]]) -> Vec<[f32; BLOCK]> {
        let silence = [0f32; BLOCK];
        let ins: Vec<Signal> = (0..self.n_in)
            .map(|k| Signal::Buffer(inputs.get(k).copied().unwrap_or(&silence)))
            .collect();
        let ps: Vec<Signal> = self.params.iter().map(|&v| Signal::Scalar(v)).collect();
        let mut outs = vec![[0f32; BLOCK]; self.n_out];
        {
            let mut refs: Vec<&mut [f32]> = outs.iter_mut().map(|o| &mut o[..]).collect();
            let mut io = ProcessIo::new(&ins, &mut refs, &ps, BLOCK);
            self.m.process(&mut io);
        }
        outs
    }

    /// `len` samples of output `out`, inputs from `input(k, sample index)`.
    pub fn render(
        &mut self,
        len: usize,
        out: usize,
        mut input: impl FnMut(usize, usize) -> f32,
    ) -> Vec<f32> {
        let mut v = Vec::with_capacity(len);
        let mut bufs = vec![[0f32; BLOCK]; self.n_in];
        while v.len() < len {
            let t0 = v.len();
            for (k, b) in bufs.iter_mut().enumerate() {
                for (i, x) in b.iter_mut().enumerate() {
                    *x = input(k, t0 + i);
                }
            }
            let refs: Vec<&[f32]> = bufs.iter().map(|b| &b[..]).collect();
            v.extend_from_slice(&self.block(&refs)[out]);
        }
        v.truncate(len);
        v
    }
}

/// In-place iterative radix-2 FFT of `(re, im)`; `re.len()` must be a power of two.
pub fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f64::consts::PI / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (s, c) = (ang * k as f64).sin_cos();
                let (a, b) = (start + k, start + k + len / 2);
                let (tr, ti) = (re[b] * c - im[b] * s, re[b] * s + im[b] * c);
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}

/// Power spectrum (linear power per bin, 0..=n/2) of `x` under a 4-term Blackman-Harris window.
pub fn spectrum(x: &[f32]) -> Vec<f64> {
    let n = x.len();
    let w = |i: usize| {
        let t = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
        0.35875 - 0.48829 * t.cos() + 0.14128 * (2.0 * t).cos() - 0.01168 * (3.0 * t).cos()
    };
    let mut re: Vec<f64> = x
        .iter()
        .enumerate()
        .map(|(i, &v)| v as f64 * w(i))
        .collect();
    let mut im = vec![0.0; n];
    fft(&mut re, &mut im);
    (0..=n / 2).map(|k| re[k] * re[k] + im[k] * im[k]).collect()
}

/// For a periodic signal of fundamental `f0`: (worst non-harmonic component below `band_hz`,
/// total non-harmonic power below `band_hz`), both in dB relative to the signal's harmonic
/// power (all harmonics, so a narrow pulse's weak fundamental doesn't flatter or penalize it). Bins
/// within ±`guard` of a harmonic (or DC) count as harmonic.
pub fn alias_db(x: &[f32], f0: f32, sr: f32, band_hz: f32) -> (f64, f64) {
    let (worst, total, fund) = alias_parts(x, f0, sr, band_hz);
    (
        10.0 * (worst / fund).max(1e-30).log10(),
        10.0 * (total / fund).max(1e-30).log10(),
    )
}

/// Total non-harmonic power below `band_hz`, in dB relative to a full-scale sine's power.
pub fn alias_abs_db(x: &[f32], f0: f32, sr: f32, band_hz: f32) -> f64 {
    let (_, total, _) = alias_parts(x, f0, sr, band_hz);
    let mut sine: Vec<f32> = (0..x.len())
        .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr).sin())
        .collect();
    let full: f64 = spectrum(&sine).iter().sum();
    sine.clear();
    10.0 * (total / full).max(1e-30).log10()
}

/// (worst non-harmonic lobe, total non-harmonic, harmonic power), linear.
fn alias_parts(x: &[f32], f0: f32, sr: f32, band_hz: f32) -> (f64, f64, f64) {
    let p = spectrum(x);
    let n = x.len();
    let hz = |k: usize| k as f64 * sr as f64 / n as f64;
    let guard = 8usize;
    let bin_of = |f: f64| (f * n as f64 / sr as f64).round() as usize;
    let mut harmonic = vec![false; p.len()];
    let mut h = 0.0;
    while h < sr as f64 / 2.0 + f0 as f64 {
        let b = bin_of(h);
        let hi = (b + guard).min(p.len() - 1);
        if b.saturating_sub(guard) <= hi {
            harmonic[b.saturating_sub(guard)..=hi].fill(true);
        }
        h += f0 as f64;
    }
    let fund: f64 = (0..p.len())
        .filter(|&k| harmonic[k] && k > guard)
        .map(|k| p[k])
        .sum();
    let mut total = 0.0f64;
    let mut worst_bin = None::<usize>;
    for k in 0..p.len() {
        if !harmonic[k] && hz(k) < band_hz as f64 {
            total += p[k];
            if worst_bin.is_none_or(|w| p[k] > p[w]) {
                worst_bin = Some(k);
            }
        }
    }
    // A sinusoid spreads over the window's main lobe (±4 bins): the worst component is its
    // lobe's non-harmonic power, like the fundamental's.
    let worst: f64 = worst_bin.map_or(0.0, |w| {
        (w.saturating_sub(4)..=(w + 4).min(p.len() - 1))
            .filter(|&k| !harmonic[k])
            .map(|k| p[k])
            .sum()
    });
    (worst, total, fund)
}

pub fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
}

pub fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |m, v| m.max(v.abs()))
}

pub fn db(x: f64) -> f64 {
    20.0 * x.max(1e-30).log10()
}
