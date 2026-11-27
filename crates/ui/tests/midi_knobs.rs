//! Macro knob profiles on `patches/echo`: a profile saved per controller drives the patch's
//! first macro module through soft takeover and gesture grouping, the patch's own mappings win,
//! lookup falls back to "any controller", and Settings sets up, lists and forgets a profile.

use egui::{Event, Pos2, RawInput, Rect};
use kabl_ui::perform::{self, KnobProfile, ANY_CONTROLLER};
use kabl_ui::{routing, show, PatchEditor, UiState};

const CLOCK: u64 = 1;
const FILTER: u64 = 9;
const MACROS: u64 = 20;
const PORT: &str = "Pad 24:0";

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
        Self::with_perform(false)
    }

    fn with_perform(perform: bool) -> Self {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/echo");
        let mut editor = PatchEditor::from_log(kabl_core::load(&dir).expect("patch"));
        // Start with no pins or MIDI mappings of the patch's own.
        let changes: Vec<_> = editor
            .state()
            .modules
            .iter()
            .flat_map(|(&id, m)| {
                m.params
                    .keys()
                    .filter(|key| {
                        key.starts_with("pin.") || key.starts_with("cc.") || key.starts_with("btn.")
                    })
                    .map(move |key| (id, key.clone(), None))
            })
            .collect();
        editor.set_presentation(&changes);
        let first_macro = editor
            .state()
            .modules
            .iter()
            .find(|(_, m)| m.kind == "macro")
            .map(|(&id, _)| id);
        assert_eq!(first_macro, Some(MACROS));
        let mut h = H {
            ctx: egui::Context::default(),
            editor,
            ui: UiState::default(),
            events: Vec::new(),
            t: 0.0,
            pointer: Pos2::ZERO,
        };
        h.ui.perform_open = perform;
        for _ in 0..30 {
            h.frame();
        }
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
        perform::rearm(&mut self.ui, self.t);
        // Headless: the frame output (shapes, platform requests) has nowhere to go.
        let _ = self.ctx.end_pass();
    }

    fn has(&self, key: &str) -> bool {
        self.ui.hits.contains_key(key)
    }

    fn click(&mut self, key: &str) {
        let r = *self
            .ui
            .hits
            .get(key)
            .unwrap_or_else(|| panic!("no hit target {key}"));
        self.pointer = r.center();
        self.events.push(Event::PointerMoved(self.pointer));
        self.frame();
        for pressed in [true, false] {
            self.events.push(Event::PointerButton {
                pos: self.pointer,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            });
            self.frame();
        }
        self.frame();
    }

    /// A CC through the control layer, as the control thread delivers it.
    fn cc(&mut self, ch: u8, cc: u8, v: u8) {
        perform::apply_cc(&mut self.editor, &mut self.ui, &[(ch, cc, v)], self.t);
        self.t += 0.02;
    }

    fn value(&self, id: u64, param: &str) -> f32 {
        routing::base_value(self.editor.state(), id, info(id, &self.editor, param))
    }

    /// The CC value at the param's position: sending it picks the param up.
    fn at(&self, id: u64, param: &str) -> u8 {
        let p = info(id, &self.editor, param);
        (p.to_norm(self.value(id, param)) * 127.0).round() as u8
    }

    fn entries(&self) -> usize {
        self.editor.log().entries().len()
    }

    fn profile(&mut self, port: &str, knobs: &[(&str, u32)]) {
        let p: KnobProfile = knobs.iter().map(|&(k, c)| (k.to_string(), c)).collect();
        self.ui.knob_profiles.insert(port.into(), p);
    }

    /// Turns a CC from the param's position up by `steps` × 3.
    fn turn(&mut self, ch: u8, cc: u8, from: u8, steps: u8) {
        for k in 0..=steps {
            self.cc(ch, cc, (from + 3 * k).min(127));
        }
    }
}

