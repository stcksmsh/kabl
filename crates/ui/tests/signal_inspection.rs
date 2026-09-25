//! D03 through real egui input on the factory sounds: selecting a signal never edits,
//! measurements are accepted only for the current selection and graph lineage, "Why no
//! sound?" keeps facts, measurements and possibilities apart, the comparison reference restores
//! as one undo step, and the recipes start through the ordinary document protection.

use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect};
use kabl_core::{PatchState, PortRef};
use kabl_engine::patch_engine::Command;
use kabl_engine::probe::{LaneStats, ProbeReport, ProbeStatus, ProbeTarget, PROBE_LANES};
use kabl_ui::browser::{is_modified, Dialog, DocOrigin};
use kabl_ui::inspect::{self, Context, Sel, Status};
use kabl_ui::library::Library;
use kabl_ui::{compare, recipes, show, PatchEditor, UiState};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn factory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches")
}

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    size: egui::Vec2,
    events: Vec<Event>,
    t: f64,
    _user: tempfile::TempDir,
}

impl H {
    fn new(w: f32, h: f32) -> Self {
        let user = tempfile::tempdir().unwrap();
        let mut ui = UiState::default();
        ui.library = Some(Library::open(Some(factory()), user.path().to_path_buf()));
        ui.inspect.audio = true;
        let mut h = H {
            ctx: egui::Context::default(),
            editor: PatchEditor::seed_from(&kabl_standalone::default_patch()),
            ui,
            size: egui::vec2(w, h),
            events: Vec::new(),
            t: 0.0,
            _user: user,
        };
        h.frame();
        h.frame();
        h
    }

    /// Opens a factory sound the way the browser does (no unsaved work here).
    fn open(&mut self, id: &str) {
        kabl_ui::browser::request(
            &mut self.editor,
            &mut self.ui,
            kabl_ui::browser::Pending::Open(id.into()),
        );
        self.frame();
        self.frame();
        self.editor.take_dirty();
        // main.rs clears these when it sends the fresh graph.
        self.ui.loaded = false;
        self.ui.load_stopped = false;
        self.ui.launches.clear();
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

    fn click(&mut self, key: &str) {
        let p = self
            .ui
            .hits
            .get(key)
            .unwrap_or_else(|| panic!("no hit target {key}"))
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
        self.frame();
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) {
        for pressed in [true, false] {
            self.events.push(Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            });
        }
        self.frame();
        self.frame();
    }

    fn snapshot(&self) -> (PatchState, usize, bool, bool) {
        (
            self.editor.state().clone(),
            self.editor.log().all_entries().len(),
            self.editor.can_undo(),
            is_modified(&self.editor, &self.ui),
        )
    }

    /// The last Inspect command sent, if any; clears the queue.
    fn sent(&mut self) -> Option<Option<ProbeTarget>> {
        let last = self.ui.launches.iter().rev().find_map(|c| match c {
            Command::Inspect(t) => Some(*t),
            _ => None,
        });
        self.ui.launches.clear();
        last
    }

    /// Selects output `port` of `id` with the drawer's port button.
    fn inspect(&mut self, id: u64, port: &str) -> ProbeTarget {
        self.ui.selected_module = Some(id);
        if !self.ui.inspect.open {
            self.click("inspect-open");
        }
        self.frame();
        self.click(&format!("inspect-port:{id}.{port}"));
        self.sent().flatten().expect("an Inspect command")
    }
}

fn report(t: &ProbeTarget, generation: u64, seq: u64, lane: LaneStats) -> ProbeReport {
    let mut lanes = [LaneStats::default(); PROBE_LANES];
    lanes[0] = lane;
    ProbeReport {
        token: t.token,
        generation,
        seq,
        status: ProbeStatus::Measured,
        fading: false,
        samples: 2432,
        lanes: 1,
        lanes_total: 1,
        voiced: false,
        lane: lanes,
    }
}

fn level(x: f32) -> LaneStats {
    LaneStats {
        min: -x,
        max: x,
        peak: x,
        sum_sq: x * x * 2432.0,
        last: x,
        ..Default::default()
    }
}

// patches/init-keyboard: MIDI #1, Osc #2, Filter #3, VCA #4, Output #5, ADSR #6.
const INIT: &str = "factory:init-keyboard";

