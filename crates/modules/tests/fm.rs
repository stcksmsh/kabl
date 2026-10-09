//! `osc.fm`: frequency ratios, the Bessel spectrum of phase modulation, feedback, and the
//! aliasing the 2x internal rate buys (measured against the plain-rate operator).

mod common;
use common::*;
use kabl_modules::builtins::{OscFm, FEEDBACK_MAX_RAD};

/// One block of a modulator->carrier pair. `fc` and `fm` are the operators' `base_hz` at pitch 0
/// (ratio 1), so the pair is exactly where the test says whatever the ratio params.
fn pair(fc: f32, fm: f32, index: f32, feedback: f32, oversample: bool) -> (Rig, Rig) {
    let mut m = Rig::new("osc.fm", &[("base_hz", fm)], SR);
    let mut c = Rig::new(
        "osc.fm",
        &[("base_hz", fc), ("index", index), ("feedback", feedback)],
        SR,
    );
    for r in [&mut m, &mut c] {
        r.m.as_any_mut()
            .downcast_mut::<OscFm>()
            .unwrap()
            .set_oversample(oversample);
    }
    (m, c)
}

fn run(m: &mut Rig, c: &mut Rig, len: usize) -> Vec<f32> {
    let mut out = Vec::new();
    while out.len() < len {
        let mo = m.block(&[])[0];
        out.extend_from_slice(&c.block(&[&[0.0; BLOCK], &mo])[0]);
    }
    out
}

fn fm(fc: f32, fm_hz: f32, index: f32, feedback: f32, oversample: bool) -> Vec<f32> {
    let (mut m, mut c) = pair(fc, fm_hz, index, feedback, oversample);
    run(&mut m, &mut c, 4096 + 16384)[4096..].to_vec()
}

/// Amplitude (linear, relative to a unit sine) of the spectral line nearest `hz`.
fn line(x: &[f32], hz: f32) -> f64 {
    let hz = hz.abs(); // a negative sideband folds onto its mirror
    let p = spectrum(x);
    let bin = (hz as f64 * x.len() as f64 / SR as f64).round() as usize;
    let peak = p[bin.saturating_sub(4)..=(bin + 4).min(p.len() - 1)]
        .iter()
        .sum::<f64>();
    let unit = {
        let s: Vec<f32> = (0..x.len())
            .map(|i| (2.0 * std::f32::consts::PI * hz * i as f32 / SR).sin())
            .collect();
        spectrum(&s)[bin.saturating_sub(4)..=(bin + 4).min(p.len() - 1)]
            .iter()
            .sum::<f64>()
    };
    (peak / unit).sqrt()
}

/// Bessel function of the first kind, integer order, by its power series.
fn bessel_j(n: i32, x: f64) -> f64 {
    let n = n.unsigned_abs() as i32;
    let mut term = (x / 2.0).powi(n) / (1..=n).map(|k| k as f64).product::<f64>();
    let mut sum = term;
    for k in 1..60 {
        term *= -(x / 2.0).powi(2) / (k as f64 * (k + n) as f64);
        sum += term;
    }
    sum
}

#[test]
fn unmodulated_operator_is_a_clean_sine_at_the_ratio_frequency() {
    for (ratio, fine, want) in [
        (1.0, 0.0, 440.0),
        (0.0, 0.0, 220.0), // 0 means one half
        (3.0, 0.0, 1320.0),
        (4.0, -231.17, 440.0 * 4.0 * 0.875), // x4 minus 231 ct is x3.5
    ] {
        let mut r = Rig::new(
            "osc.fm",
            &[("base_hz", 440.0), ("ratio", ratio), ("fine", fine)],
            SR,
        );
        let x = r.render(4096 + 16384, 0, |_, _| 0.0)[4096..].to_vec();
        let p = spectrum(&x);
        let peak = (0..p.len()).max_by(|&a, &b| p[a].total_cmp(&p[b])).unwrap();
        let hz = peak as f64 * SR as f64 / x.len() as f64;
        assert!(
            (hz - want as f64).abs() < 3.0,
            "ratio {ratio} fine {fine}: {hz} Hz, want {want}"
        );
        let (_, total) = alias_db(&x, hz as f32, SR, 20000.0);
        assert!(total < -80.0, "sine not clean: {total} dB");
    }
}

#[test]
fn zero_index_ignores_the_pm_input() {
    let (mut m, mut c) = pair(1000.0, 370.0, 0.0, 0.0, true);
    let with = run(&mut m, &mut c, 4096);
    let mut alone = Rig::new("osc.fm", &[("base_hz", 1000.0), ("index", 0.0)], SR);
    let without = alone.render(4096, 0, |_, _| 0.0);
    // The same sine, one filter latency apart in phase alignment is not allowed: bit-equal.
    assert_eq!(with, without);
}