fn info(id: u64, editor: &PatchEditor, param: &str) -> &'static kabl_modules::ParamInfo {
    kabl_modules::registry::info_for(&editor.state().modules[&id].kind)
        .unwrap()
        .params
        .iter()
        .find(|p| p.name == param)
        .unwrap()
}

#[test]
fn a_profile_drives_the_first_macro_with_pickup_and_one_undo_step_per_turn() {
    let mut h = H::new();
    h.ui.midi_input = Some(PORT.into());
    h.profile(PORT, &[("m3", 74)]);
    let base = h.value(MACROS, "m3");
    let at = h.at(MACROS, "m3");
    assert!(at < 90, "room to turn up from {at}");
    // Far from the value: soft takeover holds it.
    h.cc(0, 74, if at > 40 { 0 } else { 127 });
    assert_eq!(h.value(MACROS, "m3"), base);
    // Another channel's CC 74 is not in the profile.
    h.cc(1, 74, at + 20);
    assert_eq!(h.value(MACROS, "m3"), base);
    let entries = h.entries();
    h.turn(0, 74, at, 10);
    assert!(h.value(MACROS, "m3") > base, "the turn moves m3");
    assert_eq!(h.entries(), entries + 1, "one turn is one undo step");
    // A pause starts a new step.
    h.t += 1.5;
    h.cc(0, 74, at + 35);
    assert_eq!(h.entries(), entries + 2);
    h.editor.undo();
    h.editor.undo();
    h.frame();
    assert_eq!(h.value(MACROS, "m3"), base);
    // Undo drops the pickup: the next CC cannot jump.
    h.cc(0, 74, 127);
    assert_eq!(h.value(MACROS, "m3"), base);
}

#[test]
fn the_patch_mapping_of_a_cc_wins_over_the_profile() {
    let mut h = H::new();
    h.ui.midi_input = Some(PORT.into());
    h.profile(PORT, &[("m1", 74), ("m2", 75)]);
    // Learned on the patch: CC 74 drives the filter cutoff, CC 75 the clock's Run button.
    h.editor.set_presentation(&[
        (FILTER, "cc.cutoff_hz".into(), Some(74.0)),
        (CLOCK, "btn.run".into(), Some(75.0)),
    ]);
    let (m1, m2) = (h.value(MACROS, "m1"), h.value(MACROS, "m2"));
    let cutoff = h.value(FILTER, "cutoff_hz");
    let at = h.at(FILTER, "cutoff_hz");
    for v in (at.saturating_sub(3)..=127).step_by(3) {
        h.cc(0, 74, v);
    }
    for v in 0..=127 {
        h.cc(0, 75, v);
    }
    assert_ne!(
        h.value(FILTER, "cutoff_hz"),
        cutoff,
        "the patch mapping moves"
    );
    assert_eq!(h.value(MACROS, "m1"), m1, "the profile does not");
    assert_eq!(
        h.value(MACROS, "m2"),
        m2,
        "a mapped button is not a profile knob"
    );
}

#[test]
fn a_macro_the_patch_maps_is_not_driven_by_the_profile() {
    let mut h = H::new();
    h.ui.midi_input = Some(PORT.into());
    h.profile(PORT, &[("m3", 74)]);
    h.editor
        .set_presentation(&[(MACROS, "cc.m3".into(), Some(10.0))]);
    let base = h.value(MACROS, "m3");
    for v in 0..=127 {
        h.cc(0, 74, v);
    }
    assert_eq!(h.value(MACROS, "m3"), base, "profile CC ignored");
    let at = h.at(MACROS, "m3");
    h.turn(0, 10, at, 10);
    assert_ne!(h.value(MACROS, "m3"), base, "the patch's own CC drives it");
}

#[test]
fn without_a_macro_module_the_profile_does_nothing() {
    let mut h = H::new();
    h.editor.remove_module(MACROS);
    h.profile(ANY_CONTROLLER, &[("m1", 74)]);
    let state = h.editor.state().clone();
    for v in 0..=127 {
        h.cc(0, 74, v);
    }
    assert_eq!(h.editor.state(), &state);
}

