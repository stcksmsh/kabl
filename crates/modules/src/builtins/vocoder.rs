//! `vocoder`: imposes the spectral shape of one sound (the modulator, `mod`: a voice) on
//! another (the carrier, `in_l`/`in_r`: a synth), so a saw chord speaks.
//!
//! **Method.** A channel vocoder. `bands` (8 … 32) band-pass filters split the modulator;
//! an envelope follower per band tracks how loud each slice is. The same number of band-pass
//! filters split each carrier channel, and each carrier slice is multiplied by its modulator
//! envelope and summed. Every filter is two cascaded TPT state-variable band-passes
//! (Zavalishin, ch. 4), each normalised to unity at its centre and widened (`WIDEN`) so the
//! cascade's −3 dB width equals the band's slot: neighbours cross at about −3 dB. Bandwidth
//! follows the local band spacing. The carrier is filtered per channel, so a stereo carrier
//! keeps its width; the envelopes are shared.
//!
//! **Bands.** Centres run from `low_hz` to `high_hz`. `spacing` 0 is logarithmic (equal
//! musical steps, most bands where speech is); 1 is linear (bands evenly spread, more at the
//! top); between, the centres are interpolated in the log domain. The top is held under
//! 0.42·fs so every band exists at 44.1 … 192 kHz.
//!
//! **Formant shift.** `shift_st` moves the carrier's bands against the modulator's: band `i`
//! of the voice drives a carrier band `shift_st` semitones away. Positive makes the voice
//! smaller (chipmunk), negative larger.
//!
//! **Envelope.** Rectified band output, one-pole with `attack_ms` when rising and
//! `release_ms` when falling, scaled by `drive_db` (the modulator's gain into the followers:
//! a quiet voice needs more). With matched spectra and 0 dB drive the vocoder is close to
//! unity; a quieter modulator is quieter out. The followers are scaled by √(bands/16), so a noisy
//! voice is within about 5 dB at 8 bands and at 32 (each band hears less of it, and there are more). Each envelope is capped at 16.
//!
//! **Unvoiced.** A vocoder loses consonants: `s`, `t`, `f` are noise, and a saw has no noise.
//! `hf` mixes in white noise band-passed around 5.5 kHz (about 2.5 kHz wide, 4 to 7.5 kHz; the sibilant range), shaped by the
//! follower of the modulator above 5 kHz (fixed 3 ms attack, 35 ms release), but only while the
//! voice is mostly above 5 kHz: the noise is closed while the high follower is under 15 % of
//! high plus low and fully open from 40 %. Vowels, which have a little energy up there, and a
//! microphone's floor stay clean; `s`, `t`, `f` open it. `hf` 0 skips the path (nothing added,
//! exactly the clean vocoder); `hf` glides like the other controls. Left and right get separate
//! noise. The noise sounds only while the carrier does
//! (a follower of the carrier's level, full from −34 dBFS): a voice with no synth under it is
//! silent, not whispering.
//!
//! **Mix.** 0 is the carrier only, 1 the vocoded sound only. An unpatched `mod` is silence:
//! the vocoded part is silent too.
//!
//! **Safety.** Non-finite input is read as 0 and inputs are clamped to ±16. Filter states are
//! flushed against denormals. Nothing allocates after `new`. Band count, spacing, range and
//! shift glide (20 ms); the filters are redesigned every 16 samples while one moves, and not
//! at all while still. Parameters are read once per 16 samples.
//!
//! **View.** `Module::view` reports each band's envelope (square-rooted, 0 … 1) in
//! `cycle[..bands]` for the face's level display.

use super::fx::{
    blend, coef, flush, param, port, Glide, TimeCoef, NEPER_PER_DB, STEREO_IN, STEREO_OUT,
};
use crate::info::{
    Category, ModuleInfo, ParamInfo, PortInfo, PortType, QualitySupport, Rate, Taper,
};
use crate::io::ProcessIo;
use crate::module::{Module, QualityConfig};
use crate::view::ModuleView;

/// Most bands.
pub const MAX_BANDS: usize = 32;

const PORTS: &[PortInfo] = &[STEREO_IN, port("mod", PortType::Audio, true), STEREO_OUT];

