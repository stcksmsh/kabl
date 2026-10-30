//! A drum hit that retriggers a voice still ringing leaves no click, wherever the hit lands in
//! the 64-sample block: at offset 0, 63 or anywhere else, and across a sweep of tempos on a
//! 16th grid. A click here is the audio_defects CLICK: one sample whose second difference is far
//! above every other in its neighbourhood.

use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{registry, Module, PortDirection, ProcessIo, Signal};

const SR: f32 = 48000.0;
const BLOCK: usize = 64;
const VOICES: [&str; 5] = [
    "drum.kick",
    "drum.snare",
    "drum.clap",
    "drum.hat",
    "drum.tom",
];

/// Renders `kind` for `len` samples with rising edges at `at` (each held 100 samples), in
/// blocks of 64. The hat's closed gate is input 0 like the others.
fn render(kind: &str, at: &[usize], len: usize) -> Vec<f32> {
    let mut m: Box<dyn Module> = registry::create(kind).unwrap();
    m.prepare(
        SR,
        4096,
        &QualityConfig {
            tier: QualityTier::Live,
        },
    );
    let info = m.info();
    let ps: Vec<Signal> = info
        .params
        .iter()
        .map(|p| Signal::Scalar(p.default))
        .collect();
    let (n_in, n_out) = (
        info.slots(PortDirection::Input),
        info.slots(PortDirection::Output),
    );
    let mut g = vec![0f32; len];
    for &a in at {
        for x in g.iter_mut().skip(a).take(100) {
            *x = 1.0;
        }
    }
    let mut out = vec![vec![0f32; len]; n_out];
    for t in (0..len).step_by(BLOCK) {
        let n = BLOCK.min(len - t);
        let sig: Vec<Signal> = (0..n_in)
            .map(|k| {
                if k == 0 {
                    Signal::Buffer(&g[t..t + n])
                } else {
                    Signal::Scalar(0.0)
                }
            })
            .collect();
        let mut refs: Vec<&mut [f32]> = out.iter_mut().map(|b| &mut b[t..t + n]).collect();
        let mut io = ProcessIo::new(&sig, &mut refs, &ps, n);
        m.process(&mut io);
    }
    out.remove(0)
}

/// The audio_defects CLICK over `lo..hi` (the scanner's 50 ms window): the largest second
/// difference when it is at least -45 dBFS and five times the largest one more than 3 samples
/// away. Returns (spike, ratio, position).
fn click(x: &[f32], lo: usize, hi: usize) -> (f32, f32, usize) {
    let d2 = |i: usize| (x[i] - 2.0 * x[i - 1] + x[i - 2]).abs();
    let at = (lo..hi).max_by(|a, b| d2(*a).total_cmp(&d2(*b))).unwrap();
    let rest = (lo..hi)
        .filter(|i| i.abs_diff(at) > 3)
        .map(d2)
        .fold(0.0, f32::max);
    (d2(at), d2(at) / rest.max(1e-6), at)
}

/// Pushes a message when the hit at `second` over a hit at `first` makes a CLICK (ratio 6, the
/// scanner's 5 plus a margin for the kick's own noise burst) in the 50 ms
/// that follow it which the same hit from silence does not make.
fn check(kind: &str, first: usize, second: usize, bad: &mut Vec<String>) {
    let len = second + 2600;
    let (lo, hi) = (second - 2, second + 2400);
    let (spike, ratio, at) = click(&render(kind, &[first, second], len), lo, hi);
    if spike < 0.0056 || ratio < 6.0 {
        return;
    }
    let (_, alone, _) = click(&render(kind, &[second], len), lo, hi);
    if alone < 5.0 {
        bad.push(format!(
            "{kind}: hit {second} (block offset {}) over a hit at {first}: click {} samples in, {ratio:.1}x its neighbours (alone {alone:.1}x)",
            second % BLOCK,
            at - second
        ));
    }
}

#[test]
fn a_retrigger_on_any_sample_of_the_block_makes_no_click() {
    let mut bad = Vec::new();
    for kind in VOICES {
        for first in [100usize, 131] {
            for second in 200..3400usize {
                if matches!(second % BLOCK, 0 | 1 | 31 | 32 | 62 | 63) {
                    check(kind, first, second, &mut bad);
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "{} clicks, first {:?}",
        bad.len(),
        &bad[..bad.len().min(6)]
    );
}

#[test]
fn a_16th_grid_at_any_tempo_makes_no_click() {
    let mut bad = Vec::new();
    for kind in VOICES {
        for tenth in 600..=2000usize {
            let bpm = tenth as f32 / 10.0;
            let step = (60.0 / bpm / 4.0 * SR).round() as usize;
            check(kind, 100, 100 + step, &mut bad);
            // The next grid hit lands where the clock puts it, two steps after the first.
            check(kind, 100, 100 + 2 * step, &mut bad);
        }
    }
    assert!(
        bad.is_empty(),
        "{} clicks, first {:?}",
        bad.len(),
        &bad[..bad.len().min(6)]
    );
}