#[test]
fn lookup_falls_back_to_any_controller() {
    let mut profiles = std::collections::BTreeMap::new();
    assert_eq!(perform::knob_profile(&profiles, Some(PORT)), None);
    assert_eq!(perform::knob_profile(&profiles, None), None);
    let any: KnobProfile = [("m1".to_string(), 1)].into();
    let own: KnobProfile = [("m1".to_string(), 2)].into();
    profiles.insert(ANY_CONTROLLER.to_string(), any.clone());
    assert_eq!(perform::knob_profile(&profiles, Some(PORT)), Some(&any));
    assert_eq!(perform::knob_profile(&profiles, None), Some(&any));
    profiles.insert(PORT.to_string(), own.clone());
    assert_eq!(perform::knob_profile(&profiles, Some(PORT)), Some(&own));
    assert_eq!(perform::knob_profile(&profiles, Some("Other")), Some(&any));

    // The plugin has no port name: its CCs use the "any controller" profile.
    let mut h = H::new();
    h.ui.midi_input = None;
    h.profile(ANY_CONTROLLER, &[("m5", 30)]);
    h.profile(PORT, &[("m5", 31)]);
    let base = h.value(MACROS, "m5");
    let at = h.at(MACROS, "m5");
    h.turn(0, 31, at, 10);
    assert_eq!(
        h.value(MACROS, "m5"),
        base,
        "another port's profile is not used"
    );
    h.turn(0, 30, at, 10);
    assert!(h.value(MACROS, "m5") > base);
}

/// Opens Settings and starts "Set up macro knobs".
fn start_setup(h: &mut H) {
    if !h.has("knobs:setup") {
        h.click("view-menu");
    }
    h.click("knobs:setup");
    assert!(h.ui.knob_setup.is_some());
    // The menu resizes to the prompt over the next frames.
    h.frame();
    h.frame();
}

#[test]
fn setup_asks_for_each_knob_and_saves_for_the_port() {
    let mut h = H::new();
    let dir = tempfile::tempdir().unwrap();
    h.ui.prefs_dir = Some(dir.path().to_owned());
    h.ui.midi_input = Some(PORT.into());
    start_setup(&mut h);
    for k in 0..8u8 {
        assert_eq!(h.ui.knob_setup.as_ref().unwrap().slot, k as usize);
        // A whole turn of one knob takes one slot.
        for v in 0..20 {
            h.cc(0, 20 + k, v);
        }
        h.t += 1.5;
    }
    h.frame();
    assert!(h.ui.knob_setup.is_none(), "finished");
    let want: KnobProfile = (0..8)
        .map(|k| (format!("m{}", k + 1), 20 + k as u32))
        .collect();
    assert_eq!(h.ui.knob_profiles.get(PORT), Some(&want));
    let saved = kabl_ui::style::Prefs::load(dir.path());
    assert_eq!(saved.knob_profiles.get(PORT), Some(&want));
    // Setup CCs never touched the patch.
    assert_eq!(h.entries(), H::new().entries());
}

#[test]
fn setup_without_a_port_saves_any_controller() {
    let mut h = H::new();
    h.ui.midi_input = None;
    start_setup(&mut h);
    h.cc(0, 20, 64);
    for _ in 1..8 {
        h.click("knobs:skip");
    }
    assert!(h.ui.knob_setup.is_none());
    assert_eq!(
        h.ui.knob_profiles.get(ANY_CONTROLLER),
        Some(&[("m1".to_string(), 20)].into())
    );
}

#[test]
fn setup_the_same_cc_twice_moves_it_to_the_later_knob() {
    let mut h = H::new();
    h.ui.midi_input = Some(PORT.into());
    start_setup(&mut h);
    h.cc(0, 20, 1);
    h.t += 1.5;
    h.cc(0, 21, 1);
    h.t += 1.5;
    h.cc(0, 20, 2);
    let s = h.ui.knob_setup.clone().unwrap();
    assert_eq!(s.slot, 3);
    assert_eq!(s.knobs[..3], [None, Some((0, 21)), Some((0, 20))]);
}

