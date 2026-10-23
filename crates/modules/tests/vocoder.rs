//! `vocoder` and `audio.in`: the modulator's spectrum lands on the carrier, formant shift moves
//! it, the unvoiced path adds hiss only where the voice has it, neutral settings, extreme
//! settings and hostile input at 44.1 to 192 kHz, block-size independence, no allocation.
//! `-- --nocapture` prints the numbers.

#[cfg(debug_assertions)]
use assert_no_alloc::{assert_no_alloc, AllocDisabler};
#[cfg(not(debug_assertions))]
fn assert_no_alloc<T>(f: impl FnOnce() -> T) -> T {
    f()
}
use kabl_modules::builtins::AudioIn;
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{registry, Module, ProcessIo, Signal};
use std::f32::consts::TAU;

#[cfg(debug_assertions)]
#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const LIVE: QualityConfig = QualityConfig {
    tier: QualityTier::Live,
};

fn vocoder(set: &[(&str, f32)], sr: f32) -> (Box<dyn Module>, Vec<f32>) {
    let mut m = registry::create("vocoder").unwrap();
    m.prepare(sr, 4096, &LIVE);
    let mut p: Vec<f32> = m.info().params.iter().map(|p| p.default).collect();
    for &(k, v) in set {
        p[m.info().params.iter().position(|q| q.name == k).unwrap()] = v;
    }
    (m, p)
}

/// Runs `carrier` (both channels) and `modulator` through in blocks of `block`.
fn run(
    m: &mut dyn Module,
    p: &[f32],
    carrier: &[f32],
    modulator: Option<&[f32]>,
    block: usize,
) -> [Vec<f32>; 2] {
    let ps: Vec<Signal> = p.iter().map(|&v| Signal::Scalar(v)).collect();
    let mut out = [Vec::new(), Vec::new()];
    let mut t = 0;
    while t < carrier.len() {
        let n = block.min(carrier.len() - t);
        let c = Signal::Buffer(&carrier[t..t + n]);
        let md = modulator.map_or(Signal::Scalar(0.0), |b| Signal::Buffer(&b[t..t + n]));
        let ins = [c, c, md];
        let (mut l, mut r) = (vec![0.0; n], vec![0.0; n]);
        {
            let mut outs: [&mut [f32]; 2] = [&mut l, &mut r];
            let mut io = ProcessIo::new(&ins, &mut outs, &ps, n);
            m.process(&mut io);
        }
        out[0].extend(l);
        out[1].extend(r);
        t += n;
    }
    out
}

fn saw(hz: f32, sr: f32, n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| 2.0 * ((i as f32 * hz / sr) % 1.0) - 1.0)
        .collect()
}

fn sine(hz: f32, sr: f32, n: usize, amp: f32) -> Vec<f32> {
    (0..n)
        .map(|i| amp * (TAU * hz * i as f32 / sr).sin())
        .collect()
}

fn noise(seed: u32, n: usize, amp: f32) -> Vec<f32> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            amp * ((s as f32 / u32::MAX as f32) * 2.0 - 1.0)
        })
        .collect()
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
}

/// Power at `hz` (Goertzel), normalised to a sine's amplitude squared.
fn power(x: &[f32], hz: f32, sr: f32) -> f32 {
    let w = TAU * hz / sr;
    let (mut re, mut im) = (0.0f64, 0.0f64);
    for (i, v) in x.iter().enumerate() {
        re += *v as f64 * (w as f64 * i as f64).cos();
        im += *v as f64 * (w as f64 * i as f64).sin();
    }
    let a = 2.0 * (re * re + im * im).sqrt() / x.len() as f64;
    (a * a) as f32
}

fn db(a: f32, b: f32) -> f32 {
    10.0 * (a / b.max(1e-30)).log10()
}

const SR: f32 = 48000.0;
const N: usize = 48000;
/// Skip the settling at the start.
const SKIP: usize = 12000;

