//! `VoiceAllocator` correctness: assignment order, voice stealing, note-off resolution, and re-
//! trigger behavior — pure logic tests (no `CompiledPatch` needed, see the module's own doc
//! comment for why it's decoupled) — plus one integration test proving it drives a real compiled
//! patch's `midi.in` instances correctly end-to-end.

use std::collections::BTreeMap;

use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::compile::compile;
use kabl_engine::voice_allocator::VoiceAllocator;
use kabl_modules::builtins::MidiIn;
use kabl_modules::Module;

#[test]
fn fresh_notes_fill_voices_lowest_index_first() {
    let mut alloc = VoiceAllocator::new(4);
    assert_eq!(alloc.note_on(100), 0);
    assert_eq!(alloc.note_on(101), 1);
    assert_eq!(alloc.note_on(102), 2);
    assert_eq!(alloc.note_on(103), 3);
}

#[test]
fn freed_voice_is_reused_before_stealing() {
    let mut alloc = VoiceAllocator::new(2);
    assert_eq!(alloc.note_on(1), 0);
    assert_eq!(alloc.note_on(2), 1);
    assert_eq!(alloc.note_off(1), Some(0));
    // Voice 0 is free again -- a third note should land there, not steal voice 1.
    assert_eq!(alloc.note_on(3), 0);
}

#[test]
fn all_voices_busy_steals_the_oldest() {
    let mut alloc = VoiceAllocator::new(3);
    alloc.note_on(1); // voice 0, oldest
    alloc.note_on(2); // voice 1
    alloc.note_on(3); // voice 2
                      // All busy: note 4 should steal voice 0 (note 1's), the oldest.
    assert_eq!(alloc.note_on(4), 0);
    // Note 1 no longer has a voice -- its own note_off resolves to nothing.
    assert_eq!(alloc.note_off(1), None);
    // Note 4 now owns voice 0.
    assert_eq!(alloc.voice_for(4), Some(0));
}

#[test]
fn stealing_continues_in_oldest_first_order() {
    let mut alloc = VoiceAllocator::new(2);
    alloc.note_on(1); // voice 0
    alloc.note_on(2); // voice 1
    assert_eq!(alloc.note_on(3), 0); // steals note 1's voice (oldest)
    assert_eq!(alloc.note_on(4), 1); // steals note 2's voice (now oldest)
    assert_eq!(alloc.voice_for(3), Some(0));
    assert_eq!(alloc.voice_for(4), Some(1));
    assert_eq!(alloc.voice_for(1), None);
    assert_eq!(alloc.voice_for(2), None);
}

#[test]
fn note_off_on_a_stolen_note_does_not_disturb_the_voice_that_stole_it() {
    let mut alloc = VoiceAllocator::new(1);
    alloc.note_on(1); // voice 0
    alloc.note_on(2); // steals voice 0 from note 1
    assert_eq!(alloc.note_off(1), None, "note 1 was already stolen");
    // Voice 0 should still belong to note 2 -- a caller calling note_off(1) must not have
    // affected it.
    assert_eq!(alloc.voice_for(2), Some(0));
}

#[test]
fn note_off_on_an_unknown_note_is_a_no_op() {
    let mut alloc = VoiceAllocator::new(2);
    assert_eq!(alloc.note_off(999), None);
}

#[test]
fn repeated_note_on_for_the_same_note_reuses_its_voice() {
    let mut alloc = VoiceAllocator::new(2);
    assert_eq!(alloc.note_on(1), 0);
    assert_eq!(alloc.note_on(2), 1);
    // A duplicate note_on for note 1 (e.g. a retriggered key) should NOT steal a second voice --
    // it reuses voice 0.
    assert_eq!(alloc.note_on(1), 0);
    assert_eq!(alloc.voice_count(), 2);
}

#[test]
fn repeated_note_on_refreshes_age_so_it_is_not_stolen_next() {
    let mut alloc = VoiceAllocator::new(2);
    alloc.note_on(1); // voice 0, age 0
    alloc.note_on(2); // voice 1, age 1
    alloc.note_on(1); // re-trigger note 1 -- age refreshed, now newer than note 2
                      // Note 2 is now the oldest -- a third distinct note should steal *its* voice, not note 1's.
    assert_eq!(alloc.note_on(3), 1);
    assert_eq!(
        alloc.voice_for(1),
        Some(0),
        "note 1's voice should be untouched"
    );
}

