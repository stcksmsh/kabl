//! The arpeggiator, against what a player expects to hear: the order of the notes in every mode,
//! what happens when keys are added and let go, latch and pedal, gate length, ratchets, and a
//! render that repeats.

mod common;
use common::*;
use kabl_modules::builtins::Arp;

const STEP: usize = 6144;

struct Harness {
    r: Rig,
    t: usize,
}

impl Harness {
    fn new(set: &[(&str, f32)]) -> Self {
        let mut r = Rig::new("arp", set, SR);
        r.m.as_any_mut().downcast_mut::<Arp>().unwrap().seed(7);
        Harness { r, t: 0 }
    }

    fn arp(&mut self) -> &mut Arp {
        self.r.m.as_any_mut().downcast_mut::<Arp>().unwrap()
    }

    fn on(&mut self, notes: &[u8]) {
        for &n in notes {
            self.arp().note_on(n, 100);
        }
    }

    fn off(&mut self, notes: &[u8]) {
        for &n in notes {
            self.arp().note_off(n);
        }
    }

    /// Plays `steps` clock steps and returns the pitch heard at the start of each step, `None`
    /// for a step with no gate.
    fn steps(&mut self, steps: usize) -> Vec<Option<i32>> {
        let mut out = vec![];
        for _ in 0..steps {
            let mut got = None;
            for b in 0..STEP / BLOCK {
                let t0 = self.t;
                let clock: Vec<f32> = (0..BLOCK)
                    .map(|i| ((t0 + i) % STEP < STEP / 2) as u8 as f32)
                    .collect();
                let o = self.r.block(&[&clock]);
                if b == 0 {
                    // 1 sample into the step the edge has been seen: gate and pitch are valid.
                    if o[0][1] > 0.5 {
                        got = Some(o[1][1].round() as i32);
                    }
                }
                self.t += BLOCK;
            }
            out.push(got);
        }
        out
    }
}

fn pitches(v: Vec<Option<i32>>) -> Vec<i32> {
    v.into_iter().map(|x| x.expect("a note")).collect()
}

const CHORD: [u8; 3] = [60, 64, 67];

#[test]
fn modes_play_their_order() {
    let cases: [(f32, f32, &[i32]); 6] = [
        (0.0, 1.0, &[0, 4, 7, 0, 4, 7]),
        (0.0, 2.0, &[0, 4, 7, 12, 16, 19, 0, 4]),
        (1.0, 1.0, &[7, 4, 0, 7, 4, 0]),
        (1.0, 2.0, &[19, 16, 12, 7, 4, 0, 19]),
        (2.0, 1.0, &[0, 4, 7, 4, 0, 4, 7, 4]),
        (2.0, 2.0, &[0, 4, 7, 12, 16, 19, 16, 12, 7, 4, 0, 4]),
    ];
    for (mode, oct, want) in cases {
        let mut h = Harness::new(&[("mode", mode), ("octaves", oct)]);
        h.on(&CHORD);
        let got = pitches(h.steps(want.len()));
        assert_eq!(got, want, "mode {mode} octaves {oct}");
    }
}

#[test]
fn played_follows_the_order_pressed_in_each_octave() {
    let mut h = Harness::new(&[("mode", 3.0), ("octaves", 2.0)]);
    h.on(&[67, 60, 64]);
    assert_eq!(
        pitches(h.steps(8)),
        [7, 0, 4, 19, 12, 16, 7, 0],
        "G C E then an octave up"
    );
}

#[test]
fn one_key_repeats_and_no_key_is_silent() {
    let mut h = Harness::new(&[("mode", 2.0), ("octaves", 1.0)]);
    assert!(h.steps(2).iter().all(|x| x.is_none()));
    h.on(&[62]);
    assert_eq!(pitches(h.steps(4)), [2, 2, 2, 2]);
    h.off(&[62]);
    assert!(h.steps(2).iter().all(|x| x.is_none()));
}

#[test]
fn a_note_added_mid_pattern_joins_in_order_and_a_released_one_leaves() {
    let mut h = Harness::new(&[]);
    h.on(&[60, 67]);
    assert_eq!(pitches(h.steps(3)), [0, 7, 0]);
    // Playing 0; add E (4): the next note after 0 is now 4, then 7.
    h.on(&[64]);
    assert_eq!(pitches(h.steps(3)), [4, 7, 0]);
    // Release the middle one: 0, 7 again.
    h.off(&[64]);
    assert_eq!(pitches(h.steps(3)), [7, 0, 7]);
}

#[test]
fn letting_go_of_the_last_key_drops_the_gate_at_once_and_the_next_chord_starts_over() {
    let mut h = Harness::new(&[]);
    h.on(&CHORD);
    let _ = h.steps(2); // 0, 4: now in the third step's first block... after two steps
                        // Within a step: release all, the very next block has no gate.
    h.off(&CHORD);
    let clock = vec![0.0f32; BLOCK];
    let o = h.r.block(&[&clock]);
    assert!(o[0].iter().all(|&g| g < 0.5), "gate {:?}", &o[0][..4]);
    // A new chord: first note of the pattern on the next edge, never between edges.
    h.on(&[64, 67]);
    let o = h.r.block(&[&clock]);
    assert!(o[0].iter().all(|&g| g < 0.5), "no note before the edge");
    h.t = 0; // align the harness clock to a fresh pulse
    assert_eq!(pitches(h.steps(2)), [4, 7]);
}

