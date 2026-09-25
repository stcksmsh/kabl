//! D03 comparison reference (docs/signal-inspection/design.md §3): one in-memory copy of the
//! whole patch configuration, and an explicit Restore that is one undo step. Undo after a
//! restore brings the working version back with every edit; redo restores again: that pair is
//! the A/B. What is heard, edited and saved is always the one current patch.
//!
//! The reference is session-local (never saved) and belongs to the editor it was captured in:
//! opening another sound drops it.

use kabl_core::{Op, ParamTarget, PatchState};

use crate::{browser, PatchEditor, UiState};

#[derive(Debug, Clone)]
pub struct Reference {
    pub state: PatchState,
    /// `PatchEditor::instance` it belongs to.
    pub editor: u64,
    /// Wall-clock label (`14:03:21`, UTC).
    pub at: String,
    /// What captured it ("you", or a recipe).
    pub by: String,
}

/// Comparison state. View only.
#[derive(Debug, Default)]
pub struct Compare {
    pub open: bool,
    /// Scroll the section into view on the next frame (just opened).
    pub reveal: bool,
    pub reference: Option<Reference>,
    /// Sequence number of the log entry our last restore appended.
    restored: Option<u64>,
    /// Said once in the panel (the reference was dropped, a restore was refused).
    pub note: Option<String>,
}

impl Compare {
    /// Copies the current patch as the reference. Touches nothing else.
    pub fn capture(&mut self, editor: &PatchEditor, by: &str) {
        let at = kabl_standalone::applog::utc_now();
        self.reference = Some(Reference {
            state: editor.state().clone(),
            editor: editor.instance(),
            at: at.get(11..19).unwrap_or(&at).to_string() + " UTC",
            by: by.to_string(),
        });
        self.restored = None;
        self.note = None;
        log::debug!(target: "compare", "capture by={by} editor={}", editor.instance());
    }

    /// Drops a reference that belongs to another editor (another sound was opened).
    pub fn validate(&mut self, editor: &PatchEditor) {
        if self
            .reference
            .as_ref()
            .is_some_and(|r| r.editor != editor.instance())
        {
            self.reference = None;
            self.restored = None;
            self.note = Some("Another sound was opened: the reference was dropped.".into());
            log::debug!(target: "compare", "reference dropped: editor replaced");
        }
    }

    /// The current patch equals the reference.
    pub fn same(&self, editor: &PatchEditor) -> bool {
        self.reference
            .as_ref()
            .is_some_and(|r| &r.state == editor.state())
    }

    /// Our restore is the entry Undo would revert now.
    pub fn can_undo_restore(&self, editor: &PatchEditor) -> bool {
        self.restored.is_some() && editor.log().undo_entry().map(|e| e.seq) == self.restored
    }

    /// Our restore is the entry Redo would re-apply now.
    pub fn can_redo_restore(&self, editor: &PatchEditor) -> bool {
        self.restored.is_some() && editor.log().redo_entry().map(|e| e.seq) == self.restored
    }
}

/// Ops that turn `from` into `to` exactly (modules, params, labels, positions, cables with
/// their settings and step patterns). `None` if the result would differ (never expected;
/// checked so a restore can refuse rather than land somewhere else).
pub fn diff(from: &PatchState, to: &PatchState) -> Option<Vec<Op>> {
    let mut ops = Vec::new();
    let mut s = from.clone();
    let mut push = |s: &mut PatchState, op: Op| {
        s.apply(&op);
        ops.push(op);
    };
    for (&id, c) in &from.cables {
        match to.cables.get(&id) {
            Some(t) if t.from == c.from && t.to == c.to => {}
            _ => push(&mut s, Op::Disconnect { id }),
        }
    }
    for (&id, m) in &from.modules {
        if to.modules.get(&id).is_none_or(|t| t.kind != m.kind) {
            push(&mut s, Op::RemoveModule { id });
        }
    }
    for (&id, m) in &to.modules {
        if !s.modules.contains_key(&id) {
            push(
                &mut s,
                Op::AddModule {
                    id,
                    kind: m.kind.clone(),
                    pos: m.pos,
                },
            );
        }
        let cur = s.modules[&id].clone();
        if cur.pos != m.pos {
            push(&mut s, Op::MoveModule { id, pos: m.pos });
        }
        for (name, &v) in &m.params {
            if cur.params.get(name) != Some(&v) {
                push(
                    &mut s,
                    Op::SetParam {
                        target: ParamTarget::Module {
                            id,
                            param: name.clone(),
                        },
                        value: v,
                    },
                );
            }
        }
        for name in cur.params.keys().filter(|k| !m.params.contains_key(*k)) {
            push(
                &mut s,
                Op::UnsetParam {
                    target: ParamTarget::Module {
                        id,
                        param: name.clone(),
                    },
                },
            );
        }
        let want = to.labels.get(&id).cloned().unwrap_or_default();
        let have = s.labels.get(&id).cloned().unwrap_or_default();
        for (k, t) in &want {
            if have.get(k) != Some(t) {
                push(
                    &mut s,
                    Op::SetLabel {
                        id,
                        key: k.clone(),
                        text: Some(t.clone()),
                    },
                );
            }
        }
        for k in have.keys().filter(|k| !want.contains_key(*k)) {
            push(
                &mut s,
                Op::SetLabel {
                    id,
                    key: k.clone(),
                    text: None,
                },
            );
        }
    }
    for (&id, c) in &to.cables {
        if s.cables
            .get(&id)
            .is_none_or(|h| h.from != c.from || h.to != c.to)
        {
            push(
                &mut s,
                Op::Connect {
                    id,
                    from: c.from.clone(),
                    to: c.to.clone(),
                },
            );
        }
        let have = s.cables[&id].clone();
        for (name, &v) in &c.params {
            if have.params.get(name) != Some(&v) {
                push(
                    &mut s,
                    Op::SetParam {
                        target: ParamTarget::Cable {
                            id,
                            param: name.clone(),
                        },
                        value: v,
                    },
                );
            }
        }
        for name in have.params.keys().filter(|k| !c.params.contains_key(*k)) {
            push(
                &mut s,
                Op::UnsetParam {
                    target: ParamTarget::Cable {
                        id,
                        param: name.clone(),
                    },
                },
            );
        }
        if have.steps != c.steps {
            push(
                &mut s,
                Op::SetCablePattern {
                    id,
                    steps: c.steps.clone(),
                },
            );
        }
    }
    (&s == to).then_some(ops)
}

