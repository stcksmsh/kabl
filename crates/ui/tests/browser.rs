//! The browser workflow through real egui input (`show()`), on a real library with the
//! repository's factory sounds and a temporary user directory: browsing makes no sound,
//! Open / Play / Start, Save As and Save, unsaved-change questions (cancel, discard, save,
//! a failed save), undo back to the saved state, rename, missing recents, bad patches,
//! and every control reachable at both window sizes.

use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect};
use kabl_core::PatchState;
use kabl_engine::patch_engine::Command;
use kabl_modules::builtins::Transport;
use kabl_ui::browser::{is_modified, Dialog, DocOrigin, Pending};
use kabl_ui::library::Library;
use kabl_ui::{show, PatchEditor, UiState};
use std::path::{Path, PathBuf};

fn factory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches")
}

struct H {
    focused: bool,
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    size: egui::Vec2,
    events: Vec<Event>,
    t: f64,
    pointer: Pos2,
    _user: tempfile::TempDir,
}

impl H {
    fn new(w: f32, h: f32) -> Self {
        let user = tempfile::tempdir().unwrap();
        Self::with_user(w, h, user)
    }

    fn with_user(w: f32, h: f32, user: tempfile::TempDir) -> Self {
        let mut ui = UiState::default();
        ui.browser_open = true;
        ui.library = Some(Library::open(Some(factory()), user.path().to_path_buf()));
        let mut h = H {
            focused: true,
            ctx: egui::Context::default(),
            editor: PatchEditor::seed_from(&kabl_standalone::default_patch()),
            ui,
            size: egui::vec2(w, h),
            events: Vec::new(),
            t: 0.0,
            pointer: Pos2::ZERO,
            _user: user,
        };
        h.frame();
        h.frame();
        h
    }

    fn frame(&mut self) {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            events: std::mem::take(&mut self.events),
            time: Some(self.t),
            focused: self.focused,
            ..Default::default()
        };
        self.t += 1.0 / 60.0;
        self.ctx.begin_pass(raw);
        let mut root = egui::Ui::new(
            self.ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::ZERO, self.size)),
        );
        show(&mut self.editor, &mut self.ui, &mut root);
        let _ = self.ctx.end_pass();
    }

    fn has(&self, key: &str) -> bool {
        self.ui.hits.contains_key(key)
    }

    fn click(&mut self, key: &str) {
        let p = self
            .ui
            .hits
            .get(key)
            .unwrap_or_else(|| panic!("no hit target {key}"))
            .center();
        self.pointer = p;
        self.events.push(Event::PointerMoved(p));
        self.frame();
        for pressed in [true, false] {
            self.events.push(Event::PointerButton {
                pos: self.pointer,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
            self.frame();
        }
        self.frame();
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) {
        for pressed in [true, false] {
            self.events.push(Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            });
        }
        self.frame();
        self.frame();
    }

    fn open(&mut self, id: &str) {
        self.click(&format!("sound:{id}"));
        self.click("open");
    }

    fn previews(&self) -> usize {
        self.ui
            .launches
            .iter()
            .filter(|c| matches!(c, Command::Preview { .. }))
            .count()
    }

    fn modified(&self) -> bool {
        is_modified(&self.editor, &self.ui)
    }

    fn doc_origin(&self) -> DocOrigin {
        self.ui.doc.as_ref().unwrap().origin.clone()
    }

    /// Sets the Save As name (typing is egui's job) and presses Save.
    fn save_as_named(&mut self, name: &str) {
        match self.ui.browser.dialog.as_mut() {
            Some(Dialog::SaveAs { name: n, .. }) => *n = name.into(),
            other => panic!("no Save As dialog: {other:?}"),
        }
        self.frame();
        self.click("dlg:save");
    }

    /// An edit on the first knob-like param of module kind `kind`.
    fn edit(&mut self, kind: &str, param: &str, v: f32) {
        let id = *self
            .editor
            .state()
            .modules
            .iter()
            .find(|(_, m)| m.kind == kind)
            .unwrap()
            .0;
        self.editor.set_param(id, param, v);
        self.frame();
    }
}

