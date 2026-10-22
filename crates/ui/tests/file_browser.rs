//! The shared file browser through real egui input: the filter of each use (images, wav,
//! packages, folders only), folder and save modes with the overwrite question, an unreadable
//! folder, and the folders remembered across runs in `settings.toml`.

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect};
use kabl_ui::filebrowser::{self, Mode, Place, Purpose};
use kabl_ui::{show, PatchEditor, UiState};
use std::path::{Path, PathBuf};

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    events: Vec<Event>,
    t: f64,
    pointer: Pos2,
}

impl H {
    fn new() -> Self {
        let mut h = H {
            ctx: egui::Context::default(),
            editor: PatchEditor::new(),
            ui: UiState::default(),
            events: Vec::new(),
            t: 0.0,
            pointer: Pos2::ZERO,
        };
        h.settle();
        h
    }

    fn frame(&mut self) {
        let size = egui::vec2(1440.0, 900.0);
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            events: std::mem::take(&mut self.events),
            time: Some(self.t),
            ..Default::default()
        };
        self.t += 1.0 / 60.0;
        self.ctx.begin_pass(raw);
        let mut root = egui::Ui::new(
            self.ctx.clone(),
            egui::Id::new("root"),
            egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::ZERO, size)),
        );
        show(&mut self.editor, &mut self.ui, &mut root);
        let _ = self.ctx.end_pass();
    }

    fn settle(&mut self) {
        for _ in 0..4 {
            self.frame();
        }
    }

    fn has(&self, key: &str) -> bool {
        self.ui.hits.contains_key(key)
    }

    /// Frames until the listing thread has delivered `key`.
    fn wait_for(&mut self, key: &str) {
        for _ in 0..200 {
            self.settle();
            if self.has(key) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("no {key} in the browser");
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
                pos: p,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
            self.frame();
        }
        self.settle();
    }

    fn type_into(&mut self, key: &str, text: &str) {
        self.click(key);
        self.events.push(Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        });
        self.frame();
        self.events.push(Event::Text(text.into()));
        self.settle();
    }

    fn open(
        &mut self,
        purpose: Purpose,
        mode: Mode,
        spec: filebrowser::Spec,
        name: &str,
        at: &Path,
    ) {
        filebrowser::open(
            &mut self.ui,
            purpose,
            "Browse",
            mode,
            spec,
            name,
            at.to_str().unwrap(),
        );
        self.settle();
    }
}

/// `dir/{a.png, b.PNG, c.wav, d.json, .hidden.png, notes.txt, sub/}`.
fn tree() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for f in [
        "a.png",
        "b.PNG",
        "c.wav",
        "d.json",
        ".hidden.png",
        "notes.txt",
    ] {
        std::fs::write(dir.path().join(f), b"x").unwrap();
    }
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    dir
}

#[test]
fn each_use_lists_folders_and_its_own_kind_of_file() {
    let dir = tree();
    let mut h = H::new();
    h.open(
        Purpose::Package,
        Mode::Open,
        filebrowser::IMAGE,
        "",
        dir.path(),
    );
    h.wait_for("fb:dir:sub");
    assert!(h.has("fb:file:a.png") && h.has("fb:file:b.PNG"));
    for no in [
        "fb:file:c.wav",
        "fb:file:d.json",
        "fb:file:notes.txt",
        "fb:file:.hidden.png",
    ] {
        assert!(!h.has(no), "{no} listed among images");
    }
    h.open(
        Purpose::Package,
        Mode::Open,
        filebrowser::PACKAGE,
        "",
        dir.path(),
    );
    h.wait_for("fb:file:d.json");
    assert!(!h.has("fb:file:a.png"));
    h.open(
        Purpose::RecordingsFolder,
        Mode::Folder,
        filebrowser::FOLDERS,
        "",
        dir.path(),
    );
    h.wait_for("fb:dir:sub");
    assert!(!h.has("fb:file:a.png") && !h.has("fb:file:c.wav"));
}

