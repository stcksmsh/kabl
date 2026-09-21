//! Spike S2 (brief section 11): correctness check for the control-rate optimization before
//! trusting its benchmark numbers. `process_naive` and `process_optimized` should converge to
//! (near-)identical audio once the ADSR has settled and the LFO's block-held value is a
//! reasonable stand-in for its per-sample trajectory — the optimization is only valid if it
//! doesn't change what the patch sounds like, just how it's computed (brief section 4.1, "Patch
//! = intent; quality = render setting" — the same idea one level down, at the compiler's
//! signal-classification tier rather than the quality-tier table).

use kabl_engine::graph::BLOCK;
use kabl_engine::potato::PotatoPatch;

const SAMPLE_RATE: f32 = 48000.0;

#[test]
fn optimized_converges_to_naive_once_settled() {
    let mut naive = PotatoPatch::new(SAMPLE_RATE);
    let mut optimized = PotatoPatch::new(SAMPLE_RATE);

    // Run past the ADSR's attack (settles within ~10 time constants, well under 100ms) so both
    // paths are in the steady state the optimization actually targets.
    let settle_blocks = (SAMPLE_RATE * 0.3 / BLOCK as f32) as usize;
    for _ in 0..settle_blocks {
        let mut buf = [0f32; BLOCK];
        naive.process_naive(&mut buf);
        optimized.process_optimized(&mut buf);
    }

    let mut max_diff = 0.0f32;
    let mut sum_sq_diff = 0.0f64;
    let mut sum_sq_signal = 0.0f64;
    let compare_blocks = 200;
    for _ in 0..compare_blocks {
        let mut n_buf = [0f32; BLOCK];
        let mut o_buf = [0f32; BLOCK];
        naive.process_naive(&mut n_buf);
        optimized.process_optimized(&mut o_buf);
        for (n, o) in n_buf.iter().zip(o_buf.iter()) {
            let diff = (n - o).abs();
            max_diff = max_diff.max(diff);
            sum_sq_diff += (diff as f64) * (diff as f64);
            sum_sq_signal += (*n as f64) * (*n as f64);
        }
    }

    let rms_diff = (sum_sq_diff / (compare_blocks * BLOCK) as f64).sqrt();
    let rms_signal = (sum_sq_signal / (compare_blocks * BLOCK) as f64).sqrt();
    let relative_db = 20.0 * (rms_diff / rms_signal).log10();

    eprintln!(
        "S2 correctness: max_diff={:.3e}, relative error={:.1} dB (naive vs. optimized, once settled)",
        max_diff, relative_db
    );

    // The block-held LFO is a coarser time resolution (one value per 64-sample block instead of
    // per-sample), so some divergence versus the per-sample naive path is expected and is
    // exactly the Live-tier approximation the control-rate tier is allowed to make (brief
    // section 7: quality tiers "change fidelity ... never character"). This bounds it to a
    // small fraction of the signal, not a different-sounding patch.
    assert!(
        relative_db < -30.0,
        "optimized path should stay close to naive once settled (Live-tier approximation, not \
         a different patch); got {:.1} dB relative error",
        relative_db
    );
}
