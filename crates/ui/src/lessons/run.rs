//! The lesson runner: start, follow the open document, check a step's conditions, advance,
//! remember progress. View only: it never edits the patch.

use kabl_core::{ModuleId, PatchState, PortRef};

use super::{all, Cond, Lesson, Target};
use crate::browser::{self, DocOrigin, Pending};
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
    /// When the last step completed by its conditions (the panel says so briefly).
    pub completed_at: Option<f64>,
    /// The step's conditions have not all held at some frame since it was entered. Only then
    /// does meeting them advance by itself; a step that already holds is just marked done.
    pub armed: bool,
    /// Steps completed this session, one per step. A done step asks for nothing again, so
    /// going back to it with Previous does not mean undoing and redoing the edit.
    pub done: Vec<bool>,
}

/// Learn tab state. View only.
#[derive(Debug, Default)]
pub struct Learn {
    pub open: bool,
    pub active: Option<Active>,
    /// A start waiting for its open (lesson, editor instance at the request).
    pending: Option<(usize, u64)>,
    pub note: Option<String>,
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
        Cond::Inspecting { id, port } => ui
            .inspect
            .sel
            .as_ref()
            .is_some_and(|s| s.id == *id && s.port == *port),
        Cond::CompareRestored => {
            ui.compare.can_undo_restore(editor)
                && editor.log().undo_entry().map(|e| e.seq) != base.undo_seq
        }
    }
}

/// Starts lesson `i`: asks the ordinary unsaved question before opening its patch.
pub fn start(editor: &mut PatchEditor, ui: &mut UiState, i: usize) {
    ui.lessons.pending = Some((i, editor.instance()));
    ui.lessons.note = None;
    log::debug!(target: "lesson", "start requested lesson={}", all()[i].id);
    browser::request(editor, ui, Pending::Open(all()[i].start.sound.clone()));
}

/// Once per frame: a start whose open completed becomes active (and captures the reference);
/// one that was cancelled or failed is dropped; another open detaches an active lesson; a step
/// whose conditions all hold advances.
pub fn frame(editor: &PatchEditor, ui: &mut UiState, now: f64) {
    if let Some((i, before)) = ui.lessons.pending {
        let l = &all()[i];
        let opened = editor.instance() != before
            && matches!(&ui.doc, Some(d) if d.origin == DocOrigin::Library(l.start.sound.clone()));
        if opened {
            // Resume where the user left off; a finished lesson starts over.
            let step = ui
                .library
                .as_ref()
                .and_then(|lib| lib.prefs.lessons.get(&l.id))
                .map_or(0, |p| p.step);
            let step = if step < l.steps.len() { step } else { 0 };
            ui.lessons.pending = None;
            ui.lessons.active = Some(Active {
                lesson: i,
                step,
                editor: editor.instance(),
                detached: false,
                base: Baseline::of(editor),
                completed_at: None,
                armed: false,
                done: vec![false; l.steps.len()],
            });
            ui.lessons.open = true;
            ui.compare.capture(editor, "the lesson start");
            log::debug!(target: "lesson", "active lesson={} step={step}", l.id);
        } else if ui.browser.dialog.is_none() {
            ui.lessons.pending = None;
            ui.lessons.note = Some("The lesson did not start; your sound is unchanged.".into());
            log::debug!(target: "lesson", "start cancelled lesson={}", l.id);
        }
    }
    let Some(a) = ui.lessons.active.as_mut() else {
        return;
    };
    if !a.detached && a.editor != editor.instance() {
        a.detached = true;
        log::debug!(target: "lesson", "detached lesson={}", all()[a.lesson].id);
    }
    if a.detached {
        return;
    }
    // Borrowed, not cloned: this runs every frame and `Active` holds a whole patch.
    let Some(a) = ui.lessons.active.as_ref() else {
        return;
    };
    let step = a.step;
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
        .is_some_and(|s| !s.done.is_empty() && s.done.iter().all(|c| holds(c, editor, ui, &a.base)))
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
