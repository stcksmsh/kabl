use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::op::{CableId, ModuleId, Op, ParamTarget, PortRef, Vec2};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModuleState {
    pub kind: String,
    pub pos: Vec2,
    #[serde(deserialize_with = "crate::composite::unique_map")]
    pub params: BTreeMap<String, f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CableState {
    pub from: PortRef,
    pub to: PortRef,
    #[serde(deserialize_with = "crate::composite::unique_map")]
    pub params: BTreeMap<String, f32>,
    pub steps: Vec<f32>,
}

/// The rebuildable state of a patch: the graph the compiler reads. `Annotate` and `Snapshot`
/// deliberately do not touch this — they're log-only (construction replay shows them, but they
/// don't affect the graph). See docs/decisions.md 2026-09-21 "core: op log inverse
/// simplifications".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PatchState {
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "crate::composite::unique_map"
    )]
    pub composites: BTreeMap<crate::CompositeId, crate::Composite>,
    #[serde(deserialize_with = "crate::composite::unique_map")]
    pub modules: BTreeMap<ModuleId, ModuleState>,
    #[serde(deserialize_with = "crate::composite::unique_map")]
    pub cables: BTreeMap<CableId, CableState>,
    /// Text labels per module (`Op::SetLabel`). Absent in files from before schema v3.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "unique_labels"
    )]
    pub labels: BTreeMap<ModuleId, BTreeMap<String, String>>,
}

fn unique_labels<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<BTreeMap<ModuleId, BTreeMap<String, String>>, D::Error> {
    #[derive(Deserialize)]
    #[serde(transparent)]
    struct Labels(
        #[serde(deserialize_with = "crate::composite::unique_map")] BTreeMap<String, String>,
    );
    crate::composite::unique_map::<D, ModuleId, Labels>(d).map(|labels| {
        labels
            .into_iter()
            .map(|(id, labels)| (id, labels.0))
            .collect()
    })
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
            Op::SetComposite { id, .. } => Op::SetComposite {
                id: *id,
                value: self.composites.get(id).cloned(),
            },
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
                ops.extend(self.removed_cable_ops(op));
                for (&cid, c) in &self.composites {
                    if c.members.contains(id)
                        || c.controls
                            .values()
                            .chain(c.ports.values())
                            .any(|e| e.target.module_id() == Some(*id))
                    {
                        ops.push(Op::SetComposite {
                            id: cid,
                            value: Some(c.clone()),
                        });
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
            Op::Disconnect { .. } => Op::Group {
                ops: self.removed_cable_ops(op),
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

    /// The module cable `id` ends on: its own destination or, for a route into a cable's
    /// parameter, where that cable ends. `None` when the cable is missing or the routes loop.
    pub fn end_module(&self, id: CableId) -> Option<ModuleId> {
        let mut cur = id;
        for _ in 0..=self.cables.len() {
            match &self.cables.get(&cur)?.to {
                PortRef::CableParam { cable, .. } => cur = *cable,
                to => return to.module_id(),
            }
        }
        None
    }

    /// Ops that put back every cable `op` (a `RemoveModule` or `Disconnect`) removes from this
    /// state, the routes into a removed cable's parameters included.
    fn removed_cable_ops(&self, op: &Op) -> Vec<Op> {
        let mut after = self.clone();
        after.apply(op);
        self.cables
            .iter()
            .filter(|(id, _)| !after.cables.contains_key(id))
            .flat_map(|(&id, c)| cable_restore_ops(id, c))
            .collect()
    }

    /// A route into a cable's parameter goes with that cable (no dangling routes), and so does a
    /// route into such a route.
    fn prune_cable_routes(&mut self) {
        loop {
            let before = self.cables.len();
            let ids: std::collections::BTreeSet<CableId> = self.cables.keys().copied().collect();
            self.cables.retain(
                |_, c| !matches!(&c.to, PortRef::CableParam { cable, .. } if !ids.contains(cable)),
            );
            if self.cables.len() == before {
                return;
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
            Op::SetComposite { id, value } => {
                if let Some(c) = value {
                    self.composites.insert(*id, c.clone());
                } else {
                    self.composites.remove(id);
                }
            }
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
                for c in self.composites.values_mut() {
                    c.members.remove(id);
                    c.controls.retain(|_, e| e.target.module_id() != Some(*id));
                    c.ports.retain(|_, e| e.target.module_id() != Some(*id));
                    if let Some(panel) = &mut c.panel {
                        panel.placements.retain(|key, _| {
                            c.controls.contains_key(key) || c.ports.contains_key(key)
                        });
                    }
                }
                // No dangling cables: a cable to or from a removed module goes with it.
                self.cables.retain(|_, c| {
                    c.from.module_id() != Some(*id) && c.to.module_id() != Some(*id)
                });
                self.prune_cable_routes();
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
                self.prune_cable_routes();
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
