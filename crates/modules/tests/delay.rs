//! `delay` DSP: echo timing, decay, stereo, dry/wet, clock sync, block and rate independence.

use kabl_modules::builtins::{Delay, DelayLock};
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{Module, ProcessIo, Signal};

const SR: f32 = 48000.0;

/// time_ms, sync, feedback %, mix %, tone Hz, mode.
#[derive(Clone, Copy)]
struct P([f32; 6]);

impl P {
    fn free(ms: f32) -> Self {
        P([ms, 0.0, 0.0, 100.0, 16000.0, 0.0])
    }
    fn with(mut self, i: usize, v: f32) -> Self {
        self.0[i] = v;
        self
    }
}

fn delay(sr: f32, block: usize) -> Delay {
    let mut d = Delay::new();
    d.prepare(
        sr,
        block,
        &QualityConfig {
            tier: QualityTier::Live,
        },
    );
    d
}

/// Runs `input`/`clock` through `d` in blocks of `block`, with params from `params(sample)`
/// read at each block start.
fn run_with(
    d: &mut Delay,
    input: &[f32],
    clock: &[f32],
    block: usize,
    params: impl Fn(usize) -> P,
) -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut r) = (vec![0.0; input.len()], vec![0.0; input.len()]);
    let mut i = 0;
    while i < input.len() {
        let n = block.min(input.len() - i);
        let ins = [
            Signal::Buffer(&input[i..i + n]),
            Signal::Buffer(&clock[i..i + n]),
        ];
        let ps = params(i).0.map(Signal::Scalar);
        let (mut ol, mut or) = (vec![0.0; block], vec![0.0; block]);
        let mut outs = [&mut ol[..], &mut or[..]];
        d.process(&mut ProcessIo::new(&ins, &mut outs, &ps, n));
        l[i..i + n].copy_from_slice(&ol[..n]);
        r[i..i + n].copy_from_slice(&or[..n]);
        i += n;
    }
    (l, r)
}

fn run(p: P, input: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let clock = vec![0.0; input.len()];
    run_with(&mut delay(SR, 64), input, &clock, 64, |_| p)
}

fn impulse(len: usize) -> Vec<f32> {
    let mut v = vec![0.0; len];
    v[0] = 1.0;
    v
}

/// A 50 % duty clock, one rising edge every `period` samples from `start`, for `count` pulses.
fn pulses(out: &mut [f32], start: usize, period: usize, count: usize) {
    for k in 0..count {
        let s = start + k * period;
        for x in out.iter_mut().skip(s).take(period / 2) {
            *x = 1.0;
        }
    }
}

fn argmax(v: &[f32]) -> usize {
    (0..v.len())
        .max_by(|&a, &b| v[a].abs().total_cmp(&v[b].abs()))
        .unwrap()
}