#[test]
fn selecting_opening_and_closing_inspection_never_edits() {
    for (w, hgt) in [(1280.0, 800.0), (1440.0, 900.0)] {
        let mut h = H::new(w, hgt);
        h.open(INIT);
        let before = h.snapshot();
        let t = h.inspect(4, "out");
        assert_eq!((t.module, t.port, t.kind), (4, 0, "vca"));
        let t2 = h.inspect(6, "out");
        assert!(t2.token > t.token, "a new selection, a new token");
        assert!(Rect::from_min_size(Pos2::ZERO, h.size).contains_rect(h.ui.hits["why"]));
        h.click("why");
        h.click("inspect-close");
        assert_eq!(h.sent(), Some(None), "closing turns measurement off");
        // Closing the drawer also stops it.
        h.inspect(6, "out");
        h.click("routing");
        assert_eq!(h.sent(), Some(None));
        assert_eq!(h.snapshot(), before, "no edit, no undo step, not modified");
        assert!(!h.editor.take_dirty(), "no rebuild");
        assert!(h.ui.transport.is_empty());
    }
}

#[test]
fn only_current_measurements_are_shown_as_current() {
    let mut h = H::new(1440.0, 900.0);
    h.open(INIT);
    h.ui.inspect.rebuilt(10, None, h.editor.state());
    let t = h.inspect(4, "out");
    let now = h.t;
    let st = |h: &H| h.ui.inspect.status(h.editor.state(), h.t);
    assert_eq!(st(&h), Status::Waiting);
    // Another selection's report is ignored.
    let mut other = t;
    other.token += 99;
    assert!(!h.ui.inspect.accept(report(&other, 10, 0, level(0.5)), now));
    assert!(h.ui.inspect.accept(report(&t, 10, 0, level(0.5)), now));
    assert_eq!(st(&h), Status::Live);

    // A parameter edit: the older graph's report still counts (labelled while the edit
    // compiles), so a turning knob keeps its meter.
    h.editor.set_param(3, "cutoff_hz", 900.0);
    h.ui.inspect.rebuilt(11, None, h.editor.state());
    assert!(h.ui.inspect.accept(report(&t, 10, 1, level(0.4)), h.t));
    // A topology edit: reports from before it are not the patch on screen.
    h.editor
        .disconnect(*h.editor.state().cables.keys().next().unwrap());
    h.ui.inspect.rebuilt(12, None, h.editor.state());
    assert!(!h.ui.inspect.accept(report(&t, 11, 2, level(0.4)), h.t));
    assert!(h.ui.inspect.accept(report(&t, 12, 3, level(0.3)), h.t));
    // Overlapping swaps can't move it back.
    assert!(!h.ui.inspect.accept(report(&t, 11, 4, level(0.3)), h.t));
    assert_eq!(
        h.ui.inspect.dropped, 1,
        "seq 2 was accepted-rejected, seq gap counted once"
    );

    // A failed compile: unavailable, never an old value as current.
    h.ui.inspect
        .rebuilt(13, Some("boom".into()), h.editor.state());
    assert!(matches!(st(&h), Status::NotCompiled(_)));
    assert!(!h.ui.inspect.accept(report(&t, 12, 5, level(0.3)), h.t));

    // Stale after half a second without reports.
    h.ui.inspect.rebuilt(14, None, h.editor.state());
    assert!(h.ui.inspect.accept(report(&t, 14, 6, level(0.3)), h.t));
    for _ in 0..40 {
        h.frame();
    }
    assert!(matches!(st(&h), Status::Stale(a) if a > 0.5));

    // Deleting the module: missing; undo brings it back.
    h.editor.remove_module(4);
    h.frame();
    assert_eq!(st(&h), Status::Missing(4));
    h.editor.undo();
    h.frame();
    assert_ne!(st(&h), Status::Missing(4));
}

#[test]
fn another_sound_never_inherits_a_measurement_by_reused_id() {
    let mut h = H::new(1440.0, 900.0);
    h.open(INIT);
    let t = h.inspect(4, "out");
    assert!(h.ui.inspect.accept(report(&t, 1, 0, level(0.5)), h.t));
    // The pluck recipe patch has a VCA #4 too.
    h.open("factory:recipes/pluck-to-pad");
    assert!(
        h.ui.inspect.sel.is_none(),
        "selection dropped with the old sound"
    );
    assert!(!h.ui.inspect.accept(report(&t, 2, 1, level(0.5)), h.t));
}

#[test]
fn a_stuck_command_queue_is_retried() {
    let mut h = H::new(1440.0, 900.0);
    h.open(INIT);
    let t = h.inspect(4, "out");
    for _ in 0..80 {
        h.frame();
    }
    assert_eq!(h.sent(), Some(Some(t)), "re-sent after waiting");
    assert_eq!(
        h.ui.inspect.status(h.editor.state(), h.t),
        Status::NotArriving
    );
}

