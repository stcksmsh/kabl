//! D11 PR 5: the example code modules through the UI layer: labelled composites with valid
//! panels, offline audio through the real engine, save and reopen, and the code notice.

use kabl_core::{PortRef, Vec2};
use kabl_engine::graph::BLOCK;
use kabl_engine::keyboard::KeyEvent;
use kabl_engine::patch_engine::PatchEngine;
use kabl_modules::registry;
use kabl_ui::browser::{self, Dialog, Pending};
use kabl_ui::library::{self, Library};
use kabl_ui::{code_examples, code_notice, PatchEditor, UiState};

const SR: f32 = 48_000.0;

fn editor() -> PatchEditor {
    PatchEditor::seed_from(&kabl_standalone::default_patch())
}

fn first_port(kind: &str, dir: kabl_modules::PortDirection) -> String {
    registry::info_for(kind)
        .unwrap()
        .ports
        .iter()
        .find(|p| p.direction == dir)
        .unwrap()
        .name
        .to_string()
}

fn module_of(e: &PatchEditor, kind: &str) -> u64 {
    *e.state()
        .modules
        .iter()
        .find(|(_, m)| m.kind == kind)
        .unwrap()
        .0
}

/// Default patch with `demo.bitfilter` fed by the oscillator and summed into the output.
fn with_filter() -> PatchEditor {
    let mut e = editor();
    let (leaf, _) = code_examples::add(&mut e, "demo.bitfilter", Vec2 { x: 500.0, y: 300.0 })
        .expect("example adds");
    let osc = module_of(&e, "osc.va");
    let out = module_of(&e, "out");
    e.connect(
        PortRef::Module {
            id: osc,
            port: first_port("osc.va", kabl_modules::PortDirection::Output),
        },
        PortRef::Module {
            id: leaf,
            port: "in".into(),
        },
    );
    e.connect(
        PortRef::Module {
            id: leaf,
            port: "out".into(),
        },
        PortRef::Module {
            id: out,
            port: first_port("out", kabl_modules::PortDirection::Input),
        },
    );
    e
}

fn render(e: &PatchEditor, secs: f32) -> Vec<f32> {
    let collector = basedrop::Collector::new();
    let mut eng = PatchEngine::new(&collector.handle(), e.state(), SR, 4).expect("builds");
    eng.key(KeyEvent::On {
        note: 60,
        velocity: 100,
    });
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    let mut out = Vec::new();
    for _ in 0..(secs * SR) as usize / BLOCK {
        eng.process_block(&mut l, &mut r);
        out.extend_from_slice(&l);
        out.extend_from_slice(&r);
    }
    out
}

#[test]
fn examples_get_labels_and_a_valid_panel() {
    for kind in ["demo.logic", "demo.bitfilter"] {
        let mut e = editor();
        let (leaf, id) = code_examples::add(&mut e, kind, Vec2 { x: 500.0, y: 300.0 }).unwrap();
        kabl_core::composite::validate(e.state()).unwrap();
        let c = &e.state().composites[&id];
        assert_eq!(c.members.iter().copied().collect::<Vec<_>>(), [leaf]);
        assert_eq!(c.definition.id, kind);
        let panel = c.panel.as_ref().expect("has a panel");
        panel.validate(c).unwrap();
        for ex in c.ports.values().chain(c.controls.values()) {
            assert!(!ex.label.contains('·') && !ex.label.contains('#'), "{ex:?}");
            assert!(ex.label.chars().next().unwrap().is_uppercase(), "{ex:?}");
        }
        // Every port and param of the module is exposed and placed.
        let info = registry::info_for(kind).unwrap();
        assert_eq!(c.ports.len(), info.ports.len());
        assert_eq!(c.controls.len(), info.params.len());
        assert_eq!(panel.placements.len(), info.ports.len() + info.params.len());
    }
}

#[test]
fn bitfilter_makes_finite_non_silent_audio_and_survives_a_swap() {
    let mut e = with_filter();
    let a = render(&e, 0.5);
    assert!(a.iter().all(|v| v.is_finite()));
    assert!(a.iter().any(|v| v.abs() > 1e-3), "silent");
    assert_ne!(
        a,
        render(&editor(), 0.5),
        "the module is in the signal path"
    );
    // Turning the cutoff right down changes the sound but stays finite.
    let leaf = module_of(&e, "demo.bitfilter");
    e.set_param(leaf, "cutoff", 120.0);
    let b = render(&e, 0.5);
    assert!(b.iter().all(|v| v.is_finite()));
    assert!(b.iter().map(|v| v * v).sum::<f32>() < a.iter().map(|v| v * v).sum::<f32>());
}

