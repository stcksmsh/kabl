//! Clock transport and `clock.div` timing, sample by sample, without the UI or the engine.
//! The rig is `clock` → `seq` A directly and `clock` → `clock.div` → `seq` B, with `clock.reset`
//! patched into the divider's and both sequencers' reset inputs. Both sequencers' steps are
//! tuned 0, 1, .. 7 semitones, so their pitch output is the step number.

use kabl_modules::builtins::{Clock, ClockDiv, Seq, Transport};
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{Module, ProcessIo, Signal};

const SR: f32 = 48000.0;
const STEP: usize = 6000; // samples per 16th note at 120 bpm
const HALF: usize = STEP / 2;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
struct Frame {
    clock: bool,
    reset: bool,
    div: bool,
    a_gate: bool,
    a_step: f32,
    b_gate: bool,
    b_step: f32,
}

enum Event {
    Cmd(Transport),
    Div(f32),
}

struct Rig {
    clock: Clock,
    div: ClockDiv,
    a: Seq,
    b: Seq,
    division: f32,
    seq_params: Vec<f32>,
    frames: Vec<Frame>,
}

impl Rig {
    fn new(division: f32) -> Self {
        let q = QualityConfig {
            tier: QualityTier::Live,
        };
        let mut r = Rig {
            clock: Clock::new(),
            div: ClockDiv::new(),
            a: Seq::new(),
            b: Seq::new(),
            division,
            seq_params: Seq::new()
                .info()
                .params
                .iter()
                .enumerate()
                .map(|(i, p)| if i < 8 { i as f32 } else { p.default })
                .collect(),
            frames: Vec::new(),
        };
        r.clock.prepare(SR, 64, &q);
        r.div.prepare(SR, 64, &q);
        r.a.prepare(SR, 64, &q);
        r.b.prepare(SR, 64, &q);
        r
    }

    /// One block of `n` samples through the whole rig.
    fn block(&mut self, n: usize) {
        let (mut clk, mut rst) = (vec![0f32; n], vec![0f32; n]);
        let mut outs: [&mut [f32]; 2] = [&mut clk, &mut rst];
        self.clock.process(&mut ProcessIo::new(
            &[],
            &mut outs,
            &[Signal::Scalar(120.0)],
            n,
        ));

        let mut dg = vec![0f32; n];
        let mut outs: [&mut [f32]; 1] = [&mut dg];
        let ins = [Signal::Buffer(&clk), Signal::Buffer(&rst)];
        self.div.process(&mut ProcessIo::new(
            &ins,
            &mut outs,
            &[Signal::Scalar(self.division)],
            n,
        ));

        let params: Vec<Signal> = self.seq_params.iter().map(|&v| Signal::Scalar(v)).collect();
        let run = |seq: &mut Seq, clock: &[f32]| {
            let (mut g, mut p) = (vec![0f32; n], vec![0f32; n]);
            let mut outs: [&mut [f32]; 2] = [&mut g, &mut p];
            let ins = [Signal::Buffer(clock), Signal::Buffer(&rst)];
            seq.process(&mut ProcessIo::new(&ins, &mut outs, &params, n));
            (g, p)
        };
        let (ag, ap) = run(&mut self.a, &clk);
        let (bg, bp) = run(&mut self.b, &dg);
        for i in 0..n {
            self.frames.push(Frame {
                clock: clk[i] > 0.5,
                reset: rst[i] > 0.5,
                div: dg[i] > 0.5,
                a_gate: ag[i] > 0.5,
                a_step: ap[i],
                b_gate: bg[i] > 0.5,
                b_step: bp[i],
            });
        }
    }

    /// Runs until sample `until` in blocks of `block`, applying each event at its exact sample.
    fn run(&mut self, until: usize, block: usize, events: &[(usize, Event)]) -> &mut Self {
        while self.frames.len() < until {
            let t = self.frames.len();
            for (_, e) in events.iter().filter(|(at, _)| *at == t) {
                match e {
                    Event::Cmd(c) => self.clock.command(*c),
                    Event::Div(d) => self.division = *d,
                }
            }
            let next_event = events.iter().map(|(at, _)| *at).filter(|&at| at > t).min();
            let end = (t + block).min(until).min(next_event.unwrap_or(usize::MAX));
            self.block(end - t);
        }
        self
    }
}

