//! `macro`: outputs follow their knobs, glide over ~20 ms, start at the knob, carry state.

use kabl_modules::builtins::Macro;
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{Module, ProcessIo, Signal};

const BLOCK: usize = 64;

fn block(m: &mut Macro, knobs: [f32; 4]) -> [[f32; BLOCK]; 4] {
    let mut o = [[0f32; BLOCK]; 4];
    let [a, b, c, d] = &mut o;
    let mut outs: [&mut [f32]; 4] = [a, b, c, d];
    m.process(&mut ProcessIo::new(
        &[],
        &mut outs,
        &knobs.map(Signal::Scalar),
        BLOCK,
    ));
    o
}

#[test]
fn outputs_start_at_the_knob_and_glide_to_changes() {
    let mut m = Macro::new();
    m.prepare(
        48000.0,
        BLOCK,
        &QualityConfig {
            tier: QualityTier::Live,
        },
    );
    let o = block(&mut m, [0.25, 0.5, 0.75, 1.0]);
    assert_eq!([o[0][0], o[1][0], o[2][0], o[3][0]], [0.25, 0.5, 0.75, 1.0]);
    // A jump to 1.0 on m1: no step, ~63 % after 20 ms, settled after 400 ms.
    let mut last = 0.25;
    let mut at_20ms = 0.0;
    for b in 0..300 {
        let o = block(&mut m, [1.0, 0.5, 0.75, 1.0]);
        for (i, &v) in o[0].iter().enumerate() {
            assert!(v - last < 0.01, "step {}", v - last);
            last = v;
            if b * BLOCK + i == 960 {
                at_20ms = v;
            }
        }
    }
    assert!((at_20ms - (0.25 + 0.75 * 0.632)).abs() < 0.02, "{at_20ms}");
    assert_eq!(last, 1.0);
}
