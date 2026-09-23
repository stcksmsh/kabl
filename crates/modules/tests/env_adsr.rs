use kabl_modules::builtins::EnvAdsr;
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{Module, ProcessIo, Signal};

const SAMPLE_RATE: f32 = 48000.0;
const BLOCK: usize = 64;

fn quality() -> QualityConfig {
    QualityConfig {
        tier: QualityTier::Live,
    }
}

fn run_block(
    module: &mut EnvAdsr,
    gate: f32,
    attack_ms: f32,
    decay_ms: f32,
    sustain: f32,
    release_ms: f32,
) -> [f32; BLOCK] {
    let gate_sig = [Signal::Scalar(gate)];
    let params = [
        Signal::Scalar(attack_ms),
        Signal::Scalar(decay_ms),
        Signal::Scalar(sustain),
        Signal::Scalar(release_ms),
        Signal::Scalar(0.0),
    ];
    let mut buf = [0f32; BLOCK];
    let mut outputs: [&mut [f32]; 1] = [&mut buf];
    let mut io = ProcessIo::new(&gate_sig, &mut outputs, &params, BLOCK);
    module.process(&mut io);
    buf
}

#[test]
fn reaches_sustain_level_and_releases_to_zero() {
    let mut module = EnvAdsr::new();
    module.prepare(SAMPLE_RATE, BLOCK, &quality());

    // Enough blocks for attack+decay to fully settle at these fast times.
    let mut last = [0f32; BLOCK];
    for _ in 0..200 {
        last = run_block(&mut module, 1.0, 5.0, 5.0, 0.5, 10.0);
    }
    assert!(
        (last[BLOCK - 1] - 0.5).abs() < 1e-2,
        "should have settled near the sustain level, got {}",
        last[BLOCK - 1]
    );

    for _ in 0..200 {
        last = run_block(&mut module, 0.0, 5.0, 5.0, 0.5, 10.0);
    }
    assert!(
        last[BLOCK - 1] < 1e-3,
        "should have released to (near) zero, got {}",
        last[BLOCK - 1]
    );
}

#[test]
fn reset_returns_to_idle_at_zero() {
    let mut module = EnvAdsr::new();
    module.prepare(SAMPLE_RATE, BLOCK, &quality());
    for _ in 0..50 {
        run_block(&mut module, 1.0, 5.0, 5.0, 0.5, 10.0);
    }
    module.reset();
    let out = run_block(&mut module, 0.0, 5.0, 5.0, 0.5, 10.0);
    assert_eq!(
        out[0], 0.0,
        "a freshly-reset envelope with no gate should output 0"
    );
}

#[test]
fn save_and_load_state_continues_from_the_same_level_and_stage() {
    let mut module_a = EnvAdsr::new();
    module_a.prepare(SAMPLE_RATE, BLOCK, &quality());
    for _ in 0..30 {
        run_block(&mut module_a, 1.0, 20.0, 20.0, 0.6, 30.0);
    }

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
    module_a.save_state(&mut state);

    let mut module_b = EnvAdsr::new();
    module_b.prepare(SAMPLE_RATE, BLOCK, &quality());
    module_b.load_state(&state);

    // Continuing with the gate held should track closely (not identically — module_b doesn't
    // know module_a's exact stage-transition history within the current segment, only its
    // level/stage snapshot, so the coefficients are freshly set via set_params on the next
    // process() call rather than mid-segment-identical).
    let next_a = run_block(&mut module_a, 1.0, 20.0, 20.0, 0.6, 30.0);
    let next_b = run_block(&mut module_b, 1.0, 20.0, 20.0, 0.6, 30.0);
    assert!(
        (next_a[BLOCK - 1] - next_b[BLOCK - 1]).abs() < 1e-2,
        "loaded state should continue close to the original: a={} b={}",
        next_a[BLOCK - 1],
        next_b[BLOCK - 1]
    );
}