#[test]
fn browsing_never_makes_sound_or_loads() {
    let mut t = H::new(1440.0, 900.0);
    let before = t.editor.state().clone();
    for id in [
        "factory:palette/pad",
        "factory:palette/bass",
        "factory:composition",
    ] {
        t.click(&format!("sound:{id}"));
        assert_eq!(t.ui.browser.selected.as_deref(), Some(id));
    }
    t.click("filter:favorites");
    t.click("filter:all");
    t.click("fav:factory:palette/lead");
    t.click("search");
    t.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(t.editor.state(), &before, "nothing loaded");
    assert!(!t.ui.loaded);
    assert!(t.ui.launches.is_empty(), "no preview, no command");
    assert!(t.ui.transport.is_empty());
    assert!(t
        .ui
        .library
        .as_ref()
        .unwrap()
        .prefs
        .favorites
        .contains("factory:palette/lead"));
}

#[test]
fn open_a_voice_and_audition_it() {
    let mut t = H::new(1440.0, 900.0);
    t.open("factory:palette/pad");
    assert!(t.ui.loaded && t.editor.take_dirty(), "a fresh load");
    assert!(!t.ui.load_stopped, "voices have no clock to stop");
    assert_eq!(
        t.doc_origin(),
        DocOrigin::Library("factory:palette/pad".into())
    );
    assert_eq!(t.ui.doc.as_ref().unwrap().name, "Evolving Pad");
    assert!(!t.modified());
    assert_eq!(t.previews(), 0, "opening doesn't play");
    assert!(
        t.ui.launches.contains(&Command::PreviewStop),
        "a load ends any preview"
    );
    t.ui.launches.clear();
    assert_eq!(
        t.ui.library.as_ref().unwrap().prefs.recents,
        ["factory:palette/pad"]
    );

    t.ui.browser.audition.chord = kabl_ui::browser::Chord::Major;
    t.ui.browser.audition.secs = 2.0;
    t.frame();
    t.click("play");
    match t.ui.launches.as_slice() {
        [Command::Preview {
            notes,
            count,
            velocity,
            blocks,
        }] => {
            assert_eq!(&notes[..*count as usize], &[60, 64, 67]);
            assert_eq!(*velocity, 90);
            assert_eq!(*blocks, 1500, "2 s at 48 kHz in 64-sample blocks");
        }
        other => panic!("{other:?}"),
    }
    t.click("stop-preview");
    assert_eq!(t.ui.launches.last(), Some(&Command::PreviewStop));
    assert!(!t.modified(), "auditioning is not an edit");
    assert!(!t.has("start"), "no clock, no Start");
}

#[test]
fn a_piece_opens_stopped_and_starts_explicitly() {
    let mut t = H::new(1440.0, 900.0);
    t.open("factory:palette/bass");
    assert!(t.ui.load_stopped, "its clocks start stopped");
    assert!(!t.has("play"), "no keyboard, no Play");
    // The audio thread would report the clock stopped.
    let clocks: Vec<u64> = t
        .editor
        .state()
        .modules
        .iter()
        .filter(|(_, m)| m.kind == "clock")
        .map(|(&id, _)| id)
        .collect();
    for &id in &clocks {
        t.ui.clock_running.insert(id, false);
    }
    t.frame();
    t.click("start");
    assert_eq!(
        t.ui.transport,
        clocks
            .iter()
            .map(|&id| (id, Transport::Run))
            .collect::<Vec<_>>()
    );
    for &id in &clocks {
        t.ui.clock_running.insert(id, true);
    }
    t.ui.transport.clear();
    t.frame();
    t.click("stop");
    assert!(t.ui.transport.iter().all(|&(_, c)| c == Transport::Stop));
    assert!(!t.modified(), "transport is not an edit");
    // A piece with a keyboard has both.
    t.open("factory:composition");
    assert!(t.has("play") && t.has("start"));
}

