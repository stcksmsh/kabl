//! The modulation flow through real egui input: pointer and key events go through `show()`,
//! egui's hit-testing and the production widgets, and the result is checked on the real
//! `PatchEditor` state. Targets are found through `UiState::hits`, the rects the UI drew.
//! Runs on the committed reference patch (`patches/reference`) at both target sizes.

use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect};
use kabl_core::ParamTarget;
use kabl_ui::routing::routes_into;
use kabl_ui::{show, CableView, PatchEditor, UiState};

const ENV: u64 = 4;
const FILTER: u64 = 3;
const FAST_LFO: u64 = 8;

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    size: egui::Vec2,
    events: Vec<Event>,
    modifiers: Modifiers,
    t: f64,
    pointer: Pos2,
}

impl H {
    fn new(w: f32, h: f32) -> Self {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/reference");
        let editor = PatchEditor::from_log(kabl_core::load(&dir).expect("reference patch"));
        let mut h = H {
            ctx: egui::Context::default(),
            editor,
            ui: UiState::default(),
            size: egui::vec2(w, h),
            events: Vec::new(),
            modifiers: Modifiers::NONE,
            t: 0.0,
            pointer: Pos2::ZERO,
        };
        h.frame();
        h.frame();
        h
    }

    fn frame(&mut self) {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            events: std::mem::take(&mut self.events),
            modifiers: self.modifiers,
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
        let _ = self.ctx.end_pass();
    }

    fn at(&self, key: &str) -> Pos2 {
        self.ui
            .hits
            .get(key)
            .unwrap_or_else(|| panic!("no hit target {key}"))
            .center()
    }

    fn move_to(&mut self, p: Pos2) {
        self.pointer = p;
        self.events.push(Event::PointerMoved(p));
        self.frame();
    }

    fn button(&mut self, pressed: bool) {
        self.events.push(Event::PointerButton {
            pos: self.pointer,
            button: PointerButton::Primary,
            pressed,
            modifiers: self.modifiers,
        });
        self.frame();
    }

    fn click(&mut self, key: &str) {
        let p = self.at(key);
        self.move_to(p);
        self.button(true);
        self.button(false);
        self.frame();
    }

    fn drag(&mut self, from: Pos2, to: Pos2) {
        self.move_to(from);
        self.button(true);
        for i in 1..=6 {
            let p = from + (to - from) * (i as f32 / 6.0);
            self.move_to(p);
        }
        self.button(false);
        self.frame();
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) {
        self.modifiers = modifiers;
        self.events.push(Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        });
        self.frame();
        self.events.push(Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers,
        });
        self.modifiers = Modifiers::NONE;
        self.frame();
    }

    fn routes(&self, id: u64, param: &str) -> Vec<(u64, f32, bool)> {
        routes_into(self.editor.state(), id, param)
            .iter()
            .map(|r| (r.cable, r.amount, r.bypass))
            .collect()
    }

    fn param(&self, id: u64, param: &str) -> Option<f32> {
        self.editor.state().param(&ParamTarget::Module {
            id,
            param: param.into(),
        })
    }

    /// Drags the ring band of `(id, param)` straight up by `dy` pixels.
    fn ring_drag(&mut self, id: u64, param: &str, dy: f32) {
        let p = self.at(&format!("ring:{id}.{param}"));
        self.drag(p, p - egui::vec2(0.0, dy));
    }
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

fn sizes() -> [(f32, f32); 2] {
    [(1440.0, 900.0), (1280.0, 800.0)]
}

#[test]
fn every_control_of_the_reference_patch_is_on_screen_left_of_the_drawer() {
    for (w, h) in sizes() {
        let harness = H::new(w, h);
        for (key, r) in &harness.ui.hits {
            if key.starts_with("knob:") || key.starts_with("out:") || key.starts_with("in:") {
                assert!(
                    r.max.x < w - 360.0 && r.max.y < h - 30.0 && r.min.y > 50.0,
                    "{w}x{h}: {key} at {r:?}"
                );
            }
        }
    }
}

