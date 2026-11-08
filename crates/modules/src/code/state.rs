//! Saved settings of a code module (D12; `docs/sdk/ABI.md`, "Saved settings").
//!
//! The module owns its settings format. The host moves bytes through the transfer buffer the
//! module declares in `layout_ext`:
//!
//! - `state_save() -> len`: the module wrote `len` bytes there; `-1` means they did not fit.
//! - `state_load(len, version) -> 1|0`: the host put `len` bytes there, saved under `version`.
//! - `migrate(len, old_version) -> new_len|-1` (optional): old bytes in, bytes for the current
//!   version out.
//!
//! The host never truncates and never chunks. Whatever goes wrong with a module's settings, the
//! instrument keeps playing: a save that fails keeps the previous settings and warns; a load that
//! fails (or a `migrate` that traps) leaves the module at its defaults and the caller keeps the
//! old bytes in the song.

use super::engine::{Fault, LoadError, WasmInstance};
use super::CodeGroup;
use super::CodeModule;

/// A module's settings and the version of the layout they were written in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedState {
    pub state_version: u32,
    pub bytes: Vec<u8>,
}

impl SavedState {
    /// The form kept in a `.kabl` (`Embedded::keep_state`): the version as four little-endian
    /// bytes, then the module's bytes exactly as it wrote them.
    pub fn to_kept(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(4 + self.bytes.len());
        v.extend_from_slice(&self.state_version.to_le_bytes());
        v.extend_from_slice(&self.bytes);
        v
    }

    pub fn from_kept(kept: &[u8]) -> Option<SavedState> {
        let (v, bytes) = kept.split_first_chunk::<4>()?;
        Some(SavedState {
            state_version: u32::from_le_bytes(*v),
            bytes: bytes.to_vec(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateError {
    /// `state_save` returned -1: the settings did not fit the module's buffer.
    Overflow { cap: usize },
    /// `state_save` claimed more bytes than its buffer holds.
    BadLength { len: i32, cap: usize },
    /// The call trapped or ran out of fuel; the instance must be dropped.
    Fault(Fault),
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StateError::Overflow { cap } => write!(
                f,
                "the module's settings do not fit its {cap}-byte buffer; the previous settings are kept"
            ),
            StateError::BadLength { len, cap } => write!(
                f,
                "the module reported {len} bytes of settings in a {cap}-byte buffer; the previous settings are kept"
            ),
            StateError::Fault(x) => write!(
                f,
                "the module {} while saving its settings; the previous settings are kept",
                x.label()
            ),
        }
    }
}

/// Asks the module for its settings. `Ok(None)`: it has none to save (no state exports).
pub fn save(inst: &mut dyn WasmInstance) -> Result<Option<SavedState>, StateError> {
    let Some(x) = inst.state_xfer() else {
        return Ok(None);
    };
    let len = inst.call_state_save().map_err(StateError::Fault)?;
    if len < 0 {
        return Err(StateError::Overflow { cap: x.cap });
    }
    if len as usize > x.cap {
        return Err(StateError::BadLength { len, cap: x.cap });
    }
    let bytes = inst.memory()[x.addr..x.addr + len as usize].to_vec();
    Ok(Some(SavedState {
        state_version: x.state_version,
        bytes,
    }))
}

/// [`save`] for a caller that already holds earlier settings: on any failure the earlier ones
/// stay and a warning says why. Settings are never truncated.
pub fn capture(
    inst: &mut dyn WasmInstance,
    previous: Option<SavedState>,
) -> (Option<SavedState>, Option<String>) {
    match save(inst) {
        Ok(Some(s)) => (Some(s), None),
        Ok(None) => (previous, None),
        Err(e) => (previous, Some(e.to_string())),
    }
}

/// What became of saved settings given to a new instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Restored {
    /// The module took them as they were.
    Loaded,
    /// They were written by an older version of the layout; `migrate` brought them forward.
    Migrated { from: u32, to: u32 },
    /// The module starts at its defaults. The caller keeps the old bytes in the song, so a build
    /// that can read them still finds them.
    Defaults { why: String },
}

impl CodeGroup {
    /// A voice lane (control thread) with `saved` loaded into it: directly when the layout
    /// version matches, through the module's `migrate` when `saved` is older, and at defaults
    /// otherwise (newer than the module, no `migrate`, or any failure, trap included).
    pub fn restore(&self, saved: &SavedState) -> Result<(CodeModule, Restored), LoadError> {
        let mut inst = self.build_one()?;
        // A fault (a trap) leaves the instance unusable, since the module's own guards stay
        // taken: start over from a fresh one.
        let (inst, outcome) = match restore_into(&mut inst, saved) {
            Ok(outcome) => (inst, outcome),
            Err(why) => (self.build_one()?, Restored::Defaults { why }),
        };
        Ok((self.voice_from(inst), outcome))
    }
}

/// `Err(why)` after a fault; every other failure leaves the instance clean at its defaults.
fn restore_into(inst: &mut Box<dyn WasmInstance>, saved: &SavedState) -> Result<Restored, String> {
    let defaults = |why: String| Ok(Restored::Defaults { why });
    let Some(x) = inst.state_xfer() else {
        return defaults("the module has no saved settings".into());
    };
    if saved.state_version > x.state_version {
        return defaults(format!(
            "the settings come from a newer version of the module (layout {}, this is {})",
            saved.state_version, x.state_version
        ));
    }
    let mut bytes = saved.bytes.clone();
    let migrated = saved.state_version != x.state_version;
    if migrated {
        if !inst.has_migrate() {
            return defaults(format!(
                "the module changed its settings layout ({} to {}) and has no migrate",
                saved.state_version, x.state_version
            ));
        }
        if bytes.len() > x.cap {
            return defaults(format!(
                "the old settings ({} bytes) exceed the buffer",
                bytes.len()
            ));
        }
        inst.memory()[x.addr..x.addr + bytes.len()].copy_from_slice(&bytes);
        match inst.call_migrate(bytes.len() as u32, saved.state_version) {
            Ok(n) if (0..=x.cap as i32).contains(&n) => {
                bytes = inst.memory()[x.addr..x.addr + n as usize].to_vec();
            }
            Ok(_) => return defaults("migrate could not convert the settings".into()),
            Err(f) => return Err(format!("migrate {}", f.label())),
        }
    }
    if bytes.len() > x.cap {
        return defaults(format!(
            "the settings ({} bytes) exceed the buffer",
            bytes.len()
        ));
    }
    inst.memory()[x.addr..x.addr + bytes.len()].copy_from_slice(&bytes);
    match inst.call_state_load(bytes.len() as u32, x.state_version) {
        Ok(1) if migrated => Ok(Restored::Migrated {
            from: saved.state_version,
            to: x.state_version,
        }),
        Ok(1) => Ok(Restored::Loaded),
        Ok(_) => defaults("the module did not accept the settings".into()),
        Err(f) => Err(format!("state_load {}", f.label())),
    }
}
