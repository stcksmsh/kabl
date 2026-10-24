//! `drum.grid`: the default beat, velocity in the gate level, chance, polymeter, pattern
//! switching, reset, and the drum voices reading that level as velocity.

use kabl_modules::builtins::{cell_index, len_index, make_code, pattern_index, with_code};
use kabl_modules::module::{QualityConfig, QualityTier};
use kabl_modules::{registry, Module, ProcessIo, Signal};

const SR: f32 = 48000.0;
/// Samples per clock edge: 2 high, 6 low.
const STEP: usize = 8;

struct Grid {
    m: Box<dyn Module>,
    params: Vec<f32>,
}

impl Grid {
    fn new() -> Grid {
        let mut m = registry::create("drum.grid").unwrap();
        m.prepare(
            SR,
            4096,
            &QualityConfig {
                tier: QualityTier::Live,
            },
        );
        let params = m.info().params.iter().map(|p| p.default).collect();
        Grid { m, params }
    }

    fn set(&mut self, i: usize, v: f32) {
        self.params[i] = v;
    }

    /// Runs `edges` clock edges in one block; `reset` and `pattern` are inputs of that block.
    fn run(&mut self, edges: usize, reset: &[f32], pattern: f32) -> Vec<Vec<f32>> {
        let n = edges * STEP;
        let clock: Vec<f32> = (0..n)
            .map(|i| if i % STEP < 2 { 1.0 } else { 0.0 })
            .collect();
        let mut reset = reset.to_vec();
        reset.resize(n, 0.0);
        let inputs = [
            Signal::Buffer(&clock),
            Signal::Buffer(&reset),
            Signal::Scalar(pattern),
        ];
        let params: Vec<Signal> = self.params.iter().map(|&v| Signal::Scalar(v)).collect();
        let mut bufs = vec![vec![0.0f32; n]; 8];
        let mut outs: Vec<&mut [f32]> = bufs.iter_mut().map(|b| b.as_mut_slice()).collect();
        let mut io = ProcessIo::new(&inputs, &mut outs, &params, n);
        self.m.process(&mut io);
        bufs
    }
}

/// The level of row `row` on each edge's first sample.
fn per_step(bufs: &[Vec<f32>], row: usize) -> Vec<f32> {
    bufs[row].iter().step_by(STEP).copied().collect()
}

#[test]
fn default_beat_is_four_on_the_floor() {
    let mut g = Grid::new();
    let b = g.run(16, &[], 0.0);
    let on = |row: usize| -> Vec<usize> {
        per_step(&b, row)
            .iter()
            .enumerate()
            .filter(|(_, &v)| v > 0.5)
            .map(|(i, _)| i)
            .collect()
    };
    assert_eq!(on(0), [0, 4, 8, 12]);
    assert_eq!(on(1), [4, 12]);
    assert_eq!(on(3), [2, 6, 10, 14]);
    assert!(on(2).is_empty() && on(4).is_empty());
    // Full velocity is a gate of 1, high for the clock pulse only, low in between.
    assert_eq!(b[0][0], 1.0);
    assert_eq!(b[0][1], 1.0);
    assert_eq!(b[0][2], 0.0);
}

#[test]
fn velocity_rides_the_gate_level() {
    let mut g = Grid::new();
    let c = with_code(0.0, 0, make_code(0, 0));
    g.set(cell_index(0, 2, 0), c);
    let b = g.run(4, &[], 0.0);
    // Velocity level 0 is 0.30: gate 0.5 + 0.5 * 0.30.
    assert!((per_step(&b, 2)[0] - 0.65).abs() < 1e-6);
}

#[test]
fn chance_thins_a_row_deterministically() {
    let run = || {
        let mut g = Grid::new();
        // Row 7: every step of the first chunk at 25 % chance, length 4 so it repeats.
        let step = make_code(3, 3);
        let c = (0..4).fold(0.0, |c, k| with_code(c, k, step));
        g.set(cell_index(0, 7, 0), c);
        g.set(len_index(7), 4.0);
        per_step(&g.run(4000, &[], 0.0), 7)
    };
    let (a, b) = (run(), run());
    assert_eq!(a, b);
    let hits = a.iter().filter(|&&v| v > 0.5).count() as f32 / a.len() as f32;
    assert!((hits - 0.25).abs() < 0.04, "hit rate {hits}");
}