#[test]
fn a_tone_on_mod_picks_that_part_of_the_carrier() {
    // Saw at 100 Hz: a harmonic every 100 Hz. A 1 kHz voice should leave harmonic 10 well
    // above harmonic 30.
    let (mut m, p) = vocoder(&[("hf", 0.0), ("bands", 32.0)], SR);
    let out = run(
        &mut *m,
        &p,
        &saw(100.0, SR, N),
        Some(&sine(1000.0, SR, N, 0.3)),
        64,
    );
    let (lo, hi) = (
        power(&out[0][SKIP..], 1000.0, SR),
        power(&out[0][SKIP..], 3000.0, SR),
    );
    println!(
        "1 kHz voice: harmonic 10 over harmonic 30 = {:.1} dB",
        db(lo, hi)
    );
    assert!(db(lo, hi) > 20.0);
    // The same tone an octave up moves the peak.
    let out = run(
        &mut *vocoder(&[("hf", 0.0), ("bands", 32.0)], SR).0,
        &p,
        &saw(100.0, SR, N),
        Some(&sine(2000.0, SR, N, 0.3)),
        64,
    );
    let (a, b) = (
        power(&out[0][SKIP..], 1000.0, SR),
        power(&out[0][SKIP..], 2000.0, SR),
    );
    assert!(db(b, a) > 15.0, "{:.1}", db(b, a));
}

#[test]
fn shift_moves_the_voice() {
    let md = sine(1000.0, SR, N, 0.3);
    let car = saw(100.0, SR, N);
    let (mut m, p) = vocoder(&[("hf", 0.0), ("bands", 32.0), ("shift_st", 12.0)], SR);
    let out = run(&mut *m, &p, &car, Some(&md), 64);
    let (at1k, at2k) = (
        power(&out[0][SKIP..], 1000.0, SR),
        power(&out[0][SKIP..], 2000.0, SR),
    );
    println!("+12 st: 2 kHz over 1 kHz = {:.1} dB", db(at2k, at1k));
    assert!(db(at2k, at1k) > 15.0);
}

#[test]
fn output_follows_the_voice_level() {
    // The vocoder multiplies: a voice 6 dB louder gives 6 dB more out. Printed for the
    // default drive: saw carrier at 0.3 peak, a noisy voice at -20 dBFS RMS.
    let car: Vec<f32> = saw(110.0, SR, N).iter().map(|v| v * 0.3).collect();
    let level = |amp: f32, bands: f32| {
        let (mut m, p) = vocoder(&[("hf", 0.0), ("bands", bands)], SR);
        rms(&run(&mut *m, &p, &car, Some(&noise(7, N, amp)), 64)[0][SKIP..])
    };
    let spread = [8.0, 16.0, 32.0].map(|b| 20.0 * level(0.17, b).log10());
    println!("level spread over band counts: {spread:?}");
    assert!(spread[0] - spread[2] < 5.0 && spread[2] - spread[0] < 5.0);
    for bands in [8.0, 16.0, 32.0] {
        let (a, b) = (level(0.1, bands), level(0.2, bands));
        println!(
            "{bands} bands: carrier {:.1} dBFS, voice -20 dBFS rms -> {:.1} dBFS",
            20.0 * rms(&car).log10(),
            20.0 * level(0.17, bands).log10()
        );
        assert!(
            (db(b * b, a * a) - 6.0).abs() < 0.5,
            "{bands} bands: {:.2} dB",
            db(b * b, a * a)
        );
    }
}

#[test]
fn no_voice_no_sound_and_mix_zero_is_the_carrier() {
    let car = saw(110.0, SR, 4800);
    let (mut m, p) = vocoder(&[], SR);
    let out = run(&mut *m, &p, &car, None, 64);
    assert!(rms(&out[0]) < 1e-6);
    let (mut m, p) = vocoder(&[("mix", 0.0)], SR);
    let out = run(&mut *m, &p, &car, Some(&noise(3, 4800, 0.5)), 64);
    assert_eq!(out[0], car);
}

