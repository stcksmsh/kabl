//! The 1.0 effects and filters: phaser, flanger, compressor, eq, limiter, bitcrusher,
//! filter.comb, filter.formant and the SVF's notch output. Neutral settings, measured responses,
//! pitch accuracy, dynamics timing, extreme-setting safety at 44.1–192 kHz and block sizes 1 to
//! large, state carry, and no allocation on the audio path. `-- --nocapture` prints the numbers.

mod common;
use common::*;

// assert_no_alloc compiles to nothing in release builds (its default), so the guard is debug-only.
#[cfg(debug_assertions)]
use assert_no_alloc::{assert_no_alloc, AllocDisabler};
#[cfg(not(debug_assertions))]
fn assert_no_alloc<T>(f: impl FnOnce() -> T) -> T {
    f()
}
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{registry, Module, PortDirection, ProcessIo, Signal};
use std::f32::consts::PI;

#[cfg(debug_assertions)]
#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const NEW_KINDS: [&str; 8] = [
    "phaser",
    "flanger",
    "compressor",
    "eq",
    "limiter",
    "bitcrusher",
    "filter.comb",
    "filter.formant",
];

/// A module under test: any sample rate and block size, scalar params that tests may change
/// between calls.
struct Dut {
    m: Box<dyn Module>,
    params: Vec<f32>,
    n_in: usize,
    n_out: usize,
}

impl Dut {
    fn new(kind: &str, set: &[(&str, f32)], sr: f32, max_block: usize) -> Dut {
        let mut m = registry::create(kind).unwrap();
        m.prepare(
            sr,
            max_block,
            &QualityConfig {
                tier: QualityTier::Live,
            },
        );
        let info = m.info();
        let params = info.params.iter().map(|p| p.default).collect();
        let n_in = info
            .ports
            .iter()
            .filter(|p| p.direction == PortDirection::Input)
            .count();
        let n_out = info.ports.len() - n_in;
        let mut d = Dut {
            m,
            params,
            n_in,
            n_out,
        };
        for &(k, v) in set {
            d.set(k, v);
        }
        d
    }

    fn set(&mut self, name: &str, v: f32) {
        let i = self
            .m
            .info()
            .params
            .iter()
            .position(|p| p.name == name)
            .unwrap();
        self.params[i] = v;
    }

    /// Feeds `ins` (one slice per leading input port, all the same length; inputs beyond them
    /// are unpatched, a held scalar 0) through in chunks of `block`. Returns each output.
    fn run(&mut self, ins: &[&[f32]], block: usize) -> Vec<Vec<f32>> {
        let len = ins[0].len();
        let mut out = vec![Vec::with_capacity(len); self.n_out];
        let mut t = 0;
        while t < len {
            let n = block.min(len - t);
            let sig: Vec<Signal> = (0..self.n_in)
                .map(|k| {
                    ins.get(k)
                        .map_or(Signal::Scalar(0.0), |b| Signal::Buffer(&b[t..t + n]))
                })
                .collect();
            let ps: Vec<Signal> = self.params.iter().map(|&v| Signal::Scalar(v)).collect();
            let mut bufs = vec![vec![0f32; n]; self.n_out];
            {
                let mut refs: Vec<&mut [f32]> = bufs.iter_mut().map(|b| &mut b[..]).collect();
                let mut io = ProcessIo::new(&sig, &mut refs, &ps, n);
                self.m.process(&mut io);
            }
            for (o, b) in out.iter_mut().zip(&bufs) {
                o.extend_from_slice(b);
            }
            t += n;
        }
        out
    }

    /// Mono-ish: the same signal on the first two (stereo) or one input.
    fn run_mono(&mut self, x: &[f32], block: usize) -> Vec<Vec<f32>> {
        if self.m.info().ports[1].name == "in_r" {
            self.run(&[x, x], block)
        } else {
            self.run(&[x], block)
        }
    }
}

fn sine(f: f32, a: f32, sr: f32, len: usize) -> Vec<f32> {
    (0..len)
        .map(|i| a * (2.0 * PI * f * i as f32 / sr).sin())
        .collect()
}

fn noise(seed: u32, len: usize) -> Vec<f32> {
    let mut x = seed | 1;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            (x >> 8) as f32 / 8_388_608.0 - 1.0
        })
        .collect()
}

