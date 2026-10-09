//! Functional cables: a cable can carry a step pattern and a pass probability, locked to the
//! patch clock. Settings live in the cable's params (so undo, save and the runtime-control
//! path already carry them); this crate is the model and the per-block node, with no engine
//! types. Semantics and rejected alternatives: docs/decisions.md, "Functional cables".
//!
//! Per clock pulse (tick `t` since the last Restart) the cable picks step `t % length`, draws
//! once from a hash of (cable, `t`), and opens at that step's level (`s<k>`) or closes. A
//! rejected step is level 0. One draw per tick whatever the settings, so editing a value never
//! shifts the draws, and a Restart (tick 0 again) replays the same sequence.

use std::collections::BTreeMap;

pub const MAX_STEPS: usize = 16;
/// Pattern length, 0 (none) to `MAX_STEPS`.
pub const LENGTH: &str = "length";
/// Whole-cable pass probability, percent (absent = 100).
pub const PROB: &str = "prob";
/// Runtime slots: 0 length, 1 prob, step levels, step chances, then the morph slots.
pub const LEVEL0: usize = 2;
pub const CHANCE0: usize = LEVEL0 + MAX_STEPS;
/// 0..1: how far the cable is from pattern A (0) to pattern B (1).
pub const MORPH: usize = CHANCE0 + MAX_STEPS;
/// Level glide in ms (0 = steps jump; audio always slews 1 ms at least).
pub const GLIDE: usize = MORPH + 1;
pub const B_LENGTH: usize = GLIDE + 1;
pub const B_LEVEL0: usize = B_LENGTH + 1;
pub const B_CHANCE0: usize = B_LEVEL0 + MAX_STEPS;
pub const SLOTS: usize = B_CHANCE0 + MAX_STEPS;

/// Param name of runtime slot `slot`: `length`, `prob`, `s1..`, `r1..`, `morph`, `glide_ms`,
/// then pattern B as `b.length`, `b.s1..`, `b.r1..`.
pub fn slot_name(slot: usize) -> String {
    match slot {
        0 => LENGTH.into(),
        1 => PROB.into(),
        s if s < CHANCE0 => format!("s{}", s - LEVEL0 + 1),
        s if s < MORPH => format!("r{}", s - CHANCE0 + 1),
        MORPH => "morph".into(),
        GLIDE => "glide_ms".into(),
        B_LENGTH => "b.length".into(),
        s if s < B_CHANCE0 => format!("b.s{}", s - B_LEVEL0 + 1),
        s => format!("b.r{}", s - B_CHANCE0 + 1),
    }
}

fn is_level(slot: usize) -> bool {
    (LEVEL0..CHANCE0).contains(&slot) || (B_LEVEL0..B_CHANCE0).contains(&slot)
}

fn slot_default(slot: usize) -> f32 {
    match slot {
        0 | MORPH | GLIDE | B_LENGTH => 0.0,
        s if is_level(s) => 1.0,
        _ => 100.0,
    }
}

fn clamp_slot(slot: usize, v: f32) -> f32 {
    if !v.is_finite() {
        return slot_default(slot);
    }
    match slot {
        0 | B_LENGTH => v.round().clamp(0.0, MAX_STEPS as f32),
        MORPH => v.clamp(0.0, 1.0),
        GLIDE => v.clamp(0.0, 2000.0),
        s if is_level(s) => v.clamp(0.0, 1.0),
        _ => v.clamp(0.0, 100.0),
    }
}

/// The effective value of runtime slot `slot` for a cable with `params`.
pub fn slot_value(params: &BTreeMap<String, f32>, slot: usize) -> f32 {
    params
        .get(&slot_name(slot))
        .map_or(slot_default(slot), |&v| clamp_slot(slot, v))
}

/// A cable with a pattern, a pass probability below 100 % or a morph above 0 has a node in the compiled graph;
/// any other cable is a plain connection exactly as before.
pub fn is_functional(params: &BTreeMap<String, f32>) -> bool {
    slot_value(params, 0) >= 1.0 || slot_value(params, 1) < 100.0 || slot_value(params, MORPH) > 0.0
}

/// A cable's seed from its id.
pub fn seed_of(cable: u64) -> u32 {
    (cable ^ (cable >> 32)) as u32
}

/// What a closed step does to the signal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carry {
    /// Multiply by the level, hard edges (gates, CV, and route sources).
    Scale,
    /// Multiply by the level, with a short slew so a closing step does not click.
    Audio,
    /// A pitch cannot be scaled: a step at level 0.5 or more passes it, otherwise the last
    /// passed pitch is held.
    Hold,
}

