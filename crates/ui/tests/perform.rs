//! The performance panel and MIDI CC learn through real egui input on `patches/echo`: pins
//! are real parameters (one shared value with the rack), configuration edits never rebuild
//! audio, pins and mappings save and undo, deleted targets leave nothing behind, soft takeover
//! and gesture grouping behave, and the panel reads at both window sizes.

use egui::{Event, Pos2, RawInput, Rect};
use kabl_core::ParamTarget;
use kabl_modules::builtins::Transport;
use kabl_ui::perform::{self, pins, TRANSPORT};
use kabl_ui::{routing, show, PatchEditor, UiState};

const CLOCK: u64 = 1;
const FILTER: u64 = 9;
const DELAY: u64 = 16;

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    size: egui::Vec2,
    events: Vec<Event>,
    t: f64,
    pointer: Pos2,
}

impl H {
    fn new(w: f32, h: f32) -> Self {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/echo");
        let mut editor = PatchEditor::from_log(kabl_core::load(&dir).expect("patch"));
        // These pin-authoring tests start from an unprepared view of the real graph.
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
        let mut h = H {
            ctx: egui::Context::default(),
            editor,
            ui: UiState::default(),
            size: egui::vec2(w, h),
            events: Vec::new(),
            t: 0.0,
            pointer: Pos2::ZERO,
        };
        h.ui.perform_open = true;
        h.frame();
        h.frame();
        h
    }

    fn frame(&mut self) {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            events: std::mem::take(&mut self.events),
            time: Some(self.t),
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
        // What the control thread does every few milliseconds.
        perform::rearm(&mut self.ui, self.t);
        let _ = self.ctx.end_pass();
    }

    fn rect(&self, key: &str) -> Rect {
        *self
            .ui
            .hits
            .get(key)
            .unwrap_or_else(|| panic!("no hit target {key}"))
    }

    fn move_to(&mut self, p: Pos2) {
        self.pointer = p;
        self.events.push(Event::PointerMoved(p));
        self.frame();
    }

    fn button(&mut self, pressed: bool) {
        self.events.push(Event::PointerButton {
            pos: self.pointer,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        });
        self.frame();
    }

    fn click(&mut self, key: &str) {
        self.move_to(self.rect(key).center());
        self.button(true);
        self.button(false);
        self.frame();
    }

    /// A CC through the control layer (no frame needed), then a frame to draw it.
    fn cc(&mut self, ch: u8, cc: u8, v: u8) {
        perform::apply_cc(&mut self.editor, &mut self.ui, &[(ch, cc, v)], self.t);
        self.t += 0.02;
        self.frame();
    }

    fn value(&self, id: u64, param: &str) -> f32 {
        let info = kabl_modules::registry::info_for(&self.editor.state().modules[&id].kind)
            .unwrap()
            .params
            .iter()
            .find(|p| p.name == param)
            .unwrap();
        routing::base_value(self.editor.state(), id, info)
    }

    fn pin(&mut self, id: u64, key: &str) {
        perform::toggle_pin(&mut self.editor, id, key);
        self.frame();
        self.frame();
    }
}

fn sizes() -> [(f32, f32); 2] {
    [(1280.0, 800.0), (1440.0, 900.0)]
}

#[test]
fn pins_are_presentation_edits_that_save_undo_and_reorder() {
    let mut h = H::new(1440.0, 900.0);
    h.editor.take_dirty();
    h.pin(FILTER, "cutoff_hz");
    h.pin(DELAY, "mix");
    h.pin(CLOCK, TRANSPORT);
    assert!(!h.editor.take_dirty(), "pinning must not rebuild audio");
    let keys = |h: &H| {
        pins(h.editor.state())
            .iter()
            .map(|p| format!("{}.{}", p.id, p.key))
            .collect::<Vec<_>>()
    };
    assert_eq!(keys(&h), ["9.cutoff_hz", "16.mix", "1.transport"]);
    for k in &keys(&h) {
        h.rect(&format!("pcard:{k}"));
    }
    h.click("pmenu:1.transport");
    h.click("pleft:1.transport");
    assert_eq!(keys(&h), ["9.cutoff_hz", "1.transport", "16.mix"]);
    h.click("pmenu:9.cutoff_hz");
    h.click("pright:9.cutoff_hz");
    assert_eq!(keys(&h), ["1.transport", "9.cutoff_hz", "16.mix"]);
    assert!(!h.editor.take_dirty());
    h.editor.undo();
    assert_eq!(keys(&h), ["9.cutoff_hz", "1.transport", "16.mix"]);
    h.editor.redo();

    // Save and reload keep the order.
    let dir = tempfile::tempdir().unwrap();
    kabl_core::save(dir.path(), h.editor.log()).unwrap();
    let back = PatchEditor::from_log(kabl_core::load(dir.path()).unwrap());
    assert_eq!(
        pins(back.state()),
        pins(h.editor.state()),
        "pins survive save/reload"
    );

    h.click("pmenu:16.mix");
    h.click("punpin:16.mix");
    assert_eq!(keys(&h), ["1.transport", "9.cutoff_hz"]);
    assert!(!h.editor.take_dirty());
}

