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

/// A voice's amp and filter envelopes are one voice: four held notes never read above four.
#[test]
fn two_envelopes_per_voice_count_once() {
    use kabl_engine::keyboard::KeyEvent;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/presets");
    let mut dirs: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .flatten()
        .map(|d| d.path())
        .collect();
    dirs.sort();
    let mut checked = 0;
    for dir in dirs {
        let log = kabl_core::load(&dir).expect("patch");
        let envs = log
            .state()
            .modules
            .values()
            .filter(|m| m.kind == "env.adsr")
            .count();
        let keyed = log.state().modules.values().any(|m| m.kind == "midi.in");
        if envs < 2 || !keyed {
            continue;
        }
        let collector = Collector::new();
        let mut e =
            PatchEngine::new(&collector.handle(), log.state(), 48000.0, 8).expect("compiles");
        for n in [48, 55, 60, 64] {
            e.key(KeyEvent::On {
                note: n,
                velocity: 100,
            });
        }
        let (mut l, mut r) = ([0.0; BLOCK], [0.0; BLOCK]);
        let (mut peak, mut keyed_voices) = (0, 0);
        for _ in 0..200 {
            e.process_block(&mut l, &mut r);
            peak = peak.max(e.active_envelopes());
            keyed_voices = keyed_voices.max(e.keys().1);
        }
        assert!(
            (1..=keyed_voices.max(1)).contains(&peak),
            "{}: {peak} voices where the keyboard sounds {keyed_voices}",
            dir.display()
        );
        checked += 1;
    }
    assert!(checked > 0, "no keyed factory sound with two envelopes");
}
