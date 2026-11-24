//! The lesson runner on the real factory patches, through `kabl_ui::show`: every step of every
//! lesson completes only once its conditions hold, the runner never edits the patch, and
//! progress survives a save and reload of library.json.

use std::path::Path;

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect};
use kabl_core::PortRef;
use kabl_ui::inspect::Sel;
use kabl_ui::lessons::{self, Cond, Progress};
use kabl_ui::library::{Library, Prefs};
use kabl_ui::{compare, explain, routing, show, PatchEditor, UiState};

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    t: f64,
}

impl H {
    fn new(user: &Path) -> Self {
        let mut ui = UiState::default();
        ui.library = Some(Library::open(
            Some(kabl_ui::library::built_factory_dir()),
            user.to_path_buf(),
        ));
        let mut h = H {
            ctx: egui::Context::default(),
            editor: PatchEditor::seed_from(&kabl_standalone::default_patch()),
            ui,
            t: 0.0,
        };
        h.frame();
        h.frame();
        h
    }

    fn frame_with(&mut self, events: Vec<Event>) {
        let size = egui::vec2(1440.0, 900.0);
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            events,
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
        // Only the UI state is checked here, not what was painted.
        drop(self.ctx.end_pass());
    }

    fn frame(&mut self) {
        self.frame_with(Vec::new());
    }

    fn click(&mut self, key: &str) {
        let p = self
            .ui
            .hits
            .get(key)
            .unwrap_or_else(|| panic!("no hit target {key}"))
            .center();
        self.frame_with(vec![Event::PointerMoved(p)]);
        for pressed in [true, false] {
            self.frame_with(vec![Event::PointerButton {
                pos: p,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            }]);
        }
        self.frame();
    }

    /// Opens the Learn tab and starts lesson `id` from its card (no unsaved work here).
    fn start(&mut self, id: &str) {
        self.click("learn-open");
        self.click(&format!("lesson:{id}"));
        assert!(self.ui.lessons.active.is_some(), "{id} did not start");
    }

    fn step(&self) -> usize {
        self.ui.lessons.active.as_ref().unwrap().step
    }
}

fn route_cable(h: &H, from: u64, port: &str, to: u64, param: &str) -> kabl_core::CableId {
    *h.editor
        .state()
        .cables
        .iter()
        .find(|(_, c)| {
            matches!(&c.from, PortRef::Module { id, port: p } if *id == from && p == port)
                && matches!(&c.to, PortRef::Param { id, param: p } if *id == to && p == param)
        })
        .expect("the lesson's route is in its patch")
        .0
}

fn value(h: &H, id: u64, param: &str) -> f32 {
    let info = explain::param_of(h.editor.state(), id, param).expect("the lesson's parameter");
    routing::base_value(h.editor.state(), id, info)
}

/// Does what the user would to make `c` hold. When the condition has a threshold, first goes
/// only part of the way and asserts that alone did not complete `step`.
fn satisfy(h: &mut H, c: &Cond, step: usize) {
    match c {
        Cond::Inspecting { id, port } => {
            h.ui.inspect.select(
                Sel {
                    id: *id,
                    port: port.clone(),
                },
                h.t,
            );
        }
        Cond::ParamMoved { id, param, by, .. } => {
            let start = value(h, *id, param);
            h.editor.set_param(*id, param, start + by * 0.4);
            h.frame();
            assert_eq!(h.step(), step, "{c:?}: completed on less than `by`");
            h.editor.set_param(*id, param, start + by);
        }
        Cond::Route {
            from,
            port,
            to,
            param,
            amount_below,
            ..
        } => {
            let cable = route_cable(h, *from, port, *to, param);
            h.editor.set_route_bypass(cable, false);
            if let Some(below) = amount_below {
                h.frame();
                assert_eq!(h.step(), step, "{c:?}: completed before the depth crossed");
                h.editor.set_route_amount(cable, below - 0.2, true);
            }
        }
        Cond::CompareRestored => {
            compare::restore(&mut h.editor, &mut h.ui);
        }
        Cond::AnyParamMoved { id, params, by, .. } => {
            let p = &params[0];
            let start = value(h, *id, p);
            h.editor.set_param(*id, p, start + by * 0.4);
            h.frame();
            assert_eq!(h.step(), step, "{c:?}: completed on less than `by`");
            h.editor.set_param(*id, p, start + by);
        }
        Cond::SoundsOpen => h.ui.browser_open = true,
        Cond::Opened(_) => unreachable!("no shipped lesson opens a second sound mid-way"),
    }
}

#[test]
fn every_step_completes_only_once_its_conditions_hold() {
    // The Start card's lesson opens with a listen-only step; tests/first_run.rs walks it.
    for l in lessons::all().iter().filter(|l| !l.card) {
        let user = tempfile::tempdir().unwrap();
        let mut h = H::new(user.path());
        h.start(&l.id);
        for (i, s) in l.steps.iter().enumerate() {
            assert_eq!(h.step(), i, "{}: not on step {}", l.id, i + 1);
            assert!(!s.done.is_empty(), "{} step {}: no conditions", l.id, i + 1);
            for _ in 0..5 {
                h.frame();
            }
            assert_eq!(h.step(), i, "{} step {}: completed on its own", l.id, i + 1);
            for (k, c) in s.done.iter().enumerate() {
                satisfy(&mut h, c, i);
                h.frame();
                if k + 1 < s.done.len() {
                    assert_eq!(h.step(), i, "{} step {}: completed early", l.id, i + 1);
                }
            }
            h.frame();
            assert_eq!(h.step(), i + 1, "{} step {}: never completed", l.id, i + 1);
        }
        assert_eq!(h.step(), l.steps.len());
    }
}

