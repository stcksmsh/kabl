//! D02: Perform controls that explain themselves, through real egui input on the factory
//! sounds. Explanations follow the current graph (routes added, bypassed, removed, undone),
//! Show reaches the exact control (an off-face one too) and Back returns to the card, and
//! none of it edits the patch, dirties the document or adds an undo step.

use egui::{Event, Key, Modifiers, Pos2, RawInput, Rect};
use kabl_core::{ModuleId, PatchState, PortRef};
use kabl_ui::explain::{self, Reach, Subject, Target};
use kabl_ui::{browser, perform, routing, show, PatchEditor, UiState};

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    size: egui::Vec2,
    events: Vec<Event>,
    t: f64,
    pointer: Pos2,
}

fn load(patch: &str) -> PatchEditor {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../patches")
        .join(patch);
    PatchEditor::from_log(kabl_core::load(&dir).expect("patch"))
}

impl H {
    fn new(patch: &str, w: f32, h: f32) -> Self {
        let mut h = H {
            ctx: egui::Context::default(),
            editor: load(patch),
            ui: UiState::default(),
            size: egui::vec2(w, h),
            events: Vec::new(),
            t: 0.0,
            pointer: Pos2::ZERO,
        };
        h.ui.perform_open = true;
        h.frame();
        h.frame();
        h.editor.take_dirty();
        // The document the unsaved check compares against (set by the first frame).
        assert!(h.ui.doc.is_some() && !browser::is_modified(&h.editor, &h.ui));
        h
    }

    fn drag(&mut self, key: &str, dx: f32) {
        let p = self.rect(key).center();
        self.pointer = p;
        self.events.push(Event::PointerMoved(p));
        self.frame();
        self.events.push(Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        });
        self.frame();
        for k in 1..=6 {
            let q = p + egui::vec2(dx * k as f32 / 6.0, 0.0);
            self.pointer = q;
            self.events.push(Event::PointerMoved(q));
            self.frame();
        }
        self.events.push(Event::PointerButton {
            pos: self.pointer,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        });
        self.frame();
        self.frame();
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
        let _ = self.ctx.end_pass();
    }

    fn rect(&self, key: &str) -> Rect {
        *self
            .ui
            .hits
            .get(key)
            .unwrap_or_else(|| panic!("no hit target {key}"))
    }

    fn click(&mut self, key: &str) {
        let p = self.rect(key).center();
        self.pointer = p;
        self.events.push(Event::PointerMoved(p));
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
        self.frame();
    }

    fn key(&mut self, key: Key) {
        for pressed in [true, false] {
            self.events.push(Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: Modifiers::NONE,
            });
            self.frame();
        }
    }

    /// Patch state, undo depth and document state before an action that must not edit.
    fn snapshot(&self) -> (PatchState, usize, bool) {
        (
            self.editor.state().clone(),
            self.editor.log().entries().len(),
            self.editor.can_undo(),
        )
    }

    fn unchanged_since(&mut self, before: &(PatchState, usize, bool)) {
        assert_eq!(&self.snapshot(), before, "navigation edited the patch");
        assert!(!self.editor.take_dirty(), "navigation rebuilt audio");
        assert!(
            !browser::is_modified(&self.editor, &self.ui),
            "navigation dirtied the document"
        );
        assert!(self.ui.launches.is_empty() && self.ui.transport.is_empty());
    }
}

fn sizes() -> [(f32, f32); 2] {
    [(1280.0, 800.0), (1440.0, 900.0)]
}

// patches/palette/pad: Macros #1 (m1 "Warmth" → c28 ladder #7 cutoff, c29 ladder #7 drive,
// which is off the ladder's face), ADSR #8 attack pinned "Attack".
const MACROS: ModuleId = 1;
const LADDER: ModuleId = 7;