#[test]
fn a_pinned_slider_edits_the_real_parameter_with_one_undo_step() {
    let mut h = H::new(1280.0, 800.0);
    h.pin(FILTER, "cutoff_hz");
    let before = h.value(FILTER, "cutoff_hz");
    let r = h.rect("pslider:9.cutoff_hz");
    let entries = h.editor.log().entries().len();
    h.move_to(r.left_center() + egui::vec2(20.0, 0.0));
    h.button(true);
    for k in 1..8 {
        h.move_to(r.left_center() + egui::vec2(20.0 + 12.0 * k as f32, 0.0));
    }
    h.button(false);
    let after = h.value(FILTER, "cutoff_hz");
    assert_ne!(after, before);
    assert!(h.editor.take_dirty(), "a value edit rebuilds audio");
    assert_eq!(h.editor.log().entries().len(), entries + 1, "one undo step");
    // The rack reads the same stored value (no duplicate state).
    assert_eq!(h.editor.state().modules[&FILTER].params["cutoff_hz"], after);
    h.editor.undo();
    assert_eq!(h.value(FILTER, "cutoff_hz"), before);
}

#[test]
fn transport_card_sends_runtime_commands_only() {
    let mut h = H::new(1440.0, 900.0);
    h.pin(CLOCK, TRANSPORT);
    let entries = h.editor.log().entries().len();
    h.click("prun:1");
    h.click("prestart:1");
    assert_eq!(
        h.ui.transport,
        [(CLOCK, Transport::Stop), (CLOCK, Transport::Restart)]
    );
    assert_eq!(h.editor.log().entries().len(), entries);
}

#[test]
fn deleting_a_target_removes_its_pins_and_mappings_and_undo_restores_them() {
    let mut h = H::new(1440.0, 900.0);
    h.pin(FILTER, "cutoff_hz");
    h.click("plearn:9.cutoff_hz");
    h.cc(0, 74, 30);
    assert_eq!(
        perform::mapping(h.editor.state(), FILTER, "cutoff_hz"),
        Some((0, 74))
    );
    h.editor.remove_module(FILTER);
    h.frame();
    h.frame();
    assert!(pins(h.editor.state()).is_empty());
    assert!(!h.ui.hits.contains_key("pcard:9.cutoff_hz"));
    assert!(h.ui.takeover.is_empty(), "no stale takeover");
    // A CC for the deleted mapping does nothing (and doesn't panic).
    h.cc(0, 74, 90);
    h.editor.undo();
    h.frame();
    assert_eq!(pins(h.editor.state()).len(), 1);
    assert_eq!(
        perform::mapping(h.editor.state(), FILTER, "cutoff_hz"),
        Some((0, 74))
    );
}

