//! `osc.va` correctness: the trait-based path matches `dsp::FullOsc` called directly, all four
//! waveforms select correctly, hard sync resets phase, and `save_state`/`load_state` round-trip
//! phase and the sync edge-detect flag (brief section 7.5: state carry-over across recompiles).

use std::collections::HashMap;

use kabl_modules::builtins::OscVa;
use kabl_modules::dsp::{FullOsc, OscWaveform, Saw};
use kabl_modules::module::{QualityConfig, QualityTier, StateReader, StateWriter};
use kabl_modules::{Module, ProcessIo, Signal};

const SAMPLE_RATE: f32 = 48000.0;
const BLOCK: usize = 64;
/// `waveform` param value for Saw — matches `osc_va.rs`'s `waveform_from_param` mapping
/// (`0=Sine, 1=Triangle, 2=Saw, 3=Square`), also this module's pre-existing default.
const SAW: f32 = 2.0;
const SQUARE: f32 = 3.0;
const TRIANGLE: f32 = 1.0;
const SINE: f32 = 0.0;

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

/// Runs one block: `pitch` and `sync` as given, `base_hz`/`waveform` as given. Returns the
/// output buffer.
fn run_block(
    module: &mut OscVa,
    pitch: Signal,
    sync: Signal,
    base_hz: f32,
    waveform: f32,
) -> [f32; BLOCK] {
    let inputs = [pitch, sync];
    let params = [
        Signal::Scalar(base_hz),
        Signal::Scalar(waveform),
        Signal::Scalar(50.0),
        Signal::Scalar(0.0),
        Signal::Scalar(1.0),
        Signal::Scalar(15.0),
    ];
    let mut buf = [0f32; BLOCK];
    let mut outputs: [&mut [f32]; 1] = [&mut buf];
    let mut io = ProcessIo::new(&inputs, &mut outputs, &params, BLOCK);
    module.process(&mut io);
    buf
}

#[test]
fn matches_direct_saw_usage_at_zero_pitch() {
    let mut module = OscVa::new();
    module.prepare(SAMPLE_RATE, BLOCK, &quality());

    let mut reference = Saw::new();

    for _ in 0..10 {
        let buf = run_block(
            &mut module,
            Signal::Scalar(0.0),
            Signal::Scalar(0.0),
            261.63,
            SAW,
        );
        for &sample in &buf {
            let expected = reference.next(261.63, SAMPLE_RATE);
            assert_eq!(
                sample, expected,
                "Module-trait path should match dsp::Saw exactly at the saw waveform"
            );
        }
    }
}

#[test]
fn pitch_input_shifts_frequency_by_semitones() {
    let mut module = OscVa::new();
    module.prepare(SAMPLE_RATE, BLOCK, &quality());

    // +12 semitones = one octave up = double frequency.
    let buf = run_block(
        &mut module,
        Signal::Scalar(12.0),
        Signal::Scalar(0.0),
        220.0,
        SAW,
    );

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
    let buf = run_block(
        &mut module,
        Signal::Buffer(&pitch_buf),
        Signal::Scalar(0.0),
        220.0,
        SAW,
    );

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

    run_block(
        &mut module,
        Signal::Scalar(0.0),
        Signal::Scalar(0.0),
        261.63,
        SAW,
    );
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
    run_block(
        &mut module_a,
        Signal::Scalar(0.0),
        Signal::Scalar(0.0),
        261.63,
        SAW,
    );

    let mut state = MapState(HashMap::new());
    module_a.save_state(&mut state);

    // A fresh module instance, as if a graph recompile built a new one — loading state should
    // let it continue seamlessly rather than restart from phase 0 (brief section 7.5: "a
    // ringing filter [here: an oscillating oscillator] survives a repatch").
    let mut module_b = OscVa::new();
    module_b.prepare(SAMPLE_RATE, BLOCK, &quality());
    module_b.load_state(&state);

    let buf_a_next = run_block(
        &mut module_a,
        Signal::Scalar(0.0),
        Signal::Scalar(0.0),
        261.63,
        SAW,
    );
    let buf_b_next = run_block(
        &mut module_b,
        Signal::Scalar(0.0),
        Signal::Scalar(0.0),
        261.63,
        SAW,
    );

    assert_eq!(
        buf_a_next, buf_b_next,
        "loading saved state into a fresh module should continue identically to the original"
    );
}

