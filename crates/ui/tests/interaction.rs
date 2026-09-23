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

    /// A point on bare rack: inside the canvas, outside every module and cable target.
    fn empty_rack(&self) -> Pos2 {
        let busy = |p: Pos2| self.ui.hits.values().any(|r| r.expand(4.0).contains(p));
        let right = self.size.x - kabl_ui::DRAWER_W - 10.0;
        (0..40)
            .flat_map(|i| (0..30).map(move |j| (i, j)))
            .map(|(i, j)| {
                egui::pos2(
                    10.0 + i as f32 * (right - 10.0) / 40.0,
                    60.0 + j as f32 * (self.size.y - 100.0) / 30.0,
                )
            })
            .find(|&p| !busy(p))
            .expect("some bare rack")
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
                    r.max.x < w - kabl_ui::DRAWER_W && r.max.y < h - 30.0 && r.min.y > 40.0,
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
    t.click(&format!("base-entry:{ENV}.attack_ms"));
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
        let p = t.empty_rack();
        t.move_to(p);
        t.button(true);
        t.button(false);
        t.frame();
        assert_eq!(t.ui.inspected, None);
        assert!(!t.ui.hits.keys().any(|k| k.starts_with("lane:")));
    }
}

#[test]
fn collapsed_rings_are_display_only_and_open_the_lanes() {
    let mut t = H::new(1440.0, 900.0);
    let before = t.routes(FILTER, "cutoff_hz");
    assert_eq!(t.ui.inspected, None);
    assert!(!t.ui.hits.keys().any(|k| k.starts_with("lane:")));
    // Pressing the collapsed ring band and dragging: lanes open, no amount changes.
    t.ring_drag(FILTER, "cutoff_hz", 30.0);
    assert_eq!(t.routes(FILTER, "cutoff_hz"), before);
    assert_eq!(t.ui.inspected, Some((FILTER, "cutoff_hz".to_string())));
    assert!(t.ui.hits.contains_key(&format!("lane:{}", before[0].0)));
}

#[test]
fn single_source_dot_and_removal_never_reassigns() {
    for (w, h) in sizes() {
        let mut t = H::new(w, h);
        let attack = t.routes(ENV, "attack_ms");
        let attack_base = t.param(ENV, "attack_ms");
        let from = t.at(&format!("out:{FAST_LFO}.out"));
        let to = t.at(&format!("knob:{ENV}.decay_ms"));
        t.drag(from, to);
        let only = t.routes(ENV, "decay_ms")[0].0;
        // One source, inspected: its dot is there and drags its depth like a multi-source lane.
        let p = t.at(&format!("lane:{only}"));
        t.drag(p, p - egui::vec2(0.0, 15.0));
        assert!(close(t.routes(ENV, "decay_ms")[0].1, 0.35), "{w}x{h}");
        assert_eq!(
            t.routes(ENV, "attack_ms"),
            attack,
            "neighbour routes untouched"
        );
        assert_eq!(
            t.param(ENV, "attack_ms"),
            attack_base,
            "neighbour base untouched"
        );

        // Second source on Decay, remove the selected one: one remains, nothing is selected,
        // and the ring edits nothing until the remaining dot is grabbed.
        let from = t.at("out:7.out");
        t.drag(from, t.at(&format!("knob:{ENV}.decay_ms")));
        let second = t.ui.selected_route.unwrap();
        t.click(&format!("remove:{second}"));
        assert_eq!(t.ui.selected_route, None);
        let remaining = t.routes(ENV, "decay_ms");
        assert_eq!(remaining.len(), 1);
        t.ring_drag(ENV, "decay_ms", 30.0);
        assert_eq!(
            t.routes(ENV, "decay_ms"),
            remaining,
            "no silent reassignment"
        );
        let p = t.at(&format!("lane:{only}"));
        t.drag(p, p - egui::vec2(0.0, 15.0));
        assert_eq!(t.ui.selected_route, Some(only), "explicit grab selects");
        assert!(close(t.routes(ENV, "decay_ms")[0].1, 0.45));

        // Hidden mode: the dot still works.
        t.click("view:Hidden");
        let p = t.at(&format!("lane:{only}"));
        t.drag(p, p + egui::vec2(0.0, 15.0));
        assert!(close(t.routes(ENV, "decay_ms")[0].1, 0.35));
    }
}

impl H {
    /// Presses at `from` and moves to `to` without releasing.
    fn hold(&mut self, from: Pos2, to: Pos2) {
        self.move_to(from);
        self.button(true);
        for i in 1..=6 {
            self.move_to(from + (to - from) * (i as f32 / 6.0));
        }
    }