const PARAMS: &[ParamInfo] = &[
    param("bands", 8.0, MAX_BANDS as f32, 16.0, "bands", Taper::Linear),
    param("spacing", 0.0, 1.0, 0.0, "", Taper::Linear),
    param("low_hz", 60.0, 400.0, 120.0, "Hz", Taper::Exponential),
    param("high_hz", 3000.0, 16000.0, 9000.0, "Hz", Taper::Exponential),
    param("shift_st", -12.0, 12.0, 0.0, "st", Taper::Linear),
    param("attack_ms", 0.5, 50.0, 4.0, "ms", Taper::Exponential),
    param("release_ms", 5.0, 500.0, 40.0, "ms", Taper::Exponential),
    param("hf", 0.0, 1.0, 0.35, "", Taper::Linear),
    param("drive_db", 0.0, 36.0, 18.0, "dB", Taper::Linear),
    param("mix", 0.0, 1.0, 1.0, "", Taper::Linear),
];

pub static VOCODER_INFO: ModuleInfo = ModuleInfo {
    kind: "vocoder",
    name: "Vocoder",
    category: Category::Effect,
    rate: Rate::Global,
    explain: "Makes a synth speak. The shape of the sound on mod (a voice) is stamped on the synth: a robot voice. Bands is the detail, shift moves the voice up or down, hf keeps the s and t sounds.",
    lesson: None,
    requires: &[],
    ports: PORTS,
    params: PARAMS,
    quality: QualitySupport {
        oversampling: false,
        anti_aliasing: false,
        interpolation: false,
    },
    skin: None,
    width_units: 10,
    advanced: &["spacing", "low_hz", "high_hz"],
};

const GLIDE_TAU_S: f32 = 0.02;
/// Samples per parameter read and filter redesign.
const SUB: usize = 16;
/// Each stage's width against the slot, so two cascaded stages measure the slot at −3 dB
/// (two identical band-passes are 0.644 as wide as one).
const WIDEN: f32 = 1.55;
/// Peak to mean-absolute: a rectified band-limited signal averages about 0.8 of its RMS.
const DETECT: f32 = 1.25;
const ENV_MAX: f32 = 16.0;
const INPUT_MAX: f32 = 16.0;
/// The unvoiced path: the voice is followed above `HF_HZ`; the noise it shapes is a band
/// around `HF_NOISE_HZ` (two cascaded band-passes of `k` = `HF_NOISE_K`, about 2.5 kHz wide), not
/// everything up to Nyquist, and sits at `NOISE_REF` RMS at full envelope, about the level of a
/// saw's upper harmonics, which are quiet.
const HF_HZ: f32 = 5000.0;
const HF_NOISE_HZ: f32 = 5500.0;
const HF_NOISE_K: f32 = 0.55;
const HF_NOISE_BW: f32 = 2500.0;
const NOISE_REF: f32 = 0.12;
const HF_ATTACK_MS: f32 = 3.0;
const HF_RELEASE_MS: f32 = 35.0;
/// The noise is closed while the voice's share above `HF_HZ` is under `UNVOICED_FROM` and fully
/// open from `UNVOICED_FULL`.
const UNVOICED_FROM: f32 = 0.15;
const UNVOICED_FULL: f32 = 0.4;
const CAR_ATTACK_MS: f32 = 5.0;
const CAR_RELEASE_MS: f32 = 80.0;
const CAR_GATE: f32 = 50.0;

#[derive(Debug, Clone, Copy)]
struct Band {
    /// TPT coefficients a1, a2, a3 and k of the analysis and of the carrier filter.
    an: [f32; 4],
    sy: [f32; 4],
    /// State of the two cascaded stages: modulator, then per carrier channel.
    m: [[f32; 2]; 2],
    c: [[[f32; 2]; 2]; 2],
    env: f32,
}

const BAND0: Band = Band {
    an: [1.0, 0.0, 0.0, 0.0],
    sy: [1.0, 0.0, 0.0, 0.0],
    m: [[0.0; 2]; 2],
    c: [[[0.0; 2]; 2]; 2],
    env: 0.0,
};

