//! Import frame-size detection and correction, and the table editing operations.

use kabl_modules::tableedit as ed;
use kabl_modules::wavetable::{self, Detected, TableError, FRAME};

/// A 16-bit mono .wav of `samples`, with an optional `clm ` chunk.
fn wav(samples: &[f32], clm: Option<&str>) -> Vec<u8> {
    let mut body = b"WAVEfmt ".to_vec();
    body.extend(16u32.to_le_bytes());
    body.extend([1, 0, 1, 0]);
    body.extend(44100u32.to_le_bytes());
    body.extend(88200u32.to_le_bytes());
    body.extend([2, 0, 16, 0]);
    if let Some(c) = clm {
        let mut c = c.as_bytes().to_vec();
        body.extend(b"clm ");
        body.extend((c.len() as u32).to_le_bytes());
        if c.len() % 2 == 1 {
            c.push(0);
        }
        body.extend(c);
    }
    body.extend(b"data");
    body.extend((samples.len() as u32 * 2).to_le_bytes());
    for &v in samples {
        body.extend(((v * 32767.0) as i16).to_le_bytes());
    }
    let mut out = b"RIFF".to_vec();
    out.extend((body.len() as u32).to_le_bytes());
    out.extend(body);
    out
}

fn sine(n: usize, cycles: f32) -> Vec<f32> {
    (0..n)
        .map(|i| (std::f32::consts::TAU * cycles * i as f32 / n as f32).sin() * 0.5)
        .collect()
}

fn frames_of(canonical: &[u8]) -> Vec<Vec<f32>> {
    wavetable::decode_canonical(canonical).unwrap()
}

#[test]
fn detection_is_reported_and_a_chosen_length_overrides_it() {
    // 8 frames of 2048 with no clm: read as 8 frames, flagged ambiguous.
    let bytes = wav(&sine(8 * FRAME, 8.0), None);
    let (out, info) = wavetable::import_wav_with(&bytes, None).unwrap();
    assert_eq!((info.frames, info.frame_len), (8, 2048));
    assert_eq!(info.detected, Detected::Multiple);
    assert!(info.ambiguous());
    assert_eq!(frames_of(&out).len(), 8);
    // The same file read as one 16384-sample cycle.
    let (out, info) = wavetable::import_wav_with(&bytes, Some(16384)).unwrap();
    assert_eq!((info.frames, info.detected), (1, Detected::Chosen));
    assert_eq!(frames_of(&out).len(), 1);
    // A clm chunk wins over the multiple-of-2048 guess and is not ambiguous.
    let bytes = wav(&sine(4096, 4.0), Some("<!>1024 00000000"));
    let (_, info) = wavetable::import_wav_with(&bytes, None).unwrap();
    assert_eq!((info.frames, info.detected), (4, Detected::Clm));
    assert!(!info.ambiguous());
    // Any other length is one cycle.
    let (_, info) = wavetable::import_wav_with(&wav(&sine(1000, 1.0), None), None).unwrap();
    assert_eq!((info.frames, info.frame_len), (1, 1000));
    assert_eq!(info.detected, Detected::Cycle);
}

#[test]
fn a_long_file_is_sliced_and_the_tail_dropped() {
    let bytes = wav(&sine(10_000, 20.0), None);
    let (out, info) = wavetable::import_wav_with(&bytes, Some(1024)).unwrap();
    assert_eq!((info.frames, info.dropped), (9, 10_000 - 9 * 1024));
    assert_eq!(frames_of(&out).len(), 9);
    // Too many slices, a length longer than the file, and a length past the limit.
    assert_eq!(
        wavetable::import_wav_with(&wav(&sine(70 * 256, 5.0), None), Some(256)).unwrap_err(),
        TableError::TooManyFrames(70)
    );
    assert_eq!(
        wavetable::import_wav_with(&bytes, Some(20_000)).unwrap_err(),
        TableError::TooShort
    );
    assert_eq!(
        wavetable::import_wav_with(&wav(&vec![0.5; 100_000], None), Some(50_000)).unwrap_err(),
        TableError::FrameTooLong(50_000)
    );
}

#[test]
fn many_frames_of_an_odd_length_are_refused_not_ground_through() {
    let bytes = wav(&sine(40 * 3000, 40.0), Some("<!>3000 00000000"));
    assert!(matches!(
        wavetable::import_wav_with(&bytes, None),
        Err(TableError::Unsupported(_))
    ));
}

#[test]
fn a_power_of_two_cycle_keeps_its_harmonics() {
    // Harmonic 3 at 1024 samples must land on bin 3 of the 2048-sample frame.
    let (out, _) = wavetable::import_wav_with(&wav(&sine(1024, 3.0), None), None).unwrap();
    let h = ed::harmonics(&frames_of(&out)[0]);
    assert!(h[2] > 0.9, "{:?}", &h[..5]);
    assert!(h.iter().enumerate().all(|(i, &a)| i == 2 || a < 0.01));
}

fn unit(h: usize) -> Vec<f32> {
    (0..FRAME)
        .map(|i| (std::f32::consts::TAU * h as f32 * i as f32 / FRAME as f32).sin() * 0.5)
        .collect()
}

