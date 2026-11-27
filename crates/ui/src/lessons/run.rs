//! The lesson runner: start, follow the open document, check a step's conditions, advance,
//! remember progress. View only: it never edits the patch.

use kabl_core::{ModuleId, PatchState, PortRef};

use super::{all, Cond, Lesson, Step, Target, START_PATCH};
use crate::browser::{self, Doc, DocOrigin, Pending};
use crate::explain;
use crate::{PatchEditor, UiState};

/// What a step's conditions compare against: the patch and undo position when it was entered.
#[derive(Debug, Clone, PartialEq)]
pub struct Baseline {
    pub state: PatchState,
    pub undo_seq: Option<u64>,
}

impl Baseline {
    pub fn of(editor: &PatchEditor) -> Baseline {
        Baseline {
            state: editor.state().clone(),
            undo_seq: editor.log().undo_entry().map(|e| e.seq),
        }
    }
}

/// The running lesson.
#[derive(Debug, Clone, PartialEq)]
pub struct Active {
    /// Index into `lessons::all()`.
    pub lesson: usize,
    pub step: usize,
    /// The editor the lesson's patch was opened into.
    pub editor: u64,
    /// Another sound was opened since: the steps no longer apply.
    pub detached: bool,
    pub base: Baseline,
    /// The patch when the lesson started: a later step met already (the knob turned before its
    /// step asks) counts as done when it is reached.
    pub origin: Baseline,
    /// When the last step completed by its conditions (the panel says so briefly).
    pub completed_at: Option<f64>,
    /// The step's conditions have not all held at some frame since it was entered. Only then
    /// does meeting them advance by itself; a step that already holds is just marked done.
    pub armed: bool,
    /// Steps completed this session, one per step. A done step asks for nothing again, so
    /// going back to it with Previous does not mean undoing and redoing the edit.
    pub done: Vec<bool>,
    /// Steps whose conditions already held when the lesson started (or followed into its
    /// sound). Those are not marked done before they are reached: only conditions that come to
    /// hold count early, so "plug the cable back" is not done by a cable never unplugged.
    pub met_at_start: Vec<bool>,
}

/// Learn tab state. View only.
#[derive(Debug, Default)]
pub struct Learn {
    pub open: bool,
    pub active: Option<Active>,
    /// A start waiting for its open (lesson, editor instance at the request).
    pending: Option<(usize, u64)>,
    pub note: Option<String>,
    /// Since when the Start card has seen no running audio output.
    pub quiet_since: Option<f64>,
    /// Set by `use_user_prefs`; the first frame with a library clears it and, in a plugin
    /// (`UiState::devices` is `None`), starts the first run (`first_run::plugin`).
    pub(crate) plugin_check: bool,
}

/// Is `t` in `state` with the kind the lesson expects? The resolved Show target when it is.
pub fn resolve(state: &PatchState, t: &Target) -> Option<explain::Target> {
    match t {
        Target::Control { id, kind, param } => (kind_ok(state, *id, kind)
            && explain::param_of(state, *id, param).is_some())
        .then(|| explain::Target::Control {
            id: *id,
            param: param.clone(),
            cable: None,
        }),
        Target::Route {
            from,
            port,
            to,
            kind,
            param,
        } => {
            if !kind_ok(state, *to, kind) {
                return None;
            }
            let (cable, _) = route(state, *from, port, *to, param)?;
            Some(explain::Target::Control {
                id: *to,
                param: param.clone(),
                cable: Some(cable),
            })
        }
        Target::Module { id, kind } => {
            kind_ok(state, *id, kind).then_some(explain::Target::Module(*id))
        }
    }
}

fn kind_ok(state: &PatchState, id: ModuleId, kind: &str) -> bool {
    state.modules.get(&id).is_some_and(|m| m.kind == kind)
}