#[derive(Debug, Clone, Copy)]
struct Core {
    bands: [Band; MAX_BANDS],
    n: usize,
    spacing: Glide,
    low: Glide,
    high: Glide,
    shift: Glide,
    drive: Glide,
    mix: Glide,
    hf: Glide,
    attack: TimeCoef,
    release: TimeCoef,
    /// The settings the filters were last designed for.
    key: [f32; 5],
    /// Unvoiced path: high-pass state (two stages) for the modulator and each noise, the
    /// follower, and the two noise generators.
    hf_mod: [[f32; 2]; 2],
    hf_noise: [[[f32; 2]; 2]; 2],
    hf_env: f32,
    /// Follower of the voice below `HF_HZ`: the noise needs the voice to be mostly above it.
    lf_env: f32,
    /// Follower of the carrier's level: the unvoiced noise only sounds while the synth does.
    car_env: f32,
    rng: [u32; 2],
    started: bool,
}

pub struct Vocoder {
    c: Core,
    sample_rate: f32,
}

impl Vocoder {
    pub fn new() -> Self {
        Vocoder {
            c: Core {
                bands: [BAND0; MAX_BANDS],
                n: 0,
                spacing: Glide::default(),
                low: Glide::default(),
                high: Glide::default(),
                shift: Glide::default(),
                drive: Glide::default(),
                mix: Glide::default(),
                hf: Glide::default(),
                attack: TimeCoef::new(),
                release: TimeCoef::new(),
                key: [f32::NAN; 5],
                hf_mod: [[0.0; 2]; 2],
                hf_noise: [[[0.0; 2]; 2]; 2],
                hf_env: 0.0,
                lf_env: 0.0,
                car_env: 0.0,
                rng: [0x9E37_79B9, 0x85EB_CA6B],
                started: false,
            },
            sample_rate: 48000.0,
        }
    }
}

impl Default for Vocoder {
    fn default() -> Self {
        Self::new()
    }
}

/// The centre of each of `n` bands (`MAX_BANDS` long, 0 past `n`) at `sr`. The top is held at
/// 0.42·fs, the bottom under it.
pub fn band_centers(n: usize, low: f32, high: f32, spacing: f32, sr: f32) -> [f32; MAX_BANDS] {
    let n = n.clamp(2, MAX_BANDS);
    let top = high.min(0.42 * sr);
    let bottom = low.min(0.5 * top).max(20.0);
    let s = spacing.clamp(0.0, 1.0);
    let mut out = [0.0; MAX_BANDS];
    for (i, o) in out.iter_mut().enumerate().take(n) {
        let u = i as f32 / (n - 1) as f32;
        let log = bottom * (top / bottom).powf(u);
        let lin = bottom + (top - bottom) * u;
        *o = (log.ln() * (1.0 - s) + lin.ln() * s).exp();
    }
    out
}

/// TPT coefficients a1, a2, a3 and k of a band-pass at `f` with `k = 1/Q`.
fn tpt(f: f32, k: f32, sr: f32) -> [f32; 4] {
    let g = (std::f32::consts::PI * f.clamp(20.0, 0.45 * sr) / sr).tan();
    let a1 = 1.0 / (1.0 + g * (g + k));
    [a1, g * a1, g * g * a1, k]
}

/// Designs the analysis filters for `centers` and the carrier filters for `centers`
/// shifted by `shift_st`. Bands past `n` are cleared.
fn design(
    bands: &mut [Band; MAX_BANDS],
    n: usize,
    centers: &[f32; MAX_BANDS],
    shift_st: f32,
    sr: f32,
) {
    let ratio = (shift_st / 12.0).exp2();
    for i in 0..n {
        let lo = if i == 0 {
            centers[0] * centers[0] / centers[1]
        } else {
            centers[i - 1]
        };
        let hi = if i + 1 == n {
            centers[i] * centers[i] / centers[i - 1]
        } else {
            centers[i + 1]
        };
        let k = ((hi - lo) * 0.5 / centers[i] * WIDEN).clamp(0.05, 2.0);
        bands[i].an = tpt(centers[i], k, sr);
        bands[i].sy = tpt(centers[i] * ratio, k, sr);
    }
    for b in &mut bands[n..] {
        *b = BAND0;
    }
}