#[test]
fn edit_save_as_reopen_and_the_factory_file_stays() {
    let mut t = H::new(1440.0, 900.0);
    let factory_log = std::fs::read(factory().join("palette/pad/log.jsonl")).unwrap();
    t.open("factory:palette/pad");
    t.edit("filter.ladder", "cutoff_hz", 4321.0);
    assert!(t.modified());
    // Save on a factory sound asks for a name instead.
    t.click("save");
    assert!(
        matches!(t.ui.browser.dialog, Some(Dialog::SaveAs { ref name, .. }) if name == "My Evolving Pad")
    );
    t.save_as_named("Bright Pad");
    assert!(t.ui.browser.dialog.is_none());
    assert!(!t.modified());
    assert_eq!(t.doc_origin(), DocOrigin::Library("user:bright-pad".into()));
    assert_eq!(
        std::fs::read(factory().join("palette/pad/log.jsonl")).unwrap(),
        factory_log
    );
    let saved = t.editor.state().clone();

    // Away and back: found under Your Sounds with the edit.
    t.open("factory:palette/lead");
    t.click("filter:user");
    t.open("user:bright-pad");
    assert_eq!(t.editor.state(), &saved);
    assert_eq!(t.ui.doc.as_ref().unwrap().name, "Bright Pad");

    // Save (in place) now writes without asking.
    t.edit("filter.ladder", "cutoff_hz", 1000.0);
    t.key(Key::S, Modifiers::COMMAND);
    assert!(t.ui.browser.dialog.is_none() && !t.modified());
    let lib = t.ui.library.as_ref().unwrap();
    assert_eq!(
        lib.read("user:bright-pad").unwrap().state(),
        t.editor.state()
    );
}

#[test]
fn unsaved_edits_ask_and_cancel_keeps_them_exactly() {
    let mut t = H::new(1440.0, 900.0);
    t.open("factory:palette/strings");
    t.edit("chorus", "mix", 0.9);
    let work = t.editor.state().clone();
    let undo = t.editor.log().entries().len();

    for p in ["open", "new"] {
        if p == "open" {
            t.open("factory:palette/lead");
        } else {
            t.click("new");
        }
        assert!(
            matches!(t.ui.browser.dialog, Some(Dialog::Unsaved { .. })),
            "{p} asks"
        );
        t.click("dlg:cancel");
        assert!(t.ui.browser.dialog.is_none());
        assert_eq!(t.editor.state(), &work, "{p}: cancel keeps the work");
        assert_eq!(t.editor.log().entries().len(), undo, "and its undo history");
        assert!(t.modified());
    }
    // Escape cancels too.
    t.open("factory:palette/lead");
    t.key(Key::Escape, Modifiers::NONE);
    assert!(t.ui.browser.dialog.is_none());
    assert_eq!(t.editor.state(), &work);

    // Undo back to the saved state: nothing to ask.
    t.editor.undo();
    t.frame();
    assert!(!t.modified(), "undo to the opened state is clean");
    t.editor.redo();
    t.frame();
    assert!(t.modified());

    // Don't save: the other sound opens.
    t.open("factory:palette/lead");
    t.click("dlg:discard");
    assert_eq!(
        t.doc_origin(),
        DocOrigin::Library("factory:palette/lead".into())
    );
    assert!(!t.modified());

    // New starts from Init Keyboard, untitled and clean.
    t.edit("filter.ladder", "cutoff_hz", 900.0);
    t.click("new");
    t.click("dlg:discard");
    assert_eq!(t.doc_origin(), DocOrigin::New);
    assert!(t
        .editor
        .state()
        .modules
        .values()
        .any(|m| m.kind == "midi.in"));
    assert!(!t.modified());
}

