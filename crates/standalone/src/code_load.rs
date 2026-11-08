//! Loading the code a patch embeds, for every front end (the UI library, `kabl-ui --patch`,
//! `kabl --patch`): blobs that load are registered; anything that cannot load (declined,
//! invalid, other ABI, forbidden import, missing) becomes a stand-in (see
//! `kabl_modules::code::standin`) so the patch still opens.

use kabl_modules::code::state::SavedState;
use kabl_modules::code::{CodeHash, CodeScope};

/// Compiles and registers every blob of `code` so the patch can build, and returns the
/// [`CodeScope`] that names, for each kind, the exact blob this file embeds: compile with it
/// (`kabl_engine::compile::compile_scoped`) and another song in the process cannot change which
/// code runs. A blob that cannot load (invalid, forbidden import, other ABI, or metadata naming
/// another module or version than the file says) becomes a stand-in with the same ports and
/// params, so the patch still opens.
pub fn register(code: &kabl_core::embedded::Embedded, state: &kabl_core::PatchState) -> CodeScope {
    let mut scope = CodeScope::new();
    for (_, b) in code.blobs() {
        let cause = match kabl_modules::code::host::register_blob(&b.bytes) {
            Ok(r) if r.kind == b.module_id && r.version == b.version => {
                scope = scope.with(&b.module_id, r.key);
                continue;
            }
            Ok(r) => format!("the code says it is {} v{}", r.kind, r.version),
            Err(e) => e.to_string(),
        };
        let key = stand_in(&b.module_id, b.version, Some(&b.bytes), &cause, state);
        scope = scope.with(&b.module_id, key);
    }
    read_settings(code, &scope);
    scope
}

/// Puts the module settings the file kept into `scope`: each running instance loads its own
/// when the graph is built, and a stand-in's are written back unchanged on save.
fn read_settings(code: &kabl_core::embedded::Embedded, scope: &CodeScope) {
    for (instance, _) in code.refs() {
        let Some(blob) = code.instance_blob(instance) else {
            continue;
        };
        if let Some(saved) = code
            .kept_state(instance, blob.version)
            .and_then(SavedState::from_kept)
        {
            scope.set_kept_state(instance, &blob.module_id, saved);
        }
    }
}

/// The code a patch being saved carries: one blob per code-module kind it uses (the song's own
/// blob, by `scope`), bound to every instance of that kind, and each instance's saved settings.
/// Built-in kinds add nothing. Asks the running graph for fresh settings first, waiting up to
/// `wait` for the audio thread; an engine that is not running leaves the last known settings.
pub fn embedded_for(
    state: &kabl_core::PatchState,
    scope: &CodeScope,
    wait: std::time::Duration,
) -> kabl_core::embedded::Embedded {
    if !wait.is_zero() {
        scope.capture(wait);
    } else {
        scope.collect();
    }
    let settings: std::collections::BTreeMap<u64, SavedState> =
        scope.settings().into_iter().collect();
    let mut code = kabl_core::embedded::Embedded::new();
    for (&id, m) in &state.modules {
        let Some((version, bytes)) = kabl_modules::code::host::blob_for_in(scope, &m.kind) else {
            continue;
        };
        let Ok(hash) = code.add_blob(&m.kind, version, bytes.to_vec()) else {
            continue;
        };
        let _ = code.bind(id, hash);
        if let Some(saved) = settings.get(&id) {
            let _ = code.keep_state(id, version, saved.to_kept());
        }
    }
    code
}

/// Every blob of `code` becomes a stand-in without running (the user declined it).
pub fn decline(code: &kabl_core::embedded::Embedded, state: &kabl_core::PatchState) -> CodeScope {
    let mut scope = CodeScope::new();
    for (_, b) in code.blobs() {
        let key = stand_in(
            &b.module_id,
            b.version,
            Some(&b.bytes),
            "you declined to run it",
            state,
        );
        scope = scope.with(&b.module_id, key);
    }
    read_settings(code, &scope);
    scope
}

/// Module kinds the patch uses that nothing provides (no blob in the file, nothing installed)
/// become stand-ins too, so a missing module never stops the song opening.
pub fn stand_in_missing(state: &kabl_core::PatchState) {
    stand_in_missing_in(state, &CodeScope::new());
}

/// [`stand_in_missing`] for a song with `scope`; returns the scope extended with the stand-ins.
pub fn stand_in_missing_in(state: &kabl_core::PatchState, scope: &CodeScope) -> CodeScope {
    let mut scope = scope.clone();
    for m in state.modules.values() {
        if kabl_modules::registry::info_for_in(&scope, &m.kind).is_none() {
            let key = stand_in(
                &m.kind,
                0,
                None,
                "its code is not in this file and not installed",
                state,
            );
            scope = scope.with(&m.kind, key);
        }
    }
    scope
}

fn stand_in(
    kind: &str,
    version: u32,
    bytes: Option<&[u8]>,
    cause: &str,
    state: &kabl_core::PatchState,
) -> CodeHash {
    kabl_modules::code::host::register_stand_in_keyed(
        kind,
        version,
        bytes,
        cause,
        Some(usage_meta(state, kind, version)),
    )
    .1
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

/// The command-line `--patch` paths have no trust notice (the user named the file): register
/// everything, make stand-ins for what cannot load or is missing, and return one warning line per
/// stand-in the patch uses.
pub fn register_for_cli(
    code: &kabl_core::embedded::Embedded,
    state: &kabl_core::PatchState,
) -> Vec<String> {
    let (scope, warnings) = register_for_cli_scoped(code, state);
    // The command-line paths open one song: it is the open song.
    kabl_modules::code::host::set_active_scope(scope);
    warnings
}

/// [`register_for_cli`], also returning the song's [`CodeScope`].
pub fn register_for_cli_scoped(
    code: &kabl_core::embedded::Embedded,
    state: &kabl_core::PatchState,
) -> (CodeScope, Vec<String>) {
    let scope = stand_in_missing_in(state, &register(code, state));
    let warnings = kabl_modules::code::standin::reports_in(&scope)
        .into_iter()
        .filter(|r| state.modules.values().any(|m| m.kind == r.kind))
        .map(|r| r.to_string())
        .collect();
    (scope, warnings)
}