#[test]
fn logic_module_builds_inside_a_patch() {
    let mut e = editor();
    let (leaf, _) = code_examples::add(&mut e, "demo.logic", Vec2 { x: 500.0, y: 300.0 }).unwrap();
    // Output offset 0.5 in "threshold" mode with nothing patched still gives a finite render.
    e.set_param(leaf, "offset", 0.5);
    let out = render(&e, 0.2);
    assert!(out.iter().all(|v| v.is_finite()));
    assert!(out.iter().any(|v| v.abs() > 1e-3));
}

#[test]
fn save_and_reopen_round_trips_code_and_asks_once() {
    let e = with_filter();
    let before = render(&e, 0.3);
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("filtered.kabl");
    library::save_patch(&file, e.log()).unwrap();

    // The file carries the code, bound to the instance.
    let (_, _, code) = kabl_core::format::load_kabl_full(&file).unwrap();
    let entries = code_notice::entries(&code);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].module_id, "demo.bitfilter");

    // Opening asks: no answer remembered yet.
    let mut lib = Library::open(None, dir.path().join("user"));
    let err = library::read_patch(&file, &lib.prefs.code_trust).unwrap_err();
    assert!(matches!(err, library::LibError::CodeNotice(ref c) if *c == entries));

    // Open once; the same file (and a copy of it) never asks again, in this and a new session.
    assert!(lib.remember_code_trust(&entries, true).is_none());
    let log = library::read_patch(&file, &lib.prefs.code_trust).unwrap();
    assert_eq!(log.state(), e.state());
    let again = Library::open(None, dir.path().join("user"));
    let copy = dir.path().join("copy.kabl");
    std::fs::copy(&file, &copy).unwrap();
    let log2 = library::read_patch(&copy, &again.prefs.code_trust).unwrap();

    // Same audio after the round trip.
    let after = render(&PatchEditor::from_log(log2), 0.3);
    assert_eq!(before, after);
}

#[test]
fn opening_through_the_browser_shows_the_notice_then_never_again() {
    let e = with_filter();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("p.kabl");
    library::save_patch(&file, e.log()).unwrap();
    let path = file.to_string_lossy().into_owned();

    let mut ed = editor();
    let mut ui = UiState::default();
    ui.library = Some(Library::open(None, dir.path().join("user")));

    browser::perform(&mut ed, &mut ui, Pending::OpenFolder(path.clone()));
    let Some(Dialog::CodeNotice { entries, then }) = ui.browser.dialog.take() else {
        panic!("expected the code notice");
    };
    assert_eq!(then, Pending::OpenFolder(path.clone()));
    assert_ne!(ed.state(), e.state(), "nothing opened before the answer");

    // Cancel: remembered. The patch still opens (its code module is a stand-in), the warning
    // says why, and the answer is cleared so the next attempt asks again.
    ui.library
        .as_mut()
        .unwrap()
        .remember_code_trust(&entries, false);
    browser::perform(&mut ed, &mut ui, Pending::OpenFolder(path.clone()));
    assert!(ui.browser.dialog.is_none());
    assert_eq!(ed.state(), e.state(), "settings kept");
    assert!(ui.last_message.as_deref().unwrap().contains("declined"));
    let mut ed = editor();

    // The next attempt asks again; Open is remembered and the patch opens.
    browser::perform(&mut ed, &mut ui, Pending::OpenFolder(path.clone()));
    assert!(matches!(
        ui.browser.dialog.take(),
        Some(Dialog::CodeNotice { .. })
    ));
    ui.library
        .as_mut()
        .unwrap()
        .remember_code_trust(&entries, true);
    browser::perform(&mut ed, &mut ui, then);
    assert!(ui.browser.dialog.is_none());
    assert_eq!(ed.state(), e.state());

    // Opening it again does not ask.
    browser::perform(&mut ed, &mut ui, Pending::OpenFolder(path));
    assert!(ui.browser.dialog.is_none());
}
