//! `osc.va` sound palette checks: tuning, pulse width, unison level and continuity, state
//! carry, and measured aliasing (saw, pulse extremes, unison, hard sync) against naive
//! references. `cargo test -p kabl-modules --test osc_palette -- --nocapture` prints the table
//! quoted in docs/sound-palette-batch/README.md.

mod common;
use common::*;

use kabl_modules::dsp::{FullOsc, OscWaveform};

const SQR: f32 = 3.0;
const SINE: f32 = 0.0;
const N: usize = 16384;

fn osc(set: &[(&str, f32)]) -> Rig {
    Rig::new("osc.va", set, SR)
}

/// Rising zero crossings, linearly interpolated: mean frequency in Hz.
fn frequency(x: &[f32], sr: f32) -> f32 {
    let mut first = None;
    let mut last = 0.0;
    let mut count = 0;
    for i in 1..x.len() {
        if x[i - 1] < 0.0 && x[i] >= 0.0 {
            let t = (i - 1) as f32 + x[i - 1] / (x[i - 1] - x[i]);
            if first.is_none() {
                first = Some(t);
            } else {
                count += 1;
            }
            last = t;
        }
    }
    count as f32 * sr / (last - first.unwrap())
}

#[test]
fn fine_tuning_is_in_cents() {
    for cents in [-100.0f32, -7.0, 0.0, 33.0, 100.0] {
        let mut r = osc(&[("base_hz", 440.0), ("waveform", SINE), ("fine", cents)]);
        let x = r.render(48000, 0, |_, _| 0.0);
        let f = frequency(&x, SR);
        let want = 440.0 * 2f32.powf(cents / 1200.0);
        let err_cents = 1200.0 * (f / want).log2();
        assert!(err_cents.abs() < 0.05, "{cents} ct: {f} Hz vs {want} Hz");
    }
}

#[test]
fn pulse_width_sets_the_duty_cycle_without_dc() {
    for pw in [5.0f32, 20.0, 50.0, 80.0, 95.0] {
        let mut r = osc(&[("base_hz", 100.0), ("waveform", SQR), ("pw", pw)]);
        let x = r.render(48000, 0, |_, _| 0.0);
        // Levels 2 - 2·pw and -2·pw (DC removed): count samples above the midpoint.
        let mid = 1.0 - 2.0 * pw / 100.0;
        let high = x.iter().filter(|&&v| v > mid).count() as f32 / x.len() as f32;
        let mean = x.iter().sum::<f32>() / x.len() as f32;
        assert!((high - pw / 100.0).abs() < 0.002, "pw {pw}: duty {high}");
        assert!(mean.abs() < 1e-3, "pw {pw}: DC {mean}");
        // Peak: 2 - 2·pw at a narrow width (the DC-free pulse), never more than 1.9.
        assert!(peak(&x) <= 1.9 + 0.05);
    }
}

#[test]
fn unison_keeps_its_level_across_counts() {
    let mut levels = Vec::new();
    for count in 1..=4 {
        let mut r = osc(&[
            ("base_hz", 220.0),
            ("unison", count as f32),
            ("detune", 20.0),
        ]);
        let x = r.render(96000, 0, |_, _| 0.0);
        levels.push(db(rms(&x[4800..]) as f64));
    }
    println!("unison RMS dBFS by count 1..4 (saw 220 Hz, 20 ct): {levels:.2?}");
    for l in &levels {
        assert!((l - levels[0]).abs() < 1.5, "{levels:?}");
    }
}

#[test]
fn unison_count_changes_fade_and_keep_the_first_oscillator() {
    // 1 → 4 → 2 → 1 mid-note. At each change the output departs gradually from what the old
    // count would have played (a 20 ms fade, no step). Oscillator 0 is never restarted (it
    // moves to its place in the spread, so its phase drifts from a lone oscillator's); back at
    // one oscillator it is in tune.
    let set = [("base_hz", 220.0), ("detune", 20.0)];
    let mut r = osc(&set);
    let mut count = 1.0;
    for next in [4.0, 2.0, 1.0] {
        for _ in 0..200 {
            r.block(&[]);
        }
        // What the old count would play next, from the same state.
        let mut stay = osc(&set);
        stay.set("unison", count);
        stay.m.carry_from(r.m.as_ref());
        r.set("unison", next);
        let (a, b) = (r.block(&[])[0], stay.block(&[])[0]);
        let diff: Vec<f32> = a.iter().zip(&b).map(|(x, y)| x - y).collect();
        let rel = db((rms(&diff[..48]) / rms(&b)) as f64);
        println!("unison {count} → {next}: first ms departs by {rel:.1} dB");
        assert!(rel < -20.0);
        count = next;
    }
    let x = r.render(48000, 0, |_, _| 0.0);
    let f = frequency(&x[4800..], SR);
    assert!((1200.0 * (f / 220.0).log2()).abs() < 0.05, "{f}");
}

#[test]
fn carry_from_continues_every_unison_oscillator() {
    let set = [
        ("base_hz", 330.0),
        ("unison", 4.0),
        ("detune", 25.0),
        ("pw", 30.0),
        ("waveform", SQR),
    ];
    let mut a = osc(&set);
    let mut reference = osc(&set);
    for _ in 0..37 {
        a.block(&[]);
        reference.block(&[]);
    }
    let mut b = osc(&set);
    b.m.carry_from(a.m.as_ref());
    for _ in 0..10 {
        assert_eq!(b.block(&[])[0], reference.block(&[])[0]);
    }
}