#[test]
fn impulse_echo_lands_on_the_delay_time() {
    let (l, r) = run(P::free(100.0), &impulse(12000));
    assert_eq!(argmax(&l), 4800);
    assert_eq!(argmax(&r), 4800);
    // Mono: both lines' sum at 1/√2 per side, equal power with a one-sided ping-pong echo.
    assert!(
        (l[4800] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6,
        "{}",
        l[4800]
    );
    assert_eq!(l, r);
    assert!(
        l[..4800].iter().all(|&x| x == 0.0),
        "fully wet: no dry, nothing early"
    );
}

#[test]
fn echo_timing_holds_at_other_sample_rates() {
    for sr in [44100.0, 96000.0] {
        let clock = vec![0.0; 20000];
        let (l, _) = run_with(&mut delay(sr, 64), &impulse(20000), &clock, 64, |_| {
            P::free(100.0)
        });
        assert_eq!(argmax(&l), (0.1 * sr) as usize, "at {sr} Hz");
    }
}

/// Sum of the output over each echo's window: the tone low-pass has unity DC gain, so the
/// k-th echo's sum is feedback^(k-1) however much the filter smears it.
fn echo_sums(out: &[f32], d: usize, n: usize) -> Vec<f32> {
    (1..=n)
        .map(|k| out[k * d - d / 2..k * d + d / 2].iter().sum())
        .collect()
}

#[test]
fn repeats_decay_by_the_feedback_and_darken() {
    let p = P::free(50.0).with(2, 50.0).with(4, 2000.0).with(5, 1.0);
    let (l, r) = run(p, &impulse(2400 * 7));
    let both: Vec<f32> = l.iter().zip(&r).map(|(a, b)| a + b).collect();
    let sums = echo_sums(&both, 2400, 6);
    for (k, s) in sums.iter().enumerate() {
        let want = 0.5f32.powi(k as i32);
        assert!(
            (s - want).abs() < 1e-3,
            "echo {} sum {s}, want {want}",
            k + 1
        );
    }
    // Darker: the first echo is the bare impulse, later ones are smeared (lower peak / sum).
    let peak = |k: usize| {
        both[k * 2400 - 1200..k * 2400 + 1200]
            .iter()
            .fold(0.0f32, |m, x| m.max(x.abs()))
    };
    assert!((peak(1) - 1.0).abs() < 1e-6);
    assert!(peak(2) / sums[1] < 0.5, "second echo is low-passed");
    assert!(
        peak(3) / sums[2] < peak(2) / sums[1],
        "each repeat darker than the last"
    );
}

#[test]
fn ping_pong_alternates_left_and_right() {
    let p = P::free(50.0).with(2, 60.0).with(5, 1.0);
    let (l, r) = run(p, &impulse(2400 * 5));
    let e = |v: &[f32], k: usize| {
        v[k * 2400 - 1200..k * 2400 + 1200]
            .iter()
            .map(|x| x.abs())
            .sum::<f32>()
    };
    for k in 1..=4 {
        let (on, off) = if k % 2 == 1 { (&l, &r) } else { (&r, &l) };
        assert!(e(on, k) > 0.1, "echo {k} present");
        assert_eq!(e(off, k), 0.0, "echo {k} only on one side");
    }
}

#[test]
fn mix_endpoints_are_exactly_dry_and_exactly_wet() {
    let input: Vec<f32> = (0..9600).map(|i| (i as f32 * 0.05).sin()).collect();
    let (l, r) = run(P::free(100.0).with(2, 50.0).with(3, 0.0), &input);
    assert_eq!(l, input);
    assert_eq!(r, input);
    let (l, _) = run(P::free(100.0).with(2, 50.0).with(3, 100.0), &input);
    assert!(l[..4800].iter().all(|&x| x == 0.0), "no dry at 100 %");
}

#[test]
fn block_size_does_not_change_the_output() {
    let len = 30000;
    let input: Vec<f32> = (0..len)
        .map(|i| ((i * 7919) % 101) as f32 / 50.0 - 1.0)
        .collect();
    let mut clock = vec![0.0; len];
    pulses(&mut clock, 100, 3000, 9);
    let p = |_| P([150.0, 3.0, 70.0, 50.0, 3000.0, 1.0]);
    let (l64, r64) = run_with(&mut delay(SR, 64), &input, &clock, 64, p);
    for b in [1, 17] {
        let (l, r) = run_with(&mut delay(SR, 64), &input, &clock, b, p);
        assert_eq!(l, l64, "block {b}");
        assert_eq!(r, r64, "block {b}");
    }
}

#[test]
fn sync_locks_after_two_agreeing_intervals() {
    let mut d = delay(SR, 64);
    let len = 6000 * 3 + 10;
    let mut clock = vec![0.0; len];
    pulses(&mut clock, 0, 6000, 3);
    // 1/8 = 2 pulses.
    let p = |_| P::free(300.0).with(1, 2.0);
    run_with(&mut d, &vec![0.0; 6001], &clock[..6001], 64, p);
    assert_eq!(
        d.status(),
        (DelayLock::Unlocked, 300.0),
        "one interval: still free time"
    );
    run_with(&mut d, &vec![0.0; len - 6001], &clock[6001..], 64, p);
    assert_eq!(d.status(), (DelayLock::Locked, 250.0), "120 bpm eighth");
    for (sync, ms) in [(1.0, 125.0), (3.0, 375.0), (4.0, 500.0)] {
        let p = move |_| P::free(300.0).with(1, sync);
        run_with(&mut d, &[0.0], &[0.0], 64, p);
        assert_eq!(d.status(), (DelayLock::Locked, ms));
    }
    run_with(&mut d, &[0.0], &[0.0], 64, |_| P::free(300.0));
    assert_eq!(d.status(), (DelayLock::Free, 300.0));
}

#[test]
fn synced_echo_lands_on_the_clock() {
    // 120 bpm pulses; after lock and glide, an impulse echoes one dotted eighth (18000 samples) later.
    // The glide from the free 100 ms has settled well before sample 150000.
    let len = 200000;
    let mut clock = vec![0.0; len];
    pulses(&mut clock, 0, 6000, 34);
    let mut input = vec![0.0; len];
    input[150000] = 1.0;
    let (l, _) = run_with(&mut delay(SR, 64), &input, &clock, 64, |_| {
        P::free(100.0).with(1, 3.0)
    });
    assert_eq!(argmax(&l), 168000);
}

#[test]
fn stop_holds_the_time_and_restart_does_not_read_the_gap_as_tempo() {
    let mut d = delay(SR, 64);
    let p = |_| P::free(300.0).with(1, 2.0).with(2, 50.0);
    let mut clock = vec![0.0; 24000];
    pulses(&mut clock, 0, 6000, 4);
    let mut input = vec![0.0; 24000];
    input[20000] = 1.0;
    run_with(&mut d, &input, &clock, 64, p);
    assert_eq!(d.status(), (DelayLock::Locked, 250.0));

    // Stopped for 2 s: the time is held and the echoes keep decaying.
    let (l, _) = run_with(&mut d, &vec![0.0; 96000], &vec![0.0; 96000], 64, p);
    assert_eq!(d.status(), (DelayLock::Held, 250.0));
    assert!(
        l.iter().any(|x| x.abs() > 0.1),
        "echo tail continues while stopped"
    );

    // Run again at 150 bpm (4800). The first interval includes the stop; the second is new but
    // has nothing to agree with; the third agrees.
    let mut clock = vec![0.0; 4800 * 4];
    pulses(&mut clock, 0, 4800, 4);
    run_with(&mut d, &vec![0.0; 4801], &clock[..4801], 64, p);
    assert_eq!(
        d.status(),
        (DelayLock::Locked, 250.0),
        "gap not read as tempo"
    );
    run_with(
        &mut d,
        &vec![0.0; clock.len() - 4801],
        &clock[4801..],
        64,
        p,
    );
    assert_eq!(
        d.status(),
        (DelayLock::Locked, 200.0),
        "relocked on the third pulse"
    );

    // A restart while running: one low sample, then a new pulse. The one-sample interval and the
    // one after it are ignored.
    let mut clock = vec![0.0; 4800 * 5];
    clock[..1000].fill(1.0);
    pulses(&mut clock, 1001, 4800, 4);
    run_with(&mut d, &vec![0.0; clock.len()], &clock, 64, p);
    assert_eq!(d.status(), (DelayLock::Locked, 200.0));
}

#[test]
fn time_changes_glide_without_clicks() {
    // A 220 Hz sine fully wet with no feedback; the time jumps 300 → 700 ms and back, and sync
    // switches on and off. The output's sample-to-sample step stays near the sine's own.
    let len = 48000 * 4;
    let input: Vec<f32> = (0..len)
        .map(|i| (std::f32::consts::TAU * 220.0 * i as f32 / SR).sin())
        .collect();
    let mut clock = vec![0.0; len];
    pulses(&mut clock, 0, 6000, 32);
    let (l, _) = run_with(&mut delay(SR, 64), &input, &clock, 64, |i| match i {
        0..48000 => P::free(300.0),
        48000..96000 => P::free(700.0),
        96000..144000 => P::free(700.0).with(1, 4.0),
        _ => P::free(300.0),
    });
    let sine_step = std::f32::consts::TAU * 220.0 / SR;
    let worst = l[1..]
        .iter()
        .zip(&l)
        .skip(20000)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max);
    // At most 1.5× speed (+7 st) while gliding.
    assert!(
        worst < sine_step * 1.6,
        "worst step {worst}, sine step {sine_step}"
    );
}

