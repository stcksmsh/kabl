use kabl_modules::builtins::{RingMod, Vca};
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{Module, ProcessIo, Signal};

const BLOCK: usize = 8;

fn quality() -> QualityConfig {
    QualityConfig {
        tier: QualityTier::Live,
    }
}

#[test]
fn vca_linear_gain_scales_input_exactly() {
    let mut module = Vca::new();
    module.prepare(48000.0, BLOCK, &quality());

    let input = [0.5f32; BLOCK];
    let inputs = [Signal::Buffer(&input), Signal::Scalar(0.0)];
    let params = [Signal::Scalar(0.25), Signal::Scalar(0.0)]; // gain=0.25, linear response
    let mut buf = [0f32; BLOCK];
    let mut outputs: [&mut [f32]; 1] = [&mut buf];
    let mut io = ProcessIo::new(&inputs, &mut outputs, &params, BLOCK);
    module.process(&mut io);

    for &v in &buf {
        assert!(
            (v - 0.125).abs() < 1e-6,
            "0.5 * 0.25 should be 0.125, got {v}"
        );
    }
}

#[test]
fn vca_exponential_response_curves_below_linear() {
    let mut module = Vca::new();
    module.prepare(48000.0, BLOCK, &quality());

    let input = [1.0f32; BLOCK];
    let inputs = [Signal::Buffer(&input), Signal::Scalar(0.0)];
    let params = [Signal::Scalar(0.5), Signal::Scalar(1.0)]; // gain=0.5, exponential response
    let mut buf = [0f32; BLOCK];
    let mut outputs: [&mut [f32]; 1] = [&mut buf];
    let mut io = ProcessIo::new(&inputs, &mut outputs, &params, BLOCK);
    module.process(&mut io);

    // gain^2 = 0.25, below the linear 0.5.
    for &v in &buf {
        assert!((v - 0.25).abs() < 1e-6, "0.5^2 should be 0.25, got {v}");
    }
}

#[test]
fn ringmod_multiplies_its_two_inputs() {
    let mut module = RingMod::new();
    module.prepare(48000.0, BLOCK, &quality());

    let a = [0.5f32; BLOCK];
    let b = [-0.4f32; BLOCK];
    let inputs = [Signal::Buffer(&a), Signal::Buffer(&b)];
    let mut buf = [0f32; BLOCK];
    let mut outputs: [&mut [f32]; 1] = [&mut buf];
    let mut io = ProcessIo::new(&inputs, &mut outputs, &[], BLOCK);
    module.process(&mut io);

    for &v in &buf {
        assert!(
            (v - (-0.2)).abs() < 1e-6,
            "0.5 * -0.4 should be -0.2, got {v}"
        );
    }
}
