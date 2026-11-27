//! Computer keys and the key strip (`kabl_ui::keys`) through real egui input (`show()`):
//! typing in a field never plays, focus loss, a scene change and a sound load end every note,
//! a key held across an octave change ends the note it started, auto-repeat never retriggers,
//! and the strip and a key on the same note share one voice.

use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect};
use kabl_engine::keyboard::KeyEvent;
use kabl_ui::library::Library;
use kabl_ui::{show, PatchEditor, UiState};

struct H {
    focused: bool,
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    events: Vec<Event>,
    t: f64,
    _user: tempfile::TempDir,
}

const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);

impl H {
    fn new() -> Self {
        let user = tempfile::tempdir().unwrap();
        let mut ui = UiState::default();
        ui.keys.computer = true;
        ui.library = Some(Library::open(
            Some(kabl_ui::library::built_factory_dir()),
            user.path().to_path_buf(),
        ));
        let mut h = H {
            focused: true,
            ctx: egui::Context::default(),
            editor: PatchEditor::seed_from(&kabl_standalone::default_patch()),
            ui,
            events: Vec::new(),
            t: 0.0,
            _user: user,
        };
        h.frame();
        h.frame();
        h
    }

    fn frame(&mut self) {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)),
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
            egui::UiBuilder::new().max_rect(Rect::from_min_size(Pos2::ZERO, SIZE)),
        );
        show(&mut self.editor, &mut self.ui, &mut root);
        drop(self.ctx.end_pass());
    }

    fn key_event(&mut self, key: Key, pressed: bool, repeat: bool) {
        self.events.push(Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat,
            modifiers: Modifiers::NONE,
        });
    }

    fn press(&mut self, key: Key) {
        self.key_event(key, true, false);
        self.frame();
    }

    fn release(&mut self, key: Key) {
        self.key_event(key, false, false);
        self.frame();
    }

    fn click(&mut self, hit: &str) {
        let p = self
            .ui
            .hits
            .get(hit)
            .unwrap_or_else(|| panic!("no hit target {hit}"))
            .center();
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
    }

    /// The key events sent since the last call.
    fn out(&mut self) -> Vec<KeyEvent> {
        std::mem::take(&mut self.ui.keys.out)
    }
}

fn on(note: u8) -> KeyEvent {
    KeyEvent::On {
        note,
        velocity: 100,
    }
}

fn off(note: u8) -> KeyEvent {
    KeyEvent::Off { note }
}

const ROW: [Key; 8] = [
    Key::A,
    Key::S,
    Key::D,
    Key::F,
    Key::G,
    Key::H,
    Key::J,
    Key::K,
];

#[test]
fn typing_in_a_field_plays_nothing() {
    let mut h = H::new();
    h.ui.browser_open = true;
    h.frame();
    h.frame();
    // Nothing focused yet: the keys play.
    h.press(Key::A);
    h.release(Key::A);
    assert_eq!(h.out(), [on(60), off(60)]);
    h.click("search");
    assert!(
        h.ctx.egui_wants_keyboard_input(),
        "the search field has focus"
    );
    for key in ROW {
        h.key_event(key, true, false);
    }
    h.frame();
    for key in ROW {
        h.key_event(key, false, false);
    }
    h.frame();
    assert_eq!(h.out(), []);
    assert!(!h.ui.keys.any());
}

#[test]
fn plugin_keys_stay_with_the_host() {
    let mut h = H::new();
    h.ui.keys.computer = false;
    h.press(Key::A);
    h.release(Key::A);
    assert_eq!(h.out(), []);
}

#[test]
fn focus_loss_ends_every_note() {
    let mut h = H::new();
    h.press(Key::A);
    h.press(Key::D);
    assert_eq!(h.out(), [on(60), on(64)]);
    h.focused = false;
    h.frame();
    let mut got = h.out();
    got.sort_by_key(|e| format!("{e:?}"));
    assert_eq!(got, [off(60), off(64)]);
    assert!(!h.ui.keys.any());
    // The release that arrives after focus comes back sends nothing more.
    h.focused = true;
    h.release(Key::A);
    assert_eq!(h.out(), []);
}

