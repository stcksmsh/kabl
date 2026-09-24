//! The stereo recorder: exact samples and frame count, finalization on Stop and on drop (app
//! exit), lost frames reported as an incomplete take, and write failures made visible.

use std::time::Duration;

use kabl_ui::record::{pair, pair_sized, Outcome, Tap};

const SR: u32 = 48000;

/// One audio "callback" of `frames` frames, value = running index (left) and its negative.
fn callback(tap: &mut Tap, start: &mut u64, frames: usize) {
    if tap.begin(frames) {
        for _ in 0..frames {
            let v = *start as f32;
            tap.frame(v, -v);
            *start += 1;
        }
    }
}

fn read(path: &std::path::Path) -> (hound::WavSpec, Vec<f32>) {
    let mut r = hound::WavReader::open(path).expect("readable wav");
    let spec = r.spec();
    (spec, r.samples::<f32>().map(|s| s.unwrap()).collect())
}

#[test]
fn a_take_holds_exactly_the_pushed_frames() {
    let dir = tempfile::tempdir().unwrap();
    let (mut rec, mut tap) = pair(SR);
    let mut n = 0u64;
    // Before Start nothing is taken.
    callback(&mut tap, &mut n, 256);
    assert_eq!(n, 0);
    let path = rec.start_at(dir.path().join("take.wav")).unwrap();
    for _ in 0..200 {
        callback(&mut tap, &mut n, 256);
        std::thread::sleep(Duration::from_micros(300));
    }
    // Stop is answered by the next callback, which pushes nothing.
    let stopper = std::thread::spawn(move || {
        for _ in 0..50 {
            let mut dummy = 0;
            callback(&mut tap, &mut dummy, 256);
            std::thread::sleep(Duration::from_millis(5));
        }
        tap
    });
    let outcome = rec.stop().unwrap();
    let mut tap = stopper.join().unwrap();
    assert_eq!(
        outcome,
        Outcome::Complete {
            path: path.clone(),
            frames: n
        }
    );
    let (spec, samples) = read(&path);
    assert_eq!(
        (spec.channels, spec.sample_rate, spec.bits_per_sample),
        (2, SR, 32)
    );
    assert_eq!(spec.sample_format, hound::SampleFormat::Float);
    assert_eq!(samples.len() as u64, 2 * n);
    for (i, f) in samples.chunks(2).enumerate() {
        assert_eq!(f, [i as f32, -(i as f32)]);
    }
    // After Stop nothing is taken, and a second take starts clean.
    let mut m = 0;
    callback(&mut tap, &mut m, 256);
    assert_eq!(m, 0);
    let second = rec.start_at(dir.path().join("second.wav")).unwrap();
    callback(&mut tap, &mut m, 100);
    std::thread::sleep(Duration::from_millis(30));
    let t = std::thread::spawn(move || {
        for _ in 0..50 {
            let mut d = 0;
            callback(&mut tap, &mut d, 64);
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    rec.stop();
    t.join().unwrap();
    assert_eq!(read(&second).1.len(), 200);
}

#[test]
fn a_full_ring_is_reported_and_the_take_marked_incomplete() {
    let dir = tempfile::tempdir().unwrap();
    let (mut rec, mut tap) = pair_sized(SR, 1024);
    rec.start_at(dir.path().join("fast.wav")).unwrap();
    let mut n = 0;
    // Far faster than the writer drains: some callbacks find no room.
    for _ in 0..400 {
        callback(&mut tap, &mut n, 256);
    }
    assert!(rec.lost() > 0);
    let outcome = rec.stop().unwrap();
    let Outcome::Incomplete { path, lost, frames } = outcome else {
        panic!("{outcome:?}");
    };
    assert!(path.to_string_lossy().ends_with("fast-INCOMPLETE.wav"));
    assert_eq!(lost + frames, 400 * 256);
    assert_eq!(read(&path).1.len() as u64, 2 * frames);
    assert!(!dir.path().join("fast.wav").exists());
}

#[test]
fn write_failures_are_visible() {
    let (mut rec, mut tap) = pair(SR);
    // A folder that can't be created.
    let err = rec
        .start_at("/proc/kabl-no/take.wav".into())
        .expect_err("unwritable");
    assert!(err.contains("can't record"), "{err}");
    assert!(matches!(rec.last, Some(Outcome::Failed(_))));
    // A device that accepts the open but fails every write.
    if let Ok(_path) = rec.start_at("/dev/full".into()) {
        let mut n = 0;
        for _ in 0..100 {
            callback(&mut tap, &mut n, 256);
            std::thread::sleep(Duration::from_millis(2));
        }
        let outcome = rec.stop().unwrap();
        assert!(matches!(outcome, Outcome::Failed(_)), "{outcome:?}");
    }
}

#[test]
fn stop_without_an_audio_callback_still_finalizes() {
    let dir = tempfile::tempdir().unwrap();
    let (mut rec, mut tap) = pair(SR);
    let path = rec.start_at(dir.path().join("t.wav")).unwrap();
    let mut n = 0;
    callback(&mut tap, &mut n, 480);
    // The device is gone: no callback will answer the stop.
    let outcome = rec.stop().unwrap();
    assert!(matches!(outcome, Outcome::Complete { frames: 480, .. }));
    assert_eq!(read(&path).1.len(), 960);
}

#[test]
fn dropping_the_recorder_finalizes_the_take() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("exit.wav");
    let (mut rec, mut tap) = pair(SR);
    rec.start_at(path.clone()).unwrap();
    let mut n = 0;
    callback(&mut tap, &mut n, 4800);
    std::thread::sleep(Duration::from_millis(50));
    drop(rec);
    assert_eq!(read(&path).1.len(), 9600);
}

#[test]
fn a_killed_process_leaves_a_readable_file_up_to_the_last_checkpoint() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("killed.wav");
    let (mut rec, mut tap) = pair(SR);
    rec.start_at(path.clone()).unwrap();
    let mut n = 0;
    callback(&mut tap, &mut n, 4800);
    // Past a checkpoint, and without Stop: what's on disk is already a valid WAV.
    std::thread::sleep(Duration::from_millis(800));
    let (spec, samples) = read(&path);
    assert_eq!(spec.channels, 2);
    assert_eq!(samples.len(), 9600);
    std::mem::forget(rec);
}

#[test]
fn takes_never_overwrite_and_record_again_right_after_stop() {
    let dir = tempfile::tempdir().unwrap();
    let (mut rec, mut tap) = pair(SR);
    rec.dir = dir.path().display().to_string();
    // Someone else's file where the first take would go is left alone.
    let path = dir.path().join("take.wav");
    std::fs::write(&path, b"not ours").unwrap();
    let got = rec.start_at(path.clone()).unwrap();
    assert_eq!(got, dir.path().join("take-2.wav"));
    let mut n = 0;
    callback(&mut tap, &mut n, 480);
    rec.stop();
    assert_eq!(std::fs::read(&path).unwrap(), b"not ours");
    // Three takes back to back within one second: three files.
    let mut names = Vec::new();
    for _ in 0..3 {
        let p = rec.start().unwrap();
        callback(&mut tap, &mut n, 480);
        rec.stop().unwrap();
        names.push(p);
    }
    names.sort();
    names.dedup();
    assert_eq!(names.len(), 3);
    for p in &names {
        assert_eq!(read(p).1.len(), 960);
    }
}

#[test]
fn the_meter_holds_peaks_and_latches_clips() {
    use kabl_ui::record::{Meter, PeakTap};
    let tap = PeakTap::default();
    tap.feed(0.5, -0.25);
    tap.feed(-0.1, 0.1);
    let (p, bad) = tap.take();
    assert_eq!((p, bad), ([0.5, 0.25], false));
    assert_eq!(tap.take().0, [0.0, 0.0]);
    let mut m = Meter::default();
    m.update(p, false, 0.05);
    assert!(!m.clip);
    assert!((m.max_db.unwrap() - 20.0 * 0.5f32.log10()).abs() < 1e-4);
    m.update([0.0, 0.0], false, 1.0);
    assert!(
        m.level[0] < 0.06 && m.level[0] > 0.04,
        "falls 20 dB/s: {}",
        m.level[0]
    );
    tap.feed(1.0, 0.0);
    let (p, _) = tap.take();
    m.update(p, false, 0.05);
    assert!(m.clip);
    tap.feed(f32::NAN, 0.0);
    let (p, bad) = tap.take();
    m.update(p, bad, 0.05);
    assert!(m.nonfinite);
}
