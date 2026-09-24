//! Clock-synced `lfo`: acquisition, lock, held tempo across a stop, gaps, reset and phase,
//! driven by a real `clock`.

use kabl_modules::builtins::{Clock, Lfo, LfoSync, Transport};
use kabl_modules::dsp::{FullLfo, LfoWaveform};
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{Module, ProcessIo, Signal};

const SR: f32 = 48000.0;
const BLOCK: usize = 64;
const TICK: usize = 6000; // 16th at 120 bpm
const SAW: f32 = 2.0;
const SQUARE: f32 = 3.0;

struct Rig {
    clock: Clock,
    lfo: Lfo,
    /// rate, waveform, sync, phase
    params: [f32; 4],
    /// Patch the clock's reset output into the LFO's reset.
    reset_cable: bool,
    clock_cable: bool,
    out: Vec<f32>,
}

impl Rig {
    fn new(sync: f32, waveform: f32) -> Self {
        let q = QualityConfig {
            tier: QualityTier::Live,
        };
        let (mut clock, mut lfo) = (Clock::new(), Lfo::new());
        clock.prepare(SR, BLOCK, &q);
        lfo.prepare(SR, BLOCK, &q);
        Rig {
            clock,
            lfo,
            params: [2.0, waveform, sync, 0.0],
            reset_cable: false,
            clock_cable: true,
            out: Vec::new(),
        }
    }

    fn run(&mut self, samples: usize) -> &mut Self {
        for _ in 0..samples / BLOCK {
            let (mut gate, mut reset) = ([0f32; BLOCK], [0f32; BLOCK]);
            let mut outs: [&mut [f32]; 2] = [&mut gate, &mut reset];
            self.clock.process(&mut ProcessIo::new(
                &[],
                &mut outs,
                &[Signal::Scalar(120.0)],
                BLOCK,
            ));
            if !self.clock_cable {
                gate = [0.0; BLOCK];
            }
            if !self.reset_cable {
                reset = [0.0; BLOCK];
            }
            let ins = [Signal::Buffer(&gate), Signal::Buffer(&reset)];
            let params = self.params.map(Signal::Scalar);
            let mut o = [0f32; BLOCK];
            let mut outs: [&mut [f32]; 1] = [&mut o];
            self.lfo
                .process(&mut ProcessIo::new(&ins, &mut outs, &params, BLOCK));
            self.out.extend_from_slice(&o);
        }
        self
    }

    /// Sample indices where the saw wraps (a new cycle starts), from `from` on.
    fn wraps(&self, from: usize) -> Vec<usize> {
        (from.max(1)..self.out.len())
            .filter(|&i| self.out[i] < self.out[i - 1] - 1.0)
            .collect()
    }
}

#[test]
fn free_mode_is_the_old_lfo() {
    let mut r = Rig::new(0.0, 0.0);
    r.params[0] = 3.3;
    r.run(48000);
    let mut reference = FullLfo::new();
    for (i, &v) in r.out.iter().enumerate() {
        assert_eq!(v, reference.next(3.3, SR, LfoWaveform::Sine), "sample {i}");
    }
    assert_eq!(r.lfo.status(), LfoSync::Free);
}

#[test]
fn no_clock_runs_at_the_free_rate_while_acquiring() {
    let mut r = Rig::new(5.0, SAW);
    r.clock_cable = false;
    r.params[0] = 4.0; // 12000-sample cycles
    r.run(60000);
    let w = r.wraps(0);
    for pair in w.windows(2) {
        assert!(pair[1].abs_diff(pair[0]) <= 1 + 12000);
        assert!(pair[1] - pair[0] >= 11999);
    }
    assert_eq!(r.lfo.status(), LfoSync::Acquiring);
}