/// Gain in dB of the first output for a sine at `f` (second half of one second, so a settled
/// response).
fn gain_db(kind: &str, set: &[(&str, f32)], f: f32) -> f64 {
    let x = sine(f, 0.25, SR, SR as usize);
    let y = Dut::new(kind, set, SR, 64).run_mono(&x, 64).remove(0);
    let h = x.len() / 2;
    db(rms(&y[h..]) as f64 / rms(&x[h..]) as f64)
}

/// Welch-averaged power spectrum (8192-point, Blackman-Harris) of `x` in dB per bin.
fn welch(x: &[f32]) -> Vec<f64> {
    const N: usize = 8192;
    let mut acc = vec![0.0; N / 2 + 1];
    let mut k = 0;
    for seg in x.as_chunks::<N>().0 {
        for (a, p) in acc.iter_mut().zip(spectrum(seg)) {
            *a += p;
        }
        k += 1;
    }
    acc.iter()
        .map(|a| 10.0 * (a / k as f64).max(1e-30).log10())
        .collect()
}

fn bin_hz(bin: usize) -> f32 {
    bin as f32 * SR / 8192.0
}

// ---------------------------------------------------------------- neutral settings

#[test]
fn neutral_settings_pass_the_input_exactly() {
    let x = noise(7, 4800);
    let cases: [(&str, &[(&str, f32)]); 6] = [
        ("phaser", &[("mix", 0.0)]),
        ("flanger", &[("mix", 0.0)]),
        ("bitcrusher", &[("mix", 0.0)]),
        ("eq", &[]),
        (
            "compressor",
            &[("threshold_db", 0.0), ("ratio", 1.0), ("knee_db", 0.0)],
        ),
        ("limiter", &[("ceiling_db", 0.0)]),
    ];
    for (kind, set) in cases {
        let y = Dut::new(kind, set, SR, 64).run(&[&x, &x], 64);
        assert_eq!(y[0], x, "{kind} left");
        assert_eq!(y[1], x, "{kind} right");
    }
}

#[test]
fn stereo_sides_stay_independent() {
    // A signal on the left only leaves the right of every stereo effect silent where nothing
    // couples the sides (the dynamics link their detectors on purpose, so only the others).
    let x = noise(3, 4800);
    let z = vec![0.0; 4800];
    for kind in ["phaser", "flanger", "eq", "bitcrusher"] {
        let y = Dut::new(kind, &[], SR, 64).run(&[&x, &z], 64);
        assert!(peak(&y[1]) < 1e-6, "{kind}: right {}", peak(&y[1]));
        assert!(peak(&y[0]) > 0.05, "{kind}: left");
    }
}

// ---------------------------------------------------------------- phaser, flanger

#[test]
fn phaser_wet_is_all_pass_and_blend_has_notches() {
    let x = noise(1, 8192 * 24);
    let set = [("depth", 0.0), ("feedback", 0.0), ("stages", 4.0)];
    let wet = Dut::new("phaser", &[("mix", 100.0), set[0], set[1], set[2]], SR, 64)
        .run_mono(&x, 64)
        .remove(0);
    let ratio = db(rms(&wet[8192..]) as f64 / rms(&x[8192..]) as f64);
    println!("phaser wet level vs input: {ratio:.3} dB");
    assert!(ratio.abs() < 0.2);
    let half = Dut::new("phaser", &[("mix", 50.0), set[0], set[1], set[2]], SR, 64)
        .run_mono(&x, 64)
        .remove(0);
    let (p, q) = (welch(&half[8192..]), welch(&x[8192..]));
    let lo = (100.0 / bin_hz(1)) as usize;
    let hi = (8000.0 / bin_hz(1)) as usize;
    let deepest = (lo..hi).map(|b| p[b] - q[b]).fold(f64::MAX, f64::min);
    println!("phaser 50 % deepest notch {deepest:.1} dB");
    assert!(deepest < -20.0);
}

