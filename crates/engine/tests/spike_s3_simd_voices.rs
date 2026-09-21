//! Spike S3 (brief section 11): correctness check before trusting the SIMD benchmark. Batching
//! 4 voices into one `f32x4` lane should be the *same* computation as 4 scalar instances, not an
//! approximation — unlike S2's control-rate tier (which brief section 7 explicitly allows to
//! trade fidelity), SIMD batching is a pure implementation-strategy change. So the bar here is
//! much tighter: near machine epsilon, not "close enough."

use kabl_engine::simd_voices::{ScalarVoices, SimdVoices};

const SAMPLE_RATE: f32 = 48000.0;

#[test]
fn simd_matches_scalar() {
    let mut scalar = ScalarVoices::new(SAMPLE_RATE);
    let mut simd = SimdVoices::new(SAMPLE_RATE);

    let mut max_diff = 0.0f32;
    for _ in 0..48000 {
        // 1 second
        let s = scalar.next(SAMPLE_RATE);
        let v = simd.next(SAMPLE_RATE).to_array();
        for i in 0..4 {
            max_diff = max_diff.max((s[i] - v[i]).abs());
        }
    }

    eprintln!(
        "S3 correctness: max |scalar - simd| over 48000 samples = {:.3e}",
        max_diff
    );
    assert!(
        max_diff < 1e-5,
        "SIMD batching should reproduce the scalar path to within float rounding, not \
         approximate it; got max diff {:.3e}",
        max_diff
    );
}