    fn release(&mut self) {
        self.button(false);
        self.frame();
    }

    fn undo_depth(&self) -> usize {
        self.editor.log().entries().len()
    }
}

#[test]
fn shift_drags_ten_times_finer_on_body_ring_and_dot() {
    for (w, h) in sizes() {
        let mut t = H::new(w, h);
        let before = t.routes(ENV, "attack_ms");
        t.click(&format!("knob:{ENV}.attack_ms"));
        t.click(&format!("row:{}", before[0].0));

        // Ring, Shift held: 30 px = 0.2 of travel coarse, 0.02 fine.
        t.modifiers = Modifiers::SHIFT;
        t.ring_drag(ENV, "attack_ms", 30.0);
        assert!(
            close(t.routes(ENV, "attack_ms")[0].1, before[0].1 + 0.02),
            "{w}x{h} ring"
        );

        // Lane dot of the other route, fine.
        let p = t.at(&format!("lane:{}", before[1].0));
        t.drag(p, p - egui::vec2(0.0, 30.0));
        assert!(
            close(t.routes(ENV, "attack_ms")[1].1, before[1].1 + 0.02),
            "{w}x{h} dot"
        );

        // Body, fine: +0.02 of knob travel.
        let info = kabl_modules::registry::info_for("env.adsr").unwrap().params[0];
        let base = t.param(ENV, "attack_ms").unwrap();
        let c = t.at(&format!("knob:{ENV}.attack_ms"));
        t.drag(c, c - egui::vec2(0.0, 30.0));
        let expect = info.from_norm(info.to_norm(base) + 0.02);
        assert!(
            (t.param(ENV, "attack_ms").unwrap() - expect).abs() < 1e-3,
            "{w}x{h} body"
        );
        t.modifiers = Modifiers::NONE;

        // Shift pressed mid-drag: the coarse part and the fine part add up.
        let c = t.at(&format!("knob:{ENV}.release_ms"));
        let rel = kabl_modules::registry::info_for("env.adsr").unwrap().params[3];
        let r0 = t.param(ENV, "release_ms").unwrap();
        t.hold(c, c - egui::vec2(0.0, 30.0));
        t.modifiers = Modifiers::SHIFT;
        t.move_to(c - egui::vec2(0.0, 60.0));
        t.release();
        t.modifiers = Modifiers::NONE;
        let expect = rel.from_norm(rel.to_norm(r0) + 0.2 + 0.02);
        assert!(
            (t.param(ENV, "release_ms").unwrap() - expect).abs() < 1e-2,
            "{w}x{h} mixed"
        );
    }
}

#[test]
fn escape_cancels_body_ring_and_dot_drags_without_undo_entries() {
    for (w, h) in sizes() {
        let mut t = H::new(w, h);
        let routes = t.routes(ENV, "attack_ms");
        let base = t.param(ENV, "attack_ms");
        t.click(&format!("knob:{ENV}.attack_ms"));
        t.click(&format!("row:{}", routes[0].0));
        let depth = t.undo_depth();

        let ring = t.at(&format!("ring:{ENV}.attack_ms"));
        let dot = t.at(&format!("lane:{}", routes[1].0));
        let body = t.at(&format!("knob:{ENV}.attack_ms"));
        for (what, p) in [("body", body), ("ring", ring), ("dot", dot)] {
            t.hold(p, p - egui::vec2(0.0, 40.0));
            assert!(
                t.routes(ENV, "attack_ms") != routes || t.param(ENV, "attack_ms") != base,
                "{w}x{h} {what}: the drag changed something live"
            );
            t.editor.take_dirty();
            t.key(Key::Escape, Modifiers::NONE);
            assert!(t.editor.is_dirty(), "{what}: audio rebuild requested");
            // Keep moving after Escape: the cancelled drag stays inert until release.
            t.move_to(p - egui::vec2(0.0, 80.0));
            t.release();
            assert_eq!(
                t.routes(ENV, "attack_ms"),
                routes,
                "{w}x{h} {what}: routes restored"
            );
            assert_eq!(
                t.param(ENV, "attack_ms"),
                base,
                "{w}x{h} {what}: base restored"
            );
            assert_eq!(t.undo_depth(), depth, "{w}x{h} {what}: no undo entry");
            assert!(!t.editor.can_redo(), "{w}x{h} {what}: no redo entry");
        }

        // A completed drag is still exactly one undo step.
        t.drag(body, body - egui::vec2(0.0, 40.0));
        assert_eq!(t.undo_depth(), depth + 1);
        t.key(Key::Z, Modifiers::COMMAND);
        assert_eq!(t.param(ENV, "attack_ms"), base);
        t.key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT);
        assert_ne!(t.param(ENV, "attack_ms"), base, "redo");
    }
}

