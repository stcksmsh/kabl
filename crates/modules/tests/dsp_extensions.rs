//! Correctness checks for the DSP primitives added to back the 8 remaining built-in modules
//! (`SvfOutputs`, `FullAdsr`, `FullLfo`) — checked before wrapping them in `Module` impls, same
//! order as `osc.va`'s development (dsp primitive proven correct first, then the trait wrapper).

use kabl_modules::dsp::{AdsrStage, FullAdsr, FullLfo, LfoWaveform, Svf, SvfCoeffs};

const SAMPLE_RATE: f32 = 48000.0;

#[test]
fn svf_multi_lowpass_matches_existing_lowpass_only_path() {
    // process_with_coeffs (S1/S2/S3-tested) and process_with_coeffs_multi's .lowpass field must
    // agree exactly — same recurrence, just also returning bp/hp.
    let coeffs = SvfCoeffs::compute(1200.0, 0.4, SAMPLE_RATE);
    let mut svf_a = Svf::new();
    let mut svf_b = Svf::new();

    for i in 0..1000 {
        let input = (i as f32 * 0.017).sin() * 0.8;
        let lp_only = svf_a.process_with_coeffs(input, &coeffs);
        let multi = svf_b.process_with_coeffs_multi(input, &coeffs);
        assert_eq!(lp_only, multi.lowpass, "sample {i}");
    }
}

#[test]
fn svf_multi_outputs_satisfy_the_hp_bp_lp_identity() {
    // hp = input - k*bp - lp is the defining identity of this topology (Cytomic/Zavalishin) —
    // if this doesn't hold, the highpass derivation is wrong.
    let coeffs = SvfCoeffs::compute(800.0, 0.5, SAMPLE_RATE);
    let mut svf = Svf::new();
    for i in 0..500 {
        let input = if i % 37 == 0 { 1.0 } else { 0.0 } - 0.02; // impulses on a small DC offset
        let out = svf.process_with_coeffs_multi(input, &coeffs);
        let reconstructed = input - coeffs.k * out.bandpass - out.lowpass;
        assert!(
            (reconstructed - out.highpass).abs() < 1e-5,
            "sample {i}: hp identity violated, got hp={} expected={}",
            out.highpass,
            reconstructed
        );
    }
}

#[test]
fn svf_lowpass_attenuates_high_frequency_more_than_low_frequency() {
    // Sanity check the three outputs aren't just noise: a 200Hz-cutoff lowpass should pass a
    // slow (20Hz-ish) sustained tone at much higher RMS than a fast (5kHz) one.
    let coeffs = SvfCoeffs::compute(200.0, 0.3, SAMPLE_RATE);

    let rms_at = |freq_hz: f32| -> f32 {
        let mut svf = Svf::new();
        let mut phase = 0.0f32;
        let mut sum_sq = 0.0f64;
        let n = 4000;
        for _ in 0..n {
            let input = (2.0 * std::f32::consts::PI * phase).sin();
            phase += freq_hz / SAMPLE_RATE;
            let out = svf.process_with_coeffs_multi(input, &coeffs);
            sum_sq += (out.lowpass as f64) * (out.lowpass as f64);
        }
        ((sum_sq / n as f64).sqrt()) as f32
    };

    let low = rms_at(20.0);
    let high = rms_at(5000.0);
    assert!(
        low > high * 5.0,
        "200Hz lowpass should pass 20Hz much better than 5kHz; got low={low}, high={high}"
    );
}

#[test]
fn full_adsr_goes_through_all_four_stages_and_back_to_idle() {
    let mut env = FullAdsr::new(10.0, 20.0, 0.6, 30.0, SAMPLE_RATE);
    assert_eq!(env.stage, AdsrStage::Idle);

    // Gate on: attack, then decay, then settle in sustain at 0.6. Convergence to within
    // FULL_ADSR_SEGMENT_EPSILON of each segment's target takes several e-fold times: ~4400
    // samples for a 10ms attack, ~8000 more for a 20ms decay at these settings — budget
    // generously rather than tune this test to the exact time constants.
    let mut saw_attack = false;
    let mut saw_decay = false;
    for _ in 0..50000 {
        env.next(1.0);
        if env.stage == AdsrStage::Attack {
            saw_attack = true;
        }
        if env.stage == AdsrStage::Decay {
            saw_decay = true;
        }
        if env.stage == AdsrStage::Sustain {
            break;
        }
    }
    assert!(
        saw_attack && saw_decay,
        "should pass through attack and decay on the way to sustain"
    );
    assert_eq!(env.stage, AdsrStage::Sustain);
    assert!(
        (env.level - 0.6).abs() < 1e-3,
        "should settle at the sustain level"
    );

    // Gate off: release back toward 0, ending Idle.
    for _ in 0..50000 {
        env.next(0.0);
        if env.stage == AdsrStage::Idle {
            break;
        }
    }
    assert_eq!(env.stage, AdsrStage::Idle);
    assert_eq!(env.level, 0.0);
}

#[test]
fn full_lfo_waveforms_stay_in_range_and_differ() {
    // 50Hz, not the 2Hz a musical LFO usually runs at: S&H only draws a new random value once
    // per cycle, and at 2Hz that's just 2 draws over 48000 samples — not enough to reliably
    // exceed a range threshold. 50Hz over 48000 samples gives ~50 draws, comfortably enough.
    for waveform in [
        LfoWaveform::Sine,
        LfoWaveform::Triangle,
        LfoWaveform::Saw,
        LfoWaveform::Square,
        LfoWaveform::SampleAndHold,
    ] {
        let mut lfo = FullLfo::new();
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for _ in 0..48000 {
            let v = lfo.next(50.0, SAMPLE_RATE, waveform);
            assert!(v.is_finite(), "{waveform:?} produced non-finite output");
            min = min.min(v);
            max = max.max(v);
        }
        assert!(
            min >= -1.0001 && max <= 1.0001,
            "{waveform:?} out of [-1,1]: [{min}, {max}]"
        );
        assert!(max - min > 0.5, "{waveform:?} barely moved: [{min}, {max}]");
    }
}

#[test]
fn full_lfo_sample_and_hold_changes_only_once_per_cycle() {
    let mut lfo = FullLfo::new();
    let rate_hz = 4.0;
    let samples_per_cycle = (SAMPLE_RATE / rate_hz) as usize;

    let mut changes = 0;
    let mut last = lfo.next(rate_hz, SAMPLE_RATE, LfoWaveform::SampleAndHold);
    for _ in 0..(samples_per_cycle * 5) {
        let v = lfo.next(rate_hz, SAMPLE_RATE, LfoWaveform::SampleAndHold);
        if v != last {
            changes += 1;
        }
        last = v;
    }
    // ~5 cycles => ~5 changes, not one per sample.
    assert!(
        (1..=6).contains(&changes),
        "S&H should change roughly once per cycle (~5 expected), got {changes} changes"
    );
}