#[test]
fn a_scene_change_ends_every_note() {
    for scene in ["perform", "browser"] {
        let mut h = H::new();
        h.press(Key::A);
        assert_eq!(h.out(), [on(60)]);
        match scene {
            "perform" => h.ui.perform_open = true,
            _ => h.ui.browser_open = true,
        }
        h.frame();
        assert_eq!(h.out(), [off(60)], "{scene}");
        h.release(Key::A);
        assert_eq!(h.out(), [], "{scene}");
    }
}

#[test]
fn a_sound_load_ends_every_note() {
    let mut h = H::new();
    h.press(Key::A);
    h.ui.keys.strip_to(Some(70));
    assert_eq!(h.out(), [on(60), on(70)]);
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/presets/velvet-tine");
    kabl_ui::browser::replace_patch(&mut h.editor, &mut h.ui, kabl_core::load(&dir).unwrap());
    h.frame();
    assert_eq!(h.out(), [off(60), off(70)]);
    // The host takes the load; the next one is noticed again.
    h.ui.loaded = false;
    h.frame();
    h.press(Key::S);
    assert_eq!(h.out(), [on(62)]);
}

#[test]
fn a_key_held_across_an_octave_change_ends_its_own_note() {
    let mut h = H::new();
    h.press(Key::A);
    h.press(Key::X);
    h.release(Key::X);
    assert_eq!(h.ui.keys.octave, 1);
    h.release(Key::A);
    h.press(Key::A);
    for _ in 0..2 {
        h.press(Key::Z);
        h.release(Key::Z);
    }
    h.release(Key::A);
    assert_eq!(h.out(), [on(60), off(60), on(72), off(72)]);
    assert_eq!(h.ui.keys.octave, -1);
}

#[test]
fn auto_repeat_never_retriggers() {
    let mut h = H::new();
    h.press(Key::A);
    for _ in 0..3 {
        h.key_event(Key::A, true, true);
        h.frame();
    }
    // A repeat the OS did not mark as one.
    h.press(Key::A);
    // A repeated Z does not run the octave down.
    h.press(Key::Z);
    h.key_event(Key::Z, true, true);
    h.frame();
    assert_eq!(h.ui.keys.octave, -1);
    h.release(Key::A);
    assert_eq!(h.out(), [on(60), off(60)]);
}

#[test]
fn strip_and_key_on_one_note_share_a_voice() {
    let mut h = H::new();
    h.press(Key::A);
    h.ui.keys.strip_to(Some(60));
    h.release(Key::A);
    assert_eq!(h.out(), [on(60)], "still held on the strip");
    h.ui.keys.strip_to(None);
    assert_eq!(h.out(), [off(60)]);
}