#[test]
fn unvoiced_path_adds_hiss_only_with_a_hissing_voice() {
    // A 200 Hz sine carrier has nothing at 8 kHz: any energy there is the noise path.
    let car = sine(200.0, SR, N, 0.3);
    let hiss = noise(5, N, 0.3);
    let hum = sine(300.0, SR, N, 0.3);
    let at8k = |md: &[f32], hf: f32| {
        let (mut m, p) = vocoder(&[("hf", hf), ("bands", 8.0)], SR);
        let o = run(&mut *m, &p, &car, Some(md), 64);
        [7000.0, 8123.0, 9500.0, 11000.0]
            .map(|f| power(&o[0][SKIP..], f, SR))
            .iter()
            .sum::<f32>()
    };
    let (off, on, quiet) = (at8k(&hiss, 0.0), at8k(&hiss, 1.0), at8k(&hum, 1.0));
    println!("energy above 7 kHz, hiss voice: hf 0 {off:.2e}, hf 1 {on:.2e}; hum voice, hf 1 {quiet:.2e}");
    assert!(on > 3.0 * off);
    assert!(quiet < 0.01 * on);
}

#[test]
fn a_hissing_voice_with_no_synth_under_it_is_silent() {
    let (mut m, p) = vocoder(&[("hf", 1.0)], SR);
    let out = run(&mut *m, &p, &vec![0.0; N], Some(&noise(5, N, 0.3)), 64);
    assert!(rms(&out[0]) < 1e-6 && rms(&out[1]) < 1e-6);
}

#[test]
fn shifted_blocks_and_one_sample_blocks_agree() {
    let car = saw(110.0, SR, 8000);
    let md = noise(9, 8000, 0.4);
    let reference = {
        let (mut m, p) = vocoder(&[("shift_st", 3.0)], SR);
        run(&mut *m, &p, &car, Some(&md), 64)
    };
    for block in [1, 7, 33] {
        let (mut m, p) = vocoder(&[("shift_st", 3.0)], SR);
        assert_eq!(
            run(&mut *m, &p, &car, Some(&md), block),
            reference,
            "block {block}"
        );
    }
}

#[test]
fn extreme_settings_and_hostile_input_stay_finite() {
    let extremes: [&[(&str, f32)]; 2] = [
        &[
            ("bands", 8.0),
            ("low_hz", 400.0),
            ("high_hz", 3000.0),
            ("shift_st", -12.0),
            ("attack_ms", 0.5),
            ("release_ms", 5.0),
            ("drive_db", 0.0),
            ("spacing", 0.0),
        ],
        &[
            ("bands", 32.0),
            ("low_hz", 60.0),
            ("high_hz", 16000.0),
            ("shift_st", 12.0),
            ("attack_ms", 50.0),
            ("release_ms", 500.0),
            ("drive_db", 36.0),
            ("spacing", 1.0),
            ("hf", 1.0),
        ],
    ];
    for sr in [44100.0, 48000.0, 96000.0, 192000.0] {
        for set in extremes {
            for block in [1, 64, 1000] {
                let n = 9600;
                let mut car = saw(55.0, sr, n);
                let mut md = noise(11, n, 1.0);
                car[10] = f32::NAN;
                car[20] = f32::INFINITY;
                md[30] = f32::NEG_INFINITY;
                md[40] = f32::NAN;
                md[50] = 1e30;
                let (mut m, p) = vocoder(set, sr);
                let out = run(&mut *m, &p, &car, Some(&md), block);
                for ch in &out {
                    assert!(ch.iter().all(|v| v.is_finite()), "{sr} Hz block {block}");
                    assert!(ch.iter().all(|v| v.abs() < 1e5), "{sr} Hz block {block}");
                }
                // Recovers: once the voice stops, the envelopes (500 ms at most) die away.
                if block == 64 {
                    let n = (6.0 * sr) as usize;
                    let tail = run(&mut *m, &p, &saw(55.0, sr, n), Some(&vec![0.0; n]), block);
                    assert!(rms(&tail[0][n - 4800..]) < 1e-3, "{sr} Hz");
                }
            }
        }
    }
}

#[test]
fn processing_does_not_allocate() {
    let car = saw(110.0, SR, 64);
    let md = noise(2, 64, 0.4);
    let (mut m, p) = vocoder(&[("hf", 0.7)], SR);
    run(&mut *m, &p, &car, Some(&md), 64);
    assert_no_alloc(|| {
        for _ in 0..50 {
            run_noalloc(&mut *m, &p, &car, &md);
        }
    });
}

