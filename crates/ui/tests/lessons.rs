//! The lesson runner on the real factory patches, through `kabl_ui::show`: every step of every
//! lesson completes only once its conditions hold, the runner never edits the patch, and
//! progress survives a save and reload of library.json.

use std::path::Path;

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect};
use kabl_core::PortRef;
use kabl_ui::browser::{self, Dialog, DocOrigin, Pending};
use kabl_ui::inspect::Sel;
use kabl_ui::lessons::{self, Cond, Progress};
use kabl_ui::library::{Library, Prefs};
use kabl_ui::{compare, explain, routing, show, sound_preview, PatchEditor, UiState};

struct H {
    ctx: egui::Context,
    editor: PatchEditor,
    ui: UiState,
    t: f64,
    /// The audio side of the Sounds preview, once one is attached.
    preview: Option<sound_preview::Voice>,
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
            preview: None,
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
        Cond::Opened(id) => {
            browser::request(&mut h.editor, &mut h.ui, Pending::Open(id.clone()));
            h.frame();
        }
        Cond::PreviewPlayed => {
            let id = doc_id(h);
            play_preview(h, &id);
        }
        Cond::NotesPlayed => {
            h.ui.keys.strip_to(Some(60));
            h.frame();
            h.ui.keys.strip_to(None);
        }
        Cond::Favorited => {
            let id = doc_id(h);
            let lib = h.ui.library.as_mut().unwrap();
            if !lib.prefs.favorites.contains(&id) {
                lib.toggle_favorite(&id);
            }
        }
        Cond::SavedAsUser => {
            let lesson = h.ui.lessons.active.as_ref().unwrap().lesson;
            let name = match lessons::all()[lesson].id.as_str() {
                "find-and-play" => "My first variation",
                _ => "My pad",
            };
            save_as(h, name);
        }
        Cond::ExplainOpen { id, key } => {
            h.ui.perform_open = true;
            h.frame();
            h.click(&format!("pexplain:{id}.{key}"));
            assert!(
                h.ui.explain.subject
                    == Some(explain::Subject::Control {
                        id: *id,
                        key: key.clone()
                    }),
                "{c:?}: the card's ? did not open it"
            );
        }
        Cond::Pinned { id, param } => kabl_ui::perform::toggle_pin(&mut h.editor, *id, param),
        Cond::CableRemoved {
            from,
            port,
            to,
            input,
        } => {
            let cable = *h
                .editor
                .state()
                .cables
                .iter()
                .find(|(_, c)| {
                    matches!(&c.from, PortRef::Module { id, port: p } if id == from && p == port)
                        && matches!(&c.to, PortRef::Module { id, port: p } if id == to && p == input)
                })
                .expect("the lesson's cable is in its patch")
                .0;
            h.editor.disconnect(cable);
        }
        Cond::ReachesOutput { .. } => assert!(h.editor.undo(), "{c:?}: nothing to undo"),
    }
}

/// The library id of the open document.
fn doc_id(h: &H) -> String {
    match &h.ui.doc.as_ref().unwrap().origin {
        DocOrigin::Library(id) => id.clone(),
        other => panic!("not a library sound: {other:?}"),
    }
}