#[test]
fn picking_a_file_answers_the_place_that_asked_and_no_other() {
    let dir = tree();
    let mut h = H::new();
    h.open(
        Purpose::Package,
        Mode::Open,
        filebrowser::IMAGE,
        "",
        dir.path(),
    );
    h.wait_for("fb:file:a.png");
    h.click("fb:file:a.png");
    assert!(!h.has("fb:file:a.png"), "the window closes on a pick");
    assert_eq!(filebrowser::take(&mut h.ui, Purpose::PatchLoad), None);
    assert_eq!(
        filebrowser::take(&mut h.ui, Purpose::Package),
        Some(dir.path().join("a.png"))
    );
    assert_eq!(filebrowser::take(&mut h.ui, Purpose::Package), None, "once");
}

#[test]
fn folder_mode_picks_the_folder_shown() {
    let dir = tree();
    let mut h = H::new();
    h.open(
        Purpose::RecordingsFolder,
        Mode::Folder,
        filebrowser::FOLDERS,
        "",
        dir.path(),
    );
    h.wait_for("fb:dir:sub");
    h.click("fb:dir:sub");
    h.wait_for("fb:choose");
    h.click("fb:choose");
    assert_eq!(
        filebrowser::take(&mut h.ui, Purpose::RecordingsFolder),
        Some(dir.path().join("sub"))
    );
}

#[test]
fn save_mode_asks_before_replacing_and_adds_the_extension() {
    let dir = tree();
    let mut h = H::new();
    // A new name: no question; the first wanted extension is added.
    h.open(
        Purpose::Package,
        Mode::Save,
        filebrowser::IMAGE,
        "",
        dir.path(),
    );
    h.wait_for("fb:name");
    h.type_into("fb:name", "fresh");
    h.click("fb:save");
    assert_eq!(
        filebrowser::take(&mut h.ui, Purpose::Package),
        Some(dir.path().join("fresh.png"))
    );
    // A name that exists: the question; Cancel keeps the browser, Replace picks.
    h.open(
        Purpose::Package,
        Mode::Save,
        filebrowser::IMAGE,
        "a.png",
        dir.path(),
    );
    h.wait_for("fb:save");
    h.click("fb:save");
    assert!(h.has("fb:replace") && h.has("fb:replace-cancel"));
    assert_eq!(filebrowser::take(&mut h.ui, Purpose::Package), None);
    h.click("fb:replace-cancel");
    assert!(h.has("fb:save") && !h.has("fb:replace"));
    h.click("fb:save");
    h.click("fb:replace");
    assert_eq!(
        filebrowser::take(&mut h.ui, Purpose::Package),
        Some(dir.path().join("a.png"))
    );
}

#[test]
fn save_mode_refuses_an_empty_name_and_a_name_with_folders() {
    let dir = tree();
    let mut h = H::new();
    h.open(
        Purpose::PatchSave,
        Mode::Save,
        filebrowser::FOLDERS,
        "",
        dir.path(),
    );
    h.wait_for("fb:save");
    h.click("fb:save");
    assert_eq!(filebrowser::take(&mut h.ui, Purpose::PatchSave), None);
    h.type_into("fb:name", "../escape");
    h.click("fb:save");
    assert_eq!(filebrowser::take(&mut h.ui, Purpose::PatchSave), None);
    assert!(h.has("fb:save"), "still open");
    // A folder name for a patch: no extension added.
    h.type_into("fb:name", "my-patch");
    h.click("fb:save");
    assert_eq!(
        filebrowser::take(&mut h.ui, Purpose::PatchSave),
        Some(dir.path().join("my-patch"))
    );
}

