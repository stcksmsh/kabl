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

/// The example modules live in one process-wide registry, and the declined-code test turns them
/// into stand-ins for a while: tests that use them must not overlap.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

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
    let _serial = serial();
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
    let _serial = serial();
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
    let _serial = serial();
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
    let _serial = serial();
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
    let _serial = serial();
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

/// D12: a code module's own saved settings go through the real save and open path. The graph
/// runs on an audio-like thread; `save_patch` asks it for the settings (as the app's Save does);
/// reopening through the library restores them into the new graph. Nothing is kept by hand.
#[test]
fn module_settings_survive_save_and_reopen_through_the_real_path() {
    use kabl_modules::code::host;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let _serial = serial();
    let e = with_filter();
    let leaf = module_of(&e, "demo.bitfilter");
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::open(None, dir.path().join("user"));

    // 1. A song with the module and no settings yet, opened the way the browser opens it.
    let first = dir.path().join("first.kabl");
    library::save_patch(&first, e.log()).unwrap();
    let (_, _, code) = kabl_core::format::load_kabl_full(&first).unwrap();
    assert!(lib
        .remember_code_trust(&code_notice::entries(&code), true)
        .is_none());
    let log = library::read_patch(&first, &lib.prefs.code_trust).unwrap();

    // 2. Its graph runs (as the app's live compile does, with the open song's scope) and the
    //    module's state changes: the filter memory moves with the audio.
    let scope = host::active_scope();
    let stop = Arc::new(AtomicBool::new(false));
    let runner = {
        let (state, scope, stop) = (log.state().clone(), scope.clone(), Arc::clone(&stop));
        std::thread::spawn(move || {
            let collector = basedrop::Collector::new();
            let mut eng =
                PatchEngine::new_scoped(&collector.handle(), &state, SR, 4, &scope).unwrap();
            eng.key(KeyEvent::On {
                note: 60,
                velocity: 100,
            });
            let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
            while !stop.load(Ordering::Relaxed) {
                eng.process_block(&mut l, &mut r);
                std::thread::sleep(std::time::Duration::from_micros(200));
            }
        })
    };
    std::thread::sleep(std::time::Duration::from_millis(100));

    // 3. Save: the real call asks the running graph for the module's settings.
    let second = dir.path().join("second.kabl");
    library::save_patch(&second, &log).unwrap();
    stop.store(true, Ordering::Relaxed);
    runner.join().unwrap();
    assert!(scope.take_notes().is_empty(), "no warning on a normal save");

    let (_, _, saved_code) = kabl_core::format::load_kabl_full(&second).unwrap();
    let version = saved_code.instance_blob(leaf).unwrap().version;
    let kept = saved_code
        .kept_state(leaf, version)
        .expect("the file carries the module's settings");
    let saved = kabl_modules::code::state::SavedState::from_kept(kept).unwrap();
    let mut fresh = {
        use kabl_modules::code::{InstanceConfig, WasmEngine, WasmiEngine};
        let c = WasmiEngine::new().compile(host::EXAMPLES[1].wasm).unwrap();
        c.instantiate(&InstanceConfig::new(SR, 2048)).unwrap()
    };
    let defaults = kabl_modules::code::state::save(&mut *fresh)
        .unwrap()
        .unwrap();
    assert_ne!(saved, defaults, "the audio changed the module's state");

    // 4. Reopen through the library: the new graph's first block loads the file's settings, and
    //    saving them again gives the same bytes.
    let reopened = library::read_patch(&second, &lib.prefs.code_trust).unwrap();
    let scope2 = host::active_scope();
    let collector = basedrop::Collector::new();
    let mut eng =
        PatchEngine::new_scoped(&collector.handle(), reopened.state(), SR, 4, &scope2).unwrap();
    eng.key(KeyEvent::On {
        note: 60,
        velocity: 100,
    });
    scope2.request_capture();
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    eng.process_block(&mut l, &mut r);
    assert_eq!(scope2.collect(), 1);
    let back: Vec<_> = scope2.settings();
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].1, saved, "settings are bit-exact after reopen");
}

