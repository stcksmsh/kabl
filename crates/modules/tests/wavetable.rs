//! `osc.wt` and the wavetable import: spectral correctness, aliasing across the keyboard,
//! position morphing, and every way an import can fail.

mod common;
use common::*;
use kabl_modules::builtins::OscWt;
use kabl_modules::wavetable::{self, TableError, WaveTable, FACTORY_COUNT, FRAME};

/// A .wav of `samples` (-1..1): `tag` 1 = PCM, 3 = float, 0xFFFE = extensible PCM.
fn wav(
    tag: u16,
    bits: u16,
    channels: u16,
    samples: &[f64],
    extra: &[(&[u8; 4], Vec<u8>)],
) -> Vec<u8> {
    let mut data = Vec::new();
    for &v in samples {
        for _ in 0..channels {
            match (tag, bits) {
                (1 | 0xFFFE, 8) => data.push((v * 127.0 + 128.0) as u8),
                (1 | 0xFFFE, 16) => data.extend(((v * 32767.0) as i16).to_le_bytes()),
                (1 | 0xFFFE, 24) => data.extend(&((v * 8388607.0) as i32).to_le_bytes()[..3]),
                (1 | 0xFFFE, 32) => data.extend(((v * 2147483647.0) as i32).to_le_bytes()),
                (3, 32) => data.extend((v as f32).to_le_bytes()),
                (3, 64) => data.extend(v.to_le_bytes()),
                _ => data.extend([0u8; 2]),
            }
        }
    }
    let mut fmt = Vec::new();
    fmt.extend(tag.to_le_bytes());
    fmt.extend(channels.to_le_bytes());
    fmt.extend(44100u32.to_le_bytes());
    fmt.extend((44100 * channels as u32 * (bits as u32 / 8)).to_le_bytes());
    fmt.extend((channels * (bits / 8)).to_le_bytes());
    fmt.extend(bits.to_le_bytes());
    if tag == 0xFFFE {
        fmt.extend([22, 0, bits as u8, 0, 0, 0, 0, 0]);
        fmt.extend(1u16.to_le_bytes()); // sub-format PCM
        fmt.extend([0u8; 14]);
    }
    let mut body = b"WAVE".to_vec();
    body.extend(b"fmt ");
    body.extend((fmt.len() as u32).to_le_bytes());
    body.extend(&fmt);
    for (id, chunk) in extra {
        body.extend(*id);
        body.extend((chunk.len() as u32).to_le_bytes());
        body.extend(chunk);
        if chunk.len() % 2 == 1 {
            body.push(0);
        }
    }
    body.extend(b"data");
    body.extend((data.len() as u32).to_le_bytes());
    body.extend(&data);
    let mut out = b"RIFF".to_vec();
    out.extend((body.len() as u32).to_le_bytes());
    out.extend(body);
    out
}

fn cycle(n: usize, f: impl Fn(f64) -> f64) -> Vec<f64> {
    (0..n).map(|i| f(i as f64 / n as f64)).collect()
}

fn saw_full(n: usize) -> Vec<f64> {
    cycle(n, |t| 2.0 * t - 1.0)
}

fn osc_with(table: &[u8], set: &[(&str, f32)]) -> Rig {
    let mut r = Rig::new("osc.wt", set, SR);
    let t = WaveTable::from_canonical_cached(table).unwrap();
    r.m.as_any_mut()
        .downcast_mut::<OscWt>()
        .unwrap()
        .set_user(Some(t));
    r.set("user", 1.0);
    r
}

/// Pitch input (semitones from `base_hz`) for a note of `hz`.
fn semis(hz: f32, base: f32) -> f32 {
    12.0 * (hz / base).log2()
}

#[test]
fn factory_tables_decode_and_stay_in_range() {
    assert_eq!(wavetable::FACTORY_NAMES.len(), FACTORY_COUNT);
    for i in 0..FACTORY_COUNT {
        let t = wavetable::factory(i);
        assert!(t.frames() >= 16, "{}", wavetable::FACTORY_NAMES[i]);
        for pos in [0.0, 0.3, 1.0] {
            let v: Vec<f32> = (0..FRAME)
                .map(|k| t.read(0, pos, k as f32 / FRAME as f32))
                .collect();
            assert!(
                peak(&v) > 0.5 && peak(&v) <= 1.05,
                "{} at {pos}: peak {}",
                wavetable::FACTORY_NAMES[i],
                peak(&v)
            );
        }
    }
}