/// Plays `id`'s preview (what ▶ in Sounds does) with a preview output attached.
fn play_preview(h: &mut H, id: &str) {
    if h.preview.is_none() {
        let (player, voice) = sound_preview::channel(basedrop::Collector::new().handle());
        h.ui.browser.clips.attach(Some(player));
        h.preview = Some(voice);
    }
    // Previews play only with Sounds open.
    h.ui.browser_open = true;
    let e = h.ui.library.as_ref().unwrap().get(id).unwrap().clone();
    h.ui.browser.clips.ensure(id, &e.dir, &e.meta, 48000.0);
    h.ui.browser.clips.play(id);
    let start = std::time::Instant::now();
    while h.ui.browser.clips.playing() != Some(id) {
        assert!(
            start.elapsed().as_secs() < 60,
            "{id}: the preview never played"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.frame();
    }
}

/// Save As from the sound menu under `name`; a taken name is replaced.
fn save_as(h: &mut H, name: &str) {
    h.click("sound-menu");
    h.click("save-as");
    match h.ui.browser.dialog.as_mut() {
        Some(Dialog::SaveAs { name: n, .. }) => *n = name.into(),
        other => panic!("no Save As dialog: {other:?}"),
    }
    h.frame();
    h.click("dlg:save");
    if matches!(
        h.ui.browser.dialog,
        Some(Dialog::SaveAs { taken: Some(_), .. })
    ) {
        // The modal re-centres over the next frames when the banner appears.
        h.frame();
        h.frame();
        h.click("dlg:replace");
    }
    let start = std::time::Instant::now();
    while browser::io_busy(&h.ui) {
        assert!(start.elapsed().as_secs() < 30, "the save never finished");
        browser::poll_io(&mut h.editor, &mut h.ui);
    }
    h.frame();
    assert!(h.ui.browser.dialog.is_none(), "{:?}", h.ui.browser.dialog);
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

fn lesson(id: &str) -> &'static lessons::Lesson {
    lessons::all().iter().find(|l| l.id == id).unwrap()
}

fn moved(id: u64, kind: &str, param: &str, by: f32) -> Cond {
    Cond::ParamMoved {
        id,
        kind: kind.into(),
        param: param.into(),
        by,
    }
}

fn inspecting(id: u64) -> Cond {
    Cond::Inspecting {
        id,
        port: "out".into(),
    }
}

/// Lessons 1-3 against docs/pivot/firstrun-learning-design.md §4 rows 1-3, with the
/// deviations the PR states: one preview, notes only (no clock), the macros as the knob;
/// lesson 2 on Pluck to pad without a macro or Compare; lesson 3 with two taps.
#[test]
fn lessons_one_to_three_check_what_the_design_asks() {
    let macros = Cond::AnyParamMoved {
        id: 1,
        kind: "macro".into(),
        params: ["m1", "m2", "m3", "m4"].map(String::from).into(),
        by: 0.05,
    };
    let want: [(&str, Vec<Vec<Cond>>); 3] = [
        (
            "find-and-play",
            vec![
                vec![Cond::PreviewPlayed],
                vec![Cond::Opened("factory:presets/glass-elevator".into())],
                vec![Cond::NotesPlayed],
                vec![Cond::Favorited, macros],
                vec![Cond::SavedAsUser],
            ],
        ),
        (
            "change-sounds",
            vec![
                vec![Cond::ExplainOpen {
                    id: 6,
                    key: "sustain".into(),
                }],
                vec![moved(6, "env.adsr", "sustain", 0.3)],
                vec![moved(6, "env.adsr", "release_ms", 300.0)],
                vec![moved(6, "env.adsr", "attack_ms", 300.0)],
                vec![
                    Cond::Pinned {
                        id: 3,
                        param: "cutoff_hz".into(),
                    },
                    Cond::SavedAsUser,
                ],
            ],
        ),
        (
            "signal-flow",
            vec![
                vec![inspecting(2)],
                vec![inspecting(4)],
                vec![Cond::CableRemoved {
                    from: 4,
                    port: "out".into(),
                    to: 5,
                    input: "in".into(),
                }],
                vec![Cond::ReachesOutput {
                    id: 4,
                    kind: "vca".into(),
                }],
                vec![Cond::Route {
                    from: 7,
                    port: "out".into(),
                    to: 3,
                    kind: "filter.svf".into(),
                    param: "cutoff_hz".into(),
                    amount_below: None,
                }],
            ],
        ),
    ];
    for (id, steps) in want {
        let l = lesson(id);
        let got: Vec<_> = l.steps.iter().map(|s| s.done.clone()).collect();
        assert_eq!(got, steps, "{id}");
    }
    // The included steps are the recipes' own, text and all.
    assert_eq!(
        lesson("change-sounds").steps[1..4],
        lesson("pluck-to-pad").steps[1..4]
    );
    assert_eq!(
        lesson("signal-flow").steps[4],
        lesson("filter-movement").steps[1]
    );
}

/// Satisfies every step of lesson `id` in order, as the user would, and asserts it finishes.
fn walk(h: &mut H, id: &str) {
    h.start(id);
    let l = lesson(id);
    for (i, s) in l.steps.iter().enumerate() {
        assert_eq!(h.step(), i, "{id}: not on step {}", i + 1);
        // A step met before any frame watched it is only marked done; act after it is up.
        h.frame();
        for c in &s.done {
            satisfy(h, c, i);
            h.frame();
        }
        assert_eq!(h.step(), i + 1, "{id} step {}: never completed", i + 1);
    }
    let lib = h.ui.library.as_ref().unwrap();
    assert!(lib.prefs.lessons[id].done, "{id}: not recorded as finished");
}

fn user_sound<'a>(h: &'a H, name: &str) -> &'a kabl_ui::library::Entry {
    let lib = h.ui.library.as_ref().unwrap();
    lib.entries
        .iter()
        .find(|e| e.id.starts_with("user:") && e.meta.name == name)
        .unwrap_or_else(|| panic!("no user sound {name}"))
}