fn cx<'a>(
    clocks: &'a HashMap<u64, bool>,
    tap: Option<(&'a Sel, kabl_modules::PortType, Option<inspect::Summary>)>,
) -> Context<'a> {
    Context {
        clock_running: clocks,
        output_level: Some(0.0),
        tap,
        midi_input: None,
        sample_rate: 48000.0,
    }
}

fn summary(t_stats: LaneStats, lanes: usize) -> inspect::Summary {
    let mut i = inspect::Inspect::default();
    i.audio = true;
    i.select(
        Sel {
            id: 1,
            port: "x".into(),
            via_cable: false,
        },
        0.0,
    );
    // token 1 after the first select
    let mut r = report(
        &ProbeTarget {
            token: 1,
            module: 1,
            port: 0,
            kind: "x",
        },
        0,
        0,
        t_stats,
    );
    r.lanes = lanes as u8;
    r.lanes_total = lanes as u16;
    r.voiced = lanes > 1;
    for l in 0..lanes {
        r.lane[l] = t_stats;
    }
    assert!(i.accept(r, 0.0));
    i.summary(0.0).unwrap()
}

fn init_state() -> PatchState {
    kabl_core::load(&factory().join("init-keyboard"))
        .unwrap()
        .state()
        .clone()
}

fn cable_to(s: &PatchState, id: u64, port: &str) -> u64 {
    *s.cables
        .iter()
        .find(|(_, c)| matches!(&c.to, PortRef::Module { id: i, port: p } if *i == id && p == port))
        .unwrap()
        .0
}

#[test]
fn why_no_sound_a_removed_connection_is_a_graph_fact() {
    let clocks = HashMap::new();
    let mut s = init_state();
    let c = cable_to(&s, 5, "left");
    s.cables.remove(&c);
    let c = cable_to(&s, 5, "right");
    s.cables.remove(&c);
    let d = inspect::diagnose(&s, Some(2), &cx(&clocks, None));
    assert!(
        d.facts
            .iter()
            .any(|f| f.contains("Nothing is plugged into Output #5")),
        "{d:?}"
    );
    assert!(d
        .facts
        .iter()
        .any(|f| f.contains("No signal cable path leads from Osc #2")));
    // Facts only from the graph; no measurement is claimed.
    assert!(d.measured.iter().all(|m| !m.contains("peak")));
}

#[test]
fn why_no_sound_a_missing_trigger_is_an_observation() {
    let clocks = HashMap::new();
    let s = init_state();
    // MIDI #1 gate observed for a second: no edge, 8 lanes.
    let sel = Sel {
        id: 1,
        port: "gate".into(),
        via_cable: false,
    };
    let sum = summary(LaneStats::default(), 8);
    let d = inspect::diagnose(
        &s,
        None,
        &cx(
            &clocks,
            Some((&sel, kabl_modules::PortType::Gate, Some(sum))),
        ),
    );
    assert!(
        d.measured
            .iter()
            .any(|m| m.contains("MIDI #1 gate: no rising edge above 0.5 over the last")),
        "{d:?}"
    );
    assert!(d
        .possible
        .iter()
        .any(|p| p.contains("No key started a note")));
    assert!(
        !d.facts
            .iter()
            .any(|f| f.contains("No cable") || f.contains("Nothing is plugged")),
        "the graph is fine: {d:?}"
    );
    // The same patch with the envelope's gate cable removed: a graph fact names it.
    let mut broken = s.clone();
    let c = cable_to(&broken, 6, "gate");
    broken.cables.remove(&c);
    let d = inspect::diagnose(&broken, Some(4), &cx(&clocks, None));
    assert!(
        d.facts
            .iter()
            .any(|f| f.contains("No cable into: ADSR #6 gate")),
        "{d:?}"
    );
}

