//! `osc.va` 16-voice unison: level law, alias level at 48 and 96 kHz, width, blend, curve,
//! numeric safety. `cargo test -p kabl-modules --test osc_unison -- --nocapture` prints the
//! numbers quoted in docs/unison/README.md.

mod common;
use common::*;

const N: usize = 16384;

fn osc(set: &[(&str, f32)]) -> Rig {
    Rig::new("osc.va", set, SR)
}

#[test]
fn sixteen_voices_keep_the_level_of_one() {
    // Equal power: RMS holds within 1.5 dB from 1 to 16 oscillators, at any blend.
    for blend in [0.0f32, 50.0, 100.0] {
        let levels: Vec<f64> = (1..=16)
            .map(|count| {
                let mut r = osc(&[
                    ("base_hz", 220.0),
                    ("unison", count as f32),
                    ("detune", 30.0),
                    ("blend", blend),
                ]);
                db(rms(&r.render(96000, 0, |_, _| 0.0)[4800..]) as f64)
            })
            .collect();
        println!("RMS dBFS, count 1..16, blend {blend}: {levels:.1?}");
        for l in &levels {
            assert!((l - levels[0]).abs() < 1.5, "blend {blend}: {levels:?}");
        }
    }
}

#[test]
fn sixteen_voices_stay_band_limited_at_48_and_96_khz() {
    for sr in [48000.0f32, 96000.0] {
        for f0 in [440.0f32, 1760.0, 3520.0] {
            let set = [("base_hz", f0), ("unison", 16.0), ("detune", 0.0)];
            let mut r = Rig::new("osc.va", &set, sr);
            let x = r.render(N, 0, |_, _| 0.0);
            assert!(x.iter().all(|v| v.is_finite()) && peak(&x) < 8.0);
            let mut one = Rig::new("osc.va", &[("base_hz", f0)], sr);
            let a1 = alias_abs_db(&one.render(N, 0, |_, _| 0.0), f0, sr, 20000.0);
            let a16 = alias_abs_db(&x, f0, sr, 20000.0);
            println!("alias power @ {sr}, {f0} Hz: 16 voices {a16:.1} dB, one {a1:.1} dB");
            assert!(a16 < a1 + 3.0);
        }
    }
}

#[test]
fn width_pans_the_stack_and_zero_is_mono() {
    let set = [("base_hz", 220.0), ("unison", 8.0), ("detune", 30.0)];
    let mono = osc(&set).render(9600, 0, |_, _| 0.0);
    let right = osc(&set).render(9600, 2, |_, _| 0.0);
    assert_eq!(mono, right);
    let mut wide = osc(&[
        ("base_hz", 220.0),
        ("unison", 8.0),
        ("detune", 30.0),
        ("width", 100.0),
    ]);
    let (mut l, mut r) = (Vec::new(), Vec::new());
    for _ in 0..300 {
        let b = wide.block(&[]);
        l.extend_from_slice(&b[1]);
        r.extend_from_slice(&b[2]);
    }
    let diff: Vec<f32> = l.iter().zip(&r).map(|(a, b)| a - b).collect();
    assert!(rms(&diff) > 0.2 * rms(&l), "channels differ");
    let (dl, dr) = (db(rms(&l) as f64), db(rms(&r) as f64));
    println!(
        "width 100: left {dl:.1} dB, right {dr:.1} dB, mono {:.1} dB",
        db(rms(&mono) as f64)
    );
    assert!((dl - dr).abs() < 2.5);
}

#[test]
fn blend_and_curve_move_the_stack() {
    let render = |extra: &[(&str, f32)]| {
        let mut s = vec![("base_hz", 220.0), ("unison", 7.0), ("detune", 40.0)];
        s.extend_from_slice(extra);
        osc(&s).render(9600, 0, |_, _| 0.0)
    };
    let mid = render(&[]);
    for extra in [("blend", 0.0), ("blend", 100.0), ("curve", 100.0)] {
        let x = render(&[extra]);
        assert!(x.iter().all(|v| v.is_finite()));
        assert_ne!(x, mid, "{extra:?}");
    }
    // Centre only: the one centre oscillator (at its own start phase), a lone saw's level.
    let centre = render(&[("blend", 0.0)]);
    let one = osc(&[("base_hz", 220.0)]).render(9600, 0, |_, _| 0.0);
    assert!((db(rms(&centre[4800..]) as f64) - db(rms(&one[4800..]) as f64)).abs() < 0.1);
}

#[test]
fn every_setting_and_rate_is_finite() {
    for sr in [44100.0f32, 96000.0, 192000.0] {
        for (count, wave) in [(1.0f32, 2.0f32), (16.0, 3.0), (16.0, 2.0)] {
            let set = [
                ("base_hz", 19000.0),
                ("unison", count),
                ("detune", 100.0),
                ("curve", 100.0),
                ("width", 100.0),
                ("blend", 0.0),
                ("fine", 100.0),
                ("waveform", wave),
                ("restart", 1.0),
            ];
            let mut r = Rig::new("osc.va", &set, sr);
            let x = r.render(8192, 0, |k, i| {
                if k == 0 {
                    36.0
                } else {
                    (i % 50 > 25) as u8 as f32
                }
            });
            assert!(x.iter().all(|v| v.is_finite()), "{sr} {count} {wave}");
        }
    }
}

/// Width 0/0.5/1 on a unison saw: L/R correlation and the mono fold ((L+R)/2) against the
/// width-0 level. Run with `--nocapture` for the numbers.
#[test]
fn width_keeps_the_mono_fold_alive() {
    let mut ref_db = 0.0;
    for (w, floor) in [(0.0, -0.1), (50.0, -3.0), (100.0, -6.0)] {
        let mut o = osc(&[
            ("base_hz", 220.0),
            ("unison", 8.0),
            ("detune", 30.0),
            ("width", w),
        ]);
        let (mut l, mut r) = (Vec::new(), Vec::new());
        for _ in 0..300 {
            let b = o.block(&[]);
            l.extend_from_slice(&b[1]);
            r.extend_from_slice(&b[2]);
        }
        let (l, r) = (&l[4800..], &r[4800..]);
        let dot: f32 = l.iter().zip(r).map(|(a, b)| a * b).sum();
        let corr = dot
            / (l.iter().map(|a| a * a).sum::<f32>() * r.iter().map(|b| b * b).sum::<f32>()).sqrt();
        let fold: Vec<f32> = l.iter().zip(r).map(|(a, b)| 0.5 * (a + b)).collect();
        let fold_db = db(rms(&fold) as f64);
        if w == 0.0 {
            ref_db = fold_db;
        }
        println!(
            "width {w}: corr {corr:.3}, mono fold {:.2} dB vs width 0",
            fold_db - ref_db
        );
        assert!(fold_db - ref_db > floor);
    }
}