#[test]
fn saw_frame_has_one_over_h_harmonics() {
    // "Saw to Square" frame 0 is a saw: harmonic h has amplitude 1/h of the fundamental.
    let f0 = 440.3f32;
    let mut r = Rig::new("osc.wt", &[("table", 0.0), ("position", 0.0)], SR);
    let x = r.render(4096 + 16384, 0, |k, _| {
        if k == 0 {
            semis(f0, 261.63)
        } else {
            0.0
        }
    })[4096..]
        .to_vec();
    let fund = {
        let p = spectrum(&x);
        p[(f0 as f64 * x.len() as f64 / SR as f64).round() as usize - 2..][..5]
            .iter()
            .sum::<f64>()
            .sqrt()
    };
    for h in 2..=10usize {
        let hz = f0 as f64 * h as f64;
        let bin = (hz * x.len() as f64 / SR as f64).round() as usize;
        let p = spectrum(&x);
        let a = p[bin - 2..][..5].iter().sum::<f64>().sqrt();
        let db = 20.0 * (a / fund * h as f64).log10();
        assert!(db.abs() < 0.5, "harmonic {h}: {db:.2} dB off 1/h");
    }
}

#[test]
fn square_frame_has_no_even_harmonics() {
    let f0 = 440.3f32;
    let mut r = Rig::new("osc.wt", &[("table", 0.0), ("position", 1.0)], SR);
    let x = r.render(4096 + 16384, 0, |k, _| {
        if k == 0 {
            semis(f0, 261.63)
        } else {
            0.0
        }
    })[4096..]
        .to_vec();
    let p = spectrum(&x);
    let at = |h: usize| {
        p[(f0 as f64 * h as f64 * x.len() as f64 / SR as f64).round() as usize - 2..][..5]
            .iter()
            .sum::<f64>()
            .sqrt()
    };
    for h in [2usize, 4, 6, 8] {
        let db = 20.0 * (at(h) / at(1)).log10();
        assert!(db < -70.0, "even harmonic {h}: {db:.1} dB");
    }
    assert!(20.0 * (at(3) / at(1)).log10() > -10.5);
}

#[test]
fn full_bandwidth_saw_aliases_less_than_a_pen_stroke_across_the_keyboard() {
    // 512 harmonics, the brightest a table can be: the mipmaps must keep every note clean.
    let canonical = wavetable::import_wav(&wav(3, 32, 1, &saw_full(2048), &[])).unwrap();
    let mut worst = (f64::MIN, 0.0);
    for note in (24..=108).step_by(3) {
        let hz = 440.0 * 2f32.powf((note as f32 - 69.0) / 12.0);
        let mut r = osc_with(&canonical, &[]);
        let x = r.render(4096 + 16384, 0, |k, _| {
            if k == 0 {
                semis(hz, 261.63)
            } else {
                0.0
            }
        })[4096..]
            .to_vec();
        let (_, total) = alias_db(&x, hz, SR, 20000.0);
        if total > worst.0 {
            worst = (total, hz as f64);
        }
    }
    println!(
        "worst alias across MIDI 24-108: {:.1} dB at {:.0} Hz",
        worst.0, worst.1
    );
    assert!(worst.0 < -65.0, "{:.1} dB at {:.0} Hz", worst.0, worst.1);
}

#[test]
fn level_choice_never_allows_a_harmonic_past_nyquist() {
    let t = wavetable::factory(0);
    let mut last = 0;
    for step in 1..2000 {
        let dt = step as f32 * 0.00025;
        let k = t.level_for(dt);
        let harmonics = 512usize >> k;
        assert!(k >= last, "level went back up at dt {dt}");
        assert!(
            harmonics as f32 * dt <= 0.5 + 1e-6 || k == 9,
            "dt {dt}: {harmonics} harmonics"
        );
        last = k;
    }
}

