//! `chorus` and `drive`: exact neutral settings, stereo behaviour, levels, DC, continuity,
//! measured aliasing and latency, state carry. `-- --nocapture` prints the numbers quoted in
//! docs/sound-palette-batch/README.md.

mod common;
use common::*;

use std::f32::consts::PI;

fn sine(f: f32, a: f32) -> impl Fn(usize) -> f32 {
    move |i| a * (2.0 * PI * f * i as f32 / SR).sin()
}

/// Correlation coefficient of two signals.
fn corr(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    dot / (rms(a) * rms(b) * a.len() as f32)
}

/// Stereo render: (left, right) for inputs (l(i), r(i)).
fn stereo(
    r: &mut Rig,
    len: usize,
    l: impl Fn(usize) -> f32,
    rr: impl Fn(usize) -> f32,
) -> (Vec<f32>, Vec<f32>) {
    let (mut a, mut b) = (Vec::new(), Vec::new());
    let mut t = 0;
    while a.len() < len {
        let xl: Vec<f32> = (t..t + BLOCK).map(&l).collect();
        let xr: Vec<f32> = (t..t + BLOCK).map(&rr).collect();
        let o = r.block(&[&xl, &xr]);
        a.extend_from_slice(&o[0]);
        b.extend_from_slice(&o[1]);
        t += BLOCK;
    }
    (a, b)
}

fn noise(seed: u32) -> impl Fn(usize) -> f32 {
    move |i| {
        let mut x = (i as u32).wrapping_mul(0x9E37_79B9) ^ seed;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        (x >> 8) as f32 / 8_388_608.0 - 1.0
    }
}

#[test]
fn chorus_mix_zero_is_the_dry_input_exactly() {
    let mut r = Rig::new("chorus", &[("mix", 0.0)], SR);
    let (l, rr) = stereo(&mut r, 9600, noise(1), noise(2));
    for i in 0..l.len() {
        assert_eq!(l[i], noise(1)(i));
        assert_eq!(rr[i], noise(2)(i));
    }
}

#[test]
fn chorus_keeps_the_dry_image_and_adds_width_only_where_asked() {
    // A source on the left only: the right output stays silent at every setting.
    let mut r = Rig::new("chorus", &[("mix", 100.0), ("depth", 100.0)], SR);
    let (l, rr) = stereo(&mut r, 48000, sine(440.0, 0.5), |_| 0.0);
    assert!(rms(&l) > 0.2);
    assert!(rr.iter().all(|&v| v == 0.0));
    // A mono source into both inputs: width 0 % gives identical sides, 100 % wide ones.
    for (width, want_same) in [(0.0, true), (100.0, false)] {
        let mut r = Rig::new("chorus", &[("mix", 50.0), ("width", width)], SR);
        let src = noise(9);
        let (l, rr) = stereo(&mut r, 96000, &src, &src);
        let c = corr(&l[4800..], &rr[4800..]);
        println!("chorus, mono noise in, width {width} %: L/R correlation {c:.3}");
        if want_same {
            assert_eq!(l, rr);
        } else {
            assert!(c < 0.8);
        }
    }
}

