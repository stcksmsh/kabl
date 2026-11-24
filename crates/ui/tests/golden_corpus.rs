//! Golden render hashes for every patch the Sounds browser lists (patch folders one or two
//! levels under `patches/`; the browser reads the packed `.kabl` files at the top and one folder
//! down, so deeper folders such as `sound-engines/demos/*` are not listed). `golden_corpus.txt`
//! holds one hash per patch; a changed sound, or a patch added or removed, fails the test.
//!
//! The render, at 48 kHz with flush-to-zero on like the audio callbacks:
//! - 6000 blocks (8 s). Three notes (0, 4 and 7 semitones, velocity 0.8) go on at block 0 and off
//!   at block 3000, long enough for sequencers to pass their later steps and wrap.
//! - Cues: every cue of every `cues` module (in module-id order, then cue order) is launched
//!   once, immediately, at block `1000 + 500 * i` for the i-th cue: each of its sequencer or
//!   drum-grid targets is armed on its bank. Cue macro values and morphs are not exercised, and
//!   the cue's launch timing (clock quantisation, `Launch`, cancel) is bypassed.
//!
//! Regenerate deliberately, only when a sound is meant to change:
//! `KABL_WRITE_GOLDEN=1 cargo test -p kabl-ui --test golden_corpus`.

use std::path::Path;

use kabl_core::PatchState;
use kabl_engine::compile::compile;
use kabl_engine::golden::{chord_render, patch_dirs, SampleHash};
use kabl_ui::cues;

const COUNT: usize = 129;
const BLOCKS: usize = 6000;
/// Some corpus patches run past full scale (expressive/lead peaks near 1.06), so the non-Linux
/// check only guards against a blow-up.
#[cfg(not(target_os = "linux"))]
const CORPUS_MAX_PEAK: f32 = 2.0;
/// Patches with at least one cue launch; guards against `cues()` silently returning nothing.
const CUE_PATCHES: usize = 4;

fn hash_text(h: &Option<SampleHash>) -> String {
    h.as_ref()
        .map_or_else(|| "compile-error".into(), SampleHash::hex)
}

/// Cue launches as `(block, sequencer, bank)`, in launch order.
fn cue_launches(state: &PatchState) -> Vec<(usize, kabl_core::ModuleId, usize)> {
    let mut ids: Vec<_> = state
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "cues")
        .map(|(id, _)| *id)
        .collect();
    ids.sort();
    let mut out = Vec::new();
    let mut i = 0;
    for id in ids {
        for cue in cues::cues(state, id) {
            for (seq, bank) in cue.targets {
                out.push((1000 + 500 * i, seq, bank));
            }
            i += 1;
        }
    }
    out
}

/// FNV-1a over the bit patterns of the left and right output (`None` when the patch does not
/// compile), and the number of cue launches.
fn render_hash(dir: &Path) -> (Option<SampleHash>, usize) {
    let _ftz = kabl_engine::ftz::FtzGuard::new();
    let log = kabl_core::load(dir).expect("loads");
    let launches = cue_launches(log.state());
    let Ok(mut c) = compile(log.state(), 48000.0, 8) else {
        return (None, 0);
    };
    (
        Some(chord_render(&mut c, BLOCKS, 3000, |c, b| {
            for &(_, seq, bank) in launches.iter().filter(|l| l.0 == b) {
                c.arm_bank(seq, bank, 0);
            }
        })),
        launches.len(),
    )
}

#[test]
fn browser_corpus_renders_as_it_did() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches");
    let golden_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden_corpus.txt");
    let mut dirs = Vec::new();
    patch_dirs(&root, &mut dirs, 2);
    let mut names: Vec<String> = dirs
        .iter()
        .map(|d| {
            d.strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    if std::env::var_os("KABL_WRITE_GOLDEN").is_some() {
        let mut with_cues = 0;
        let text: String = names
            .iter()
            .map(|n| {
                let (h, l) = render_hash(&root.join(n));
                with_cues += usize::from(l > 0);
                format!("{n} {}\n", hash_text(&h))
            })
            .collect();
        eprintln!("patches with cue launches: {with_cues}");
        std::fs::write(&golden_path, text).unwrap();
        return;
    }
    assert_eq!(names.len(), COUNT, "browser corpus size changed");
    let golden = std::fs::read_to_string(&golden_path).unwrap();
    let recorded: Vec<(&str, &str)> = golden.lines().map(|l| l.split_once(' ').unwrap()).collect();
    assert_eq!(
        recorded.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
        names,
        "golden_corpus.txt and the patches folder list different patches"
    );
    let mut with_cues = 0;
    for (name, want) in recorded {
        let (hash, launches) = render_hash(&root.join(name));
        // Bit-exact hashes are a Linux reference; libm differs in the last bit elsewhere.
        #[cfg(target_os = "linux")]
        assert_eq!(hash_text(&hash), want, "{name} sounds different");
        #[cfg(not(target_os = "linux"))]
        match &hash {
            Some(h) => {
                assert_ne!(want, "compile-error", "{name} now compiles");
                h.assert_clean(name, CORPUS_MAX_PEAK);
            }
            None => assert_eq!(want, "compile-error", "{name} no longer compiles"),
        }
        with_cues += usize::from(launches > 0);
    }
    assert_eq!(with_cues, CUE_PATCHES, "patches with cue launches changed");
}
