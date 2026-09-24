//! `gain`: unity by default (exact), dB scaling, smooth changes, state carry.

use kabl_modules::builtins::{db_to_gain, Gain};
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{Module, ProcessIo, Signal};

fn run(g: &mut Gain, input: &[f32], db: impl Fn(usize) -> f32) -> Vec<f32> {
    let mut out = vec![0.0; input.len()];
    for (k, chunk) in input.chunks(64).enumerate() {
        let ins = [Signal::Buffer(chunk)];
        let ps = [Signal::Scalar(db(k * 64))];
        let mut o = vec![0.0; 64];
        let mut outs = [&mut o[..]];
        g.process(&mut ProcessIo::new(&ins, &mut outs, &ps, chunk.len()));
        out[k * 64..k * 64 + chunk.len()].copy_from_slice(&o[..chunk.len()]);
    }
    out
}

fn gain() -> Gain {
    let mut g = Gain::new();
    g.prepare(48000.0, 64, &QualityConfig { tier: QualityTier::Live });
    g
}

fn sine(n: usize) -> Vec<f32> {
    (0..n).map(|i| (i as f32 * 0.05).sin() * 0.5).collect()
}

#[test]
fn default_is_exactly_unity_and_db_scales() {
    let x = sine(4800);
    assert_eq!(run(&mut gain(), &x, |_| 0.0), x);
    let up = run(&mut gain(), &x, |_| 6.0);
    assert!((up[100] / x[100] - db_to_gain(6.0)).abs() < 1e-5);
    assert!((db_to_gain(6.0) - 1.9953).abs() < 1e-3);
    assert!((db_to_gain(-20.0) - 0.1).abs() < 1e-6);
}

#[test]
fn changes_glide_without_steps() {
    let x = vec![1.0; 48000];
    let y = run(&mut gain(), &x, |i| if i < 24000 { 0.0 } else { 18.0 });
    let step = y.windows(2).fold(0f32, |m, w| m.max((w[1] - w[0]).abs()));
    assert!(step < 0.01, "{step}");
    assert!((y[47999] - db_to_gain(18.0)).abs() < 1e-3);
}

#[test]
fn state_carries_the_smoothed_gain() {
    let x = vec![1.0; 640];
    let mut a = gain();
    run(&mut a, &x, |_| 12.0);
    let mut s = kabl_modules::StateBuf::default();
    a.save_state(&mut s);
    let mut b = gain();
    b.load_state(&s);
    assert_eq!(run(&mut a, &x, |_| 12.0), run(&mut b, &x, |_| 12.0));
}
