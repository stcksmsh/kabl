//! Morphs of any parameter (`morph.rs`), as a snapshot recall sends them: `Command::Morph`
//! parts, explicit start values, taper, ramping, and every path that abandons a recall morph
//! landing it at its target. The audio-thread calls run under `assert_no_alloc`.

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::Collector;
use kabl_core::{ModuleId, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::morph::{MacroTarget, MorphPart, MorphTarget, MAX_DRIVEN, PART_TARGETS};
use kabl_engine::patch_engine::{Command, Launch, PatchEngine, Timing};
use kabl_engine::runtime::{Feedback, ParamSet, RuntimeTarget, ToAudio};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;
const CLOCK: ModuleId = 1;
const MA: ModuleId = 2;
const MB: ModuleId = 4;
const OUT: ModuleId = 3;
const JOIN: ModuleId = 90;
const LFO: ModuleId = 10;
/// First of `ATTS` attenuverters: two targets each (amount, offset), 160 in all.
const ATT0: ModuleId = 100;
const ATTS: usize = 80;
/// 120 bpm: a bar (16 pulses) is 96000 samples = 1500 blocks.
const BAR_BLOCKS: usize = 1500;

fn module(kind: &str, params: &[(&str, f32)]) -> ModuleState {
    ModuleState {
        kind: kind.into(),
        pos: Vec2::default(),
        params: params.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
    }
}

/// Clock, two macro modules, an LFO and the attenuverters; the first attenuverter (offset
/// only, no input) is the left output, the LFO the right.
fn patch(clock: bool) -> PatchState {
    let mut p = PatchState::new();
    if clock {
        p.modules.insert(CLOCK, module("clock", &[("bpm", 120.0)]));
    }
    p.modules
        .insert(MA, module("macro", &[("m1", 0.2), ("m2", 0.7)]));
    p.modules.insert(MB, module("macro", &[]));
    p.modules
        .insert(LFO, module("lfo", &[("rate_hz", 0.1), ("waveform", 0.0)]));
    for i in 0..ATTS {
        p.modules.insert(
            ATT0 + i as ModuleId,
            module("attenuverter", &[("amount", 1.0), ("offset", 0.0)]),
        );
    }
    p.modules.insert(OUT, module("out", &[]));
    p.modules.insert(JOIN, module("join", &[]));
    let m = |id, port: &str| PortRef::Module {
        id,
        port: port.into(),
    };
    for (i, (from, to)) in [
        (m(ATT0, "out"), m(JOIN, "left")),
        (m(LFO, "out"), m(JOIN, "right")),
        (m(JOIN, "out"), m(OUT, "in")),
    ]
    .into_iter()
    .enumerate()
    {
        p.cables.insert(
            i as u64 + 1,
            kabl_core::CableState {
                from,
                to,
                params: Default::default(),
                steps: Vec::new(),
            },
        );
    }
    p
}

/// Recall target `n`: attenuverter `n / 2`, amount (even) or offset (odd).
fn att(n: usize) -> RuntimeTarget {
    RuntimeTarget::Param {
        id: ATT0 + (n / 2) as ModuleId,
        kind: "attenuverter",
        index: (n % 2) as u16,
    }
}

fn mac(index: u16) -> RuntimeTarget {
    RuntimeTarget::Param {
        id: MA,
        kind: "macro",
        index,
    }
}

fn lfo(index: u16) -> RuntimeTarget {
    RuntimeTarget::Param {
        id: LFO,
        kind: "lfo",
        index,
    }
}

/// A target of `n` that its default does not hold: amount 1 -> -2 - n/100, offset 0 -> 0.3.
fn mt(n: usize) -> MorphTarget {
    let (from, to) = if n.is_multiple_of(2) {
        (1.0, -2.0 - n as f32 / 100.0)
    } else {
        (0.0, 0.3 + n as f32 / 1000.0)
    };
    MorphTarget {
        target: att(n),
        from,
        to,
    }
}

/// The parts of recall `seq` of `targets`, 8 a part, the last one marked.
fn parts(seq: u32, targets: &[MorphTarget], pulses: f32, clock: ModuleId) -> Vec<Command> {
    let chunks: Vec<_> = targets.chunks(PART_TARGETS).collect();
    chunks
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let mut t = [targets[0]; PART_TARGETS];
            t[..c.len()].copy_from_slice(c);
            Command::Morph(MorphPart {
                seq,
                last: i + 1 == chunks.len(),
                clock,
                pulses,
                count: c.len() as u8,
                targets: t,
            })
        })
        .collect()
}

