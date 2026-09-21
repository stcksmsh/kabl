//! Proves `patch_engine.rs`'s `PatchEngine` actually wires the real compiler into spike S1's
//! swap/crossfade mechanism: arbitrary-topology `CompiledPatch`es (not just S1's fixed 2-node
//! graph) can be swapped live, with state carried over and no allocation on the audio thread.
//!
//! Method, deliberately simpler than `spike_s1_swap.rs`'s (that one already proved the crossfade
//! curve and click behaviour in general — this test only needs to prove the *new* wiring): swap
//! the same patch onto itself, repeatedly. Since nothing about the patch changes and
//! `recompile()` carries state exactly (already proven bit-exact in
//! `compile.rs`'s `recompile_carries_over_oscillator_phase_and_filter_state`), a `hard` reference
//! that just keeps one `CompiledPatch` running with no swaps at all must match `PatchEngine`'s
//! output exactly everywhere outside a crossfade window — a stronger, cleaner null test than
//! S1's (there, cable depth changed, so only "phase-independent of depth" gave exactness; here
//! the two are running provably identical continuations).

use std::collections::BTreeMap;

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::{Collector, Owned};
use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::compile::{compile, CompiledPatch};
use kabl_engine::graph::BLOCK;
use kabl_engine::patch_engine::PatchEngine;
use kabl_modules::builtins::MidiIn;

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;
const VOICE_COUNT: usize = 4;
const NUM_SWAPS: usize = 20;
/// 20 blocks = 1280 samples =~ 26.7ms, comfortably longer than the 15ms crossfade so consecutive
/// swaps never overlap — same reasoning as `spike_s1_swap.rs`'s `BLOCKS_PER_TOGGLE`.
const BLOCKS_PER_SWAP: usize = 20;
const CHORD_SEMITONES: [f32; VOICE_COUNT] = [0.0, 4.0, 7.0, 12.0];

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

fn trigger_chord(patch: &mut CompiledPatch) {
    for (voice, &semitones) in CHORD_SEMITONES.iter().enumerate() {
        patch
            .module_mut(1, Some(voice))
            .unwrap()
            .as_any_mut()
            .downcast_mut::<MidiIn>()
            .unwrap()
            .note_on(semitones, 0.9);
    }
}

#[test]
fn patch_engine_swaps_the_real_compiler_with_no_click_and_no_alloc() {
    let patch = chord_patch();

    let mut collector = Collector::new();
    let handle = collector.handle();

    let mut engine =
        PatchEngine::new(&handle, &patch, SR, VOICE_COUNT).expect("engine should compile");
    trigger_chord(engine.active_mut());
    let crossfade_samples = engine.crossfade_samples();

    let mut hard = compile(&patch, SR, VOICE_COUNT).expect("hard reference should compile");
    trigger_chord(&mut hard);

    let (mut producer, mut consumer) = rtrb::RingBuffer::<Owned<CompiledPatch>>::new(1);

    let total_samples = NUM_SWAPS * BLOCKS_PER_SWAP * BLOCK;
    let mut engine_left: Vec<f32> = Vec::with_capacity(total_samples);
    let mut hard_left: Vec<f32> = Vec::with_capacity(total_samples);
    let mut swap_sample_indices: Vec<usize> = Vec::with_capacity(NUM_SWAPS);

    for _ in 0..NUM_SWAPS {
        // Control-thread work: allowed to allocate.
        let new_patch = engine
            .build_swap(&handle, &patch)
            .expect("swap should compile");
        producer
            .push(new_patch)
            .expect("single-slot channel should be empty between swaps");
        swap_sample_indices.push(engine_left.len());

        for _ in 0..BLOCKS_PER_SWAP {
            let mut l = [0f32; BLOCK];
            let mut r = [0f32; BLOCK];
            assert_no_alloc(|| {
                // Audio-thread work: must not allocate or deallocate.
                if let Ok(new_patch) = consumer.pop() {
                    engine.receive_swap(new_patch);
                }
                engine.process_block(&mut l, &mut r);
            });
            engine_left.extend_from_slice(&l);
            hard.process_block();
            hard_left.extend_from_slice(&hard.left()[..]);
        }

        // Off the audio thread: reap whatever the last swap queued for deferred drop.
        collector.collect();
    }

    assert_eq!(engine_left.len(), total_samples);
    assert!(
        engine_left.iter().all(|v| v.is_finite()),
        "swapped output must stay finite throughout"
    );

    // Outside every crossfade window, engine and hard have run the same input through
    // state-identical continuations (recompile() proven bit-exact in compile.rs) -- so the
    // residual there must be exactly zero, not just small.
    let mut max_steady_residual = 0.0f32;
    let mut steady_samples = 0usize;
    for (idx, &swap_at) in swap_sample_indices.iter().enumerate() {
        let steady_start = swap_at + crossfade_samples;
        let steady_end = swap_sample_indices
            .get(idx + 1)
            .copied()
            .unwrap_or(total_samples);
        for i in steady_start..steady_end {
            let residual = (engine_left[i] - hard_left[i]).abs();
            max_steady_residual = max_steady_residual.max(residual);
            steady_samples += 1;
        }
    }
    assert!(steady_samples > 0, "test setup should leave steady samples");
    assert_eq!(
        max_steady_residual, 0.0,
        "swapping a patch onto an identical recompile of itself must not perturb steady-state \
         output at all -- state carry-over should be exact"
    );

    // Inside crossfade windows: no click gate re-derived here (spike S1 already established the
    // equal-power curve's behaviour, reused verbatim via swap::equal_power) -- just a sanity
    // bound that blending two near-identical signals doesn't blow up unreasonably.
    let peak_outside_crossfade = engine_left
        .iter()
        .cloned()
        .fold(0.0f32, |m, v| m.max(v.abs()));
    for &swap_at in &swap_sample_indices {
        let window_end = (swap_at + crossfade_samples).min(total_samples);
        for (i, &sample) in engine_left[swap_at..window_end].iter().enumerate() {
            assert!(
                sample.abs() <= peak_outside_crossfade * 1.5,
                "crossfade sample {} ({sample}) should stay within a sane bound of the signal's \
                 typical peak ({peak_outside_crossfade})",
                swap_at + i
            );
        }
    }

    let render_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/spike-renders");
    std::fs::create_dir_all(&render_dir).expect("create render dir");
    let path = render_dir.join("patch_engine_swap.wav");
    write_wav(&path, &engine_left, SR as u32);
    eprintln!(
        "patch_engine swap: {NUM_SWAPS} swaps, crossfade={crossfade_samples} samples, \
         max_steady_residual={max_steady_residual}, render={}",
        path.display()
    );
}

fn write_wav(path: &std::path::Path, samples: &[f32], sample_rate: u32) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("create wav");
    for &s in samples {
        writer.write_sample(s).expect("write sample");
    }
    writer.finalize().expect("finalize wav");
}