#[test]
fn why_no_sound_does_not_diagnose_a_valid_quiet_or_stopped_patch() {
    // A held gate (no new edge) and a stopped piece are not faults.
    let s = init_state();
    let clocks = HashMap::new();
    let sel = Sel {
        id: 1,
        port: "gate".into(),
        via_cable: false,
    };
    let held = LaneStats {
        min: 1.0,
        max: 1.0,
        peak: 1.0,
        high: 2432,
        last: 1.0,
        ..Default::default()
    };
    let d = inspect::diagnose(
        &s,
        None,
        &cx(
            &clocks,
            Some((&sel, kabl_modules::PortType::Gate, Some(summary(held, 1)))),
        ),
    );
    assert!(d.measured.iter().any(|m| m.contains("held high")), "{d:?}");
    assert!(
        d.facts.iter().all(|f| f.starts_with("Signal path")),
        "nothing wrong in the graph: {d:?}"
    );
    assert!(
        !d.possible.iter().any(|p| p.contains("No key started")),
        "a held key is not a missing trigger: {d:?}"
    );

    let piece = kabl_core::load(&factory().join("interlocking"))
        .unwrap()
        .state()
        .clone();
    let stopped = HashMap::from([(1u64, false)]);
    let d = inspect::diagnose(&piece, None, &cx(&stopped, None));
    assert_eq!(
        d.facts,
        vec!["Clock #1 is stopped (transport): what it drives gets no new steps.".to_string()]
    );
    // Audio measured at a stage: present signal points downstream, not at a fault.
    let sel = Sel {
        id: 11,
        port: "out".into(),
        via_cable: false,
    };
    let running = HashMap::new();
    let d = inspect::diagnose(
        &piece,
        None,
        &cx(
            &running,
            Some((
                &sel,
                kabl_modules::PortType::Audio,
                Some(summary(level(0.2), 1)),
            )),
        ),
    );
    assert!(
        d.facts.iter().all(|f| f.starts_with("Signal path")),
        "{d:?}"
    );
    assert!(d
        .possible
        .iter()
        .any(|p| p.contains("inspect the next stage, Mixer #6")));
}

#[test]
fn why_no_sound_names_the_output_the_engine_plays() {
    let mut s = init_state();
    s.modules.insert(
        9,
        kabl_core::ModuleState {
            kind: "out".into(),
            pos: kabl_core::Vec2 { x: 0.0, y: 0.0 },
            params: Default::default(),
        },
    );
    let clocks = HashMap::new();
    let d = inspect::diagnose(&s, Some(4), &cx(&clocks, None));
    assert!(
        d.facts
            .iter()
            .any(|f| f.contains("2 Output modules: the engine plays only Output #")),
        "{d:?}"
    );
}

#[test]
fn a_restore_is_one_undo_step_and_keeps_the_working_version() {
    let mut h = H::new(1440.0, 900.0);
    h.open("factory:palette/pad");
    h.click("compare-open");
    let log_before = h.editor.log().all_entries().len();
    h.click("compare-capture");
    assert_eq!(
        h.editor.log().all_entries().len(),
        log_before,
        "capture edits nothing"
    );
    assert!(!is_modified(&h.editor, &h.ui));
    let reference = h.editor.state().clone();
    // An experiment plus an unrelated edit, a new module, a label and a route change.
    h.editor.set_param(7, "cutoff_hz", 333.0);
    h.editor.set_param(8, "attack_ms", 5.0);
    let added = h
        .editor
        .add_module("noise", kabl_core::Vec2 { x: 10.0, y: 10.0 });
    h.editor.set_label(7, "pin.cutoff_hz", Some("Dark".into()));
    let c = *h.editor.state().cables.keys().next().unwrap();
    h.editor.disconnect(c);
    h.frame();
    let working = h.editor.state().clone();
    let depth = h.editor.log().entries().len();
    // Restore asks first; Cancel changes nothing.
    h.click("compare-restore");
    assert!(matches!(h.ui.browser.dialog, Some(Dialog::Restore)));
    h.click("dlg:cancel");
    assert_eq!(h.editor.state(), &working);
    // Escape cancels too.
    h.click("compare-restore");
    h.key(Key::Escape, Modifiers::NONE);
    assert!(h.ui.browser.dialog.is_none());
    assert_eq!(h.editor.state(), &working);
    h.click("compare-restore");
    h.click("dlg:restore");
    assert_eq!(h.editor.state(), &reference, "exactly the reference");
    assert_eq!(h.editor.log().entries().len(), depth + 1, "one step");
    assert!(h.editor.take_dirty(), "rebuilds like an ordinary edit");
    assert!(!h.ui.loaded, "not a fresh load: notes and tails continue");
    // Back to my version: everything, including the unrelated edits.
    h.click("compare-undo");
    assert_eq!(h.editor.state(), &working);
    h.click("compare-redo");
    assert_eq!(h.editor.state(), &reference);
    h.key(Key::Z, Modifiers::COMMAND);
    assert_eq!(h.editor.state(), &working, "Ctrl+Z is the same");
    // The earlier history is intact: undo goes on through the edits before the capture.
    let mut n = 0;
    while h.editor.undo() {
        n += 1;
    }
    assert!(n >= 5);
    assert!(!h.editor.state().modules.contains_key(&added));
    // New ids never collide with restored ones.
    while h.editor.redo() {}
    let next = h
        .editor
        .add_module("noise", kabl_core::Vec2 { x: 0.0, y: 0.0 });
    assert!(next > added);
}