/// One TPT band-pass stage, unity gain at its centre.
#[inline(always)]
fn stage(ic: &mut [f32; 2], co: &[f32; 4], x: f32) -> f32 {
    let v3 = x - ic[1];
    let v1 = co[0] * ic[0] + co[1] * v3;
    let v2 = ic[1] + co[1] * ic[0] + co[2] * v3;
    ic[0] = flush(2.0 * v1 - ic[0]);
    ic[1] = flush(2.0 * v2 - ic[1]);
    co[3] * v1
}

/// Two cascaded stages sharing one coefficient set.
#[inline(always)]
fn cascade(
    ic: &mut [[f32; 2]; 2],
    co: &[f32; 4],
    x: f32,
    f: fn(&mut [f32; 2], &[f32; 4], f32) -> f32,
) -> f32 {
    let t = f(&mut ic[0], co, x);
    f(&mut ic[1], co, t)
}

/// A 2nd-order Butterworth high-pass stage at the fixed corner (coefficients from `hp_co`).
#[inline(always)]
fn hp_stage(ic: &mut [f32; 2], co: &[f32; 4], x: f32) -> f32 {
    let v3 = x - ic[1];
    let v1 = co[0] * ic[0] + co[1] * v3;
    let v2 = ic[1] + co[1] * ic[0] + co[2] * v3;
    ic[0] = flush(2.0 * v1 - ic[0]);
    ic[1] = flush(2.0 * v2 - ic[1]);
    x - co[3] * v1 - v2
}

