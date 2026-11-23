//! Lessons: optional, revisitable walks through real patches. A lesson is data
//! (`lessons/*.toml` at the repository root, embedded by `build.rs`; format in
//! docs/learning/LESSON-FORMAT.md): a start sound, steps with explicit targets, an anchor to ring
//! and completion conditions from a closed vocabulary that observe the real patch and view.
//!
//! One runner (`run`) for every lesson. It never edits the patch: it opens the start sound
//! through the ordinary unsaved-changes protection and sends audio only from a button the user
//! presses. Nothing is locked. Progress is kept in `library.json` (`library::Prefs::lessons`),
//! shared by the standalone app and the plugin.

mod panel;
pub mod patches;
mod run;

use std::collections::BTreeSet;
use std::sync::OnceLock;

use kabl_core::ModuleId;
use serde::{Deserialize, Serialize};

pub use panel::{mark, panel};
pub use run::{frame, holds, resolve, start, Active, Baseline, Learn};

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lesson {
    /// Stable progress key in `library.json`; never renamed.
    pub id: String,
    pub title: String,
    /// One line under the title on the Learn card.
    pub summary: String,
    pub start: Start,
    #[serde(rename = "step")]
    pub steps: Vec<Step>,
    pub end: End,
}

/// The patch a lesson opens when started.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    /// A library id (`factory:…`).
    pub sound: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct End {
    /// The last page: a task without guidance.
    pub unguided: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub text: String,
    /// "Show" brings this into view (`explain::go`).
    #[serde(default)]
    pub show: Option<Target>,
    /// "Inspect" selects this output in the signal inspector.
    #[serde(default)]
    pub inspect: Option<Port>,
    /// "Play test note" of this many seconds: middle C, the same every time.
    #[serde(default)]
    pub play_note: Option<f32>,
    /// "Open Compare".
    #[serde(default)]
    pub compare: bool,
    /// A `UiState::hits` key to ring while the step shows; a trailing `*` matches any rest.
    #[serde(default)]
    pub anchor: Option<String>,
    /// The step advances by itself once all of these hold. Empty: Next only.
    #[serde(default)]
    pub done: Vec<Cond>,
}

/// What a step points at, by explicit identity (id and kind; a route by its endpoints). Never
/// by name: a missing or changed target is reported, not guessed.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    Control {
        id: ModuleId,
        kind: String,
        param: String,
    },
    /// The modulation route from `from`'s `port` into `to`'s `param`.
    Route {
        from: ModuleId,
        port: String,
        to: ModuleId,
        kind: String,
        param: String,
    },
    Module {
        id: ModuleId,
        kind: String,
    },
}

/// One module output.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Port {
    pub id: ModuleId,
    pub kind: String,
    pub port: String,
}

/// A completion condition. Each observes real state; "since the step started" means against
/// the patch and undo position captured when the step was entered.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Cond {
    /// The open document is this library sound.
    Opened(String),
    /// The parameter moved by at least `by` (in its own units) since the step started.
    ParamMoved {
        id: ModuleId,
        kind: String,
        param: String,
        by: f32,
    },
    /// A modulation route from `from`'s `port` into `to`'s `param` exists and is not bypassed.
    Route {
        from: ModuleId,
        port: String,
        to: ModuleId,
        kind: String,
        param: String,
    },
    /// The signal inspector has this output selected.
    Inspecting { id: ModuleId, port: String },
    /// The Compare reference was restored since the step started (and not undone).
    CompareRestored,
}

/// Progress of one lesson, by `Lesson::id`, in `library.json`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Progress {
    /// The furthest step reached (the step count once on the unguided page).
    pub step: usize,
    /// Reached the end at least once.
    pub done: bool,
}

/// Parses one lesson file. `name` is only for the message.
pub fn parse(name: &str, text: &str) -> Result<Lesson, String> {
    let l: Lesson = toml::from_str(text).map_err(|e| format!("lessons/{name}: {e}"))?;
    let bad = |what: &str| Err(format!("lessons/{name}: {what}"));
    if l.id.trim().is_empty() || l.title.trim().is_empty() {
        return bad("`id` and `title` must not be empty");
    }
    if l.steps.is_empty() {
        return bad("needs at least one [[step]]");
    }
    for (i, s) in l.steps.iter().enumerate() {
        let n = i + 1;
        if s.text.trim().is_empty() {
            return bad(&format!("step {n}: `text` is empty"));
        }
        if s.play_note
            .is_some_and(|t| !(t.is_finite() && t > 0.0 && t <= 30.0))
        {
            return bad(&format!("step {n}: `play_note` must be 0 < seconds <= 30"));
        }
        if s.anchor.as_deref().is_some_and(|a| a.trim().is_empty()) {
            return bad(&format!("step {n}: `anchor` is empty"));
        }
        for c in &s.done {
            if let Cond::ParamMoved { by, .. } = c {
                if !(by.is_finite() && *by > 0.0) {
                    return bad(&format!("step {n}: `param_moved.by` must be above 0"));
                }
            }
        }
    }
    Ok(l)
}

/// The embedded lesson files: (file name, text), in file-name order.
const FILES: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/lessons.rs"));

/// Every lesson that parses, in file-name order. A bad file (caught by the tests before it can
/// ship) is left out with an error in the log; so is a second lesson with an id already used.
pub fn all() -> &'static [Lesson] {
    static ALL: OnceLock<Vec<Lesson>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut ids = BTreeSet::new();
        FILES
            .iter()
            .filter_map(|(name, text)| match parse(name, text) {
                Ok(l) if ids.insert(l.id.clone()) => Some(l),
                Ok(l) => {
                    log::error!(target: "lesson", "lessons/{name}: id {:?} used twice", l.id);
                    None
                }
                Err(e) => {
                    log::error!(target: "lesson", "{e}");
                    None
                }
            })
            .collect()
    })
}

#[cfg(test)]
mod tests;