#[test]
fn dragging_an_lfo_onto_a_knob_creates_and_selects_one_route() {
    for (w, h) in sizes() {
        let mut t = H::new(w, h);
        assert!(t.routes(ENV, "decay_ms").is_empty());
        let from = t.at(&format!("out:{FAST_LFO}.out"));
        let to = t.at(&format!("knob:{ENV}.decay_ms"));
        t.drag(from, to);
        let routes = t.routes(ENV, "decay_ms");
        assert_eq!(routes.len(), 1, "{w}x{h}");
        assert!(close(routes[0].1, 0.25), "default +25 %");
        assert_eq!(t.ui.selected_route, Some(routes[0].0));
        assert_eq!(t.ui.inspected, Some((ENV, "decay_ms".to_string())));
        assert!(t.ui.hits.contains_key(&format!("ring:{ENV}.decay_ms")));
        // One undo removes it; the selection goes with it.
        t.key(Key::Z, Modifiers::COMMAND);
        assert!(t.routes(ENV, "decay_ms").is_empty());
        assert_eq!(t.ui.selected_route, None);
        t.key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT);
        assert_eq!(t.routes(ENV, "decay_ms").len(), 1, "redo");
    }
}

#[test]
fn two_sources_ring_edits_only_the_selected_one() {
    let mut t = H::new(1440.0, 900.0);
    let before = t.routes(ENV, "attack_ms");
    assert_eq!(before.len(), 2);
    let base = t.param(ENV, "attack_ms");

    // Several sources and nothing selected: the ring edits nothing.
    t.click(&format!("knob:{ENV}.attack_ms"));
    assert_eq!(t.ui.selected_route, None);
    t.ring_drag(ENV, "attack_ms", 30.0);
    assert_eq!(t.routes(ENV, "attack_ms"), before, "no silent edit");

    // Select the second source in the drawer; the ring moves only it (+30 px = +20 %).
    let a2 = before[1].0;
    t.click(&format!("row:{a2}"));
    assert_eq!(t.ui.selected_route, Some(a2));
    t.ring_drag(ENV, "attack_ms", 30.0);
    let after = t.routes(ENV, "attack_ms");
    assert_eq!(after[0], before[0], "first route untouched");
    assert!(close(after[1].1, before[1].1 + 0.2), "{:?}", after[1]);
    assert_eq!(t.param(ENV, "attack_ms"), base, "base untouched");

    // Knob body edits the base, never a route.
    let c = t.at(&format!("knob:{ENV}.attack_ms"));
    t.drag(c, c - egui::vec2(0.0, 15.0));
    assert_ne!(t.param(ENV, "attack_ms"), base);
    assert_eq!(t.routes(ENV, "attack_ms"), after);

    // Undo body drag, then the ring drag: back to the original amount.
    t.key(Key::Z, Modifiers::COMMAND);
    assert_eq!(t.param(ENV, "attack_ms"), base);
    t.key(Key::Z, Modifiers::COMMAND);
    assert_eq!(t.routes(ENV, "attack_ms"), before);
}

#[test]
fn invert_bypass_remove_through_the_drawer() {
    let mut t = H::new(1440.0, 900.0);
    let before = t.routes(ENV, "attack_ms");
    let a1 = before[0].0;
    t.click(&format!("knob:{ENV}.attack_ms"));
    t.click(&format!("row:{a1}"));

    t.click(&format!("invert:{a1}"));
    assert!(close(t.routes(ENV, "attack_ms")[0].1, -before[0].1));
    t.click(&format!("bypass:{a1}"));
    assert!(t.routes(ENV, "attack_ms")[0].2, "bypassed");
    assert!(
        close(t.routes(ENV, "attack_ms")[0].1, -before[0].1),
        "amount kept"
    );

    // Removing the selected route clears the selection; the ring then edits nothing,
    // not the remaining route.
    t.click(&format!("remove:{a1}"));
    assert_eq!(t.routes(ENV, "attack_ms").len(), 1);
    assert_eq!(t.ui.selected_route, None);
    let remaining = t.routes(ENV, "attack_ms");
    t.ring_drag(ENV, "attack_ms", 30.0);
    assert_eq!(t.routes(ENV, "attack_ms"), remaining);

    // Undo remove, bypass, invert: exactly the original routes again, same ids.
    for _ in 0..3 {
        t.key(Key::Z, Modifiers::COMMAND);
    }
    assert_eq!(t.routes(ENV, "attack_ms"), before);
}