#[test]
fn save_from_the_question_then_continues() {
    let mut t = H::new(1440.0, 900.0);
    t.open("factory:palette/breath");
    t.edit("chorus", "mix", 0.2);
    let work = t.editor.state().clone();
    t.open("factory:palette/pad");
    t.click("dlg:save");
    // A factory sound: Save goes through Save As, then the open continues.
    t.save_as_named("Soft Breath");
    assert_eq!(
        t.doc_origin(),
        DocOrigin::Library("factory:palette/pad".into())
    );
    let lib = t.ui.library.as_ref().unwrap();
    assert_eq!(lib.read("user:soft-breath").unwrap().state(), &work);
}

#[test]
fn quit_with_unsaved_changes_asks() {
    let mut t = H::new(1440.0, 900.0);
    kabl_ui::browser::request(&mut t.editor, &mut t.ui, Pending::Quit);
    assert!(t.ui.quit_now, "nothing unsaved: quits");
    t.ui.quit_now = false;
    t.open("factory:palette/lead");
    t.edit("drive", "drive_db", 3.0);
    kabl_ui::browser::request(&mut t.editor, &mut t.ui, Pending::Quit);
    t.frame();
    assert!(!t.ui.quit_now);
    t.click("dlg:cancel");
    assert!(!t.ui.quit_now);
    kabl_ui::browser::request(&mut t.editor, &mut t.ui, Pending::Quit);
    t.frame();
    t.click("dlg:discard");
    assert!(t.ui.quit_now);
}