fn run_noalloc(m: &mut dyn Module, p: &[f32], car: &[f32], md: &[f32]) {
    let ps: [Signal; 10] = std::array::from_fn(|i| Signal::Scalar(p[i]));
    let ins = [Signal::Buffer(car), Signal::Buffer(car), Signal::Buffer(md)];
    let (mut l, mut r) = ([0.0f32; 64], [0.0f32; 64]);
    let mut outs: [&mut [f32]; 2] = [&mut l, &mut r];
    let mut io = ProcessIo::new(&ins, &mut outs, &ps, 64);
    m.process(&mut io);
}

#[test]
#[ignore = "prints the cost per 64-sample block; run with --release"]
fn cpu_at_32_bands() {
    for sr in [48000.0, 96000.0, 192000.0] {
        let (mut m, p) = vocoder(&[("bands", 32.0), ("hf", 0.5), ("shift_st", 3.0)], sr);
        let car = saw(110.0, sr, 64);
        let md = noise(2, 64, 0.4);
        for _ in 0..200 {
            run_noalloc(&mut *m, &p, &car, &md);
        }
        let blocks = 20000;
        let t = std::time::Instant::now();
        for _ in 0..blocks {
            run_noalloc(&mut *m, &p, &car, &md);
        }
        let us = t.elapsed().as_secs_f64() * 1e6 / blocks as f64;
        let budget = 64.0 / sr as f64 * 1e6;
        println!(
            "CPU {sr} Hz: {us:.1} us per 64-sample block, {:.1} % of the {budget:.0} us budget",
            100.0 * us / budget
        );
    }
}

#[test]
fn a_voiced_voice_with_a_little_top_end_stays_clean() {
    // 200 Hz at 0.1 plus 6 kHz at 0.01: a vowel with a little energy above 5 kHz.
    let voice: Vec<f32> = sine(200.0, SR, N, 0.1)
        .iter()
        .zip(sine(6000.0, SR, N, 0.01))
        .map(|(a, b)| a + b)
        .collect();
    let car = saw(110.0, SR, N);
    let go = |hf: f32| {
        let (mut m, p) = vocoder(&[("hf", hf)], SR);
        run(&mut *m, &p, &car, Some(&voice), 64)[0][SKIP..].to_vec()
    };
    let (off, on) = (go(0.0), go(1.0));
    let diff: Vec<f32> = off.iter().zip(&on).map(|(a, b)| a - b).collect();
    println!("hf 1 over hf 0: {:.1} dB", db(rms(&diff), rms(&off)));
    assert!(rms(&diff) < 0.03 * rms(&off), "voiced speech gets hiss");
    // A hissing voice does open it.
    let hiss = noise(3, N, 0.1);
    let (mut m, p) = vocoder(&[("hf", 1.0)], SR);
    let with = run(&mut *m, &p, &car, Some(&hiss), 64)[0][SKIP..].to_vec();
    let (mut m, p) = vocoder(&[("hf", 0.0)], SR);
    let without = run(&mut *m, &p, &car, Some(&hiss), 64)[0][SKIP..].to_vec();
    assert!(rms(&with) > 1.5 * rms(&without), "a hiss gets none");
}

/// A steady broadband voice (60 tones, 150 Hz to 8 kHz, in every band), so any level change is
/// the module's.
fn sweep_voice(n: usize) -> Vec<f32> {
    (0..60)
        .map(|k| {
            sine(
                150.0 * (8000.0f32 / 150.0).powf(k as f32 / 59.0),
                SR,
                n,
                0.03,
            )
        })
        .fold(vec![0.0; n], |a, s| {
            a.iter().zip(&s).map(|(x, y)| x + y).collect()
        })
}

