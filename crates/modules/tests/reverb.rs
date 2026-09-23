//! `reverb` DSP: decay time, damping, stereo image, mix endpoints, block and rate independence,
//! finite output, smooth changes and history carry.

use kabl_modules::builtins::Reverb;
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{Module, ProcessIo, Signal};

const SR: f32 = 48000.0;

/// decay s, damp Hz, mix %, predelay ms, width %.
#[derive(Clone, Copy)]
struct P([f32; 5]);

impl P {
    fn wet(decay: f32) -> Self {
        P([decay, 16000.0, 100.0, 0.0, 100.0])
    }
    fn with(mut self, i: usize, v: f32) -> Self {
        self.0[i] = v;
        self
    }
}

fn reverb(sr: f32, block: usize) -> Reverb {
    let mut r = Reverb::new();
    r.prepare(
        sr,
        block,
        &QualityConfig {
            tier: QualityTier::Live,
        },
    );
    r
}

fn run_with(
    r: &mut Reverb,
    l: &[f32],
    rr: &[f32],
    block: usize,
    params: impl Fn(usize) -> P,
) -> (Vec<f32>, Vec<f32>) {
    let (mut ol, mut or) = (vec![0.0; l.len()], vec![0.0; l.len()]);
    let mut i = 0;
    while i < l.len() {
        let n = block.min(l.len() - i);
        let ins = [Signal::Buffer(&l[i..i + n]), Signal::Buffer(&rr[i..i + n])];
        let ps = params(i).0.map(Signal::Scalar);
        let (mut a, mut b) = (vec![0.0; block], vec![0.0; block]);
        let mut outs = [&mut a[..], &mut b[..]];
        r.process(&mut ProcessIo::new(&ins, &mut outs, &ps, n));
        ol[i..i + n].copy_from_slice(&a[..n]);
        or[i..i + n].copy_from_slice(&b[..n]);
        i += n;
    }
    (ol, or)
}

fn run(sr: f32, p: P, l: &[f32], r: &[f32]) -> (Vec<f32>, Vec<f32>) {
    run_with(&mut reverb(sr, 64), l, r, 64, |_| p)
}

