//! Composite ownership and public bindings. DSP remains the document's flat leaf graph.
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
}

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

pub fn validate(p: &PatchState) -> Result<(), String> {
    if p.composites.len() > 128 {
        return Err("Composite count limit (128)".into());
    }
    let mut owned = BTreeSet::new();
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
            if !public.insert(key)
                || key >= c.next_interface
                || e.label.is_empty()
                || e.label.len() > 128
                || !members.contains(&e.target.module_id())
            {
                return Err(format!("Composite {id}: invalid public binding {key}"));
            }
        }
        if c.controls
            .values()
            .any(|e| !matches!(e.target, PortRef::Param { .. }))
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
