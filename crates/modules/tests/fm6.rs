//! `osc.fm6`: every algorithm against an independent f64 evaluation of the same FM equations
//! (so chains of any depth keep their ideal phase relations at any pitch), key sync, velocity,
//! envelopes, silence, bad input, carry and state.

mod common;
use common::*;
use kabl_modules::module::{StateReader, StateWriter};
use kabl_modules::view::ModuleView;
use std::collections::HashMap;
use std::f64::consts::TAU;

/// (modulators of operator k+1, as 1-based numbers) and the carriers, from the algorithm labels
/// (`ALGORITHM_LABELS`), written out here on purpose and not read from the module.
const ALGS: [(&[&[usize]], &[usize]); 8] = [
    (&[&[2], &[3], &[4], &[5], &[6], &[]], &[1]),
    (&[&[2], &[], &[4], &[], &[6], &[]], &[1, 3, 5]),
    (&[&[2], &[3], &[], &[5], &[6], &[]], &[1, 4]),
    (&[&[2], &[], &[4], &[5], &[6], &[]], &[1, 3]),
    (&[&[2, 3, 4, 5, 6], &[], &[], &[], &[], &[]], &[1]),
    (&[&[6], &[6], &[6], &[6], &[6], &[]], &[1, 2, 3, 4, 5]),
    (&[&[2, 3], &[4], &[5], &[6], &[], &[]], &[1]),
    (&[&[], &[], &[], &[], &[], &[]], &[1, 2, 3, 4, 5, 6]),
];
const RATIOS: [f64; 6] = [1.0, 2.0, 3.0, 1.0, 5.0, 2.0];
const LEVELS: [f64; 6] = [1.0, 0.6, 0.5, 0.4, 0.3, 0.2];
const INDEX: f64 = 0.15;

fn rig(alg: usize, hz: f32, oversample: bool, extra: &[(&str, f32)]) -> Rig {
    let mut set: Vec<(String, f32)> = vec![
        ("base_hz".into(), hz),
        ("algorithm".into(), alg as f32),
        ("index".into(), INDEX as f32),
        ("oversample".into(), oversample as u8 as f32),
    ];
    for k in 0..6 {
        let n = k + 1;
        set.push((format!("ratio{n}"), RATIOS[k] as f32));
        set.push((format!("level{n}"), LEVELS[k] as f32));
        set.push((format!("attack{n}"), 0.1));
        set.push((format!("sustain{n}"), 1.0));
    }
    set.extend(extra.iter().map(|&(k, v)| (k.to_string(), v)));
    let set: Vec<(&str, f32)> = set.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    Rig::new("osc.fm6", &set, SR)
}

/// Gate high from sample 0; velocity 1.
fn play(r: &mut Rig, len: usize) -> Vec<f32> {
    r.render(len, 0, |k, _| if k == 1 || k == 2 { 1.0 } else { 0.0 })
}

/// The voice at time `t` seconds after the note-on, by the equations only.
fn reference(alg: usize, f0: f64, t: f64) -> f64 {
    let (mods, carriers) = ALGS[alg];
    let mut o = [0.0f64; 7];
    for k in (1..=6).rev() {
        let m: f64 = mods[k - 1].iter().map(|&j| o[j]).sum();
        o[k] = LEVELS[k - 1] * (TAU * RATIOS[k - 1] * f0 * t + INDEX * 8.0 * m).sin();
    }
    carriers.iter().map(|&k| o[k]).sum::<f64>() / (carriers.len() as f64).sqrt()
}

#[test]
fn every_algorithm_matches_the_equations_at_several_pitches_and_both_rates() {
    // Output sample n carries the voice as it was (n - delay) samples ago: the anti-alias
    // filter's 16 (2x) or 32 (4x) internal samples of delay, from its newest tap.
    for alg in 0..8 {
        for hz in [130.8f32, 523.3, 1046.5] {
            for (over, delay) in [(false, 7.5), (true, 7.25)] {
                let x = play(&mut rig(alg, hz, over, &[]), 4096);
                let mut worst = 0.0f64;
                for (n, &v) in x.iter().enumerate().skip(64) {
                    let want = reference(alg, hz as f64, (n as f64 - delay) / SR as f64);
                    worst = worst.max((v as f64 - want).abs());
                }
                assert!(
                    worst < 0.005,
                    "algorithm {alg} {hz} Hz {}x: worst error {worst:.4}",
                    if over { 4 } else { 2 }
                );
            }
        }
    }
}