#[test]
fn learn_pickup_gestures_and_reassignment() {
    let mut h = H::new(1440.0, 900.0);
    h.pin(FILTER, "cutoff_hz");
    h.pin(DELAY, "feedback");
    h.editor.take_dirty();
    // Escape cancels a learn.
    h.click("plearn:9.cutoff_hz");
    assert!(h.ui.learn.is_some());
    h.events.push(Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    });
    h.frame();
    assert!(h.ui.learn.is_none());

    h.click("plearn:9.cutoff_hz");
    let base = h.value(FILTER, "cutoff_hz");
    let p = kabl_modules::registry::info_for("filter.svf")
        .unwrap()
        .params
        .iter()
        .find(|p| p.name == "cutoff_hz")
        .unwrap();
    let at = (p.to_norm(base) * 127.0) as u8;
    // Learn with the hardware well below the value: mapped, value untouched, pickup pending.
    h.cc(2, 20, 5);
    assert_eq!(
        perform::mapping(h.editor.state(), FILTER, "cutoff_hz"),
        Some((2, 20))
    );
    assert!(!h.editor.take_dirty(), "learning never rebuilds audio");
    assert_eq!(h.value(FILTER, "cutoff_hz"), base);
    h.rect("ppickup:9.cutoff_hz");
    assert!(!h.ui.takeover[&(FILTER, "cutoff_hz".to_string())].picked);
    // Another channel's CC 20 is not this mapping.
    h.cc(3, 20, at);
    assert_eq!(h.value(FILTER, "cutoff_hz"), base);
    // Turning up below the value: no jump.
    for v in (5..at.saturating_sub(4)).step_by(6) {
        h.cc(2, 20, v);
        assert_eq!(h.value(FILTER, "cutoff_hz"), base, "jumped at {v}");
    }
    // Crossing it picks up; the continuing turn is one undo step.
    let entries = h.editor.log().entries().len();
    for v in (at.saturating_sub(2)..=(at + 30).min(127)).step_by(3) {
        h.cc(2, 20, v);
    }
    let turned = h.value(FILTER, "cutoff_hz");
    assert!(turned > base);
    assert!(h.ui.takeover[&(FILTER, "cutoff_hz".to_string())].picked);
    assert_eq!(h.editor.log().entries().len(), entries + 1);
    assert!(h.editor.take_dirty());
    // Undo goes back past the whole turn, and drops the pickup: the next CC can't jump.
    h.editor.undo();
    h.frame();
    assert_eq!(h.value(FILTER, "cutoff_hz"), base);
    assert!(!h.ui.takeover[&(FILTER, "cutoff_hz".to_string())].picked);
    h.cc(2, 20, 127);
    assert_eq!(h.value(FILTER, "cutoff_hz"), base);
    // A pause longer than a second starts a new undo step.
    h.cc(2, 20, at);
    h.cc(2, 20, at + 1);
    let entries = h.editor.log().entries().len();
    h.t += 1.5;
    h.cc(2, 20, at + 5);
    assert_eq!(h.editor.log().entries().len(), entries + 1);

    // Reassigning CC 20 to the delay feedback takes it from the filter.
    h.click("plearn:16.feedback");
    h.cc(2, 20, 60);
    assert_eq!(
        perform::mapping(h.editor.state(), FILTER, "cutoff_hz"),
        None
    );
    assert_eq!(
        perform::mapping(h.editor.state(), DELAY, "feedback"),
        Some((2, 20))
    );
    // Clear removes it; saved mappings reload.
    let dir = tempfile::tempdir().unwrap();
    kabl_core::save(dir.path(), h.editor.log()).unwrap();
    let back = PatchEditor::from_log(kabl_core::load(dir.path()).unwrap());
    assert_eq!(
        perform::mapping(back.state(), DELAY, "feedback"),
        Some((2, 20))
    );
    h.click("pclear:16.feedback");
    assert_eq!(perform::mapping(h.editor.state(), DELAY, "feedback"), None);
}

#[test]
fn a_mouse_edit_drops_the_pickup() {
    let mut h = H::new(1440.0, 900.0);
    h.pin(DELAY, "mix");
    h.click("plearn:16.mix");
    let at = (h.value(DELAY, "mix") / 100.0 * 127.0).round() as u8;
    h.cc(0, 7, at);
    h.cc(0, 7, at + 10);
    assert!(h.ui.takeover[&(DELAY, "mix".to_string())].picked);
    let now = h.value(DELAY, "mix");
    // The same parameter moved from the rack side.
    h.editor.set_param_gesture(
        ParamTarget::Module {
            id: DELAY,
            param: "mix".into(),
        },
        10.0,
        true,
    );
    h.frame();
    assert!(!h.ui.takeover[&(DELAY, "mix".to_string())].picked);
    h.cc(0, 7, at + 11);
    assert_eq!(h.value(DELAY, "mix"), 10.0, "hardware far away: no jump");
    assert_ne!(now, 10.0);
}