/// Sample indices where `f` rises.
fn rises(frames: &[Frame], f: impl Fn(&Frame) -> bool) -> Vec<usize> {
    let mut prev = false;
    let mut out = Vec::new();
    for (i, fr) in frames.iter().enumerate() {
        let v = f(fr);
        if v && !prev {
            out.push(i);
        }
        prev = v;
    }
    out
}

/// Length of every high run of `f`.
fn widths(frames: &[Frame], f: impl Fn(&Frame) -> bool) -> Vec<usize> {
    let mut out = Vec::new();
    let mut run = 0;
    for fr in frames {
        if f(fr) {
            run += 1;
        } else if run > 0 {
            out.push(run);
            run = 0;
        }
    }
    out
}

/// The step each sequencer plays on each of its rising gates.
fn steps(frames: &[Frame], b: bool) -> Vec<f32> {
    let gate = |f: &Frame| if b { f.b_gate } else { f.a_gate };
    rises(frames, gate)
        .into_iter()
        .map(|i| {
            if b {
                frames[i].b_step
            } else {
                frames[i].a_step
            }
        })
        .collect()
}

#[test]
fn division_passes_every_nth_pulse_at_full_width_first_pulse_first() {
    for d in 1..=8 {
        let mut r = Rig::new(d as f32);
        r.run(32 * STEP, 64, &[]);
        let want: Vec<usize> = (0..32).step_by(d).map(|k| k * STEP).collect();
        assert_eq!(rises(&r.frames, |f| f.div), want, "div {d}");
        let w = widths(&r.frames, |f| f.div);
        assert!(w.iter().all(|&w| w == HALF), "div {d}: widths {w:?}");
    }
}

#[test]
fn clock_is_a_half_duty_16th_note_and_runs_from_load() {
    let mut r = Rig::new(1.0);
    r.run(8 * STEP, 64, &[]);
    let want: Vec<usize> = (0..8).map(|k| k * STEP).collect();
    assert_eq!(rises(&r.frames, |f| f.clock), want);
    assert!(widths(&r.frames, |f| f.clock).iter().all(|&w| w == HALF));
    assert!(
        r.frames.iter().all(|f| !f.reset),
        "no reset without Restart"
    );
    assert_eq!(
        steps(&r.frames, false),
        [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]
    );
}

#[test]
fn block_size_does_not_change_timing() {
    let events = || {
        vec![
            (5 * STEP + 100, Event::Cmd(Transport::Stop)),
            (6 * STEP + 17, Event::Cmd(Transport::Restart)),
            (7 * STEP + 333, Event::Cmd(Transport::Run)),
            (9 * STEP + 1000, Event::Div(3.0)),
            (12 * STEP + 2000, Event::Cmd(Transport::Restart)),
        ]
    };
    let mut reference = Rig::new(2.0);
    reference.run(20 * STEP, 64, &events());
    for block in [1, 7, 37, 63] {
        let mut r = Rig::new(2.0);
        r.run(20 * STEP, block, &events());
        assert!(r.frames == reference.frames, "block {block}");
    }
}