#[test]
fn mode_switch_mid_tail_is_smooth() {
    let len = 48000;
    let input: Vec<f32> = (0..len)
        .map(|i| {
            if i < 4800 {
                (std::f32::consts::TAU * 220.0 * i as f32 / SR).sin()
            } else {
                0.0
            }
        })
        .collect();
    let clock = vec![0.0; len];
    let (l, r) = run_with(&mut delay(SR, 64), &input, &clock, 64, |i| {
        P::free(100.0)
            .with(2, 80.0)
            .with(5, if i < 20000 { 0.0 } else { 1.0 })
    });
    let sine_step = std::f32::consts::TAU * 220.0 / SR;
    for v in [&l, &r] {
        let worst = v[1..]
            .iter()
            .zip(v.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f32::max);
        assert!(worst < sine_step * 1.5, "worst {worst}");
    }
}

#[test]
fn maximum_feedback_stays_finite_and_bounded() {
    let len = 48000 * 30;
    let mut seed = 1u32;
    let input: Vec<f32> = (0..len)
        .map(|_| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / (1 << 23) as f32 - 1.0
        })
        .collect();
    let mut clock = vec![0.0; len];
    pulses(&mut clock, 0, 3000, len / 3000);
    let (l, r) = run_with(&mut delay(SR, 64), &input, &clock, 64, |i| {
        // Time swept by a slow triangle, both modes, sync on and off.
        let tri = ((i / 64) % 1000) as f32 / 1000.0;
        P([
            20.0 + 3980.0 * tri,
            ((i / 480000) % 5) as f32,
            95.0,
            100.0,
            16000.0,
            ((i / 240000) % 2) as f32,
        ])
    });
    for v in [l, r] {
        assert!(v.iter().all(|x| x.is_finite()));
        // White noise at ±1 through a loop gain ≤ 0.95: bounded well below this.
        assert!(
            v.iter().all(|x| x.abs() < 40.0),
            "peak {}",
            v.iter().fold(0.0f32, |m, x| m.max(x.abs()))
        );
    }
}

#[test]
fn carry_from_copies_history_and_ignores_other_kinds() {
    let clock = vec![0.0; 4800];
    let mut a = delay(SR, 64);
    run_with(&mut a, &impulse(2400), &clock[..2400], 64, |_| {
        P::free(100.0)
    });
    let mut b = delay(SR, 64);
    b.carry_from(&a);
    let (la, _) = run_with(&mut a, &vec![0.0; 4800], &clock, 64, |_| P::free(100.0));
    let (lb, _) = run_with(&mut b, &vec![0.0; 4800], &clock, 64, |_| P::free(100.0));
    assert_eq!(la, lb);
    assert!(la.iter().any(|&x| x != 0.0));

    let mut c = delay(SR, 64);
    c.carry_from(&kabl_modules::builtins::Mixer::new());
    let (lc, _) = run_with(&mut c, &vec![0.0; 4800], &clock, 64, |_| P::free(100.0));
    assert!(lc.iter().all(|&x| x == 0.0));
}
