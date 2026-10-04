//! Versioned complete sound state: graph, parameter bases, routing and labels, never keys or
//! device/session machinery. Built-ins have no external audio/code dependencies in D07.
use kabl_core::PatchState;
use serde::{Deserialize, Serialize};

// Keep the D07 patch bound unchanged; reserve bounded space for the new envelope.
pub const MAX_STATE_BYTES: usize = 2 * 1024 * 1024 + 8192;
const MAX_PATCH_BYTES: usize = 2 * 1024 * 1024 - 1024;
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoundState {
    pub version: u32,
    pub patch: PatchState,
    pub output_gain: f32,
    #[serde(default)]
    pub lanes: [crate::automation::Lane; crate::automation::SLOTS],
    #[serde(default)]
    pub slot_values: [f32; crate::automation::SLOTS],
    #[serde(default)]
    pub host_clock: bool,
}
impl SoundState {
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > MAX_STATE_BYTES {
            return Err("State size limit".into());
        }
        let state: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        state.validate()?;
        Ok(state)
    }
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.version, 1 | 2) {
            return Err("Unsupported sound state version".into());
        }
        if !self.output_gain.is_finite() || !(0.0..=1.0).contains(&self.output_gain) {
            return Err("Invalid output gain".into());
        }
        if self.version == 1
            && (self.host_clock
                || self.lanes.iter().any(|l| l.target.is_some() || l.retired)
                || self.slot_values.iter().any(|&v| v != 0.0))
        {
            return Err("Version 1 cannot contain D08 settings".into());
        }
        for (i, lane) in self.lanes.iter().enumerate() {
            if !self.slot_values[i].is_finite() || !(0.0..=1.0).contains(&self.slot_values[i]) {
                return Err("Invalid automation value".into());
            }
            if let Some(target) = &lane.target {
                if lane.retired
                    || crate::automation::resolve(&self.patch, target).is_none()
                    || self.lanes[..i]
                        .iter()
                        .any(|l| l.target.as_ref() == Some(target))
                {
                    return Err("Invalid automation mapping".into());
                }
            }
        }
        validate_patch(&self.patch)
    }
}
pub fn validate_patch(p: &PatchState) -> Result<(), String> {
    if p.modules.len() > 128 || p.cables.len() > 512 || p.labels.len() > 128 {
        return Err("Patch graph limit".into());
    }
    // The editor reserves the successor when seeding IDs. Reject overflow before any mutation.
    if p.modules
        .keys()
        .chain(p.cables.keys())
        .any(|&id| id == u64::MAX)
        || p.labels.keys().any(|id| !p.modules.contains_key(id))
    {
        return Err("Invalid patch identity".into());
    }
    let effects = p
        .modules
        .values()
        .filter(|m| matches!(m.kind.as_str(), "delay" | "reverb" | "chorus"))
        .count();
    if effects > 8 {
        return Err("Patch effect limit (8)".into());
    }
    for m in p.modules.values() {
        if m.kind.len() > 64
            || m.params.len() > 256
            || !m.pos.x.is_finite()
            || !m.pos.y.is_finite()
            || m.params
                .iter()
                .any(|(k, v)| k.len() > 128 || !v.is_finite())
        {
            return Err("Invalid module data".into());
        }
    }
    for c in p.cables.values() {
        if c.params.len() > 32
            || c.steps.len() > 256
            || c.params
                .iter()
                .any(|(k, v)| k.len() > 128 || !v.is_finite())
            || c.steps.iter().any(|v| !v.is_finite())
        {
            return Err("Invalid cable data".into());
        }
    }
    if p.labels.values().any(|labels| {
        labels.len() > 256 || labels.iter().any(|(k, v)| k.len() > 128 || v.len() > 4096)
    }) {
        return Err("Patch label limit".into());
    }
    // The same cap applies to editor/browser documents and project loads. Reserve space
    // for the version/gain envelope so every accepted patch can also be saved.
    if serde_json::to_vec(p)
        .map_err(|error| error.to_string())?
        .len()
        > MAX_PATCH_BYTES
    {
        return Err("Patch serialized size limit".into());
    }
    Ok(())
}
