//! The utility modules: quantizer, sample.hold, slew, attenuverter, logic, comparator,
//! crossfade, pan and random. Each against an independent statement of what it must do.

mod common;
use common::*;
use kabl_modules::builtins::Random;
use kabl_modules::module::{StateReader, StateWriter};
use std::collections::HashMap;

#[test]
fn attenuverter_knob_is_cubic_and_round_trips() {
    let info = kabl_modules::registry::info_for("attenuverter").unwrap();
    for p in info.params {
        assert_eq!(p.from_norm(0.5), 0.0, "{}", p.name);
        for v in [-24.0f32, -12.0, -1.0, -0.1, 0.0, 0.5, 1.0, 12.0, 24.0] {
            assert!(
                (p.from_norm(p.to_norm(v)) - v).abs() < 1e-3,
                "{} {v}",
                p.name
            );
        }
        // Fine near unity: a 1 % step of travel moves the value by less than 1.
        assert!(p.from_norm(p.to_norm(1.0) + 0.01) - 1.0 < 1.0, "{}", p.name);
    }
}

/// A gate that is high for `high` samples of every `period`.
fn pulses(period: usize, high: usize) -> impl Fn(usize) -> f32 {
    move |n| if n % period < high { 1.0 } else { 0.0 }
}

// ---- quantizer ------------------------------------------------------------------------------

const DEGREES: [&[i32]; 12] = [
    &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
    &[0, 2, 4, 5, 7, 9, 11],
    &[0, 2, 3, 5, 7, 8, 10],
    &[0, 2, 3, 5, 7, 8, 11],
    &[0, 2, 3, 5, 7, 9, 10],
    &[0, 1, 3, 5, 7, 8, 10],
    &[0, 2, 4, 6, 7, 9, 11],
    &[0, 2, 4, 5, 7, 9, 10],
    &[0, 2, 4, 7, 9],
    &[0, 3, 5, 7, 10],
    &[0, 3, 5, 6, 7, 10],
    &[0, 2, 4, 6, 8, 10],
];

fn quantize(scale: usize, root: i32, x: &[f32]) -> Vec<f32> {
    let mut r = Rig::new(
        "quantizer",
        &[("scale", scale as f32), ("root", root as f32)],
        SR,
    );
    r.render(x.len(), 0, |_, n| x[n])
}

#[test]
fn every_scale_outputs_only_its_notes_and_the_nearest_one() {
    // Inputs on a 0.05 grid over three octaves, each held for a block so hysteresis is moot.
    let xs: Vec<f32> = (-60..=60).map(|k| k as f32 * 0.3 + 0.02).collect();
    for (scale, degrees) in DEGREES.iter().enumerate() {
        for root in [0, 2, 9] {
            let input: Vec<f32> = xs.iter().flat_map(|&x| [x; BLOCK]).collect();
            let out = quantize(scale, root, &input);
            for (k, &x) in xs.iter().enumerate() {
                let y = out[k * BLOCK + BLOCK - 1];
                let pc = ((y.round() as i32 - root) % 12 + 12) % 12;
                assert!(
                    (y - y.round()).abs() < 1e-5,
                    "scale {scale}: {y} not a note"
                );
                assert!(
                    degrees.contains(&pc),
                    "scale {scale} root {root}: {x} -> {y}"
                );
                // No allowed note is nearer than the one chosen (hysteresis aside, a sample
                // that follows a distant one may sit up to 0.05 st past the midpoint).
                let best = (-80..80)
                    .filter(|n| degrees.contains(&((n - root).rem_euclid(12))))
                    .map(|n| (x - n as f32).abs())
                    .fold(f32::MAX, f32::min);
                assert!(
                    (x - y).abs() <= best + 0.1 + 1e-4,
                    "scale {scale} {x} -> {y}"
                );
            }
        }
    }
}

#[test]
fn chromatic_passes_whole_notes_and_transpose_shifts_after_snapping() {
    let out = quantize(0, 0, &[7.0; 256]);
    assert!(out.iter().all(|&v| v == 7.0));
    let mut r = Rig::new("quantizer", &[("scale", 1.0), ("transpose", 5.0)], SR);
    let y = r.render(256, 0, |_, _| 1.2); // snaps to C# -> D (2), then +5
    assert!(y.iter().all(|&v| v == 7.0), "{}", y[255]);
}