#[test]
fn phaser_stage_count_changes_the_notch_count() {
    let x = noise(2, 8192 * 24);
    let count = |stages: f32| {
        let y = Dut::new(
            "phaser",
            &[
                ("depth", 0.0),
                ("feedback", 0.0),
                ("mix", 50.0),
                ("stages", stages),
            ],
            SR,
            64,
        )
        .run_mono(&x, 64)
        .remove(0);
        let (p, q) = (welch(&y[8192..]), welch(&x[8192..]));
        let r: Vec<f64> = (1..p.len()).map(|b| p[b] - q[b]).collect();
        // Notches: local minima below -10 dB, at least 50 bins apart.
        let mut n = 0;
        let mut last = 0;
        for b in 60..r.len() - 60 {
            if r[b] < -10.0 && r[b] < r[b - 1] && r[b] <= r[b + 1] && b > last + 50 {
                n += 1;
                last = b;
            }
        }
        n
    };
    let (two, six, twelve) = (count(2.0), count(6.0), count(12.0));
    println!("phaser notches at 2, 6, 12 stages: {two}, {six}, {twelve}");
    assert!(two < six && six < twelve, "{two} {six} {twelve}");
}

#[test]
fn flanger_comb_notch_and_feedback_sign() {
    // Depth 0: a fixed 3.5 ms delay (168 samples at 48 kHz): the dry + delayed blend notches at
    // 1/(2·3.5 ms) = 142.86 Hz and peaks at twice that.
    let set = [("depth", 0.0), ("mix", 50.0)];
    let notch = gain_db("flanger", &[set[0], set[1], ("feedback", 0.0)], 142.857);
    let peak = gain_db("flanger", &[set[0], set[1], ("feedback", 0.0)], 285.714);
    println!("flanger notch {notch:.1} dB, peak {peak:.1} dB");
    assert!(notch < -40.0 && peak > -0.5);
    let pos = gain_db("flanger", &[set[0], set[1], ("feedback", 70.0)], 285.714);
    let neg = gain_db("flanger", &[set[0], set[1], ("feedback", -70.0)], 285.714);
    println!("flanger at a comb peak: +70 % {pos:.1} dB, -70 % {neg:.1} dB");
    assert!(pos > peak + 3.0 && neg < peak - 1.0);
    // At the notch the signs swap: positive feedback deepens it, negative turns it into a peak.
    let pos_n = gain_db("flanger", &[set[0], set[1], ("feedback", 70.0)], 142.857);
    let neg_n = gain_db("flanger", &[set[0], set[1], ("feedback", -70.0)], 142.857);
    println!("flanger at the notch: +70 % {pos_n:.1} dB, -70 % {neg_n:.1} dB");
    assert!(pos_n < -10.0 && neg_n > -3.0);
}

// ---------------------------------------------------------------- compressor, limiter

fn dc(v: f32, len: usize) -> Vec<f32> {
    vec![v; len]
}

#[test]
fn compressor_static_curve_and_knee() {
    // DC at -6 dBFS, threshold -20: over = 14 dB, ratio 4: 10.5 dB of reduction (hard knee).
    let x = dc(0.5011872, 48000);
    let hard = [
        ("threshold_db", -20.0),
        ("ratio", 4.0),
        ("knee_db", 0.0),
        ("attack_ms", 0.1),
    ];
    let y = Dut::new("compressor", &hard, SR, 64).run(&[&x, &x], 64);
    let gr = y[2][40000];
    println!("compressor hard knee: {gr:.2} dB (expect 10.50)");
    assert!((gr - 10.5).abs() < 0.05);
    assert!((db(y[0][40000] as f64 / x[0] as f64) + 10.5).abs() < 0.05);
    // Level exactly at the threshold with a 10 dB knee: 0.75·10/8 = 0.9375 dB.
    let at = dc(0.1, 48000);
    let soft = [
        ("threshold_db", -20.0),
        ("ratio", 4.0),
        ("knee_db", 10.0),
        ("attack_ms", 0.1),
    ];
    let gr = Dut::new("compressor", &soft, SR, 64).run(&[&at, &at], 64)[2][40000];
    println!("compressor soft knee at threshold: {gr:.4} dB (expect 0.9375)");
    assert!((gr - 0.9375).abs() < 0.01);
    // Makeup adds after.
    let mk = Dut::new(
        "compressor",
        &[("threshold_db", 0.0), ("makeup_db", 6.0)],
        SR,
        64,
    )
    .run(&[&at, &at], 64);
    assert!((db(mk[0][40000] as f64 / 0.1) - 6.0).abs() < 0.01);
}

