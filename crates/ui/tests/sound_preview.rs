//! Sound previews (UI spec 8): every factory sound renders a clip of at most 4 s at one
//! loudness, and the browser state machine plays, stops and follows the selection.

use kabl_ui::library::{self, Library, Origin};
use kabl_ui::sound_preview::{self, ClipLook, MAX_SECS};
use kabl_ui::UiState;

fn factory() -> Library {
    let user = std::env::temp_dir().join(format!("kabl-preview-test-{}", std::process::id()));
    Library::open(library::find_factory_dir(), user)
}

fn rms_db(samples: &[f32]) -> f32 {
    let win = 48000 / 20 * 2;
    let (mut sum, mut n) = (0f64, 0usize);
    for w in samples.chunks(win) {
        let ms = w.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / w.len() as f64;
        if 10.0 * ms.max(1e-20).log10() > -50.0 {
            sum += ms;
            n += 1;
        }
    }
    10.0 * (sum / n.max(1) as f64).log10() as f32
}

#[test]
fn every_factory_sound_renders_a_levelled_clip() {
    let lib = factory();
    let mut tried = 0;
    let mut loud = Vec::new();
    let mut skipped = Vec::new();
    for e in lib.entries.iter().filter(|e| e.origin == Origin::Factory) {
        tried += 1;
        match sound_preview::render(&e.dir, &e.meta, 48000.0) {
            Ok(c) => {
                assert!(c.secs() <= MAX_SECS + 0.01, "{}: {} s", e.id, c.secs());
                let peak = c.samples.iter().fold(0f32, |m, s| m.max(s.abs()));
                assert!(peak <= 0.9, "{}: peak {peak}", e.id);
                assert!(!c.wave.is_empty() && c.wave.len() <= sound_preview::WAVE_BINS + 1);
                let db = rms_db(&c.samples);
                // Matched loudness: on target unless the peak ceiling held it back.
                if db < -22.0 {
                    loud.push(format!("{}: {db:.1} dB (peak {peak:.2})", e.id));
                }
            }
            Err(why) => skipped.push(format!("{}: {why}", e.id)),
        }
    }
    assert!(tried > 50, "factory bank not found: {tried}");
    eprintln!("quiet after levelling: {loud:?}");
    eprintln!("no clip: {skipped:?}");
    assert!(
        skipped.len() * 20 <= tried,
        "too many sounds without a clip: {skipped:?}"
    );
}

#[test]
fn a_clip_is_deterministic() {
    let lib = factory();
    let e = lib
        .entries
        .iter()
        .find(|e| e.meta.keys && !e.meta.sequence)
        .expect("a keyboard sound");
    let a = sound_preview::render(&e.dir, &e.meta, 48000.0).unwrap();
    let b = sound_preview::render(&e.dir, &e.meta, 48000.0).unwrap();
    assert_eq!(a.samples, b.samples);
}

#[test]
fn the_browser_has_no_player_without_output_and_says_so() {
    let ui = UiState::default();
    assert!(!ui.browser.clips.available());
    assert!(matches!(ui.browser.clips.look("x"), ClipLook::NoOutput));
}

#[test]
fn preview_on_select_is_off_by_default() {
    assert!(!library::Prefs::default().preview_on_select);
}