/// The modulation cable from `from`.`port` into `to`.`param`, and whether it is bypassed.
fn route(
    state: &PatchState,
    from: ModuleId,
    port: &str,
    to: ModuleId,
    param: &str,
) -> Option<(kabl_core::CableId, bool)> {
    state.cables.iter().find_map(|(&cid, c)| {
        let from_ok =
            matches!(&c.from, PortRef::Module { id, port: p } if *id == from && p == port);
        let to_ok = matches!(&c.to, PortRef::Param { id, param: p } if *id == to && p == param);
        (from_ok && to_ok).then(|| (cid, c.params.get("bypass").is_some_and(|b| *b >= 0.5)))
    })
}

/// The stored value of a parameter, or its default when never set.
fn param_value(state: &PatchState, id: ModuleId, param: &str) -> Option<f32> {
    let info = explain::param_of(state, id, param)?;
    Some(crate::routing::base_value(state, id, info))
}

/// Does `c` hold now?
pub fn holds(c: &Cond, editor: &PatchEditor, ui: &UiState, base: &Baseline) -> bool {
    let state = editor.state();
    match c {
        Cond::Opened(sound) => {
            matches!(&ui.doc, Some(d) if matches!(&d.origin, DocOrigin::Library(s) if s == sound))
        }
        Cond::ParamMoved {
            id,
            kind,
            param,
            by,
        } => {
            kind_ok(state, *id, kind)
                && match (
                    param_value(state, *id, param),
                    param_value(&base.state, *id, param),
                ) {
                    (Some(now), Some(then)) => (now - then).abs() >= *by,
                    _ => false,
                }
        }
        Cond::Route {
            from,
            port,
            to,
            kind,
            param,
            amount_below,
        } => {
            kind_ok(state, *to, kind)
                && route(state, *from, port, *to, param).is_some_and(|(cable, bypassed)| {
                    !bypassed
                        && amount_below.is_none_or(|below| {
                            state.cables[&cable]
                                .params
                                .get("amount")
                                .is_some_and(|a| *a < below)
                        })
                })
        }
        Cond::AnyParamMoved {
            id,
            kind,
            params,
            by,
        } => {
            kind_ok(state, *id, kind)
                && params.iter().any(|param| {
                    match (
                        param_value(state, *id, param),
                        param_value(&base.state, *id, param),
                    ) {
                        (Some(now), Some(then)) => (now - then).abs() >= *by,
                        _ => false,
                    }
                })
        }
        Cond::SoundsOpen => ui.browser_open,
        Cond::Inspecting { id, port } => ui
            .inspect
            .sel
            .as_ref()
            .is_some_and(|s| s.id == *id && s.port == *port),
        Cond::CompareRestored => {
            ui.compare.can_undo_restore(editor)
                && editor.log().undo_entry().map(|e| e.seq) != base.undo_seq
        }
        Cond::PreviewPlayed => ui.browser.clips.playing().is_some(),
        Cond::NotesPlayed => ui.keys.any() || ui.status_voices.is_some_and(|(n, _)| n > 0),
        Cond::Favorited => library_doc(ui).is_some_and(|id| {
            ui.library
                .as_ref()
                .is_some_and(|l| l.prefs.favorites.contains(id))
        }),
        Cond::SavedAsUser => {
            library_doc(ui).is_some_and(|id| id.starts_with("user:"))
                && !browser::is_modified(editor, ui)
        }
        Cond::ExplainOpen { id, key } => matches!(
            &ui.explain.subject,
            Some(explain::Subject::Control { id: i, key: k }) if i == id && k == key
        ),
        Cond::Pinned { id, param } => crate::perform::is_pinned(state, *id, param),
        Cond::ReachesOutput { id, kind } => {
            kind_ok(state, *id, kind) && matches!(explain::path_to_output(state, *id), Ok(Some(_)))
        }
        Cond::CableRemoved {
            from,
            port,
            to,
            input,
        } => state.modules.contains_key(from)
            && state.modules.contains_key(to)
            && !state.cables.values().any(|c| {
                matches!(&c.from, PortRef::Module { id, port: p } if id == from && p == port)
                    && matches!(&c.to, PortRef::Module { id, port: p } if id == to && p == input)
            }),
    }
}