#[test]
fn compressor_attack_and_release_times() {
    let set = [
        ("threshold_db", -20.0),
        ("ratio", 20.0),
        ("knee_db", 0.0),
        ("attack_ms", 10.0),
        ("release_ms", 100.0),
    ];
    let mut x = dc(0.0, 4800);
    x.extend(dc(0.5, 48000));
    let t_off = x.len();
    x.extend(dc(0.0, 48000));
    let y = Dut::new("compressor", &set, SR, 64).run(&[&x, &x], 64);
    let gr = &y[2];
    let top = gr[t_off - 1];
    let t63 = gr[4800..].iter().position(|&g| g >= 0.632 * top).unwrap() as f32 / SR * 1000.0;
    let t37 = gr[t_off..].iter().position(|&g| g <= 0.368 * top).unwrap() as f32 / SR * 1000.0;
    println!("compressor: attack 63 % at {t63:.2} ms (10), release 37 % at {t37:.1} ms (100)");
    assert!((t63 - 10.0).abs() < 1.0 && (t37 - 100.0).abs() < 5.0);
}

#[test]
fn compressor_sidechain_ducks_by_the_other_signal() {
    let quiet = dc(0.1, 48000);
    let loud = dc(0.5, 48000);
    let set = [
        ("threshold_db", -20.0),
        ("ratio", 4.0),
        ("knee_db", 0.0),
        ("attack_ms", 0.1),
    ];
    // Unpatched: 0.1 is -20 dB, at the threshold: untouched.
    let free = Dut::new("compressor", &set, SR, 64).run(&[&quiet, &quiet], 64);
    assert_eq!(free[2][40000], 0.0);
    // Patched with the loud signal: ducked by what the loud signal needs (-6 dB: 10.5 dB).
    let duck = Dut::new("compressor", &set, SR, 64).run(&[&quiet, &quiet, &loud], 64);
    println!("sidechain gr {:.2} dB", duck[2][40000]);
    assert!((duck[2][40000] - 10.46).abs() < 0.1);
    assert!((db(duck[0][40000] as f64 / 0.1) + 10.46).abs() < 0.1);
}

#[test]
fn limiter_never_exceeds_the_ceiling() {
    let x: Vec<f32> = noise(5, 48000).iter().map(|v| v * 8.0).collect();
    for (gain, ceil) in [
        (0.0, -6.0),
        (24.0, -6.0),
        (12.0, -1.0),
        (24.0, -24.0),
        (0.0, 0.0),
    ] {
        let set = [("gain_db", gain), ("ceiling_db", ceil)];
        let y = Dut::new("limiter", &set, SR, 64).run(&[&x, &x], 64);
        let c = 10f32.powf(ceil / 20.0);
        let p = peak(&y[0]).max(peak(&y[1]));
        assert!(
            p <= c * (1.0 + 1e-6),
            "gain {gain} ceiling {ceil}: peak {p} > {c}"
        );
    }
}

#[test]
fn limiter_releases() {
    let mut x = dc(2.0, 2400);
    x.extend(dc(0.05, 48000));
    let set = [("ceiling_db", 0.0), ("release_ms", 100.0)];
    let y = Dut::new("limiter", &set, SR, 64)
        .run(&[&x, &x], 64)
        .remove(0);
    // After the burst the gain is 0.5; a 100 ms one-pole passes 63 % of the way to 1 in 100 ms.
    let g = |i: usize| y[i] / 0.05;
    let g0 = g(2400 + 8);
    let g100 = g(2400 + 4800);
    println!("limiter gain after burst {g0:.3}, +100 ms {g100:.3} (expect 0.5, 0.816)");
    assert!((g0 - 0.5).abs() < 0.02 && (g100 - 0.816).abs() < 0.02);
    assert!(g(2400 + 48000 - 1) > 0.999);
}

// ---------------------------------------------------------------- eq