struct Rig {
    e: PatchEngine,
    c: Collector,
    left: Vec<f32>,
}

impl Rig {
    fn new() -> Self {
        let c = Collector::new();
        let e = PatchEngine::new(&c.handle(), &patch(true), SR, 8).unwrap();
        let mut r = Rig {
            e,
            c,
            left: Vec::new(),
        };
        r.run(50);
        r.left.clear();
        r
    }

    fn run(&mut self, blocks: usize) {
        let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
        for _ in 0..blocks {
            assert_no_alloc(|| self.e.process_block(&mut l, &mut r));
            self.left.extend_from_slice(&l);
        }
    }

    fn send(&mut self, cs: &[Command]) {
        for c in cs {
            assert_no_alloc(|| self.e.command(c));
        }
    }

    fn recall(&mut self, seq: u32, targets: &[MorphTarget], pulses: f32) {
        self.send(&parts(seq, targets, pulses, CLOCK));
    }

    fn launch(&mut self, to: &[(ModuleId, u8, f32)], pulses: f32, timing: Timing) {
        let ms: Vec<_> = to
            .iter()
            .map(|&(id, index, value)| MacroTarget { id, index, value })
            .collect();
        let l = Launch::new(CLOCK, timing, &[]).with_macros(&ms, pulses);
        self.send(&[Command::Launch(l)]);
    }

    fn value(&mut self, t: RuntimeTarget) -> f32 {
        self.e.active_mut().runtime_value(t).unwrap()
    }

    /// `(target, value, to, progress)` of every driven param.
    fn report(&self) -> Vec<(RuntimeTarget, f32, f32, f32)> {
        let mut v = Vec::new();
        self.e.param_morphs(|t, x, to, p| v.push((t, x, to, p)));
        v
    }

    fn live(&self, t: RuntimeTarget) -> Option<(f32, f32)> {
        self.report()
            .into_iter()
            .find(|r| r.0 == t)
            .map(|r| (r.1, r.3))
    }

    fn graph(&self, clock: bool, rev: u64) -> basedrop::Owned<kabl_engine::compile::CompiledPatch> {
        let g = kabl_engine::compile::compile(&patch(clock), SR, 8).unwrap();
        let mut g = basedrop::Owned::new(&self.c.handle(), g);
        g.rev = rev;
        g
    }
}

fn close(a: f32, b: f32) -> bool {
    let ok = (a - b).abs() < 1e-5;
    if !ok {
        eprintln!("{a} != {b}");
    }
    ok
}

#[test]
fn command_is_not_larger_than_before() {
    // 544 bytes on origin/master before `Command::Morph`: `Launch` is the largest variant.
    assert!(std::mem::size_of::<Command>() <= 544);
}

#[test]
fn exponential_param_morphs_evenly_in_knob_travel() {
    let mut r = Rig::new();
    let p = &kabl_modules::builtins::LFO_INFO.params[0];
    let (from, to) = (0.1, 10.0);
    r.recall(
        1,
        &[MorphTarget {
            target: lfo(0),
            from,
            to,
        }],
        16.0,
    );
    let mut checked = 0;
    for _ in 0..BAR_BLOCKS - 10 {
        r.run(1);
        let (v, x) = r.live(lfo(0)).expect("driven");
        let want = p.to_norm(from) + (p.to_norm(to) - p.to_norm(from)) * x;
        assert!((p.to_norm(v) - want).abs() < 1e-4, "{v} at {x}");
        if (x - 0.5).abs() < 0.002 {
            // Half way in travel is the geometric mean, 1 Hz, not the linear 5.05 Hz.
            assert!((v - 1.0).abs() < 0.02, "mid {v}");
            checked += 1;
        }
    }
    assert!(checked > 0);
    r.run(30);
    assert!(close(r.value(lfo(0)), to));
}

#[test]
fn stepped_param_holds_until_half_way_then_jumps() {
    let mut r = Rig::new();
    r.recall(
        1,
        &[MorphTarget {
            target: lfo(1),
            from: 0.0,
            to: 3.0,
        }],
        16.0,
    );
    let mut jumped_at = None;
    for _ in 0..BAR_BLOCKS + 10 {
        r.run(1);
        let v = r.value(lfo(1));
        assert!(v == 0.0 || v == 3.0, "visited {v}");
        let x = r.live(lfo(1)).map_or(1.0, |l| l.1);
        if v == 3.0 && jumped_at.is_none() {
            jumped_at = Some(x);
        }
        if x < 0.5 {
            assert_eq!(v, 0.0, "held before half way, at {x}");
        }
    }
    let x = jumped_at.expect("jumped");
    assert!((0.5..0.51).contains(&x), "jumped at {x}");
    assert_eq!(r.value(lfo(1)), 3.0);
}

