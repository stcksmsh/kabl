//! A kabl control module: a CV in, a CV out, an `amount` knob and an `offset`. Edit `META` and
//! `process`. `cargo kabl new --control` copies this file over `src/lib.rs`.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use kabl_module_sdk::*;

struct Scale;

const IN: usize = 0;
const OUT: usize = 0;
const AMOUNT: usize = 0;
const OFFSET: usize = 1;

impl Module for Scale {
    // The id is `githubhandle.module`. `kabl.`, `demo.` and `test.` are reserved.
    const META: Meta = Meta::new("yourhandle.gain", 1)
        .name("Scale")
        .explain("Scales a control signal and adds an offset.")
        .category(Category::Utility)
        .rate(Rate::Global)
        .width(4)
        .port_in(PortType::Cv, "in", 1)
        .port_out(PortType::Cv, "out", 1)
        .param("amount", "-2", "2", "1", Taper::Linear, 5, "x")
        .param("offset", "-1", "1", "0", Taper::Linear, 5, "cv");

    fn new(_: &mut Init) -> Self {
        Scale
    }

    fn reset(&mut self) {}

    fn process(&mut self, io: &mut Io) {
        let (amount, offset) = (io.param(AMOUNT), io.param(OFFSET));
        for i in 0..io.n {
            // Inputs are always finite; the host disables a module whose output is not.
            io.output[OUT][i] = io.input[IN][i] * amount + offset;
        }
    }
}

const _: () = {
    Scale::META.assert_in(IN, "in");
    Scale::META.assert_out(OUT, "out");
    Scale::META.assert_param(AMOUNT, "amount");
    Scale::META.assert_param(OFFSET, "offset");
};

kabl_module!(Scale);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_and_offsets() {
        let mut m = Scale::new(&mut Init {
            sample_rate: 48_000.0,
            max_block: 64,
        });
        let mut input = [[0.0; MAX_BLOCK]];
        let mut output = [[0.0; MAX_BLOCK]];
        input[IN][..3].copy_from_slice(&[1.0, 0.5, -1.0]);
        let mut io = Io {
            n: 3,
            input: &input,
            output: &mut output,
            params: &[2.0, 0.5],
        };
        m.process(&mut io);
        assert_eq!(&output[OUT][..3], &[2.5, 1.5, -1.5]);
    }
}
