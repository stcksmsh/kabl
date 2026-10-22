//! A patch with an embedded table sounds from the first block after it is loaded: nothing waits
//! on a table that is still being prepared.

use basedrop::Collector;
use kabl_core::PatchState;
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::PatchEngine;

const SR: f32 = 48000.0;

fn demo() -> PatchState {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../patches/sound-engines/demos/imported-morph");
    let mut st = kabl_core::load(&dir).unwrap().state().clone();
    assert!(!st.tables.is_empty());
    // The envelope opens in 50 ms so the first block of the note is already audible.
    for m in st.modules.values_mut().filter(|m| m.kind == "env.adsr") {
        m.params.insert("attack_ms".into(), 50.0);
    }
    st
}

fn first_second(e: &mut PatchEngine) -> Vec<f32> {
    e.key(KeyEvent::On {
        note: 48,
        velocity: 100,
    });
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    let mut out = Vec::new();
    for _ in 0..(SR as usize / BLOCK) {
        e.process_block(&mut l, &mut r);
        out.extend_from_slice(&l);
    }
    out
}

fn rms(v: &[f32]) -> f32 {
    (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt()
}

#[test]
fn the_first_note_after_load_sounds_at_once_with_the_embedded_table() {
    let c = Collector::new();
    let mut e = PatchEngine::new(&c.handle(), &demo(), SR, 8).unwrap();
    e.reset();
    let out = first_second(&mut e);
    // 100 ms after the note: audible. (An envelope still at zero, or a silent table, gives 0.)
    let early = rms(&out[4800..9600]);
    assert!(early > 0.005, "silent in the first 100-200 ms: rms {early}");
    // And it is the embedded table, not a factory one: dropping the table changes the sound.
    let mut without = demo();
    without.tables.clear();
    let c2 = Collector::new();
    let mut e2 = PatchEngine::new(&c2.handle(), &without, SR, 8).unwrap();
    e2.reset();
    let other = first_second(&mut e2);
    let diff = out
        .iter()
        .zip(&other)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max);
    assert!(diff > 0.01, "the embedded table is not what plays: {diff}");
}