#[test]
fn bars_edit_one_harmonic_and_leave_the_rest() {
    let mut f = unit(2);
    ed::set_harmonic(&mut f, 5, 0.25);
    ed::set_harmonic(&mut f, 2, 0.1);
    let h = ed::harmonics(&f);
    assert!((h[4] - 0.25).abs() < 1e-3, "{}", h[4]);
    assert!((h[1] - 0.1).abs() < 1e-3, "{}", h[1]);
    assert!(h[0] < 1e-3 && h[2] < 1e-3);
    // Out-of-range harmonic: no change.
    let before = f.clone();
    ed::set_harmonic(&mut f, 0, 1.0);
    ed::set_harmonic(&mut f, 513, 1.0);
    assert_eq!(f, before);
}

#[test]
fn normalise_dc_and_fade_do_what_they_say() {
    let mut f: Vec<f32> = unit(1).iter().map(|v| v * 0.2 + 0.1).collect();
    ed::remove_dc(&mut f);
    assert!((f.iter().sum::<f32>() / FRAME as f32).abs() < 1e-5);
    ed::normalise(&mut f);
    assert!((ed::peak(&f) - ed::PEAK).abs() < 1e-5);
    ed::fade_edges(&mut f, 64);
    assert!(f[0].abs() < 1e-6 && f[FRAME - 1].abs() < 1e-3);
    assert_eq!(f[200], {
        let mut g: Vec<f32> = unit(1).iter().map(|v| v * 0.2 + 0.1).collect();
        ed::remove_dc(&mut g);
        ed::normalise(&mut g);
        g[200]
    });
    let mut silent = vec![0.0; FRAME];
    ed::normalise(&mut silent);
    assert!(silent.iter().all(|&v| v == 0.0));
}

#[test]
fn fill_between_steps_from_one_frame_to_the_other() {
    for spectral in [false, true] {
        let mut t = vec![unit(1), vec![0.0; FRAME], vec![0.0; FRAME], unit(3)];
        ed::fill_between(&mut t, 0, 3, spectral);
        let a: Vec<f32> = t.iter().map(|f| ed::harmonics(f)[0]).collect();
        let c: Vec<f32> = t.iter().map(|f| ed::harmonics(f)[2]).collect();
        assert!(
            a[0] > 0.49 && a[1] < a[0] && a[2] < a[1] && a[3] < 0.01,
            "{a:?}"
        );
        assert!(
            c[3] > 0.49 && c[2] < c[3] && c[1] < c[2] && c[0] < 0.01,
            "{c:?}"
        );
        // Ends untouched.
        assert_eq!(t[0], unit(1));
        assert_eq!(t[3], unit(3));
    }
    // Adjacent or out-of-range: nothing happens.
    let mut t = vec![unit(1), unit(2)];
    ed::fill_between(&mut t, 0, 1, true);
    ed::fill_between(&mut t, 1, 0, true);
    ed::fill_between(&mut t, 0, 9, true);
    assert_eq!(t, vec![unit(1), unit(2)]);
}

#[test]
fn spectral_morph_of_a_shifted_harmonic_keeps_its_level() {
    // The same sine, a quarter cycle apart: a waveform crossfade dips to 0.707 midway, the
    // spectral morph holds the level.
    let a = unit(1);
    let b: Vec<f32> = (0..FRAME)
        .map(|i| (std::f32::consts::TAU * (i as f32 / FRAME as f32 + 0.25)).sin() * 0.5)
        .collect();
    let lin = ed::harmonics(&ed::interpolate(&a, &b, 0.5))[0];
    let spec = ed::harmonics(&ed::morph(&a, &b, 0.5))[0];
    assert!(
        (lin - 0.5 * std::f32::consts::FRAC_1_SQRT_2).abs() < 0.01,
        "{lin}"
    );
    assert!((spec - 0.5).abs() < 0.01, "{spec}");
}

#[test]
fn edited_frames_round_trip_through_canonical_bytes() {
    let mut f = ed::sine();
    ed::set_harmonic(&mut f, 3, 0.01);
    let bytes = wavetable::canonical_wav(&[f.clone(), ed::sine()]);
    let back = frames_of(&bytes);
    assert_eq!(back.len(), 2);
    assert!(f.iter().zip(&back[0]).all(|(a, b)| (a - b).abs() < 1e-3));
    // Over full scale: `fit` brings the whole table under, keeping the ratio of the frames.
    let mut t = vec![ed::sine(), ed::sine()];
    ed::set_harmonic(&mut t[0], 3, 0.6);
    let before = (ed::peak(&t[0]), ed::peak(&t[1]));
    assert!(before.0 > 1.0 && !ed::fit(&mut [vec![0.5; FRAME]]));
    assert!(ed::fit(&mut t));
    assert!((ed::peak(&t[0]) - ed::PEAK).abs() < 1e-4);
    assert!((ed::peak(&t[1]) / ed::peak(&t[0]) - before.1 / before.0).abs() < 1e-4);
}
