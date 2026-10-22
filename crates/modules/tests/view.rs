//! Display data (`Module::view`): the wavetable's position and one cycle of output, an FM
//! operator's cycle, nothing before a note, and the factory tables for drawing.

mod common;
use common::*;
use kabl_modules::view::{ModuleView, VIEW_CYCLE};
use kabl_modules::wavetable;
use std::f32::consts::TAU;

fn view(r: &Rig) -> ModuleView {
    let mut v = ModuleView::default();
    r.m.view(&mut v);
    v
}

#[test]
fn osc_wt_shows_its_position_and_a_cycle_of_the_table_frame() {
    // Table 0, position 0: the first frame is a sine-free saw/square; one cycle of the output
    // must have the unit period of the frame: 375 Hz = 128 samples.
    let mut r = Rig::new("osc.wt", &[("base_hz", 375.0), ("position", 0.25)], SR);
    assert!(!view(&r).valid);
    r.render(9600, 0, |_, _| 0.0);
    let v = view(&r);
    assert!(v.valid);
    assert!((v.position - 0.25).abs() < 1e-3, "{}", v.position);
    assert!(peak(&v.cycle) > 0.3 && v.cycle.iter().all(|x| x.is_finite()));
    // The cycle repeats the output: take it from the rendered signal one period earlier.
    let x = r.render(512, 0, |_, _| 0.0);
    let v2 = view(&r);
    let dot: f32 = v.cycle.iter().zip(&v2.cycle).map(|(a, b)| a * b).sum();
    let n: f32 = v.cycle.iter().map(|a| a * a).sum();
    assert!(dot / n > 0.99, "{}", dot / n);
    assert!(peak(&x) > 0.3);
}

#[test]
fn osc_wt_position_follows_the_pos_input() {
    let mut r = Rig::new("osc.wt", &[("position", 0.0)], SR);
    r.render(4800, 0, |k, _| if k == 1 { 0.5 } else { 0.0 });
    let v = view(&r);
    assert!((v.position - 0.5).abs() < 0.01, "{}", v.position);
}

#[test]
fn osc_fm_shows_one_sine_cycle() {
    let mut r = Rig::new("osc.fm", &[("base_hz", 375.0)], SR);
    assert!(!view(&r).valid);
    r.render(9600, 0, |_, _| 0.0);
    let v = view(&r);
    assert!(v.valid);
    for (k, &y) in v.cycle.iter().enumerate() {
        let want = (TAU * k as f32 / VIEW_CYCLE as f32).sin();
        assert!((y - want).abs() < 0.05, "point {k}: {y} vs {want}");
    }
}

#[test]
fn modules_without_a_picture_stay_invalid() {
    let mut r = Rig::new("vca", &[], SR);
    r.render(256, 0, |_, _| 0.5);
    assert!(!view(&r).valid);
}

#[test]
fn factory_frames_decode_for_drawing() {
    for i in 0..wavetable::FACTORY_COUNT {
        let frames = wavetable::factory_frames(i);
        assert!(!frames.is_empty(), "table {i}");
        assert!(frames
            .iter()
            .all(|f| f.len() == wavetable::FRAME && f.iter().all(|v| v.is_finite())));
    }
}

#[test]
fn lfo_shows_its_phase() {
    let mut r = Rig::new("lfo", &[("rate_hz", 1.0)], SR);
    r.render(12000, 0, |_, _| 0.0);
    let v = view(&r);
    assert!(v.valid);
    assert!((v.position - 0.25).abs() < 0.01, "{}", v.position);
}

#[test]
fn envelope_shows_its_stage() {
    let params = [
        ("attack_ms", 10.0),
        // A short decay: a long one stops within f32 rounding of the sustain level and the
        // stage stays Decay (the level, the output, is right either way).
        ("decay_ms", 5.0),
        ("sustain", 0.5),
        ("release_ms", 400.0),
    ];
    let mut r = Rig::new("env.adsr", &params, SR);
    assert_eq!(view(&r).position, 0.0, "idle");
    r.render(240, 0, |_, _| 1.0);
    assert_eq!(view(&r).position, 1.0, "attack");
    r.render(240000, 0, |_, _| 1.0);
    assert_eq!(view(&r).position, 3.0, "sustain");
    r.render(2400, 0, |_, _| 0.0);
    assert_eq!(view(&r).position, 4.0, "release");
}

#[test]
fn limiter_shows_its_gain_reduction() {
    let mut r = Rig::new("limiter", &[("gain_db", 12.0), ("ceiling_db", -6.0)], SR);
    r.render(12000, 0, |_, _| 0.5);
    let v = view(&r);
    assert!(v.valid);
    assert!((v.position - 12.0).abs() < 0.3, "{}", v.position);
    let mut quiet = Rig::new("limiter", &[("gain_db", 0.0), ("ceiling_db", -6.0)], SR);
    quiet.render(12000, 0, |_, _| 0.1);
    assert!(view(&quiet).position < 0.01);
}

#[test]
fn phaser_and_flanger_show_their_lfo_phase() {
    for kind in ["phaser", "flanger"] {
        let mut r = Rig::new(kind, &[("rate_hz", 1.0)], SR);
        r.render(12000, 0, |_, _| 0.0);
        let v = view(&r);
        assert!(v.valid, "{kind}");
        assert!((v.position - 0.25).abs() < 0.01, "{kind} {}", v.position);
    }
}

#[test]
fn eq_curve_matches_what_the_module_does() {
    use kabl_modules::builtins::band_response_db;
    let mut r = Rig::new(
        "eq",
        &[("mid1_hz", 1000.0), ("mid1_db", 12.0), ("mid1_q", 1.0)],
        SR,
    );
    for hz in [300.0f32, 1000.0, 2500.0] {
        let x = r.render(24000, 0, |_, t| (TAU * hz * t as f32 / SR).sin());
        let measured = 20.0 * (rms(&x[12000..]) / std::f32::consts::FRAC_1_SQRT_2).log10();
        let want = band_response_db(1, 1000.0, 12.0, 1.0, SR, hz);
        assert!(
            (measured - want).abs() < 0.3,
            "{hz} Hz: {measured} vs {want}"
        );
    }
    assert!(band_response_db(0, 100.0, 6.0, 1.0, SR, 20.0) > 5.0);
    assert!(band_response_db(3, 8000.0, 6.0, 1.0, SR, 20000.0) > 5.0);
    assert!(band_response_db(1, 400.0, 0.0, 1.0, SR, 3000.0).abs() < 1e-3);
}

#[test]
fn formant_bands_follow_the_vowel() {
    use kabl_modules::builtins::formant_bands;
    let a = formant_bands(0.0, 0.0);
    assert!((a[0].0 - 650.0).abs() < 0.01 && a[0].1 == 0.0 && a[0].2 == 80.0);
    let mid = formant_bands(0.5, 0.0);
    assert!(mid[0].0 < 650.0 && mid[0].0 > 400.0);
    assert!((formant_bands(0.0, 12.0)[0].0 - 1300.0).abs() < 0.1);
}