/// Not a check by itself: runs the closeout scenario headlessly (asserting as it goes) and
/// writes, per size, the pointer/key script for `docs/rack-migration/closeout-real-x.sh` plus
/// the expected saved patch. The real-X run must save an identical `checkpoint.json`.
/// `cargo test -p kabl-ui --test interaction -- --ignored`.
#[test]
#[ignore]
fn closeout_scenario_script() {
    for (w, h) in sizes() {
        let mut t = H::new(w, h);
        let mut s: Vec<String> = Vec::new();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../target/slice-closeout/{w}x{h}"));
        std::fs::create_dir_all(&dir).unwrap();
        // Lines for docs/rack-migration/drive.py: targets by key, so the real app (other
        // fonts, a status bar, its own start-up fit) is hit where it drew them.
        macro_rules! drag_up {
            ($key:expr, $dy:expr, $extra:expr) => {{
                let k: String = $key;
                s.push(format!("dragby {k} 0 {}{}", -$dy, $extra));
                let p = t.at(&k);
                t.drag(p, p - egui::vec2(0.0, $dy as f32));
            }};
        }
        macro_rules! click {
            ($key:expr) => {{
                let k = $key;
                s.push(format!("click {k}"));
                t.click(&k);
            }};
        }
        let up = |p: Pos2, dy: f32| p - egui::vec2(0.0, dy);

        // 1. New single source on Decay, edited on its dot, fine, then a cancelled drag.
        s.push(format!("drag out:8.out knob:{ENV}.decay_ms"));
        t.drag(t.at("out:8.out"), t.at(&format!("knob:{ENV}.decay_ms")));
        let d = t.ui.selected_route.unwrap();
        s.push("shot 01-single-dot".into());
        drag_up!(format!("lane:{d}"), 30, "");
        assert!(close(t.routes(ENV, "decay_ms")[0].1, 0.45));
        t.modifiers = Modifiers::SHIFT;
        drag_up!(format!("lane:{d}"), 30, " shift");
        t.modifiers = Modifiers::NONE;
        assert!(close(t.routes(ENV, "decay_ms")[0].1, 0.47));
        let p = t.at(&format!("lane:{d}"));
        s.push(format!("hold lane:{d} 0 -40"));
        t.hold(p, up(p, 40.0));
        s.push("shot 02-dot-drag-before-escape".into());
        s.push("key Escape".into());
        t.key(Key::Escape, Modifiers::NONE);
        s.push("nudge 0 -30".into());
        t.move_to(up(p, 70.0));
        s.push("release".into());
        t.release();
        assert!(close(t.routes(ENV, "decay_ms")[0].1, 0.47), "cancelled");
        s.push("shot 03-after-escape".into());

        // 2. Body drag on Attack cancelled.
        let base = t.param(ENV, "attack_ms");
        let c = t.at(&format!("knob:{ENV}.attack_ms"));
        s.push(format!("hold knob:{ENV}.attack_ms 0 -40"));
        t.hold(c, up(c, 40.0));
        s.push("key Escape".into());
        t.key(Key::Escape, Modifiers::NONE);
        s.push("release".into());
        t.release();
        assert_eq!(t.param(ENV, "attack_ms"), base);

        // 3. Attack's two sources: velocity on its dot; LFO #7 fine in Hidden view.
        click!(format!("knob:{ENV}.attack_ms"));
        let a = t.routes(ENV, "attack_ms");
        drag_up!(format!("lane:{}", a[1].0), 15, "");
        assert!(close(t.routes(ENV, "attack_ms")[1].1, -0.15));
        s.push("shot 04-attack-lanes".into());
        click!("view:Hidden".to_string());
        t.modifiers = Modifiers::SHIFT;
        drag_up!(format!("lane:{}", a[0].0), 30, " shift");
        t.modifiers = Modifiers::NONE;
        assert!(close(t.routes(ENV, "attack_ms")[0].1, 0.32));
        s.push("shot 05-hidden-fine".into());
        click!("view:All".to_string());

        // 4. Remove the selected source: nothing selected, the ring edits nothing.
        click!(format!("remove:{}", a[0].0));
        assert_eq!(t.ui.selected_route, None);
        let rem = t.routes(ENV, "attack_ms");
        drag_up!(format!("ring:{ENV}.attack_ms"), 30, "");
        assert_eq!(t.routes(ENV, "attack_ms"), rem);
        s.push("shot 06-removed-none-selected".into());

        // 5. Undo the removal and the fine drag, redo the fine drag.
        let p = t.empty_rack();
        t.move_to(p);
        for _ in 0..2 {
            s.push("key ctrl+z".into());
            t.key(Key::Z, Modifiers::COMMAND);
        }
        assert!(close(t.routes(ENV, "attack_ms")[0].1, 0.30));
        s.push("key ctrl+shift+z".into());
        t.key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT);
        assert!(close(t.routes(ENV, "attack_ms")[0].1, 0.32));
        s.push("shot 07-after-undo-redo".into());
        s.push(format!("save {}", dir.join("real").display()));

        kabl_core::save(&dir.join("expected"), t.editor.log()).unwrap();
        std::fs::write(dir.join("script.txt"), s.join("\n") + "\n").unwrap();
    }
}