#[test]
fn a_macro_card_lists_its_real_destinations_and_reveals_an_off_face_one() {
    for (w, hgt) in sizes() {
        let mut h = H::new("palette/pad", w, hgt);
        let before = h.snapshot();
        assert!(
            !h.ui.hits.contains_key("knob:7.drive_db"),
            "drive starts off the face"
        );
        h.click("pexplain:1.m1");
        assert_eq!(
            h.ui.explain.subject,
            Some(Subject::Control {
                id: MACROS,
                key: "m1".into()
            })
        );
        assert!(h.ui.drawer_open);
        // Both routes, by their cables, each with a Show.
        for c in [28, 29] {
            let r = h.rect(&format!("explain-dest:{c}"));
            assert!(
                Rect::from_min_size(Pos2::ZERO, h.size).contains_rect(r),
                "{c} off screen at {w}x{hgt}"
            );
        }
        h.click("explain-dest:29");
        assert_eq!(h.ui.inspected, Some((LADDER, "drive_db".into())));
        assert_eq!(h.ui.selected_route, Some(29));
        assert!(
            h.ui.expanded.contains(&LADDER),
            "the hidden control is revealed"
        );
        let knob = h.rect("knob:7.drive_db");
        let canvas = h.rect("routing").left();
        assert!(knob.right() < canvas + 400.0 && knob.left() >= 0.0);
        assert!(matches!(
            h.ui.explain.target,
            Some(Target::Control { id: LADDER, .. })
        ));
        // Its route row (bypass, depth) is in view in the drawer, above the Perform panel.
        let row = h.rect("bypass:29");
        let perform_top = h.rect("pcard:1.m1").top();
        assert!(
            row.top() > h.rect("explain-dest:29").bottom() && row.bottom() < perform_top,
            "route row hidden at {w}x{hgt}: {row:?}"
        );
        h.unchanged_since(&before);
        // Back: the view before Show, the card again.
        h.click("explain-back");
        assert_eq!(h.ui.inspected, None);
        assert!(
            !h.ui.expanded.contains(&LADDER),
            "the transient reveal is undone"
        );
        assert!(h.ui.explain.target.is_none());
        assert!(h.ui.explain.is_open(), "Back keeps the explanation");
        assert!(h.ui.hits.contains_key("pcard:1.m1"));
        h.unchanged_since(&before);
        // Nothing saved as a face choice.
        let m = &h.editor.state().modules[&LADDER];
        assert!(m.params.keys().all(|k| !k.starts_with("face.")));
    }
}

#[test]
fn a_direct_pin_reaches_its_exact_parameter() {
    let mut h = H::new("palette/pad", 1440.0, 900.0);
    let before = h.snapshot();
    h.click("pexplain:8.attack_ms");
    h.click("explain-show:8.attack_ms");
    assert_eq!(h.ui.inspected, Some((8, "attack_ms".into())));
    assert_eq!(h.ui.selected_module, Some(8));
    assert!(h.ui.hits.contains_key("knob:8.attack_ms"));
    h.unchanged_since(&before);
    // Escape closes the explanation without moving anything or editing.
    h.key(Key::Escape);
    assert!(!h.ui.explain.is_open());
    assert_eq!(h.ui.inspected, Some((8, "attack_ms".into())));
    h.unchanged_since(&before);
}

#[test]
fn the_explanation_follows_route_edits_and_undo() {
    let mut h = H::new("palette/pad", 1440.0, 900.0);
    h.click("pexplain:1.m1");
    let dests = |h: &H| explain::destinations(h.editor.state(), MACROS, "m1").0;
    assert_eq!(
        dests(&h).iter().map(|d| d.cable).collect::<Vec<_>>(),
        [28, 29]
    );
    // Bypass one: listed as bypassed, the other untouched.
    h.editor.set_route_bypass(28, true);
    h.frame();
    let d = dests(&h);
    assert!(matches!(d[0].reach, Reach::Param { bypass: true, .. }));
    assert!(matches!(d[1].reach, Reach::Param { bypass: false, .. }));
    let lines =
        explain::base_and_modulation(h.editor.state(), LADDER, param(&h, LADDER, "cutoff_hz"));
    assert!(
        lines
            .iter()
            .any(|l| l.contains("bypassed, adds nothing now")),
        "{lines:?}"
    );
    // Inverted depth shows its sign.
    h.editor.set_route_amount(29, -0.3, true);
    h.frame();
    assert!(matches!(dests(&h)[1].reach, Reach::Param { amount, .. } if amount == -0.3));
    // Show one, then remove it: the target goes, the panel says so; undo brings it back.
    h.click("explain-dest:29");
    h.editor.disconnect(29);
    h.frame();
    assert_eq!(dests(&h).len(), 1);
    assert!(h.ui.explain.target.is_none() && h.ui.explain.target_lost);
    assert!(!h.ui.hits.contains_key("explain-dest:29"));
    h.editor.undo();
    h.frame();
    h.frame();
    assert_eq!(dests(&h).len(), 2);
    assert!(h.ui.hits.contains_key("explain-dest:29"));
    // A new route appears at once.
    let c = h.editor.connect_route(
        PortRef::Module {
            id: MACROS,
            port: "m1".into(),
        },
        8,
        "attack_ms",
    );
    h.frame();
    h.frame();
    assert!(h.ui.hits.contains_key(&format!("explain-dest:{c}")));
}

