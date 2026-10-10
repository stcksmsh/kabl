//! The clock's position as the audio thread reports it to the interface: every report call runs
//! under `assert_no_alloc`, the position counts 16ths at the clock's tempo, freezes on Stop,
//! starts over on Restart and follows the host's transport in host mode.

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::Collector;
use kabl_core::{ModuleId, PatchState};
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::PatchEngine;
use kabl_modules::builtins::Transport;

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;
const CLOCK: ModuleId = 1;
/// The interlocking patch's clock runs at 116 bpm: 16ths per block.
const PER_BLOCK: f64 = 116.0 / 60.0 * 4.0 * BLOCK as f64 / SR as f64;

fn patch() -> PatchState {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/interlocking");
    kabl_core::load(&dir).expect("patch").state().clone()
}

struct Rig {
    e: PatchEngine,
    _collector: Collector,
}

impl Rig {
    fn new() -> Self {
        let collector = Collector::new();
        let e = PatchEngine::new(&collector.handle(), &patch(), SR, 8).unwrap();
        Rig {
            e,
            _collector: collector,
        }
    }

    /// One block, then the report the interface gets: `(running, position)`.
    fn block(&mut self) -> (bool, f64) {
        let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
        let mut out = (false, f64::NAN);
        assert_no_alloc(|| {
            self.e.process_block(&mut l, &mut r);
            self.e.clocks(|id, run, pos| {
                if id == CLOCK {
                    out = (run, pos)
                }
            });
        });
        out
    }

    fn run(&mut self, blocks: usize) -> (bool, f64) {
        (0..blocks).map(|_| self.block()).last().unwrap()
    }
}

#[test]
fn position_counts_sixteenths_at_the_clock_tempo() {
    let mut rig = Rig::new();
    // Never steps back, not even at the wrap between two pulses.
    let mut prev = 0.0;
    for _ in 0..1000 {
        let (_, p) = rig.block();
        assert!(p >= prev, "stepped back from {prev} to {p}");
        prev = p;
    }
    let (run, pos) = rig.block();
    assert!(run);
    // The pulse being played counts as it begins: within one 16th of the elapsed time.
    let expect = 1001.0 * PER_BLOCK;
    assert!((pos - expect).abs() < 1.0, "pos {pos}, want about {expect}");
    // A bar is 16 of them: position / 16 is the bar, the remainder the place in it.
    let (_, later) = rig.run(1000);
    assert!(((later - pos) - 1000.0 * PER_BLOCK).abs() < 0.2);
}

#[test]
fn stop_freezes_it_and_restart_starts_the_bar_over() {
    let mut rig = Rig::new();
    rig.run(500);
    rig.e.transport(CLOCK, Transport::Stop);
    let (run, frozen) = rig.block();
    assert!(!run);
    let (_, still) = rig.run(200);
    assert_eq!(frozen, still, "a stopped clock does not move");
    rig.e.transport(CLOCK, Transport::Run);
    rig.e.transport(CLOCK, Transport::Restart);
    let (run, pos) = rig.run(10);
    assert!(run && pos < 1.0, "restarted: pos {pos}");
}

#[test]
fn host_mode_follows_the_host_transport() {
    let mut rig = Rig::new();
    // 120 bpm: 8 blocks of beats per second of host time; the host plays from beat 0.
    let beats_per_block = 120.0 / 60.0 * BLOCK as f64 / SR as f64;
    let mut beats = 0.0;
    let mut last = (false, 0.0);
    for _ in 0..1500 {
        rig.e.host_clock(Some((beats, 120.0, true)));
        last = rig.block();
        beats += beats_per_block;
    }
    // About two bars (8 beats = 32 sixteenths) in: bar 3 of the host.
    let (run, pos) = last;
    assert!(run);
    assert!(
        (pos - beats * 4.0).abs() < 1.0,
        "pos {pos}, host {beats} beats"
    );
    // The host stops: the clock reports stopped and the position holds.
    rig.e.host_clock(Some((beats, 120.0, false)));
    let (run, held) = rig.block();
    assert!(!run);
    rig.e.host_clock(Some((beats, 120.0, false)));
    assert_eq!(rig.block().1, held);
}
