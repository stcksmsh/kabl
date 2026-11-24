//! The first run in the plugin: a plugin instance on a profile that never had it shows lesson 00
//! on the Start card over its own patch, once ever, shared with the standalone app and every
//! other instance, and silent: nothing reaches the audio side unless the user asks.
use std::path::Path;
use std::sync::OnceLock;

use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect};
use kabl_ui::first_run::devices;
use kabl_ui::library::Library;
use kabl_ui::{first_run, lessons, show, PatchEditor, UiState};

/// `use_user_prefs` reads `KABL_CONFIG_DIR`, which is process-wide: one folder for every test
/// here, kept for the whole run. Lesson progress is in each test's own library folder.
fn config() {
    static DIR: OnceLock<tempfile::TempDir> = OnceLock::new();
    let d = DIR.get_or_init(|| tempfile::tempdir().unwrap());
    std::env::set_var("KABL_CONFIG_DIR", d.path());
}

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    events: Vec<Event>,
    t: f64,
    texts: Vec<String>,
}

impl H {
    fn new(editor: PatchEditor, ui: UiState) -> Self {
        H {
            ctx: egui::Context::default(),
            editor,
            ui,
            events: Vec::new(),
            t: 0.0,
            texts: Vec::new(),
        }
    }

    /// What the plugin does: its own patch, the user's prefs, devices left to the host, and the
    /// library opened lazily by the editor. No frame yet.
    fn plugin_unshown(dir: &Path, editor: PatchEditor) -> Self {
        config();
        let mut ui = UiState::default();
        ui.use_user_prefs();
        ui.library = Some(Library::open(None, dir.to_path_buf()));
        H::new(editor, ui)
    }

    fn plugin_on(dir: &Path, editor: PatchEditor) -> Self {
        let mut h = H::plugin_unshown(dir, editor);
        h.frames(3);
        h
    }

    /// A plugin on its factory patch.
    fn plugin(dir: &Path) -> Self {
        H::plugin_on(
            dir,
            PatchEditor::seed_from(&kabl_standalone::default_patch()),
        )
    }

    /// What the standalone app does on launch with no patch named.
    fn standalone(dir: &Path) -> Self {
        config();
        let mut ui = UiState::default();
        ui.use_user_prefs();
        devices::take_over(&mut ui);
        ui.library = Some(Library::open(None, dir.to_path_buf()));
        let editor = first_run::open(&mut ui);
        let mut h = H::new(editor, ui);
        h.frames(3);
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
        let shapes = self.ctx.end_pass().shapes;
        self.texts.clear();
        for c in &shapes {
            collect_texts(&c.shape, &mut self.texts);
        }
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.frame();
        }
    }

    /// The Start card is up with lesson 00 on it.
    fn card(&self) -> bool {
        self.ui.hits.contains_key("start-card")
            && self
                .ui
                .lessons
                .active
                .as_ref()
                .is_some_and(|a| lessons::all()[a.lesson].id == "start-here")
    }

    fn step(&self) -> usize {
        self.ui.lessons.active.as_ref().expect("lesson 00").step
    }

    fn click(&mut self, key: &str) {
        let at = self.ui.hits[key].center();
        self.events.push(Event::PointerMoved(at));
        self.frame();
        for pressed in [true, false] {
            self.events.push(Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
            self.frame();
        }
        self.frames(2);
    }

    fn esc(&mut self) {
        for pressed in [true, false] {
            self.events.push(Event::Key {
                key: Key::Escape,
                physical_key: Some(Key::Escape),
                pressed,
                repeat: false,
                modifiers: Modifiers::NONE,
            });
            self.frame();
        }
    }

    fn shows(&self, text: &str) -> bool {
        self.texts.iter().any(|t| t == text)
    }
}

fn collect_texts(s: &egui::Shape, out: &mut Vec<String>) {
    match s {
        egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
        egui::Shape::Vec(v) => v.iter().for_each(|s| collect_texts(s, out)),
        _ => {}
    }
}

fn first_run_done(dir: &Path) -> bool {
    Library::open(None, dir.to_path_buf()).prefs.first_run_done
}

fn step1() -> &'static lessons::Step {
    &lessons::all()
        .iter()
        .find(|l| l.card)
        .expect("lesson 00")
        .steps[0]
}

#[test]
fn a_plugin_on_a_fresh_profile_shows_the_start_card_and_resumes_it_when_reopened() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = H::plugin(dir.path());
    assert!(
        h.card(),
        "a fresh profile's first plugin shows the Start card"
    );
    assert!(first_run_done(dir.path()), "saved as it starts");
    assert!(!h.ui.browser_open, "the rack stays in view");

    h.click("start-next");
    assert_eq!(h.step(), 1);
    // The editor closed and opened again: the plugin keeps its view, the window is new.
    h.ctx = egui::Context::default();
    h.frames(3);
    assert!(h.card());
    assert_eq!(h.step(), 1, "resumes at the same step");
}