#[test]
fn a_note_on_restarts_every_phase() {
    // A second note, after the first has died away, is sample for sample the first one again.
    let dead: Vec<(String, f32)> = (1..=6).map(|n| (format!("release{n}"), 0.1)).collect();
    let dead: Vec<(&str, f32)> = dead.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    let mut r = rig(0, 261.6, false, &dead);
    let gate = |n: usize| if !(64..4096).contains(&n) { 1.0 } else { 0.0 };
    let x = r.render(4096 + 512, 0, |k, n| if k == 1 { gate(n) } else { 1.0 });
    assert!(x[2048..4096].iter().all(|&v| v == 0.0));
    assert_eq!(x[..64], x[4096..4160]);
}

#[test]
fn velocity_scales_only_the_operators_that_follow_it() {
    let vel = |v: f32| {
        let mut r = rig(7, 261.6, false, &[("vel1", 1.0)]);
        r.render(4096, 0, |k, _| match k {
            1 => 1.0,
            2 => v,
            _ => 0.0,
        })
    };
    // ADD: operators 2..6 ignore velocity (vel = 0), operator 1 follows it.
    let (full, half, none) = (vel(1.0), vel(0.5), vel(0.0));
    let op1_only = |x: &[f32], y: &[f32]| -> f32 {
        x.iter()
            .zip(y)
            .map(|(a, b)| a - b)
            .fold(0.0f32, |m, v| m.max(v.abs()))
    };
    // full - half = op1 * 0.5 / sqrt(6): peak of a unit sine over sqrt(6)/2.
    let d = op1_only(&full, &half);
    assert!((d - 0.5 / 6f32.sqrt()).abs() < 0.01, "{d}");
    let d0 = op1_only(&full, &none);
    assert!((d0 - 1.0 / 6f32.sqrt()).abs() < 0.01, "{d0}");
}

#[test]
fn an_operator_envelope_sets_its_level_and_release_ends_in_exact_silence() {
    let mut r = rig(
        7,
        261.6,
        false,
        &[
            ("sustain1", 0.5),
            ("decay1", 5.0),
            ("release1", 20.0),
            ("level2", 0.0),
            ("level3", 0.0),
            ("level4", 0.0),
            ("level5", 0.0),
            ("level6", 0.0),
        ],
    );
    let gate = |n: usize| if n < 24000 { 1.0 } else { 0.0 };
    let x = r.render(48000, 0, |k, n| if k == 1 { gate(n) } else { 1.0 });
    let hold = peak(&x[20000..24000]);
    assert!((hold - 0.5 / 6f32.sqrt()).abs() < 0.005, "sustain {hold}");
    // 20 ms to -40 dB: well below -60 dB after half a second, then exactly zero.
    assert!(peak(&x[24000 + 12000..24000 + 12100]) < 1e-3);
    assert!(x[40000..].iter().all(|&v| v == 0.0));
}

#[test]
fn no_gate_no_sound() {
    let mut r = rig(0, 261.6, false, &[]);
    assert!(r.render(1024, 0, |_, _| 0.0).iter().all(|&v| v == 0.0));
}

#[test]
fn extreme_settings_and_nan_never_poison_the_voice() {
    let mut r = rig(
        0,
        20000.0,
        true,
        &[
            ("index", 2.0),
            ("feedback", 1.0),
            ("ratio6", 32.0),
            ("fine6", 600.0),
        ],
    );
    let pitch = [60.0f32; BLOCK];
    let one = [1.0f32; BLOCK];
    for _ in 0..2000 {
        let o = r.block(&[&pitch, &one, &one])[0];
        assert!(o.iter().all(|v| v.is_finite() && v.abs() <= 3.0));
    }
    let nan = [f32::NAN; BLOCK];
    let bad = r.block(&[&nan, &nan, &nan]);
    assert!(bad[0].iter().all(|v| v.is_finite()));
    let after = r.block(&[&[0.0; BLOCK], &one, &one])[0];
    assert!(after.iter().all(|v| v.is_finite()));
}