fn noise(len: usize, seed: u32) -> Vec<f32> {
    let mut s = seed;
    (0..len)
        .map(|_| {
            s = s.wrapping_mul(1664525).wrapping_add(1013904223);
            (s >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
        })
        .collect()
}

/// `burst` seconds of noise, then silence, to `total` seconds.
fn burst(sr: f32, burst: f32, total: f32) -> Vec<f32> {
    let mut v = noise((total * sr) as usize, 7);
    v[(burst * sr) as usize..].fill(0.0);
    v
}

fn energy(v: &[f32]) -> f64 {
    v.iter().map(|&x| (x as f64).powi(2)).sum::<f64>() / v.len().max(1) as f64
}

fn db(e: f64) -> f64 {
    10.0 * e.log10()
}

/// RT60 from the slope of the energy envelope (50 ms windows) between −5 and −35 dB below the
/// level just after the input stops.
fn rt60(sr: f32, out: &[f32], stop: usize) -> f32 {
    let w = (0.05 * sr) as usize;
    let levels: Vec<f64> = out[stop..]
        .chunks(w)
        .map(|c| db(energy(c).max(1e-30)))
        .collect();
    let top = levels[1];
    let t = |drop: f64| levels.iter().position(|&l| l < top - drop).unwrap() as f32;
    let slope = 30.0 / ((t(35.0) - t(5.0)) * 0.05);
    60.0 / slope
}

#[test]
fn decay_time_follows_the_decay_knob() {
    for (sr, decay) in [(SR, 1.0), (SR, 4.0), (44100.0, 2.0), (96000.0, 2.0)] {
        let input = burst(sr, 0.3, 0.3 + 4.0 * decay);
        let (l, r) = run(sr, P::wet(decay), &input, &input);
        let stop = (0.3 * sr) as usize;
        let (tl, tr) = (rt60(sr, &l, stop), rt60(sr, &r, stop));
        println!("decay {decay} s at {sr} Hz: measured RT60 {tl:.2} / {tr:.2} s");
        for t in [tl, tr] {
            assert!(
                t > decay * 0.6 && t < decay * 1.5,
                "decay {decay} at {sr}: RT60 {t}"
            );
        }
    }
}

#[test]
fn short_space_setting_dies_quickly() {
    let input = burst(SR, 0.2, 1.7);
    let (l, _) = run(SR, P::wet(0.3), &input, &input);
    let stop = (0.2 * SR) as usize;
    let after = energy(&l[stop + (0.6 * SR) as usize..stop + (0.8 * SR) as usize]);
    let during = energy(&l[stop - (0.1 * SR) as usize..stop]);
    assert!(db(after / during) < -60.0, "{}", db(after / during));
}

/// Tail energy above ~4 kHz relative to all of it, from a first difference.
fn brightness(v: &[f32]) -> f64 {
    let diff: Vec<f32> = v.windows(2).map(|w| w[1] - w[0]).collect();
    energy(&diff) / energy(v)
}

#[test]
fn damping_darkens_the_tail() {
    let input = burst(SR, 0.2, 2.2);
    let stop = (0.7 * SR) as usize;
    let bright = run(SR, P::wet(3.0), &input, &input).0;
    let dark = run(SR, P::wet(3.0).with(1, 1000.0), &input, &input).0;
    let (b, d) = (brightness(&bright[stop..]), brightness(&dark[stop..]));
    println!("brightness: 16 kHz {b:.4}, 1 kHz {d:.4}");
    assert!(d < b * 0.3, "{d} vs {b}");
}

#[test]
fn a_left_source_stays_left_and_width_zero_is_mono() {
    let n = (0.08 * SR) as usize;
    let mut input = vec![0.0; n];
    input[..(0.02 * SR) as usize].copy_from_slice(&noise((0.02 * SR) as usize, 3));
    let silent = vec![0.0; n];
    let (l, r) = run(SR, P::wet(2.0), &input, &silent);
    let ratio = db(energy(&l) / energy(&r));
    println!("left-only input, first 80 ms: L/R {ratio:.1} dB");
    assert!(ratio > 3.0, "{ratio}");
    let (l, r) = run(SR, P::wet(2.0).with(4, 0.0), &input, &silent);
    assert_eq!(l, r);
}

#[test]
fn mix_endpoints_are_exactly_dry_and_exactly_wet() {
    let input = noise(4800, 5);
    let other = noise(4800, 9);
    let (l, r) = run(SR, P::wet(3.0).with(2, 0.0), &input, &other);
    assert_eq!(l, input);
    assert_eq!(r, other);
    // Fully wet with a 50 ms pre-delay: nothing comes out before the pre-delay.
    let (l, r) = run(SR, P::wet(3.0).with(3, 50.0), &input, &other);
    let quiet = (0.049 * SR) as usize;
    assert!(l[..quiet].iter().chain(&r[..quiet]).all(|&x| x == 0.0));
    assert!(l[quiet..].iter().any(|&x| x != 0.0));
}

#[test]
fn block_size_does_not_change_the_output() {
    let input = burst(SR, 0.05, 0.5);
    let reference = run_with(&mut reverb(SR, 64), &input, &input, 64, |_| P::wet(2.0));
    for block in [1, 17] {
        let out = run_with(&mut reverb(SR, block), &input, &input, block, |_| {
            P::wet(2.0)
        });
        assert_eq!(out, reference, "block {block}");
    }
}

#[test]
fn longest_decay_on_loud_noise_stays_finite_and_bounded() {
    let len = (20.0 * SR) as usize;
    let input = noise(len, 11);
    let (l, r) = run_with(&mut reverb(SR, 64), &input, &input, 64, |i| {
        // Swept damping and pre-delay while it runs.
        let t = i as f32 / len as f32;
        P([30.0, 500.0 + 15000.0 * t, 100.0, 250.0 * t, 100.0])
    });
    let peak = l.iter().chain(&r).fold(0f32, |m, &x| m.max(x.abs()));
    println!("30 s decay, 20 s full-scale noise: peak {peak:.2}");
    assert!(l.iter().chain(&r).all(|x| x.is_finite()));
    assert!(peak < 8.0, "{peak}");
}

/// Largest sample-to-sample step.
fn max_step(v: &[f32]) -> f32 {
    v.windows(2).fold(0f32, |m, w| m.max((w[1] - w[0]).abs()))
}

#[test]
fn control_changes_mid_tail_are_smooth() {
    let input = burst(SR, 0.3, 2.0);
    let at = (1.0 * SR) as usize;
    let steady = run(SR, P::wet(4.0).with(2, 50.0), &input, &input).0;
    let window = |v: &[f32]| max_step(&v[at..at + 9600]);
    let base = window(&steady);
    for (i, v) in [(0, 0.5), (1, 800.0), (2, 100.0), (3, 200.0), (4, 0.0)] {
        let changed = run_with(&mut reverb(SR, 64), &input, &input, 64, |s| {
            let p = P::wet(4.0).with(2, 50.0);
            if s >= at {
                p.with(i, v)
            } else {
                p
            }
        })
        .0;
        let step = window(&changed);
        println!("param {i} → {v}: max step {step:.4} (steady {base:.4})");
        assert!(step < base * 2.0 + 1e-3, "param {i}: {step} vs {base}");
    }
}

#[test]
fn carry_from_copies_the_whole_history() {
    let input = burst(SR, 0.1, 0.5);
    let mut a = reverb(SR, 64);
    run_with(&mut a, &input, &input, 64, |_| P::wet(3.0));
    let mut b = reverb(SR, 64);
    b.carry_from(&a);
    let silence = vec![0.0; 24000];
    let from_a = run_with(&mut a, &silence, &silence, 64, |_| P::wet(3.0));
    let from_b = run_with(&mut b, &silence, &silence, 64, |_| P::wet(3.0));
    assert_eq!(from_a, from_b);
    assert!(energy(&from_b.0) > 1e-6);
    // Another kind carries nothing.
    let mut c = reverb(SR, 64);
    c.carry_from(&kabl_modules::builtins::Delay::new());
    let from_c = run_with(&mut c, &silence, &silence, 64, |_| P::wet(3.0));
    assert!(from_c.0.iter().all(|&x| x == 0.0));
}
