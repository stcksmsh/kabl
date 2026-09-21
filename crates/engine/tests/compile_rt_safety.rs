//! Proves `CompiledPatch::process_block` allocates nothing (brief section 3), the same way
//! spike S1 (`tests/spike_s1_swap.rs`) proves it for the swap mechanism: run the real audio-
//! thread call inside `assert_no_alloc!` with a global allocator that panics on any alloc/dealloc
//! while it's active.
//!
//! Uses `compile.rs`'s own real 5-stage voice chain (`chord_patch`, duplicated here rather than
//! shared — it's five lines of ops, not worth a shared test-only crate) at a nontrivial voice
//! count so every code path in `process_block` (voice-rate instancing, `SumVoices` averaging,
//! multi-output `filter.svf`) actually runs inside the no-alloc guard, not just a trivial patch.

use std::collections::BTreeMap;

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::compile::compile;
use kabl_modules::builtins::MidiIn;

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SAMPLE_RATE: f32 = 48000.0;

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

fn chord_patch() -> PatchState {
    let mut patch = PatchState::new();
    patch.modules.insert(1, module("midi.in", &[]));
    patch
        .modules
        .insert(2, module("osc.va", &[("base_hz", 261.63)]));
    patch.modules.insert(
        3,
        module("filter.svf", &[("cutoff_hz", 2000.0), ("resonance", 0.3)]),
    );
    patch.modules.insert(
        4,
        module(
            "env.adsr",
            &[
                ("attack_ms", 10.0),
                ("decay_ms", 80.0),
                ("sustain", 0.7),
                ("release_ms", 300.0),
            ],
        ),
    );
    patch
        .modules
        .insert(5, module("vca", &[("gain", 0.0), ("exponential", 0.0)]));
    patch.modules.insert(6, module("out", &[]));

    patch.cables.insert(1, cable(1, "pitch", 2, "pitch"));
    patch.cables.insert(2, cable(2, "out", 3, "in"));
    patch.cables.insert(3, cable(1, "gate", 4, "gate"));
    patch.cables.insert(4, cable(3, "lp", 5, "in"));
    patch.cables.insert(5, cable(4, "out", 5, "cv"));
    patch.cables.insert(6, cable(5, "out", 6, "left"));
    patch.cables.insert(7, cable(5, "out", 6, "right"));
    patch
}

#[test]
fn process_block_does_not_allocate() {
    let patch = chord_patch();
    let voice_count = 4;
    // Compiling and triggering notes both allocate freely (control thread) — outside the guard.
    let mut compiled = compile(&patch, SAMPLE_RATE, voice_count).expect("should compile");

    const CHORD_SEMITONES: [f32; 4] = [0.0, 4.0, 7.0, 12.0];
    for (voice, &semitones) in CHORD_SEMITONES.iter().enumerate() {
        compiled
            .module_mut(1, Some(voice))
            .unwrap()
            .as_any_mut()
            .downcast_mut::<MidiIn>()
            .unwrap()
            .note_on(semitones, 0.9);
    }

    // The actual audio-thread call: 200 blocks (~267ms), spanning attack, sustain, and well into
    // the SumVoices/filter/env steady state — the guard must hold for every one of them.
    for _ in 0..200 {
        assert_no_alloc(|| {
            compiled.process_block();
        });
    }

    // Sanity: still producing real, finite audio after 200 no-alloc blocks, not silently broken.
    assert!(compiled.left().iter().all(|v| v.is_finite()));
    assert!(compiled.left().iter().any(|&v| v != 0.0));
}