#[test]
fn modulation_routes_are_kept_when_a_cc_moves_the_base() {
    let mut h = H::new(1440.0, 900.0);
    // The echo patch's LFO route on the delay time.
    let routes = |h: &H| {
        h.editor
            .state()
            .cables
            .values()
            .filter(|c| matches!(&c.to, kabl_core::PortRef::Param { id, .. } if *id == DELAY))
            .count()
    };
    let n = routes(&h);
    assert!(n > 0);
    h.pin(DELAY, "time_ms");
    h.click("plearn:16.time_ms");
    h.cc(0, 1, 0);
    h.cc(0, 1, 127);
    assert_eq!(h.value(DELAY, "time_ms"), 4000.0);
    assert_eq!(routes(&h), n);
}

#[test]
fn the_panel_reads_at_both_sizes_whatever_the_rack_zoom() {
    for (w, hgt) in sizes() {
        let mut h = H::new(w, hgt);
        h.pin(CLOCK, TRANSPORT);
        h.pin(FILTER, "cutoff_hz");
        h.pin(DELAY, "mix");
        h.pin(DELAY, "feedback");
        let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(w, hgt));
        let cards: Vec<Rect> = ["1.transport", "9.cutoff_hz", "16.mix", "16.feedback"]
            .iter()
            .map(|k| h.rect(&format!("pcard:{k}")))
            .collect();
        for (i, c) in cards.iter().enumerate() {
            assert!(screen.contains_rect(*c), "{w}: card {i} {c:?} off screen");
            assert!(c.width() >= 138.0 && c.height() >= 100.0, "{c:?}");
            for d in &cards[i + 1..] {
                assert!(!c.intersects(*d));
            }
        }
        // Rack zoom leaves the panel alone.
        h.click("zoom:out");
        h.click("zoom:out");
        let again: Vec<Rect> = ["1.transport", "9.cutoff_hz", "16.mix", "16.feedback"]
            .iter()
            .map(|k| h.rect(&format!("pcard:{k}")))
            .collect();
        assert_eq!(cards, again);
        for key in ["midi-input", "perform", "routing"] {
            assert!(screen.contains_rect(h.rect(key)), "{key}");
        }
    }
}

#[test]
fn a_new_pin_scrolls_into_view() {
    let mut h = H::new(1280.0, 800.0);
    // Enough pins to overflow the panel, then one more from the module menu path.
    for (id, k) in [
        (FILTER, "cutoff_hz"),
        (FILTER, "resonance"),
        (DELAY, "mix"),
        (DELAY, "feedback"),
        (DELAY, "time_ms"),
        (DELAY, "tone_hz"),
        (CLOCK, "bpm"),
    ] {
        h.pin(id, k);
    }
    perform::toggle_pin(&mut h.editor, CLOCK, TRANSPORT);
    // What pinning from the module menu sets.
    h.ui.pin_reveal = Some((CLOCK, TRANSPORT.into()));
    for _ in 0..40 {
        h.frame();
    }
    let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(1280.0, 800.0));
    assert!(screen.contains_rect(h.rect("pcard:1.transport")));
}

/// A key press through egui.
fn key(h: &mut H, key: egui::Key) {
    for pressed in [true, false] {
        h.events.push(Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Default::default(),
        });
    }
    h.frame();
}

#[test]
fn pins_take_labels_that_undo_save_and_keep_their_source() {
    let mut h = H::new(1440.0, 900.0);
    h.pin(FILTER, "cutoff_hz");
    h.editor.take_dirty();
    // Double-click the title, type, Enter.
    let r = h.rect("plabel:9.cutoff_hz");
    h.move_to(r.center());
    for _ in 0..2 {
        h.button(true);
        h.button(false);
    }
    h.frame();
    assert!(h.ui.renaming.is_some());
    h.events.push(Event::Key {
        key: egui::Key::A,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    });
    h.events.push(Event::Text("Bass cutoff".into()));
    h.frame();
    key(&mut h, egui::Key::Enter);
    h.frame();
    let pin = &pins(h.editor.state())[0];
    assert_eq!(perform::pin_label(h.editor.state(), pin), "Bass cutoff");
    assert!(perform::pin_source(h.editor.state(), pin).contains("SVF Filter #9 · Cutoff"));
    assert!(!h.editor.take_dirty(), "a label never rebuilds audio");
    let dir = tempfile::tempdir().unwrap();
    kabl_core::save(dir.path(), h.editor.log()).unwrap();
    let back = PatchEditor::from_log(kabl_core::load(dir.path()).unwrap());
    assert_eq!(perform::pin_label(back.state(), pin), "Bass cutoff");
    h.editor.undo();
    assert_eq!(perform::pin_label(h.editor.state(), pin), "Cutoff");
    // Escape cancels an edit.
    h.ui.renaming = Some((FILTER, "cutoff_hz".into(), "nope".into()));
    h.frame();
    key(&mut h, egui::Key::Escape);
    assert_eq!(perform::pin_label(h.editor.state(), pin), "Cutoff");
}