#[test]
fn trig_pulses_once_per_note_change_but_not_for_the_first_note() {
    let mut r = Rig::new("quantizer", &[("scale", 0.0)], SR);
    // Notes 0, 0.3 (same note), 5, 5, 9, back to 0, each for 2000 samples.
    let seq = [0.0, 0.3, 5.0, 5.2, 9.0, 0.0];
    let trig = r.render(12000, 1, |_, n| seq[(n / 2000).min(5)]);
    let rises = trig
        .windows(2)
        .filter(|w| w[0] < 0.5 && w[1] >= 0.5)
        .count();
    assert_eq!(
        rises, 3,
        "5, 9 and 0 are changes; the first note and 0.3, 5.2 are not"
    );
    let width = trig.iter().filter(|&&v| v > 0.5).count() / 3;
    assert!((width as f32 - 0.005 * SR).abs() < 2.0, "{width} samples");
}

#[test]
fn a_slow_input_crossing_a_boundary_does_not_chatter() {
    // A 0.001 st ripple around the midpoint between 0 and 2 (major: 1 is not a note).
    let mut r = Rig::new("quantizer", &[("scale", 1.0)], SR);
    let trig = r.render(24000, 1, |_, n| 1.0 + 0.01 * ((n as f32) * 0.5).sin());
    let rises = trig
        .windows(2)
        .filter(|w| w[0] < 0.5 && w[1] >= 0.5)
        .count();
    assert!(rises <= 1, "{rises} changes from a 0.01 st ripple");
}

#[test]
fn changing_the_scale_requantizes_at_once_and_nan_is_ignored() {
    let mut r = Rig::new("quantizer", &[("scale", 0.0)], SR);
    let y = r.render(128, 0, |_, _| 1.0);
    assert_eq!(y[127], 1.0);
    r.set("scale", 1.0); // major: 1 is not a note, ties go down
    let y = r.render(128, 0, |_, _| 1.0);
    assert_eq!(y[127], 0.0);
    let y = r.render(128, 0, |_, _| f32::NAN);
    assert!(y.iter().all(|v| v.is_finite()));
}

// ---- sample and hold ------------------------------------------------------------------------

#[test]
fn sample_takes_the_value_at_each_rising_edge_and_holds() {
    let mut r = Rig::new("sample.hold", &[], SR);
    let gate = pulses(100, 10);
    let y = r.render(
        1000,
        0,
        |k, n| if k == 0 { n as f32 * 0.5 } else { gate(n) },
    );
    assert_eq!(y[0], 0.0, "edge at sample 0 samples the value there");
    for edge in (0..1000).step_by(100) {
        assert!(y[edge..edge + 100].iter().all(|&v| v == edge as f32 * 0.5));
    }
}

#[test]
fn track_follows_while_high_and_holds_when_low() {
    let mut r = Rig::new("sample.hold", &[("mode", 1.0)], SR);
    let gate = pulses(100, 50);
    let y = r.render(300, 0, |k, n| if k == 0 { n as f32 } else { gate(n) });
    assert_eq!(y[30], 30.0);
    assert_eq!(y[49], 49.0);
    assert!(y[50..100].iter().all(|&v| v == 49.0));
    assert_eq!(y[120], 120.0);
}

#[test]
fn a_held_pitch_stays_exact_and_starts_at_zero() {
    let mut r = Rig::new("sample.hold", &[], SR);
    let y = r.render(200, 0, |k, n| {
        if k == 0 {
            37.123455
        } else if n >= 100 {
            1.0
        } else {
            0.0
        }
    });
    assert!(y[..100].iter().all(|&v| v == 0.0));
    assert!(y[100..].iter().all(|&v| v == 37.123455));
}

// ---- slew -----------------------------------------------------------------------------------

