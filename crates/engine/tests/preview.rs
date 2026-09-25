//! Audition previews (docs/find-play-save/README.md, "Preview and controller notes"): a
//! preview goes through the same keyboards as the controller, releases itself after its
//! block count, and never releases a key the controller still holds (or the reverse).
//! Sustain, All Notes Off and Load (a fresh graph) keep their meaning.

use assert_no_alloc::{assert_no_alloc, AllocDisabler};
use basedrop::{Collector, Owned};
use kabl_core::{CableState, ModuleId, ModuleState, PatchState, PortRef, Vec2};
use kabl_engine::compile::compile;
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::{Command, PatchEngine, MAX_PREVIEW_SECS};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const SR: f32 = 48000.0;
const VOICES: usize = 8;

fn port(id: ModuleId, p: &str) -> PortRef {
    PortRef::Module { id, port: p.into() }
}

/// `midi.in` gate to the output: the right channel reads (voices sounding) / VOICES.
fn probe(mode: f32) -> PatchState {
    let mut p = PatchState::new();
    let mut params = std::collections::BTreeMap::new();
    params.insert("mode".to_string(), mode);
    p.modules.insert(
        1,
        ModuleState {
            kind: "midi.in".into(),
            pos: Vec2::default(),
            params,
        },
    );
    p.modules.insert(
        2,
        ModuleState {
            kind: "out".into(),
            pos: Vec2::default(),
            params: Default::default(),
        },
    );
    p.cables.insert(
        1,
        CableState {
            from: port(1, "gate"),
            to: port(2, "right"),
            params: Default::default(),
            steps: vec![],
        },
    );
    p
}

fn on(note: u8) -> KeyEvent {
    KeyEvent::On {
        note,
        velocity: 100,
    }
}

fn sounding(e: &mut PatchEngine) -> usize {
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    e.process_block(&mut l, &mut r);
    (r[BLOCK - 1] * VOICES as f32).round() as usize
}

fn engine(c: &Collector, mode: f32) -> PatchEngine {
    PatchEngine::new(&c.handle(), &probe(mode), SR, VOICES).unwrap()
}

fn chord(notes: &[u8], blocks: u32) -> Command {
    let mut n = [0u8; 4];
    n[..notes.len()].copy_from_slice(notes);
    Command::Preview {
        notes: n,
        count: notes.len() as u8,
        velocity: 90,
        blocks,
    }
}

#[test]
fn a_preview_releases_itself_after_its_duration() {
    let c = Collector::new();
    let mut e = engine(&c, 0.0);
    e.command(&chord(&[60, 64, 67], 10));
    for _ in 0..9 {
        assert_eq!(sounding(&mut e), 3);
    }
    assert_eq!(sounding(&mut e), 0, "released on its tenth block");
    assert_eq!(e.preview_keys(), 0);
    assert_eq!(e.keys(), (0, 0));
}

#[test]
fn a_preview_is_capped_whatever_the_ui_asks() {
    let c = Collector::new();
    let mut e = engine(&c, 0.0);
    e.command(&chord(&[60], u32::MAX));
    let cap = (MAX_PREVIEW_SECS * SR / BLOCK as f32) as usize;
    for _ in 0..cap - 1 {
        sounding(&mut e);
    }
    assert_eq!(sounding(&mut e), 0);
}

#[test]
fn stop_and_a_new_preview_release_the_old_one() {
    let c = Collector::new();
    let mut e = engine(&c, 0.0);
    e.command(&chord(&[60, 64], 1000));
    assert_eq!(sounding(&mut e), 2);
    e.command(&chord(&[62], 1000));
    assert_eq!(sounding(&mut e), 1, "repeated previews never pile up");
    e.command(&Command::PreviewStop);
    assert_eq!(sounding(&mut e), 0);
    // Stop with nothing playing is harmless.
    e.command(&Command::PreviewStop);
    assert_eq!(sounding(&mut e), 0);
}

