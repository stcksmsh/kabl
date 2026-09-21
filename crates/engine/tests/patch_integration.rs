//! First real patch assembled from `Module` trait objects (brief section 8), not hand-rolled DSP
//! calls like S1/S2/S3. This is the thing those three spikes and the 9 built-in modules were
//! all building toward: proving the `Module`/`ProcessIo` design actually composes into something
//! that plays, before writing the general flat-schedule compiler (brief section 7) that will
//! wire patches like this automatically instead of the manual glue code in `src/patch_demo.rs`.
//!
//! Topology, 4 voices: `midi.in` -> `osc.va` (pitch-driven) -> `filter.svf` (lowpass tap) ->
//! `vca` (gain from `env.adsr`, itself gated by `midi.in`) -> `mixer` (4 channels) -> `out`.
//! This is exactly S1/S2's chord-through-a-filter shape, but every node is now a real `Module`
//! wired through `ProcessIo`, not a direct `dsp::Saw`/`dsp::Svf` call.
//!
//! No cable system exists yet (that's v1/v2 scope), so `src/patch_demo.rs` *is* the patch: the
//! Rust code there plays the role cables + the compiler will play later. It's a hand-wired
//! stand-in, the same relationship S1's `CompiledGraph` has to the real compiler.

use kabl_engine::patch_demo::{Patch, BLOCK, SAMPLE_RATE};

#[test]
fn chord_through_real_modules_renders_clean_sustain_and_release() {
    let mut patch = Patch::new();
    patch.prepare();
    patch.note_on_chord();

    let sustain_blocks = (SAMPLE_RATE * 1.0 / BLOCK as f32) as usize; // 1s
    let release_blocks = (SAMPLE_RATE * 1.0 / BLOCK as f32) as usize; // 1s tail, plenty for a 300ms release

    let mut rendered = Vec::with_capacity((sustain_blocks + release_blocks) * BLOCK);
    for _ in 0..sustain_blocks {
        rendered.extend_from_slice(&patch.process_block());
    }
    patch.note_off_chord();
    for _ in 0..release_blocks {
        rendered.extend_from_slice(&patch.process_block());
    }

    // Robustness (brief section 13's fuzz idea, applied here): the real module chain, driven
    // through the actual Module trait, must never produce NaN/Inf.
    assert!(
        rendered.iter().all(|v| v.is_finite()),
        "patch produced non-finite output"
    );

    // Sanity: the sustained chord should have real energy...
    let sustain_rms = rms(&rendered[..sustain_blocks * BLOCK]);
    assert!(
        sustain_rms > 0.01,
        "sustained chord should have audible energy, got RMS {sustain_rms}"
    );

    // ...and after release, the tail should have decayed to (near) silence.
    let tail_start = rendered.len() - BLOCK * 10;
    let tail_rms = rms(&rendered[tail_start..]);
    assert!(
        tail_rms < sustain_rms * 0.05,
        "after release, output should have decayed well below sustain level: sustain={sustain_rms}, tail={tail_rms}"
    );

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/spike-renders/patch_integration.wav");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    write_wav(&path, &rendered, SAMPLE_RATE as u32);
    eprintln!(
        "patch integration: sustain_rms={sustain_rms:.4}, tail_rms={tail_rms:.6}, render={}",
        path.display()
    );
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
