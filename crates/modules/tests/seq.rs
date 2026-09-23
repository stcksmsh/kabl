use kabl_modules::builtins::{Clock, Seq};
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{Module, ProcessIo, Signal};

const SR: f32 = 48000.0;
const BLOCK: usize = 64;
const STEP: usize = 6000; // samples per 16th note at 120 bpm
const LENGTH: usize = 16;

/// Runs a 120 bpm clock into a sequencer for `steps` 16th notes, with `params` changed from the
/// defaults and a reset pulse at each sample index in `resets`. Returns the pitch of every note
/// played, read on its last gated sample (so a pitch change mid-note shows).
fn notes(changes: &[(usize, f32)], resets: &[usize], steps: usize) -> Vec<f32> {
    let q = QualityConfig {
        tier: QualityTier::Live,
    };
    let (mut clock, mut seq) = (Clock::new(), Seq::new());
    clock.prepare(SR, BLOCK, &q);
    seq.prepare(SR, BLOCK, &q);
    let mut values: Vec<f32> = seq.info().params.iter().map(|p| p.default).collect();
    for &(i, v) in changes {
        values[i] = v;
    }
    let params: Vec<Signal> = values.iter().map(|&v| Signal::Scalar(v)).collect();

    let (mut out, mut high) = (Vec::new(), false);
    for b in 0..(steps * STEP / BLOCK) {
        let mut tick = [0f32; BLOCK];
        let mut outs: [&mut [f32]; 1] = [&mut tick];
        clock.process(&mut ProcessIo::new(
            &[],
            &mut outs,
            &[Signal::Scalar(120.0)],
            BLOCK,
        ));

        let mut reset = [0f32; BLOCK];
        for &r in resets {
            // A 100-sample pulse.
            for (i, s) in reset.iter_mut().enumerate() {
                let t = b * BLOCK + i;
                if t >= r && t < r + 100 {
                    *s = 1.0;
                }
            }
        }
        let (mut gate, mut pitch) = ([0f32; BLOCK], [0f32; BLOCK]);
        let mut outs: [&mut [f32]; 2] = [&mut gate, &mut pitch];
        let ins = [Signal::Buffer(&tick), Signal::Buffer(&reset)];
        seq.process(&mut ProcessIo::new(&ins, &mut outs, &params, BLOCK));
        for i in 0..BLOCK {
            if gate[i] > 0.5 && !high {
                out.push(pitch[i]);
            }
            if gate[i] > 0.5 {
                *out.last_mut().unwrap() = pitch[i];
            }
            high = gate[i] > 0.5;
        }
    }
    out
}

/// Default pattern, one step per 16th note, step 3 switched off (a rest), wrapping after 8.
#[test]
fn clock_steps_the_pattern_and_rests_on_off_steps() {
    assert_eq!(
        notes(&[(8 + 2, 0.0)], &[], 10),
        [0.0, 3.0, 10.0, 12.0, 10.0, 7.0, 3.0, 0.0, 3.0]
    );
}

#[test]
fn length_loops_the_first_steps() {
    assert_eq!(
        notes(&[(LENGTH, 3.0)], &[], 7),
        [0.0, 3.0, 7.0, 0.0, 3.0, 7.0, 0.0]
    );
}

/// A reset between steps 3 and 4 makes the next tick play step 1 again.
#[test]
fn reset_restarts_the_pattern_on_the_next_tick() {
    assert_eq!(
        notes(&[], &[2 * STEP + 4000], 6),
        [0.0, 3.0, 7.0, 0.0, 3.0, 7.0]
    );
}

/// A reset landing just after a tick (two clocks a few samples apart) restarts on that tick's note:
/// step 1 plays now instead of the loop's last step.
#[test]
fn reset_just_after_a_tick_plays_step_one() {
    assert_eq!(notes(&[], &[3 * STEP + 50], 5), [0.0, 3.0, 7.0, 0.0, 3.0]);
}
