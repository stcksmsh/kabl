use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::op::{CableId, ModuleId, Op, ParamTarget, PortRef, Vec2};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModuleState {
    pub kind: String,
    pub pos: Vec2,
    pub params: BTreeMap<String, f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CableState {
    pub from: PortRef,
    pub to: PortRef,
    pub params: BTreeMap<String, f32>,
    pub steps: Vec<f32>,
}

/// The rebuildable state of a patch: the graph the compiler reads. `Annotate` and `Snapshot`
/// deliberately do not touch this — they're log-only (construction replay shows them, but they
/// don't affect the graph). See docs/decisions.md 2026-09-21 "core: op log inverse
/// simplifications".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PatchState {
    pub modules: BTreeMap<ModuleId, ModuleState>,
    pub cables: BTreeMap<CableId, CableState>,
}

impl PatchState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inverse of `op` computed against the state *before* `op` is applied.
    pub fn inverse_for(&self, op: &Op) -> Op {
        match op {
            Op::AddModule { id, .. } => Op::RemoveModule { id: *id },
            Op::RemoveModule { id } => {
                let m = self
                    .modules
                    .get(id)
                    .expect("inverse_for(RemoveModule) requires the module to exist");
                Op::AddModule {
                    id: *id,
                    kind: m.kind.clone(),
                    pos: m.pos,
                }
            }
            Op::Connect { id, .. } => Op::Disconnect { id: *id },
            Op::Disconnect { id } => {
                let c = self
                    .cables
                    .get(id)
                    .expect("inverse_for(Disconnect) requires the cable to exist");
                Op::Connect {
                    id: *id,
                    from: c.from.clone(),
                    to: c.to.clone(),
                }
            }
            Op::SetParam { target, .. } => {
                let old = self.param(target).unwrap_or(0.0);
                Op::SetParam {
                    target: target.clone(),
                    value: old,
                }
            }
            Op::SetCablePattern { id, .. } => {
                let old = self
                    .cables
                    .get(id)
                    .map(|c| c.steps.clone())
                    .unwrap_or_default();
                Op::SetCablePattern {
                    id: *id,
                    steps: old,
                }
            }
            Op::MoveModule { id, .. } => {
                let old = self
                    .modules
                    .get(id)
                    .expect("inverse_for(MoveModule) requires the module to exist")
                    .pos;
                Op::MoveModule { id: *id, pos: old }
            }
            Op::Snapshot { name } => Op::Snapshot { name: name.clone() },
            Op::Annotate { text } => Op::Annotate { text: text.clone() },
        }
    }

    fn param(&self, target: &ParamTarget) -> Option<f32> {
        match target {
            ParamTarget::Module { id, param } => self.modules.get(id)?.params.get(param).copied(),
            ParamTarget::Cable { id, param } => self.cables.get(id)?.params.get(param).copied(),
        }
    }

    pub fn apply(&mut self, op: &Op) {
        match op {
            Op::AddModule { id, kind, pos } => {
                self.modules.insert(
                    *id,
                    ModuleState {
                        kind: kind.clone(),
                        pos: *pos,
                        params: BTreeMap::new(),
                    },
                );
            }
            Op::RemoveModule { id } => {
                self.modules.remove(id);
            }
            Op::Connect { id, from, to } => {
                self.cables.insert(
                    *id,
                    CableState {
                        from: from.clone(),
                        to: to.clone(),
                        params: BTreeMap::new(),
                        steps: Vec::new(),
                    },
                );
            }
            Op::Disconnect { id } => {
                self.cables.remove(id);
            }
            Op::SetParam { target, value } => match target {
                ParamTarget::Module { id, param } => {
                    if let Some(m) = self.modules.get_mut(id) {
                        m.params.insert(param.clone(), *value);
                    }
                }
                ParamTarget::Cable { id, param } => {
                    if let Some(c) = self.cables.get_mut(id) {
                        c.params.insert(param.clone(), *value);
                    }
                }
            },
            Op::SetCablePattern { id, steps } => {
                if let Some(c) = self.cables.get_mut(id) {
                    c.steps = steps.clone();
                }
            }
            Op::MoveModule { id, pos } => {
                if let Some(m) = self.modules.get_mut(id) {
                    m.pos = *pos;
                }
            }
            Op::Snapshot { .. } | Op::Annotate { .. } => {}
        }
    }
}