#[test]
fn square_waveform_matches_direct_full_osc_usage() {
    let mut module = OscVa::new();
    module.prepare(SAMPLE_RATE, BLOCK, &quality());
    let mut reference = FullOsc::new();

    for _ in 0..5 {
        let buf = run_block(
            &mut module,
            Signal::Scalar(0.0),
            Signal::Scalar(0.0),
            330.0,
            SQUARE,
        );
        for &sample in &buf {
            let expected = reference.next(330.0, SAMPLE_RATE, OscWaveform::Square);
            assert_eq!(
                sample, expected,
                "square waveform should match FullOsc directly"
            );
        }
    }
}

#[test]
fn triangle_waveform_matches_direct_full_osc_usage() {
    let mut module = OscVa::new();
    module.prepare(SAMPLE_RATE, BLOCK, &quality());
    let mut reference = FullOsc::new();

    for _ in 0..5 {
        let buf = run_block(
            &mut module,
            Signal::Scalar(0.0),
            Signal::Scalar(0.0),
            330.0,
            TRIANGLE,
        );
        for &sample in &buf {
            let expected = reference.next(330.0, SAMPLE_RATE, OscWaveform::Triangle);
            assert_eq!(
                sample, expected,
                "triangle waveform should match FullOsc directly"
            );
        }
    }
}

#[test]
fn sine_waveform_matches_direct_full_osc_usage() {
    let mut module = OscVa::new();
    module.prepare(SAMPLE_RATE, BLOCK, &quality());
    let mut reference = FullOsc::new();

    for _ in 0..5 {
        let buf = run_block(
            &mut module,
            Signal::Scalar(0.0),
            Signal::Scalar(0.0),
            330.0,
            SINE,
        );
        for &sample in &buf {
            let expected = reference.next(330.0, SAMPLE_RATE, OscWaveform::Sine);
            assert_eq!(
                sample, expected,
                "sine waveform should match FullOsc directly"
            );
        }
    }
}

#[test]
fn square_waveform_stays_in_range_and_has_correct_period() {
    // Sanity check independent of the internal reference: a 100Hz square at 48kHz should have a
    // period of 480 samples, and every sample should be within the PolyBLEP correction's
    // bounded overshoot of [-1, 1] (PolyBLEP can overshoot slightly right at the correction
    // window, unlike a naive square, so allow a small margin rather than asserting exactly 1.0).
    let mut module = OscVa::new();
    module.prepare(SAMPLE_RATE, BLOCK, &quality());

    let mut samples = Vec::with_capacity(BLOCK * 20);
    for _ in 0..20 {
        let buf = run_block(
            &mut module,
            Signal::Scalar(0.0),
            Signal::Scalar(0.0),
            100.0,
            SQUARE,
        );
        samples.extend_from_slice(&buf);
    }

    assert!(
        samples.iter().all(|v| v.is_finite() && v.abs() <= 1.2),
        "square wave should stay near [-1, 1] (PolyBLEP's small correction overshoot allowed)"
    );

    let mut sign_changes = 0;
    for w in samples.windows(2) {
        if w[0].signum() != w[1].signum() && w[0] != 0.0 && w[1] != 0.0 {
            sign_changes += 1;
        }
    }
    // 100Hz over 20 blocks (1280 samples =~ 26.7ms) covers ~2.67 periods -> ~5.3 sign changes.
    assert!(
        (3..=8).contains(&sign_changes),
        "expected roughly 5 sign changes for a 100Hz square over 1280 samples, got {sign_changes}"
    );
}

#[test]
fn triangle_waveform_is_smooth_and_bounded() {
    let mut module = OscVa::new();
    module.prepare(SAMPLE_RATE, BLOCK, &quality());

    let buf = run_block(
        &mut module,
        Signal::Scalar(0.0),
        Signal::Scalar(0.0),
        220.0,
        TRIANGLE,
    );

    assert!(
        buf.iter()
            .all(|v| v.is_finite() && (-1.0..=1.0).contains(v)),
        "naive triangle should stay exactly within [-1, 1]"
    );
    // No sample-to-sample jump should ever exceed the triangle's steepest slope
    // (4.0 * dt, the piecewise-linear rise/fall rate) by more than a small margin.
    let dt = 220.0 / SAMPLE_RATE;
    let max_step = 4.0 * dt * 1.1;
    for w in buf.windows(2) {
        assert!(
            (w[1] - w[0]).abs() <= max_step,
            "triangle should move smoothly, got a step of {}",
            (w[1] - w[0]).abs()
        );
    }
}