#[test]
fn a_preview_never_releases_a_controller_key() {
    let c = Collector::new();
    let mut e = engine(&c, 0.0);
    e.key(on(60));
    e.command(&chord(&[60, 64, 67], 5));
    assert_eq!(sounding(&mut e), 3, "60 shares its voice");
    for _ in 0..5 {
        sounding(&mut e);
    }
    assert_eq!(sounding(&mut e), 1, "the controller's 60 keeps sounding");
    e.command(&Command::PreviewStop);
    assert_eq!(sounding(&mut e), 1);
    e.key(KeyEvent::Off { note: 60 });
    assert_eq!(sounding(&mut e), 0);
}

#[test]
fn a_controller_release_never_cuts_a_preview_key() {
    let c = Collector::new();
    let mut e = engine(&c, 0.0);
    e.command(&chord(&[60], 20));
    e.key(on(60));
    e.key(KeyEvent::Off { note: 60 });
    assert_eq!(sounding(&mut e), 1, "the preview still holds 60");
    e.key(on(62));
    e.key(KeyEvent::Off { note: 62 });
    assert_eq!(sounding(&mut e), 1, "other keys behave as always");
    for _ in 0..20 {
        sounding(&mut e);
    }
    assert_eq!(sounding(&mut e), 0);
}

#[test]
fn the_pedal_holds_a_released_preview_like_any_key() {
    let c = Collector::new();
    let mut e = engine(&c, 0.0);
    e.key(KeyEvent::Sustain(true));
    e.command(&chord(&[60], 2));
    for _ in 0..4 {
        assert_eq!(sounding(&mut e), 1);
    }
    e.key(KeyEvent::Sustain(false));
    assert_eq!(sounding(&mut e), 0);
}

#[test]
fn all_notes_off_ends_a_preview() {
    let c = Collector::new();
    let mut e = engine(&c, 0.0);
    e.command(&chord(&[60, 64], 1000));
    e.key(on(70));
    e.key(KeyEvent::AllOff);
    assert_eq!(sounding(&mut e), 0);
    assert_eq!(e.preview_keys(), 0);
}

#[test]
fn mono_preview_and_controller_share_the_held_list() {
    let c = Collector::new();
    let mut e = engine(&c, 1.0);
    e.key(on(60));
    e.command(&chord(&[67], 3));
    assert_eq!(sounding(&mut e), 1);
    for _ in 0..3 {
        sounding(&mut e);
    }
    // Mono returns to the controller's held 60.
    assert_eq!(sounding(&mut e), 1);
    e.key(KeyEvent::Off { note: 60 });
    assert_eq!(sounding(&mut e), 0);
}

#[test]
fn load_starts_over_and_the_next_preview_works() {
    let c = Collector::new();
    let mut e = engine(&c, 0.0);
    e.key(on(60));
    e.command(&chord(&[60, 64], 1000));
    assert_eq!(sounding(&mut e), 2);
    let mut g = compile(&probe(0.0), SR, VOICES).unwrap();
    g.fresh = true;
    e.receive_swap(Owned::new(&c.handle(), g));
    for _ in 0..40 {
        sounding(&mut e);
    }
    assert_eq!(sounding(&mut e), 0, "Load releases everything");
    assert_eq!(e.preview_keys(), 0);
    // The controller's 60 was forgotten: a preview of 60 now releases normally.
    e.command(&chord(&[60], 3));
    assert_eq!(sounding(&mut e), 1);
    for _ in 0..3 {
        sounding(&mut e);
    }
    assert_eq!(sounding(&mut e), 0, "no stuck note after Load");
}

#[test]
fn preview_does_not_allocate() {
    let c = Collector::new();
    let mut e = engine(&c, 0.0);
    sounding(&mut e);
    let start = chord(&[60, 64, 67, 71], 3);
    assert_no_alloc(|| {
        e.command(&start);
        e.key(on(60));
        for _ in 0..4 {
            sounding(&mut e);
        }
        e.command(&Command::PreviewStop);
        e.key(KeyEvent::Off { note: 60 });
        sounding(&mut e);
    });
}