#[test]
fn a_fast_body_drag_to_the_range_end_cancels_and_undoes_cleanly() {
    // Found in the real-X A/V run. A fast drag from the knob centre (first move past egui's
    // drag threshold in one step) used to grab the ring; the body must win, clamp at the range
    // end, cancel with Escape, and a completed drag must stay one undo entry.
    for (w, h) in sizes() {
        let mut t = H::new(w, h);
        t.click(&format!("knob:{FILTER}.cutoff_hz"));
        let depth = t.undo_depth();
        let c = t.at(&format!("knob:{FILTER}.cutoff_hz"));
        t.hold(c, c - egui::vec2(0.0, 120.0));
        assert_eq!(
            t.param(FILTER, "cutoff_hz"),
            Some(20000.0),
            "{w}x{h}: body drag, clamped"
        );
        t.key(Key::Escape, Modifiers::NONE);
        t.release();
        assert_eq!(
            t.param(FILTER, "cutoff_hz"),
            Some(1400.0),
            "{w}x{h} cancelled"
        );
        assert_eq!(t.undo_depth(), depth, "{w}x{h}");
        t.drag(c, c - egui::vec2(0.0, 120.0));
        for _ in 0..10 {
            t.frame();
        }
        assert_eq!(
            t.undo_depth(),
            depth + 1,
            "{w}x{h}: one drag, one entry, idle frames add none"
        );
        t.key(Key::Z, Modifiers::COMMAND);
        assert_eq!(t.param(FILTER, "cutoff_hz"), Some(1400.0), "{w}x{h} undo");
    }
}

// --- Rack migration: faces, expansion, zoom/pan, presentation-only edits ---

impl H {
    fn rclick(&mut self, key: &str) {
        let p = self.at(key);
        self.move_to(p);
        for pressed in [true, false] {
            self.events.push(Event::PointerButton {
                pos: p,
                button: PointerButton::Secondary,
                pressed,
                modifiers: Modifiers::NONE,
            });
            self.frame();
        }
        self.frame();
    }

    fn zoom_at(&mut self, p: Pos2, factor: f32) {
        self.move_to(p);
        self.modifiers = Modifiers::COMMAND;
        self.events.push(Event::Zoom(factor));
        self.frame();
        self.modifiers = Modifiers::NONE;
        self.frame();
    }

    fn face(&self, id: u64, param: &str) -> Option<f32> {
        self.param(id, &format!("face.{param}"))
    }
}

const VCA: u64 = 5;
const OUTPUT: u64 = 6;

