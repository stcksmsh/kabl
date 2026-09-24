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
    /// Text labels per module (`Op::SetLabel`). Absent in files from before schema v3.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<ModuleId, BTreeMap<String, String>>,
}

impl PatchState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inverse of `op` computed against the state *before* `op` is applied. An op on something
    /// that doesn't exist has an empty `Group` (no-op) inverse. Applying `op` then its inverse
    /// restores the state exactly: params absent before stay absent, a removed
    /// module comes back with its params and cables, a removed cable with its settings.
    pub fn inverse_for(&self, op: &Op) -> Op {
        match op {
            Op::AddModule { id, .. } => Op::RemoveModule { id: *id },
            Op::RemoveModule { id } => {
                let Some(m) = self.modules.get(id) else {
                    return Op::Group { ops: Vec::new() };
                };
                let mut ops = vec![Op::AddModule {
                    id: *id,
                    kind: m.kind.clone(),
                    pos: m.pos,
                }];
                for (param, &value) in &m.params {
                    ops.push(Op::SetParam {
                        target: ParamTarget::Module {
                            id: *id,
                            param: param.clone(),
                        },
                        value,
                    });
                }
                for (key, text) in self.labels.get(id).into_iter().flatten() {
                    ops.push(Op::SetLabel {
                        id: *id,
                        key: key.clone(),
                        text: Some(text.clone()),
                    });
                }
                for (&cid, c) in &self.cables {
                    if c.from.module_id() == *id || c.to.module_id() == *id {
                        ops.extend(cable_restore_ops(cid, c));
                    }
                }
                Op::Group { ops }
            }
            Op::Connect { id, .. } => match self.cables.get(id) {
                // Connecting over an existing id replaces it; the inverse puts the old one back.
                Some(c) => Op::Group {
                    ops: cable_restore_ops(*id, c),
                },
                None => Op::Disconnect { id: *id },
            },
            Op::Disconnect { id } => Op::Group {
                ops: self
                    .cables
                    .get(id)
                    .map(|c| cable_restore_ops(*id, c))
                    .unwrap_or_default(),
            },
            Op::SetParam { target, .. } | Op::UnsetParam { target } => match self.param(target) {
                Some(old) => Op::SetParam {
                    target: target.clone(),
                    value: old,
                },
                None => Op::UnsetParam {
                    target: target.clone(),
                },
            },
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
            Op::MoveModule { id, .. } => match self.modules.get(id) {
                Some(m) => Op::MoveModule {
                    id: *id,
                    pos: m.pos,
                },
                None => Op::Group { ops: Vec::new() },
            },
            Op::SetLabel { id, key, .. } => Op::SetLabel {
                id: *id,
                key: key.clone(),
                text: self.label(*id, key).map(str::to_string),
            },
            Op::Snapshot { name } => Op::Snapshot { name: name.clone() },
            Op::Annotate { text } => Op::Annotate { text: text.clone() },
            Op::Group { ops } => {
                let mut scratch = self.clone();
                let mut inverses = Vec::with_capacity(ops.len());
                for op in ops {
                    inverses.push(scratch.inverse_for(op));
                    scratch.apply(op);
                }
                inverses.reverse();
                Op::Group { ops: inverses }
            }
        }
    }

    pub fn label(&self, id: ModuleId, key: &str) -> Option<&str> {
        self.labels.get(&id)?.get(key).map(String::as_str)
    }

    pub fn param(&self, target: &ParamTarget) -> Option<f32> {
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
                self.labels.remove(id);
                // No dangling cables: a cable to or from a removed module goes with it.
                self.cables
                    .retain(|_, c| c.from.module_id() != *id && c.to.module_id() != *id);
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
            Op::UnsetParam { target } => match target {
                ParamTarget::Module { id, param } => {
                    if let Some(m) = self.modules.get_mut(id) {
                        m.params.remove(param);
                    }
                }
                ParamTarget::Cable { id, param } => {
                    if let Some(c) = self.cables.get_mut(id) {
                        c.params.remove(param);
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
            Op::SetLabel { id, key, text } => match text {
                Some(t) if self.modules.contains_key(id) => {
                    self.labels
                        .entry(*id)
                        .or_default()
                        .insert(key.clone(), t.clone());
                }
                _ => {
                    if let Some(m) = self.labels.get_mut(id) {
                        m.remove(key);
                        if m.is_empty() {
                            self.labels.remove(id);
                        }
                    }
                }
            },
            Op::Snapshot { .. } | Op::Annotate { .. } => {}
            Op::Group { ops } => {
                for op in ops {
                    self.apply(op);
                }
            }
        }
    }
}

/// Ops that recreate cable `id` exactly: the connection, its params, its step pattern.
fn cable_restore_ops(id: CableId, c: &CableState) -> Vec<Op> {
    let mut ops = vec![Op::Connect {
        id,
        from: c.from.clone(),
        to: c.to.clone(),
    }];
    for (param, &value) in &c.params {
        ops.push(Op::SetParam {
            target: ParamTarget::Cable {
                id,
                param: param.clone(),
            },
            value,
        });
    }
    if !c.steps.is_empty() {
        ops.push(Op::SetCablePattern {
            id,
            steps: c.steps.clone(),
        });
    }
    ops
}