fn inspect_envelope(h: &mut H) {
    h.ui.inspect.select(
        Sel {
            id: 6,
            port: "out".into(),
        },
        h.t,
    );
}

fn step_done(h: &H) -> bool {
    let a = h.ui.lessons.active.as_ref().unwrap();
    a.done[a.step]
}

/// A step whose conditions already hold when it is reached counts as done at once: it stays
/// put, shows done, and Next moves on. Nothing has to be undone and redone.
#[test]
fn a_step_already_satisfied_when_reached_is_done_and_next_moves_on() {
    let user = tempfile::tempdir().unwrap();
    let mut h = H::new(user.path());
    h.start("pluck-to-pad");
    h.click("lesson-next");
    assert_eq!(h.step(), 1);
    inspect_envelope(&mut h);
    h.click("lesson-prev");
    for _ in 0..5 {
        h.frame();
    }
    assert_eq!(h.step(), 0, "a step met on arrival does not skip itself");
    assert!(step_done(&h), "a step met on arrival counts as done");
    h.click("lesson-next");
    assert_eq!(h.step(), 1);
}

/// Going back to a step completed earlier shows it done and asks for nothing again.
#[test]
fn a_completed_step_stays_done_when_revisited() {
    let user = tempfile::tempdir().unwrap();
    let mut h = H::new(user.path());
    h.start("pluck-to-pad");
    inspect_envelope(&mut h);
    h.frame();
    assert_eq!(h.step(), 1);
    h.ui.inspect.clear();
    h.click("lesson-prev");
    for _ in 0..5 {
        h.frame();
    }
    assert_eq!(h.step(), 0);
    assert!(
        step_done(&h),
        "a completed step stays done though its condition no longer holds"
    );
    h.click("lesson-next");
    assert_eq!(h.step(), 1);
    assert!(!step_done(&h), "step 2 was never done");
}

/// The runner is view only: walking every lesson with its own buttons (Show, Inspect, test
/// note, Compare, Next) adds nothing to the op log and leaves the patch as it opened.
#[test]
fn the_runner_never_edits_the_patch() {
    for l in lessons::all() {
        let user = tempfile::tempdir().unwrap();
        let mut h = H::new(user.path());
        h.start(&l.id);
        let ops = h.editor.log().all_entries().len();
        let state = h.editor.state().clone();
        for _ in 0..4 * l.steps.len() {
            if h.step() >= l.steps.len() {
                break;
            }
            for key in [
                "lesson-show",
                "lesson-inspect",
                "lesson-note",
                "lesson-compare",
            ] {
                if h.ui.hits.contains_key(key) && h.step() < l.steps.len() {
                    h.click(key);
                }
            }
            for _ in 0..30 {
                h.frame();
            }
            if h.step() < l.steps.len() {
                h.click("lesson-next");
            }
        }
        assert_eq!(h.step(), l.steps.len(), "{}", l.id);
        assert_eq!(h.editor.log().all_entries().len(), ops, "{}", l.id);
        assert_eq!(h.editor.state(), &state, "{}", l.id);
    }
}

#[test]
fn progress_survives_library_json_and_a_reopened_lesson_resumes() {
    let user = tempfile::tempdir().unwrap();
    {
        let mut h = H::new(user.path());
        h.start("filter-movement");
        h.click("lesson-next");
        h.click("lesson-next");
        assert_eq!(h.step(), 2);
    }
    let saved = Library::open(None, user.path().into()).prefs.lessons["filter-movement"];
    assert_eq!(
        saved,
        Progress {
            step: 2,
            done: false
        }
    );
    // A new session (or the plugin) reads the same file and resumes at the saved step.
    let mut h = H::new(user.path());
    h.start("filter-movement");
    assert_eq!(h.step(), 2);
    let n = lessons::all()
        .iter()
        .find(|l| l.id == "filter-movement")
        .unwrap()
        .steps
        .len();
    for _ in 2..n {
        h.click("lesson-next");
    }
    let saved = Library::open(None, user.path().into()).prefs.lessons["filter-movement"];
    assert!(saved.done);
    // Finished: opening it again starts from the top.
    h.click("routing-tab");
    h.start("filter-movement");
    assert_eq!(h.step(), 0);
}

#[test]
fn a_library_json_from_before_lessons_loads_with_no_progress() {
    let old = r#"{"favorites":["factory:palette/pad"],"recents":[],"first_run_done":true}"#;
    let p: Prefs = serde_json::from_str(old).unwrap();
    assert!(p.lessons.is_empty());
    assert!(p.first_run_done && p.favorites.contains("factory:palette/pad"));
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("library.json"), old).unwrap();
    let lib = Library::open(None, dir.path().into());
    assert_eq!(lib.prefs, p, "an old file keeps everything it had");
}
