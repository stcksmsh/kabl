//! Spike S1 (brief section 11): "Can we swap compiled graphs with state carry-over and no
//! click?" Pass criterion: "Swap gate passes on a sustained saw->SVF chord while toggling a
//! cable 100x."
//!
//! Method (see docs/decisions.md 2026-09-21 "Spike S1" for the full writeup):
//!   - A sustained 4-voice saw chord runs through `Engine`, whose cable `depth` is toggled
//!     between 1.0 and 0.7, 100 times, each toggle handed across an `rtrb` single-slot channel
//!     (brief section 7.6) exactly like the real control->audio hand-off would be.
//!   - In parallel, a `hard` reference graph applies the *same* depth changes at the *same*
//!     sample, instantaneously (no crossfade) — used only as a diagnostic baseline (see
//!     `excess_jump` below), not as the click gate itself.
//!   - Because oscillator phase doesn't depend on `cable_depth` (only the mix->filter stage
//!     does), `engine`'s post-crossfade output is provably bit-identical to `hard`'s output
//!     everywhere *outside* a crossfade window — see the decisions.md entry for the derivation.
//!     That gives an exact, not approximate, null-test residual (brief section 13).
//!   - The click gate itself is curvature-based: a click is an anomalous kink, so engine's
//!     second-difference at each swap boundary is compared against the worst curvature the
//!     sustained chord produces on its own — a first attempt at comparing raw sample-to-sample
//!     deltas against the hard-switch reference turned out not to be a stable "click" signal at
//!     this filter's cutoff/sample-rate ratio (see the git history / decisions.md for why).
//!   - The whole per-block audio-thread path (`consumer.pop` + `Engine::process_block`) runs
//!     inside `assert_no_alloc!`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::{Collector, Owned};
use kabl_engine::graph::{CompiledGraph, BLOCK};
use kabl_engine::swap::{Engine, CROSSFADE_MS};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;
const DEPTH_A: f32 = 1.0;
const DEPTH_B: f32 = 0.7;
const NUM_TOGGLES: usize = 100;
/// 20 blocks = 1280 samples =~ 26.7 ms at 48 kHz, comfortably longer than the 15 ms crossfade
/// so consecutive swaps never overlap (see `Engine::build_swap`'s documented limitation).
const BLOCKS_PER_TOGGLE: usize = 20;