#[test]
fn a_failed_save_keeps_the_work_and_the_question() {
    let dir = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir_in(dir.path()).unwrap();
    let mut t = H::with_user(1440.0, 900.0, user);
    t.open("factory:palette/pad");
    t.click("save-as");
    t.save_as_named("Mine");
    assert_eq!(t.doc_origin(), DocOrigin::Library("user:mine".into()));
    t.edit("filter.ladder", "cutoff_hz", 777.0);
    let work = t.editor.state().clone();
    // The sounds directory becomes unusable: a file where the staging dir goes.
    let sounds = t.ui.library.as_ref().unwrap().sounds_dir();
    std::fs::write(sounds.join(".mine.new"), b"x").unwrap();
    t.open("factory:palette/lead");
    t.click("dlg:save");
    match &t.ui.browser.dialog {
        Some(Dialog::Unsaved { error: Some(e), .. }) => {
            assert!(e.contains("Nothing was overwritten"), "{e}")
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(t.editor.state(), &work, "nothing opened, work kept");
    assert_eq!(t.doc_origin(), DocOrigin::Library("user:mine".into()));
    assert!(t.modified());
    t.click("dlg:cancel");
    assert_eq!(t.editor.state(), &work);
}

#[test]
fn save_as_onto_a_taken_name_offers_replace() {
    let mut t = H::new(1440.0, 900.0);
    t.open("factory:palette/pad");
    t.click("save-as");
    t.save_as_named("Twin");
    t.edit("filter.ladder", "cutoff_hz", 555.0);
    let second = t.editor.state().clone();
    t.click("save-as");
    t.save_as_named("twin");
    match &t.ui.browser.dialog {
        Some(Dialog::SaveAs {
            error: Some(e),
            taken: Some((id, _)),
            ..
        }) => {
            assert!(e.contains("already in Your Sounds"), "{e}");
            assert_eq!(id, "user:twin");
        }
        other => panic!("{other:?}"),
    }
    assert!(t.modified(), "not saved yet");
    t.click("dlg:replace");
    assert!(t.ui.browser.dialog.is_none());
    let lib = t.ui.library.as_ref().unwrap();
    assert_eq!(lib.read("user:twin").unwrap().state(), &second);
    assert_eq!(
        lib.entries
            .iter()
            .filter(|e| e.id.starts_with("user:"))
            .count(),
        1,
        "replaced, not duplicated"
    );
}

#[test]
fn rename_through_the_browser() {
    let mut t = H::new(1440.0, 900.0);
    t.open("factory:palette/pad");
    t.click("save-as");
    t.save_as_named("First");
    t.click("save-as");
    t.save_as_named("Second");
    t.click("filter:user");
    t.click("sound:user:first");
    t.click("rename");
    if let Some(Dialog::Rename { name, .. }) = t.ui.browser.dialog.as_mut() {
        *name = "second".into();
    }
    t.frame();
    t.click("dlg:save");
    assert!(
        matches!(
            &t.ui.browser.dialog,
            Some(Dialog::Rename { error: Some(_), .. })
        ),
        "a taken name is refused"
    );
    if let Some(Dialog::Rename { name, .. }) = t.ui.browser.dialog.as_mut() {
        *name = "Renamed".into();
    }
    t.frame();
    t.click("dlg:save");
    assert!(t.ui.browser.dialog.is_none());
    let lib = t.ui.library.as_ref().unwrap();
    assert_eq!(lib.get("user:first").unwrap().meta.name, "Renamed");
    // The open document (Second) keeps its own name.
    assert_eq!(t.ui.doc.as_ref().unwrap().name, "Second");
}

#[test]
fn missing_recents_and_bad_patches_are_explained() {
    let user = tempfile::tempdir().unwrap();
    let sounds = user.path().join("sounds");
    // An uncompilable patch among the user's sounds.
    let mut alien = PatchEditor::new();
    alien.add_module("no.such.module", kabl_core::Vec2::default());
    kabl_core::save(&sounds.join("alien"), alien.log()).unwrap();
    std::fs::write(
        user.path().join("library.json"),
        r#"{"favorites":["user:gone"],"recents":["user:gone","factory:palette/pad"]}"#,
    )
    .unwrap();
    let mut t = H::with_user(1440.0, 900.0, user);
    t.edit("filter.svf", "cutoff_hz", 300.0);
    let work: PatchState = t.editor.state().clone();
    t.click("filter:user");
    t.open("user:alien");
    t.click("dlg:discard");
    let msg = t.ui.last_message.clone().unwrap();
    assert!(
        msg.contains("no.such.module") && msg.contains("unchanged"),
        "{msg}"
    );
    assert_eq!(t.editor.state(), &work, "the working sound survives");
    assert!(t.modified(), "and is still unsaved");

    t.click("filter:recent");
    assert!(t.has("forget:user:gone"));
    assert!(t.has("sound:factory:palette/pad"));
    t.click("forget:user:gone");
    assert!(!t.has("forget:user:gone"));
    let prefs = &t.ui.library.as_ref().unwrap().prefs;
    assert!(!prefs.favorites.contains("user:gone"));
    assert_eq!(prefs.recents, ["factory:palette/pad"]);
}

#[test]
fn every_control_is_reachable_at_both_sizes() {
    for (w, h) in [(1440.0, 900.0), (1280.0, 800.0)] {
        let mut t = H::new(w, h);
        t.ui.perform_open = true;
        t.open("factory:composition");
        let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(w, h));
        for key in [
            "browser",
            "save",
            "save-as",
            "doc-name",
            "search",
            "new",
            "open",
            "play",
            "start",
            "category",
            "routing",
            "perform",
            "folder-header",
        ] {
            let r = t.ui.hits.get(key).unwrap_or_else(|| panic!("{w}: {key}"));
            assert!(
                screen.contains_rect(*r),
                "{w}×{h}: {key} at {r:?} off screen"
            );
        }
        // Toolbar items don't overlap.
        let row = [
            "browser", "doc-name", "save", "save-as", "routing", "perform",
        ];
        for (i, a) in row.iter().enumerate() {
            for b in &row[i + 1..] {
                assert!(
                    !t.ui.hits[*a].intersects(t.ui.hits[*b]),
                    "{w}: {a} overlaps {b}"
                );
            }
        }
    }
}

#[test]
fn losing_window_focus_stops_a_preview() {
    let mut t = H::new(1440.0, 900.0);
    t.open("factory:init-keyboard");
    t.click("play");
    t.ui.launches.clear();
    t.focused = false;
    t.frame();
    assert_eq!(t.ui.launches, [Command::PreviewStop]);
    t.focused = true;
    t.frame();
    t.frame();
    assert_eq!(t.ui.launches.len(), 1, "once");
    // Without a preview running, focus changes send nothing.
    t.ui.launches.clear();
    t.focused = false;
    t.frame();
    assert!(t.ui.launches.is_empty());
}

// ---- D01-R1 repairs (docs/find-play-save/repair-1) ----

fn saved_state(t: &H, id: &str) -> PatchState {
    t.ui.library
        .as_ref()
        .unwrap()
        .read(id)
        .unwrap()
        .state()
        .clone()
}

fn user_ids(t: &H) -> Vec<String> {
    t.ui.library
        .as_ref()
        .unwrap()
        .entries
        .iter()
        .filter(|e| e.id.starts_with("user:"))
        .map(|e| e.id.clone())
        .collect()
}

/// R1: after a collision, changing the name must not keep the old replacement target.
#[test]
fn replace_never_targets_a_sound_after_the_name_changed() {
    let mut t = H::new(1440.0, 900.0);
    t.open("factory:palette/pad");
    t.click("save-as");
    t.save_as_named("Alpha");
    let alpha = saved_state(&t, "user:alpha");
    t.edit("filter.ladder", "cutoff_hz", 1234.0);
    let work = t.editor.state().clone();
    t.click("save-as");
    t.save_as_named("alpha");
    assert!(t.has("dlg:replace"), "a collision offers Replace it");
    // The user types a new, unused name.
    if let Some(Dialog::SaveAs { name, .. }) = t.ui.browser.dialog.as_mut() {
        *name = "Beta".into();
    }
    t.frame();
    assert!(
        !t.has("dlg:replace"),
        "a new name drops the old confirmation"
    );
    assert!(
        matches!(
            &t.ui.browser.dialog,
            Some(Dialog::SaveAs {
                taken: None,
                error: None,
                ..
            })
        ),
        "and its message"
    );
    t.click("dlg:save");
    let lib = t.ui.library.as_ref().unwrap();
    assert!(
        saved_state(&t, "user:alpha") == alpha
            && lib.get("user:alpha").unwrap().meta.name == "Alpha",
        "Alpha was replaced: it is now named {:?}, and user sounds are {:?}",
        lib.get("user:alpha").unwrap().meta.name,
        user_ids(&t)
    );
    assert_eq!(
        lib.get("user:beta").map(|e| e.meta.name.as_str()),
        Some("Beta")
    );
    assert_eq!(saved_state(&t, "user:beta"), work);
    assert_eq!(t.doc_origin(), DocOrigin::Library("user:beta".into()));
    assert_eq!(user_ids(&t).len(), 2);
}

fn folder_doc(t: &mut H, dir: &Path) {
    t.ui.doc = Some(kabl_ui::browser::Doc::new(
        "folder",
        DocOrigin::Folder(dir.display().to_string()),
        t.editor.state(),
    ));
}

/// R2: a folder save that fails part way leaves the folder as it was and says so. At the
/// base (58e6622) this was reproduced with `checkpoint.json` made a directory, which failed
/// the old direct writer after it had rewritten `log.jsonl` (evidence/repro-before.txt); the
/// staged save moves such a directory aside, so the failure is injected here instead: after
/// the new `log.jsonl` is in place, the stage the old writer failed after.
#[test]
fn a_failed_folder_save_leaves_the_saved_folder_unchanged() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("song");
    let mut t = H::new(1440.0, 900.0);
    kabl_core::save(&dir, t.editor.log()).unwrap();
    std::fs::write(dir.join("notes.txt"), "mine").unwrap();
    let files = |d: &Path| -> Vec<(String, Vec<u8>)> {
        let mut v: Vec<_> = std::fs::read_dir(d)
            .unwrap()
            .flatten()
            .map(|e| {
                (
                    e.file_name().to_string_lossy().to_string(),
                    std::fs::read(e.path()).unwrap(),
                )
            })
            .collect();
        v.sort();
        v
    };
    let before = files(&dir);
    folder_doc(&mut t, &dir);
    t.edit("filter.svf", "cutoff_hz", 321.0);
    let work = t.editor.state().clone();
    kabl_ui::library::fail_saves_at(Some("replace:log.jsonl"));
    t.click("save");
    let msg = t.ui.last_message.clone().unwrap();
    assert!(
        msg.starts_with("save failed") && msg.contains("Nothing was overwritten"),
        "{msg}"
    );
    assert!(
        files(&dir) == before,
        "folder changed though the message says: {msg}"
    );
    assert_eq!(t.editor.state(), &work);
    assert!(t.modified(), "still unsaved");

    // Save-then-Open doesn't continue after the failure.
    t.open("factory:palette/pad");
    t.click("dlg:save");
    assert!(
        matches!(
            &t.ui.browser.dialog,
            Some(Dialog::Unsaved { error: Some(_), .. })
        ),
        "the question stays with the reason"
    );
    assert_eq!(t.editor.state(), &work, "nothing opened");

    // Once saving works again, Save writes the folder and the open continues.
    kabl_ui::library::fail_saves_at(None);
    // The pointer leaves and comes back: a second press on the same spot right away is a
    // double click to egui.
    t.events.push(Event::PointerMoved(Pos2::new(5.0, 5.0)));
    t.frame();
    t.click("dlg:save");
    assert_eq!(
        t.doc_origin(),
        DocOrigin::Library("factory:palette/pad".into())
    );
    assert_eq!(kabl_core::load(&dir).unwrap().state(), &work);
    assert_eq!(std::fs::read(dir.join("notes.txt")).unwrap(), b"mine");
    assert!(!dir.join(".kabl-save").exists());
}