#[test]
fn carry_from_continues_bit_exactly() {
    let mut a = rig(2, 330.0, true, &[("feedback", 0.3)]);
    play(&mut a, 640);
    let mut b = rig(2, 330.0, true, &[("feedback", 0.3)]);
    b.m.carry_from(a.m.as_ref());
    let one = [1.0f32; BLOCK];
    for _ in 0..5 {
        assert_eq!(
            a.block(&[&[0.0; BLOCK], &one, &one]),
            b.block(&[&[0.0; BLOCK], &one, &one])
        );
    }
}

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
fn saved_state_restores_phases_and_envelopes() {
    let mut a = rig(1, 220.0, false, &[]);
    play(&mut a, 960);
    let mut s = MapState::default();
    a.m.save_state(&mut s);
    assert_eq!(s.0.len(), 12);
    let mut b = rig(1, 220.0, false, &[]);
    b.m.load_state(&s);
    let mut s2 = MapState::default();
    b.m.save_state(&mut s2);
    assert_eq!(s.0, s2.0);
}

#[test]
fn view_shows_one_cycle_of_the_output() {
    let mut r = rig(
        7,
        375.0,
        false,
        &[
            ("level2", 0.0),
            ("level3", 0.0),
            ("level4", 0.0),
            ("level5", 0.0),
            ("level6", 0.0),
        ],
    );
    let mut v = ModuleView::default();
    r.m.view(&mut v);
    assert!(!v.valid, "nothing played yet");
    play(&mut r, 4800);
    r.m.view(&mut v);
    assert!(v.valid);
    // 375 Hz is 128 samples: one cycle of a sine, starting at a rising zero crossing.
    let amp = 1.0 / 6f32.sqrt();
    for (k, &y) in v.cycle.iter().enumerate() {
        let want = amp * (TAU as f32 * k as f32 / 128.0).sin();
        assert!((y - want).abs() < 0.03, "point {k}: {y} vs {want}");
    }
}

#[test]
fn four_x_is_cleaner_where_energy_passes_the_double_rate() {
    // Operator 6 alone with full feedback at 3.1 kHz: 2x -40.9 dB, 4x -52.2 dB (README table).
    let alone: Vec<(&str, f32)> = vec![
        ("feedback", 1.0),
        ("level1", 0.0),
        ("level2", 0.0),
        ("level3", 0.0),
        ("level4", 0.0),
        ("level5", 0.0),
        ("level6", 1.0),
    ];
    let db = |over| {
        let x = play(&mut rig(7, 3136.0, over, &alone), 4096 + 16384)[4096..].to_vec();
        alias_db(&x, 3136.0, SR, 20000.0).1
    };
    let (two, four) = (db(false), db(true));
    assert!(four < two - 8.0, "2x {two:.1} dB, 4x {four:.1} dB");
}

/// Not a check: 2x vs 4x aliasing for the README. Chains of `depth` equal-ratio operators at
/// full level (depth 0: operator 6 alone, added, with feedback); `index` is the control (radians per hop = 8 * index).
/// `cargo test -p kabl-modules --test fm6 print_alias_table -- --ignored --nocapture`
#[test]
#[ignore]
fn print_alias_table() {
    println!("depth  hz      index  feedback | 2x dB | 4x dB   (non-harmonic power below 20 kHz)");
    for (depth, hz, index, fb) in [
        (2, 440.3, 0.25, 0.0),
        (2, 1046.5, 0.5, 0.0),
        (2, 2093.0, 0.5, 0.0),
        (2, 2093.0, 1.0, 0.0),
        (3, 1046.5, 0.35, 0.0),
        (4, 523.3, 0.3, 0.0),
        (4, 1046.5, 0.3, 0.0),
        (6, 523.3, 0.2, 0.0),
        (6, 1046.5, 0.2, 0.0),
        (0, 440.3, 0.0, 1.0),
        (0, 1760.3, 0.0, 0.5),
        (0, 1760.3, 0.0, 1.0),
        (0, 3136.0, 0.0, 1.0),
    ] {
        let db = |over| {
            let mut set: Vec<(String, f32)> =
                vec![("index".into(), index), ("feedback".into(), fb)];
            for k in 1..=6 {
                set.push((format!("ratio{k}"), 1.0));
                let on = if depth == 0 { k == 6 } else { k <= depth };
                set.push((format!("level{k}"), on as u8 as f32));
            }
            let set: Vec<(&str, f32)> = set.iter().map(|(k, v)| (k.as_str(), *v)).collect();
            let mut r = rig(if depth == 0 { 7 } else { 0 }, hz, over, &set);
            let x = play(&mut r, 4096 + 16384)[4096..].to_vec();
            alias_db(&x, hz, SR, 20000.0).1
        };
        println!(
            "{depth:5} {hz:7.1} {index:5.2} {fb:8.1} | {:5.1} | {:5.1}",
            db(false),
            db(true)
        );
    }
}