#[test]
fn lesson_one_ends_with_a_favorite_and_a_saved_variation() {
    let user = tempfile::tempdir().unwrap();
    let mut h = H::new(user.path());
    walk(&mut h, "find-and-play");
    let lib = h.ui.library.as_ref().unwrap();
    assert!(lib
        .prefs
        .favorites
        .contains("factory:presets/glass-elevator"));
    let saved = user_sound(&h, "My first variation").id.clone();
    let log = h.ui.library.as_ref().unwrap().read(&saved).unwrap();
    assert_eq!(log.state(), h.editor.state(), "the variation, macro turned");
}

/// A variation of the same name from an earlier run: Save As offers Replace, and the check
/// still passes on the replaced sound.
#[test]
fn lesson_one_passes_when_my_first_variation_already_exists() {
    let user = tempfile::tempdir().unwrap();
    {
        let mut h = H::new(user.path());
        walk(&mut h, "find-and-play");
    }
    let mut h = H::new(user.path());
    walk(&mut h, "find-and-play");
    let lib = h.ui.library.as_ref().unwrap();
    let mine = lib
        .entries
        .iter()
        .filter(|e| e.meta.name == "My first variation")
        .count();
    assert_eq!(mine, 1, "replaced, not duplicated");
}

#[test]
fn lesson_two_ends_with_a_saved_pad() {
    let user = tempfile::tempdir().unwrap();
    let mut h = H::new(user.path());
    walk(&mut h, "change-sounds");
    let saved = user_sound(&h, "My pad").id.clone();
    let log = h.ui.library.as_ref().unwrap().read(&saved).unwrap();
    let s = log.state();
    let p = |param: &str| s.modules[&6].params[param];
    let recipe = Library::open(
        Some(kabl_ui::library::built_factory_dir()),
        user.path().into(),
    )
    .read("factory:recipes/pluck-to-pad")
    .unwrap();
    let r = |param: &str| recipe.state().modules[&6].params[param];
    assert!(
        p("sustain") >= r("sustain") + 0.3,
        "held at the sustain level"
    );
    assert!(p("attack_ms") >= r("attack_ms") + 300.0, "a slow attack");
    assert!(p("release_ms") >= r("release_ms") + 300.0, "a long release");
    assert!(kabl_ui::perform::is_pinned(s, 3, "cutoff_hz"));
}

fn moving_filter(h: &H) -> bool {
    h.editor.state().cables.values().any(|c| {
        matches!(&c.from, PortRef::Module { id: 7, port } if port == "out")
            && matches!(&c.to, PortRef::Param { id: 3, param } if param == "cutoff_hz")
            && c.params.get("bypass").is_none_or(|b| *b < 0.5)
    })
}

#[test]
fn lesson_three_ends_with_the_moving_filter_patch() {
    let user = tempfile::tempdir().unwrap();
    let mut h = H::new(user.path());
    walk(&mut h, "signal-flow");
    assert!(moving_filter(&h), "the LFO route is on");
    let reaches = explain::path_to_output(h.editor.state(), 4);
    assert!(matches!(reaches, Ok(Some(_))), "VCA #4 is plugged back in");
}

/// Undo of the unplugged cable: done in its own step it completes "Fix it"; done before the
/// lesson asks, "Break the chain" stays done and "Fix it" is not done by a cable never pulled.
#[test]
fn lesson_three_undo_of_the_removed_cable() {
    let user = tempfile::tempdir().unwrap();
    let mut h = H::new(user.path());
    h.start("signal-flow");
    let l = lesson("signal-flow");
    let unplug = &l.steps[2].done[0];
    satisfy(&mut h, unplug, 0);
    h.frame();
    assert_eq!(h.step(), 0, "an early step does not move the lesson");
    h.editor.undo();
    h.frame();
    let a = h.ui.lessons.active.as_ref().unwrap();
    assert!(a.done[2], "unplugged once: Break the chain stays done");
    assert!(
        !a.done[3],
        "plugged back before it was asked: Fix it is not done yet"
    );

    let user = tempfile::tempdir().unwrap();
    let mut h = H::new(user.path());
    h.start("signal-flow");
    h.click("lesson-next");
    h.click("lesson-next");
    assert_eq!(h.step(), 2);
    satisfy(&mut h, unplug, 2);
    h.frame();
    assert_eq!(h.step(), 3, "unplugged: on to Fix it");
    h.frame();
    assert_eq!(h.step(), 3, "silence does not fix itself");
    h.editor.undo();
    h.frame();
    assert_eq!(h.step(), 4, "Undo plugged it back");
    h.click("lesson-prev");
    h.click("lesson-prev");
    assert_eq!(h.step(), 2);
    let a = h.ui.lessons.active.as_ref().unwrap();
    assert!(a.done[2] && a.done[3], "both stay done with the cable back");
}