#[test]
fn simultaneous_cc_turns_undo_together_and_keep_final_values() {
    let mut h = H::new(1440.0, 900.0);
    h.pin(DELAY, "mix");
    h.pin(DELAY, "feedback");
    h.click("plearn:16.mix");
    let mix0 = (h.value(DELAY, "mix") / 100.0 * 127.0).round() as u8;
    h.cc(0, 1, mix0);
    h.click("plearn:16.feedback");
    let fb0 = (h.value(DELAY, "feedback") / 95.0 * 127.0).round() as u8;
    h.cc(0, 2, fb0);
    let (mix_before, fb_before) = (h.value(DELAY, "mix"), h.value(DELAY, "feedback"));
    let entries = h.editor.log().entries().len();
    // Both knobs turned together: messages interleave, several per frame.
    for k in 1..=20u8 {
        let t = h.t;
        perform::apply_cc(
            &mut h.editor,
            &mut h.ui,
            &[(0, 1, mix0 + k), (0, 2, fb0 - k)],
            t,
        );
        h.t += 0.01;
        h.frame();
    }
    let (mix, fb) = (h.value(DELAY, "mix"), h.value(DELAY, "feedback"));
    let p = |kind: &str, name: &str| {
        *kabl_modules::registry::info_for(kind)
            .unwrap()
            .params
            .iter()
            .find(|p| p.name == name)
            .unwrap()
    };
    assert_eq!(mix, p("delay", "mix").from_norm((mix0 + 20) as f32 / 127.0));
    assert_eq!(
        fb,
        p("delay", "feedback").from_norm((fb0 - 20) as f32 / 127.0)
    );
    assert_eq!(h.editor.log().entries().len(), entries + 1, "one undo step");
    h.editor.undo();
    assert_eq!(
        (h.value(DELAY, "mix"), h.value(DELAY, "feedback")),
        (mix_before, fb_before)
    );
    h.editor.redo();
    assert_eq!(
        (h.value(DELAY, "mix"), h.value(DELAY, "feedback")),
        (mix, fb)
    );
}

#[test]
fn the_demo_controls_fit_together_at_1280() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/performance");
    for (w, hgt) in sizes() {
        let mut h = H::new(w, hgt);
        h.editor = PatchEditor::from_log(kabl_core::load(&dir).unwrap());
        // The full Perform view: the dense layout needs the whole window, not a docked strip.
        h.ui.perform_tall = true;
        let (rec, tap) = kabl_ui::record::pair(48000);
        h.ui.recorder = Some(rec);
        std::mem::forget(tap);
        for _ in 0..4 {
            h.frame();
        }
        let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(w, hgt));
        let all = pins(h.editor.state());
        assert_eq!(all.len(), 12);
        let mut rects: Vec<Rect> = Vec::new();
        for p in &all {
            let r = h.rect(&format!("pcard:{}.{}", p.id, p.key));
            assert!(screen.contains_rect(r), "{w}: {}.{} at {r:?}", p.id, p.key);
            for q in &rects {
                assert!(!q.shrink(1.0).intersects(r), "{w}: overlap");
            }
            rects.push(r);
        }
        for k in [
            "rec-start",
            "midi-input",
            "notes-off",
            "prun:1",
            "prestart:1",
        ] {
            assert!(screen.contains_rect(h.rect(k)), "{w}: {k}");
        }
    }
}

#[test]
fn the_toolbar_switch_opens_perform_as_a_view_and_show_rack_docks_it() {
    let mut h = H::new(1280.0, 800.0);
    h.ui.perform_open = false;
    h.frame();
    h.pin(CLOCK, TRANSPORT);
    h.click("perform");
    assert!(h.ui.perform_open && h.ui.perform_tall, "Perform is a view");
    assert!(h.ui.hits.contains_key("pcard:1.transport"));
    assert!(
        !h.ui.hits.keys().any(|k| k.starts_with("knob:")),
        "the rack is hidden in the view"
    );
    h.click("perform-size");
    assert!(
        h.ui.perform_open && !h.ui.perform_tall,
        "docked under the rack"
    );
    assert!(
        h.ui.hits.keys().any(|k| k.starts_with("knob:")),
        "rack and Perform together"
    );
    assert!(h.ui.hits.contains_key("pcard:1.transport"));
    h.click("rack");
    assert!(!h.ui.perform_open);
}