#[test]
fn chorus_adds_no_dc_and_its_wet_signal_is_flat() {
    let mut r = Rig::new("chorus", &[("mix", 100.0), ("depth", 100.0)], SR);
    let (l, rr) = stereo(&mut r, 48000, |_| 0.0, |_| 0.0);
    assert!(
        l.iter().chain(&rr).all(|&v| v == 0.0),
        "silence in, silence out"
    );
    for f in [30.0f32, 100.0, 200.0, 1000.0, 5000.0, 12000.0] {
        let mut r = Rig::new("chorus", &[("mix", 100.0), ("depth", 100.0)], SR);
        let (l, rr) = stereo(&mut r, 96000, sine(f, 0.5), sine(f, 0.5));
        let g = db((rms(&l[9600..]) / (0.5 / 2f32.sqrt())) as f64);
        let gr = db((rms(&rr[9600..]) / (0.5 / 2f32.sqrt())) as f64);
        let mean = l[9600..].iter().sum::<f32>() / (l.len() - 9600) as f32;
        println!("chorus wet, full depth, {f} Hz: L {g:+.2} dB, R {gr:+.2} dB, mean {mean:.6}");
        // Flat within the moving Hermite read's top-octave loss (−0.5 dB at 12 kHz).
        let tol = if f > 10000.0 { 0.6 } else { 0.1 };
        assert!(g.abs() < tol && gr.abs() < tol && mean.abs() < 1e-3);
    }
    // Broadband, the default 50 % blend: the comb averages out.
    let mut r = Rig::new("chorus", &[], SR);
    let src = noise(11);
    let (l, _) = stereo(&mut r, 96000, &src, &src);
    let dry: Vec<f32> = (9600..96000).map(&src).collect();
    let g = db((rms(&l[9600..]) / rms(&dry)) as f64);
    println!("chorus default (50 % mix), white noise: {g:+.2} dB against dry");
    assert!(g.abs() < 0.5);
    let mut r = Rig::new("chorus", &[], SR);
    let (l, _) = stereo(&mut r, 96000, sine(30.0, 0.5), sine(30.0, 0.5));
    let g = db((rms(&l[9600..]) / (0.5 / 2f32.sqrt())) as f64);
    println!("chorus default (50 % mix), 30 Hz sine: {g:+.2} dB against dry");
    assert!(g < 3.1);
}

#[test]
fn chorus_modulation_and_control_changes_are_continuous() {
    // A pure tone: the largest sample-to-sample step stays that of the (slightly pitch-
    // shifted) tone, even while rate, depth, width and mix jump every 50 ms.
    let mut r = Rig::new("chorus", &[("mix", 50.0)], SR);
    let x = sine(1000.0, 0.5);
    let mut out = Vec::new();
    let mut t = 0;
    for b in 0..1500 {
        if b % 37 == 0 {
            let k = (b / 37) % 4;
            r.set("rate_hz", [0.1, 8.0, 0.5, 3.0][k]);
            r.set("depth", [0.0, 100.0, 30.0, 70.0][k]);
            r.set("width", [0.0, 100.0, 50.0, 100.0][k]);
            r.set("mix", [0.0, 100.0, 50.0, 20.0][k]);
        }
        let xl: Vec<f32> = (t..t + BLOCK).map(&x).collect();
        out.extend_from_slice(&r.block(&[&xl, &xl])[0]);
        t += BLOCK;
    }
    let step = out
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0, f32::max);
    // A 1 kHz, 0.5 sine steps at most 2π·1000/48000·0.5 = 0.065; an equal-power blend of two
    // in-phase copies reaches √2 of that, and the vibrato raises the pitch a few percent.
    println!("chorus under control jumps: largest sample step {step:.4}");
    assert!(step < 0.065 * 1.4143 * 1.08);
}

#[test]
fn chorus_carries_its_lines_and_phase() {
    let set = [("mix", 70.0), ("rate_hz", 2.0)];
    let mut a = Rig::new("chorus", &set, SR);
    let mut reference = Rig::new("chorus", &set, SR);
    let src = noise(4);
    stereo(&mut a, 6400, &src, &src);
    stereo(&mut reference, 6400, &src, &src);
    let mut b = Rig::new("chorus", &set, SR);
    b.m.carry_from(a.m.as_ref());
    let xl: Vec<f32> = (6400..6464).map(&src).collect();
    assert_eq!(b.block(&[&xl, &xl]), reference.block(&[&xl, &xl]));
}

fn drive_rig(set: &[(&str, f32)]) -> Rig {
    Rig::new("drive", set, SR)
}

#[test]
fn drive_default_and_zero_drive_pass_the_input_exactly() {
    let src = noise(3);
    let mut r = drive_rig(&[]);
    let y = r.render(4800, 0, |_, i| src(i));
    assert!(y.iter().enumerate().all(|(i, &v)| v == src(i)));
    let mut r = drive_rig(&[("trim_db", -6.0206)]);
    let y = r.render(4800, 0, |_, i| src(i));
    assert!(y
        .iter()
        .enumerate()
        .all(|(i, &v)| (v - 0.5 * src(i)).abs() < 1e-6));
}