#[test]
fn an_unreadable_folder_gives_a_message_and_the_browser_still_works() {
    let mut h = H::new();
    h.open(
        Purpose::Package,
        Mode::Open,
        filebrowser::IMAGE,
        "",
        Path::new("/"),
    );
    h.wait_for("fb:up");
    // Browse into a folder that goes away: the listing fails, nothing hangs.
    let dir = tempfile::tempdir().unwrap();
    let gone: PathBuf = dir.path().join("gone");
    std::fs::create_dir(&gone).unwrap();
    h.open(Purpose::Package, Mode::Open, filebrowser::IMAGE, "", &gone);
    std::fs::remove_dir(&gone).unwrap();
    for _ in 0..30 {
        h.settle();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(h.has("fb:up") && h.has("fb:home") && h.has("fb:browse-close"));
    h.click("fb:browse-close");
    assert!(!h.has("fb:up"));
}

#[test]
fn a_huge_folder_is_cut_with_a_message() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..2100 {
        std::fs::write(dir.path().join(format!("{i:05}.png")), b"").unwrap();
    }
    let mut h = H::new();
    h.open(
        Purpose::Package,
        Mode::Open,
        filebrowser::IMAGE,
        "",
        dir.path(),
    );
    h.wait_for("fb:file:00000.png");
    // The listing stops at 2000 rows (the scan order is the disk's, so which rows is not fixed).
    let rows =
        h.ui.hits
            .keys()
            .filter(|k| k.starts_with("fb:file:"))
            .count();
    assert_eq!(rows, 2000);
}

#[test]
fn the_last_folder_of_each_place_survives_a_restart_and_a_missing_one_is_dropped() {
    let cfg = tempfile::tempdir().unwrap();
    let (wt, art) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    // KABL_CONFIG_DIR is process-wide; no other test in this file reads it.
    std::env::set_var("KABL_CONFIG_DIR", cfg.path());
    let mut h = H::new();
    h.ui.use_user_prefs();
    h.open(
        Purpose::Package,
        Mode::Open,
        filebrowser::IMAGE,
        "",
        art.path(),
    );
    h.wait_for("fb:browse-close");
    h.ui.folders_remember(Place::Wavetables, wt.path());
    h.settle();
    let saved = std::fs::read_to_string(cfg.path().join("settings.toml")).unwrap();
    assert!(
        saved.contains("[folders]") && saved.contains("wavetables"),
        "{saved}"
    );
    // A new run reads them back; a folder that has gone is dropped.
    let mut again = UiState::default();
    again.use_user_prefs();
    assert_eq!(again.folders.get("art"), Some(&art.path().to_path_buf()));
    assert_eq!(
        again.folders.get("wavetables"),
        Some(&wt.path().to_path_buf())
    );
    let (gone, kept) = (wt.path().to_path_buf(), art.path().to_path_buf());
    drop(wt);
    let mut after = UiState::default();
    after.use_user_prefs();
    assert_eq!(after.folders.get("wavetables"), None, "{gone:?} is gone");
    assert_eq!(after.folders.get("art"), Some(&kept));
    // And a place with nothing remembered starts in home.
    assert_eq!(
        filebrowser::start_dir(&after, Place::Patches, ""),
        std::env::var_os("HOME").map_or(PathBuf::from("/"), PathBuf::from)
    );
}

#[test]
fn a_picked_image_reaches_the_panel_authoring_path_field() {
    let dir = tree();
    let mut h = H::new();
    h.open(
        Purpose::PanelArt,
        Mode::Open,
        filebrowser::IMAGE,
        "",
        dir.path(),
    );
    h.wait_for("fb:file:b.PNG");
    h.click("fb:file:b.PNG");
    h.settle();
    assert_eq!(h.ui.panels.path, dir.path().join("b.PNG").to_str().unwrap());
}

#[test]
fn opening_a_patch_folder_remembers_the_folder_it_was_in() {
    let dir = tree();
    let mut h = H::new();
    h.open(
        Purpose::PatchLoad,
        Mode::Folder,
        filebrowser::FOLDERS,
        "",
        dir.path(),
    );
    h.wait_for("fb:dir:sub");
    h.click("fb:dir:sub");
    h.wait_for("fb:choose");
    h.click("fb:choose");
    assert_eq!(
        filebrowser::take(&mut h.ui, Purpose::PatchLoad),
        Some(dir.path().join("sub"))
    );
    assert_eq!(h.ui.folders.get("patches"), Some(&dir.path().to_path_buf()));
}