fn param(h: &H, id: ModuleId, name: &str) -> &'static kabl_modules::ParamInfo {
    explain::param_of(h.editor.state(), id, name).unwrap()
}

#[test]
fn a_macro_without_routes_says_so_and_duplicate_labels_stay_distinct() {
    let mut h = H::new("palette/pad", 1440.0, 900.0);
    let m = h
        .editor
        .add_module("macro", kabl_core::Vec2 { x: 0.0, y: 900.0 });
    h.editor.set_label(m, "name.m4", Some("Space".into()));
    perform::toggle_pin(&mut h.editor, m, "m4");
    h.frame();
    h.frame();
    let (rows, more) = explain::destinations(h.editor.state(), m, "m4");
    assert!(rows.is_empty() && !more);
    // Two cards both named "Space": each explanation names its own module.
    let a = perform::pin_source(h.editor.state(), &pin(MACROS, "m4"));
    let b = perform::pin_source(h.editor.state(), &pin(m, "m4"));
    assert_ne!(a, b);
    assert!(b.contains(&format!("#{m}")));
    h.click(&format!("pexplain:{m}.m4"));
    assert_eq!(
        h.ui.explain.subject,
        Some(Subject::Control {
            id: m,
            key: "m4".into()
        })
    );
    assert!(!h.ui.hits.keys().any(|k| k.starts_with("explain-dest:")));
}

fn pin(id: ModuleId, key: &str) -> perform::Pin {
    perform::Pin {
        id,
        key: key.into(),
        order: 0.0,
    }
}

#[test]
fn signal_jack_destinations_list_one_bounded_hop() {
    // sound-palette: "Motion" (Macros #3 m2) feeds ring modulators' b inputs (with LFOs on
    // a); their outputs move filter cutoffs and oscillator fine tuning.
    let e = load("sound-palette");
    let (rows, _) = explain::destinations(e.state(), 3, "m2");
    let jacks: Vec<_> = rows
        .iter()
        .filter(|d| matches!(&d.reach, Reach::Jack(p) if p == "b"))
        .map(|d| d.to)
        .collect();
    assert_eq!(jacks, [46, 69, 80]);
    let via46: Vec<_> = rows
        .iter()
        .filter(|d| d.via == Some(46))
        .map(|d| d.cable)
        .collect();
    assert_eq!(via46, [61, 62]);
    assert!(
        rows.iter().any(|d| d.via.is_none() && d.cable == 149),
        "chorus depth"
    );
    assert!(rows.len() <= explain::MAX_ROWS);
}