#[test]
fn drive_keeps_full_scale_and_adds_no_dc() {
    for db_ in [6.0, 18.0, 36.0] {
        let mut r = drive_rig(&[("drive_db", db_)]);
        let y = r.render(48000, 0, |_, i| sine(100.0, 1.0)(i));
        let mean = y.iter().sum::<f32>() / y.len() as f32;
        let quiet = drive_rig(&[("drive_db", db_)]).render(48000, 0, |_, i| sine(100.0, 0.01)(i));
        println!(
            "drive {db_} dB: full-scale sine peak {:.3}, mean {mean:.5}; −40 dBFS sine gains {:+.1} dB",
            peak(&y),
            db((rms(&quiet[4800..]) / (0.01 / 2f32.sqrt())) as f64)
        );
        assert!(peak(&y) <= 1.0 + 1e-4 && mean.abs() < 1e-3);
    }
}

/// tanh(g·x)/tanh(g) sample by sample, the naive reference.
fn naive(x: &[f32], drive: f32) -> Vec<f32> {
    let g = kabl_modules::builtins::drive_g(drive) as f32;
    x.iter().map(|&v| (g * v).tanh() / g.tanh()).collect()
}

#[test]
fn drive_aliases_less_than_the_naive_curve() {
    for (f0, drive) in [
        (440.0f32, 12.0),
        (1244.5, 24.0),
        (2489.0, 24.0),
        (2489.0, 36.0),
        (4978.0, 36.0),
    ] {
        let x: Vec<f32> = (0..16384).map(sine(f0, 0.7)).collect();
        let mut r = drive_rig(&[("drive_db", drive)]);
        let y = r.render(x.len(), 0, |_, i| x[i]);
        let (w, t) = alias_db(&y, f0, SR, 20000.0);
        let (nw, nt) = alias_db(&naive(&x, drive), f0, SR, 20000.0);
        println!("drive {drive} dB, 0.7 sine {f0} Hz: ADAA worst {w:.1} / total {t:.1} dB; naive {nw:.1} / {nt:.1} dB");
        // Where the naive curve aliases audibly at all, ADAA takes at least 6 dB off.
        if nt > -80.0 {
            assert!(t < nt - 6.0, "{f0} {drive}");
        }
    }
}

#[test]
fn drive_latency_is_half_a_sample() {
    // A quiet tone through a gently engaged curve: the phase lag, in samples.
    let f = 1000.0;
    let mut r = drive_rig(&[("drive_db", 3.0)]);
    let y = r.render(48000, 0, |_, i| sine(f, 0.01)(i));
    let (mut re, mut im) = (0.0f64, 0.0f64);
    for (i, &v) in y.iter().enumerate().skip(4800) {
        let ph = 2.0 * std::f64::consts::PI * f as f64 * i as f64 / SR as f64;
        re += v as f64 * ph.sin();
        im += v as f64 * ph.cos();
    }
    let lag = -im.atan2(re) / (2.0 * std::f64::consts::PI) * SR as f64 / f as f64;
    println!("drive latency: {lag:.3} samples");
    assert!((lag - 0.5).abs() < 0.02);
}

#[test]
fn drive_engages_and_changes_without_steps() {
    let x = sine(3000.0, 0.8);
    let mut r = drive_rig(&[]);
    let mut out = Vec::new();
    let mut t = 0;
    for b in 0..600 {
        r.set("drive_db", [0.0, 24.0, 6.0, 0.0][(b / 100) % 4]);
        let xl: Vec<f32> = (t..t + BLOCK).map(&x).collect();
        out.extend_from_slice(&r.block(&[&xl])[0]);
        t += BLOCK;
    }
    // No step larger than the steepest the tone gets at steady 24 dB (the curve squares it).
    let steps = |y: &[f32]| {
        y.windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0, f32::max)
    };
    let steady = drive_rig(&[("drive_db", 24.0)]).render(out.len(), 0, |_, i| x(i));
    let (step, limit) = (steps(&out), steps(&steady[4800..]));
    println!(
        "drive under 0↔24 dB changes: largest sample step {step:.3} (steady 24 dB: {limit:.3})"
    );
    assert!(step <= limit * 1.01);
}