#[test]
fn eq_bands_hit_their_gains() {
    let g = |set: &[(&str, f32)], f| gain_db("eq", set, f);
    let low = [("low_db", 12.0), ("low_hz", 100.0)];
    let (l20, l5k) = (g(&low, 20.0), g(&low, 5000.0));
    let high = [("high_db", -12.0), ("high_hz", 8000.0)];
    let (h100, h18k) = (g(&high, 100.0), g(&high, 18000.0));
    let bell = [("mid1_db", 12.0), ("mid1_hz", 1000.0), ("mid1_q", 1.0)];
    let (b1k, b4k) = (g(&bell, 1000.0), g(&bell, 4000.0));
    let narrow = [("mid2_db", -12.0), ("mid2_hz", 3000.0), ("mid2_q", 8.0)];
    let (n3k, n4k) = (g(&narrow, 3000.0), g(&narrow, 3600.0));
    println!(
        "eq: low shelf {l20:.2}/{l5k:.2}, high shelf {h100:.2}/{h18k:.2}, bell {b1k:.2}/{b4k:.2}, narrow {n3k:.2}/{n4k:.2}"
    );
    assert!((l20 - 12.0).abs() < 0.6 && l5k.abs() < 0.3);
    assert!(h100.abs() < 0.3 && (h18k + 12.0).abs() < 1.5);
    assert!((b1k - 12.0).abs() < 0.2 && b4k < 3.0);
    assert!((n3k + 12.0).abs() < 0.2 && n4k > -6.0);
}

#[test]
fn eq_gain_step_does_not_zipper() {
    // A 5 kHz tone through a bell at 5 kHz whose gain steps 0 -> +12 dB halfway: the glide keeps
    // the step's sidebands far under the tone.
    let x = sine(5000.0, 0.25, SR, 48000);
    let mut d = Dut::new("eq", &[("mid2_hz", 5000.0), ("mid2_q", 2.0)], SR, 64);
    let mut y = d.run(&[&x[..24000], &x[..24000]], 64).remove(0);
    d.set("mid2_db", 12.0);
    y.extend(d.run(&[&x[24000..], &x[24000..]], 64).remove(0));
    let win = &y[24000..24000 + 8192];
    let s = spectrum(win);
    let centre = (5000.0 / SR * 8192.0) as usize;
    let near = s[centre - 4..=centre + 4].iter().sum::<f64>();
    let far: f64 = s
        .iter()
        .enumerate()
        .filter(|(b, _)| b.abs_diff(centre) > 60)
        .map(|(_, p)| p)
        .sum();
    let ratio = 10.0 * (far / near).log10();
    println!("eq step: energy away from the tone {ratio:.1} dB");
    assert!(ratio < -60.0);
}

// ---------------------------------------------------------------- bitcrusher

#[test]
fn bitcrusher_quantises_and_holds() {
    let x = sine(220.0, 0.9, SR, 4800);
    let y = Dut::new("bitcrusher", &[("bits", 3.0), ("downsample", 1.0)], SR, 64)
        .run(&[&x, &x], 64)
        .remove(0);
    // 3 bits: step 2^-2 = 0.25.
    assert!(y
        .iter()
        .all(|v| (v / 0.25 - (v / 0.25).round()).abs() < 1e-6));
    assert!(
        y.iter()
            .map(|v| (v * 1000.0).round() as i32)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            <= 9
    );
    let z = Dut::new("bitcrusher", &[("bits", 16.0), ("downsample", 8.0)], SR, 64)
        .run(&[&x, &x], 64)
        .remove(0);
    // Each held value lasts 8 samples.
    for run in z.chunks(8).take(100) {
        assert!(run.iter().all(|&v| v == run[0]));
    }
    let c: Vec<_> = z.chunks(8).take(100).map(|r| r[0]).collect();
    assert!(c.windows(2).any(|w| w[0] != w[1]));
    // A fractional factor holds 2.5 samples on average.
    let f = Dut::new("bitcrusher", &[("bits", 16.0), ("downsample", 2.5)], SR, 64)
        .run(&[&x, &x], 64)
        .remove(0);
    let changes = f.windows(2).filter(|w| w[0] != w[1]).count() as f32;
    assert!((changes / f.len() as f32 - 0.4).abs() < 0.02);
}

// ---------------------------------------------------------------- filter.comb