#[test]
fn a_step_settles_within_one_percent_in_the_set_time_and_not_much_sooner() {
    for ms in [5.0f32, 50.0, 500.0] {
        let mut r = Rig::new("slew", &[("rise_ms", ms), ("fall_ms", ms)], SR);
        let n = (ms * 0.001 * SR) as usize;
        let y = r.render(n + 400, 0, |_, _| 1.0);
        assert!(
            (1.0 - y[n]).abs() <= 0.0101,
            "{ms} ms: {} at the set time",
            y[n]
        );
        assert!(1.0 - y[n * 8 / 10] > 0.01, "{ms} ms: settled early");
    }
}

#[test]
fn rise_and_fall_are_separate() {
    let mut r = Rig::new("slew", &[("rise_ms", 1.0), ("fall_ms", 200.0)], SR);
    let up = r.render(480, 0, |_, _| 1.0);
    assert!(up[479] > 0.99);
    let down = r.render(480, 0, |_, _| 0.0);
    assert!(down[479] > 0.7, "fall of 200 ms is slow: {}", down[479]);
}

#[test]
fn minimum_time_passes_the_signal_and_settled_output_is_exact() {
    let mut r = Rig::new("slew", &[("rise_ms", 0.1), ("fall_ms", 0.1)], SR);
    let x: Vec<f32> = (0..256).map(|n| ((n * 37) % 11) as f32 - 5.0).collect();
    assert_eq!(r.render(256, 0, |_, n| x[n]), x);
    let mut r = Rig::new("slew", &[], SR);
    let y = r.render(48000, 0, |_, _| 7.5);
    assert_eq!(y[47999], 7.5);
}

#[test]
fn follow_mode_tracks_the_size_of_the_signal() {
    let mut r = Rig::new(
        "slew",
        &[("mode", 1.0), ("rise_ms", 2.0), ("fall_ms", 80.0)],
        SR,
    );
    let y = r.render(9600, 0, |_, n| (n as f32 * 0.31).sin() * 0.5);
    // Mean of |0.5 sin| is 0.318; fast rise and slow fall sit above it, below the 0.5 peak.
    let tail = &y[4800..];
    let mean = tail.iter().sum::<f32>() / tail.len() as f32;
    assert!((0.3..0.5).contains(&mean), "{mean}"); // peak-biased: fast rise, slow fall
    assert!(tail.iter().all(|&v| v >= 0.0));
}

#[test]
fn slew_survives_nan() {
    let mut r = Rig::new("slew", &[], SR);
    r.render(100, 0, |_, _| 1.0);
    assert!(r
        .render(100, 0, |_, _| f32::NAN)
        .iter()
        .all(|v| v.is_finite()));
}

// ---- attenuverter ---------------------------------------------------------------------------

#[test]
fn attenuverter_scales_inverts_and_offsets() {
    for (amount, offset) in [
        (1.0f32, 0.0f32),
        (0.5, 0.25),
        (-1.0, 0.0),
        (12.0, -3.0),
        (-2.5, 7.0),
    ] {
        let mut r = Rig::new(
            "attenuverter",
            &[("amount", amount), ("offset", offset)],
            SR,
        );
        let x: Vec<f32> = (0..256).map(|n| (n as f32 * 0.1).sin()).collect();
        let y = r.render(256, 0, |_, n| x[n]);
        for (a, b) in x.iter().zip(&y) {
            assert!((a * amount + offset - b).abs() < 1e-5);
        }
    }
}

// ---- logic and comparator -------------------------------------------------------------------

#[test]
fn logic_follows_its_truth_table_exactly() {
    let mut r = Rig::new("logic", &[], SR);
    // Samples 0..4 cover (a, b) = (0,0) (1,0) (0,1) (1,1); 0.49 is low, 0.5 is high.
    let a = [0.0, 1.0, 0.0, 1.0, 0.49, 0.5];
    let b = [0.0, 0.0, 1.0, 1.0, 0.5, 0.49];
    let ins = [&a[..], &b[..]];
    let mut pa = [0.0f32; BLOCK];
    let mut pb = [0.0f32; BLOCK];
    pa[..6].copy_from_slice(&a);
    pb[..6].copy_from_slice(&b);
    let _ = ins;
    let o = r.block(&[&pa, &pb]);
    let want = |f: fn(bool, bool) -> bool| -> Vec<f32> {
        (0..6)
            .map(|i| f(a[i] >= 0.5, b[i] >= 0.5) as u8 as f32)
            .collect()
    };
    assert_eq!(&o[0][..6], &want(|x, y| x && y)[..], "and");
    assert_eq!(&o[1][..6], &want(|x, y| x || y)[..], "or");
    assert_eq!(&o[2][..6], &want(|x, y| x != y)[..], "xor");
    assert_eq!(&o[3][..6], &want(|x, _| !x)[..], "not");
}