#[test]
fn stop_during_a_pulse_drops_the_gate_and_run_plays_the_next_step_once() {
    // Stop 1000 samples into the 3rd pulse (step index 2), Run a while later.
    let (stop, run) = (2 * STEP + 1000, 4 * STEP + 1234);
    let mut r = Rig::new(1.0);
    r.run(
        8 * STEP,
        64,
        &[
            (stop, Event::Cmd(Transport::Stop)),
            (run, Event::Cmd(Transport::Run)),
        ],
    );
    let f = &r.frames;
    assert!(f[stop - 1].clock && f[stop - 1].a_gate);
    assert!(f[stop..run].iter().all(|f| !f.clock && !f.a_gate && !f.div));
    assert!(f[run].clock && f[run].a_gate, "Run starts a pulse at once");
    assert_eq!(f[run].a_step, 3.0, "the next step, not a repeat or a skip");
    assert_eq!(widths(&f[run..], |f| f.clock)[0], HALF, "full-width pulse");
    assert_eq!(steps(f, false), [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
}

#[test]
fn stop_between_pulses_and_run_plays_the_next_step_once() {
    let (stop, run) = (2 * STEP + HALF + 500, 3 * STEP + HALF + 1);
    let mut r = Rig::new(2.0);
    r.run(
        8 * STEP,
        64,
        &[
            (stop, Event::Cmd(Transport::Stop)),
            (run, Event::Cmd(Transport::Run)),
        ],
    );
    let f = &r.frames;
    assert!(f[stop..run].iter().all(|f| !f.clock));
    assert_eq!(f[run].a_step, 3.0);
    assert_eq!(steps(f, false), [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]);
    // The divider keeps counting across a stop: pulses 0, 2 passed before, 3 is skipped
    // (odd), then 4 and 6 pass; nothing extra.
    assert_eq!(steps(f, true), [0.0, 1.0, 2.0, 3.0]);
}

#[test]
fn run_and_stop_are_idempotent() {
    let mut a = Rig::new(2.0);
    a.run(
        6 * STEP,
        64,
        &[
            (1000, Event::Cmd(Transport::Run)),
            (2 * STEP + 10, Event::Cmd(Transport::Stop)),
            (2 * STEP + 20, Event::Cmd(Transport::Stop)),
        ],
    );
    let mut b = Rig::new(2.0);
    b.run(
        6 * STEP,
        64,
        &[(2 * STEP + 10, Event::Cmd(Transport::Stop))],
    );
    assert!(a.frames == b.frames);
}

/// Restart while running, at several points of the pulse: both patterns play step 1 on the
/// same sample, the divided one too, with one rising edge each and `reset` high with that pulse.
#[test]
fn restart_while_running_lines_up_both_patterns_on_step_one() {
    for at in [
        3 * STEP,            // on a rising edge
        3 * STEP + 1,        // just after it
        3 * STEP + HALF - 1, // last high sample
        3 * STEP + HALF,     // first low sample
        4 * STEP - 1,        // just before the next edge
        5 * STEP + 777,      // pulse the divider is not passing (div 2, pulse 5)
    ] {
        let mut r = Rig::new(2.0);
        r.run(12 * STEP, 64, &[(at, Event::Cmd(Transport::Restart))]);
        let f = &r.frames;
        let clock_edges: Vec<usize> = rises(f, |f| f.clock)
            .into_iter()
            .filter(|&i| i >= at)
            .collect();
        let start = clock_edges[0];
        assert!(start <= at + 1, "restart at {at}: pulse starts at {start}");
        assert_eq!(rises(f, |f| f.reset), [start], "restart at {at}");
        assert_eq!(
            widths(f, |f| f.reset),
            [HALF],
            "reset lasts the first pulse"
        );
        assert!(f[start].a_gate && f[start].b_gate && f[start].div);
        assert_eq!(
            (f[start].a_step, f[start].b_step),
            (0.0, 0.0),
            "restart at {at}"
        );
        // Afterwards: steady steps from the new downbeat, B at half speed.
        let a_after: Vec<f32> = steps(&f[start..], false);
        let b_after: Vec<f32> = steps(&f[start..], true);
        assert_eq!(a_after[..5], [0.0, 1.0, 2.0, 3.0, 4.0], "restart at {at}");
        assert_eq!(b_after[..3], [0.0, 1.0, 2.0], "restart at {at}");
        let a_edges = rises(&f[start..], |f| f.a_gate);
        assert!(
            a_edges.windows(2).all(|w| w[1] - w[0] == STEP),
            "no duplicate tick"
        );
        let b_edges = rises(&f[start..], |f| f.b_gate);
        assert!(
            b_edges.windows(2).all(|w| w[1] - w[0] == 2 * STEP),
            "no duplicate tick"
        );
    }
}

#[test]
fn restart_while_stopped_waits_for_run_and_starts_on_step_one() {
    let (stop, restart, run) = (2 * STEP + 1000, 3 * STEP, 4 * STEP + 50);
    let mut r = Rig::new(3.0);
    r.run(
        12 * STEP,
        64,
        &[
            (stop, Event::Cmd(Transport::Stop)),
            (restart, Event::Cmd(Transport::Restart)),
            (run, Event::Cmd(Transport::Run)),
        ],
    );
    let f = &r.frames;
    assert!(f[stop..run]
        .iter()
        .all(|f| !f.clock && !f.reset && !f.a_gate));
    assert_eq!(rises(f, |f| f.reset), [run]);
    assert!(f[run].clock && f[run].div);
    assert_eq!((f[run].a_step, f[run].b_step), (0.0, 0.0));
    assert_eq!(steps(&f[run..], false)[..4], [0.0, 1.0, 2.0, 3.0]);
    assert_eq!(steps(&f[run..], true)[..3], [0.0, 1.0, 2.0]);
}

#[test]
fn a_second_restart_before_run_is_one_restart() {
    let mut a = Rig::new(2.0);
    let ev = |twice: bool| {
        let mut v = vec![
            (1000, Event::Cmd(Transport::Stop)),
            (STEP, Event::Cmd(Transport::Restart)),
            (2 * STEP, Event::Cmd(Transport::Run)),
        ];
        if twice {
            v.push((STEP + 10, Event::Cmd(Transport::Restart)));
        }
        v
    };
    a.run(8 * STEP, 64, &ev(true));
    let mut b = Rig::new(2.0);
    b.run(8 * STEP, 64, &ev(false));
    assert!(a.frames == b.frames);
}

#[test]
fn reset_input_restarts_the_divider_count() {
    // Hand-driven clock and reset into a div-3 divider.
    let mut d = ClockDiv::new();
    let pulse = |t: usize, start: usize| (t >= start && t < start + 10) as u8 as f32;
    let (clock_at, reset_at) = ([0, 20, 40, 60, 80, 100, 120], [45, 80]);
    let n = 140;
    let clk: Vec<f32> = (0..n)
        .map(|t| clock_at.iter().map(|&s| pulse(t, s)).sum())
        .collect();
    // A reset while a skipped pulse is high (45) must not turn it on mid-pulse; one on the
    // same sample as an edge (80) lets that pulse through.
    let rst: Vec<f32> = (0..n)
        .map(|t| reset_at.iter().map(|&s| pulse(t, s)).sum())
        .collect();
    let mut out = vec![0f32; n];
    let mut outs: [&mut [f32]; 1] = [&mut out];
    let ins = [Signal::Buffer(&clk), Signal::Buffer(&rst)];
    d.process(&mut ProcessIo::new(
        &ins,
        &mut outs,
        &[Signal::Scalar(3.0)],
        n,
    ));
    let frames: Vec<Frame> = out
        .iter()
        .map(|&v| Frame {
            div: v > 0.5,
            ..Default::default()
        })
        .collect();
    // 0 passes (first); 20 skipped; 40 skipped (reset at 45 re-arms); 60 passes; 80 passes
    // (reset on its edge); 100, 120 skipped.
    assert_eq!(rises(&frames, |f| f.div), [0, 60, 80]);
    assert_eq!(widths(&frames, |f| f.div), [10, 10, 10]);
}

#[test]
fn division_change_commits_on_the_next_edge_without_glitches() {
    // Change 2 -> 4 mid-pulse (while a passed pulse is high) and 4 -> 3 between pulses.
    let (c1, c2) = (4 * STEP + 100, 9 * STEP + HALF + 100);
    let mut r = Rig::new(2.0);
    r.run(
        24 * STEP,
        64,
        &[(c1, Event::Div(4.0)), (c2, Event::Div(3.0))],
    );
    let f = &r.frames;
    assert!(
        widths(f, |f| f.div).iter().all(|&w| w == HALF),
        "no cut pulses"
    );
    // 0, 2, 4 at div 2; 5 is the first edge after the change: it passes and re-anchors, so 9;
    // then 10 commits div 3 (passes), 13, 16, ...
    let want: Vec<usize> = [0, 2, 4, 5, 9, 10, 13, 16, 19, 22]
        .iter()
        .map(|k| k * STEP)
        .collect();
    assert_eq!(rises(f, |f| f.div), want);
}