/// Declick slew of `Carry::Audio`, ms for a full 0..1 swing.
pub const SLEW_MS: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    len: u8,
    len_b: u8,
    prob: f32,
    morph: f32,
    glide_ms: f32,
    level: [f32; MAX_STEPS],
    chance: [f32; MAX_STEPS],
    level_b: [f32; MAX_STEPS],
    chance_b: [f32; MAX_STEPS],
}

impl Settings {
    pub fn from_params(params: &BTreeMap<String, f32>) -> Self {
        let mut s = Settings {
            len: 0,
            len_b: 0,
            prob: 1.0,
            morph: 0.0,
            glide_ms: 0.0,
            level: [1.0; MAX_STEPS],
            chance: [1.0; MAX_STEPS],
            level_b: [1.0; MAX_STEPS],
            chance_b: [1.0; MAX_STEPS],
        };
        for slot in 0..SLOTS {
            s.set(slot, slot_value(params, slot));
        }
        s
    }

    /// Sets runtime slot `slot`. No allocation.
    pub fn set(&mut self, slot: usize, v: f32) {
        if slot >= SLOTS {
            return;
        }
        let v = clamp_slot(slot, v);
        match slot {
            0 => self.len = v as u8,
            1 => self.prob = v / 100.0,
            MORPH => self.morph = v,
            GLIDE => self.glide_ms = v,
            B_LENGTH => self.len_b = v as u8,
            s if s < CHANCE0 => self.level[s - LEVEL0] = v,
            s if s < MORPH => self.chance[s - CHANCE0] = v / 100.0,
            s if s < B_CHANCE0 => self.level_b[s - B_LEVEL0] = v,
            s => self.chance_b[s - B_CHANCE0] = v / 100.0,
        }
    }

    /// The level the cable has from pulse `tick` on: the step's level, or 0 when it is
    /// rejected. A cable with only a probability is a one-step pattern at level 1. With a
    /// morph above 0 pattern B plays beside A at its own length: the pass chance and the level
    /// are the blend of the two steps, drawn once, so a half morph is a real in-between.
    pub fn level_at(&self, seed: u32, tick: u64) -> f32 {
        let k = (tick % self.len.max(1) as u64) as usize;
        let (mut p, mut level) = (self.prob * self.chance[k], self.level[k]);
        if self.morph > 0.0 {
            let kb = (tick % self.len_b.max(1) as u64) as usize;
            let m = self.morph;
            p += (self.prob * self.chance_b[kb] - p) * m;
            level += (self.level_b[kb] - level) * m;
        }
        if draw(seed, tick) < p {
            level
        } else {
            0.0
        }
    }
}

/// Uniform in [0, 1) from (seed, tick): splitmix64 finalizer, so no stream state to carry.
fn draw(seed: u32, tick: u64) -> f32 {
    let mut x = ((seed as u64) << 32 ^ tick).wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x >> 40) as f32 / (1u32 << 24) as f32
}

/// One cable's node. Its only state is the slewed gain, the held pitch and the last level.
#[derive(Clone, Copy, Debug)]
pub struct Node {
    pub settings: Settings,
    seed: u32,
    carry: Carry,
    slew: f32,
    sample_rate: f32,
    level: f32,
    gain: f32,
    held: f32,
    started: bool,
}

impl Node {
    pub fn new(settings: Settings, cable: u64, carry: Carry, sample_rate: f32) -> Self {
        Node {
            settings,
            seed: seed_of(cable),
            carry,
            slew: 1.0 / (SLEW_MS * 0.001 * sample_rate).max(1.0),
            sample_rate,
            level: 1.0,
            gain: 1.0,
            held: 0.0,
            started: false,
        }
    }