/// Sweeps one parameter there and back over `secs` while a saw is held under a noise voice, in
/// 64-sample blocks. Returns the left output.
fn sweep(name: &str, secs: f32) -> Vec<f32> {
    let (mut m, mut p) = vocoder(&[], SR);
    let idx = m.info().params.iter().position(|q| q.name == name).unwrap();
    let (lo, hi) = (m.info().params[idx].min, m.info().params[idx].max);
    let n = (secs * SR) as usize;
    let car = saw(110.0, SR, n);
    let voice = sweep_voice(n);
    let mut out = Vec::with_capacity(n);
    let mut t = 0;
    while t < n {
        let len = 64.min(n - t);
        // Triangle, 2 s there and back: crosses every integer band count.
        let u = (t as f32 / SR / 2.0).fract();
        let tri = if u < 0.5 { 2.0 * u } else { 2.0 - 2.0 * u };
        p[idx] = lo + (hi - lo) * tri;
        let ps: Vec<Signal> = p.iter().map(|&v| Signal::Scalar(v)).collect();
        let c = Signal::Buffer(&car[t..t + len]);
        let ins = [c, c, Signal::Buffer(&voice[t..t + len])];
        let (mut l, mut r) = (vec![0.0; len], vec![0.0; len]);
        {
            let mut outs: [&mut [f32]; 2] = [&mut l, &mut r];
            let mut io = ProcessIo::new(&ins, &mut outs, &ps, len);
            m.process(&mut io);
        }
        out.extend(l);
        t += len;
    }
    out
}

#[test]
fn moving_any_parameter_while_sounding_does_not_drop_out_or_click() {
    let names: Vec<&str> = registry::create("vocoder")
        .unwrap()
        .info()
        .params
        .iter()
        .map(|p| p.name)
        .collect();
    // The same inputs with every parameter held at its default.
    let steady = {
        let (mut m, p) = vocoder(&[], SR);
        let car = saw(110.0, SR, 8 * 48000);
        let voice = sweep_voice(car.len());
        max_step(&run(&mut *m, &p, &car, Some(&voice), 64)[0][SKIP..])
    };
    for name in names {
        let all = sweep(name, 8.0);
        let out = &all[SKIP..];
        // Dropout: a 512-sample block (longer than a 110 Hz period) far below the mean block
        // level of the last 0.5 s.
        let blocks: Vec<f32> = out.chunks(512).map(rms).collect();
        let win = 47;
        let mut drop = f32::MAX;
        for i in win..blocks.len() {
            let level = blocks[i - win..i].iter().sum::<f32>() / win as f32;
            drop = drop.min(blocks[i] / level.max(1e-9));
        }
        // Click: largest sample-to-sample step against the block's own RMS, not above the
        // steadiest run's (the held saw has steps of its own).
        let step = max_step(out);
        println!("{name}: lowest block / level {drop:.2}, largest step / rms {step:.1} (steady {steady:.1})");
        // Sweeping these to their quiet ends (no unvoiced noise, no drive, all dry) is a level
        // change, not a dropout.
        if !["hf", "drive_db", "mix"].contains(&name) {
            assert!(drop > 0.25, "{name}: dropout, block at {drop} of the level");
        }
        assert!(
            step < steady * 1.5,
            "{name}: click, step {step} x block rms, steady run {steady}"
        );
    }
}

fn max_step(x: &[f32]) -> f32 {
    let mut step = 0.0f32;
    for ch in x.chunks(64) {
        let r = rms(ch).max(1e-6);
        for w in ch.windows(2) {
            step = step.max((w[1] - w[0]).abs() / r);
        }
    }
    step
}

#[test]
fn audio_in_passes_what_it_is_fed_and_sanitises_the_rest() {
    let mut a = AudioIn::new();
    a.prepare(SR, 64, &LIVE);
    let mut left: Vec<f32> = (0..64).map(|i| i as f32 / 64.0 - 0.5).collect();
    left[3] = f32::NAN;
    left[4] = f32::INFINITY;
    left[5] = 1e9;
    a.feed(&left, &[]);
    let ps = [Signal::Scalar(0.0)];
    let (mut l, mut r) = ([0.0f32; 64], [0.0f32; 64]);
    {
        let mut outs: [&mut [f32]; 2] = [&mut l, &mut r];
        let mut io = ProcessIo::new(&[], &mut outs, &ps, 64);
        a.process(&mut io);
    }
    assert_eq!(l[10], left[10]);
    assert_eq!(r[10], left[10], "mono plays on both sides");
    assert_eq!(l[3], 0.0);
    assert_eq!(l[4], 0.0);
    assert_eq!(l[5], 16.0);
    // Nothing fed after a reset: silence.
    a.reset();
    let mut outs: [&mut [f32]; 2] = [&mut l, &mut r];
    let mut io = ProcessIo::new(&[], &mut outs, &ps, 64);
    a.process(&mut io);
    assert!(l.iter().chain(&r).all(|&v| v == 0.0));
}