/// The loop period in samples from an impulse response: the centroid of echo `k` (the
/// interpolator and the damping filter keep the first moment, so it is `k` times the loop
/// delay however they smear each echo), divided by `k`.
fn loop_period(y: &[f32], nominal: f32, k: usize) -> f32 {
    let (lo, hi) = (
        ((k as f32 - 0.5) * nominal) as usize,
        ((k as f32 + 0.5) * nominal) as usize,
    );
    let (m0, m1) = y[lo..hi]
        .iter()
        .enumerate()
        .fold((0.0f64, 0.0f64), |(a, b), (i, &v)| {
            (a + v as f64, b + (lo + i) as f64 * v as f64)
        });
    (m1 / m0) as f32 / k as f32
}

#[test]
fn comb_plays_in_tune() {
    let mut x = vec![0.0; 24000];
    x[0] = 1.0;
    for damp in [0.0, 60.0] {
        for pitch in [0.0f32, 7.0, 12.0, -12.0, 19.0, 36.0] {
            let mut d = Dut::new("filter.comb", &[("feedback", 90.0), ("damp", damp)], SR, 64);
            let p = vec![pitch; x.len()];
            let y = d.run(&[&x, &p], 64).remove(0);
            let want = SR / (261.63 * 2f32.powf(pitch / 12.0));
            let got = loop_period(&y, want, 8);
            let cents = 1200.0 * (want / got).log2();
            println!(
                "comb damp {damp} pitch {pitch:+}: period {got:.3} vs {want:.3} samples ({cents:+.3} cents)"
            );
            assert!(cents.abs() < 0.5, "{cents}");
        }
    }
}

#[test]
fn comb_negative_feedback_inverts_each_echo() {
    // The loop delay is the same; every trip round flips the sign, so the response repeats
    // every two periods: an octave lower.
    let mut x = vec![0.0; 24000];
    x[0] = 1.0;
    let mut d = Dut::new("filter.comb", &[("feedback", -90.0), ("damp", 0.0)], SR, 64);
    let y = d.run(&[&x], 64).remove(0);
    let period = SR / 261.63;
    let echo = |k: usize| -> f32 {
        let c = (k as f32 * period).round() as usize;
        y[c - 20..c + 20].iter().sum()
    };
    println!(
        "comb -90 %: echoes {:.3} {:.3} {:.3}",
        echo(1),
        echo(2),
        echo(3)
    );
    assert!(echo(1) < -0.1 && echo(2) > 0.1 && echo(3) < -0.1);
    let got = loop_period(&y.iter().map(|v| v.abs()).collect::<Vec<_>>(), period, 8);
    assert!((1200.0 * (period / got).log2()).abs() < 0.5);
}

// ---------------------------------------------------------------- filter.formant, svf notch

#[test]
fn formant_peaks_follow_the_vowel() {
    let x = noise(11, 8192 * 24);
    let peak_hz = |vowel: f32, shift: f32, lo: f32, hi: f32| {
        let y = Dut::new(
            "filter.formant",
            &[("vowel", vowel), ("shift_st", shift)],
            SR,
            64,
        )
        .run(&[&x], 64)
        .remove(0);
        let p = welch(&y[8192..]);
        let (a, b) = ((lo / bin_hz(1)) as usize, (hi / bin_hz(1)) as usize);
        let best = (a..b)
            .max_by(|&i, &j| p[i].partial_cmp(&p[j]).unwrap())
            .unwrap();
        bin_hz(best)
    };
    let a = peak_hz(0.0, 0.0, 200.0, 1500.0);
    let i = peak_hz(2.0, 0.0, 200.0, 1500.0);
    let u = peak_hz(4.0, 0.0, 200.0, 1500.0);
    let mid = peak_hz(1.0, 0.0, 200.0, 1500.0);
    let up = peak_hz(0.0, 12.0, 400.0, 3000.0);
    println!("formant F1: a {a:.0} (650), e {mid:.0} (400), i {i:.0} (290), u {u:.0} (350), a+12 st {up:.0} (1300)");
    assert!((a - 650.0).abs() < 60.0 && (mid - 400.0).abs() < 50.0);
    assert!((i - 290.0).abs() < 40.0 && (u - 350.0).abs() < 40.0);
    assert!((up - 1300.0).abs() < 120.0);
    // Halfway between a and e the first formant sits between theirs.
    let half = peak_hz(0.5, 0.0, 200.0, 1500.0);
    assert!(half > mid && half < a, "{half}");
}

