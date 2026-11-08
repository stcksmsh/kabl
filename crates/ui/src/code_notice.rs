//! The one-time "This patch contains custom code modules" notice (D11 decision 2).
//!
//! A patch with embedded code asks once per content hash: Open runs it, Cancel does not. The
//! answer is remembered in the library preferences (`Prefs::code_trust`, keyed by the lower-case
//! hex SHA-256), so the same code never asks again, wherever it turns up.

use std::collections::BTreeMap;

use kabl_core::embedded::{hash_hex, Embedded};

use crate::kit::{self, g4, Tone};
use crate::style::{Role, Style};

/// One embedded module as the notice lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeEntry {
    pub module_id: String,
    pub version: u32,
    /// Lower-case hex SHA-256 of the code.
    pub hash: String,
}

impl CodeEntry {
    /// The first 12 hex digits, enough to tell two versions apart on screen.
    pub fn short(&self) -> &str {
        &self.hash[..12.min(self.hash.len())]
    }
}

pub fn entries(code: &Embedded) -> Vec<CodeEntry> {
    code.blobs()
        .map(|(h, b)| CodeEntry {
            module_id: b.module_id.clone(),
            version: b.version,
            hash: hash_hex(h),
        })
        .collect()
}

/// What opening a patch with this code should do, given the remembered choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// No code, or every hash was accepted before.
    Open,
    /// At least one hash has no remembered choice and none was refused: show the notice.
    Ask,
    /// A hash was cancelled before: open the patch without running its code.
    Refuse,
}

pub fn gate(code: &[CodeEntry], trust: &BTreeMap<String, bool>) -> Gate {
    if code.iter().any(|c| trust.get(&c.hash) == Some(&false)) {
        Gate::Refuse
    } else if code.iter().all(|c| trust.get(&c.hash) == Some(&true)) {
        Gate::Open
    } else {
        Gate::Ask
    }
}

/// Draws the notice. `Some(true)` = Open, `Some(false)` = Cancel (also Esc, close, backdrop).
pub fn show(ctx: &egui::Context, st: &Style, code: &[CodeEntry]) -> Option<bool> {
    let mut choice = None;
    let closed = g4::modal_framed(
        ctx,
        st,
        "dlg-code-notice",
        g4::Size::M,
        "This patch contains custom code modules",
        |ui| {
            kit::label(
                ui,
                st,
                Role::Body,
                Tone::Text2,
                "Custom modules are small programs saved inside the patch. They run in a \
                 sandbox with no access to your files or network. Open the patch only if you \
                 trust where it came from. Your choice is remembered for this exact code.",
            );
            kit::gap(ui, st, 2);
            for c in code {
                kit::label(
                    ui,
                    st,
                    Role::Body,
                    Tone::Text,
                    format!("{}  v{}", c.module_id, c.version),
                );
                kit::label(
                    ui,
                    st,
                    Role::Caption,
                    Tone::Text2,
                    format!("sha256 {}…", c.short()),
                );
            }
        },
        |_| {},
        |ui| {
            if ui
                .add(kit::Button::new(st, "Open").g4(32.0).primary())
                .clicked()
            {
                choice = Some(true);
            }
            if ui.add(kit::Button::new(st, "Cancel").g4(32.0)).clicked() {
                choice = Some(false);
            }
        },
    );
    choice.or(closed.then_some(false))
}

/// Records `open` for every hash in `code`.
pub fn remember(trust: &mut BTreeMap<String, bool>, code: &[CodeEntry], open: bool) {
    for c in code {
        trust.insert(c.hash.clone(), open);
    }
}

/// The code a patch being saved carries: one blob per code-module kind it uses, bound to every
/// instance of that kind. Built-in kinds add nothing.
pub fn embedded_for(state: &kabl_core::PatchState) -> Embedded {
    let mut code = Embedded::new();
    for (&id, m) in &state.modules {
        let Some((version, bytes)) = kabl_modules::code::host::blob_for(&m.kind) else {
            continue;
        };
        let Ok(hash) = code.add_blob(&m.kind, version, bytes.to_vec()) else {
            continue;
        };
        let _ = code.bind(id, hash);
    }
    code
}

pub use kabl_standalone::code_load::{decline, register, stand_in_missing};

/// What the user is told when the patch opened with stand-ins: one line per module this patch
/// uses, naming the module id, version and cause.
pub fn warnings(state: &kabl_core::PatchState) -> Vec<String> {
    kabl_modules::code::standin::reports()
        .into_iter()
        .filter(|r| state.modules.values().any(|m| m.kind == r.kind))
        .map(|r| r.to_string())
        .collect()
}