#[test]
fn four_sources_each_selectable_and_edited_alone() {
    let mut t = H::new(1280.0, 800.0);
    let before = t.routes(FILTER, "cutoff_hz");
    assert_eq!(before.len(), 4);
    t.click(&format!("knob:{FILTER}.cutoff_hz"));
    assert_eq!(t.ui.selected_route, None);
    let mut expect = before.clone();
    for i in 0..4 {
        t.click(&format!("row:{}", before[i].0));
        assert_eq!(t.ui.selected_route, Some(before[i].0));
        t.ring_drag(FILTER, "cutoff_hz", 15.0);
        expect[i].1 += 0.1;
        let now = t.routes(FILTER, "cutoff_hz");
        for (a, b) in now.iter().zip(&expect) {
            assert_eq!(a.0, b.0);
            assert!(close(a.1, b.1), "route {} {} vs {}", a.0, a.1, b.1);
        }
    }
}

#[test]
fn envelope_timing_selector_sets_and_undoes() {
    let mut t = H::new(1440.0, 900.0);
    assert_eq!(t.param(ENV, "timing"), None);
    t.click(&format!("sel:{ENV}.timing.1"));
    assert_eq!(t.param(ENV, "timing"), Some(1.0));
    t.click(&format!("sel:{ENV}.timing.0"));
    assert_eq!(t.param(ENV, "timing"), Some(0.0));
    t.key(Key::Z, Modifiers::COMMAND);
    t.key(Key::Z, Modifiers::COMMAND);
    assert_eq!(t.param(ENV, "timing"), None, "back to default (absent)");
}

#[test]
fn base_value_entry_in_the_drawer() {
    let mut t = H::new(1440.0, 900.0);
    t.click(&format!("knob:{ENV}.attack_ms"));
    t.click("base-entry");
    t.key(Key::A, Modifiers::COMMAND);
    t.events.push(Event::Text("25 ms".into()));
    t.frame();
    t.key(Key::Enter, Modifiers::NONE);
    assert_eq!(t.param(ENV, "attack_ms"), Some(25.0));
}

#[test]
fn cable_views_keep_positions_and_routes() {
    let mut t = H::new(1440.0, 900.0);
    let knobs: Vec<(String, Rect)> =
        t.ui.hits
            .iter()
            .filter(|(k, _)| k.starts_with("knob:"))
            .map(|(k, r)| (k.clone(), *r))
            .collect();
    let has_route_cables = |t: &H| t.ui.hits.keys().any(|k| k.starts_with("route:"));
    assert!(has_route_cables(&t));
    for view in ["Hidden", "Focus", "All"] {
        t.click(&format!("view:{view}"));
        for (k, r) in &knobs {
            assert_eq!(t.ui.hits.get(k), Some(r), "{view}: {k} did not move");
        }
        assert_eq!(has_route_cables(&t), view != "Hidden", "{view}");
    }
    assert_eq!(t.ui.cable_view, CableView::All);

    // Hidden mode still edits the selected source through the ring.
    t.click("view:Hidden");
    t.click(&format!("knob:{ENV}.attack_ms"));
    let before = t.routes(ENV, "attack_ms");
    t.click(&format!("row:{}", before[0].0));
    t.ring_drag(ENV, "attack_ms", 15.0);
    assert!(close(t.routes(ENV, "attack_ms")[0].1, before[0].1 + 0.1));
}

#[test]
fn clicking_a_route_cable_selects_it_and_never_deletes() {
    let mut t = H::new(1440.0, 900.0);
    let before = t.routes(ENV, "attack_ms");
    let key = format!("route:{}", before[1].0);
    t.click(&key);
    assert_eq!(t.routes(ENV, "attack_ms"), before);
    assert_eq!(t.ui.selected_route, Some(before[1].0));
    assert_eq!(t.ui.inspected, Some((ENV, "attack_ms".to_string())));
}