// --- integration: drives a real CompiledPatch's midi.in instances ---

fn module(kind: &str, params: &[(&str, f32)]) -> ModuleState {
    ModuleState {
        kind: kind.to_string(),
        pos: Vec2 { x: 0.0, y: 0.0 },
        params: params.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
    }
}

fn cable(from_id: u64, from_port: &str, to_id: u64, to_port: &str) -> CableState {
    CableState {
        from: PortRef::Module {
            id: from_id,
            port: from_port.to_string(),
        },
        to: PortRef::Module {
            id: to_id,
            port: to_port.to_string(),
        },
        params: BTreeMap::new(),
        steps: Vec::new(),
    }
}

/// Minimal single-stage voice chain: midi.in -> osc.va -> out. Enough to prove the allocator
/// correctly targets a specific voice instance's `midi.in`, without the full 5-stage chord
/// chain's unrelated detail.
fn minimal_patch() -> PatchState {
    let mut patch = PatchState::new();
    patch.modules.insert(1, module("midi.in", &[]));
    patch
        .modules
        .insert(2, module("osc.va", &[("base_hz", 261.63)]));
    patch.modules.insert(3, module("out", &[]));
    patch.cables.insert(1, cable(1, "pitch", 2, "pitch"));
    patch.cables.insert(2, cable(2, "out", 3, "left"));
    patch.cables.insert(3, cable(2, "out", 3, "right"));
    patch
}

fn semitones_for_voice(compiled: &mut kabl_engine::compile::CompiledPatch, voice: usize) -> f32 {
    let midi = compiled
        .module_mut(1, Some(voice))
        .unwrap()
        .as_any_mut()
        .downcast_mut::<MidiIn>()
        .unwrap();
    let mut state = std::collections::HashMap::new();
    struct Probe<'a>(&'a mut std::collections::HashMap<String, f32>);
    impl kabl_modules::StateWriter for Probe<'_> {
        fn write_f32(&mut self, key: &str, value: f32) {
            self.0.insert(key.to_string(), value);
        }
    }
    midi.save_state(&mut Probe(&mut state));
    state["pitch"]
}

#[test]
fn allocator_routes_note_on_to_the_correct_compiled_voice() {
    let patch = minimal_patch();
    let voice_count = 3;
    let mut compiled = compile(&patch, 48000.0, voice_count).expect("should compile");
    let mut alloc = VoiceAllocator::new(voice_count);

    // Three distinct MIDI notes, each carrying a distinct pitch -- the allocator's returned
    // voice index must be the one that actually gets that pitch.
    for (note_id, semitones) in [(60u32, 0.0f32), (64, 4.0), (67, 7.0)] {
        let voice = alloc.note_on(note_id);
        compiled
            .module_mut(1, Some(voice))
            .unwrap()
            .as_any_mut()
            .downcast_mut::<MidiIn>()
            .unwrap()
            .note_on(semitones, 0.9);
    }

    assert_eq!(semitones_for_voice(&mut compiled, 0), 0.0);
    assert_eq!(semitones_for_voice(&mut compiled, 1), 4.0);
    assert_eq!(semitones_for_voice(&mut compiled, 2), 7.0);

    // Release the middle note (voice 1) explicitly, so a subsequent 4th note reuses that freed
    // voice instead of stealing.
    let freed = alloc.note_off(64);
    assert_eq!(freed, Some(1));
    compiled
        .module_mut(1, Some(1))
        .unwrap()
        .as_any_mut()
        .downcast_mut::<MidiIn>()
        .unwrap()
        .note_off();

    let voice = alloc.note_on(72);
    assert_eq!(voice, 1, "freed voice should be reused before stealing");
    compiled
        .module_mut(1, Some(voice))
        .unwrap()
        .as_any_mut()
        .downcast_mut::<MidiIn>()
        .unwrap()
        .note_on(12.0, 0.9);

    assert_eq!(
        semitones_for_voice(&mut compiled, 0),
        0.0,
        "voice 0 untouched"
    );
    assert_eq!(semitones_for_voice(&mut compiled, 1), 12.0);
    assert_eq!(
        semitones_for_voice(&mut compiled, 2),
        7.0,
        "voice 2 untouched"
    );
}
