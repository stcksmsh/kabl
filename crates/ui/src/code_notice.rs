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

/// Compiles and registers every blob of `code` so the patch can build. A blob that cannot load
/// (invalid, forbidden import, other ABI, or metadata naming another module or version than the
/// file says) becomes a stand-in with the same ports and params, so the patch still opens.
pub fn register(code: &Embedded, state: &kabl_core::PatchState) {
    for (_, b) in code.blobs() {
        let cause = match kabl_modules::code::host::register_blob(&b.bytes) {
            Ok(r) if r.kind == b.module_id && r.version == b.version => continue,
            Ok(r) => format!("the code says it is {} v{}", r.kind, r.version),
            Err(e) => e.to_string(),
        };
        stand_in(&b.module_id, b.version, Some(&b.bytes), &cause, state);
    }
}

/// Every blob of `code` becomes a stand-in without running (the user declined it).
pub fn decline(code: &Embedded, state: &kabl_core::PatchState) {
    for (_, b) in code.blobs() {
        stand_in(
            &b.module_id,
            b.version,
            Some(&b.bytes),
            "you declined to run it",
            state,
        );
    }
}

/// Module kinds the patch uses that nothing provides (no blob in the file, nothing installed)
/// become stand-ins too, so a missing module never stops the song opening.
pub fn stand_in_missing(state: &kabl_core::PatchState) {
    for m in state.modules.values() {
        if kabl_modules::registry::info_for(&m.kind).is_none() {
            stand_in(
                &m.kind,
                0,
                None,
                "its code is not in this file and not installed",
                state,
            );
        }
    }
}

fn stand_in(
    kind: &str,
    version: u32,
    bytes: Option<&[u8]>,
    cause: &str,
    state: &kabl_core::PatchState,
) {
    kabl_modules::code::host::register_stand_in(
        kind,
        version,
        bytes,
        cause,
        Some(usage_meta(state, kind, version)),
    );
}

/// Ports and params of `kind` as the patch uses them: for a module whose own metadata cannot be
/// read. Every port is audio, one wide; every param spans a wide range around what is stored.
fn usage_meta(
    state: &kabl_core::PatchState,
    kind: &str,
    version: u32,
) -> kabl_modules::code::CodeMeta {
    use kabl_core::PortRef;
    use kabl_modules::code::{CodeMeta, CodeParam, CodePort};
    use kabl_modules::info::{Category, PortDirection, PortType, Rate, Taper};
    let ids: Vec<_> = state
        .modules
        .iter()
        .filter(|(_, m)| m.kind == kind)
        .map(|(&id, _)| id)
        .collect();
    let mut ports: Vec<CodePort> = Vec::new();
    let mut params: Vec<CodeParam> = Vec::new();
    let mut add_port = |name: &str, direction| {
        if !ports
            .iter()
            .any(|p| p.name == name && p.direction == direction)
        {
            ports.push(CodePort {
                name: name.to_owned(),
                port_type: PortType::Audio,
                direction,
                width: 1,
            });
        }
    };
    let mut add_param = |name: &str| {
        if !params.iter().any(|p| p.name == name) {
            params.push(CodeParam {
                name: name.to_owned(),
                min: -1.0e6,
                max: 1.0e6,
                default: 0.0,
                unit: String::new(),
                taper: Taper::Linear,
                smoothing_ms: 0.0,
            });
        }
    };
    for c in state.cables.values() {
        for (end, dir) in [
            (&c.from, PortDirection::Output),
            (&c.to, PortDirection::Input),
        ] {
            match end {
                PortRef::Module { id, port } if ids.contains(id) => add_port(port, dir),
                PortRef::Param { id, param } if ids.contains(id) => add_param(param),
                _ => {}
            }
        }
    }
    for id in &ids {
        for name in state.modules[id].params.keys() {
            add_param(name);
        }
    }
    CodeMeta {
        kind: kind.to_owned(),
        version,
        name: kind.to_owned(),
        explain: String::new(),
        category: Category::Utility,
        rate: Rate::Voice,
        ports,
        params,
        advanced: vec![],
        width_units: 4,
    }
}

/// What the user is told when the patch opened with stand-ins: one line per module this patch
/// uses, naming the module id, version and cause.
pub fn warnings(state: &kabl_core::PatchState) -> Vec<String> {
    kabl_modules::code::standin::reports()
        .into_iter()
        .filter(|r| state.modules.values().any(|m| m.kind == r.kind))
        .map(|r| r.to_string())
        .collect()
}