#[test]
fn locks_to_one_bar_cycles_aligned_to_the_reset_and_glides_in() {
    // One bar = 16 pulses = 96000 samples; the clock's first pulse (a load) resets the count.
    let mut r = Rig::new(5.0, SAW);
    r.reset_cable = true;
    r.params[0] = 0.7; // far from 0.5 Hz, so acquisition has work to do
    r.clock.command(Transport::Restart);
    r.run(96000 * 6);
    assert_eq!(r.lfo.status(), LfoSync::Synced);
    let w = r.wraps(96000 * 3);
    assert!(w.len() >= 2);
    for &x in &w {
        // Cycles start on bar lines (tick 0 of each bar, sample multiple of 96000).
        let off = x % 96000;
        assert!(off.min(96000 - off) <= 2, "wrap at {x}");
    }
    // Gliding, not jumping: the saw never steps by more than 1.5 × its locked slope, except
    // at its wraps.
    let slope = 2.0 / 96000.0;
    for i in 1..r.out.len() {
        let d = r.out[i] - r.out[i - 1];
        assert!(
            d < 0.0 || d <= 1.5 * slope + 1e-5 || i < 12000,
            "step {d} at {i}"
        );
    }
}

#[test]
fn clock_stop_holds_the_tempo_and_a_gap_is_not_tempo() {
    let mut r = Rig::new(3.0, SAW); // 1/4 note = 4 pulses = 24000 samples
    r.run(TICK * 40);
    assert_eq!(r.lfo.status(), LfoSync::Synced);
    r.clock.command(Transport::Stop);
    let from = r.out.len();
    r.run(TICK * 30);
    assert_eq!(r.lfo.status(), LfoSync::Held);
    let w = r.wraps(from + 100);
    for pair in w.windows(2) {
        assert!((pair[1] - pair[0]).abs_diff(24000) <= 2, "{pair:?}");
    }
    // Run again: the long interval across the stop never becomes the tempo.
    r.clock.command(Transport::Run);
    let from = r.out.len();
    r.run(TICK * 40);
    let w = r.wraps(from + TICK * 20);
    for pair in w.windows(2) {
        assert!((pair[1] - pair[0]).abs_diff(24000) <= 60, "{pair:?}");
    }
    assert_eq!(r.lfo.status(), LfoSync::Synced);
}

#[test]
fn the_reset_input_jumps_to_the_phase_control_only_when_patched() {
    // Not patched: a Restart does not touch the LFO.
    let mut a = Rig::new(3.0, SAW);
    let mut b = Rig::new(3.0, SAW);
    a.run(TICK * 21 + 64 * 10);
    b.run(TICK * 21 + 64 * 10);
    a.clock.command(Transport::Restart);
    b.clock.command(Transport::Restart);
    b.reset_cable = true;
    b.params[3] = 90.0;
    a.run(640);
    b.run(640);
    let restart = TICK * 21 + 640 + 1;
    // b jumped to a quarter cycle at the restart pulse; a kept its phase.
    let pb = (b.out[restart + 2] + 1.0) / 2.0;
    assert!((pb - 0.25).abs() < 0.01, "b phase {pb}");
    let pa0 = (a.out[restart - 2] + 1.0) / 2.0;
    let pa1 = (a.out[restart + 2] + 1.0) / 2.0;
    assert!((pa1 - pa0).abs() < 0.01, "a jumped {pa0} → {pa1}");
}

#[test]
fn square_edges_stay_hard() {
    let mut r = Rig::new(2.0, SQUARE);
    r.run(TICK * 40);
    assert!(r.out.iter().all(|&v| v == 1.0 || v == -1.0));
}

#[test]
fn carried_state_continues_exactly() {
    let mut r = Rig::new(4.0, SAW);
    r.run(TICK * 20);
    let mut twin = Rig::new(4.0, SAW);
    twin.clock = r.clock;
    twin.lfo.carry_from(&r.lfo);
    r.out.clear();
    r.run(TICK * 10);
    twin.run(TICK * 10);
    assert_eq!(r.out, twin.out);
}