#[test]
fn an_explicit_from_is_used() {
    let mut r = Rig::new();
    assert_eq!(r.value(att(1)), 0.0);
    r.recall(
        1,
        &[MorphTarget {
            target: att(1),
            from: 0.6,
            to: 0.9,
        }],
        16.0,
    );
    r.run(1);
    let (v, x) = r.live(att(1)).unwrap();
    assert!(x < 0.01);
    assert!(
        (v - 0.6).abs() < 0.01,
        "starts from 0.6, not the stored 0: {v}"
    );
    r.run(BAR_BLOCKS / 2);
    let (v, _) = r.live(att(1)).unwrap();
    assert!(v > 0.6 && v < 0.9, "{v}");
    r.run(BAR_BLOCKS / 2 + 30);
    assert!(close(r.value(att(1)), 0.9));
}

#[test]
fn a_value_write_releases_the_morph_of_any_param() {
    let mut r = Rig::new();
    r.recall(1, &[mt(1)], 16.0);
    r.run(BAR_BLOCKS / 4);
    assert!(r.live(att(1)).is_some());
    let s = ParamSet {
        rev: 1,
        target: att(1),
        value: -0.5,
    };
    assert!(assert_no_alloc(|| r.e.set(&s)));
    r.run(BAR_BLOCKS);
    assert!(r.live(att(1)).is_none(), "released");
    assert!(close(r.value(att(1)), -0.5));
}

#[test]
fn an_unsmoothed_param_ramps_with_no_step_above_one_block_of_a_ramp() {
    // The attenuverter's offset (the left output) is stored at 0 and does not smooth itself.
    // The recall starts from 0.5, where the UI saw a morph: set at once, that would be one
    // step of 0.5. Ramped, the graph's ramps move once a block, so no rendered step is larger
    // than one block of a RAMP_MS ramp over that distance, plus the morph's own motion.
    let mut r = Rig::new();
    let (from, to) = (0.5, 0.6);
    r.recall(
        1,
        &[MorphTarget {
            target: att(1),
            from,
            to,
        }],
        16.0,
    );
    r.run(BAR_BLOCKS + 100);
    let ramp_blocks = (kabl_engine::compile::RAMP_MS / 1000.0 * SR / BLOCK as f32).round();
    let morph_block = (to - from) / BAR_BLOCKS as f32;
    let bound = from / ramp_blocks + 2.0 * morph_block;
    let steps: Vec<f32> = r.left.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
    let worst = steps.iter().copied().fold(0.0, f32::max);
    assert!(worst <= bound, "sample step {worst} above {bound}");
    assert!(
        steps.iter().filter(|&&s| s > 1e-3).count() >= ramp_blocks as usize - 1,
        "the start is spread over the ramp"
    );
    assert!((r.left[BLOCK * BAR_BLOCKS / 2] - 0.55).abs() < 0.01);
    assert!((r.left.last().unwrap() - to).abs() < 1e-5);
}

#[test]
fn a_recall_in_parts_starts_every_target_in_the_same_block() {
    let mut r = Rig::new();
    let targets: Vec<_> = (0..32).map(mt).collect();
    let cs = parts(1, &targets, 16.0, CLOCK);
    assert_eq!(cs.len(), 4);
    let (mut tx, mut rx) = rtrb::RingBuffer::<ToAudio>::new(8);
    let fb = Feedback::default();
    for c in &cs[..2] {
        tx.push(ToAudio::Command(*c)).ok().unwrap();
    }
    assert_no_alloc(|| r.e.drain(&mut rx, 8, &fb));
    r.run(20);
    assert!(r.report().is_empty(), "nothing starts on half a recall");
    for n in 0..32usize {
        let old = if n.is_multiple_of(2) { 1.0 } else { 0.0 };
        assert!(close(r.value(att(n)), old), "target {n} kept its value");
    }
    for c in &cs[2..] {
        tx.push(ToAudio::Command(*c)).ok().unwrap();
    }
    assert_no_alloc(|| r.e.drain(&mut rx, 8, &fb));
    r.run(1);
    let rep = r.report();
    assert_eq!(rep.len(), 32);
    assert!(rep.iter().all(|x| x.3 == rep[0].3), "one start block");
    r.run(BAR_BLOCKS + 30);
    for t in &targets {
        assert!(close(r.value(t.target), t.to));
    }
}

