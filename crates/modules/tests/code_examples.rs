//! D11 PR 5: the example modules load from their committed blobs and behave.

use kabl_modules::code::host;
use kabl_modules::{registry, Module, ProcessIo, QualityConfig, QualityTier, Rate, Signal};

const SR: f32 = 48_000.0;
const N: usize = 128;

fn make(kind: &str) -> Box<dyn Module> {
    host::register_examples().expect("examples register");
    let mut m = registry::create(kind).expect("registered");
    m.prepare(
        SR,
        N,
        &QualityConfig {
            tier: QualityTier::Live,
        },
    );
    m
}

fn sine(freq: f32, offset: usize) -> Vec<f32> {
    (0..N)
        .map(|i| (std::f32::consts::TAU * freq * (i + offset) as f32 / SR).sin() * 0.5)
        .collect()
}

fn rms(v: &[f32]) -> f32 {
    (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt()
}

#[test]
fn metadata_has_labels_and_the_declared_shape() {
    host::register_examples().unwrap();
    let logic = registry::info_for("demo.logic").unwrap();
    assert_eq!(logic.name, "Logic");
    assert_eq!(logic.rate, Rate::Global);
    assert_eq!(logic.ports.len(), 5);
    assert_eq!(logic.params.len(), 3);
    let bf = registry::info_for("demo.bitfilter").unwrap();
    assert_eq!(bf.name, "Bit Filter");
    assert_eq!(bf.rate, Rate::Voice);
    assert_eq!(bf.advanced, ["drive"]);
    assert_eq!(bf.ports.iter().map(|p| p.width as usize).sum::<usize>(), 5);
}

/// Runs `blocks` blocks of the filter on a left sine and returns the left output.
fn filter_run(m: &mut dyn Module, freq: f32, params: [f32; 3], blocks: usize) -> Vec<f32> {
    let mut out = Vec::new();
    for b in 0..blocks {
        let l = sine(freq, b * N);
        let r = sine(freq, b * N);
        let cv = vec![0.0f32; N];
        let ins = [Signal::Buffer(&l), Signal::Buffer(&r), Signal::Buffer(&cv)];
        let ps = params.map(Signal::Scalar);
        let (mut ol, mut or) = (vec![0.0f32; N], vec![0.0f32; N]);
        {
            let mut outs: [&mut [f32]; 2] = [&mut ol, &mut or];
            let mut io = ProcessIo::new(&ins, &mut outs, &ps, N);
            m.process(&mut io);
        }
        out.extend(ol);
    }
    out
}

#[test]
fn bitfilter_passes_lows_cuts_highs_and_stays_finite() {
    let mut m = make("demo.bitfilter");
    let low = filter_run(&mut *m, 200.0, [2000.0, 0.2, 0.0], 8);
    assert!(low.iter().all(|v| v.is_finite()));
    assert!(rms(&low[N * 4..]) > 0.25, "low tone should pass");
    m.reset();
    let high = filter_run(&mut *m, 12_000.0, [2000.0, 0.2, 0.0], 8);
    assert!(high.iter().all(|v| v.is_finite()));
    assert!(rms(&high[N * 4..]) < 0.02, "12 kHz should be cut hard");
    // Drive and full resonance stay finite and audible.
    m.reset();
    let hot = filter_run(&mut *m, 500.0, [800.0, 1.0, 1.0], 16);
    assert!(hot.iter().all(|v| v.is_finite()));
    assert!(rms(&hot[N * 8..]) > 0.05);
}

#[test]
fn bitfilter_state_survives_blocks_and_reset_clears_it() {
    let mut m = make("demo.bitfilter");
    let a = filter_run(&mut *m, 300.0, [1500.0, 0.5, 0.0], 3);
    m.reset();
    let b = filter_run(&mut *m, 300.0, [1500.0, 0.5, 0.0], 3);
    assert_eq!(a, b, "reset returns to the fresh state");
}

fn logic_run(m: &mut dyn Module, a: f32, b: f32, gate: f32, params: [f32; 3]) -> (f32, f32) {
    let ins = [Signal::Scalar(a), Signal::Scalar(b), Signal::Scalar(gate)];
    let ps = params.map(Signal::Scalar);
    let (mut out, mut trig) = (vec![0.0f32; N], vec![0.0f32; N]);
    {
        let mut outs: [&mut [f32]; 2] = [&mut out, &mut trig];
        let mut io = ProcessIo::new(&ins, &mut outs, &ps, N);
        m.process(&mut io);
    }
    assert!(out.iter().chain(&trig).all(|v| v.is_finite()));
    (out[N - 1], trig[N - 1])
}

#[test]
fn logic_modes_offset_and_latch() {
    let mut m = make("demo.logic");
    assert_eq!(
        logic_run(&mut *m, 0.2, 0.8, 0.0, [0.0, 0.5, 0.0]),
        (0.2, 0.0)
    );
    assert_eq!(
        logic_run(&mut *m, 0.2, 0.8, 0.0, [1.0, 0.5, 0.0]),
        (0.8, 1.0)
    );
    assert_eq!(
        logic_run(&mut *m, 0.9, 0.8, 0.0, [2.0, 0.5, 0.0]),
        (1.0, 1.0)
    ); // AND
    assert_eq!(
        logic_run(&mut *m, 0.9, 0.8, 0.0, [3.0, 0.5, 0.0]),
        (0.0, 0.0)
    ); // XOR
    assert_eq!(
        logic_run(&mut *m, 0.9, 0.1, 0.0, [3.0, 0.5, 0.0]),
        (1.0, 1.0)
    );
    assert_eq!(
        logic_run(&mut *m, 0.9, 0.1, 0.0, [4.0, 0.5, 0.25]),
        (1.25, 1.0)
    ); // threshold
       // Gate high freezes the output at its last value.
    assert_eq!(
        logic_run(&mut *m, 0.0, 0.0, 1.0, [4.0, 0.5, 0.25]),
        (1.25, 1.0)
    );
    m.reset();
    assert_eq!(
        logic_run(&mut *m, 0.0, 0.0, 1.0, [4.0, 0.5, 0.0]),
        (0.0, 0.0)
    );
}