#[test]
fn two_clocks_through_and_fire_only_where_they_coincide() {
    let mut r = Rig::new("logic", &[], SR);
    let (a, b) = (pulses(8, 4), pulses(12, 6));
    let y = r.render(240, 0, |k, n| if k == 0 { a(n) } else { b(n) });
    for (n, &v) in y.iter().enumerate() {
        assert_eq!(v > 0.5, a(n) > 0.5 && b(n) > 0.5, "sample {n}");
    }
}

#[test]
fn comparator_has_a_threshold_and_a_dead_band() {
    let mut r = Rig::new("comparator", &[("threshold", 0.2), ("hysteresis", 0.0)], SR);
    let y = r.render(4, 0, |_, n| [0.1, 0.19, 0.21, 0.5][n.min(3)]);
    assert_eq!(y[..4], [0.0, 0.0, 1.0, 1.0]);
    // A ±0.04 wobble around the threshold: with a 0.1 dead band it never flips, without it does.
    let wob = |n: usize| 0.2 + 0.04 * if n.is_multiple_of(2) { 1.0 } else { -1.0 };
    let flips = |h: f32| {
        let mut r = Rig::new("comparator", &[("threshold", 0.2), ("hysteresis", h)], SR);
        let y = r.render(200, 0, |_, n| wob(n));
        y.windows(2).filter(|w| w[0] != w[1]).count()
    };
    assert!(flips(0.0) > 100);
    assert_eq!(flips(0.1), 0);
}

// ---- crossfade and pan ----------------------------------------------------------------------

#[test]
fn crossfade_ends_are_exact_and_the_middle_blends() {
    let a = |n: usize| (n as f32 * 0.2).sin();
    let b = |n: usize| (n as f32 * 0.07).cos() * 0.8;
    for (mix, want) in [(0.0f32, 0.0f32), (1.0, 1.0), (0.25, 0.25)] {
        let mut r = Rig::new("crossfade", &[("mix", mix)], SR);
        let y = r.render(64, 0, |k, n| [a(n), b(n), 0.0][k]);
        for (n, &v) in y.iter().enumerate() {
            let w = a(n) * (1.0 - want) + b(n) * want;
            assert!((v - w).abs() < 1e-6);
        }
    }
    // The same signal on both inputs is unchanged at every position with the linear curve.
    let mut r = Rig::new("crossfade", &[("mix", 0.37)], SR);
    let y = r.render(64, 0, |k, n| if k < 2 { a(n) } else { 0.0 });
    assert!((0..64).all(|n| (y[n] - a(n)).abs() < 1e-6));
}

#[test]
fn the_fade_input_adds_to_mix_and_the_sum_is_held_in_range() {
    let mut r = Rig::new("crossfade", &[("mix", 0.5)], SR);
    let y = r.render(8, 0, |k, _| [1.0, 3.0, 0.2][k]);
    assert!((y[0] - (0.3 * 1.0 + 0.7 * 3.0)).abs() < 1e-6);
    let y = r.render(8, 0, |k, _| [1.0, 3.0, 5.0][k]);
    assert_eq!(y[0], 3.0, "clamped to all B");
}

#[test]
fn equal_power_crossfade_dips_less_in_the_middle() {
    let mut r = Rig::new("crossfade", &[("mix", 0.5), ("curve", 1.0)], SR);
    let y = r.render(8, 0, |k, _| [1.0, 0.0, 0.0][k]);
    assert!((y[0] - 0.5f32.sqrt()).abs() < 1e-6);
    let y = r.render(8, 0, |k, _| [1.0, 1.0, 0.0][k]);
    assert!(
        (y[0] - 2.0 * 0.5f32.sqrt()).abs() < 1e-6,
        "uncorrelated-power law"
    );
}