fn naive_pulse(f0: f32, pw: f32, len: usize) -> Vec<f32> {
    let mut p = 0.0f32;
    (0..len)
        .map(|_| {
            let v = if p < pw / 100.0 { 1.0 } else { -1.0 };
            p = (p + f0 / SR).fract();
            v
        })
        .collect()
}

/// A naive (not band-limited) saw, the reference the PolyBLEP one is compared with.
fn naive_saw(f0: f32, len: usize) -> Vec<f32> {
    let mut p = 0.0f32;
    (0..len)
        .map(|_| {
            let v = 2.0 * p - 1.0;
            p = (p + f0 / SR).fract();
            v
        })
        .collect()
}

fn measure(label: &str, x: &[f32], f0: f32) -> (f64, f64) {
    let (worst, total) = alias_db(x, f0, SR, 20000.0);
    println!("{label:<44} worst alias {worst:7.1} dB, total {total:7.1} dB");
    (worst, total)
}

#[test]
fn saw_and_pulse_alias_far_less_than_naive() {
    for f0 in [110.0f32, 440.0, 1760.0, 3520.0] {
        let mut r = osc(&[("base_hz", f0)]);
        let x = r.render(N, 0, |_, _| 0.0);
        let (w, t) = measure(&format!("saw {f0} Hz"), &x, f0);
        let (nw, nt) = measure(&format!("naive saw {f0} Hz"), &naive_saw(f0, N), f0);
        assert!(w < nw - 10.0 && t < nt - 10.0);
    }
    for pw in [5.0f32, 50.0, 95.0] {
        for f0 in [440.0f32, 1760.0] {
            let mut r = osc(&[("base_hz", f0), ("waveform", SQR), ("pw", pw)]);
            let x = r.render(N, 0, |_, _| 0.0);
            let (w, t) = measure(&format!("pulse {pw} % {f0} Hz"), &x, f0);
            let (nw, nt) = measure(
                &format!("naive pulse {pw} % {f0} Hz"),
                &naive_pulse(f0, pw, N),
                f0,
            );
            assert!(w < nw - 8.0 && t < nt - 12.0);
        }
    }
}

#[test]
fn modulated_pulse_width_stays_band_limited() {
    // PWM by a block-rate ramp (what a route does): the width moves every block, and the output
    // has no steps bigger than the pulse's own band-limited edges.
    let mut r = osc(&[("base_hz", 440.0), ("waveform", SQR)]);
    let mut x = Vec::new();
    for b in 0..1500 {
        let pw = 50.0 + 45.0 * (b as f32 * 0.02).sin();
        r.set("pw", pw);
        x.extend_from_slice(&r.block(&[])[0]);
    }
    assert!(x.iter().all(|v| v.is_finite()));
    assert!(peak(&x) <= 1.95);
    let mean = x.iter().sum::<f32>() / x.len() as f32;
    assert!(mean.abs() < 0.01, "PWM DC {mean}");
}

#[test]
fn unison_alias_level_matches_one_oscillator() {
    // Each unison oscillator is band-limited on its own: with no detune the stack is periodic
    // and measurable like one oscillator.
    for f0 in [440.0f32, 1760.0] {
        let mut r = osc(&[("base_hz", f0), ("unison", 4.0), ("detune", 0.0)]);
        let x = r.render(N, 0, |_, _| 0.0);
        measure(&format!("unison 4, 0 ct, saw {f0} Hz"), &x, f0);
        let mut one = osc(&[("base_hz", f0)]);
        let a4 = alias_abs_db(&x, f0, SR, 20000.0);
        let a1 = alias_abs_db(&one.render(N, 0, |_, _| 0.0), f0, SR, 20000.0);
        println!("  alias power: 4 oscillators {a4:.1} dB, one {a1:.1} dB (full-scale sine = 0)");
        assert!(a4 < a1 + 3.0);
    }
}

/// The old hard sync, for comparison: the phase snaps to 0 at the sample after the edge.
fn old_sync(master: &[f32], f: f32) -> Vec<f32> {
    let mut o = FullOsc::new();
    let mut was = false;
    master
        .iter()
        .map(|&m| {
            let high = m > 0.5;
            if high && !was {
                o.hard_sync();
            }
            was = high;
            o.next(f, SR, OscWaveform::Saw)
        })
        .collect()
}

#[test]
fn hard_sync_is_band_limited() {
    for (fm, ratio) in [(220.0f32, 2.37f32), (440.0, 1.61), (880.0, 3.3)] {
        let mut master = osc(&[("base_hz", fm), ("waveform", SQR)]);
        let m = master.render(N, 0, |_, _| 0.0);
        let mut slave = osc(&[("base_hz", fm * ratio)]);
        let x = slave.render(N, 0, |k, i| if k == 1 { m[i] } else { 0.0 });
        let (w, t) = measure(&format!("sync saw {fm}×{ratio}"), &x, fm);
        let (ow, ot) = measure(
            &format!("old naive sync {fm}×{ratio}"),
            &old_sync(&m, fm * ratio),
            fm,
        );
        assert!(w < ow - 6.0 && t < ot - 6.0, "{w} {t} vs {ow} {ot}");
    }
}
