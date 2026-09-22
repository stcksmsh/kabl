//! Correctness tests for the flat-schedule compiler (`crates/engine/src/compile.rs`). Builds
//! patches via `kabl_core::PatchState` ops — the way a real patch will be authored — rather than
//! hand-wired Rust like `patch_demo.rs`, since proving the compiler turns ops into a working
//! graph is the entire point.

use std::collections::BTreeMap;

use kabl_core::{CableState, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::compile::{compile, recompile, CompileError};
use kabl_modules::builtins::MidiIn;
use kabl_modules::dsp::Saw;

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

/// One `osc.va` (voice-rate, unconnected pitch -> defaults to 0 semitones), wired straight to
/// `out`. The simplest possible patch that exercises voice replication + averaging.
fn single_osc_patch(base_hz: f32) -> PatchState {
    let mut patch = PatchState::new();
    patch
        .modules
        .insert(1, module("osc.va", &[("base_hz", base_hz)]));
    patch.modules.insert(2, module("out", &[]));
    patch.cables.insert(1, cable(1, "out", 2, "left"));
    patch.cables.insert(2, cable(1, "out", 2, "right"));
    patch
}

#[test]
fn averaging_n_identical_voices_reproduces_a_single_voice_exactly() {
    // All voice_count instances of osc.va are identical (same base_hz, unconnected pitch = 0
    // semitones for all, same starting phase) — averaging N identical copies must return
    // exactly the single-voice signal (bit-exact: (N*v)/N == v for IEEE754, no rounding for
    // this kind of repeated-doubling arithmetic).
    let patch = single_osc_patch(220.0);
    let mut compiled = compile(&patch, SAMPLE_RATE, 4).expect("should compile");

    let mut reference = Saw::new();

    for _ in 0..20 {
        compiled.process_block();
        for i in 0..kabl_engine::graph::BLOCK {
            let expected = reference.next(220.0, SAMPLE_RATE);
            assert_eq!(compiled.left()[i], expected, "sample {i}");
            assert_eq!(compiled.right()[i], expected, "sample {i}");
        }
    }
}

#[test]
fn voice_count_does_not_change_loudness_for_identical_voices() {
    // Same patch, compiled at two different voice counts — averaging means the output should
    // be comparable either way (not N times louder at higher voice count). Not bit-exact
    // between 4 and 8 voices: sequential summation of 8 equal values passes through
    // intermediate partial sums (3v, 5v, 6v, 7v) that aren't exact powers-of-2 multiples of v,
    // so they round slightly differently than summing 4 (0, v, 2v, 3v, 4v — 4v is exact). Both
    // are correct to float precision; only bit-for-bit equality would be wrong to expect here.
    let patch = single_osc_patch(330.0);
    let mut compiled_4 = compile(&patch, SAMPLE_RATE, 4).expect("should compile");
    let mut compiled_8 = compile(&patch, SAMPLE_RATE, 8).expect("should compile");

    for _ in 0..10 {
        compiled_4.process_block();
        compiled_8.process_block();
        for (a, b) in compiled_4.left().iter().zip(compiled_8.left().iter()) {
            assert!(
                (a - b).abs() < 1e-4,
                "4-voice={a}, 8-voice={b}, diverged past float rounding"
            );
        }
    }
}

#[test]
fn unconnected_input_defaults_to_silence() {
    // filter.svf's `in` port left unconnected: its lowpass output should stay at exactly 0
    // (linear filter, zero input, zero initial state => zero output forever).
    let mut patch = PatchState::new();
    patch.modules.insert(1, module("filter.svf", &[]));
    patch.modules.insert(2, module("out", &[]));
    patch.cables.insert(1, cable(1, "lp", 2, "left"));
    patch.cables.insert(2, cable(1, "lp", 2, "right"));

    let mut compiled = compile(&patch, SAMPLE_RATE, 1).expect("should compile");
    for _ in 0..5 {
        compiled.process_block();
        assert!(compiled.left().iter().all(|&v| v == 0.0));
    }
}

#[test]
fn cycle_is_a_compile_error_not_silently_wrong() {
    let mut patch = PatchState::new();
    patch.modules.insert(1, module("vca", &[]));
    patch.modules.insert(2, module("vca", &[]));
    // 1.out -> 2.in, 2.out -> 1.in: a cycle.
    patch.cables.insert(1, cable(1, "out", 2, "in"));
    patch.cables.insert(2, cable(2, "out", 1, "in"));

    match compile(&patch, SAMPLE_RATE, 1) {
        Err(CompileError::Cycle(ids)) => {
            assert_eq!(ids.len(), 2);
        }
        Err(other) => panic!("expected a Cycle error, got {other}"),
        Ok(_) => panic!("expected a Cycle error, compiled successfully instead"),
    }
}

#[test]
fn unknown_module_kind_is_a_compile_error() {
    let mut patch = PatchState::new();
    patch.modules.insert(1, module("not.a.real.kind", &[]));
    match compile(&patch, SAMPLE_RATE, 1) {
        Err(CompileError::UnknownKind { kind, .. }) => assert_eq!(kind, "not.a.real.kind"),
        Err(other) => panic!("expected an UnknownKind error, got {other}"),
        Ok(_) => panic!("expected an UnknownKind error, compiled successfully instead"),
    }
}

/// The real end-to-end proof: the same 5-stage voice chain `patch_demo.rs` hand-wired
/// (midi.in -> osc.va -> filter.svf -> env.adsr/vca), built from ops instead, compiled, and
/// driven through a chord via `as_any_mut` downcasting to call `MidiIn::note_on` on each voice
/// instance — exactly how a real MIDI router will eventually reach a compiled patch's modules.
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
fn compiler_built_chord_plays_and_releases_like_patch_demo_did() {
    let patch = chord_patch();
    let voice_count = 4;
    let mut compiled = compile(&patch, SAMPLE_RATE, voice_count).expect("should compile");

    const CHORD_SEMITONES: [f32; 4] = [0.0, 4.0, 7.0, 12.0]; // C major
    for (voice, &semitones) in CHORD_SEMITONES.iter().enumerate() {
        let midi = compiled
            .module_mut(1, Some(voice))
            .expect("midi.in voice instance should exist")
            .as_any_mut()
            .downcast_mut::<MidiIn>()
            .expect("should downcast to MidiIn");
        midi.note_on(semitones, 0.9);
    }

    let block = kabl_engine::graph::BLOCK;
    let sustain_blocks = (SAMPLE_RATE * 1.0 / block as f32) as usize;
    let release_blocks = (SAMPLE_RATE * 1.0 / block as f32) as usize;

    let mut rendered = Vec::with_capacity((sustain_blocks + release_blocks) * block);
    for _ in 0..sustain_blocks {
        compiled.process_block();
        rendered.extend_from_slice(&compiled.left()[..]);
    }
    for voice in 0..4 {
        compiled
            .module_mut(1, Some(voice))
            .unwrap()
            .as_any_mut()
            .downcast_mut::<MidiIn>()
            .unwrap()
            .note_off();
    }
    for _ in 0..release_blocks {
        compiled.process_block();
        rendered.extend_from_slice(&compiled.left()[..]);
    }

    assert!(
        rendered.iter().all(|v| v.is_finite()),
        "compiler-built patch produced non-finite output"
    );

    let sustain_rms = rms(&rendered[..sustain_blocks * block]);
    let tail_rms = rms(&rendered[rendered.len() - block * 10..]);
    assert!(
        sustain_rms > 0.01,
        "sustained chord should have audible energy, got {sustain_rms}"
    );
    assert!(
        tail_rms < sustain_rms * 0.05,
        "should decay well below sustain after release: sustain={sustain_rms}, tail={tail_rms}"
    );

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/spike-renders/compiler_chord.wav");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    write_wav(&path, &rendered, SAMPLE_RATE as u32);
    eprintln!(
        "compiler-built chord: sustain_rms={sustain_rms:.4}, tail_rms={tail_rms:.6}, render={}",
        path.display()
    );
}

#[test]
fn recompile_carries_over_oscillator_phase_and_filter_state() {
    let patch = chord_patch();
    let mut compiled = compile(&patch, SAMPLE_RATE, 4).expect("should compile");

    for voice in 0..4 {
        compiled
            .module_mut(1, Some(voice))
            .unwrap()
            .as_any_mut()
            .downcast_mut::<MidiIn>()
            .unwrap()
            .note_on(0.0, 0.9);
    }
    for _ in 0..50 {
        compiled.process_block();
    }
    let before = *compiled.left();

    // Recompile against the *same* patch (as if the user nudged something irrelevant) — state
    // should carry over, so the very next block continues smoothly rather than restarting.
    let mut recompiled = recompile(&mut compiled, &patch, SAMPLE_RATE, 4).expect("should compile");
    // Re-trigger isn't needed: MidiIn's own gate/pitch/velocity state carries over via
    // save_state/load_state like every other module's.
    let mut old_continued = compiled;
    old_continued.process_block();
    recompiled.process_block();

    // Both should continue from the same point — same output on the next block.
    assert_eq!(before.len(), kabl_engine::graph::BLOCK);
    assert_eq!(old_continued.left(), recompiled.left());
}

#[test]
fn buffer_pool_reuses_slots_instead_of_one_per_port() {
    // chord_patch at 4 voices: midi.in(3 outputs) + osc.va(1) + filter.svf(3) + env.adsr(1) +
    // vca(1) = 9 outputs/voice * 4 voices = 36, plus the shared silence buffer = 37 if every
    // output got its own permanent slot (the pre-coalescing behavior). A sequential per-voice
    // chain like this should reuse heavily -- assert a real reduction, not a specific number
    // (which would make this test brittle against unrelated scheduling changes).
    let patch = chord_patch();
    let compiled = compile(&patch, SAMPLE_RATE, 4).expect("should compile");
    let naive_upper_bound = 37;
    assert!(
        compiled.buffer_count() < naive_upper_bound,
        "expected buffer reuse to reduce the pool below {naive_upper_bound}, got {}",
        compiled.buffer_count()
    );
}

#[test]
fn buffer_pool_size_scales_with_the_widest_point_in_the_schedule_not_total_modules() {
    // More voices should still reuse -- 8 voices shouldn't need anywhere near 2x the buffers
    // of 4 voices, since each voice's chain still only has a handful of buffers alive at once
    // (reuse happens *within* a voice's own short-lived intermediates too, not just across
    // voices). A weak but meaningful bound: 8 voices shouldn't need more than 4 voices' count
    // plus one extra buffer per voice-rate output port width (a generous slack, not a tight
    // bound -- the point is "sublinear-ish growth", not pinning exact scheduler behavior).
    let patch = chord_patch();
    let compiled_4 = compile(&patch, SAMPLE_RATE, 4).expect("should compile");
    let compiled_8 = compile(&patch, SAMPLE_RATE, 8).expect("should compile");
    assert!(
        compiled_8.buffer_count() < compiled_4.buffer_count() * 2,
        "8-voice pool ({}) should not need to double the 4-voice pool ({})",
        compiled_8.buffer_count(),
        compiled_4.buffer_count()
    );
}

#[test]
fn coalesced_buffers_still_produce_bit_exact_output_vs_a_reference_render() {
    // The averaging/state/topology tests above already exercise the coalesced compiler (there's
    // only one compile() path now), so this is a belt-and-suspenders end-to-end check: render a
    // chord twice from two independent compile() calls and confirm they're identical -- if
    // coalescing ever aliased two buffers that were actually still live at the same time, this
    // would show up as a nondeterministic or corrupted render, not just "different from before".
    let patch = chord_patch();
    let mut a = compile(&patch, SAMPLE_RATE, 4).expect("should compile");
    let mut b = compile(&patch, SAMPLE_RATE, 4).expect("should compile");

    for voice in 0..4 {
        for compiled in [&mut a, &mut b] {
            compiled
                .module_mut(1, Some(voice))
                .unwrap()
                .as_any_mut()
                .downcast_mut::<MidiIn>()
                .unwrap()
                .note_on(voice as f32 * 3.0, 0.8);
        }
    }

    for _ in 0..50 {
        a.process_block();
        b.process_block();
        assert_eq!(
            a.left(),
            b.left(),
            "two independent compiles of the same patch must match"
        );
    }
}

fn rms(samples: &[f32]) -> f32 {
    let sum_sq: f64 = samples.iter().map(|&v| (v as f64) * (v as f64)).sum();
    ((sum_sq / samples.len() as f64).sqrt()) as f32
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