#[test]
fn the_restore_difference_reproduces_every_factory_sound() {
    let lib = Library::open(Some(factory()), tempfile::tempdir().unwrap().path().into());
    let states: Vec<PatchState> = lib
        .entries
        .iter()
        .map(|e| lib.read(&e.id).unwrap().state().clone())
        .collect();
    for a in &states {
        for b in &states {
            let ops = compare::diff(a, b).expect("reproduces");
            let mut s = a.clone();
            s.apply(&kabl_core::Op::Group { ops });
            assert_eq!(&s, b);
        }
    }
}

#[test]
fn another_sound_drops_the_reference_and_a_failed_open_keeps_it() {
    let mut h = H::new(1440.0, 900.0);
    h.open(INIT);
    h.ui.compare.capture(&h.editor, "you");
    // A failed open (missing sound) changes nothing.
    kabl_ui::browser::perform(
        &mut h.editor,
        &mut h.ui,
        kabl_ui::browser::Pending::Open("factory:nope".into()),
    );
    h.frame();
    assert!(h.ui.compare.reference.is_some());
    h.open("factory:palette/pad");
    assert!(h.ui.compare.reference.is_none());
    assert!(h.ui.compare.note.as_deref().unwrap().contains("dropped"));
}

#[test]
fn save_targets_the_current_patch_while_comparing() {
    let mut h = H::new(1440.0, 900.0);
    h.open(INIT);
    h.ui.compare.capture(&h.editor, "you");
    h.editor.set_param(3, "cutoff_hz", 500.0);
    h.frame();
    // Save As over an open comparison: saves exactly what is heard and edited.
    h.ui.compare.open = true;
    h.ui.inspect.open = true;
    h.frame();
    h.click("save-as");
    match h.ui.browser.dialog.as_mut() {
        Some(Dialog::SaveAs { name, .. }) => *name = "Compared".into(),
        other => panic!("{other:?}"),
    }
    h.frame();
    h.click("dlg:save");
    let lib = h.ui.library.as_ref().unwrap();
    let saved = lib.read("user:compared").unwrap();
    assert_eq!(saved.state(), h.editor.state());
    assert!(
        h.ui.compare.reference.is_some(),
        "Save As keeps the same editor"
    );
    assert!(!is_modified(&h.editor, &h.ui));
}

#[test]
fn escape_in_a_dialog_over_inspection_and_comparison_closes_only_the_dialog() {
    let mut h = H::new(1280.0, 800.0);
    h.open(INIT);
    h.inspect(4, "out");
    h.ui.compare.open = true;
    h.frame();
    h.editor.set_param(3, "cutoff_hz", 500.0);
    h.frame();
    // Unsaved question from Open.
    kabl_ui::browser::request(
        &mut h.editor,
        &mut h.ui,
        kabl_ui::browser::Pending::Open("factory:palette/pad".into()),
    );
    h.frame();
    assert!(matches!(h.ui.browser.dialog, Some(Dialog::Unsaved { .. })));
    h.key(Key::Escape, Modifiers::NONE);
    assert!(h.ui.browser.dialog.is_none());
    assert!(h.ui.inspect.open && h.ui.compare.open && h.ui.inspect.sel.is_some());
    // Rename over the same.
    h.click("save-as");
    match h.ui.browser.dialog.as_mut() {
        Some(Dialog::SaveAs { name, .. }) => *name = "Mine".into(),
        other => panic!("{other:?}"),
    }
    h.frame();
    h.click("dlg:save");
    h.ui.browser.dialog = Some(Dialog::Rename {
        id: "user:mine".into(),
        name: "Mine 2".into(),
        error: None,
    });
    h.frame();
    h.key(Key::Escape, Modifiers::NONE);
    assert!(h.ui.browser.dialog.is_none());
    assert!(h.ui.inspect.open && h.ui.compare.open && h.ui.inspect.sel.is_some());
}

