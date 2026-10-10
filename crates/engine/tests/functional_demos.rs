//! The functional-cable demo patches (`patches/functional-cables`) load, play by themselves
//! and sound different from the same patch with plain cables.

use std::path::Path;

use kabl_engine::compile::compile;

fn render(state: &kabl_core::PatchState) -> (Vec<f32>, usize) {
    let mut c = compile(state, 48000.0, 4).unwrap();
    let mut out = Vec::new();
    for _ in 0..(48000 * 6 / 64) {
        c.process_block();
        out.extend_from_slice(c.left());
    }
    (out, c.profile_counts().2)
}

#[test]
fn demo_patches_play_and_depend_on_their_cables() {
    for name in ["echo-throws", "five-against-eight"] {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../patches/functional-cables")
            .join(name);
        let state = kabl_core::load(&dir).unwrap().state().clone();
        let mut plain = state.clone();
        for c in plain.cables.values_mut() {
            c.params.retain(|k, _| k == "amount" || k == "bypass");
        }
        let (a, steps_a) = render(&state);
        let (b, steps_b) = render(&plain);
        assert!(steps_a > steps_b, "{name}: cable nodes are in the graph");
        assert!(a.iter().any(|x| x.abs() > 0.05), "{name} is audible");
        assert!(a.iter().all(|x| x.is_finite()));
        assert_ne!(a, b, "{name} sounds different with plain cables");
        assert_eq!(a, render(&state).0, "{name} renders the same every time");
    }
}