#[test]
fn position_is_a_linear_morph_between_frames() {
    // Two frames: a sine and its octave. Halfway is their average.
    let mut s = cycle(FRAME, |t| (std::f64::consts::TAU * t).sin());
    s.extend(cycle(FRAME, |t| (std::f64::consts::TAU * 2.0 * t).sin()));
    let canonical = wavetable::import_wav(&wav(1, 16, 1, &s, &[])).unwrap();
    let t = WaveTable::from_canonical_cached(&canonical).unwrap();
    assert_eq!(t.frames(), 2);
    for k in 0..64 {
        let ph = k as f32 / 64.0;
        let (a, b, m) = (t.read(0, 0.0, ph), t.read(0, 1.0, ph), t.read(0, 0.5, ph));
        assert!((m - 0.5 * (a + b)).abs() < 1e-5);
    }
}

#[test]
fn pos_input_equals_the_position_param() {
    let a = {
        let mut r = Rig::new("osc.wt", &[("table", 5.0), ("position", 0.4)], SR);
        r.render(2048, 0, |_, _| 0.0)
    };
    let b = {
        let mut r = Rig::new("osc.wt", &[("table", 5.0), ("position", 0.0)], SR);
        r.render(2048, 0, |k, _| if k == 1 { 0.4 } else { 0.0 })
    };
    for (x, y) in a.iter().zip(&b) {
        assert!((x - y).abs() < 1e-6);
    }
    // pos_mod scales the input.
    let c = {
        let mut r = Rig::new(
            "osc.wt",
            &[("table", 5.0), ("position", 0.0), ("pos_mod", 0.5)],
            SR,
        );
        r.render(2048, 0, |k, _| if k == 1 { 0.8 } else { 0.0 })
    };
    for (x, y) in a.iter().zip(&c) {
        assert!((x - y).abs() < 1e-6);
    }
}

#[test]
fn position_sweep_has_no_discontinuities() {
    // A sweep across 16 frames of a smooth table must not click: the largest sample step stays
    // within what the waveform itself does at that pitch.
    let f0 = 110.0f32;
    let mut stepped = Rig::new("osc.wt", &[("table", 4.0)], SR);
    let swept = stepped.render(48000, 0, |k, i| {
        if k == 0 {
            semis(f0, 261.63)
        } else {
            i as f32 / 48000.0
        }
    });
    let jump = swept
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0f32, f32::max);
    let mut fixed = Rig::new("osc.wt", &[("table", 4.0)], SR);
    let still = fixed.render(
        48000,
        0,
        |k, _| if k == 0 { semis(f0, 261.63) } else { 0.5 },
    );
    let natural = still
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0f32, f32::max);
    assert!(
        jump < natural * 1.6,
        "sweep jump {jump} vs natural {natural}"
    );
}

#[test]
fn block_rate_position_changes_ramp_instead_of_stepping() {
    let mut r = Rig::new("osc.wt", &[("table", 0.0), ("position", 0.0)], SR);
    let pitch = [semis(110.0, 261.63); BLOCK];
    for _ in 0..4 {
        r.block(&[&pitch]);
    }
    r.set("position", 1.0);
    let after = r.block(&[&pitch])[0];
    let at = |position: f32| {
        let mut twin = Rig::new("osc.wt", &[("table", 0.0), ("position", position)], SR);
        for _ in 0..4 {
            twin.block(&[&pitch]);
        }
        twin.block(&[&pitch])[0]
    };
    let (old, new) = (at(0.0), at(1.0));
    // The block starts a 64th of the way along the ramp and ends on the new position.
    let first = (after[0] - old[0]).abs() / (new[0] - old[0]).abs();
    assert!(
        first < 0.1,
        "first sample moved {first:.2} of the way at once"
    );
    assert!((after[BLOCK - 1] - new[BLOCK - 1]).abs() < 1e-4);
}

#[test]
fn missing_user_table_plays_the_factory_table() {
    let mut r = Rig::new("osc.wt", &[("table", 2.0), ("user", 3.0)], SR);
    let a = r.render(1024, 0, |_, _| 0.0);
    let mut f = Rig::new("osc.wt", &[("table", 2.0), ("user", 0.0)], SR);
    assert_eq!(a, f.render(1024, 0, |_, _| 0.0));
    assert!(peak(&a) > 0.1);
}