#[test]
fn svf_notch_removes_the_cutoff() {
    for res in [0.0, 0.5, 0.9] {
        let set = [("cutoff_hz", 1000.0), ("resonance", res)];
        let at = |f: f32| {
            let x = sine(f, 0.25, SR, 24000);
            let y = Dut::new("filter.svf", &set, SR, 64)
                .run(&[&x], 64)
                .remove(3);
            db(rms(&y[12000..]) as f64 / rms(&x[12000..]) as f64)
        };
        let (n, lo, hi) = (at(1000.0), at(100.0), at(10000.0));
        println!("svf notch res {res}: {n:.1} dB at cutoff, {lo:.2} at 100 Hz, {hi:.2} at 10 kHz");
        assert!(n < -40.0 && lo.abs() < 1.0 && hi.abs() < 1.0);
    }
}

// ---------------------------------------------------------------- safety

/// Every param at its minimum, at its maximum, and alternating each block, on loud noise with a
/// DC step, at three sample rates and block sizes 1, 7, 64 and 4096: everything stays finite and
/// bounded.
#[test]
fn extreme_settings_stay_finite_and_bounded() {
    let mut x = noise(9, 48000);
    for v in &mut x {
        *v *= 2.0;
    }
    x[20000..30000].iter_mut().for_each(|v| *v += 1.5);
    for kind in NEW_KINDS {
        let info = registry::info_for(kind).unwrap();
        for sr in [44100.0f32, 96000.0, 192000.0] {
            for block in [1usize, 7, 64, 4096] {
                let len = if block == 1 { 6000 } else { 20000 };
                let mut ins: Vec<Vec<f32>> = (0..2).map(|_| x[..len].to_vec()).collect();
                if kind == "filter.comb" {
                    ins[1] = sine(3.0, 60.0, sr, len); // wild pitch: ±60 semitones
                }
                let refs: Vec<&[f32]> = ins.iter().map(|v| &v[..]).collect();
                for pattern in 0..3 {
                    let mut d = Dut::new(kind, &[], sr, block.max(64));
                    let mut y: Vec<Vec<f32>> = vec![Vec::new(); d.n_out];
                    let mut t = 0;
                    while t < len {
                        let n = (block * 8).min(len - t).max(1);
                        for (i, p) in info.params.iter().enumerate() {
                            d.params[i] = match (pattern, (t / n) % 2) {
                                (0, _) => p.min,
                                (1, _) => p.max,
                                (_, 0) => p.min,
                                _ => p.max,
                            };
                        }
                        let cut: Vec<&[f32]> = refs.iter().map(|r| &r[t..t + n]).collect();
                        for (o, r) in y.iter_mut().zip(d.run(&cut, block)) {
                            o.extend(r);
                        }
                        t += n;
                    }
                    for (port, ys) in y.iter().enumerate() {
                        let p = peak(ys);
                        assert!(
                            ys.iter().all(|v| v.is_finite()) && p < 200.0,
                            "{kind} sr {sr} block {block} pattern {pattern} out {port}: peak {p}"
                        );
                    }
                }
            }
        }
    }
}

/// A cable into every param (a per-sample buffer, random between min and max) at every rate.
#[test]
fn audio_rate_modulation_stays_finite() {
    let x = noise(4, 9600);
    for kind in NEW_KINDS {
        let info = registry::info_for(kind).unwrap();
        let mut m = registry::create(kind).unwrap();
        m.prepare(
            SR,
            64,
            &QualityConfig {
                tier: QualityTier::Live,
            },
        );
        let n_in = info
            .ports
            .iter()
            .filter(|p| p.direction == PortDirection::Input)
            .count();
        let n_out = info.ports.len() - n_in;
        let mods: Vec<Vec<f32>> = info
            .params
            .iter()
            .enumerate()
            .map(|(i, p)| {
                noise(100 + i as u32, 9600)
                    .iter()
                    .map(|v| p.min + (v * 0.5 + 0.5) * (p.max - p.min))
                    .collect()
            })
            .collect();
        for t in (0..9600).step_by(64) {
            let sig: Vec<Signal> = (0..n_in)
                .map(|k| {
                    if k < 2 {
                        Signal::Buffer(&x[t..t + 64])
                    } else {
                        Signal::Scalar(0.0)
                    }
                })
                .collect();
            let ps: Vec<Signal> = mods.iter().map(|b| Signal::Buffer(&b[t..t + 64])).collect();
            let mut bufs = vec![vec![0f32; 64]; n_out];
            let mut refs: Vec<&mut [f32]> = bufs.iter_mut().map(|b| &mut b[..]).collect();
            let mut io = ProcessIo::new(&sig, &mut refs, &ps, 64);
            m.process(&mut io);
            for b in &bufs {
                assert!(b.iter().all(|v| v.is_finite() && v.abs() < 200.0), "{kind}");
            }
        }
    }
}