#[test]
fn sidebands_follow_the_bessel_functions() {
    // Carrier 1000 Hz, modulator 370 Hz: every line c + k*m is distinct, so each is |J_k(I)|.
    for index in [1.0f32, 3.0, 5.0] {
        let x = fm(1000.0, 370.0, index, 0.0, true);
        for k in -3..=3 {
            let want = bessel_j(k, index as f64).abs();
            let got = line(&x, 1000.0 + 370.0 * k as f32);
            assert!(
                (got - want).abs() < 0.01 + 0.02 * want,
                "I={index} k={k}: {got:.4}, J = {want:.4}"
            );
        }
    }
}

#[test]
fn oversampling_removes_the_alias_the_plain_rate_makes() {
    // (carrier, modulator, index): highest sideband (I+1)*fm + fc stays under ~30 kHz, so the
    // doubled rate holds everything that matters.
    for (fc, fm_hz, index) in [
        (2113.7, 2113.7, 8.0),
        (1471.1, 2942.2, 5.0),
        (997.3, 997.3, 8.0),
    ] {
        let plain = fm(fc, fm_hz, index, 0.0, false);
        let over = fm(fc, fm_hz, index, 0.0, true);
        let (_, a) = alias_db(&plain, fc, SR, 20000.0);
        let (_, b) = alias_db(&over, fc, SR, 20000.0);
        assert!(b < -80.0, "fc={fc} m={fm_hz} I={index}: {b:.1} dB with 2x");
        assert!(b <= a + 1.0, "2x not better: {b:.1} vs {a:.1}");
        if fc > 2000.0 {
            assert!(a > -60.0, "plain rate should alias here: {a:.1} dB");
        }
    }
}

#[test]
fn extreme_index_is_bounded_not_clean() {
    // Index 10 at 3.3 kHz puts sidebands out to 40 kHz: 2x folds some back. Pin what we measure
    // (-51.7 dB total, plain rate -4.6 dB) so a regression shows, and the limit is written down.
    let x = fm(3331.3, 3331.3, 10.0, 0.0, true);
    let (_, total) = alias_db(&x, 3331.3, SR, 20000.0);
    assert!(total < -45.0, "{total:.1} dB");
}

#[test]
fn feedback_brightens_and_stays_bounded_and_periodic() {
    let dry = fm(440.3, 440.3, 0.0, 0.0, true);
    let wet = fm(440.3, 440.3, 0.0, 1.0, true);
    let h = |x: &[f32], n: usize| line(x, 440.3 * n as f32);
    assert!(
        h(&wet, 3) > 0.05,
        "no 3rd harmonic with feedback: {}",
        h(&wet, 3)
    );
    assert!(h(&dry, 3) < 1e-3);
    assert!(peak(&wet) < 1.1);
    let (_, total) = alias_db(&wet, 440.3, SR, 20000.0);
    assert!(
        total < -80.0,
        "feedback at 440 Hz: {total:.1} dB (not periodic or aliased)"
    );
    assert!(FEEDBACK_MAX_RAD <= 2.0);
}

#[test]
fn extreme_settings_never_produce_nan_or_runaway() {
    let mut m = Rig::new("osc.fm", &[("ratio", 32.0)], SR);
    let mut c = Rig::new(
        "osc.fm",
        &[
            ("index", 16.0),
            ("feedback", 1.0),
            ("ratio", 32.0),
            ("base_hz", 20000.0),
        ],
        SR,
    );
    let pitch = [60.0f32; BLOCK];
    for _ in 0..2000 {
        let mo = m.block(&[&pitch])[0];
        let co = c.block(&[&pitch, &mo])[0];
        assert!(co.iter().all(|v| v.is_finite() && v.abs() <= 1.5));
    }
    // A NaN on the inputs cannot poison the operator for good.
    let nan = [f32::NAN; BLOCK];
    c.block(&[&nan, &nan]);
    let after = c.block(&[&pitch, &[0.0; BLOCK]])[0];
    assert!(after.iter().all(|v| v.is_finite()));
}

#[test]
fn carry_from_continues_bit_exactly() {
    let params = [("index", 2.5), ("feedback", 0.4)];
    let mut a = Rig::new("osc.fm", &params, SR);
    let silence = [0.0f32; BLOCK];
    let drive: Vec<f32> = (0..BLOCK).map(|i| (i as f32 * 0.3).sin()).collect();
    for _ in 0..10 {
        a.block(&[&silence, &drive]);
    }
    let mut b = Rig::new("osc.fm", &params, SR);
    b.m.carry_from(a.m.as_ref());
    for _ in 0..5 {
        assert_eq!(a.block(&[&silence, &drive]), b.block(&[&silence, &drive]));
    }
}

#[test]
fn level_scales_the_output() {
    let mut full = Rig::new("osc.fm", &[], SR);
    let mut half = Rig::new("osc.fm", &[("level", 0.5)], SR);
    let a = full.render(2048, 0, |_, _| 0.0);
    let b = half.render(2048, 0, |_, _| 0.0);
    for (x, y) in a.iter().zip(&b) {
        assert!((x * 0.5 - y).abs() < 1e-6);
    }
}
