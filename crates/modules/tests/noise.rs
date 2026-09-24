//! `noise`: level and colour, pink slope, independence of seeds, determinism, state carry.
//! `-- --nocapture` prints the measured numbers quoted in docs/sound-palette-batch/README.md.

mod common;
use common::*;

use kabl_modules::builtins::Noise;

fn noise(color: f32, seed: (u64, usize), sr: f32) -> Rig {
    let mut r = Rig::new("noise", &[("color", color)], sr);
    r.m.as_any_mut()
        .downcast_mut::<Noise>()
        .unwrap()
        .seed(seed.0, seed.1);
    r
}

/// Mean power in dB of the band [lo, hi) Hz, averaged over 32 windows of 8192.
fn band_db(x: &[f32], sr: f32, lo: f32, hi: f32) -> f64 {
    let n = 8192;
    let mut acc = 0.0;
    let mut windows = 0;
    for c in x.chunks_exact(n) {
        let p = spectrum(c);
        let (a, b) = ((lo * n as f32 / sr) as usize, (hi * n as f32 / sr) as usize);
        acc += p[a..b].iter().sum::<f64>() / (b - a) as f64;
        windows += 1;
    }
    10.0 * (acc / windows as f64).log10()
}

#[test]
fn both_colours_have_the_same_level_and_no_dc() {
    for color in [0.0, 1.0] {
        let x = noise(color, (7, 0), SR).render(480_000, 0, |_, _| 0.0);
        let mean = x.iter().sum::<f32>() / x.len() as f32;
        println!(
            "colour {color}: RMS {:.2} dBFS, peak {:.2} dBFS, mean {mean:.5}",
            db(rms(&x) as f64),
            db(peak(&x) as f64)
        );
        assert!((db(rms(&x) as f64) - db(0.2)).abs() < 0.3);
        assert!(mean.abs() < 0.02);
        assert!(peak(&x) < 1.0);
    }
}

#[test]
fn white_is_flat_and_pink_falls_3_db_per_octave() {
    for sr in [44100.0, 48000.0, 96000.0] {
        let w = noise(0.0, (3, 0), sr).render(262_144, 0, |_, _| 0.0);
        let p = noise(1.0, (3, 0), sr).render(262_144, 0, |_, _| 0.0);
        let bands = [(100.0, 200.0), (1000.0, 2000.0), (8000.0, 16000.0)];
        let wd: Vec<f64> = bands.iter().map(|&(a, b)| band_db(&w, sr, a, b)).collect();
        let pd: Vec<f64> = bands.iter().map(|&(a, b)| band_db(&p, sr, a, b)).collect();
        // Per-bin power: white is flat; pink loses 10 dB per decade (≈ 3.01 dB/octave).
        let slope = |d: &[f64], i: usize, j: usize, dec: f64| (d[j] - d[i]) / dec;
        let (w1, p1, p2) = (
            slope(&wd, 0, 2, 80f64.log10()),
            slope(&pd, 0, 1, 1.0),
            slope(&pd, 1, 2, 8f64.log10()),
        );
        println!("{sr} Hz: white {w1:+.2} dB/decade; pink {p1:+.2} (100 Hz–1 kHz), {p2:+.2} (1–8 kHz) dB/decade");
        assert!(w1.abs() < 0.5);
        assert!((p1 + 10.0).abs() < 1.0 && (p2 + 10.0).abs() < 1.5);
    }
}

#[test]
fn seeds_give_independent_streams() {
    // Two modules, and two voices of one: correlation near zero, never the same samples.
    let a = noise(0.0, (5, 0), SR).render(96_000, 0, |_, _| 0.0);
    for other in [(6, 0), (5, 1), (5, 7), (500, 0)] {
        let b = noise(0.0, other, SR).render(96_000, 0, |_, _| 0.0);
        let dot: f32 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
        let r = dot / (rms(&a) * rms(&b) * a.len() as f32);
        assert!(r.abs() < 0.02, "{other:?}: correlation {r}");
        assert!(a[..64] != b[..64]);
    }
}

#[test]
fn a_seed_is_reproducible() {
    let a = noise(1.0, (9, 2), SR).render(10_000, 0, |_, _| 0.0);
    let b = noise(1.0, (9, 2), SR).render(10_000, 0, |_, _| 0.0);
    assert_eq!(a, b);
}

#[test]
fn carry_continues_the_stream_and_never_copies_another_lane() {
    let mut a = noise(1.0, (4, 1), SR);
    let mut reference = noise(1.0, (4, 1), SR);
    for _ in 0..50 {
        a.block(&[]);
        reference.block(&[]);
    }
    // The compiler's order: a fresh instance is seeded, then state is carried into it.
    let mut b = noise(1.0, (4, 1), SR);
    b.m.carry_from(a.m.as_ref());
    for _ in 0..20 {
        assert_eq!(b.block(&[])[0], reference.block(&[])[0]);
    }
    // Lane 2 offered lane 1's state keeps its own stream.
    let mut c = noise(1.0, (4, 2), SR);
    let mut own = noise(1.0, (4, 2), SR);
    c.m.carry_from(a.m.as_ref());
    assert_eq!(c.block(&[])[0], own.block(&[])[0]);
}

#[test]
fn level_glides() {
    let mut r = noise(0.0, (1, 0), SR);
    r.set("level_db", 0.0);
    r.block(&[]);
    r.set("level_db", -60.0);
    let first = r.block(&[])[0];
    assert!(rms(&first) > 0.1, "a level change fades, it doesn't cut");
    for _ in 0..200 {
        r.block(&[]);
    }
    assert!(rms(&r.block(&[])[0]) < 0.001);
}