#[test]
fn signal_paths_come_from_cables_and_stay_bounded() {
    let e = load("init-keyboard");
    let path = explain::path_to_output(e.state(), 2).unwrap().unwrap();
    assert_eq!(path.first(), Some(&2));
    assert_eq!(e.state().modules[path.last().unwrap()].kind, "out");
    assert!(explain::path_text(e.state(), &path).contains("Osc #2"));
    // A cycle with no output: terminates, no path.
    let mut ed = PatchEditor::new();
    let a = ed.add_module("vca", kabl_core::Vec2::default());
    let b = ed.add_module("vca", kabl_core::Vec2::default());
    let port = |id, p: &str| PortRef::Module { id, port: p.into() };
    ed.connect(port(a, "out"), port(b, "in"));
    ed.connect(port(b, "out"), port(a, "in"));
    assert_eq!(explain::path_to_output(ed.state(), a), Ok(None));
    // A chain longer than the search limit: unknown, not "no path".
    let mut ed = PatchEditor::new();
    let first = ed.add_module("vca", kabl_core::Vec2::default());
    let mut prev = first;
    for _ in 0..600 {
        let n = ed.add_module("vca", kabl_core::Vec2::default());
        ed.connect(port(prev, "out"), port(n, "in"));
        prev = n;
    }
    let out = ed.add_module("out", kabl_core::Vec2::default());
    ed.connect(port(prev, "out"), port(out, "left"));
    assert_eq!(
        explain::path_to_output(ed.state(), first),
        Err(explain::PathLimit)
    );
}

#[test]
fn base_and_modulation_are_told_apart_with_units() {
    let e = load("reference");
    let s = e.state();
    let cutoff = explain::param_of(s, 3, "cutoff_hz").unwrap();
    let lines = explain::base_and_modulation(s, 3, cutoff);
    assert!(lines[0].starts_with("Base (stored): "), "{lines:?}");
    assert!(lines[0].contains("Hz"));
    // LFO: bipolar; velocity: unipolar, signed; combined reach is labelled as configured.
    assert!(lines
        .iter()
        .any(|l| l.contains("LFO #7") && l.contains("±12 % of travel")));
    assert!(lines
        .iter()
        .any(|l| l.contains("velocity") && l.contains("+15 % of travel at full")));
    assert!(lines
        .iter()
        .any(|l| l.contains("not") && l.contains("measured")));
    let attack = explain::param_of(s, 4, "attack_ms").unwrap();
    let lines = explain::base_and_modulation(s, 4, attack);
    assert!(
        lines.iter().any(|l| l.contains("-25 % of travel at full")),
        "{lines:?}"
    );
    // A plain control.
    let e = load("init-keyboard");
    let p = explain::param_of(e.state(), 3, "cutoff_hz").unwrap();
    let lines = explain::base_and_modulation(e.state(), 3, p);
    assert!(lines[0].contains("2.20 kHz") && lines[0].contains("Perform card"));
    assert!(lines.iter().any(|l| l.starts_with("Modulation: none")));
    // A macro route states the value at the macro's stored position; a CC is not modulation.
    let e = load("palette/pad");
    let p = explain::param_of(e.state(), LADDER, "cutoff_hz").unwrap();
    let lines = explain::base_and_modulation(e.state(), LADDER, p);
    assert!(lines.iter().any(|l| l.contains("Warmth")
        && l.contains("base + this route alone at its stored 40 %")
        && l.contains("the other routes move it further")));
    let p = explain::param_of(e.state(), MACROS, "m1").unwrap();
    let lines = explain::base_and_modulation(e.state(), MACROS, p);
    assert!(lines.iter().any(|l| l.contains("MIDI CC 20")), "{lines:?}");
    assert!(lines.iter().any(|l| l.contains("not a modulation")));
}

#[test]
fn edits_from_perform_and_rack_are_one_state_while_explained() {
    let mut h = H::new("init-keyboard", 1440.0, 900.0);
    h.click("pexplain:3.cutoff_hz");
    h.click("explain-show:3.cutoff_hz");
    // The rack's Base field and the card write the same stored value.
    h.editor.set_param(3, "cutoff_hz", 1000.0);
    h.frame();
    let p = param(&h, 3, "cutoff_hz");
    assert_eq!(routing::base_value(h.editor.state(), 3, p), 1000.0);
    let lines = explain::base_and_modulation(h.editor.state(), 3, p);
    assert!(lines[0].contains("1.00 kHz"));
    // Save and reload keep it and the explanation reads the same.
    let dir = tempfile::tempdir().unwrap();
    kabl_core::save(dir.path(), h.editor.log()).unwrap();
    let back = PatchEditor::from_log(kabl_core::load(dir.path()).unwrap());
    assert_eq!(back.state(), h.editor.state());
    assert_eq!(explain::base_and_modulation(back.state(), 3, p), lines);
    h.editor.undo();
    h.frame();
    assert!(explain::base_and_modulation(h.editor.state(), 3, p)[0].contains("2.20 kHz"));
}

