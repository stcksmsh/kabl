//! `filter.ladder`: small-signal response, resonance and its gain compensation, self-
//! oscillation, stability under extreme modulation at 44.1/48/96 kHz, drive, state carry.
//! `-- --nocapture` prints the numbers quoted in docs/sound-palette-batch/README.md.

mod common;
use common::*;

fn ladder(set: &[(&str, f32)], sr: f32) -> Rig {
    Rig::new("filter.ladder", set, sr)
}

/// Gain in dB of a small (−40 dBFS) sine at `f` through the filter, after it settles.
fn gain_db(set: &[(&str, f32)], f: f32, sr: f32) -> f64 {
    let mut r = ladder(set, sr);
    let a = 0.01;
    let len = (sr * 0.5) as usize;
    let y = r.render(len, 0, |k, i| {
        if k == 0 {
            a * (2.0 * std::f32::consts::PI * f * i as f32 / sr).sin()
        } else {
            0.0
        }
    });
    // Only the component at `f` (a DFT bin over the settled half), not distortion or noise.
    let tail = &y[len / 2..];
    let (mut re, mut im) = (0.0f64, 0.0f64);
    for (i, &v) in tail.iter().enumerate() {
        let ph = 2.0 * std::f64::consts::PI * f as f64 * (len / 2 + i) as f64 / sr as f64;
        re += v as f64 * ph.cos();
        im += v as f64 * ph.sin();
    }
    let amp = 2.0 * (re * re + im * im).sqrt() / tail.len() as f64;
    db(amp / a as f64)
}

#[test]
fn response_is_24_db_per_octave_with_minus_12_at_cutoff() {
    for sr in [44100.0, 48000.0, 96000.0] {
        let set = [("cutoff_hz", 1000.0), ("resonance", 0.0)];
        let pass = gain_db(&set, 50.0, sr);
        let at = gain_db(&set, 1000.0, sr);
        let (o2, o4) = (gain_db(&set, 4000.0, sr), gain_db(&set, 8000.0, sr));
        println!("{sr} Hz, res 0: 50 Hz {pass:+.2} dB, fc {at:+.2} dB, 4k {o2:+.1}, 8k {o4:+.1}");
        assert!(pass.abs() < 0.2);
        assert!((at + 12.0).abs() < 0.5);
        // The prewarped bilinear 4-pole: the analog response at tan(πf/fs)/tan(πfc/fs).
        for (f, got) in [(4000.0f32, o2), (8000.0, o4)] {
            let r = ((std::f32::consts::PI * f / sr).tan()
                / (std::f32::consts::PI * 1000.0 / sr).tan()) as f64;
            let want = -40.0 * (1.0 + r * r).log10();
            assert!((got - want).abs() < 0.5, "{f} Hz: {got} vs {want}");
        }
    }
}

#[test]
fn resonance_peaks_at_cutoff_and_compensation_holds_the_pass_band() {
    let mut last_peak = f64::MIN;
    for res in [0.0f32, 0.25, 0.5, 0.75, 0.9] {
        let set = [("cutoff_hz", 1000.0), ("resonance", res)];
        let pass = gain_db(&set, 50.0, 48000.0);
        let peak = gain_db(&set, 1000.0, 48000.0) - pass;
        let k = 4.4 * res as f64;
        let want = db((1.0 + k / 2.0) / (1.0 + k));
        println!("res {res}: pass band {pass:+.2} dB (formula {want:+.2}), cutoff vs pass band {peak:+.1} dB");
        assert!((pass - want).abs() < 0.3);
        assert!(peak > last_peak);
        last_peak = peak;
    }
    assert!(last_peak > 10.0);
}

#[test]
fn full_resonance_self_oscillates_near_cutoff_at_a_bounded_level() {
    for (sr, fc) in [(44100.0, 440.0), (48000.0, 1000.0), (96000.0, 3000.0)] {
        let mut r = ladder(&[("cutoff_hz", fc), ("resonance", 1.0)], sr);
        let y = r.render(
            sr as usize * 2,
            0,
            |k, i| if k == 0 && i == 0 { 0.1 } else { 0.0 },
        );
        let tail = &y[sr as usize..];
        let mut crossings = 0;
        for w in tail.windows(2) {
            if w[0] < 0.0 && w[1] >= 0.0 {
                crossings += 1;
            }
        }
        let f = crossings as f32 * sr / tail.len() as f32;
        println!(
            "{sr} Hz: self-oscillation at {f:.0} Hz for fc {fc} Hz, peak {:.2}",
            peak(tail)
        );
        assert!(peak(tail) > 0.1 && peak(tail) < 1.5);
        assert!((f / fc - 1.0).abs() < 0.08, "{f} vs {fc}");
    }
}