/// The library id of the open document, if it is a library sound.
fn library_doc(ui: &UiState) -> Option<&str> {
    match &ui.doc.as_ref()?.origin {
        DocOrigin::Library(id) => Some(id),
        _ => None,
    }
}

/// Starts lesson `i`: asks the ordinary unsaved question before opening its patch. The start
/// patch is not a library sound, so a lesson on it opens only over a sound with nothing unsaved.
pub fn start(editor: &mut PatchEditor, ui: &mut UiState, i: usize) {
    ui.lessons.note = None;
    if all()[i].start.sound == START_PATCH {
        if browser::io_busy(ui) || browser::is_modified(editor, ui) {
            ui.lessons.note = Some(
                "Save your sound first: this lesson opens the start patch in its place.".into(),
            );
            return;
        }
        browser::replace_patch(editor, ui, crate::first_run::start_patch().log().clone());
        ui.doc = Some(Doc::new(
            crate::first_run::start_name(),
            DocOrigin::New,
            editor.state(),
        ));
        activate(editor, ui, i);
        return;
    }
    ui.lessons.pending = Some((i, editor.instance()));
    log::debug!(target: "lesson", "start requested lesson={}", all()[i].id);
    browser::request(editor, ui, Pending::Open(all()[i].start.sound.clone()));
}

/// Once per frame: a start whose open completed becomes active (and captures the reference);
/// one that was cancelled or failed is dropped; another open detaches an active lesson; a step
/// whose conditions all hold advances.
pub fn frame(editor: &PatchEditor, ui: &mut UiState, now: f64) {
    if ui.lessons.plugin_check && ui.library.is_some() {
        ui.lessons.plugin_check = false;
        if ui.devices.is_none() {
            crate::first_run::plugin(editor, ui);
        }
    }
    if let Some((i, before)) = ui.lessons.pending {
        let l = &all()[i];
        let opened = editor.instance() != before
            && matches!(&ui.doc, Some(d) if d.origin == DocOrigin::Library(l.start.sound.clone()));
        if opened {
            ui.lessons.pending = None;
            activate(editor, ui, i);
            ui.lessons.open = true;
        } else if ui.browser.dialog.is_none() {
            ui.lessons.pending = None;
            ui.lessons.note = Some("The lesson did not start; your sound is unchanged.".into());
            log::debug!(target: "lesson", "start cancelled lesson={}", l.id);
        }
    }
    let Some(a) = ui.lessons.active.as_ref() else {
        return;
    };
    if !a.detached && a.editor != editor.instance() {
        let l = &all()[a.lesson];
        // A sound the lesson itself asks for (an `opened` condition) carries it on.
        let asked = library_doc(ui).is_some_and(|id| {
            l.steps
                .iter()
                .flat_map(|s| &s.done)
                .any(|c| matches!(c, Cond::Opened(s) if s == id))
        });
        let met = asked.then(|| met_now(l, editor, ui, &Baseline::of(editor)));
        if let Some(a) = ui.lessons.active.as_mut() {
            match met {
                Some(met) => {
                    a.editor = editor.instance();
                    a.base = Baseline::of(editor);
                    a.origin = a.base.clone();
                    a.met_at_start = met;
                    log::debug!(target: "lesson", "followed lesson={} into its sound", l.id);
                }
                None => {
                    a.detached = true;
                    log::debug!(target: "lesson", "detached lesson={}", l.id);
                }
            }
        }
    }
    if ui.lessons.active.as_ref().is_none_or(|a| a.detached) {
        return;
    }
    // Borrowed, not cloned: this runs every frame and `Active` holds a whole patch.
    let Some(a) = ui.lessons.active.as_ref() else {
        return;
    };
    let step = a.step;
    let l = &all()[a.lesson];
    let early: Vec<usize> = (step + 1..l.steps.len())
        .filter(|&k| !a.done[k] && !a.met_at_start[k] && meets(&l.steps[k], editor, ui, &a.origin))
        .collect();
    if !early.is_empty() {
        if let Some(a) = ui.lessons.active.as_mut() {
            for k in early {
                a.done[k] = true;
            }
        }
    }
    let Some(a) = ui.lessons.active.as_ref() else {
        return;
    };
    // Already done, or the end page: nothing to watch.
    if a.done.get(step).copied().unwrap_or(true) {
        return;
    }
    let (ok, armed) = (complete(&all()[a.lesson], a, editor, ui), a.armed);
    let Some(a) = ui.lessons.active.as_mut() else {
        return;
    };
    if !ok {
        a.armed = true;
        return;
    }
    a.done[step] = true;
    // Met by what the user did here: move on. Met on arrival: show it done and let Next move on.
    if armed {
        step_to(editor, ui, step + 1);
        if let Some(a) = ui.lessons.active.as_mut() {
            a.completed_at = Some(now);
        }
    }
}