/// R2: Save to folder (the advanced section) goes through the same staged save.
#[test]
fn save_to_folder_is_staged_too() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("other");
    let mut t = H::new(1440.0, 900.0);
    kabl_core::save(&dir, t.editor.log()).unwrap();
    let log_before = std::fs::read(dir.join("log.jsonl")).unwrap();
    t.edit("filter.svf", "cutoff_hz", 222.0);
    t.ui.browser.folder = dir.display().to_string();
    t.click("folder-header");
    kabl_ui::library::fail_saves_at(Some("replace:meta.toml"));
    t.click("save-folder");
    let msg = t.ui.last_message.clone().unwrap();
    assert!(msg.contains("Nothing was overwritten"), "{msg}");
    assert_eq!(std::fs::read(dir.join("log.jsonl")).unwrap(), log_before);
    kabl_ui::library::fail_saves_at(None);
    t.click("save-folder");
    assert_eq!(kabl_core::load(&dir).unwrap().state(), t.editor.state());
    assert!(!t.modified(), "the folder is now the document");
}

/// R1: Cancel after a collision writes nothing.
#[test]
fn cancel_after_a_collision_keeps_the_other_sound() {
    let mut t = H::new(1440.0, 900.0);
    t.open("factory:palette/pad");
    t.click("save-as");
    t.save_as_named("Alpha");
    let dir =
        t.ui.library
            .as_ref()
            .unwrap()
            .get("user:alpha")
            .unwrap()
            .dir
            .clone();
    let bytes = |d: &Path| -> Vec<Vec<u8>> {
        let mut v: Vec<_> = std::fs::read_dir(d)
            .unwrap()
            .flatten()
            .map(|e| std::fs::read(e.path()).unwrap())
            .collect();
        v.sort();
        v
    };
    let before = bytes(&dir);
    t.edit("filter.ladder", "cutoff_hz", 999.0);
    let work = t.editor.state().clone();
    t.click("save-as");
    t.save_as_named("ALPHA");
    assert!(t.has("dlg:replace"));
    t.click("dlg:cancel");
    assert!(t.ui.browser.dialog.is_none());
    assert_eq!(bytes(&dir), before);
    assert_eq!(user_ids(&t), ["user:alpha"]);
    assert_eq!(t.editor.state(), &work);
    assert!(t.modified());
}