#[test]
fn inspecting_a_single_source_knob_selects_it_automatically() {
    let mut t = H::new(1440.0, 900.0);
    let from = t.at(&format!("out:{FAST_LFO}.out"));
    let to = t.at(&format!("knob:{ENV}.release_ms"));
    t.drag(from, to);
    let only = t.routes(ENV, "release_ms")[0].0;
    t.click(&format!("knob:{ENV}.attack_ms")); // two sources: nothing selected
    assert_eq!(t.ui.selected_route, None);
    t.click(&format!("knob:{ENV}.release_ms"));
    assert_eq!(t.ui.selected_route, Some(only));
    t.ring_drag(ENV, "release_ms", 15.0);
    assert!(close(t.routes(ENV, "release_ms")[0].1, 0.35));
}

/// Not a check: writes the target rects for the scripted real-X (`xdotool`) pass to
/// `target/slice-shots/hits-WxH.txt`. `cargo test -p kabl-ui --test interaction -- --ignored`.
#[test]
#[ignore]
fn dump_hit_targets_for_real_input_runs() {
    for (w, h) in sizes() {
        let mut t = H::new(w, h);
        let mut all = t.ui.hits.clone();
        for knob in [
            format!("knob:{ENV}.attack_ms"),
            format!("knob:{FILTER}.cutoff_hz"),
        ] {
            t.click(&knob);
            all.extend(t.ui.hits.clone());
        }
        let mut out = String::new();
        for (k, r) in all {
            out.push_str(&format!("{k} {:.0} {:.0}\n", r.center().x, r.center().y));
        }
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/slice-shots");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("hits-{w}x{h}.txt")), out).unwrap();
    }
}

#[test]
fn selecting_a_source_does_not_move_the_drawer_rows() {
    let mut t = H::new(1440.0, 900.0);
    t.click(&format!("knob:{ENV}.attack_ms"));
    let rows: Vec<(String, Rect)> =
        t.ui.hits
            .iter()
            .filter(|(k, _)| k.starts_with("invert:") || k.starts_with("row:"))
            .map(|(k, r)| (k.clone(), *r))
            .collect();
    let first = rows
        .iter()
        .find(|(k, _)| k.starts_with("row:"))
        .unwrap()
        .0
        .clone();
    t.click(&first);
    for (k, r) in rows {
        assert_eq!(t.ui.hits.get(&k), Some(&r), "{k} moved");
    }
}

#[test]
fn source_lanes_select_and_edit_each_route_on_the_knob() {
    for (w, h) in sizes() {
        let mut t = H::new(w, h);
        assert!(
            !t.ui.hits.keys().any(|k| k.starts_with("lane:")),
            "hidden until inspected"
        );
        t.click(&format!("knob:{FILTER}.cutoff_hz"));
        let before = t.routes(FILTER, "cutoff_hz");
        for (i, r) in before.iter().enumerate() {
            assert!(
                t.ui.hits.contains_key(&format!("lane:{}", r.0)),
                "{w}x{h} lane {i}"
            );
        }
        let mut expect = before.clone();
        for i in [2, 0, 3, 1] {
            let cable = before[i].0;
            let p = t.at(&format!("lane:{cable}"));
            t.drag(p, p - egui::vec2(0.0, 15.0));
            expect[i].1 += 0.1;
            assert_eq!(t.ui.selected_route, Some(cable));
            for (a, b) in t.routes(FILTER, "cutoff_hz").iter().zip(&expect) {
                assert!(close(a.1, b.1), "{w}x{h}: route {} {} vs {}", a.0, a.1, b.1);
            }
        }
        // Base untouched by lane drags; one undo reverts one lane drag.
        assert_eq!(t.param(FILTER, "cutoff_hz"), Some(1400.0));
        t.key(Key::Z, Modifiers::COMMAND);
        expect[1].1 -= 0.1;
        for (a, b) in t.routes(FILTER, "cutoff_hz").iter().zip(&expect) {
            assert!(close(a.1, b.1));
        }
        // Clicking empty canvas closes the lanes.
        t.move_to(egui::pos2(300.0, h - 60.0));
        t.button(true);
        t.button(false);
        t.frame();
        assert_eq!(t.ui.inspected, None);
        assert!(!t.ui.hits.keys().any(|k| k.starts_with("lane:")));
    }
}