#[test]
fn thirty_two_recall_targets_and_sixteen_macros_run_together() {
    let mut r = Rig::new();
    let targets: Vec<_> = (0..32).map(mt).collect();
    let macros: Vec<_> = (0..16)
        .map(|k| (if k < 8 { MA } else { MB }, (k % 8) as u8, 0.9))
        .collect();
    r.launch(&macros, 16.0, Timing::Now);
    r.recall(1, &targets, 16.0);
    r.run(BAR_BLOCKS / 2);
    let rep = r.report();
    assert_eq!(rep.len(), 48);
    assert!(rep.iter().all(|x| x.3 > 0.3 && x.3 < 0.7), "all moving");
    r.run(BAR_BLOCKS / 2 + 30);
    for t in &targets {
        assert!(close(r.value(t.target), t.to));
    }
    for &(id, k, v) in &macros {
        let t = RuntimeTarget::Param {
            id,
            kind: "macro",
            index: k as u16,
        };
        assert!(close(r.value(t), v));
    }
}

// Abandon paths (`morph.rs`, "A recall morph always lands"): a recall target lands at `to`, a
// cue macro is not landed.

#[test]
fn clock_gone_lands_the_recall_and_not_the_macro() {
    let mut r = Rig::new();
    r.launch(&[(MA, 0, 0.9)], 16.0, Timing::NextBar);
    r.recall(1, &[mt(1)], 16.0);
    let g = r.graph(false, 5);
    r.e.receive_swap(g);
    r.run(BAR_BLOCKS + 300);
    assert!(close(r.value(att(1)), mt(1).to));
    assert!(close(r.value(mac(0)), 0.2));
    assert!(r.report().is_empty());
}

#[test]
fn full_pending_table_lands_the_recall_and_not_the_macro() {
    let mut r = Rig::new();
    let full: Vec<_> = (0..MAX_DRIVEN).map(mt).collect();
    r.recall(1, &full[..32], 16.0);
    r.recall(2, &full[32..], 16.0);
    r.launch(&[(MA, 0, 0.9)], 0.0, Timing::Now);
    let extra = mt(MAX_DRIVEN);
    r.recall(3, &[extra], 16.0);
    r.run(1);
    assert!(close(r.value(extra.target), extra.to), "landed at once");
    assert!(r.live(extra.target).is_none());
    r.run(BAR_BLOCKS);
    assert!(close(r.value(mac(0)), 0.2));
    assert!(close(r.value(extra.target), extra.to));
}

#[test]
fn full_driven_table_lands_the_recall_and_not_the_macro() {
    let mut r = Rig::new();
    let full: Vec<_> = (0..MAX_DRIVEN).map(mt).collect();
    r.recall(1, &full[..32], 64.0);
    r.recall(2, &full[32..], 64.0);
    r.run(2);
    assert_eq!(r.report().len(), MAX_DRIVEN);
    r.launch(&[(MA, 0, 0.9)], 0.0, Timing::Now);
    let extra = mt(MAX_DRIVEN);
    r.recall(3, &[extra], 16.0);
    r.run(1);
    assert!(close(r.value(extra.target), extra.to));
    assert!(close(r.value(mac(0)), 0.2));
    assert_eq!(r.report().len(), MAX_DRIVEN);
}

#[test]
fn replaced_seq_lands_the_unfinished_recall_and_not_the_macro() {
    let mut r = Rig::new();
    r.launch(&[(MA, 0, 1.0)], 16.0, Timing::Now);
    r.run(BAR_BLOCKS / 4);
    let mut first = parts(1, &(0..16).map(mt).collect::<Vec<_>>(), 16.0, CLOCK);
    first.pop(); // the set never finishes
    r.send(&first);
    r.recall(2, &[mt(40)], 16.0);
    r.run(1);
    for n in 0..8 {
        assert!(close(r.value(att(n)), mt(n).to), "target {n} landed");
    }
    assert!(r.live(att(0)).is_none());
    let (m, x) = r.live(mac(0)).unwrap();
    assert!(
        x > 0.2 && x < 0.3 && m < 0.5,
        "the macro morph goes on: {m} at {x}"
    );
    assert!(r.live(att(40)).is_some());
}

#[test]
fn reset_lands_the_recall_and_not_the_macro() {
    let mut r = Rig::new();
    r.launch(&[(MA, 0, 1.0)], 16.0, Timing::Now);
    r.recall(1, &[mt(1)], 16.0);
    r.run(BAR_BLOCKS / 4);
    let m = r.value(mac(0));
    assert!(m > 0.3 && m < 0.5);
    assert_no_alloc(|| r.e.reset());
    r.run(BAR_BLOCKS);
    assert!(close(r.value(att(1)), mt(1).to));
    assert!(close(r.value(mac(0)), m), "the macro stays where it was");
}