#[test]
fn stays_bounded_under_extreme_input_and_modulation() {
    for sr in [44100.0, 48000.0, 96000.0] {
        let mut r = ladder(&[("resonance", 1.0), ("drive_db", 24.0)], sr);
        let mut rng = 1u32;
        let mut worst = 0.0f32;
        for b in 0..4000 {
            // The knob jumps across its whole range every few blocks; the CV moves per sample.
            r.set("cutoff_hz", if (b / 3) % 2 == 0 { 20.0 } else { 20000.0 });
            r.set("resonance", ((b % 7) as f32 / 6.0).min(1.0));
            let mut x = [0f32; BLOCK];
            let mut cv = [0f32; BLOCK];
            for i in 0..BLOCK {
                rng ^= rng << 13;
                rng ^= rng >> 17;
                rng ^= rng << 5;
                x[i] = (rng >> 8) as f32 / 8_388_608.0 - 1.0;
                cv[i] = 4.0 * ((b * BLOCK + i) as f32 * 0.01).sin();
            }
            let y = r.block(&[&x, &cv])[0];
            assert!(y.iter().all(|v| v.is_finite()));
            worst = worst.max(peak(&y));
        }
        println!("{sr} Hz: worst |y| {worst:.2} under full-scale noise, res jumps, cutoff 20 Hz↔20 kHz, CV ±4 oct");
        // Adversarial: every control jumping at once. Bounded (never runs away), not ±1.
        assert!(worst < 4.0);
    }
}

#[test]
fn musical_modulation_keeps_the_level_near_full_scale() {
    // A full-scale saw with a fast LFO sweeping the cutoff over 6 octaves at full resonance.
    for sr in [44100.0, 48000.0, 96000.0] {
        let mut r = ladder(&[("cutoff_hz", 400.0), ("resonance", 1.0)], sr);
        let mut worst = 0.0f32;
        for b in 0..(sr as usize * 4 / BLOCK) {
            let t = (b * BLOCK) as f32 / sr;
            r.set(
                "cutoff_hz",
                400.0 * 2f32.powf(3.0 + 3.0 * (t * 2.0 * std::f32::consts::PI * 3.0).sin()),
            );
            let x: Vec<f32> = (0..BLOCK)
                .map(|i| ((b * BLOCK + i) as f32 * 110.0 / sr).fract() * 2.0 - 1.0)
                .collect();
            worst = worst.max(peak(&r.block(&[&x])[0]));
        }
        println!("{sr} Hz: saw through a 3 Hz, 6-octave sweep at full resonance: peak {worst:.2}");
        assert!(worst < 1.5);
    }
}

#[test]
fn drive_adds_harmonics_without_running_away() {
    let tone = |k: usize, i: usize| {
        if k == 0 {
            0.5 * (2.0 * std::f32::consts::PI * 220.0 * i as f32 / SR).sin()
        } else {
            0.0
        }
    };
    let mut clean = ladder(&[("cutoff_hz", 20000.0), ("resonance", 0.0)], SR);
    let mut driven = ladder(
        &[
            ("cutoff_hz", 20000.0),
            ("resonance", 0.0),
            ("drive_db", 18.0),
        ],
        SR,
    );
    let (a, b) = (clean.render(16384, 0, tone), driven.render(16384, 0, tone));
    let h3 = |x: &[f32]| {
        let p = spectrum(x);
        let bin = |f: f32| (f * x.len() as f32 / SR).round() as usize;
        10.0 * (p[bin(660.0) - 3..=bin(660.0) + 3].iter().sum::<f64>()
            / p[bin(220.0) - 3..=bin(220.0) + 3].iter().sum::<f64>())
        .log10()
    };
    println!(
        "0.5 sine at 220 Hz: 3rd harmonic {:.1} dB (0 dB drive), {:.1} dB (18 dB drive); peaks {:.2} / {:.2}",
        h3(&a),
        h3(&b),
        peak(&a),
        peak(&b)
    );
    assert!(h3(&b) > h3(&a) + 20.0);
    assert!(peak(&b) < 1.2);
}

#[test]
fn state_carries_across_a_swap() {
    let set = [("cutoff_hz", 700.0), ("resonance", 0.8)];
    let mut a = ladder(&set, SR);
    let mut reference = ladder(&set, SR);
    let saw = |k: usize, i: usize| {
        if k == 0 {
            ((i % 200) as f32 / 100.0) - 1.0
        } else {
            0.0
        }
    };
    a.render(640, 0, saw);
    reference.render(640, 0, saw);
    let mut state = std::collections::HashMap::new();
    struct W<'a>(&'a mut std::collections::HashMap<String, f32>);
    impl kabl_modules::StateWriter for W<'_> {
        fn write_f32(&mut self, k: &str, v: f32) {
            self.0.insert(k.into(), v);
        }
    }
    struct R<'a>(&'a std::collections::HashMap<String, f32>);
    impl kabl_modules::StateReader for R<'_> {
        fn read_f32(&self, k: &str) -> Option<f32> {
            self.0.get(k).copied()
        }
    }
    a.m.save_state(&mut W(&mut state));
    let mut b = ladder(&set, SR);
    b.m.load_state(&R(&state));
    let x: Vec<f32> = (640..704).map(|i| saw(0, i)).collect();
    assert_eq!(b.block(&[&x])[0], reference.block(&[&x])[0]);
}