/// Writes a file whose module has `kept` as its saved settings (the fixture for `--patch`).
fn file_with_settings(
    dir: &std::path::Path,
    e: &PatchEditor,
    leaf: u64,
    kept: &kabl_modules::code::state::SavedState,
) -> std::path::PathBuf {
    use kabl_modules::code::host;
    let mut code = kabl_standalone::code_load::embedded_for(
        e.state(),
        &host::active_scope(),
        std::time::Duration::ZERO,
    );
    let version = code.instance_blob(leaf).unwrap().version;
    code.keep_state(leaf, version, kept.to_kept()).unwrap();
    let file = dir.join("settings.kabl");
    kabl_core::format::save_kabl_full(&file, e.log(), None, Some(&code)).unwrap();
    file
}

fn good_settings() -> kabl_modules::code::state::SavedState {
    use kabl_modules::code::{host, InstanceConfig, WasmEngine, WasmiEngine};
    let c = WasmiEngine::new().compile(host::EXAMPLES[1].wasm).unwrap();
    let inst = c.instantiate(&InstanceConfig::new(SR, 2048)).unwrap();
    let x = inst.state_xfer().unwrap();
    // eight f32, none the default (zeros)
    let bytes = (1..=8u32)
        .flat_map(|i| (i as f32 * 0.125).to_le_bytes())
        .collect();
    kabl_modules::code::state::SavedState {
        state_version: x.state_version,
        bytes,
    }
}

/// `--patch` (standalone and kabl-ui) registers the file's code and then builds the engine
/// with `PatchEngine::new`: the song's settings reach the module bit-exact.
#[test]
fn patch_flag_restores_saved_module_settings() {
    use kabl_modules::code::host;
    let _serial = serial();
    let e = with_filter();
    let leaf = module_of(&e, "demo.bitfilter");
    let dir = tempfile::tempdir().unwrap();
    let kept = good_settings();
    let file = file_with_settings(dir.path(), &e, leaf, &kept);

    let (log, _, code) = kabl_core::format::load_kabl_full(&file).unwrap();
    assert!(kabl_standalone::code_load::register_for_cli(&code, log.state()).is_empty());
    let collector = basedrop::Collector::new();
    let mut eng = PatchEngine::new(&collector.handle(), log.state(), SR, 4).unwrap();
    eng.key(KeyEvent::On {
        note: 60,
        velocity: 100,
    });
    let scope = host::active_scope();
    scope.request_capture();
    let (mut l, mut r) = ([0f32; BLOCK], [0f32; BLOCK]);
    eng.process_block(&mut l, &mut r);
    let got = scope.collect();
    assert_eq!(got, 1, "notes: {:?}", scope.take_notes());
    assert_eq!(scope.settings(), vec![(leaf, kept)]);
    assert!(scope.take_notes().is_empty());
}

/// Settings the module cannot take are reported the way stand-in warnings are, and the song
/// keeps them for a build that can.
#[test]
fn unusable_saved_settings_are_reported_and_kept() {
    use kabl_modules::code::host;
    use kabl_modules::code::state::SavedState;
    use kabl_ui::control::{self, Delivery};
    let _serial = serial();
    let e = with_filter();
    let leaf = module_of(&e, "demo.bitfilter");
    let dir = tempfile::tempdir().unwrap();
    let corrupt = SavedState {
        state_version: 99, // from a layout newer than the module knows
        bytes: vec![1, 2, 3],
    };
    let file = file_with_settings(dir.path(), &e, leaf, &corrupt);

    let (log, _, code) = kabl_core::format::load_kabl_full(&file).unwrap();
    kabl_standalone::code_load::register_for_cli(&code, log.state());
    let collector = basedrop::Collector::new();
    let mut d = Delivery::new(
        None,
        collector.handle(),
        std::sync::Arc::new(kabl_engine::runtime::Feedback::default()),
        SR,
        4,
        None,
    );
    let mut editor = PatchEditor::from_log(log);
    let mut ui = UiState::default();
    editor.mark_dirty();
    control::deliver(&mut editor, &mut ui, &mut d);
    let msg = ui.last_message.clone().expect("the user is told");
    assert!(
        msg.contains("demo.bitfilter") && msg.contains("starts at its defaults"),
        "{msg}"
    );
    assert!(msg.contains("keeps the old settings"), "{msg}");
    // Saving writes the unusable settings back unchanged.
    let out = kabl_standalone::code_load::embedded_for(
        editor.state(),
        &host::active_scope(),
        std::time::Duration::ZERO,
    );
    let version = out.instance_blob(leaf).unwrap().version;
    assert_eq!(
        SavedState::from_kept(out.kept_state(leaf, version).unwrap()),
        Some(corrupt)
    );
}