#[test]
fn pan_is_equal_power_and_hard_ends_are_one_sided() {
    for p in [-1.0f32, -0.5, 0.0, 0.3, 1.0] {
        let mut r = Rig::new("pan", &[("pan", p)], SR);
        let o = r.block(&[&[1.0; BLOCK]]);
        let (l, rt) = (o[0][0], o[1][0]);
        assert!(
            (l * l + rt * rt - 1.0).abs() < 1e-5,
            "pan {p}: power {}",
            l * l + rt * rt
        );
        if p == -1.0 {
            assert!((l - 1.0).abs() < 1e-6 && rt.abs() < 1e-6);
        }
        if p == 1.0 {
            assert!((rt - 1.0).abs() < 1e-6 && l.abs() < 1e-6);
        }
        if p == 0.0 {
            assert!((l - rt).abs() < 1e-6 && (l - 0.5f32.sqrt()).abs() < 1e-6);
        }
    }
    // The pan input adds to the knob.
    let mut r = Rig::new("pan", &[("pan", -0.5)], SR);
    let o = r.block(&[&[1.0; BLOCK], &[0.5; BLOCK]]);
    assert!((o[0][0] - o[1][0]).abs() < 1e-6);
}

// ---- random ---------------------------------------------------------------------------------

fn random(params: &[(&str, f32)], module: u64, lane: usize) -> Rig {
    let mut r = Rig::new("random", params, SR);
    r.m.as_any_mut()
        .downcast_mut::<Random>()
        .unwrap()
        .seed(module, lane);
    r
}

/// The value after each of `ticks` clock pulses (64 samples apart), `reset` at the given ticks.
fn draws(r: &mut Rig, ticks: usize, reset_at: &[usize]) -> Vec<f32> {
    let y = r.render(ticks * 64, 0, |k, n| {
        let tick = n / 64;
        let high = n % 64 < 8;
        match k {
            0 => {
                (high && !reset_at.contains(&tick)) as u8 as f32
                    + (high && reset_at.contains(&tick)) as u8 as f32
            }
            _ => (high && reset_at.contains(&tick)) as u8 as f32,
        }
    });
    (0..ticks).map(|t| y[t * 64 + 32]).collect()
}

#[test]
fn the_same_seed_gives_the_same_values_and_another_seed_gives_others() {
    let a = draws(&mut random(&[], 5, 0), 64, &[]);
    let b = draws(&mut random(&[], 5, 0), 64, &[]);
    let c = draws(&mut random(&[], 6, 0), 64, &[]);
    let d = draws(&mut random(&[], 5, 1), 64, &[]);
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_ne!(a, d);
    assert!(
        a.windows(2).all(|w| w[0] != w[1]),
        "a new value on each tick"
    );
}

#[test]
fn values_are_in_range_and_cover_it() {
    let bi = draws(&mut random(&[], 1, 0), 2000, &[]);
    assert!(bi.iter().all(|v| (-1.0..1.0).contains(v)));
    let mean = bi.iter().sum::<f32>() / bi.len() as f32;
    assert!(mean.abs() < 0.08, "mean {mean}");
    assert!(bi.iter().any(|&v| v > 0.9) && bi.iter().any(|&v| v < -0.9));
    let uni = draws(
        &mut random(&[("bipolar", 0.0), ("range", 12.0)], 1, 0),
        2000,
        &[],
    );
    assert!(uni.iter().all(|v| (0.0..12.0).contains(v)));
    // Same stream, scaled: uni*range = (bi + 1) / 2 * range.
    for (u, b) in uni.iter().zip(&bi) {
        assert!((u - (b + 1.0) * 6.0).abs() < 1e-4);
    }
}