#[test]
fn a_macro_knob_edits_the_real_parameter_with_one_undo_step() {
    const MACROS: u64 = 18;
    let mut h = H::new(1440.0, 900.0);
    h.ui.perform_tall = true;
    h.pin(MACROS, "m1");
    let before = h.value(MACROS, "m1");
    let r = h.rect(&format!("pslider:{MACROS}.m1"));
    let entries = h.editor.log().entries().len();
    h.move_to(r.center());
    h.button(true);
    for k in 1..8 {
        h.move_to(r.center() + egui::vec2(0.0, -6.0 * k as f32));
    }
    h.button(false);
    let after = h.value(MACROS, "m1");
    assert!(after > before, "dragging up raises the macro");
    assert_eq!(h.editor.log().entries().len(), entries + 1, "one undo step");
    h.editor.undo();
    assert_eq!(h.value(MACROS, "m1"), before);
}

#[test]
fn the_full_view_takes_the_drawer_unless_pinned_and_gives_it_back() {
    let mut h = H::new(1440.0, 900.0);
    h.ui.perform_open = false;
    h.ui.drawer_open = true;
    h.frame();
    h.pin(CLOCK, TRANSPORT);
    h.click("perform");
    assert!(
        !h.ui.drawer_open,
        "entering the full view closes the drawer"
    );
    h.click("rack");
    assert!(h.ui.drawer_open, "back in the rack it returns");

    // Pinned from inside Perform: the drawer comes back and stays through the trip.
    h.click("perform");
    assert!(!h.ui.drawer_open);
    h.click("drawer-pin");
    assert!(h.ui.drawer_pinned && h.ui.drawer_open);
    h.click("rack");
    assert!(h.ui.drawer_open);
    h.click("perform");
    assert!(h.ui.drawer_open, "a pinned drawer stays open");
    h.click("rack");
    assert!(h.ui.drawer_open);

    // A drawer the user closed stays closed, and one opened inside Perform stays open.
    h.ui.drawer_pinned = false;
    h.click("routing");
    assert!(!h.ui.drawer_open);
    h.click("perform");
    h.click("routing");
    assert!(h.ui.drawer_open, "opened by hand inside Perform");
    h.click("rack");
    assert!(h.ui.drawer_open);
}

#[test]
fn scenes_and_macros_use_the_width_at_every_size() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/composition");
    for (w, hgt) in [(1280.0, 800.0), (1440.0, 900.0)] {
        let mut h = H::new(w, hgt);
        h.editor = PatchEditor::from_log(kabl_core::load(&dir).unwrap());
        h.ui.perform_open = true;
        h.ui.perform_tall = true;
        h.ui.drawer_open = false;
        for _ in 0..4 {
            h.frame();
        }
        let right = w - 44.0;
        let scenes =
            h.ui.hits
                .iter()
                .find(|(k, _)| k.ends_with(".cues"))
                .map(|(_, r)| *r)
                .expect("scenes card");
        assert!(
            right - scenes.right() < 30.0,
            "{w}: scenes end at {}",
            scenes.right()
        );
        // The bank cards of the second row also reach the right edge.
        let last =
            h.ui.hits
                .iter()
                .filter(|(k, _)| k.starts_with("pcard:") && k.ends_with(".banks"))
                .map(|(_, r)| r.right())
                .fold(0.0, f32::max);
        assert!(right - last < 30.0, "{w}: banks end at {last}");
        // Pads read from a distance, and the first control row stays above the fold.
        assert!(
            scenes.height() >= 150.0,
            "{w}: scene row {}",
            scenes.height()
        );
        let ctrl_bottom = h
            .ui
            .hits
            .iter()
            .filter(|(k, _)| k.starts_with("pcard:") && !k.ends_with(".banks") && !k.contains(".m"))
            .map(|(_, r)| r.bottom())
            .fold(0.0, f32::max);
        assert!(
            ctrl_bottom <= hgt - 20.0,
            "{w}: controls end at {ctrl_bottom}"
        );
    }
}
