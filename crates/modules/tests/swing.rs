//! Clock swing: the delayed pulses land where the maths says, to the sample, in free running
//! and under a host; a swung `seq` and a straight one play the same notes.

mod common;
use common::*;
use kabl_modules::builtins::Clock;

/// Sample indices of the rising edges of the clock's `gate` output over `len` samples.
fn edges(r: &mut Rig, len: usize) -> Vec<usize> {
    let g = r.render(len, 0, |_, _| 0.0);
    (1..g.len())
        .filter(|&i| g[i] > 0.5 && g[i - 1] < 0.5)
        .collect()
}

// 120 bpm at 48 kHz: one pulse is 6000 samples, a pair 12000.
#[test]
fn zero_swing_is_the_straight_clock() {
    let mut r = Rig::new("clock", &[("bpm", 120.0)], SR);
    let e = edges(&mut r, 48000);
    assert_eq!(e, vec![6000, 12000, 18000, 24000, 30000, 36000, 42000]);
}

#[test]
fn swing_delays_every_second_pulse_and_keeps_the_pair() {
    for (swing, shift) in [(100.0, 3000), (50.0, 1500), (0.0, 0)] {
        let mut r = Rig::new("clock", &[("bpm", 120.0), ("swing", swing)], SR);
        let e = edges(&mut r, 48000);
        // The first pulse starts at sample 0 (no rising edge to see); the pair is 12000.
        let want: Vec<usize> = (1..8)
            .map(|k| k * 6000 + if k % 2 == 1 { shift } else { 0 })
            .collect();
        assert_eq!(e, want[..e.len()], "swing {swing}");
        assert!(e.len() >= 6, "swing {swing}: {e:?}");
    }
}

#[test]
fn swing_changes_only_timing_never_the_count() {
    let count = |swing: f32| {
        let mut r = Rig::new("clock", &[("bpm", 137.0), ("swing", swing)], SR);
        edges(&mut r, 48000 * 20).len()
    };
    let straight = count(0.0);
    for s in [20.0, 67.0, 100.0] {
        assert!((count(s) as i64 - straight as i64).abs() <= 1, "swing {s}");
    }
}

/// Host position: the delayed pulse is found from the beat position, so a jump back (a loop)
/// or a tempo change lands on the same pair half as running there would.
#[test]
fn host_position_maps_through_the_pair() {
    let host_edges = |start_beats: f64, bpm: f64| {
        let mut r = Rig::new("clock", &[("swing", 100.0)], SR);
        let mut beats = start_beats;
        let mut out = vec![];
        let mut t = 0usize;
        for _ in 0..(48000 / BLOCK) {
            r.m.as_any_mut()
                .downcast_mut::<Clock>()
                .unwrap()
                .host(Some((beats, bpm, true)));
            let g = r.block(&[])[0];
            for (i, &x) in g.iter().enumerate() {
                if x > 0.5 && (i > 0 && g[i - 1] < 0.5 || i == 0 && out.is_empty()) {
                    out.push(t + i);
                }
            }
            t += BLOCK;
            beats += BLOCK as f64 / SR as f64 * bpm / 60.0;
        }
        out
    };
    // Beat 0 is a pair start. At 120 bpm a 16th is 6000 samples: pulse 1 (the second of the
    // pair) is delayed by 3000.
    let e = host_edges(0.0, 120.0);
    assert!(
        e.contains(&0) && e.iter().any(|&x| (9000..9100).contains(&x)),
        "{e:?}"
    );
    // Starting at beat 0.25 (pulse index 1, the delayed one), the first full pulse boundary
    // inside the pair is the next pair start: 0.5 * 2 pulses = 0.5 beat away less the delay.
    let e2 = host_edges(0.5, 120.0);
    assert!(e2.iter().any(|&x| x < 100), "{e2:?}");
}

// ---- ratchets -------------------------------------------------------------------------------

/// A steady clock into `seq` for `steps` steps of 6000 samples; the gate's rising edges.
fn seq_edges(set: &[(&str, f32)], steps: usize) -> Vec<usize> {
    let mut r = Rig::new("seq", set, SR);
    let g = r.render(steps * 6000, 0, |k, n| {
        if k == 0 && n % 6000 < 3000 {
            1.0
        } else {
            0.0
        }
    });
    (0..g.len())
        .filter(|&i| g[i] > 0.5 && (i == 0 || g[i - 1] < 0.5))
        .collect()
}

#[test]
fn no_ratchet_by_default_one_note_per_step() {
    let e = seq_edges(&[], 8);
    assert_eq!(e, (0..8).map(|k| k * 6000).collect::<Vec<_>>());
    let info = kabl_modules::registry::info_for("seq").unwrap();
    for p in info
        .params
        .iter()
        .filter(|p| p.name.ends_with(|c: char| c.is_ascii_digit()) && p.name.contains('k'))
    {
        assert_eq!(p.default, 1.0, "{}", p.name);
    }
}

