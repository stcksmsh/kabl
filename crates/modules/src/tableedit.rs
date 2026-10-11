//! Editing operations on wavetable frames, for the table editor. Pure functions on the raw
//! frames of `wavetable::decode_canonical` (each `FRAME` samples); the editor writes the result
//! back with `wavetable::canonical_wav`. Control thread only: they allocate.

use crate::wavetable::{fft, FRAME, MAX_FRAMES};

/// Peak used by `normalise`, the same headroom `import_wav` leaves.
pub const PEAK: f32 = 0.98;
/// Harmonics the additive bars show and edit.
pub const BARS: usize = 64;

fn spectrum_of(f: &[f32]) -> (Vec<f64>, Vec<f64>) {
    let mut re: Vec<f64> = f.iter().map(|&v| v as f64).collect();
    let mut im = vec![0.0; FRAME];
    fft(&mut re, &mut im, false);
    (re, im)
}

/// Inverse transform of the half spectrum `bins` (harmonic `h` in `bins[h]`); DC is dropped and
/// everything above 512 is ignored, as the oscillator's tables are.
fn frame_from(bins: &[(f64, f64)]) -> Vec<f32> {
    let mut re = vec![0.0; FRAME];
    let mut im = vec![0.0; FRAME];
    for (h, &(a, b)) in bins.iter().enumerate().take(FRAME / 4 + 1).skip(1) {
        re[h] = a;
        im[h] = b;
        re[FRAME - h] = a;
        im[FRAME - h] = -b;
    }
    fft(&mut re, &mut im, true);
    re.iter().map(|&v| v as f32).collect()
}

pub fn peak(f: &[f32]) -> f32 {
    f.iter().fold(0.0, |m, v| m.max(v.abs()))
}

/// Scales the frame so its peak is `PEAK`; a silent frame stays silent.
pub fn normalise(f: &mut [f32]) {
    let p = peak(f);
    if p > 1e-9 {
        f.iter_mut().for_each(|v| *v *= PEAK / p);
    }
}

pub fn remove_dc(f: &mut [f32]) {
    let mean = f.iter().sum::<f32>() / f.len() as f32;
    f.iter_mut().for_each(|v| *v -= mean);
}

/// Fades the first and last `len` samples to zero with a raised-cosine ramp, so the cycle starts
/// and ends at silence.
pub fn fade_edges(f: &mut [f32], len: usize) {
    let n = f.len();
    let len = len.min(n / 2);
    for i in 0..len {
        let g = 0.5 - 0.5 * (std::f32::consts::PI * i as f32 / len as f32).cos();
        f[i] *= g;
        f[n - 1 - i] *= g;
    }
}

/// Amplitudes of harmonics `1..=BARS` (0 to 1 is a full-scale sine; louder is possible).
pub fn harmonics(f: &[f32]) -> Vec<f32> {
    let (re, im) = spectrum_of(f);
    (1..=BARS)
        .map(|h| (re[h].hypot(im[h]) * 2.0 / FRAME as f64) as f32)
        .collect()
}

/// Sets harmonic `h` (1..=512) to amplitude `amp`, keeping its phase (a new harmonic is a sine).
pub fn set_harmonic(f: &mut [f32], h: usize, amp: f32) {
    if !(1..=FRAME / 4).contains(&h) {
        return;
    }
    let (mut re, mut im) = spectrum_of(f);
    let scale = amp.max(0.0) as f64 * FRAME as f64 / 2.0;
    let mag = re[h].hypot(im[h]);
    let (c, s) = if mag > 1e-9 {
        (re[h] / mag, im[h] / mag)
    } else {
        (0.0, -1.0)
    };
    (re[h], im[h]) = (c * scale, s * scale);
    let bins: Vec<(f64, f64)> = (0..=FRAME / 4).map(|k| (re[k], im[k])).collect();
    f.copy_from_slice(&frame_from(&bins));
}

/// The frame a fraction `t` (0 to 1) of the way from `a` to `b`, waveform crossfade.
pub fn interpolate(a: &[f32], b: &[f32], t: f32) -> Vec<f32> {
    a.iter().zip(b).map(|(x, y)| x + (y - x) * t).collect()
}

/// The frame a fraction `t` of the way from `a` to `b`, harmonic by harmonic: amplitude and
/// phase each move (phase by the shorter way round), so a harmonic that is in both never dips
/// the way it does in a waveform crossfade.
pub fn morph(a: &[f32], b: &[f32], t: f32) -> Vec<f32> {
    let (ar, ai) = spectrum_of(a);
    let (br, bi) = spectrum_of(b);
    let t = t as f64;
    let tau = std::f64::consts::TAU;
    let bins: Vec<(f64, f64)> = (0..=FRAME / 4)
        .map(|h| {
            let (ma, mb) = (ar[h].hypot(ai[h]), br[h].hypot(bi[h]));
            let (pa, pb) = (ai[h].atan2(ar[h]), bi[h].atan2(br[h]));
            // A harmonic missing from one end takes the other end's phase.
            let (pa, pb) = match (ma > 1e-9, mb > 1e-9) {
                (false, true) => (pb, pb),
                (true, false) => (pa, pa),
                _ => (pa, pb),
            };
            let d = (pb - pa + std::f64::consts::PI).rem_euclid(tau) - std::f64::consts::PI;
            let (m, p) = (ma + (mb - ma) * t, pa + d * t);
            (m * p.cos(), m * p.sin())
        })
        .collect();
    frame_from(&bins)
}

/// Replaces the frames strictly between `i` and `j` (`i < j`) with steps from frame `i` to
/// frame `j`: `spectral` for `morph`, else `interpolate`. Does nothing when they are adjacent.
pub fn fill_between(frames: &mut [Vec<f32>], i: usize, j: usize, spectral: bool) {
    if i >= j || j >= frames.len() {
        return;
    }
    for k in i + 1..j {
        let t = (k - i) as f32 / (j - i) as f32;
        frames[k] = if spectral {
            morph(&frames[i], &frames[j], t)
        } else {
            interpolate(&frames[i], &frames[j], t)
        };
    }
}

/// Scales the whole table down by one factor when any sample passes full scale, so an edit never
/// clips when written as 16-bit; relative levels between frames stay. Returns whether it scaled.
pub fn fit(frames: &mut [Vec<f32>]) -> bool {
    let p = frames.iter().map(|f| peak(f)).fold(0.0, f32::max);
    if p <= 1.0 {
        return false;
    }
    frames
        .iter_mut()
        .flatten()
        .for_each(|v| *v *= PEAK / p);
    true
}

/// A fresh frame: one cycle of a sine.
pub fn sine() -> Vec<f32> {
    (0..FRAME)
        .map(|i| (std::f32::consts::TAU * i as f32 / FRAME as f32).sin() * PEAK)
        .collect()
}

/// `true` when another frame fits in a table.
pub fn has_room(frames: &[Vec<f32>]) -> bool {
    frames.len() < MAX_FRAMES
}
