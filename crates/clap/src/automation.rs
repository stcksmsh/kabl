//! Stable lanes name document identities, never panel positions or compiled indices.
use kabl_core::PatchState;
use kabl_engine::runtime::{param_class, ParamClass, RuntimeTarget};
use kabl_modules::{registry, ParamInfo, Taper};
use serde::{Deserialize, Serialize};
pub const SLOTS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub module: u64,
    pub kind: String,
    pub param: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lane {
    pub target: Option<Target>,
    /// Deletion is durable. Only explicit assignment may reuse this lane.
    pub retired: bool,
}
#[derive(Clone, Copy)]
pub struct Resolved {
    pub target: RuntimeTarget,
    pub param: &'static ParamInfo,
    pub base: f32,
}
pub type Bank = [Option<Resolved>; SLOTS];
pub fn resolve(patch: &PatchState, target: &Target) -> Option<Resolved> {
    let module = patch.modules.get(&target.module)?;
    if module.kind != target.kind {
        return None;
    }
    let info = registry::info_for(&target.kind)?;
    let (index, param) = info
        .params
        .iter()
        .enumerate()
        .find(|(_, p)| p.name == target.param)?;
    if info.kind == "seq" && kabl_modules::builtins::seq::is_step_param(param.name) {
        return None;
    }
    if param_class(info.kind, param.name) != ParamClass::Runtime || param.taper == Taper::Stepped {
        return None;
    }
    Some(Resolved {
        target: RuntimeTarget::Param {
            id: target.module,
            kind: info.kind,
            index: index as u16,
        },
        param,
        base: kabl_ui::routing::base_value(patch, target.module, param),
    })
}
pub fn bank(patch: &PatchState, lanes: &[Lane; SLOTS]) -> Bank {
    std::array::from_fn(|i| lanes[i].target.as_ref().and_then(|t| resolve(patch, t)))
}
pub fn candidates(patch: &PatchState) -> Vec<Target> {
    let mut targets = Vec::new();
    for (&id, module) in &patch.modules {
        if let Some(info) = registry::info_for(&module.kind) {
            for param in info.params {
                let target = Target {
                    module: id,
                    kind: module.kind.clone(),
                    param: param.name.into(),
                };
                if resolve(patch, &target).is_some() {
                    targets.push(target);
                }
            }
        }
    }
    // Pins and macros first, then face controls before advanced ones (a module's growing
    // advanced list must not push the next module's controls out of the sixteen slots);
    // deterministic module/name ordering breaks ties.
    targets.sort_by_key(|t| {
        let advanced =
            registry::info_for(&t.kind).is_some_and(|i| i.advanced.contains(&t.param.as_str()));
        (
            !kabl_ui::perform::is_pinned(patch, t.module, &t.param),
            t.kind != "macro",
            advanced,
            t.module,
            t.param.clone(),
        )
    });
    targets
}
pub fn retire_missing(patch: &PatchState, lanes: &mut [Lane; SLOTS]) -> bool {
    let mut changed = false;
    for lane in lanes {
        if lane
            .target
            .as_ref()
            .is_some_and(|t| resolve(patch, t).is_none())
        {
            lane.target = None;
            lane.retired = true;
            changed = true;
        }
    }
    changed
}
pub fn normalized(patch: &PatchState, target: &Target) -> f32 {
    resolve(patch, target).map_or(0.0, |r| {
        r.param
            .to_norm(kabl_ui::routing::base_value(patch, target.module, r.param))
    })
}

/// Removing or moving a lane restores the former target's document base.
pub fn replace_bank(
    engine: &mut kabl_engine::patch_engine::PatchEngine,
    current: &mut Bank,
    next: Bank,
    restored: Bank,
) {
    for (i, previous) in current.iter().enumerate() {
        if let Some(previous) = previous {
            if next[i].is_none_or(|new| new.target != previous.target) {
                if let Some(restored) = restored[i].filter(|r| r.target == previous.target) {
                    engine.automate(i, restored.target, restored.base);
                }
            }
        }
    }
    *current = next;
}

/// Resolve release values from the latest accepted document, including publication retries.
pub fn release_bases(patch: &PatchState, previous: Bank) -> Bank {
    previous.map(|previous| {
        previous.and_then(|mut resolved| {
            let RuntimeTarget::Param { id, kind, .. } = resolved.target else {
                return None;
            };
            let module = patch.modules.get(&id)?;
            if module.kind != kind {
                return None;
            }
            resolved.base = kabl_ui::routing::base_value(patch, id, resolved.param);
            Some(resolved)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seq_step_params_are_never_lane_candidates() {
        use kabl_modules::builtins::seq::is_step_param;
        for name in ["p1", "g8", "v12", "r32", "k5", "b.p1", "c.v32", "d.k9"] {
            assert!(is_step_param(name), "{name}");
        }
        for name in [
            "length",
            "transpose",
            "gate_len",
            "gate_mode",
            "b.length",
            "b.gate_len",
        ] {
            assert!(!is_step_param(name), "{name}");
        }
        let info = registry::info_for("seq").unwrap();
        assert!(info.params.iter().any(|p| is_step_param(p.name)));
        let mut patch = PatchState::default();
        patch.modules.insert(
            1,
            kabl_core::ModuleState {
                kind: "seq".into(),
                pos: kabl_core::Vec2::default(),
                params: Default::default(),
            },
        );
        assert!(!candidates(&patch).is_empty());
        for t in candidates(&patch) {
            assert!(!is_step_param(&t.param), "{}", t.param);
        }
    }
}
