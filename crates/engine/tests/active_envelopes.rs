//! The status bar's voice count: envelopes that are not idle, driven by a sequencer as well
//! as by keys.
use basedrop::Collector;
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::PatchEngine;

#[test]
fn a_sequencer_patch_has_sounding_envelopes_and_a_silent_one_has_none() {
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/presets/acid-line");
    let log = kabl_core::load(&dir).expect("patch");
    let collector = Collector::new();
    let mut e = PatchEngine::new(&collector.handle(), log.state(), 48000.0, 8).expect("compiles");
    assert_eq!(e.active_envelopes(), 0, "nothing has played yet");
    let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
    let mut peak = 0;
    for _ in 0..(3.0 * 48000.0 / BLOCK as f32) as usize {
        e.process_block(&mut l, &mut r);
        peak = peak.max(e.active_envelopes());
    }
    assert!(peak >= 1, "the sequencer opened an envelope");
}