#[test]
fn the_first_run_is_shared_between_the_standalone_app_and_the_plugin() {
    // The standalone app had it: the plugin shows nothing.
    let dir = tempfile::tempdir().unwrap();
    assert!(H::standalone(dir.path()).card());
    assert!(!H::plugin(dir.path()).card());

    // Skipped in the plugin: the standalone app shows nothing next launch.
    let dir = tempfile::tempdir().unwrap();
    let mut h = H::plugin(dir.path());
    assert!(h.card());
    h.esc();
    assert!(h.ui.lessons.active.is_none(), "Esc skips");
    assert!(first_run_done(dir.path()));
    assert!(!H::standalone(dir.path()).card());
    assert!(!H::plugin(dir.path()).card());

    // Finished in the plugin: the same.
    let dir = tempfile::tempdir().unwrap();
    let mut h = H::plugin(dir.path());
    let n = lessons::all()[h.ui.lessons.active.as_ref().unwrap().lesson]
        .steps
        .len();
    for _ in 0..n {
        h.click("start-next");
    }
    assert_eq!(h.step(), n, "the last page");
    h.click("start-skip");
    assert!(h.ui.lessons.active.is_none(), "Close ends it");
    assert!(!H::standalone(dir.path()).card());
}

#[test]
fn the_card_in_a_plugin_sends_nothing_to_the_audio_side() {
    // The start patch has a clock: the one thing the card could start.
    let dir = tempfile::tempdir().unwrap();
    let mut h = H::plugin_on(dir.path(), first_run::start_patch());
    let before = h.editor.state().clone();
    assert!(h.card());
    let n = lessons::all()[h.ui.lessons.active.as_ref().unwrap().lesson]
        .steps
        .len();
    let silent = |h: &H| {
        assert!(h.ui.transport.is_empty(), "transport: {:?}", h.ui.transport);
        assert!(h.ui.launches.is_empty(), "launches: {:?}", h.ui.launches);
        assert!(h.ui.keys.out.is_empty(), "notes: {:?}", h.ui.keys.out);
        assert!(h.ui.clock_running.is_empty());
        assert!(!h.ui.loaded && !h.ui.all_notes_off);
        assert!(h.ui.output_select.is_none() && h.ui.midi_select.is_none());
        assert_eq!(h.editor.state(), &before, "the patch is untouched");
    };
    silent(&h);
    for _ in 0..n {
        h.click("start-next");
        silent(&h);
    }
    h.click("start-skip");
    silent(&h);
}

#[test]
fn step_one_in_a_plugin_does_not_say_the_sound_is_playing() {
    let s = step1();
    let plugin_text = s
        .plugin_text
        .as_deref()
        .expect("lesson 00 step 1 has plugin text");
    assert_ne!(plugin_text, s.text);
    assert!(s.text.contains("already playing"));
    assert!(!plugin_text.contains("playing"), "{plugin_text}");

    let dir = tempfile::tempdir().unwrap();
    let h = H::plugin(dir.path());
    assert_eq!(h.step(), 0);
    assert!(h.shows(plugin_text), "plugin card: {:?}", h.texts);
    assert!(!h.shows(&s.text));

    let dir = tempfile::tempdir().unwrap();
    let h = H::standalone(dir.path());
    assert_eq!(h.step(), 0);
    assert!(h.shows(&s.text), "standalone card: {:?}", h.texts);
    assert!(!h.shows(plugin_text));
}

#[test]
fn the_play_button_is_on_the_standalone_card_only() {
    let dir = tempfile::tempdir().unwrap();
    let h = H::plugin_on(dir.path(), first_run::start_patch());
    assert!(h.card());
    assert!(
        !h.ui.hits.contains_key("start-stop"),
        "the host's transport decides in a plugin"
    );

    let dir = tempfile::tempdir().unwrap();
    let h = H::standalone(dir.path());
    assert!(h.card());
    assert!(h.ui.hits.contains_key("start-stop"));
}

#[test]
fn two_plugins_opened_together_show_the_card_once() {
    let dir = tempfile::tempdir().unwrap();
    let factory = || PatchEditor::seed_from(&kabl_standalone::default_patch());
    // Both read the profile before either drew a frame.
    let mut a = H::plugin_unshown(dir.path(), factory());
    let mut b = H::plugin_unshown(dir.path(), factory());
    a.frames(3);
    b.frames(3);
    assert!(a.card());
    assert!(!b.card(), "the second instance shows none");
    a.esc();
    assert!(first_run_done(dir.path()));
    assert!(!H::plugin(dir.path()).card());
}
