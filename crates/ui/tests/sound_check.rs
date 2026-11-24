//! Lesson 00's Sound-check strip: the output device and MIDI input choices and the output meter
//! on the Start card, and the choices remembered in `settings.toml` by the standalone app only.
use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect};
use kabl_ui::first_run::devices::{self, Choice, Devices};
use kabl_ui::library::Library;
use kabl_ui::{first_run, lessons, show, style, PatchEditor, UiState};

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    _dir: tempfile::TempDir,
    events: Vec<Event>,
    t: f64,
}

impl H {
    /// A fresh profile's launch, as the standalone app (which owns the devices) does it.
    fn standalone() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let mut ui = UiState::default();
        ui.library = Some(Library::open(None, dir.path().to_path_buf()));
        ui.drawer_open = false;
        ui.devices = Some(Devices::default());
        ui.audio_outputs = vec![(0, "Speakers".into()), (1, "USB Audio".into())];
        ui.midi_inputs = vec!["Keystep 24:0".into()];
        let editor = first_run::open(&mut ui);
        let mut h = H {
            ctx: egui::Context::default(),
            editor,
            ui,
            _dir: dir,
            events: Vec::new(),
            t: 0.0,
        };
        h.frames(3);
        h
    }

    fn click(&mut self, key: &str) {
        let at = self.ui.hits[key].center();
        self.events.push(Event::PointerMoved(at));
        self.frames(1);
        for pressed in [true, false] {
            self.events.push(Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
            self.frames(1);
        }
        self.frames(2);
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            let size = egui::vec2(1280.0, 800.0);
            let raw = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                time: Some(self.t),
                events: std::mem::take(&mut self.events),
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
            drop(self.ctx.end_pass());
        }
    }

    fn has(&self, key: &str) -> bool {
        self.ui.hits.contains_key(key)
    }
}

fn sound_check_step() -> usize {
    let l = lessons::all().iter().find(|l| l.card).expect("lesson 00");
    l.steps
        .iter()
        .position(|s| s.sound_check)
        .expect("lesson 00 has a Sound-check step")
}

#[test]
fn lesson_00_shows_the_sound_check_strip_with_both_choices_and_the_meter() {
    let mut h = H::standalone();
    let step = sound_check_step();
    // Reach the step the way the user does: Next until it shows.
    while h.ui.lessons.active.as_ref().unwrap().step < step {
        h.click("start-next");
    }
    for key in [
        "sound-check",
        "sound-check-output",
        "sound-check-midi",
        "sound-check-meter",
    ] {
        assert!(h.has(key), "{key} is not on the Start card");
    }
    let card = h.ui.hits["start-card"];
    assert!(card.contains_rect(h.ui.hits["sound-check"]));
    // The next step has no strip.
    h.ui.lessons.active.as_mut().unwrap().step = step + 1;
    h.ui.hits.clear();
    h.frames(2);
    assert!(!h.has("sound-check-output"));
}

#[test]
fn a_plugin_shows_no_strip() {
    let mut h = H::standalone();
    h.ui.devices = None;
    h.ui.hits.clear();
    h.ui.lessons.active.as_mut().unwrap().step = sound_check_step();
    h.frames(2);
    assert!(h.has("start-card"));
    assert!(!h.has("sound-check-output") && !h.has("sound-check-midi"));
}

#[test]
fn picks_are_remembered_by_the_standalone_app_and_left_alone_by_a_plugin() {
    let cfg = tempfile::tempdir().unwrap();
    // KABL_CONFIG_DIR is process-wide; no other test in this file reads it.
    std::env::set_var("KABL_CONFIG_DIR", cfg.path());
    let load = || style::Prefs::load(cfg.path()).devices;

    let mut app = UiState::default();
    app.use_user_prefs();
    assert_eq!(devices::take_over(&mut app), Devices::default());
    devices::remember_output(&mut app, Some("USB Audio".into()));
    devices::remember_midi(&mut app, Some("Keystep 24:0".into()));
    assert_eq!(
        load(),
        Devices {
            output: Choice::Named("USB Audio".into()),
            midi: Choice::Named("Keystep 24:0".into()),
        }
    );
    // "No MIDI input" is remembered as a choice; "System default" as no choice.
    devices::remember_midi(&mut app, None);
    devices::remember_output(&mut app, None);
    assert_eq!(
        load(),
        Devices {
            output: Choice::Unset,
            midi: Choice::None,
        }
    );

    // The next launch gets them back.
    let mut again = UiState::default();
    again.use_user_prefs();
    assert_eq!(devices::take_over(&mut again).midi, Choice::None);

    // A plugin neither takes them over nor changes them, and saving its theme keeps what the
    // standalone app wrote since the plugin loaded the file.
    let mut plugin = UiState::default();
    plugin.use_user_prefs();
    devices::remember_midi(&mut plugin, Some("Other".into()));
    assert_eq!(load().midi, Choice::None);
    devices::remember_midi(&mut again, Some("Keystep 24:0".into()));
    plugin.set_theme_mode(kabl_ui::desktop::ThemeMode::Dark);
    assert_eq!(load().midi, Choice::Named("Keystep 24:0".into()));
}
