//! A kabl audio module: one input, one output, a `gain` knob. Edit `META` and `process`.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use kabl_module_sdk::*;

struct Gain {
    /// Last gain seen, kept across save and load as an example of saved settings.
    last: f32,
}

// Slot indices follow the port order in META. The checks at the bottom turn a rename or reorder
// into a compile error instead of a silent swap.
const IN: usize = 0;
const OUT: usize = 0;
const GAIN: usize = 0;

impl Module for Gain {
    // The id is `githubhandle.module`. `kabl.`, `demo.` and `test.` are reserved.
    const META: Meta = Meta::new("yourhandle.gain", 1)
        .name("Gain")
        .explain("Turns the signal up or down.")
        .category(Category::Utility)
        .rate(Rate::Voice)
        .width(4)
        .port_in(PortType::Audio, "in", 1)
        .port_out(PortType::Audio, "out", 1)
        .param("gain", "0", "4", "1", Taper::Linear, 5, "x");
    const STATE_CAP: usize = 4;

    fn new(_: &mut Init) -> Self {
        Gain { last: 1.0 }
    }

    fn reset(&mut self) {
        self.last = 1.0;
    }

    fn process(&mut self, io: &mut Io) {
        let g = io.param(GAIN);
        for i in 0..io.n {
            // The host disables a module that outputs NaN or infinity, and `cargo kabl validate`
            // feeds you both, so guard the input.
            let x = io.input[IN][i];
            io.output[OUT][i] = if x.is_finite() { x * g } else { 0.0 };
        }
        self.last = g;
    }

    fn save(&self, w: &mut StateWriter) {
        w.f32(self.last);
    }

    fn load(&mut self, _version: u32, bytes: &[u8]) -> bool {
        match StateReader::new(bytes).f32() {
            Some(v) => {
                self.last = v;
                true
            }
            None => false,
        }
    }
}

const _: () = {
    Gain::META.assert_in(IN, "in");
    Gain::META.assert_out(OUT, "out");
    Gain::META.assert_param(GAIN, "gain");
};

kabl_module!(Gain);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_scales_the_input() {
        let mut m = Gain::new(&mut Init {
            sample_rate: 48_000.0,
            max_block: 64,
        });
        let mut input = [[0.0; MAX_BLOCK]];
        let mut output = [[0.0; MAX_BLOCK]];
        input[IN][..4].copy_from_slice(&[1.0, -0.5, 0.25, 0.0]);
        let mut io = Io {
            n: 4,
            input: &input,
            output: &mut output,
            params: &[2.0],
        };
        m.process(&mut io);
        assert_eq!(&output[OUT][..4], &[2.0, -1.0, 0.5, 0.0]);
    }
}