#[test]
fn choosing_primary_controls_is_one_undo_step_saved_and_never_rebuilds_audio() {
    let mut t = H::new(1440.0, 900.0);
    let _ = t.editor.take_dirty();
    assert!(
        !t.ui.hits.contains_key(&format!("sel:{VCA}.exponential.1")),
        "advanced by default"
    );
    t.rclick(&format!("module:{VCA}"));
    t.click("menu:choose");
    let entries = t.undo_depth();
    t.click(&format!("pin:{VCA}.exponential"));
    t.click(&format!("pin:{VCA}.gain"));
    assert_eq!(t.undo_depth(), entries, "pins are pending until Done");
    t.click(&format!("done:{VCA}"));
    assert_eq!(t.undo_depth(), entries + 1, "one step");
    assert_eq!(t.face(VCA, "exponential"), Some(1.0));
    assert_eq!(t.face(VCA, "gain"), Some(0.0));
    assert!(
        !t.editor.take_dirty(),
        "presentation only: no audio rebuild"
    );
    // The face now shows Response and not Gain, without expanding.
    assert!(t.ui.hits.contains_key(&format!("sel:{VCA}.exponential.1")));
    assert!(!t.ui.hits.contains_key(&format!("knob:{VCA}.gain")));
    assert!(t.ui.hits.contains_key(&format!("toggle:{VCA}")));

    // Saved and reloaded.
    let dir = tempfile::tempdir().unwrap();
    kabl_core::save(dir.path(), t.editor.log()).unwrap();
    let back = kabl_ui::PatchEditor::from_log(kabl_core::load(dir.path()).unwrap());
    assert_eq!(back.state(), t.editor.state());

    // Undo restores the default face exactly (params absent), redo re-applies.
    t.key(Key::Z, Modifiers::COMMAND);
    assert_eq!(
        (t.face(VCA, "exponential"), t.face(VCA, "gain")),
        (None, None)
    );
    assert!(!t.editor.take_dirty());
    t.key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT);
    assert_eq!(t.face(VCA, "exponential"), Some(1.0));

    // Escape in choose mode discards the pending choice.
    t.rclick(&format!("module:{VCA}"));
    t.click("menu:choose");
    t.click(&format!("pin:{VCA}.gain"));
    t.key(Key::Escape, Modifiers::NONE);
    assert_eq!(t.face(VCA, "gain"), Some(0.0));
    assert!(t.ui.choose.is_none());
}

#[test]
fn push_expansion_moves_neighbours_and_collapses_back_exactly() {
    for (w, h) in sizes() {
        let mut t = H::new(w, h);
        let out = t.ui.hits[&format!("module:{OUTPUT}")];
        let gain = t.ui.hits[&format!("knob:{VCA}.gain")];
        let jack = t.ui.hits[&format!("in:{VCA}.cv")];
        for _ in 0..3 {
            t.click(&format!("toggle:{VCA}"));
            assert!(
                t.ui.hits[&format!("module:{OUTPUT}")].min.x > out.min.x + 50.0,
                "{w}x{h} pushed"
            );
            assert_eq!(
                t.ui.hits[&format!("knob:{VCA}.gain")],
                gain,
                "face control stays"
            );
            assert_eq!(t.ui.hits[&format!("in:{VCA}.cv")], jack, "jack stays");
            // The expanded area is fully on screen (reachable).
            let sel = t.ui.hits[&format!("sel:{VCA}.exponential.1")];
            assert!(sel.max.x < w - kabl_ui::DRAWER_W, "{w}x{h} {sel:?}");
            t.click(&format!("toggle:{VCA}"));
            assert_eq!(
                t.ui.hits[&format!("module:{OUTPUT}")],
                out,
                "{w}x{h} no drift"
            );
        }
    }
}

#[test]
fn floating_expansion_owns_its_area_and_moves_nothing() {
    let mut t = H::new(1440.0, 900.0);
    t.ui.float_expansion = true;
    t.frame();
    let out = t.ui.hits[&format!("module:{OUTPUT}")];
    let out_pos = t.editor.state().modules[&OUTPUT].pos;
    t.click(&format!("toggle:{VCA}"));
    assert_eq!(
        t.ui.hits[&format!("module:{OUTPUT}")],
        out,
        "float pushes nothing"
    );
    let float = t.ui.hits[&format!("float:{VCA}")];
    assert!(float.intersects(out), "the area covers the neighbour");
    // A drag on the float where it covers Output neither moves nor selects Output.
    let p = float.intersect(out).center();
    t.drag(p, p + egui::vec2(60.0, 40.0));
    assert_eq!(t.editor.state().modules[&OUTPUT].pos, out_pos);
    assert_ne!(t.ui.selected_module, Some(OUTPUT));
    // Its own controls work.
    t.click(&format!("sel:{VCA}.exponential.1"));
    assert_eq!(t.param(VCA, "exponential"), Some(1.0));
    // A cable dropped on the float lands on the float's control, not the one under it.
    let from = t.at(&format!("out:{FAST_LFO}.out"));
    let to = t.at(&format!("knob:{VCA}.exponential"));
    t.drag(from, to);
    assert_eq!(t.routes(VCA, "exponential").len(), 1);
    t.click(&format!("toggle:{VCA}"));
    assert_eq!(t.ui.hits[&format!("module:{OUTPUT}")], out);
}