#[test]
fn a_loop_repeats_exactly_and_a_free_run_does_not() {
    let l = draws(&mut random(&[("length", 4.0)], 3, 0), 16, &[]);
    for k in 4..16 {
        assert_eq!(l[k], l[k - 4], "step {k}");
    }
    assert!(l[..4].windows(2).all(|w| w[0] != w[1]));
    let free = draws(&mut random(&[], 3, 0), 16, &[]);
    assert_eq!(
        &free[..4],
        &l[..4],
        "the loop's first pass is the free stream"
    );
    assert_ne!(free[4], free[0]);
}

#[test]
fn change_lets_a_loop_drift_but_zero_keeps_it() {
    let still = draws(
        &mut random(&[("length", 8.0), ("change", 0.0)], 9, 0),
        64,
        &[],
    );
    assert!((8..64).all(|k| still[k] == still[k - 8]));
    let drift = draws(
        &mut random(&[("length", 8.0), ("change", 50.0)], 9, 0),
        64,
        &[],
    );
    let same = (8..64).filter(|&k| drift[k] == drift[k - 8]).count();
    assert!(
        same > 10 && same < 54,
        "{same} of 56 repeated at 50 % change"
    );
}

#[test]
fn reset_returns_to_the_first_value() {
    let d = draws(&mut random(&[("length", 4.0)], 2, 0), 12, &[6]);
    // A reset on tick 6 is also a clock edge there; the loop restarts, so tick 6 plays step 0
    // and the loop's order follows.
    assert_eq!(d[6], d[0]);
    assert_eq!(&d[6..10], &d[0..4]);
    let free = draws(&mut random(&[], 2, 0), 12, &[6]);
    assert_eq!(free[6], free[0], "a free run reseeds too");
}

#[test]
fn carry_from_continues_the_stream_and_a_different_seed_does_not_carry() {
    let mut a = random(&[("length", 4.0), ("change", 30.0)], 4, 0);
    draws(&mut a, 10, &[]);
    let mut b = random(&[("length", 4.0), ("change", 30.0)], 4, 0);
    b.m.carry_from(a.m.as_ref());
    assert_eq!(draws(&mut a, 20, &[]), draws(&mut b, 20, &[]));
    let mut c = random(&[], 99, 0);
    let before = draws(&mut c, 3, &[]);
    let mut c2 = random(&[], 99, 0);
    c2.m.carry_from(a.m.as_ref());
    assert_eq!(draws(&mut c2, 3, &[]), before);
}

// ---- state and bad input --------------------------------------------------------------------

#[derive(Default)]
struct MapState(HashMap<String, f32>);
impl StateWriter for MapState {
    fn write_f32(&mut self, k: &str, v: f32) {
        self.0.insert(k.into(), v);
    }
}
impl StateReader for MapState {
    fn read_f32(&self, k: &str) -> Option<f32> {
        self.0.get(k).copied()
    }
}

#[test]
fn state_round_trips_and_nan_never_escapes() {
    for kind in ["quantizer", "sample.hold", "slew", "comparator", "random"] {
        let mut a = Rig::new(kind, &[], SR);
        a.render(1000, 0, |k, n| {
            if k == 1 {
                ((n / 100) % 2) as f32
            } else {
                (n as f32 * 0.01).sin()
            }
        });
        let mut s = MapState::default();
        a.m.save_state(&mut s);
        let mut b = Rig::new(kind, &[], SR);
        b.m.load_state(&s);
        let mut s2 = MapState::default();
        b.m.save_state(&mut s2);
        assert_eq!(s.0, s2.0, "{kind}");
    }
    for kind in [
        "quantizer",
        "sample.hold",
        "slew",
        "attenuverter",
        "logic",
        "comparator",
        "crossfade",
        "pan",
        "random",
    ] {
        let mut r = Rig::new(kind, &[], SR);
        let nan = [f32::NAN; BLOCK];
        let inf = [f32::INFINITY; BLOCK];
        for ins in [[&nan, &nan, &nan], [&inf, &inf, &inf]] {
            let o = r.block(&[ins[0], ins[1], ins[2]]);
            assert!(o.iter().all(|b| b.iter().all(|v| v.is_finite())), "{kind}");
        }
    }
}