#[test]
fn hard_sync_resets_phase_on_next_rising_edge() {
    let mut module = OscVa::new();
    module.prepare(SAMPLE_RATE, BLOCK, &quality());

    // Run a few blocks with sync low so phase advances well away from 0.
    run_block(
        &mut module,
        Signal::Scalar(0.0),
        Signal::Scalar(0.0),
        220.0,
        SAW,
    );
    run_block(
        &mut module,
        Signal::Scalar(0.0),
        Signal::Scalar(0.0),
        220.0,
        SAW,
    );
    let mut state = MapState(HashMap::new());
    module.save_state(&mut state);
    assert_ne!(state.0["phase"], 0.0, "phase should have advanced by now");

    // A sync buffer that's high only at sample 10 -- the rising edge should hard-sync the
    // oscillator right at that sample, so the output there equals a fresh oscillator's phase-0
    // sample for the saw waveform (2*0 - 1 = -1, modulo the PolyBLEP correction at phase 0).
    let mut sync_buf = [0f32; BLOCK];
    sync_buf[10] = 1.0;
    let buf = run_block(
        &mut module,
        Signal::Scalar(0.0),
        Signal::Buffer(&sync_buf),
        220.0,
        SAW,
    );

    // The edge lies halfway between samples 9 and 10 (0 → 1 crosses 0.5 at 0.5), so the
    // phase restarts there: at sample 10 it is half a sample's increment, and the block ends
    // (BLOCK - 10) increments later. A second oscillator at a different phase, synced by the
    // same edge, plays the same samples from 11 on.
    let dt = 220.0 / SAMPLE_RATE;
    let mut state = MapState(HashMap::new());
    module.save_state(&mut state);
    let expected = (0.5 + (BLOCK - 10) as f32) * dt;
    assert!((state.0["phase"] - expected).abs() < 1e-5);
    let mut other = OscVa::new();
    other.prepare(SAMPLE_RATE, BLOCK, &quality());
    let other_buf = run_block(
        &mut other,
        Signal::Scalar(0.0),
        Signal::Buffer(&sync_buf),
        220.0,
        SAW,
    );
    for i in 11..BLOCK {
        assert!((buf[i] - other_buf[i]).abs() < 1e-5, "sample {i}");
    }
    // The samples either side of the edge are band-limited, not a bare jump to -1.
    assert!(buf[10] > -1.0 && buf[9] < buf[8] + 0.1);
}

#[test]
fn save_and_load_state_round_trips_sync_edge_flag() {
    let mut module_a = OscVa::new();
    module_a.prepare(SAMPLE_RATE, BLOCK, &quality());

    // Leave sync held high through the end of the block -- sync_was_high should be true.
    let mut sync_buf = [0f32; BLOCK];
    sync_buf[BLOCK - 1] = 1.0;
    run_block(
        &mut module_a,
        Signal::Scalar(0.0),
        Signal::Buffer(&sync_buf),
        220.0,
        SAW,
    );

    let mut state = MapState(HashMap::new());
    module_a.save_state(&mut state);
    assert_eq!(state.0["sync_was_high"], 1.0);

    let mut module_b = OscVa::new();
    module_b.prepare(SAMPLE_RATE, BLOCK, &quality());
    module_b.load_state(&state);

    // With sync held high continuously into the next block, a fresh instance that didn't load
    // sync_was_high would see a false rising edge and hard-sync (a bug this exact test would
    // catch -- see docs/decisions.md's env.adsr gate_was_high entry for why this matters).
    let sync_still_high = [1.0f32; BLOCK];
    let buf_a = run_block(
        &mut module_a,
        Signal::Scalar(0.0),
        Signal::Buffer(&sync_still_high),
        220.0,
        SAW,
    );
    let buf_b = run_block(
        &mut module_b,
        Signal::Scalar(0.0),
        Signal::Buffer(&sync_still_high),
        220.0,
        SAW,
    );
    assert_eq!(
        buf_a, buf_b,
        "a continuously-held sync line must not look like a fresh edge after state load"
    );
}