#[test]
fn nan_pitch_does_not_poison_the_phase() {
    let mut r = Rig::new("osc.wt", &[], SR);
    r.block(&[&[f32::NAN; BLOCK]]);
    let x = r.block(&[&[0.0; BLOCK]])[0];
    assert!(x.iter().all(|v| v.is_finite()));
}

#[test]
fn carry_from_continues_bit_exactly() {
    let mut a = Rig::new("osc.wt", &[("table", 3.0), ("position", 0.7)], SR);
    for _ in 0..10 {
        a.block(&[&[3.0; BLOCK]]);
    }
    let mut b = Rig::new("osc.wt", &[("table", 3.0), ("position", 0.7)], SR);
    b.m.carry_from(a.m.as_ref());
    for _ in 0..5 {
        assert_eq!(a.block(&[&[3.0; BLOCK]]), b.block(&[&[3.0; BLOCK]]));
    }
}

// ---- import ----

#[test]
fn single_cycle_of_any_length_becomes_one_frame() {
    // An AKWF-style 600-sample cycle.
    let c = cycle(600, |t| {
        (std::f64::consts::TAU * t).sin() + 0.3 * (std::f64::consts::TAU * 3.0 * t).sin()
    });
    let canonical = wavetable::import_wav(&wav(1, 16, 1, &c, &[])).unwrap();
    let frames = wavetable::decode_canonical(&canonical).unwrap();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].len(), FRAME);
    let peak = frames[0].iter().fold(0.0f32, |m, v| m.max(v.abs()));
    assert!((peak - 0.98).abs() < 0.01, "normalised to 0.98, got {peak}");
}

#[test]
fn multiple_of_2048_is_that_many_frames() {
    let mut s = Vec::new();
    for h in 1..=4 {
        s.extend(cycle(FRAME, |t| {
            (std::f64::consts::TAU * h as f64 * t).sin()
        }));
    }
    let canonical = wavetable::import_wav(&wav(1, 24, 1, &s, &[])).unwrap();
    assert_eq!(wavetable::decode_canonical(&canonical).unwrap().len(), 4);
}

#[test]
fn clm_chunk_sets_the_frame_length() {
    let mut s = Vec::new();
    for h in 1..=3 {
        s.extend(cycle(1024, |t| {
            (std::f64::consts::TAU * h as f64 * t).sin()
        }));
    }
    let clm = b"<!>1024 10000000 wavetable (test)".to_vec();
    let canonical = wavetable::import_wav(&wav(1, 16, 1, &s, &[(b"clm ", clm)])).unwrap();
    assert_eq!(wavetable::decode_canonical(&canonical).unwrap().len(), 3);
}

#[test]
fn formats_stereo_extensible_float_and_8_bit_all_import() {
    let c = cycle(512, |t| (std::f64::consts::TAU * t).sin());
    for (tag, bits, ch) in [
        (1, 8, 1),
        (1, 16, 2),
        (1, 24, 1),
        (1, 32, 1),
        (3, 32, 1),
        (3, 64, 2),
        (0xFFFE, 16, 1),
    ] {
        let r = wavetable::import_wav(&wav(tag, bits, ch, &c, &[]));
        assert!(r.is_ok(), "tag {tag} bits {bits} ch {ch}: {r:?}");
    }
}

#[test]
fn reimporting_canonical_bytes_changes_nothing_audible() {
    let c = cycle(FRAME, |t| {
        (std::f64::consts::TAU * t).sin() + 0.5 * (std::f64::consts::TAU * 5.0 * t).sin()
    });
    let once = wavetable::import_wav(&wav(1, 16, 1, &c, &[])).unwrap();
    let twice = wavetable::import_wav(&once).unwrap();
    let (a, b) = (
        wavetable::decode_canonical(&once).unwrap(),
        wavetable::decode_canonical(&twice).unwrap(),
    );
    for (x, y) in a[0].iter().zip(&b[0]) {
        assert!((x - y).abs() < 2e-4);
    }
}