#[test]
fn rows_loop_at_their_own_length() {
    let mut g = Grid::new();
    // Row 6: only step 2 set, length 3, so it plays on edges 2, 5, 8.
    g.set(cell_index(0, 5, 0), with_code(0.0, 2, make_code(3, 0)));
    g.set(len_index(5), 3.0);
    let on: Vec<usize> = per_step(&g.run(10, &[], 0.0), 5)
        .iter()
        .enumerate()
        .filter(|(_, &v)| v > 0.5)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(on, [2, 5, 8]);
}

#[test]
fn reset_restarts_the_pattern() {
    let mut g = Grid::new();
    // Reset high from sample 4 edges in: the next edge plays step 1 of the loop again.
    let mut reset = vec![0.0; 6 * STEP];
    reset[2 * STEP..2 * STEP + 2].fill(1.0);
    let b = g.run(8, &reset, 0.0);
    // Edge 2 coincides with the reset and plays step 0, so the kick sounds there, and again at 6.
    let kick: Vec<usize> = per_step(&b, 0)
        .iter()
        .enumerate()
        .filter(|(_, &v)| v > 0.5)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(kick, [0, 2, 6]);
}

#[test]
fn pattern_param_and_input_pick_the_pattern() {
    let mut g = Grid::new();
    // Pattern B: one kick on step 1.
    g.set(cell_index(1, 0, 0), with_code(0.0, 1, make_code(3, 0)));
    g.set(pattern_index(), 1.0);
    let on = |b: &[Vec<f32>]| per_step(b, 0).iter().position(|&v| v > 0.5);
    assert_eq!(on(&g.run(4, &[], 0.0)), Some(1));
    // Param A plus input 1/3 is pattern B.
    let mut g2 = Grid::new();
    g2.set(cell_index(1, 0, 0), with_code(0.0, 1, make_code(3, 0)));
    assert_eq!(on(&g2.run(4, &[], 1.0 / 3.0)), Some(1));
    // Pattern C is empty.
    g.set(pattern_index(), 2.0);
    assert_eq!(on(&g.run(4, &[], 0.0)), None);
}

fn peak_of_hit(gate_level: f32) -> f32 {
    let mut k = registry::create("drum.kick").unwrap();
    k.prepare(
        SR,
        4096,
        &QualityConfig {
            tier: QualityTier::Live,
        },
    );
    let n = 4096;
    let gate: Vec<f32> = (0..n)
        .map(|i| if i < 480 { gate_level } else { 0.0 })
        .collect();
    let params: Vec<Signal> = k
        .info()
        .params
        .iter()
        .map(|p| Signal::Scalar(p.default))
        .collect();
    let inputs = [
        Signal::Buffer(&gate),
        Signal::Scalar(0.0),
        Signal::Scalar(0.0),
    ];
    let mut out = vec![0.0f32; n];
    let mut outs: Vec<&mut [f32]> = vec![out.as_mut_slice()];
    k.process(&mut ProcessIo::new(&inputs, &mut outs, &params, n));
    out.iter().fold(0.0f32, |m, v| m.max(v.abs()))
}

#[test]
fn voices_read_velocity_from_the_gate_level() {
    let full = peak_of_hit(1.0);
    let soft = peak_of_hit(0.65);
    assert!(full > 0.1);
    assert!(soft < full * 0.95, "soft {soft} full {full}");
    // A gate only just above the threshold is the softest hit, still audible.
    assert!(peak_of_hit(0.55) > 0.0);
}

#[cfg(debug_assertions)]
#[global_allocator]
static ALLOCATOR: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

#[test]
fn processing_allocates_nothing() {
    let mut g = Grid::new();
    let n = 64 * STEP;
    let clock: Vec<f32> = (0..n)
        .map(|i| if i % STEP < 2 { 1.0 } else { 0.0 })
        .collect();
    let reset = vec![0.0f32; n];
    let inputs = [
        Signal::Buffer(&clock),
        Signal::Buffer(&reset),
        Signal::Scalar(0.4),
    ];
    let params: Vec<Signal> = g.params.iter().map(|&v| Signal::Scalar(v)).collect();
    let mut bufs = vec![vec![0.0f32; n]; 8];
    let mut outs: Vec<&mut [f32]> = bufs.iter_mut().map(|b| b.as_mut_slice()).collect();
    let mut io = ProcessIo::new(&inputs, &mut outs, &params, n);
    #[cfg(debug_assertions)]
    assert_no_alloc::assert_no_alloc(|| g.m.process(&mut io));
    #[cfg(not(debug_assertions))]
    g.m.process(&mut io);
}