#[test]
fn spike_s1_swap_no_click_and_no_alloc() {
    let mut collector = Collector::new();
    let handle = collector.handle();

    let mut engine = Engine::new(&handle, SR, DEPTH_A);
    let mut hard = CompiledGraph::new(DEPTH_A);
    let crossfade_samples = engine.crossfade_samples();

    let (mut producer, mut consumer) = rtrb::RingBuffer::<Owned<CompiledGraph>>::new(1);

    let total_samples = NUM_TOGGLES * BLOCKS_PER_TOGGLE * BLOCK;
    let mut engine_out: Vec<f32> = Vec::with_capacity(total_samples);
    let mut hard_out: Vec<f32> = Vec::with_capacity(total_samples);
    let mut swap_sample_indices: Vec<usize> = Vec::with_capacity(NUM_TOGGLES);

    let mut current_depth = DEPTH_A;

    for _ in 0..NUM_TOGGLES {
        let new_depth = if current_depth == DEPTH_A {
            DEPTH_B
        } else {
            DEPTH_A
        };
        current_depth = new_depth;

        // Control-thread work: allowed to allocate.
        let new_graph = engine.build_swap(&handle, new_depth);
        producer
            .push(new_graph)
            .expect("single-slot channel should be empty between swaps");
        hard.cable_depth = new_depth;
        swap_sample_indices.push(engine_out.len());

        for _ in 0..BLOCKS_PER_TOGGLE {
            let mut buf = [0f32; BLOCK];
            let mut hbuf = [0f32; BLOCK];
            assert_no_alloc(|| {
                // Audio-thread work: must not allocate or deallocate.
                if let Ok(new_graph) = consumer.pop() {
                    engine.receive_swap(new_graph);
                }
                engine.process_block(SR, &mut buf);
                hard.process_block(SR, &mut hbuf);
            });
            engine_out.extend_from_slice(&buf);
            hard_out.extend_from_slice(&hbuf);
        }

        // Off the audio thread: reap whatever the last swap queued for deferred drop.
        collector.collect();
    }

    assert_eq!(engine_out.len(), total_samples);

    // --- click metric ---
    // A "click" is an anomalous *kink*: a jump in slope bigger than the waveform's own
    // movement ever produces elsewhere. Curvature (second difference) is the natural measure of
    // that, and it's self-contained — no need to compare against a synthetic hard-switch
    // reference, whose jump size (see `excess_jump` below) turns out to depend heavily on the
    // filter's cutoff/sample-rate ratio and isn't a stable "what a click looks like" baseline.
    //
    // Separately, `excess_jump` reports the actual value a naive (uncrossfaded) parameter
    // change would have injected at the transition sample, isolated from ordinary waveform
    // movement: since engine's crossfade is defined to start at (old=1,new=0) — i.e.
    // `engine_out[swap_at]` is *exactly* the old graph's natural continuation, with zero
    // contribution from the new graph yet — `hard_out[swap_at] - engine_out[swap_at]` is
    // precisely the component a hard switch would have added that the crossfade instead defers.
    // It's diagnostic, not a pass/fail gate (see below).
    let curvature =
        |buf: &[f32], i: usize| -> f32 { (buf[i + 1] - 2.0 * buf[i] + buf[i - 1]).abs() };

    let mut typical_curvature = 0.0f32;
    for &swap_at in &swap_sample_indices {
        let steady_start = swap_at + crossfade_samples + 8;
        for i in steady_start..(swap_at + BLOCKS_PER_TOGGLE * BLOCK).saturating_sub(8) {
            if i + 1 < total_samples && i > 0 {
                typical_curvature = typical_curvature.max(curvature(&engine_out, i));
            }
        }
    }

    let mut boundary_curvature = 0.0f32;
    let mut excess_jumps = Vec::with_capacity(NUM_TOGGLES);
    for &swap_at in &swap_sample_indices {
        if swap_at == 0 || swap_at + 1 >= total_samples {
            continue;
        }
        boundary_curvature = boundary_curvature.max(curvature(&engine_out, swap_at));
        excess_jumps.push((hard_out[swap_at] - engine_out[swap_at]).abs());
    }
    let avg_excess_jump = excess_jumps.iter().sum::<f32>() / excess_jumps.len() as f32;
    let max_excess_jump = excess_jumps.iter().cloned().fold(0.0f32, f32::max);

    // --- residual metric: outside crossfade windows, engine and hard must match exactly ---
    // (see module doc: oscillator phase doesn't depend on depth, so once a swap's crossfade
    // finishes, engine's active graph and hard's continuous instance have run the identical
    // input sequence through identical starting filter state.)
    let mut max_residual = 0.0f32;
    let mut sum_sq_residual = 0.0f64;
    let mut steady_samples = 0usize;
    for (idx, &swap_at) in swap_sample_indices.iter().enumerate() {
        let steady_start = swap_at + crossfade_samples;
        let steady_end = swap_sample_indices
            .get(idx + 1)
            .copied()
            .unwrap_or(total_samples);
        for i in steady_start..steady_end {
            let residual = (engine_out[i] - hard_out[i]).abs();
            max_residual = max_residual.max(residual);
            sum_sq_residual += (residual as f64) * (residual as f64);
            steady_samples += 1;
        }
    }
    let residual_rms = (sum_sq_residual / steady_samples as f64).sqrt() as f32;
    let residual_dbfs = if residual_rms > 0.0 {
        20.0 * residual_rms.log10()
    } else {
        f32::NEG_INFINITY
    };

    eprintln!(
        "spike S1: {} toggles, crossfade={:.1}ms ({} samples)",
        NUM_TOGGLES, CROSSFADE_MS, crossfade_samples
    );
    eprintln!(
        "  curvature: typical (in-chord) max={:.5}, at swap boundaries max={:.5} ({:.2}x)",
        typical_curvature,
        boundary_curvature,
        boundary_curvature / typical_curvature.max(1e-12)
    );
    eprintln!(
        "  diagnostic — excess jump a hard switch would add at the transition (engine defers \
         this over the crossfade instead): avg={:.5}, max={:.5}",
        avg_excess_jump, max_excess_jump
    );
    eprintln!(
        "  steady-state residual (engine vs. hard-switch reference): max={:.3e}, rms={:.3e} ({:.1} dBFS) over {} samples",
        max_residual, residual_rms, residual_dbfs, steady_samples
    );

    assert!(
        boundary_curvature <= typical_curvature * 1.5,
        "engine's own output should have no anomalous kink at a swap boundary — curvature there \
         ({:.5}) should stay within 1.5x of the worst curvature the sustained chord produces on \
         its own ({:.5}); a real click would show up as a large outlier here",
        boundary_curvature,
        typical_curvature
    );
    assert!(
        residual_dbfs < -80.0,
        "steady-state residual should be below -80 dBFS (brief section 13's null-test gate); \
         got {:.1} dBFS",
        residual_dbfs
    );

    let render_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/spike-renders");
    std::fs::create_dir_all(&render_dir).expect("create render dir");
    let path = render_dir.join("spike_s1_render.wav");
    write_wav(&path, &engine_out, SR as u32);
    eprintln!("  render: {}", path.display());
}

/// Confirms `basedrop`'s actual deferred-collection behaviour in isolation from `Engine`: an
/// `Owned<T>` dropped inline does not run `T`'s destructor — it queues the node — and the
/// destructor only runs once `Collector::collect()` is called. This is the mechanism the swap
/// path above relies on to keep the audio thread free of deallocation.
#[test]
fn owned_drop_is_deferred_until_collect() {
    struct DropCounted(Arc<AtomicUsize>, #[allow(dead_code)] Vec<f32>);
    impl Drop for DropCounted {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    let mut collector = Collector::new();
    let handle = collector.handle();
    let counter = Arc::new(AtomicUsize::new(0));

    const N: usize = 8;
    for _ in 0..N {
        let owned = Owned::new(&handle, DropCounted(counter.clone(), vec![0.0; 64]));
        assert_no_alloc(|| {
            drop(owned);
        });
        assert_eq!(
            counter.load(Ordering::SeqCst),
            0,
            "dropping Owned<T> on the (simulated) audio thread must not run T's destructor yet"
        );
    }

    collector.collect();
    assert_eq!(
        counter.load(Ordering::SeqCst),
        N,
        "Collector::collect() should have run every queued destructor exactly once"
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