    /// One block. `current` is the pulse in effect before the block's first sample (`None`
    /// before the first pulse, or when the pulse before belongs to the previous epoch: the
    /// level then carries over). `ticks` are the pulses the block starts, `(epoch, tick,
    /// sample offset)` as `Clock::block_ticks`; a pulse changes the level at its exact sample.
    pub fn process(
        &mut self,
        src: &[f32],
        out: &mut [f32],
        current: Option<u64>,
        ticks: &[(u32, u64, u32)],
    ) {
        if let Some(t) = current {
            self.level = self.settings.level_at(self.seed, t);
        }
        let n = out.len().min(src.len());
        let (mut i, mut next) = (0, 0);
        while i < n {
            while next < ticks.len() && ticks[next].2 as usize <= i {
                self.level = self.settings.level_at(self.seed, ticks[next].1);
                next += 1;
            }
            // The level is constant up to the next pulse.
            let end = ticks.get(next).map_or(n, |t| (t.2 as usize).min(n));
            if !self.started {
                (self.gain, self.held, self.started) = (self.level, src[i], true);
            }
            let (src, out) = (&src[i..end], &mut out[i..end]);
            let glide = self.settings.glide_ms;
            let slew = if glide > 0.0 {
                1.0 / (glide * 0.001 * self.sample_rate).max(1.0)
            } else {
                self.slew
            };
            match self.carry {
                Carry::Scale if glide <= 0.0 => {
                    self.gain = self.level;
                    for (o, &x) in out.iter_mut().zip(src) {
                        *o = x * self.level;
                    }
                }
                Carry::Scale | Carry::Audio => {
                    let mut k = 0;
                    while k < out.len() && self.gain != self.level {
                        let d = self.level - self.gain;
                        self.gain += d.clamp(-slew, slew);
                        out[k] = src[k] * self.gain;
                        k += 1;
                    }
                    for (o, &x) in out[k..].iter_mut().zip(&src[k..]) {
                        *o = x * self.gain;
                    }
                }
                Carry::Hold => {
                    if self.level >= 0.5 {
                        out.copy_from_slice(src);
                        self.held = src.last().copied().unwrap_or(self.held);
                    } else {
                        out.fill(self.held);
                    }
                }
            }
            i = end;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(kv: &[(&str, f32)]) -> BTreeMap<String, f32> {
        kv.iter().map(|&(k, v)| (k.to_string(), v)).collect()
    }

    fn node(kv: &[(&str, f32)], carry: Carry) -> Node {
        Node::new(Settings::from_params(&params(kv)), 7, carry, 48000.0)
    }

    #[test]
    fn plain_cable_is_not_functional() {
        assert!(!is_functional(&params(&[("amount", 0.5), ("bypass", 0.0)])));
        assert!(is_functional(&params(&[("length", 4.0)])));
        assert!(is_functional(&params(&[("prob", 99.0)])));
    }

    #[test]
    fn level_changes_on_the_exact_sample() {
        let mut n = node(&[("length", 2.0), ("s2", 0.0)], Carry::Scale);
        let src = [1.0; 64];
        let mut out = [9.0; 64];
        // pulse 0 at sample 10, pulse 1 at sample 40
        n.process(&src, &mut out, None, &[(0, 0, 10), (0, 1, 40)]);
        assert!(out[..10].iter().all(|&x| x == 1.0), "open before any pulse");
        assert!(out[10..40].iter().all(|&x| x == 1.0));
        assert!(out[40..].iter().all(|&x| x == 0.0));
        // next block starts in pulse 1: still closed, reopens on pulse 2 at sample 5
        n.process(&src, &mut out, Some(1), &[(0, 2, 5)]);
        assert!(out[..5].iter().all(|&x| x == 0.0));
        assert!(out[5..].iter().all(|&x| x == 1.0));
    }

    #[test]
    fn draws_repeat_per_tick_and_follow_probability() {
        let s = Settings::from_params(&params(&[("prob", 50.0)]));
        let a: Vec<f32> = (0..2000).map(|t| s.level_at(3, t)).collect();
        let b: Vec<f32> = (0..2000).map(|t| s.level_at(3, t)).collect();
        assert_eq!(a, b);
        let open = a.iter().filter(|&&x| x > 0.0).count();
        assert!((900..1100).contains(&open), "{open} of 2000 open at 50 %");
        let c: Vec<f32> = (0..2000).map(|t| s.level_at(4, t)).collect();
        assert_ne!(a, c, "another cable draws another sequence");
    }

    #[test]
    fn probability_edges_are_exact() {
        let always = Settings::from_params(&params(&[("length", 3.0)]));
        let never = Settings::from_params(&params(&[("prob", 0.0)]));
        assert!((0..5000).all(|t| always.level_at(1, t) == 1.0));
        assert!((0..5000).all(|t| never.level_at(1, t) == 0.0));
    }

    #[test]
    fn editing_a_value_does_not_shift_other_draws() {
        let a = Settings::from_params(&params(&[("length", 4.0), ("r1", 50.0), ("r2", 50.0)]));
        let b = Settings::from_params(&params(&[("length", 4.0), ("r1", 50.0), ("r2", 80.0)]));
        for t in (0..400).filter(|t| t % 4 != 1) {
            assert_eq!(a.level_at(9, t), b.level_at(9, t));
        }
    }

    #[test]
    fn audio_closes_with_a_slew_not_a_step() {
        let mut n = node(&[("length", 2.0), ("s2", 0.0)], Carry::Audio);
        let src = [1.0; 64];
        let mut out = [0.0; 64];
        n.process(&src, &mut out, None, &[(0, 0, 0), (0, 1, 8)]);
        assert_eq!(out[7], 1.0);
        assert!(out[8] > 0.9 && out[8] < 1.0);
        assert!(out.windows(2).all(|w| (w[1] - w[0]).abs() < 0.03));
        assert_eq!(out[63], 0.0);
    }

    #[test]
    fn pitch_holds_while_closed() {
        let mut n = node(&[("length", 2.0), ("s2", 0.0)], Carry::Hold);
        let src: Vec<f32> = (0..64).map(|i| i as f32).collect();
        let mut out = [0.0; 64];
        n.process(&src, &mut out, None, &[(0, 0, 0), (0, 1, 10)]);
        assert_eq!(out[9], 9.0);
        assert!(out[10..].iter().all(|&x| x == 9.0));
    }

    #[test]
    fn morph_blends_two_patterns_of_different_lengths() {
        let base = [
            ("length", 2.0),
            ("s2", 0.0),
            ("b.length", 3.0),
            ("b.s1", 0.5),
        ];
        let at = |m: f32| {
            let mut kv = base.to_vec();
            kv.push(("morph", m));
            Settings::from_params(&params(&kv))
        };
        let (a, half, b) = (at(0.0), at(0.5), at(1.0));
        // Morph 0 is pattern A alone, morph 1 is pattern B alone.
        let a_only = Settings::from_params(&params(&base[..2]));
        assert!((0..60).all(|t| a.level_at(1, t) == a_only.level_at(1, t)));
        assert_eq!(b.level_at(1, 0), 0.5);
        assert_eq!(b.level_at(1, 1), 1.0);
        // Halfway the levels are the mean of the two steps.
        assert_eq!(half.level_at(1, 1), 0.5);
        assert_eq!(half.level_at(1, 3), 0.25); // A step 2 is 0, B step 1 is 0.5
        assert!(is_functional(&params(&[("morph", 0.1)])));
    }

    #[test]
    fn morph_blends_chances_so_the_middle_is_in_between() {
        let at =
            |m: f32| Settings::from_params(&params(&[("r1", 0.0), ("b.r1", 100.0), ("morph", m)]));
        let open = |s: &Settings| (0..4000).filter(|&t| s.level_at(5, t) > 0.0).count();
        assert_eq!(open(&at(0.0)), 0);
        assert_eq!(open(&at(1.0)), 4000);
        let mid = open(&at(0.5));
        assert!(
            (1800..2200).contains(&mid),
            "{mid} of 4000 open at half morph"
        );
        let (q, r) = (open(&at(0.25)), open(&at(0.75)));
        assert!(q < mid && mid < r);
    }

    #[test]
    fn glide_ramps_a_cv_level_instead_of_stepping() {
        let mut n = node(
            &[("length", 2.0), ("s2", 0.0), ("glide_ms", 1.0)],
            Carry::Scale,
        );
        let (src, mut out) = ([1.0; 64], [0.0; 64]);
        n.process(&src, &mut out, None, &[(0, 0, 0), (0, 1, 8)]);
        assert_eq!(out[7], 1.0);
        assert!(
            out[8] > 0.9 && out[8] < 1.0,
            "ramp starts at the pulse sample"
        );
        assert_eq!(out[63], 0.0);
        let mut hard = node(&[("length", 2.0), ("s2", 0.0)], Carry::Scale);
        hard.process(&src, &mut out, None, &[(0, 0, 0), (0, 1, 8)]);
        assert_eq!(out[8], 0.0);
    }

    #[test]
    fn hostile_values_are_clamped() {
        let s = Settings::from_params(&params(&[
            ("length", 99.0),
            ("prob", f32::NAN),
            ("s1", 7.0),
            ("r1", -3.0),
        ]));
        assert_eq!(s.len as usize, MAX_STEPS);
        assert_eq!(s.prob, 1.0);
        assert_eq!(s.level[0], 1.0);
        assert_eq!(s.chance[0], 0.0);
    }
}
