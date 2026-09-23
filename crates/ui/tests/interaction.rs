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
    // Timing is an advanced control: expand the envelope first.
    assert!(!t.ui.hits.contains_key(&format!("sel:{ENV}.timing.1")));
    t.click(&format!("toggle:{ENV}"));
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
/// writes, per size, the pointer/key script for `docs/modulation-slice/closeout-real-x.sh` plus
/// the expected saved patch. The real-X run must save an identical `checkpoint.json`.
/// `cargo test -p kabl-ui --test interaction -- --ignored`.
#[test]
#[ignore]
fn closeout_scenario_script() {
    fn xy(p: Pos2) -> String {
        format!("{:.0} {:.0}", p.x, p.y)
    }
    for (w, h) in sizes() {
        let mut t = H::new(w, h);
        let mut s: Vec<String> = Vec::new();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../target/slice-closeout/{w}x{h}"));
        std::fs::create_dir_all(&dir).unwrap();
        macro_rules! drag {
            ($from:expr, $to:expr) => {{
                let (a, b) = ($from, $to);
                s.push(format!("drag {} {}", xy(a), xy(b)));
                t.drag(a, b);
            }};
        }
        macro_rules! click {
            ($key:expr) => {{
                let k = $key;
                s.push(format!("click {}", xy(t.at(&k))));
                t.click(&k);
            }};
        }
        let up = |p: Pos2, dy: f32| p - egui::vec2(0.0, dy);

        // 1. New single source on Decay, edited on its dot, fine, then a cancelled drag.
        drag!(t.at("out:8.out"), t.at(&format!("knob:{ENV}.decay_ms")));
        let d = t.ui.selected_route.unwrap();
        s.push("shot 01-single-dot".into());
        drag!(
            t.at(&format!("lane:{d}")),
            up(t.at(&format!("lane:{d}")), 30.0)
        );
        assert!(close(t.routes(ENV, "decay_ms")[0].1, 0.45));
        s.push("shift down".into());
        t.modifiers = Modifiers::SHIFT;
        drag!(
            t.at(&format!("lane:{d}")),
            up(t.at(&format!("lane:{d}")), 30.0)
        );
        s.push("shift up".into());
        t.modifiers = Modifiers::NONE;
        assert!(close(t.routes(ENV, "decay_ms")[0].1, 0.47));
        let p = t.at(&format!("lane:{d}"));
        s.push(format!("hold {} {}", xy(p), xy(up(p, 40.0))));
        t.hold(p, up(p, 40.0));
        s.push("shot 02-dot-drag-before-escape".into());
        s.push("key Escape".into());
        t.key(Key::Escape, Modifiers::NONE);
        s.push(format!("move {}", xy(up(p, 70.0))));
        t.move_to(up(p, 70.0));
        s.push("release".into());
        t.release();
        assert!(close(t.routes(ENV, "decay_ms")[0].1, 0.47), "cancelled");
        s.push("shot 03-after-escape".into());

        // 2. Body drag on Attack cancelled.
        let base = t.param(ENV, "attack_ms");
        let c = t.at(&format!("knob:{ENV}.attack_ms"));
        s.push(format!("hold {} {}", xy(c), xy(up(c, 40.0))));
        t.hold(c, up(c, 40.0));
        s.push("key Escape".into());
        t.key(Key::Escape, Modifiers::NONE);
        s.push("release".into());
        t.release();
        assert_eq!(t.param(ENV, "attack_ms"), base);

        // 3. Attack's two sources: velocity on its dot; LFO #7 fine in Hidden view.
        click!(format!("knob:{ENV}.attack_ms"));
        let a = t.routes(ENV, "attack_ms");
        drag!(
            t.at(&format!("lane:{}", a[1].0)),
            up(t.at(&format!("lane:{}", a[1].0)), 15.0)
        );
        assert!(close(t.routes(ENV, "attack_ms")[1].1, -0.15));
        s.push("shot 04-attack-lanes".into());
        click!("view:Hidden".to_string());
        s.push("shift down".into());
        t.modifiers = Modifiers::SHIFT;
        drag!(
            t.at(&format!("lane:{}", a[0].0)),
            up(t.at(&format!("lane:{}", a[0].0)), 30.0)
        );
        s.push("shift up".into());
        t.modifiers = Modifiers::NONE;
        assert!(close(t.routes(ENV, "attack_ms")[0].1, 0.32));
        s.push("shot 05-hidden-fine".into());
        click!("view:All".to_string());

        // 4. Remove the selected source: nothing selected, the ring edits nothing.
        click!(format!("remove:{}", a[0].0));
        assert_eq!(t.ui.selected_route, None);
        let rem = t.routes(ENV, "attack_ms");
        let r = t.at(&format!("ring:{ENV}.attack_ms"));
        drag!(r, up(r, 30.0));
        assert_eq!(t.routes(ENV, "attack_ms"), rem);
        s.push("shot 06-removed-none-selected".into());

        // 5. Undo the removal and the fine drag, redo the fine drag.
        s.push("move 700 700".into());
        t.move_to(egui::pos2(700.0, 700.0));
        for _ in 0..2 {
            s.push("key ctrl+z".into());
            t.key(Key::Z, Modifiers::COMMAND);
        }
        assert!(close(t.routes(ENV, "attack_ms")[0].1, 0.30));
        s.push("key ctrl+shift+z".into());
        t.key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT);
        assert!(close(t.routes(ENV, "attack_ms")[0].1, 0.32));
        s.push("shot 07-after-undo-redo".into());
        s.push("save".into());

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