#[test]
fn a_route_to_an_off_face_control_docks_on_the_toggle_and_selecting_it_reveals() {
    let mut t = H::new(1440.0, 900.0);
    let cable = t.editor.connect_route(
        kabl_core::PortRef::Module {
            id: FAST_LFO,
            port: "out".into(),
        },
        VCA,
        "exponential",
    );
    t.frame();
    t.frame();
    assert!(!t.ui.expanded.contains(&VCA));
    let toggle = t.ui.hits[&format!("toggle:{VCA}")];
    assert!(
        t.ui.hits.contains_key(&format!("route:{cable}")),
        "the lead is drawn"
    );
    // Selecting the route (its cable) inspects the destination and reveals it.
    t.click(&format!("route:{cable}"));
    assert_eq!(t.ui.inspected, Some((VCA, "exponential".to_string())));
    t.frame();
    assert!(t.ui.expanded.contains(&VCA));
    assert!(t.ui.hits.contains_key(&format!("sel:{VCA}.exponential.1")));
    let _ = toggle;
    // Hidden cables: the toggle still exists, the route is still editable in the drawer.
    t.click(&format!("toggle:{VCA}"));
    t.click("view:Hidden");
    assert!(t.ui.hits.contains_key(&format!("toggle:{VCA}")));
}

#[test]
fn zoom_and_pan_keep_every_knob_gesture_and_scale_hit_regions() {
    for (w, h) in sizes() {
        let mut t = H::new(w, h);
        let knob = t.ui.hits[&format!("knob:{ENV}.attack_ms")];
        t.zoom_at(knob.center(), 1.5);
        let z = t.ui.zoom;
        let big = t.ui.hits[&format!("knob:{ENV}.attack_ms")];
        assert!(
            (big.width() / knob.width() - 1.5).abs() < 0.05,
            "{w}x{h}: {knob:?} -> {big:?} at {z}"
        );
        assert!(
            big.center().distance(knob.center()) < 2.0,
            "zoom about the pointer"
        );
        // Pan by dragging bare rack.
        let p = t.empty_rack();
        t.drag(p, p + egui::vec2(-40.0, -30.0));
        let moved = t.ui.hits[&format!("knob:{ENV}.attack_ms")];
        assert!(
            (moved.center() - (big.center() + egui::vec2(-40.0, -30.0))).length() < 2.0,
            "{w}x{h} pan"
        );

        // Ring (selected source), Shift fine, Escape cancel: as at 100 %.
        let before = t.routes(ENV, "attack_ms");
        t.click(&format!("knob:{ENV}.attack_ms"));
        t.click(&format!("row:{}", before[0].0));
        t.ring_drag(ENV, "attack_ms", 30.0);
        assert!(
            close(t.routes(ENV, "attack_ms")[0].1, before[0].1 + 0.2),
            "{w}x{h} ring"
        );
        t.modifiers = Modifiers::SHIFT;
        t.ring_drag(ENV, "attack_ms", 30.0);
        t.modifiers = Modifiers::NONE;
        assert!(
            close(t.routes(ENV, "attack_ms")[0].1, before[0].1 + 0.22),
            "{w}x{h} fine"
        );
        let depth = t.undo_depth();
        let r = t.at(&format!("ring:{ENV}.attack_ms"));
        t.hold(r, r - egui::vec2(0.0, 30.0));
        t.key(Key::Escape, Modifiers::NONE);
        t.release();
        assert_eq!(t.undo_depth(), depth, "{w}x{h} escape leaves no entry");
        assert!(close(t.routes(ENV, "attack_ms")[0].1, before[0].1 + 0.22));
        // Lane dot of the other source, zoomed.
        let lane = t.at(&format!("lane:{}", before[1].0));
        t.drag(lane, lane - egui::vec2(0.0, 15.0));
        assert!(
            close(t.routes(ENV, "attack_ms")[1].1, before[1].1 + 0.1),
            "{w}x{h} lane"
        );
        t.key(Key::Z, Modifiers::COMMAND);
        assert!(
            close(t.routes(ENV, "attack_ms")[1].1, before[1].1),
            "{w}x{h} undo"
        );
    }
}

#[test]
fn view_changes_never_touch_the_patch_or_the_audio() {
    let mut t = H::new(1280.0, 800.0);
    let _ = t.editor.take_dirty();
    let state = t.editor.state().clone();
    let depth = t.undo_depth();
    for key in [
        "theme:dark",
        "view:Hidden",
        "zoom:in",
        "zoom:out",
        "zoom:fit",
        "view:Focus",
        "theme:light",
        "zoom:100",
    ] {
        t.click(key);
    }
    t.click(&format!("toggle:{VCA}"));
    t.ui.float_expansion = true;
    t.ui.skins = true;
    t.frame();
    t.click("routing");
    t.click("routing");
    assert_eq!(t.editor.state(), &state);
    assert_eq!(t.undo_depth(), depth);
    assert!(!t.editor.take_dirty());
}

