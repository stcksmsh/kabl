use kabl_modules::builtins::Lfo;
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{Module, ProcessIo, Signal};

const SAMPLE_RATE: f32 = 48000.0;
const BLOCK: usize = 64;

fn quality() -> QualityConfig {
    QualityConfig {
        tier: QualityTier::Live,
    }
}

fn run(module: &mut Lfo, rate_hz: f32, waveform: f32) -> [f32; BLOCK] {
    let inputs = [Signal::Scalar(0.0), Signal::Scalar(0.0)];
    let params = [
        Signal::Scalar(rate_hz),
        Signal::Scalar(waveform),
        Signal::Scalar(0.0),
        Signal::Scalar(0.0),
    ];
    let mut buf = [0f32; BLOCK];
    let mut outputs: [&mut [f32]; 1] = [&mut buf];
    let mut io = ProcessIo::new(&inputs, &mut outputs, &params, BLOCK);
    module.process(&mut io);
    buf
}

#[test]
fn all_five_waveform_selections_produce_finite_bounded_output() {
    for waveform in 0..5 {
        let mut module = Lfo::new();
        module.prepare(SAMPLE_RATE, BLOCK, &quality());
        let out = run(&mut module, 5.0, waveform as f32);
        for &v in &out {
            assert!(
                v.is_finite() && (-1.0..=1.0).contains(&v),
                "waveform {waveform}: {v}"
            );
        }
    }
}

#[test]
fn reset_zeroes_phase_so_output_restarts_from_the_same_point() {
    let mut a = Lfo::new();
    a.prepare(SAMPLE_RATE, BLOCK, &quality());
    let first = run(&mut a, 3.0, 0.0);
    run(&mut a, 3.0, 0.0); // advance further

    a.reset();
    let after_reset = run(&mut a, 3.0, 0.0);
    assert_eq!(
        first, after_reset,
        "resetting should restart the waveform from phase 0"
    );
}

#[test]
fn save_and_load_state_continues_identically() {
    let mut a = Lfo::new();
    a.prepare(SAMPLE_RATE, BLOCK, &quality());
    run(&mut a, 3.0, 0.0);

    struct MapState(std::collections::HashMap<String, f32>);
    impl kabl_modules::module::StateWriter for MapState {
        fn write_f32(&mut self, key: &str, value: f32) {
            self.0.insert(key.to_string(), value);
        }
    }
    impl kabl_modules::module::StateReader for MapState {
        fn read_f32(&self, key: &str) -> Option<f32> {
            self.0.get(key).copied()
        }
    }
    let mut state = MapState(std::collections::HashMap::new());
    a.save_state(&mut state);

    let mut b = Lfo::new();
    b.prepare(SAMPLE_RATE, BLOCK, &quality());
    b.load_state(&state);

    assert_eq!(run(&mut a, 3.0, 0.0), run(&mut b, 3.0, 0.0));
}