#[test]
fn setup_skip_leaves_a_knob_empty() {
    let mut h = H::new();
    h.ui.midi_input = Some(PORT.into());
    start_setup(&mut h);
    h.click("knobs:skip");
    h.cc(0, 20, 1);
    for _ in 2..8 {
        h.click("knobs:skip");
    }
    assert!(h.ui.knob_setup.is_none());
    assert_eq!(
        h.ui.knob_profiles.get(PORT),
        Some(&[("m2".to_string(), 20)].into())
    );
}

#[test]
fn setup_cancel_keeps_the_stored_profile() {
    let mut h = H::new();
    let dir = tempfile::tempdir().unwrap();
    h.ui.prefs_dir = Some(dir.path().to_owned());
    h.ui.midi_input = Some(PORT.into());
    h.profile(PORT, &[("m1", 74)]);
    let before = h.ui.knob_profiles.clone();
    start_setup(&mut h);
    for k in 0..7 {
        h.cc(0, 30 + k, 1);
        h.t += 1.5;
    }
    h.click("knobs:cancel");
    assert!(h.ui.knob_setup.is_none());
    h.frame();
    assert_eq!(h.ui.knob_profiles, before);
    assert!(kabl_ui::style::Prefs::load(dir.path())
        .knob_profiles
        .is_empty());
    // Escape cancels too.
    start_setup(&mut h);
    h.events.push(Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    });
    h.frame();
    assert!(h.ui.knob_setup.is_none());
    assert_eq!(h.ui.knob_profiles, before);
}

#[test]
fn setup_and_midi_learn_cancel_each_other() {
    let mut h = H::with_perform(true);
    perform::toggle_pin(&mut h.editor, FILTER, "cutoff_hz");
    h.frame();
    h.frame();
    h.click("plearn:9.cutoff_hz");
    assert!(h.ui.learn.is_some());
    start_setup(&mut h);
    assert!(h.ui.learn.is_none(), "setup cancels the learn");
    // Learn starts from the panel while the setup prompt is open.
    assert!(h.has("knobs:skip"));
    h.click("plearn:9.cutoff_hz");
    assert!(h.ui.learn.is_some());
    assert!(h.ui.knob_setup.is_none(), "learn cancels the setup");
}

#[test]
fn settings_lists_the_profile_of_the_device_and_forgets_it() {
    let mut h = H::new();
    let dir = tempfile::tempdir().unwrap();
    h.ui.prefs_dir = Some(dir.path().to_owned());
    h.ui.midi_input = Some(PORT.into());
    h.click("view-menu");
    assert!(!h.has("knobs:forget"), "nothing to forget");
    h.ui.set_knob_profile(PORT, Some([("m1".to_string(), 74)].into()));
    h.ui.set_knob_profile(ANY_CONTROLLER, Some([("m2".to_string(), 75)].into()));
    h.frame();
    assert_eq!(
        perform::profile_knobs(&h.ui.knob_profiles[PORT]),
        vec![(1, (0, 74))]
    );
    h.click("knobs:forget");
    assert!(!h.ui.knob_profiles.contains_key(PORT));
    let saved = kabl_ui::style::Prefs::load(dir.path()).knob_profiles;
    assert!(!saved.contains_key(PORT));
    assert!(
        saved.contains_key(ANY_CONTROLLER),
        "only the device's profile goes"
    );
    // The fallback is listed next, and goes the same way.
    h.click("knobs:forget");
    assert!(h.ui.knob_profiles.is_empty());
    assert!(kabl_ui::style::Prefs::load(dir.path())
        .knob_profiles
        .is_empty());
    assert!(!h.has("knobs:forget"));
}