#[test]
fn the_strip_opens_and_plays_under_the_rack() {
    let mut h = H::new();
    assert!(!h.ui.keys.strip_open, "hidden until opened");
    assert!(!h.ui.hits.contains_key("keys:strip"));
    h.click("keys:toggle");
    assert!(h.ui.keys.strip_open);
    h.frame();
    let strip = h.ui.hits["keys:strip"];
    let p = strip.left_bottom() + egui::vec2(4.0, -4.0);
    h.events.push(Event::PointerMoved(p));
    h.frame();
    h.events.push(Event::PointerButton {
        pos: p,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    h.frame();
    h.events.push(Event::PointerButton {
        pos: p,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    h.frame();
    assert_eq!(h.out(), [on(60), off(60)]);
}

impl H {
    /// Clicks a knob on the rack: it takes keyboard focus (arrows), not text.
    fn focus_a_knob(&mut self) {
        let knob = self
            .ui
            .hits
            .keys()
            .find(|k| k.starts_with("knob:") && k.ends_with(".cutoff_hz"))
            .expect("the default patch shows a filter cutoff knob")
            .clone();
        self.click(&knob);
        assert!(
            self.ctx.egui_wants_keyboard_input(),
            "the knob {knob} has focus"
        );
        self.out();
    }
}

/// A focused knob takes arrows, not letters: A still plays, and never opens the add menu.
#[test]
fn a_focused_knob_leaves_the_keys_playing() {
    let mut h = H::new();
    h.focus_a_knob();
    h.press(Key::A);
    assert_eq!(h.out(), [on(60)]);
    assert!(!egui::Popup::is_any_open(&h.ctx), "no add menu");
    h.release(Key::A);
    assert_eq!(h.out(), [off(60)]);
}

/// With computer keys off, A is the add-module shortcut again, knob focused or not.
#[test]
fn with_computer_keys_off_a_opens_the_add_menu() {
    let mut h = H::new();
    h.ui.keys.computer = false;
    h.focus_a_knob();
    h.press(Key::A);
    assert!(h.out().is_empty());
    assert!(egui::Popup::is_any_open(&h.ctx), "add menu open");
}

/// A window that loses focus with the button still down on the strip ends the note once and
/// does not start it again every frame.
#[test]
fn an_unfocused_window_never_retriggers_the_strip() {
    let mut h = H::new();
    h.click("keys:toggle");
    h.frame();
    let p = h.ui.hits["keys:strip"].left_bottom() + egui::vec2(4.0, -4.0);
    h.events.push(Event::PointerMoved(p));
    h.frame();
    h.events.push(Event::PointerButton {
        pos: p,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    h.frame();
    assert_eq!(h.out(), [on(60)]);
    h.focused = false;
    for _ in 0..5 {
        h.frame();
    }
    assert_eq!(h.out(), [off(60)], "one note-off, no retrigger");
}

/// The plugin's key split (`keys::Owner`): which keys kabl keeps from the host, per view, as
/// a key arrives after a frame. Every row names its state, key and modifiers; a row that flips
/// fails with its name.
#[test]
fn kabl_keeps_its_keys_and_the_host_gets_the_rest() {
    use kabl_ui::keys::Owner;
    const NONE: Modifiers = Modifiers::NONE;
    const SHIFT: Modifiers = Modifiers::SHIFT;
    const CMD: Modifiers = Modifiers::COMMAND;
    const KABL: bool = true;
    const HOST: bool = false;
    type Row = (&'static str, Option<Key>, Modifiers, bool);
    let check = |state: &str, owner: Owner, rows: &[Row]| {
        for &(name, key, modifiers, kabl) in rows {
            assert_eq!(
                owner.keeps(key, modifiers),
                kabl,
                "{state}: {name} should go to {}",
                if kabl { "kabl" } else { "the host" }
            );
        }
    };

    // Computer keys on, rack, nothing focused.
    let mut h = H::new();
    check(
        "computer keys on",
        Owner::of(&h.ui, &h.ctx),
        &[
            ("A (note)", Some(Key::A), NONE, KABL),
            ("W (note)", Some(Key::W), NONE, KABL),
            ("' (top note)", Some(Key::Quote), NONE, KABL),
            ("Z (octave down)", Some(Key::Z), NONE, KABL),
            ("X (octave up)", Some(Key::X), NONE, KABL),
            ("Shift+A", Some(Key::A), SHIFT, HOST),
            ("Ctrl+A", Some(Key::A), CMD, HOST),
            ("Space", Some(Key::Space), NONE, HOST),
            ("Q (not a note)", Some(Key::Q), NONE, HOST),
            ("Escape", Some(Key::Escape), NONE, KABL),
        ],
    );

    // Computer keys off (the plugin's default): the note keys are the host's, A is add-module.
    h.ui.keys.computer = false;
    h.frame();
    check(
        "computer keys off, rack",
        Owner::of(&h.ui, &h.ctx),
        &[
            ("A (add-module menu)", Some(Key::A), NONE, KABL),
            ("W", Some(Key::W), NONE, HOST),
            ("Z", Some(Key::Z), NONE, HOST),
            ("X", Some(Key::X), NONE, HOST),
            ("Space (browser closed)", Some(Key::Space), NONE, HOST),
            ("Enter (browser closed)", Some(Key::Enter), NONE, HOST),
            ("Down (browser closed)", Some(Key::ArrowDown), NONE, HOST),
            ("Left", Some(Key::ArrowLeft), NONE, HOST),
            ("Delete", Some(Key::Delete), NONE, HOST),
            ("Tab", Some(Key::Tab), NONE, HOST),
            ("Ctrl+Z (rack undo)", Some(Key::Z), CMD, KABL),
            ("Ctrl+Shift+Z (rack redo)", Some(Key::Z), CMD | SHIFT, KABL),
            ("Ctrl+Y (rack redo)", Some(Key::Y), CMD, KABL),
            ("Ctrl+S (save)", Some(Key::S), CMD, KABL),
            ("Ctrl+Q", Some(Key::Q), CMD, HOST),
            ("Ctrl+Space", Some(Key::Space), CMD, HOST),
            ("F5", Some(Key::F5), NONE, HOST),
            ("a key egui cannot name", None, NONE, HOST),
        ],
    );

    // A focused knob takes the arrows; Space stays the host's.
    h.focus_a_knob();
    check(
        "knob focused",
        Owner::of(&h.ui, &h.ctx),
        &[
            ("Up", Some(Key::ArrowUp), NONE, KABL),
            ("Shift+Left (fine)", Some(Key::ArrowLeft), SHIFT, KABL),
            ("Space", Some(Key::Space), NONE, HOST),
            ("Ctrl+Z", Some(Key::Z), CMD, KABL),
        ],
    );

    // The browser with a row selected: Space previews, Enter loads, Up and Down move.
    let mut h = H::new();
    h.ui.keys.computer = false;
    h.ui.browser_open = true;
    h.frame();
    h.ui.browser.selected = Some("factory:init-keyboard".into());
    h.frame();
    assert!(h.ui.browser.selected.is_some());
    check(
        "browser, a row selected",
        Owner::of(&h.ui, &h.ctx),
        &[
            ("Space (preview)", Some(Key::Space), NONE, KABL),
            ("Enter (load)", Some(Key::Enter), NONE, KABL),
            ("Down", Some(Key::ArrowDown), NONE, KABL),
            ("Up", Some(Key::ArrowUp), NONE, KABL),
            ("Shift+Space", Some(Key::Space), SHIFT, HOST),
            ("W", Some(Key::W), NONE, HOST),
        ],
    );

    // A text field has focus: every key is kabl's, shortcuts and note keys included.
    h.click("search");
    assert!(
        h.ctx.egui_wants_keyboard_input(),
        "the search field has focus"
    );
    check(
        "text field focused",
        Owner::of(&h.ui, &h.ctx),
        &[
            ("Space", Some(Key::Space), NONE, KABL),
            ("W", Some(Key::W), NONE, KABL),
            ("Ctrl+A", Some(Key::A), CMD, KABL),
            ("Tab", Some(Key::Tab), NONE, KABL),
            ("a key egui cannot name", None, NONE, KABL),
        ],
    );

    // The design page: its own undo, the arrows nudge, Delete removes.
    let mut h = H::new();
    h.ui.keys.computer = false;
    let id =
        kabl_ui::composites::encapsulate(&mut h.editor, [3].into(), Default::default(), "Voice")
            .unwrap();
    let c = h.editor.state().composites[&id].clone();
    kabl_ui::panels::open(&mut h.ui, id, &c);
    h.frame();
    h.frame();
    assert!(h.ui.panels.page.is_some(), "the design page is open");
    check(
        "design page",
        Owner::of(&h.ui, &h.ctx),
        &[
            ("Ctrl+Z (page undo)", Some(Key::Z), CMD, KABL),
            ("Left (nudge)", Some(Key::ArrowLeft), NONE, KABL),
            ("Down (nudge)", Some(Key::ArrowDown), NONE, KABL),
            ("Delete", Some(Key::Delete), NONE, KABL),
            ("Escape", Some(Key::Escape), NONE, KABL),
            ("Space", Some(Key::Space), NONE, HOST),
        ],
    );
}
