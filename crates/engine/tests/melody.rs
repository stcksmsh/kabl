//! A simple melody through the same real-`Module` patch as `patch_integration.rs` — proving the
//! `midi.in` stand-in (note_on/note_off called directly, since `standalone`'s real MIDI/keyboard
//! input doesn't exist yet — see `midi_in.rs`'s doc comment) works for a *sequence* of notes on
//! one voice, not just a single held chord across all four. Same mechanism, different sequencing.
//!
//! Melody: "Twinkle Twinkle Little Star," opening phrase (C C G G A A G), voice 0 only; voices
//! 1-3 stay silent. Not a real player — a fixed, hand-timed sequence, the same relationship
//! `patch_integration.rs`'s chord has to a live performance.

use kabl_engine::patch_demo::{Patch, BLOCK, SAMPLE_RATE};

// Semitones from C4 (patch_demo's BASE_HZ, 261.63 Hz): C=0, G=7, A=9.
const MELODY_SEMITONES: [f32; 7] = [0.0, 0.0, 7.0, 7.0, 9.0, 9.0, 7.0];

#[test]
fn twinkle_twinkle_opening_phrase_plays_as_a_sequence_of_distinct_notes() {
    let mut patch = Patch::new();
    patch.prepare();

    let hold_blocks = (SAMPLE_RATE * 0.28 / BLOCK as f32) as usize; // ~280ms per note
    let gap_blocks = (SAMPLE_RATE * 0.15 / BLOCK as f32) as usize; // ~150ms silence between notes
    let tail_blocks = (SAMPLE_RATE * 0.6 / BLOCK as f32) as usize; // final release tail

    let mut rendered = Vec::with_capacity(
        (MELODY_SEMITONES.len() * (hold_blocks + gap_blocks) + tail_blocks) * BLOCK,
    );
    let mut note_windows = Vec::with_capacity(MELODY_SEMITONES.len());

    for &semitones in &MELODY_SEMITONES {
        patch.note_on(0, semitones, 0.9);
        let note_start = rendered.len();
        for _ in 0..hold_blocks {
            rendered.extend_from_slice(&patch.process_block());
        }
        patch.note_off(0);
        for _ in 0..gap_blocks {
            rendered.extend_from_slice(&patch.process_block());
        }
        note_windows.push((note_start, rendered.len()));
    }
    for _ in 0..tail_blocks {
        rendered.extend_from_slice(&patch.process_block());
    }

    assert!(
        rendered.iter().all(|v| v.is_finite()),
        "melody render produced non-finite output"
    );

    // Each note's hold window should have real energy, and each gap should dip measurably below
    // its note's peak — evidence this is actually a sequence of distinct notes, not one smeared
    // held tone. (Not silence in the gaps: release_ms=300 in patch_demo means a 150ms gap
    // doesn't fully decay, which is expected and fine — it should still dip.)
    for (i, &(start, end)) in note_windows.iter().enumerate() {
        let hold_region = &rendered[start..start + hold_blocks * BLOCK];
        let gap_region = &rendered[start + hold_blocks * BLOCK..end];
        let hold_rms = rms(hold_region);
        let gap_rms = rms(gap_region);
        assert!(
            hold_rms > 0.01,
            "note {i}: hold should have audible energy, got RMS {hold_rms}"
        );
        assert!(
            gap_rms < hold_rms,
            "note {i}: gap should dip below the note's hold level: hold={hold_rms}, gap={gap_rms}"
        );
    }

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/spike-renders/melody.wav");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    write_wav(&path, &rendered, SAMPLE_RATE as u32);
    eprintln!(
        "melody render: {} notes, {}",
        MELODY_SEMITONES.len(),
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