// ---------------------------------------------------------------- real-time rules

#[test]
fn the_audio_path_allocates_nothing() {
    let x = noise(6, 64 * 40);
    for kind in NEW_KINDS {
        let info = registry::info_for(kind).unwrap();
        let mut d = Dut::new(kind, &[], SR, 64);
        let n_in = d.n_in;
        let sig: Vec<Vec<Signal>> = (0..40)
            .map(|b| {
                (0..n_in)
                    .map(|k| {
                        if k < 2 {
                            Signal::Buffer(&x[b * 64..b * 64 + 64])
                        } else {
                            Signal::Scalar(0.0)
                        }
                    })
                    .collect()
            })
            .collect();
        let mut bufs: Vec<Vec<Vec<f32>>> = (0..40).map(|_| vec![vec![0f32; 64]; d.n_out]).collect();
        let mut refs: Vec<Vec<&mut [f32]>> = bufs
            .iter_mut()
            .map(|b| b.iter_mut().map(|v| &mut v[..]).collect())
            .collect();
        let ps: Vec<Vec<Signal>> = (0..40)
            .map(|b| {
                info.params
                    .iter()
                    .map(|p| Signal::Scalar(if b % 2 == 0 { p.min } else { p.max }))
                    .collect()
            })
            .collect();
        assert_no_alloc(|| {
            for (b, r) in refs.iter_mut().enumerate() {
                let mut io = ProcessIo::new(&sig[b], r, &ps[b], 64);
                d.m.process(&mut io);
            }
        });
    }
}

// ---------------------------------------------------------------- state carry

#[test]
fn live_edit_carries_state_across() {
    // A fresh instance that took over from a running one continues its output exactly.
    let x = noise(8, 6400);
    for kind in NEW_KINDS {
        let mut a = Dut::new(kind, &[], SR, 64);
        let _ = a.run_mono(&x[..3200], 64);
        let mut b = Dut::new(kind, &[], SR, 64);
        b.m.carry_from(&*a.m);
        let ya = a.run_mono(&x[3200..], 64);
        let yb = b.run_mono(&x[3200..], 64);
        assert_eq!(ya, yb, "{kind}");
    }
}

/// CPU per module at 48 kHz, block 256, default params, noise in. Not asserted; run with
/// `cargo test --release -p kabl-modules --test effects cpu_per_module -- --ignored --nocapture`.
#[test]
#[ignore]
fn cpu_per_module() {
    let x = noise(7, 256);
    for kind in NEW_KINDS
        .into_iter()
        .chain(["filter.svf", "filter.ladder", "chorus"])
    {
        let mut d = Dut::new(kind, &[], 48000.0, 256);
        let sig: Vec<Signal> = (0..d.n_in).map(|_| Signal::Buffer(&x)).collect();
        let ps: Vec<Signal> = d.params.iter().map(|&v| Signal::Scalar(v)).collect();
        let mut bufs = vec![vec![0f32; 256]; d.n_out];
        let mut run = |n: usize| {
            let t = std::time::Instant::now();
            for _ in 0..n {
                let mut refs: Vec<&mut [f32]> = bufs.iter_mut().map(|b| &mut b[..]).collect();
                let mut io = ProcessIo::new(&sig, &mut refs, &ps, 256);
                d.m.process(&mut io);
            }
            t.elapsed().as_secs_f64() / (n * 256) as f64 * 1e9
        };
        run(2000);
        let ns = (0..5).map(|_| run(4000)).fold(f64::MAX, f64::min);
        println!(
            "{kind}: {ns:.1} ns/sample, {:.3} % of one core at 48 kHz",
            ns * 48000.0 / 1e7
        );
    }
}