#[test]
fn face_params_do_not_change_the_sound() {
    use kabl_engine::compile::compile;
    let t = H::new(1440.0, 900.0);
    let mut faced = t.editor.state().clone();
    for m in faced.modules.values_mut() {
        m.params.insert("face.gain".into(), 0.0);
        m.params.insert("face.timing".into(), 1.0);
        m.params.insert("face.rate_hz".into(), 0.0);
    }
    let render = |p: &kabl_core::PatchState| {
        let mut c = compile(p, 48000.0, 4).unwrap();
        c.note_on(0, 0.0, 1.0);
        let mut out = Vec::new();
        for _ in 0..40 {
            c.process_block();
            out.extend_from_slice(c.left());
        }
        out
    };
    let a = render(t.editor.state());
    assert!(a.iter().any(|s| *s != 0.0));
    assert_eq!(a, render(&faced), "bit-identical");
}

#[test]
fn opening_the_drawer_keeps_the_inspected_control_reachable() {
    let mut t = H::new(1280.0, 800.0);
    t.click("routing"); // close
                        // Pan so the filter cutoff sits where the drawer will be.
    let k = t.at(&format!("knob:{FILTER}.cutoff_hz"));
    let target = egui::pos2(1280.0 - 150.0, k.y);
    let p = t.empty_rack();
    t.drag(p, p + (target - k));
    t.click(&format!("knob:{FILTER}.cutoff_hz"));
    t.click("routing"); // open
    t.frame();
    let r = t.ui.hits[&format!("knob:{FILTER}.cutoff_hz")];
    assert!(r.max.x < 1280.0 - kabl_ui::DRAWER_W, "{r:?}");
}