#[test]
fn module_removal_and_patch_replacement_invalidate_safely() {
    let mut h = H::new("palette/pad", 1440.0, 900.0);
    h.click("pexplain:1.m1");
    h.click("explain-dest:28");
    // Delete the ladder: its routes and the target go; the panel keeps drawing.
    h.editor.remove_module(LADDER);
    h.frame();
    h.frame();
    assert!(h.ui.explain.target.is_none() && h.ui.explain.target_lost);
    assert!(explain::destinations(h.editor.state(), MACROS, "m1")
        .0
        .is_empty());
    h.editor.undo();
    h.frame();
    h.frame();
    assert!(h.ui.hits.contains_key("explain-dest:28"));
    // Delete the macro module itself while explaining it.
    h.editor.remove_module(MACROS);
    h.frame();
    h.frame();
    assert!(
        h.ui.explain.is_open(),
        "says the module is gone rather than vanishing"
    );
    h.editor.undo();
    h.frame();
    // Opening another sound closes everything tied to this one.
    h.click("explain-dest:29");
    let log = kabl_core::load(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches/palette/lead"),
    )
    .unwrap();
    browser::replace_patch(&mut h.editor, &mut h.ui, log);
    h.frame();
    assert!(!h.ui.explain.is_open() && h.ui.explain.target.is_none());
    // Back after a replacement does not restore the old patch's view.
    h.frame();
    assert!(!h.ui.hits.contains_key("explain-back"));
}

#[test]
fn escape_and_text_fields_keep_their_priority() {
    let mut h = H::new("palette/pad", 1440.0, 900.0);
    h.click("pexplain:1.m1");
    // Escape first cancels a pending MIDI learn, then closes the explanation.
    h.ui.learn = Some((MACROS, "m2".into()));
    h.key(Key::Escape);
    assert!(h.ui.learn.is_none() && h.ui.explain.is_open());
    // Renaming a card: Escape ends the rename, not the explanation.
    h.ui.renaming = Some((MACROS, "m1".into(), "Warmth".into()));
    h.frame();
    h.frame();
    h.key(Key::Escape);
    assert!(h.ui.renaming.is_none());
    assert!(
        h.ui.explain.is_open(),
        "Escape in a text field is the field's"
    );
    assert!(
        !h.ctx.egui_wants_keyboard_input(),
        "the ended rename keeps no focus"
    );
    let before = h.snapshot();
    h.key(Key::Escape);
    assert!(!h.ui.explain.is_open());
    h.unchanged_since(&before);
}

#[test]
fn inline_help_is_opt_in_and_changes_nothing() {
    let mut h = H::new("init-keyboard", 1440.0, 900.0);
    let before = h.snapshot();
    assert!(!h.ui.explain.help);
    h.click("help");
    assert!(h.ui.explain.help);
    h.ui.selected_module = Some(3);
    h.frame();
    h.click("explain-module:3");
    assert_eq!(h.ui.explain.subject, Some(Subject::Module(3)));
    h.click("explain-show:3");
    assert_eq!(h.ui.selected_module, Some(3));
    h.unchanged_since(&before);
    // Common controls stay reachable with help closed.
    h.click("help");
    h.ui.explain.close();
    h.frame();
    h.frame();
    for k in [
        "pslider:3.cutoff_hz",
        "pslider:6.attack_ms",
        "knob:3.cutoff_hz",
    ] {
        assert!(h.ui.hits.contains_key(k), "{k}");
    }
}

/// Every factory sound directory under `patches/` (one with a `log.jsonl`).
fn factory() -> Vec<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches");
    let mut out = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(d) = stack.pop() {
        if d.join("log.jsonl").exists() {
            out.push(
                d.strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            );
            continue;
        }
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            if e.path().is_dir() {
                stack.push(e.path());
            }
        }
    }
    out.sort();
    out
}