#[test]
fn latch_keeps_the_chord_and_a_new_press_replaces_it() {
    let mut h = Harness::new(&[("latch", 1.0)]);
    let _ = h.steps(1); // the arp reads the latch knob once per block
    h.on(&CHORD);
    h.off(&CHORD);
    assert_eq!(
        pitches(h.steps(4)),
        [0, 4, 7, 0],
        "keeps playing after release"
    );
    // Nothing down: a press starts a new chord.
    h.on(&[62]);
    assert_eq!(pitches(h.steps(2)), [2, 2]);
    h.off(&[62]);
    // A press while another is down adds to it.
    h.on(&[60]);
    h.on(&[67]);
    h.off(&[60, 67]);
    let got = pitches(h.steps(4));
    assert!(
        got.contains(&0) && got.contains(&7) && !got.contains(&2),
        "{got:?}"
    );
    // Latch off while released: it stops.
    h.r.set("latch", 0.0);
    assert!(h.steps(2).iter().all(|x| x.is_none()));
}

#[test]
fn the_sustain_pedal_holds_released_keys_until_it_comes_up() {
    let mut h = Harness::new(&[]);
    h.arp().sustain(true);
    h.on(&[60, 64]);
    h.off(&[60, 64]);
    assert_eq!(pitches(h.steps(3)), [0, 4, 0]);
    h.arp().sustain(false);
    assert!(h.steps(2).iter().all(|x| x.is_none()));
}

#[test]
fn random_is_seeded_repeats_on_reset_and_stays_in_the_chord() {
    let run = |seed: u64| {
        let mut h = Harness::new(&[("mode", 4.0), ("octaves", 2.0)]);
        h.arp().seed(seed);
        h.on(&CHORD);
        pitches(h.steps(24))
    };
    let (a, b, c) = (run(1), run(1), run(2));
    assert_eq!(a, b, "same seed, same render");
    assert_ne!(a, c, "another module id plays another line");
    let allowed = [0, 4, 7, 12, 16, 19];
    assert!(a.iter().all(|p| allowed.contains(p)), "{a:?}");
    assert!(
        allowed.iter().all(|p| a.contains(p)),
        "covers the span: {a:?}"
    );
}

#[test]
fn gate_length_and_ratchets_are_fractions_of_the_step() {
    let high = |set: &[(&str, f32)]| {
        let mut h = Harness::new(set);
        h.on(&[60]);
        let _ = h.steps(3); // the step length is known from the third edge on
        let mut gates = vec![];
        for _ in 0..STEP / BLOCK {
            let t0 = h.t;
            let clock: Vec<f32> = (0..BLOCK)
                .map(|i| ((t0 + i) % STEP < STEP / 2) as u8 as f32)
                .collect();
            gates.extend_from_slice(&h.r.block(&[&clock])[0]);
            h.t += BLOCK;
        }
        let edges = (1..gates.len())
            .filter(|&i| gates[i] > 0.5 && gates[i - 1] < 0.5)
            .count()
            + (gates[0] > 0.5) as usize;
        (gates.iter().filter(|&&g| g > 0.5).count(), edges)
    };
    assert_eq!(high(&[("gate_len", 25.0)]), (1536, 1));
    assert_eq!(high(&[("gate_len", 50.0), ("ratchet", 3.0)]), (3072, 3));
    // 100 %: still one drop per note so each retriggers.
    let (n, edges) = high(&[("gate_len", 100.0), ("ratchet", 2.0)]);
    assert_eq!(edges, 2);
    assert!(n < STEP && n > STEP - 10, "{n}");
}

#[test]
fn reset_starts_the_pattern_over() {
    let mut h = Harness::new(&[]);
    h.on(&CHORD);
    assert_eq!(pitches(h.steps(2)), [0, 4]);
    // A reset between pulses: the next edge plays the first note again.
    let clock = vec![0.0f32; BLOCK];
    let reset = vec![1.0f32; BLOCK];
    let _ = h.r.block(&[&clock, &reset]);
    h.t = 0;
    assert_eq!(pitches(h.steps(2)), [0, 4]);
}

#[test]
fn the_state_carries_to_a_new_graph() {
    let mut h = Harness::new(&[]);
    h.on(&CHORD);
    let _ = h.steps(2);
    let mut new = Rig::new("arp", &[], SR);
    new.m.carry_from(&*h.r.m);
    assert_eq!(
        new.m.as_any().downcast_ref::<Arp>().unwrap().held().count(),
        3
    );
}

#[test]
fn rate_plays_every_nth_clock_pulse() {
    // 1/8 (every second pulse): a note, then a pulse with no gate.
    let mut h = Harness::new(&[("rate", 1.0)]);
    h.on(&CHORD);
    let got = h.steps(8);
    let want = [Some(0), None, Some(4), None, Some(7), None, Some(0), None];
    assert_eq!(got, want);
    // 1/4: one note in four pulses.
    let mut h = Harness::new(&[("rate", 3.0)]);
    h.on(&CHORD);
    let got = h.steps(9);
    // The gate is half a step long, so it spans two pulses once the step is known.
    assert!(got[1..4].iter().all(|x| x.is_none()), "{got:?}");
    assert_eq!(got[0], Some(0));
    assert_eq!(got[4], Some(4));
    assert_eq!(got[8], Some(7));
}

#[test]
fn a_new_chord_after_silence_plays_on_the_next_pulse_at_any_rate() {
    let mut h = Harness::new(&[("rate", 4.0)]);
    h.on(&[60]);
    let _ = h.steps(3);
    h.off(&[60]);
    let _ = h.steps(3);
    h.on(&[64]);
    assert_eq!(h.steps(1), [Some(4)], "not up to eight pulses late");
}

#[test]
fn channel_selects_which_events_it_hears() {
    let mut h = Harness::new(&[("channel", 3.0)]);
    let _ = h.steps(1);
    assert!(h.arp().hears(2) && !h.arp().hears(0) && !h.arp().hears(15));
    let mut h = Harness::new(&[]);
    let _ = h.steps(1);
    assert!((0..16).all(|c| h.arp().hears(c)), "default hears all");
}