/// Not a check: writes `patches/crowded` (15 modules on three rows, jack cables and knob routes
/// crossing everywhere), the fixture for crowding screenshots.
/// `cargo test -p kabl-ui --test interaction write_crowded_patch -- --ignored`.
#[test]
#[ignore]
fn write_crowded_patch() {
    use kabl_core::{PortRef, Vec2};
    let mut e = kabl_ui::PatchEditor::new();
    let row = |r: usize| kabl_ui::rack::row_y(r);
    let mut ids = std::collections::BTreeMap::new();
    let layout: [(&str, &str, usize, f32); 15] = [
        ("midi", "midi.in", 0, 24.0),
        ("osc1", "osc.va", 0, 180.0),
        ("osc2", "osc.va", 0, 390.0),
        ("mix", "mixer", 0, 600.0),
        ("ring", "ringmod", 0, 840.0),
        ("f1", "filter.svf", 1, 24.0),
        ("f2", "filter.svf", 1, 240.0),
        ("vca1", "vca", 1, 450.0),
        ("vca2", "vca", 1, 630.0),
        ("out", "out", 1, 810.0),
        ("env1", "env.adsr", 2, 24.0),
        ("env2", "env.adsr", 2, 270.0),
        ("lfo1", "lfo", 2, 510.0),
        ("lfo2", "lfo", 2, 720.0),
        ("lfo3", "lfo", 2, 930.0),
    ];
    for (name, kind, r, x) in layout {
        ids.insert(name, e.add_module(kind, Vec2 { x, y: row(r) }));
    }
    let port = |n: &str, p: &str| PortRef::Module {
        id: ids[n],
        port: p.into(),
    };
    for (a, ap, b, bp) in [
        ("midi", "pitch", "osc1", "pitch"),
        ("midi", "pitch", "osc2", "pitch"),
        ("midi", "gate", "env1", "gate"),
        ("midi", "gate", "env2", "gate"),
        ("osc1", "out", "mix", "in1"),
        ("osc2", "out", "mix", "in2"),
        ("osc1", "out", "ring", "a"),
        ("osc2", "out", "ring", "b"),
        ("ring", "out", "mix", "in3"),
        ("mix", "out", "f1", "in"),
        ("mix", "out", "f2", "in"),
        ("f1", "lp", "vca1", "in"),
        ("f2", "bp", "vca2", "in"),
        ("env1", "out", "vca1", "cv"),
        ("env2", "out", "vca2", "cv"),
        ("vca1", "out", "out", "left"),
        ("vca2", "out", "out", "right"),
    ] {
        e.connect(port(a, ap), port(b, bp));
    }
    for (a, ap, b, param) in [
        ("lfo1", "out", "f1", "cutoff_hz"),
        ("lfo2", "out", "f1", "cutoff_hz"),
        ("env2", "out", "f1", "cutoff_hz"),
        ("midi", "velocity", "f1", "cutoff_hz"),
        ("lfo3", "out", "f2", "resonance"),
        ("lfo1", "out", "osc2", "base_hz"),
        ("lfo2", "out", "env1", "attack_ms"),
        ("midi", "velocity", "env1", "attack_ms"),
        ("lfo3", "out", "mix", "level2"),
        ("lfo1", "out", "lfo3", "rate_hz"),
        ("env1", "out", "env2", "timing"),
    ] {
        e.connect_route(port(a, ap), ids[b], param);
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/crowded");
    kabl_core::save(&dir, e.log()).unwrap();
}

#[test]
fn a_jack_cable_is_removed_by_pulling_its_plug_never_by_a_click() {
    for (w, h) in sizes() {
        let mut t = H::new(w, h);
        let into = |t: &H, id: u64, port: &str| {
            t.editor
                .state()
                .cables
                .iter()
                .find(|(_, c)| {
                    c.to == kabl_core::PortRef::Module {
                        id,
                        port: port.into(),
                    }
                })
                .map(|(&cid, c)| (cid, c.from.clone()))
        };
        let (cable, from) = into(&t, VCA, "in").expect("filter -> vca in");
        // A click on the cable only selects nothing and deletes nothing.
        t.click(&format!("cable:{cable}"));
        assert_eq!(into(&t, VCA, "in").map(|c| c.0), Some(cable), "{w}x{h}");
        // Pull the plug out onto bare rack: removed, one undo brings the same cable back.
        let depth = t.undo_depth();
        let p = t.empty_rack();
        t.drag(t.at(&format!("in:{VCA}.in")), p);
        assert_eq!(into(&t, VCA, "in"), None, "{w}x{h} removed");
        assert_eq!(t.undo_depth(), depth + 1);
        t.key(Key::Z, Modifiers::COMMAND);
        assert_eq!(
            into(&t, VCA, "in").map(|c| c.0),
            Some(cable),
            "{w}x{h} undo"
        );
        // Move it to another input: one step, same source.
        t.drag(
            t.at(&format!("in:{VCA}.in")),
            t.at(&format!("in:{FILTER}.cutoff_cv")),
        );
        assert_eq!(into(&t, VCA, "in"), None);
        assert_eq!(
            into(&t, FILTER, "cutoff_cv").map(|c| c.1),
            Some(from.clone())
        );
        assert_eq!(t.undo_depth(), depth + 1);
        t.key(Key::Z, Modifiers::COMMAND);
        assert_eq!(into(&t, VCA, "in").map(|c| c.0), Some(cable));
        assert_eq!(into(&t, FILTER, "cutoff_cv"), None);
        // Pulled out and pushed back into its own jack: nothing changes, no undo entry.
        let depth = t.undo_depth();
        let j = t.at(&format!("in:{VCA}.in"));
        t.hold(j, j + egui::vec2(60.0, 0.0));
        t.move_to(j);
        t.release();
        assert_eq!(into(&t, VCA, "in").map(|c| c.0), Some(cable), "{w}x{h}");
        assert_eq!(t.undo_depth(), depth);
    }
}

#[test]
fn source_lanes_of_a_knob_at_the_canvas_edge_pan_fully_into_view_and_stay_draggable() {
    for (w, h) in sizes() {
        let right = w - kabl_ui::DRAWER_W;
        // Corners of the canvas: next to the drawer, and at the bottom left.
        for target in [egui::pos2(right - 20.0, 60.0), egui::pos2(20.0, h - 40.0)] {
            let mut t = H::new(w, h);
            let key = format!("knob:{FILTER}.cutoff_hz");
            let k = t.at(&key);
            let p = t.empty_rack();
            t.drag(p, p + (target - k));
            t.click(&key);
            t.frame();
            let routes = t.routes(FILTER, "cutoff_hz");
            for r in &routes {
                let dot = t.ui.hits[&format!("lane:{}", r.0)];
                assert!(
                    dot.min.x > 0.0
                        && dot.max.x < right
                        && dot.min.y > 40.0
                        && dot.max.y < h - 20.0,
                    "{w}x{h} {target:?}: lane {} at {dot:?}",
                    r.0
                );
            }
            let cable = routes[0].0;
            let p = t.at(&format!("lane:{cable}"));
            t.drag(p, p - egui::vec2(0.0, 15.0));
            assert_eq!(t.ui.selected_route, Some(cable));
            assert!(close(t.routes(FILTER, "cutoff_hz")[0].1, routes[0].1 + 0.1));
        }
    }
}