#[test]
fn import_errors_are_specific() {
    let ok = wav(1, 16, 1, &cycle(600, |t| t - 0.5), &[]);
    assert_eq!(wavetable::import_wav(b"hello"), Err(TableError::NotWav));
    assert_eq!(wavetable::import_wav(&[]), Err(TableError::NotWav));
    let mut not_wave = ok.clone();
    not_wave[8..12].copy_from_slice(b"AVI ");
    assert_eq!(wavetable::import_wav(&not_wave), Err(TableError::NotWav));
    // Cut inside the fmt chunk, and a chunk size running past the end.
    assert_eq!(wavetable::import_wav(&ok[..30]), Err(TableError::Truncated));
    let mut lies = ok.clone();
    lies[16..20].copy_from_slice(&1_000_000u32.to_le_bytes());
    assert_eq!(wavetable::import_wav(&lies), Err(TableError::Truncated));
    // No data chunk.
    let mut nodata = ok.clone();
    let at = nodata.windows(4).position(|w| w == b"data").unwrap();
    nodata[at..at + 4].copy_from_slice(b"junk");
    assert_eq!(wavetable::import_wav(&nodata), Err(TableError::NotWav));
    // ADPCM, 12-bit, zero channels.
    assert!(matches!(
        wavetable::import_wav(&wav(2, 4, 1, &[0.0; 64], &[])),
        Err(TableError::Unsupported(_))
    ));
    assert!(matches!(
        wavetable::import_wav(&wav(1, 12, 1, &[0.0; 64], &[])),
        Err(TableError::Unsupported(_))
    ));
    let mut zero_ch = ok.clone();
    zero_ch[22..24].copy_from_slice(&0u16.to_le_bytes());
    assert!(matches!(
        wavetable::import_wav(&zero_ch),
        Err(TableError::Unsupported(_))
    ));
    // Too short, silent, NaN.
    assert_eq!(
        wavetable::import_wav(&wav(1, 16, 1, &[0.1; 8], &[])),
        Err(TableError::TooShort)
    );
    assert_eq!(
        wavetable::import_wav(&wav(1, 16, 1, &[0.0; 600], &[])),
        Err(TableError::Silent)
    );
    let mut nan = cycle(600, |t| t - 0.5);
    nan[10] = f64::NAN;
    assert_eq!(
        wavetable::import_wav(&wav(3, 32, 1, &nan, &[])),
        Err(TableError::NotFinite)
    );
    // 65 frames, and a file over the size limit.
    assert_eq!(
        wavetable::import_wav(&wav(
            1,
            16,
            1,
            &cycle(65 * FRAME, |t| (t * 400.0).sin()),
            &[]
        )),
        Err(TableError::TooManyFrames(65))
    );
    let mut huge = ok.clone();
    huge.resize(wavetable::MAX_IMPORT_BYTES + 1, 0);
    assert_eq!(wavetable::import_wav(&huge), Err(TableError::TooLarge));
}

#[test]
fn a_cut_short_data_chunk_still_imports_what_is_there() {
    let mut w = wav(
        1,
        16,
        1,
        &cycle(600, |t| (std::f64::consts::TAU * t).sin()),
        &[],
    );
    w.truncate(w.len() - 7); // streamed file: the data chunk claims more than the file holds
    assert!(wavetable::import_wav(&w).is_ok());
}

#[test]
fn decode_canonical_refuses_other_wavs() {
    let plain = wav(1, 16, 1, &cycle(600, |t| t), &[]);
    assert_eq!(
        wavetable::decode_canonical(&plain).err(),
        Some(TableError::NotCanonical)
    );
    let stereo = wav(1, 16, 2, &cycle(FRAME, |t| t), &[]);
    assert_eq!(
        wavetable::decode_canonical(&stereo).err(),
        Some(TableError::NotCanonical)
    );
    assert!(WaveTable::from_canonical_cached(&plain).is_err());
}

#[test]
fn identical_bytes_share_one_decoded_table() {
    let c = wavetable::import_wav(&wav(1, 16, 1, &cycle(600, |t| (t * 9.0).sin()), &[])).unwrap();
    let a = WaveTable::from_canonical_cached(&c).unwrap();
    let b = WaveTable::from_canonical_cached(&c).unwrap();
    assert!(std::sync::Arc::ptr_eq(&a, &b));
}