#[test]
fn a_ratcheted_step_plays_evenly_spaced_notes_inside_its_length() {
    // Step 4 (index 3, from sample 18000) ratcheted 4 times; the others stay single.
    let e = seq_edges(&[("k4", 4.0)], 8);
    let want: Vec<usize> = [0, 6000, 12000]
        .into_iter()
        .chain([18000, 19500, 21000, 22500])
        .chain([24000, 30000, 36000, 42000])
        .collect();
    assert_eq!(e, want);
    let e3 = seq_edges(&[("k4", 3.0)], 8);
    assert_eq!(&e3[3..6], &[18000, 20000, 22000]);
    assert_eq!(e3.len(), 3 + 3 + 4);
}

#[test]
fn a_rest_stays_a_rest_and_probability_takes_the_whole_ratchet() {
    // Step 4 off: ratchet makes no sound.
    let e = seq_edges(&[("k4", 4.0), ("g4", 0.0)], 8);
    assert!(!e.iter().any(|&x| (18000..24000).contains(&x)), "{e:?}");
    // Step 4 at 0 %: none either; at 100 % all four. Over many loops each pass is all or none.
    let e = seq_edges(&[("k4", 4.0), ("r4", 0.0)], 8);
    assert!(!e.iter().any(|&x| (18000..24000).contains(&x)), "{e:?}");
    let e = seq_edges(&[("k4", 4.0), ("r4", 50.0)], 8 * 40);
    for pass in 0..40 {
        let lo = pass * 48000 + 18000;
        let n = e.iter().filter(|&&x| (lo..lo + 6000).contains(&x)).count();
        assert!(n == 0 || n == 4, "pass {pass}: {n}");
    }
}

#[test]
fn ratchets_follow_a_swung_clock() {
    // Real clock at 100 % swing into a seq with every step ratcheted twice: the two notes of
    // each step both fall inside that step, whether it is the long or the short one.
    let mut clock = Rig::new("clock", &[("bpm", 120.0), ("swing", 100.0)], SR);
    let mut seq = Rig::new(
        "seq",
        &[
            ("k1", 2.0),
            ("k2", 2.0),
            ("k3", 2.0),
            ("k4", 2.0),
            ("k5", 2.0),
            ("k6", 2.0),
            ("k7", 2.0),
            ("k8", 2.0),
        ],
        SR,
    );
    let len = 48000 * 4;
    let c = clock.render(len, 0, |_, _| 0.0);
    let g = seq.render(len, 0, |_, n| c[n]);
    let edge = |x: &[f32]| -> Vec<usize> {
        (1..x.len())
            .filter(|&i| x[i] > 0.5 && x[i - 1] < 0.5)
            .collect()
    };
    let (ce, ge) = (edge(&c), edge(&g));
    // Skip the first steps (the clock history is still filling).
    for w in ce.windows(2).skip(4).take(10) {
        let inside = ge.iter().filter(|&&x| x >= w[0] && x < w[1]).count();
        assert_eq!(inside, 2, "step {w:?}");
    }
}

/// The position report (what Perform's beat readout and queued-pad progress read) follows the
/// swung pulses: pulse k starts exactly when the position reaches k, and a pair's end, hence every
/// beat, stays on the straight grid.
#[test]
fn the_reported_position_agrees_with_the_swung_pulses() {
    for swing in [0.0f32, 58.0, 100.0] {
        let (bpm, s) = (120.0f64, swing as f64 / 200.0);
        let inc = bpm / 60.0 * 4.0 / SR as f64;
        let mut r = Rig::new("clock", &[("bpm", bpm as f32), ("swing", swing)], SR);
        let mut samples = 0usize;
        let mut last = 0.0f64;
        for _ in 0..(SR as usize * 10 / BLOCK) {
            let _ = r.block(&[]);
            samples += BLOCK;
            let got = r.m.as_any().downcast_ref::<Clock>().unwrap().position();
            // Straight time in pulses, mapped through the pair like the pulses are.
            let u = samples as f64 * inc;
            let (k, v) = ((u / 2.0).floor(), u % 2.0);
            let want = 2.0 * k
                + if v < 1.0 + s {
                    v / (1.0 + s)
                } else {
                    1.0 + (v - 1.0 - s) / (1.0 - s)
                };
            assert!(
                (got - want).abs() < 2.0 * inc / (1.0 - s).min(1.0),
                "swing {swing} at {samples}: reported {got}, pulses say {want}"
            );
            assert!(got >= last, "swing {swing}: position went back");
            last = got;
            // Every second pulse's start, a whole beat pair, is on the straight grid.
            if u % 2.0 < inc * BLOCK as f64 {
                assert!((got % 2.0).min(2.0 - got % 2.0) < 0.05, "swing {swing}: {got}");
            }
        }
    }
}
