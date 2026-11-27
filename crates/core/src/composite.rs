//! Composite ownership and public bindings. DSP remains the document's flat leaf graph.
use crate::panel::{Kind, Source};
use crate::{ModuleId, PatchState, PortRef, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub type CompositeId = u64;
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub id: String,
    pub version: u32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exposure {
    pub label: String,
    pub target: PortRef,
    /// Further inputs this port feeds (fan-in, schema v9): one outside source reaching several
    /// inside targets is one public port. Empty for a control and for a one-target port.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub also: Vec<PortRef>,
}
impl Exposure {
    /// `target` then `also`.
    pub fn targets(&self) -> impl Iterator<Item = &PortRef> {
        std::iter::once(&self.target).chain(&self.also)
    }
    /// Keeps the targets `keep` accepts, the first of them as `target`. False when none is
    /// left, and the caller drops the exposure.
    pub fn retain_targets(&mut self, mut keep: impl FnMut(&PortRef) -> bool) -> bool {
        let mut left = std::iter::once(self.target.clone())
            .chain(std::mem::take(&mut self.also))
            .filter(|t| keep(t));
        let Some(first) = left.next() else {
            return false;
        };
        self.target = first;
        self.also = left.collect();
        true
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Composite {
    pub name: String,
    pub help: String,
    pub definition: Definition,
    pub parent: Option<CompositeId>,
    #[serde(deserialize_with = "unique_set")]
    pub members: BTreeSet<ModuleId>,
    pub pos: Vec2,
    #[serde(deserialize_with = "unique_map")]
    pub ports: BTreeMap<u64, Exposure>,
    #[serde(deserialize_with = "unique_map")]
    pub controls: BTreeMap<u64, Exposure>,
    pub next_interface: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel: Option<Box<crate::panel::Panel>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub snapshots: Vec<Snapshot>,
}

/// A named set of values for the composite's exposed controls.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    /// From `next_interface`; never reused, so a deletion shifts nothing.
    pub id: u64,
    pub name: String,
    /// Control exposure ID -> value in the param's own units.
    #[serde(deserialize_with = "unique_map")]
    pub values: BTreeMap<u64, f32>,
    /// 0 = jump.
    #[serde(default)]
    pub morph_beats: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock: Option<ModuleId>,
}

/// The most snapshots one composite holds.
pub const MAX_SNAPSHOTS: usize = 16;
/// The longest snapshot morph, in beats (`cues::MAX_MORPH_BEATS` in the UI crate).
pub const MAX_MORPH_BEATS: f32 = 16.0;

/// Reject repeated semantic keys, including differently spelled numeric IDs.
pub fn unique_map<'de, D, K, V>(d: D) -> Result<BTreeMap<K, V>, D::Error>
where
    D: serde::Deserializer<'de>,
    K: Deserialize<'de> + Ord,
    V: Deserialize<'de>,
{
    struct Unique<K, V>(std::marker::PhantomData<(K, V)>);
    impl<'de, K: Deserialize<'de> + Ord, V: Deserialize<'de>> serde::de::Visitor<'de> for Unique<K, V> {
        type Value = BTreeMap<K, V>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("map with unique identities")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut a: A,
        ) -> Result<Self::Value, A::Error> {
            let mut map = BTreeMap::new();
            while let Some((k, v)) = a.next_entry()? {
                if map.insert(k, v).is_some() {
                    return Err(serde::de::Error::custom("Duplicate identity/key"));
                }
            }
            Ok(map)
        }
    }
    d.deserialize_map(Unique(std::marker::PhantomData))
}

pub fn descendants(p: &PatchState, root: CompositeId) -> BTreeSet<CompositeId> {
    let mut ids = BTreeSet::from([root]);
    for _ in 0..8 {
        let before = ids.len();
        for (&id, c) in &p.composites {
            if c.parent.is_some_and(|parent| ids.contains(&parent)) {
                ids.insert(id);
            }
        }
        if ids.len() == before {
            break;
        }
    }
    ids
}
pub fn leaves(p: &PatchState, root: CompositeId) -> BTreeSet<ModuleId> {
    descendants(p, root)
        .into_iter()
        .filter_map(|id| p.composites.get(&id))
        .flat_map(|c| c.members.iter().copied())
        .collect()
}

/// The module an interface binding belongs to: its own module, or, for a cable parameter, the
/// module that cable ends on (where its route takes effect). Schema v9 allows a cable parameter
/// as a port, schema v10 as a control as well.
pub fn owner(p: &PatchState, target: &PortRef) -> Option<ModuleId> {
    match target {
        PortRef::CableParam { cable, .. } => p.end_module(*cable),
        t => t.module_id(),
    }
}

/// Whether the composite binds a cable parameter, which only schema v9 readers accept.
pub fn binds_cable_param(c: &Composite) -> bool {
    c.ports
        .values()
        .flat_map(Exposure::targets)
        .any(|t| matches!(t, PortRef::CableParam { .. }))
}

/// The most further targets (`Exposure::also`) one port may hold.
pub const MAX_ALSO: usize = 32;

/// Whether a port of the composite feeds several targets, which only schema v9 readers accept.
pub fn fans_in(c: &Composite) -> bool {
    c.ports.values().any(|e| !e.also.is_empty())
}

fn is_clock(p: &PatchState, m: ModuleId) -> bool {
    p.modules.get(&m).is_some_and(|m| m.kind == "clock")
}

/// Whether `(d, key)` is a `button` on the face of a strict descendant of `c`: the only
/// target a face button may press, so every press chain goes strictly deeper and ends.
fn is_face_button(p: &PatchState, c: CompositeId, d: CompositeId, key: u64) -> bool {
    d != c
        && descendants(p, c).contains(&d)
        && p.composites[&d]
            .panel
            .as_ref()
            .and_then(|panel| panel.placements.get(&key))
            .is_some_and(|pl| pl.kind == Kind::Button)
}

/// The edits that drop every reference left dangling in `p`, one `SetComposite` per changed
/// composite, innermost first (dropping a nested button can orphan a parent's face target).
/// `PatchEditor` logs them in the same step as the edit that caused them.
pub fn prune(p: &PatchState) -> Vec<crate::Op> {
    let depth = |mut id: CompositeId| {
        let mut n = 0;
        while let Some(parent) = p.composites.get(&id).and_then(|c| c.parent) {
            n += 1;
            id = parent;
            if n > 8 {
                break;
            }
        }
        n
    };
    let mut order: Vec<CompositeId> = p.composites.keys().copied().collect();
    order.sort_by_key(|&id| std::cmp::Reverse(depth(id)));
    let mut state = p.clone();
    let mut ops = Vec::new();
    for id in order {
        let mut c = state.composites[&id].clone();
        let leaves = leaves(&state, id);
        let snapshot_ids: BTreeSet<u64> = c.snapshots.iter().map(|s| s.id).collect();
        for s in &mut c.snapshots {
            s.values.retain(|k, _| c.controls.contains_key(k));
            if s.clock.is_some_and(|m| !is_clock(&state, m)) {
                s.clock = None;
            }
        }
        if let Some(panel) = &mut c.panel {
            panel.placements.retain(|key, pl| match &pl.source {
                _ if pl.kind == Kind::Jack => c.ports.contains_key(key),
                _ if pl.kind.binds_control() => c.controls.contains_key(key),
                None => true,
                Some(Source::Module { id: m, .. }) => leaves.contains(m),
                Some(Source::Face { composite, key }) => {
                    is_face_button(&state, id, *composite, *key)
                }
                Some(Source::Snapshot { id: s }) => snapshot_ids.contains(s),
            });
        }
        if c != state.composites[&id] {
            state.composites.insert(id, c.clone());
            ops.push(crate::Op::SetComposite { id, value: Some(c) });
        }
    }
    ops
}

/// Whether the composite uses a face element or field that only schema v10 readers accept,
/// a control on a cable parameter included.
pub fn needs_faces(c: &Composite) -> bool {
    !c.snapshots.is_empty()
        || c.controls
            .values()
            .any(|e| matches!(e.target, PortRef::CableParam { .. }))
        || c.panel.as_ref().is_some_and(|panel| {
            panel.compact_width.is_some()
                || panel.placements.values().any(|pl| {
                    !matches!(pl.kind, Kind::Knob | Kind::Selector | Kind::Jack)
                        || pl.source.is_some()
                        || pl.label.is_some()
                        || !pl.icons.is_empty()
                })
        })
}

pub fn validate(p: &PatchState) -> Result<(), String> {
    if p.composites.len() > 128 {
        return Err("Composite count limit (128)".into());
    }
    for (&slot, t) in &p.tables {
        if !(1..=crate::table::MAX_TABLES).contains(&slot) {
            return Err(format!(
                "Table slot {slot}: slots are 1-{}",
                crate::table::MAX_TABLES
            ));
        }
        t.validate()?;
    }
    let mut owned = BTreeSet::new();
    let mut artwork_pixels = 0u64;
    for (&id, c) in &p.composites {
        if id == u64::MAX
            || c.name.is_empty()
            || c.name.len() > 128
            || c.help.len() > 4096
            || c.definition.id.is_empty()
            || c.definition.id.len() > 128
            || !c.pos.x.is_finite()
            || !c.pos.y.is_finite()
            || c.ports.len() > 32
            || c.controls.len() > 32
            || c.next_interface == u64::MAX
        {
            return Err(format!(
                "Composite {id}: invalid metadata or interface limit"
            ));
        }
        for member in &c.members {
            if !p.modules.contains_key(member) || !owned.insert(*member) {
                return Err(format!(
                    "Composite {id}: missing or multiply owned module {member}"
                ));
            }
        }
        let mut at = Some(id);
        let mut path = BTreeSet::new();
        let mut definitions = BTreeSet::new();
        let mut depth = 0;
        while let Some(parent) = at {
            depth += 1;
            if !path.insert(parent) {
                return Err(format!(
                    "Composite {id}: recursive ownership through {parent}"
                ));
            }
            if depth > 8 {
                return Err(format!("Composite {id}: nesting limit (8)"));
            }
            let ancestor = p
                .composites
                .get(&parent)
                .ok_or_else(|| format!("Composite {id}: missing parent {parent}"))?;
            if !definitions.insert(&ancestor.definition) {
                return Err(format!(
                    "Composite {id}: recursive definition {} version {}",
                    ancestor.definition.id, ancestor.definition.version
                ));
            }
            at = ancestor.parent;
        }
        let members = leaves(p, id);
        let mut public = BTreeSet::new();
        for (&key, e) in c.ports.iter().chain(&c.controls) {
            let mut targets = Vec::new();
            if !public.insert(key)
                || key >= c.next_interface
                || e.label.is_empty()
                || e.label.len() > 128
                || e.also.len() > MAX_ALSO
                || !e.targets().all(|t| {
                    let fresh = !targets.contains(&t);
                    targets.push(t);
                    fresh && owner(p, t).is_some_and(|m| members.contains(&m))
                })
            {
                return Err(format!("Composite {id}: invalid public binding {key}"));
            }
        }
        if c.snapshots.len() > MAX_SNAPSHOTS {
            return Err(format!("Composite {id}: at most {MAX_SNAPSHOTS} snapshots"));
        }
        let mut snapshot_ids = BTreeSet::new();
        for s in &c.snapshots {
            if !snapshot_ids.insert(s.id) || s.id >= c.next_interface {
                return Err(format!(
                    "Composite {id}: snapshot {} reuses an id or is not from next_interface",
                    s.id
                ));
            }
            if s.name.is_empty() || s.name.chars().count() > 40 {
                return Err(format!(
                    "Composite {id}: snapshot {} name must be 1-40 characters",
                    s.id
                ));
            }
            if let Some(key) = s.values.keys().find(|k| !c.controls.contains_key(k)) {
                return Err(format!(
                    "Composite {id}: snapshot {} holds {key}, which is not a control",
                    s.id
                ));
            }
            if s.values.values().any(|v| !v.is_finite())
                || !(0.0..=MAX_MORPH_BEATS).contains(&s.morph_beats)
            {
                return Err(format!(
                    "Composite {id}: snapshot {} value or morph length out of range",
                    s.id
                ));
            }
            if s.clock.is_some_and(|m| !is_clock(p, m)) {
                return Err(format!(
                    "Composite {id}: snapshot {} clock is not a clock module",
                    s.id
                ));
            }
        }
        if let Some(panel) = &c.panel {
            panel.validate(c)?;
            for (&key, pl) in &panel.placements {
                let valid = match &pl.source {
                    None => true,
                    Some(Source::Module { id: m, .. }) => members.contains(m),
                    Some(Source::Face { composite, key }) => {
                        is_face_button(p, id, *composite, *key)
                    }
                    Some(Source::Snapshot { id: s }) => snapshot_ids.contains(s),
                };
                if !valid {
                    return Err(format!(
                        "Composite {id}: panel element {key} names a missing source"
                    ));
                }
            }
            let icons = panel.placements.values().flat_map(|pl| &pl.icons);
            for art in [&panel.light, &panel.dark]
                .into_iter()
                .flatten()
                .chain(icons)
            {
                artwork_pixels +=
                    u64::from(u32::from_be_bytes(art.png[16..20].try_into().unwrap()))
                        * u64::from(u32::from_be_bytes(art.png[20..24].try_into().unwrap()));
                if artwork_pixels > 8 * 1024 * 1024 {
                    return Err(
                        "Document artwork limit: 8 megapixels across both themes (32 MiB decoded)"
                            .into(),
                    );
                }
            }
        }
        if c.controls
            .values()
            .any(|e| !e.target.is_route() || !e.also.is_empty())
        {
            return Err(format!("Composite {id}: control must bind a parameter"));
        }
    }
    Ok(())
}

fn unique_set<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BTreeSet<ModuleId>, D::Error> {
    let ids = Vec::<ModuleId>::deserialize(d)?;
    let count = ids.len();
    let set: BTreeSet<_> = ids.into_iter().collect();
    if set.len() != count {
        return Err(serde::de::Error::custom("Duplicate member identity"));
    }
    Ok(set)
}