#[test]
fn every_recipe_target_resolves_on_its_patch() {
    let lib = Library::open(Some(factory()), tempfile::tempdir().unwrap().path().into());
    for r in &recipes::RECIPES {
        let state = lib.read(r.sound).unwrap().state().clone();
        assert!((3..=5).contains(&r.steps.len()), "{}", r.key);
        for s in r.steps {
            if let Some(t) = &s.target {
                assert!(recipes::resolve(&state, t).is_some(), "{}: {t:?}", r.key);
            }
            if let Some((id, kind, port)) = s.inspect {
                assert_eq!(state.modules[&id].kind, kind, "{}", r.key);
                assert!(inspect::port_type(&state, id, port).is_some());
            }
        }
    }
    // A same-id module of another kind is not a match.
    let mut s = lib
        .read("factory:recipes/pluck-to-pad")
        .unwrap()
        .state()
        .clone();
    s.modules.get_mut(&6).unwrap().kind = "lfo".into();
    let t = recipes::RECIPES[0].steps[1].target.unwrap();
    assert!(recipes::resolve(&s, &t).is_none());
}

#[test]
fn a_recipe_starts_through_the_unsaved_question_and_exit_keeps_the_patch() {
    for (w, hgt) in [(1280.0, 800.0), (1440.0, 900.0)] {
        let mut h = H::new(w, hgt);
        h.open(INIT);
        h.editor.set_param(3, "cutoff_hz", 444.0);
        h.frame();
        let song = h.editor.state().clone();
        h.click("learn-open");
        h.click("recipe:pluck-to-pad");
        assert!(matches!(h.ui.browser.dialog, Some(Dialog::Unsaved { .. })));
        h.click("dlg:cancel");
        assert_eq!(h.editor.state(), &song, "cancel: the song is untouched");
        assert!(h.ui.recipes.active.is_none());
        assert!(h
            .ui
            .recipes
            .note
            .as_deref()
            .unwrap()
            .contains("did not start"));

        h.click("recipe:pluck-to-pad");
        h.click("dlg:discard");
        let a = h.ui.recipes.active.clone().expect("started");
        assert_eq!((a.recipe, a.step, a.detached), (0, 0, false));
        assert_eq!(
            h.ui.doc.as_ref().unwrap().origin,
            DocOrigin::Library("factory:recipes/pluck-to-pad".into())
        );
        assert!(h.ui.compare.reference.is_some(), "starting point captured");
        // Step 1: inspect the envelope, play the fixed test note.
        h.click("recipe-inspect");
        assert_eq!(
            h.sent().flatten().map(|t| (t.module, t.kind)),
            Some((6, "env.adsr"))
        );
        h.click("recipe-note");
        assert!(h
            .ui
            .launches
            .iter()
            .any(|c| matches!(c, Command::Preview { count: 1, .. })));
        h.ui.launches.clear();
        // Step 2: Show reaches the envelope's Sustain; Back returns.
        h.click("recipe-next");
        h.click("recipe-show");
        assert_eq!(h.ui.inspected, Some((6, "sustain".into())));
        h.click("recipe-back");
        assert_ne!(h.ui.inspected, Some((6, "sustain".into())));
        // An edit, then the target deleted: a clear state, and undo brings it back.
        h.editor.set_param(6, "sustain", 0.7);
        h.editor.remove_module(6);
        h.frame();
        assert!(!h.ui.hits.contains_key("recipe-show"));
        h.editor.undo();
        h.frame();
        assert!(h.ui.hits.contains_key("recipe-show"));
        // Through the last step to the unguided task; close keeps the patch and edits.
        for _ in 0..4 {
            h.click("recipe-next");
        }
        assert_eq!(h.ui.recipes.active.as_ref().unwrap().step, 5);
        let kept = h.editor.state().clone();
        h.click("learn-close");
        assert!(h.ui.recipes.active.is_none());
        assert_eq!(h.editor.state(), &kept);
        assert_eq!(kept.modules[&6].params["sustain"], 0.7);
    }
}

#[test]
fn opening_another_sound_detaches_a_recipe() {
    let mut h = H::new(1440.0, 900.0);
    h.open(INIT);
    h.click("learn-open");
    h.click("recipe:filter-movement");
    assert!(h.ui.recipes.active.is_some());
    h.open("factory:palette/pad");
    assert!(h.ui.recipes.active.as_ref().unwrap().detached);
    assert!(!h.ui.hits.contains_key("recipe-show"));
    assert!(h.ui.hits.contains_key("recipe-reopen"));
    h.click("recipe-reopen");
    let a = h.ui.recipes.active.clone().unwrap();
    assert!(!a.detached && a.recipe == 1);
}