#[test]
fn cancel_lands_the_recall_and_not_the_macro() {
    let mut r = Rig::new();
    r.launch(&[(MA, 0, 0.9)], 16.0, Timing::NextBar);
    r.recall(1, &[mt(1)], 16.0);
    r.send(&[Command::Cancel(None)]);
    r.run(1);
    assert!(close(r.value(att(1)), mt(1).to));
    assert!(r.report().is_empty());
    r.run(BAR_BLOCKS + 10);
    assert!(close(r.value(mac(0)), 0.2));
}

#[test]
fn a_reset_lands_160_pending_driven_and_staged_recall_targets() {
    let mut r = Rig::new();
    let all: Vec<_> = (0..2 * ATTS).map(mt).collect();
    r.recall(1, &all[..32], 64.0);
    r.recall(2, &all[32..64], 64.0);
    r.run(2);
    assert_eq!(r.report().len(), 64, "64 driven");
    r.recall(3, &all[64..96], 64.0);
    r.recall(4, &all[96..128], 64.0);
    // All four parts of seq 5 with none marked last: 32 staged.
    let staged: Vec<_> = parts(5, &all[128..], 64.0, CLOCK)
        .into_iter()
        .map(|c| match c {
            Command::Morph(p) => Command::Morph(MorphPart { last: false, ..p }),
            c => c,
        })
        .collect();
    r.send(&staged);
    assert_no_alloc(|| r.e.reset());
    r.run(1);
    for (n, t) in all.iter().enumerate() {
        assert!(close(r.value(t.target), t.to), "target {n} landed");
    }
    assert!(r.report().is_empty());
}

#[test]
fn finished_recalls_free_their_slots_for_later_morphs() {
    let mut r = Rig::new();
    let full: Vec<_> = (0..MAX_DRIVEN).map(mt).collect();
    r.recall(1, &full[..32], 16.0);
    r.recall(2, &full[32..], 16.0);
    r.run(BAR_BLOCKS + 100);
    assert!(close(r.value(att(0)), mt(0).to));
    assert!(r.report().is_empty(), "finished recalls hold no slot");
    r.launch(&[(MA, 0, 0.9)], 16.0, Timing::Now);
    let third = mt(MAX_DRIVEN);
    r.recall(3, &[third], 16.0);
    r.run(BAR_BLOCKS / 2);
    let m = r.value(mac(0));
    assert!(m > 0.3 && m < 0.8, "the macro morphs: {m}");
    let v = r.value(third.target);
    let (lo, hi) = (third.to.min(third.from), third.to.max(third.from));
    assert!(
        v > lo + 0.1 * (hi - lo) && v < hi - 0.1 * (hi - lo),
        "the third recall morphs: {v}"
    );
    r.run(BAR_BLOCKS);
    assert!(close(r.value(mac(0)), 0.9));
    assert!(close(r.value(third.target), third.to));
}

#[test]
fn a_load_does_not_write_a_finished_recall_into_the_new_patch() {
    let mut r = Rig::new();
    r.recall(1, &[mt(1)], 16.0);
    r.run(BAR_BLOCKS + 100);
    assert!(close(r.value(att(1)), mt(1).to));
    // Doc B has the same attenuverter id, its offset stored at 0.
    let mut b = r.graph(true, 5);
    b.fresh = true;
    r.e.receive_swap(b);
    r.run(200);
    assert!(close(r.value(att(1)), 0.0), "B keeps its own value");
}

#[test]
fn a_recall_behind_a_fresh_load_is_dropped_like_cue_macros() {
    let mut r = Rig::new();
    // An edit fading in, then a Load waiting behind it.
    let edit = r.graph(true, 5);
    r.e.receive_swap(edit);
    let mut b = r.graph(true, 6);
    b.fresh = true;
    r.e.receive_swap(b);
    r.launch(&[(MA, 0, 0.9)], 16.0, Timing::Now);
    r.recall(1, &[mt(1)], 16.0);
    r.run(1);
    assert!(r.report().is_empty(), "nothing is scheduled");
    r.run(BAR_BLOCKS + 300);
    assert!(
        close(r.value(att(1)), 0.0),
        "the loaded graph keeps its value"
    );
    assert!(close(r.value(mac(0)), 0.2));
    assert!(r.report().is_empty());
}
