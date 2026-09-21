//! First end-to-end exercise of the `Module` trait/`ProcessIo` design (brief section 8), via the
//! `osc.va` module. Checks: the trait-based path produces identical output to calling `dsp::Saw`
//! directly (the trait plumbing doesn't change the math), that `ProcessIo`'s scalar-vs-buffer
//! input handling both work, and that `save_state`/`load_state` round-trip phase correctly
//! (brief section 7.5: state carry-over across recompiles, generalized from what S1's spike
//! hand-rolled).

use std::collections::HashMap;

use kabl_modules::builtins::OscVa;
use kabl_modules::dsp::Saw;
use kabl_modules::module::{QualityConfig, QualityTier, StateReader, StateWriter};
use kabl_modules::{Module, ProcessIo, Signal};

const SAMPLE_RATE: f32 = 48000.0;
const BLOCK: usize = 64;

struct MapState(HashMap<String, f32>);
impl StateWriter for MapState {
    fn write_f32(&mut self, key: &str, value: f32) {
        self.0.insert(key.to_string(), value);
    }
}
impl StateReader for MapState {
    fn read_f32(&self, key: &str) -> Option<f32> {
        self.0.get(key).copied()
    }
}

fn quality() -> QualityConfig {
    QualityConfig {
        tier: QualityTier::Live,
    }
}

#[test]
fn matches_direct_saw_usage_at_zero_pitch() {
    let mut module = OscVa::new();
    module.prepare(SAMPLE_RATE, BLOCK, &quality());

    let mut reference = Saw::new();

    for _ in 0..10 {
        let pitch = [Signal::Scalar(0.0)]; // 0 semitones -> base_hz param (261.63, C4)
        let base_hz = [Signal::Scalar(261.63)];
        let mut buf = [0f32; BLOCK];
        {
            let mut outputs: [&mut [f32]; 1] = [&mut buf];
            let mut io = ProcessIo::new(&pitch, &mut outputs, &base_hz, BLOCK);
            module.process(&mut io);
        }

        for &sample in &buf {
            let expected = reference.next(261.63, SAMPLE_RATE);
            assert_eq!(
                sample, expected,
                "Module-trait path should match dsp::Saw exactly"
            );
        }
    }
}

#[test]
fn pitch_input_shifts_frequency_by_semitones() {
    let mut module = OscVa::new();
    module.prepare(SAMPLE_RATE, BLOCK, &quality());

    // +12 semitones = one octave up = double frequency.
    let pitch = [Signal::Scalar(12.0)];
    let base_hz = [Signal::Scalar(220.0)];
    let mut buf = [0f32; BLOCK];
    {
        let mut outputs: [&mut [f32]; 1] = [&mut buf];
        let mut io = ProcessIo::new(&pitch, &mut outputs, &base_hz, BLOCK);
        module.process(&mut io);
    }

    let mut reference = Saw::new();
    let mut expected_buf = [0f32; BLOCK];
    for slot in expected_buf.iter_mut() {
        *slot = reference.next(440.0, SAMPLE_RATE);
    }

    assert_eq!(
        buf, expected_buf,
        "an octave up should double the effective frequency"
    );
}

#[test]
fn buffer_pitch_input_is_read_per_sample() {
    let mut module = OscVa::new();
    module.prepare(SAMPLE_RATE, BLOCK, &quality());

    // A buffer input (not a scalar) ramping pitch up within the block — exercises
    // Signal::Buffer's per-sample path, not just Signal::Scalar's.
    let pitch_buf: [f32; BLOCK] = std::array::from_fn(|i| i as f32 * (12.0 / BLOCK as f32));
    let pitch = [Signal::Buffer(&pitch_buf)];
    let base_hz = [Signal::Scalar(220.0)];
    let mut buf = [0f32; BLOCK];
    {
        let mut outputs: [&mut [f32]; 1] = [&mut buf];
        let mut io = ProcessIo::new(&pitch, &mut outputs, &base_hz, BLOCK);
        module.process(&mut io);
    }

    let mut reference = Saw::new();
    for (i, &sample) in buf.iter().enumerate() {
        let freq = 220.0 * 2f32.powf(pitch_buf[i] / 12.0);
        let expected = reference.next(freq, SAMPLE_RATE);
        assert_eq!(
            sample, expected,
            "buffer input should be read per-sample, not as a scalar"
        );
    }
}

#[test]
fn reset_returns_to_phase_zero() {
    let mut module = OscVa::new();
    module.prepare(SAMPLE_RATE, BLOCK, &quality());

    let pitch = [Signal::Scalar(0.0)];
    let base_hz = [Signal::Scalar(261.63)];
    let mut buf = [0f32; BLOCK];
    {
        let mut outputs: [&mut [f32]; 1] = [&mut buf];
        let mut io = ProcessIo::new(&pitch, &mut outputs, &base_hz, BLOCK);
        module.process(&mut io);
    }
    // Phase should have advanced from zero by now.
    let mut state = MapState(HashMap::new());
    module.save_state(&mut state);
    assert_ne!(state.0["phase"], 0.0);

    module.reset();
    let mut state_after_reset = MapState(HashMap::new());
    module.save_state(&mut state_after_reset);
    assert_eq!(state_after_reset.0["phase"], 0.0);
}

#[test]
fn save_and_load_state_round_trips_phase() {
    let mut module_a = OscVa::new();
    module_a.prepare(SAMPLE_RATE, BLOCK, &quality());

    let pitch = [Signal::Scalar(0.0)];
    let base_hz = [Signal::Scalar(261.63)];
    let mut buf = [0f32; BLOCK];
    {
        let mut outputs: [&mut [f32]; 1] = [&mut buf];
        let mut io = ProcessIo::new(&pitch, &mut outputs, &base_hz, BLOCK);
        module_a.process(&mut io);
    }

    let mut state = MapState(HashMap::new());
    module_a.save_state(&mut state);

    // A fresh module instance, as if a graph recompile built a new one — loading state should
    // let it continue seamlessly rather than restart from phase 0 (brief section 7.5: "a
    // ringing filter [here: an oscillating oscillator] survives a repatch").
    let mut module_b = OscVa::new();
    module_b.prepare(SAMPLE_RATE, BLOCK, &quality());
    module_b.load_state(&state);

    let mut buf_a_next = [0f32; BLOCK];
    let mut buf_b_next = [0f32; BLOCK];
    {
        let mut outputs: [&mut [f32]; 1] = [&mut buf_a_next];
        let mut io = ProcessIo::new(&pitch, &mut outputs, &base_hz, BLOCK);
        module_a.process(&mut io);
    }
    {
        let mut outputs: [&mut [f32]; 1] = [&mut buf_b_next];
        let mut io = ProcessIo::new(&pitch, &mut outputs, &base_hz, BLOCK);
        module_b.process(&mut io);
    }

    assert_eq!(
        buf_a_next, buf_b_next,
        "loading saved state into a fresh module should continue identically to the original"
    );
}
