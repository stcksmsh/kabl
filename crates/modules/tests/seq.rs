use kabl_modules::builtins::{Clock, Seq};
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{Module, ProcessIo, Signal};

const SR: f32 = 48000.0;
const BLOCK: usize = 64;

/// Clock at 120 bpm into the sequencer with step 3 switched off: the sequencer plays its default
/// pattern one step per 16th note, rests on step 3, and wraps after step 8.
#[test]
fn clock_steps_the_pattern_and_rests_on_off_steps() {
    let q = QualityConfig {
        tier: QualityTier::Live,
    };
    let (mut clock, mut seq) = (Clock::new(), Seq::new());
    clock.prepare(SR, BLOCK, &q);
    seq.prepare(SR, BLOCK, &q);
    let defaults: Vec<f32> = seq.info().params.iter().map(|p| p.default).collect();
    let mut params: Vec<Signal> = defaults.iter().map(|&v| Signal::Scalar(v)).collect();
    params[8 + 2] = Signal::Scalar(0.0);

    // (pitch, gate) at each gate-or-pitch change.
    let mut events: Vec<(f32, f32)> = Vec::new();
    let samples_per_step = (SR * 60.0 / 120.0 / 4.0) as usize; // 6000
    for _ in 0..(10 * samples_per_step / BLOCK) {
        let mut tick = [0f32; BLOCK];
        let mut outs: [&mut [f32]; 1] = [&mut tick];
        clock.process(&mut ProcessIo::new(
            &[],
            &mut outs,
            &[Signal::Scalar(120.0)],
            BLOCK,
        ));

        let (mut gate, mut pitch) = ([0f32; BLOCK], [0f32; BLOCK]);
        let mut outs: [&mut [f32]; 2] = [&mut gate, &mut pitch];
        let ins = [Signal::Buffer(&tick)];
        seq.process(&mut ProcessIo::new(&ins, &mut outs, &params, BLOCK));
        for i in 0..BLOCK {
            let e = (pitch[i], gate[i]);
            if events.last() != Some(&e) {
                events.push(e);
            }
        }
    }
    let notes: Vec<f32> = events.iter().filter(|e| e.1 == 1.0).map(|e| e.0).collect();
    assert_eq!(notes, [0.0, 3.0, 10.0, 12.0, 10.0, 7.0, 3.0, 0.0, 3.0]);
    // Step 3 (7 st) is present on the pitch output but never gated.
    assert!(events.contains(&(7.0, 0.0)));
}