/// Replaces the current patch with the reference as one undo step. Returns a message.
pub fn restore(editor: &mut PatchEditor, ui: &mut UiState) -> String {
    let Some(r) = ui.compare.reference.clone() else {
        return "no reference to restore".into();
    };
    if r.editor != editor.instance() {
        return "the reference belongs to another sound".into();
    }
    match diff(editor.state(), &r.state) {
        None => {
            log::warn!(target: "compare", "restore refused: difference did not reproduce the reference");
            "couldn't restore the reference exactly; nothing changed".into()
        }
        Some(ops) if ops.is_empty() => "already the same as the reference".into(),
        Some(ops) => {
            editor.restore_to(ops);
            ui.compare.restored = editor.log().undo_entry().map(|e| e.seq);
            log::debug!(target: "compare", "restore applied");
            "reference restored as one step: Undo brings your version back".into()
        }
    }
}

fn para(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.add(egui::Label::new(text.into()).wrap());
}

fn weak(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.add(egui::Label::new(egui::RichText::new(text.into()).small().weak()).wrap());
}

/// The drawer section.
pub fn panel(editor: &mut PatchEditor, ui_state: &mut UiState, ui: &mut egui::Ui) {
    if std::mem::take(&mut ui_state.compare.reveal) {
        ui.scroll_to_cursor(Some(egui::Align::TOP));
    }
    ui.horizontal(|ui| {
        ui.strong("Compare with a reference");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let r = ui.small_button("Close");
            ui_state.record("compare-close".into(), r.rect);
            if r.clicked() {
                ui_state.compare.open = false;
            }
        });
    });
    weak(
        ui,
        "The reference is a copy of the whole patch (values, cables, routes, labels, Perform \
         pins and MIDI mappings), kept only in this session and not saved with the sound.",
    );
    if let Some(n) = ui_state.compare.note.clone() {
        para(ui, n);
    }
    let has = ui_state.compare.reference.is_some();
    let same = ui_state.compare.same(editor);
    match &ui_state.compare.reference {
        Some(r) => para(
            ui,
            format!(
                "Reference: captured {} by {}. You hear and edit the current patch{}.",
                r.at,
                r.by,
                if same {
                    ", which is the same as the reference now"
                } else {
                    ", which differs from it"
                }
            ),
        ),
        None => para(ui, "No reference yet. You hear and edit the current patch."),
    }
    ui.horizontal_wrapped(|ui| {
        let r = ui
            .button(if has {
                "Replace reference"
            } else {
                "Capture reference"
            })
            .on_hover_text("Copy the current patch as the reference (changes nothing)");
        ui_state.record("compare-capture".into(), r.rect);
        if r.clicked() {
            ui_state.compare.capture(editor, "you");
            ui_state.last_message = Some("reference captured (not saved with the sound)".into());
        }
        let r = ui
            .add_enabled(has && !same, egui::Button::new("Restore reference…"))
            .on_hover_text("Replace the whole current patch with the reference, as one undo step");
        ui_state.record("compare-restore".into(), r.rect);
        if r.clicked() {
            ui_state.browser.dialog = Some(browser::Dialog::Restore);
        }
        if ui_state.compare.can_undo_restore(editor) {
            let r = ui
                .button("Back to my version (Undo)")
                .on_hover_text("Undo the restore: your version with all its edits");
            ui_state.record("compare-undo".into(), r.rect);
            if r.clicked() {
                editor.undo();
            }
        } else if ui_state.compare.can_redo_restore(editor) {
            let r = ui
                .button("Hear the reference again (Redo)")
                .on_hover_text("Redo the restore");
            ui_state.record("compare-redo".into(), r.rect);
            if r.clicked() {
                editor.redo();
            }
        }
    });
    weak(
        ui,
        "Switching rebuilds the sound like any edit (a short crossfade): held notes, tails and \
         running phases continue, so it is not a sample-exact A/B. Levels are not adjusted: use \
         the output meter to compare loudness.",
    );
}
