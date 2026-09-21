use kabl_modules::builtins::FilterSvf;
use kabl_modules::dsp::Svf;
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{Module, ProcessIo, Signal};

const SAMPLE_RATE: f32 = 48000.0;
const BLOCK: usize = 64;

fn quality() -> QualityConfig {
    QualityConfig {
        tier: QualityTier::Live,
    }
}

fn run(
    module: &mut FilterSvf,
    input: &[f32; BLOCK],
    cutoff_hz: f32,
    resonance: f32,
) -> [f32; 3 * BLOCK] {
    let in_sig = [Signal::Buffer(input)];
    let cutoff_cv = [Signal::Scalar(0.0)];
    let resonance_cv = [Signal::Scalar(0.0)];
    let inputs = [in_sig[0], cutoff_cv[0], resonance_cv[0]];
    let params = [Signal::Scalar(cutoff_hz), Signal::Scalar(resonance)];

    let mut lp = [0f32; BLOCK];
    let mut bp = [0f32; BLOCK];
    let mut hp = [0f32; BLOCK];
    {
        let mut outputs: [&mut [f32]; 3] = [&mut lp, &mut bp, &mut hp];
        let mut io = ProcessIo::new(&inputs, &mut outputs, &params, BLOCK);
        module.process(&mut io);
    }
    let mut combined = [0f32; 3 * BLOCK];
    combined[..BLOCK].copy_from_slice(&lp);
    combined[BLOCK..2 * BLOCK].copy_from_slice(&bp);
    combined[2 * BLOCK..].copy_from_slice(&hp);
    combined
}

#[test]
fn fast_path_scalar_cv_matches_reference_svf_lowpass() {
    let mut module = FilterSvf::new();
    module.prepare(SAMPLE_RATE, BLOCK, &quality());

    let input: [f32; BLOCK] = std::array::from_fn(|i| (i as f32 * 0.1).sin());
    let out = run(&mut module, &input, 1000.0, 0.4);
    let lp = &out[..BLOCK];

    let mut reference = Svf::new();
    for (i, &x) in input.iter().enumerate() {
        let expected = reference.process_lowpass(x, 1000.0, 0.4, SAMPLE_RATE);
        assert_eq!(lp[i], expected, "sample {i}");
    }
}

#[test]
fn per_sample_cv_path_matches_scalar_path_when_cv_is_actually_constant() {
    // A buffer input that happens to be constant should produce the same output as a scalar
    // input with the same value — the fast-path/slow-path split is an optimization, not a
    // behavior change.
    let mut fast = FilterSvf::new();
    fast.prepare(SAMPLE_RATE, BLOCK, &quality());
    let mut slow = FilterSvf::new();
    slow.prepare(SAMPLE_RATE, BLOCK, &quality());

    let input: [f32; BLOCK] = std::array::from_fn(|i| (i as f32 * 0.1).cos());
    let constant_cv: [f32; BLOCK] = [0.0; BLOCK];

    let in_sig = [Signal::Buffer(&input)];
    let fast_inputs = [in_sig[0], Signal::Scalar(0.0), Signal::Scalar(0.0)];
    let slow_inputs = [in_sig[0], Signal::Buffer(&constant_cv), Signal::Scalar(0.0)];
    let params = [Signal::Scalar(1500.0), Signal::Scalar(0.3)];

    let mut fast_lp = [0f32; BLOCK];
    let mut fast_bp = [0f32; BLOCK];
    let mut fast_hp = [0f32; BLOCK];
    {
        let mut outputs: [&mut [f32]; 3] = [&mut fast_lp, &mut fast_bp, &mut fast_hp];
        let mut io = ProcessIo::new(&fast_inputs, &mut outputs, &params, BLOCK);
        fast.process(&mut io);
    }
    let mut slow_lp = [0f32; BLOCK];
    let mut slow_bp = [0f32; BLOCK];
    let mut slow_hp = [0f32; BLOCK];
    {
        let mut outputs: [&mut [f32]; 3] = [&mut slow_lp, &mut slow_bp, &mut slow_hp];
        let mut io = ProcessIo::new(&slow_inputs, &mut outputs, &params, BLOCK);
        slow.process(&mut io);
    }

    for i in 0..BLOCK {
        assert!(
            (fast_lp[i] - slow_lp[i]).abs() < 1e-4,
            "sample {i}: fast={} slow={}",
            fast_lp[i],
            slow_lp[i]
        );
    }
}

#[test]
fn reset_clears_filter_state() {
    let mut module = FilterSvf::new();
    module.prepare(SAMPLE_RATE, BLOCK, &quality());
    let input: [f32; BLOCK] = [1.0; BLOCK];
    let _ = run(&mut module, &input, 500.0, 0.5);

    module.reset();
    let silence: [f32; BLOCK] = [0.0; BLOCK];
    let out = run(&mut module, &silence, 500.0, 0.5);
    for &v in &out {
        assert_eq!(
            v, 0.0,
            "a freshly-reset filter fed silence should output silence"
        );
    }
}