/// The current step has conditions and all of them hold.
pub(super) fn complete(l: &Lesson, a: &Active, editor: &PatchEditor, ui: &UiState) -> bool {
    l.steps
        .get(a.step)
        .is_some_and(|s| meets(s, editor, ui, &a.base))
}

/// For each step of `l`: its conditions all hold now.
fn met_now(l: &Lesson, editor: &PatchEditor, ui: &UiState, base: &Baseline) -> Vec<bool> {
    l.steps.iter().map(|s| meets(s, editor, ui, base)).collect()
}

/// `s` has conditions and all of them hold against `base`.
fn meets(s: &Step, editor: &PatchEditor, ui: &UiState, base: &Baseline) -> bool {
    !s.done.is_empty() && s.done.iter().all(|c| holds(c, editor, ui, base))
}

/// Makes lesson `i` the running one on the patch now open, resuming where the user left off (a
/// finished lesson starts over).
fn activate(editor: &PatchEditor, ui: &mut UiState, i: usize) {
    let l = &all()[i];
    let step = ui
        .library
        .as_ref()
        .and_then(|lib| lib.prefs.lessons.get(&l.id))
        .map_or(0, |p| p.step);
    let step = if step < l.steps.len() { step } else { 0 };
    let base = Baseline::of(editor);
    let met_at_start = met_now(l, editor, ui, &base);
    ui.lessons.active = Some(Active {
        lesson: i,
        step,
        editor: editor.instance(),
        detached: false,
        origin: base.clone(),
        base,
        completed_at: None,
        armed: false,
        done: vec![false; l.steps.len()],
        met_at_start,
    });
    ui.compare.capture(editor, "the lesson start");
    log::debug!(target: "lesson", "active lesson={} step={step}", l.id);
}

/// The first run: lesson 00 (the Start card) over the start patch already open, once per
/// profile. `Prefs::first_run_done` (also what the tour before it set) is saved as it starts,
/// so a profile that has it, or an app closed mid-lesson, does not start it again; Learn
/// always has it. True when it started.
pub fn first_run(editor: &PatchEditor, ui: &mut UiState) -> bool {
    let Some(i) = all().iter().position(|l| l.card) else {
        return false;
    };
    let Some(lib) = ui.library.as_mut().filter(|l| !l.prefs.first_run_done) else {
        return false;
    };
    lib.prefs.first_run_done = true;
    if let Some(e) = lib.save_prefs() {
        log::warn!(target: "lesson", "{e}");
    }
    activate(editor, ui, i);
    true
}

/// Moves the active lesson to `step` (the step count is the unguided page) and records it.
pub(super) fn step_to(editor: &PatchEditor, ui: &mut UiState, step: usize) {
    let Some(a) = ui.lessons.active.as_mut() else {
        return;
    };
    a.step = step;
    a.base = Baseline::of(editor);
    a.completed_at = None;
    a.armed = false;
    let l: &Lesson = &all()[a.lesson];
    log::debug!(target: "lesson", "step lesson={} step={step}", l.id);
    if let Some(lib) = ui.library.as_mut() {
        let p = lib.prefs.lessons.entry(l.id.clone()).or_default();
        let next = super::Progress {
            step,
            done: p.done || step >= l.steps.len(),
        };
        if *p != next {
            *p = next;
            if let Some(e) = lib.save_prefs() {
                log::warn!(target: "lesson", "{e}");
            }
        }
    }
}
