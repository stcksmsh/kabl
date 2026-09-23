//! Schema-v1 patches keep their exact sound. `../core/tests/fixtures/*.render.f32` were rendered
//! by the baseline build (commit e6db377) from the same files: 8 voices, notes 0/4/7 semitones
//! on voices 0..2 at velocity 0.8 from block 0, released at block 250, 400 blocks, interleaved
//! stereo f32le. The current engine must reproduce them bit for bit.

use std::path::Path;

use kabl_engine::compile::compile;

fn render(name: &str) -> Vec<f32> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../core/tests/fixtures");
    let log = kabl_core::load(&dir.join(name)).expect("v1 loads");
    let mut c = compile(log.state(), 48000.0, 8).expect("v1 compiles");
    let mut out = Vec::new();
    for b in 0..400 {
        if b == 0 {
            for (v, n) in [(0usize, 0.0f32), (1, 4.0), (2, 7.0)] {
                c.note_on(v, n, 0.8);
            }
        }
        if b == 250 {
            for v in 0..3 {
                c.note_off(v);
            }
        }
        c.process_block();
        for (l, r) in c.left().iter().zip(c.right().iter()) {
            out.push(*l);
            out.push(*r);
        }
    }
    out
}

fn baseline(name: &str) -> Vec<f32> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../core/tests/fixtures")
        .join(format!("{name}.render.f32"));
    std::fs::read(path)
        .unwrap()
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

#[test]
fn v1_patches_render_bit_exact_to_the_baseline() {
    for name in ["v1_default", "v1_edited"] {
        let (now, then) = (render(name), baseline(name));
        assert_eq!(now.len(), then.len());
        let peak = then.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(
            peak > 0.05,
            "{name}: baseline render is not silent ({peak})"
        );
        let first_diff = now
            .iter()
            .zip(&then)
            .position(|(a, b)| a.to_bits() != b.to_bits());
        assert_eq!(first_diff, None, "{name}: differs from baseline");
    }
}