/// One coverage row per Perform card of every factory sound: its label, what it really is,
/// and which help it gets.
fn coverage_rows() -> Vec<[String; 4]> {
    let mut rows = Vec::new();
    for patch in factory() {
        let e = load(&patch);
        let s = e.state();
        for pin in perform::pins(s) {
            let kind = s.modules[&pin.id].kind.clone();
            let label = perform::pin_label(s, &pin);
            let what = perform::pin_source(s, &pin);
            let help = if let Some(_h) = kabl_ui::help::special_pin_help(&pin.key) {
                "specific (card help + what it acts on)".to_string()
            } else if kind == "macro" {
                let (d, _) = explain::destinations(s, pin.id, &pin.key);
                let names: Vec<String> = d
                    .iter()
                    .filter(|d| d.via.is_none())
                    .map(|d| explain::dest_title(s, d))
                    .collect();
                format!("destinations: {}", names.join("; "))
            } else {
                match kabl_ui::help::param_help(&kind, &pin.key) {
                    Some(_) => "specific parameter help".to_string(),
                    None => "FALLBACK (range only)".to_string(),
                }
            };
            rows.push([patch.clone(), label, what, help]);
        }
    }
    rows
}

#[test]
fn every_factory_card_gets_specific_help() {
    let rows = coverage_rows();
    assert!(rows.len() > 60, "{} cards", rows.len());
    for r in &rows {
        assert!(!r[3].contains("FALLBACK"), "{r:?}");
        assert!(
            r[3] != "destinations: ",
            "a factory macro with no destination: {r:?}"
        );
    }
}

/// Writes `docs/musical-controls/coverage.md` from the factory sounds as they are.
#[test]
#[ignore]
fn write_help_coverage() {
    use std::fmt::Write;
    let mut md = String::from(
        "# Help coverage — factory Perform cards\n\nGenerated by `cargo test -p kabl-ui --test \
         explain write_help_coverage -- --ignored` from the patches as they are. \
         \"Card\" is the author's label (intent); \"Controls\" is what the patch graph says it \
         is. Macro destinations are listed from the macro output's cables at generation \
         time; the app recomputes them every frame.\n\n| Sound | Card | Controls | Help |\n|---|---|---|---|\n",
    );
    for [p, l, w, h] in coverage_rows() {
        writeln!(md, "| {p} | {l} | {w} | {h} |").unwrap();
    }
    md.push_str(
        "\n## Parameter help per module kind\n\nEvery parameter of every built-in module \
         has authored help (`crates/ui/src/help.rs`, checked by `every_builtin_param_has_help`). \
         Range, unit, taper and stepped choices come from `ParamInfo` and the rack's own \
         formatters.\n\n| Kind | Module | Parameters |\n|---|---|---|\n",
    );
    for info in kabl_modules::registry::all_infos() {
        let mut names: Vec<String> = info
            .params
            .iter()
            .map(|p| kabl_modules::builtins::seq::slot_name(p.name).to_string())
            .collect();
        names.dedup();
        let names: Vec<String> = names
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        writeln!(
            md,
            "| `{}` | {} | {} |",
            info.kind,
            info.name,
            if names.is_empty() {
                "—".into()
            } else {
                names.join(", ")
            }
        )
        .unwrap();
    }
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/musical-controls/coverage.md");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    std::fs::write(out, md).unwrap();
}

#[test]
fn a_direct_pin_to_an_off_face_control_reveals_it() {
    // palette/lead "Glide time" is MIDI In #2 glide_ms, an advanced (off-face) control.
    let mut h = H::new("palette/lead", 1280.0, 800.0);
    let before = h.snapshot();
    assert!(
        !h.ui.hits.contains_key("knob:2.glide_ms"),
        "starts off the face"
    );
    h.click("pexplain:2.glide_ms");
    h.click("explain-show:2.glide_ms");
    assert_eq!(h.ui.inspected, Some((2, "glide_ms".into())));
    assert!(h.ui.expanded.contains(&2));
    assert!(h.ui.hits.contains_key("knob:2.glide_ms"));
    h.unchanged_since(&before);
    h.click("explain-back");
    assert!(!h.ui.expanded.contains(&2));
    h.unchanged_since(&before);
}

