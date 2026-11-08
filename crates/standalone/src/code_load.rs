//! Loading the code a patch embeds, for every front end (the UI library, `kabl-ui --patch`,
//! `kabl --patch`): blobs that load are registered; anything that cannot load (declined,
//! invalid, other ABI, forbidden import, missing) becomes a stand-in (see
//! `kabl_modules::code::standin`) so the patch still opens.

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
    scope
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
    register_for_cli_scoped(code, state).1
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