#[inline(always)]
fn white(rng: &mut u32) -> f32 {
    *rng ^= *rng << 13;
    *rng ^= *rng >> 17;
    *rng ^= *rng << 5;
    (*rng as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// Per-sample retention of a one-pole of `ms`.
fn retention(ms: f32, sr: f32) -> f32 {
    (-1.0 / (ms * 0.001 * sr)).exp()
}

#[inline(always)]
fn clean(x: f32) -> f32 {
    if x.is_finite() {
        x.clamp(-INPUT_MAX, INPUT_MAX)
    } else {
        0.0
    }
}

impl Module for Vocoder {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn info(&self) -> &'static ModuleInfo {
        &VOCODER_INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, _quality: &QualityConfig) {
        self.sample_rate = sample_rate;
        self.c.key = [f32::NAN; 5];
    }

    #[inline]
    fn process(&mut self, io: &mut ProcessIo) {
        let n = io.block_len();
        let sr = self.sample_rate;
        let (in_l, in_r, modulator) = (io.input(0), io.input(1), io.input(2));
        let p = [
            io.param(0),
            io.param(1),
            io.param(2),
            io.param(3),
            io.param(4),
            io.param(5),
            io.param(6),
            io.param(7),
            io.param(8),
            io.param(9),
        ];
        let kg = coef(GLIDE_TAU_S, sr / SUB as f32);
        let hp = tpt(HF_HZ.min(0.4 * sr), std::f32::consts::SQRT_2, sr);
        let nbp = tpt(HF_NOISE_HZ.min(0.4 * sr), HF_NOISE_K, sr);
        // Unit RMS after the band-pass: white noise is 0.577 RMS and the band takes its share.
        let noise_gain = NOISE_REF * (0.5 * sr / HF_NOISE_BW).sqrt() / 0.577;
        let c = &mut self.c;
        let mut i0 = 0;
        while i0 < n {
            let len = SUB.min(n - i0);
            let nb = (p[0].at(i0).round() as usize).clamp(8, MAX_BANDS);
            let spacing = c.spacing.next(p[1], i0, kg).clamp(0.0, 1.0);
            let low = c.low.next(p[2], i0, kg).clamp(60.0, 400.0);
            let high = c.high.next(p[3], i0, kg).clamp(3000.0, 16000.0);
            let shift = c.shift.next(p[4], i0, kg).clamp(-12.0, 12.0);
            let key = [nb as f32, spacing, low, high, shift];
            if c.key != key {
                c.key = key;
                c.n = nb;
                let centers = band_centers(nb, low, high, spacing, sr);
                design(&mut c.bands, nb, &centers, shift, sr);
            }
            let att = c.attack.get(p[5].at(i0).clamp(0.5, 50.0), sr);
            let rel = c.release.get(p[6].at(i0).clamp(5.0, 500.0), sr);
            let hf = c.hf.next(p[7], i0, kg).clamp(0.0, 1.0);
            let drive = c.drive.next(p[8], i0, kg).clamp(0.0, 36.0);
            let det = DETECT * (drive * NEPER_PER_DB).exp() * (nb as f32 / 16.0).sqrt();
            let mix = c.mix.next(p[9], i0, kg).clamp(0.0, 1.0);

            let mut x = [[0.0f32; SUB]; 3];
            #[allow(clippy::needless_range_loop)]
            for j in 0..len {
                x[0][j] = clean(in_l.at(i0 + j));
                x[1][j] = clean(in_r.at(i0 + j));
                x[2][j] = clean(modulator.at(i0 + j));
            }
            let mut wet = [[0.0f32; SUB]; 2];
            for b in &mut c.bands[..c.n] {
                for j in 0..len {
                    let m = cascade(&mut b.m, &b.an, x[2][j], stage);
                    let a = m.abs() * det;
                    let k = if a > b.env { att } else { rel };
                    b.env = flush((a + (b.env - a) * k).min(ENV_MAX));
                    for (ch, w) in wet.iter_mut().enumerate() {
                        w[j] += cascade(&mut b.c[ch], &b.sy, x[ch][j], stage) * b.env;
                    }
                }
            }
            if hf > 0.0 {
                let (ka, kr) = (retention(HF_ATTACK_MS, sr), retention(HF_RELEASE_MS, sr));
                let (ca, cr) = (retention(CAR_ATTACK_MS, sr), retention(CAR_RELEASE_MS, sr));
                for j in 0..len {
                    let h = cascade(&mut c.hf_mod, &hp, x[2][j], hp_stage);
                    let a = h.abs() * det;
                    let k = if a > c.hf_env { ka } else { kr };
                    c.hf_env = flush((a + (c.hf_env - a) * k).min(ENV_MAX));
                    let a = (x[2][j] - h).abs() * det;
                    let k = if a > c.lf_env { ka } else { kr };
                    c.lf_env = flush((a + (c.lf_env - a) * k).min(ENV_MAX));
                    // Voiced speech has its energy below HF_HZ and a little above it; s, t, f
                    // have it above. Only the second kind opens the noise.
                    let share = c.hf_env / (c.hf_env + c.lf_env + 1e-6);
                    let unvoiced =
                        ((share - UNVOICED_FROM) / (UNVOICED_FULL - UNVOICED_FROM)).clamp(0.0, 1.0);
                    let level = x[0][j].abs().max(x[1][j].abs());
                    let k = if level > c.car_env { ca } else { cr };
                    c.car_env = flush(level + (c.car_env - level) * k);
                    // Full from -34 dBFS up: the noise follows the synth, not the voice alone.
                    let gate = (c.car_env * CAR_GATE).min(1.0);
                    for (ch, w) in wet.iter_mut().enumerate() {
                        let nz = cascade(&mut c.hf_noise[ch], &nbp, white(&mut c.rng[ch]), stage);
                        w[j] += nz * c.hf_env * unvoiced * hf * gate * noise_gain;
                    }
                }
            }
            for j in 0..len {
                io.output(0)[i0 + j] = blend(x[0][j], wet[0][j], mix);
                io.output(1)[i0 + j] = blend(x[1][j], wet[1][j], mix);
            }
            c.started = true;
            i0 += len;
        }
    }

    fn reset(&mut self) {
        self.c = Vocoder::new().c;
    }

    fn carry_from(&mut self, old: &dyn Module) {
        if let Some(o) = old.as_any().downcast_ref::<Vocoder>() {
            if o.sample_rate == self.sample_rate {
                self.c = o.c;
            }
        }
    }

    fn view(&self, out: &mut ModuleView) {
        out.valid = self.c.started;
        out.position = self.c.n as f32 / MAX_BANDS as f32;
        for (o, b) in out.cycle.iter_mut().zip(&self.c.bands[..self.c.n]) {
            *o = b.env.sqrt().min(1.0);
        }
    }
}
