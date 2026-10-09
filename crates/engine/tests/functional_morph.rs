//! Morph on the real demo piece: moving each cable's morph through the runtime path changes
//! the sound, in place and without a rebuild.

use std::path::Path;

use kabl_cables::MORPH;
use kabl_engine::compile::compile;
use kabl_engine::runtime::RuntimeTarget;

fn render(morph: [f32; 3]) -> (Vec<f32>, usize) {
    let dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/functional-cables/morph-arc");
    let state = kabl_core::load(&dir).unwrap().state().clone();
    let cables: Vec<u64> = state
        .cables
        .iter()
        .filter(|(_, c)| c.params.contains_key("morph"))
        .map(|(&id, _)| id)
        .collect();
    assert_eq!(cables.len(), 3);
    let mut c = compile(&state, 48000.0, 4).unwrap();
    let steps = c.profile_counts().2;
    for (&cable, &m) in cables.iter().zip(&morph) {
        let slot = MORPH as u8;
        assert!(c.set_runtime(RuntimeTarget::Cable { cable, slot }, m, false));
    }
    let mut out = Vec::new();
    for _ in 0..(48000 * 10 / 64) {
        c.process_block();
        out.extend_from_slice(c.left());
    }
    assert_eq!(c.profile_counts().2, steps, "no rebuild");
    (out, steps)
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|s| s * s).sum::<f32>() / x.len() as f32).sqrt()
}

#[test]
fn each_morph_changes_the_sound() {
    let (base, _) = render([0.0, 0.0, 0.0]);
    assert!(rms(&base) > 0.01);
    for which in 0..3 {
        let mut m = [0.0; 3];
        m[which] = 1.0;
        let (moved, _) = render(m);
        let diff: Vec<f32> = base.iter().zip(&moved).map(|(a, b)| a - b).collect();
        println!(
            "morph {which}: rms of difference {:.4} (signal {:.4})",
            rms(&diff),
            rms(&base)
        );
        assert!(
            rms(&diff) > 0.005 * rms(&base).max(0.01),
            "morph {which} is audible"
        );
    }
}