#[test]
fn a_legacy_mixer_level_route_is_explained_on_channel_1() {
    // An old patch's route to the shared `level` reaches channel 1 in the engine.
    let mut ed = PatchEditor::new();
    let mac = ed.add_module("macro", kabl_core::Vec2::default());
    let mix = ed.add_module("mixer", kabl_core::Vec2::default());
    let c = ed.connect(
        PortRef::Module {
            id: mac,
            port: "m1".into(),
        },
        PortRef::Param {
            id: mix,
            param: "level".into(),
        },
    );
    let s = ed.state();
    let (rows, _) = explain::destinations(s, mac, "m1");
    assert!(matches!(&rows[0].reach, Reach::Param { param, .. } if param.name == "level1"));
    let l1 = explain::param_of(s, mix, "level1").unwrap();
    assert_eq!(routing::routes_into(s, mix, "level1")[0].cable, c);
    assert!(routing::routes_into(s, mix, "level2").is_empty());
    let lines = explain::base_and_modulation(s, mix, l1);
    assert!(
        !lines.iter().any(|l| l.starts_with("Modulation: none")),
        "{lines:?}"
    );
}

#[test]
fn transport_acts_on_sequencers_behind_a_divider() {
    // interlocking: Clock #1 → Sequencer #3, and → Divider #2 → Sequencer #4.
    let e = load("interlocking");
    assert_eq!(explain::clocked(e.state(), 1), [3, 4]);
}

#[test]
fn edits_through_the_card_and_the_rack_agree_while_explained() {
    let mut h = H::new("init-keyboard", 1440.0, 900.0);
    h.click("pexplain:3.cutoff_hz");
    let p = param(&h, 3, "cutoff_hz");
    let base = |h: &H| routing::base_value(h.editor.state(), 3, p);
    let b0 = base(&h);
    h.drag("pslider:3.cutoff_hz", 30.0);
    let b1 = base(&h);
    assert!(b1 > b0, "the card slider moves the stored base");
    assert!(explain::base_and_modulation(h.editor.state(), 3, p)[0]
        .contains(&routing::fmt_value(p, b1)));
    // The same value from the rack's drawer control.
    h.click("explain-show:3.cutoff_hz");
    h.drag("dparam:3.cutoff_hz", -60.0);
    let b2 = base(&h);
    assert!(b2 < b1);
    assert!(
        browser::is_modified(&h.editor, &h.ui),
        "real edits do mark the document"
    );
    h.editor.undo();
    h.editor.undo();
    h.frame();
    assert_eq!(base(&h), b0);
    assert!(!browser::is_modified(&h.editor, &h.ui));
}

#[test]
fn escape_with_a_menu_open_closes_only_the_menu() {
    let mut h = H::new("palette/pad", 1440.0, 900.0);
    h.click("pexplain:1.m1");
    h.click("pmenu:1.m1");
    assert!(egui::Popup::is_any_open(&h.ctx), "the card menu is open");
    h.key(Key::Escape);
    assert!(h.ui.explain.is_open());
    h.frame();
    h.frame();
    h.key(Key::Escape);
    assert!(!h.ui.explain.is_open());
}

#[test]
fn a_card_rename_commits_when_you_click_elsewhere() {
    let mut h = H::new("palette/pad", 1440.0, 900.0);
    h.ui.renaming = Some((MACROS, "m1".into(), "Heat".into()));
    h.frame();
    h.frame();
    let undo = h.editor.log().entries().len();
    h.click("pexplain:1.m2");
    assert!(h.ui.renaming.is_none());
    assert_eq!(
        perform::pin_label(h.editor.state(), &pin(MACROS, "m1")),
        "Heat"
    );
    assert_eq!(h.editor.log().entries().len(), undo + 1, "one undo step");
    assert!(!h.ctx.egui_wants_keyboard_input(), "no focus left behind");
}
