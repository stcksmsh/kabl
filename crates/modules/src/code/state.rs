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
//!
//! Two ways in. [`save`], [`capture`] and [`CodeGroup::restore`] work on an instance the caller
//! owns (control thread). A running graph's instance is owned by the audio thread, so there the
//! settings travel through a [`StateCell`]: the control side asks, the audio thread does the wasm
//! call at the start of its next block and leaves the bytes in the cell.

use super::engine::{Fault, LoadError, WasmInstance};
use super::CodeGroup;
use super::CodeModule;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering::*};
use std::sync::Mutex;

/// Fuel for a state call on the control thread (the same allowance `init` gets).
pub const STATE_FUEL: u64 = 50_000_000;

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
    inst.set_fuel(STATE_FUEL);
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

/// Saved settings turned into bytes for the module's current layout version.
enum Conv {
    Ready {
        bytes: Vec<u8>,
        migrated: bool,
    },
    /// Cannot be used; the module stays at its defaults. The instance is still clean.
    Defaults(String),
    /// A call trapped: the instance is unusable.
    Fault(String),
}

fn to_current(inst: &mut dyn WasmInstance, saved: &SavedState) -> Conv {
    let Some(x) = inst.state_xfer() else {
        return Conv::Defaults("the module has no saved settings".into());
    };
    if saved.state_version > x.state_version {
        return Conv::Defaults(format!(
            "the settings come from a newer version of the module (layout {}, this is {})",
            saved.state_version, x.state_version
        ));
    }
    let mut bytes = saved.bytes.clone();
    let migrated = saved.state_version != x.state_version;
    if migrated {
        if !inst.has_migrate() {
            return Conv::Defaults(format!(
                "the module changed its settings layout ({} to {}) and has no migrate",
                saved.state_version, x.state_version
            ));
        }
        if bytes.len() > x.cap {
            return Conv::Defaults(format!(
                "the old settings ({} bytes) exceed the buffer",
                bytes.len()
            ));
        }
        inst.memory()[x.addr..x.addr + bytes.len()].copy_from_slice(&bytes);
        inst.set_fuel(STATE_FUEL);
        match inst.call_migrate(bytes.len() as u32, saved.state_version) {
            Ok(n) if (0..=x.cap as i32).contains(&n) => {
                bytes = inst.memory()[x.addr..x.addr + n as usize].to_vec();
            }
            Ok(_) => return Conv::Defaults("migrate could not convert the settings".into()),
            Err(f) => return Conv::Fault(format!("migrate {}", f.label())),
        }
    }
    if bytes.len() > x.cap {
        return Conv::Defaults(format!(
            "the settings ({} bytes) exceed the buffer",
            bytes.len()
        ));
    }
    Conv::Ready { bytes, migrated }
}

impl CodeGroup {
    /// A voice lane (control thread) with `saved` loaded into it: directly when the layout
    /// version matches, through the module's `migrate` when `saved` is older, and at defaults
    /// otherwise (newer than the module, no `migrate`, or any failure, trap included).
    pub fn restore(&self, saved: &SavedState) -> Result<(CodeModule, Restored), LoadError> {
        let mut inst = self.build_one()?;
        // A fault (a trap) leaves the instance unusable, since the module's own guards stay
        // taken: start over from a fresh one.
        let outcome = match to_current(&mut *inst, saved) {
            Conv::Defaults(why) => Restored::Defaults { why },
            Conv::Fault(why) => {
                inst = self.build_one()?;
                Restored::Defaults { why }
            }
            Conv::Ready { bytes, migrated } => {
                let x = inst.state_xfer().expect("checked by to_current");
                inst.memory()[x.addr..x.addr + bytes.len()].copy_from_slice(&bytes);
                inst.set_fuel(STATE_FUEL);
                match inst.call_state_load(bytes.len() as u32, x.state_version) {
                    Ok(1) if migrated => Restored::Migrated {
                        from: saved.state_version,
                        to: x.state_version,
                    },
                    Ok(1) => Restored::Loaded,
                    Ok(_) => Restored::Defaults {
                        why: "the module did not accept the settings".into(),
                    },
                    Err(f) => {
                        inst = self.build_one()?;
                        Restored::Defaults {
                            why: format!("state_load {}", f.label()),
                        }
                    }
                }
            }
        };
        Ok((self.voice_from(inst), outcome))
    }

    /// `saved` as bytes the running module can `state_load` (migrated if the layout is older),
    /// found on a scratch instance (control thread). `Err(why)`: it starts at its defaults.
    pub fn settings_for_load(&self, saved: &SavedState) -> Result<SavedState, String> {
        let mut inst = self.build_one().map_err(|e| e.to_string())?;
        match to_current(&mut *inst, saved) {
            Conv::Ready { bytes, .. } => Ok(SavedState {
                state_version: inst
                    .state_xfer()
                    .map_or(saved.state_version, |x| x.state_version),
                bytes,
            }),
            Conv::Defaults(why) | Conv::Fault(why) => Err(why),
        }
    }
}

// ---- a running instance's settings

/// Why the audio thread could not save or load a module's settings (a code, not a string: the
/// audio thread allocates nothing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateFail {
    NoState = 1,
    Rejected,
    Fuel,
    Overflow,
}

impl StateFail {
    fn from_code(c: u8) -> Option<StateFail> {
        [
            StateFail::NoState,
            StateFail::Rejected,
            StateFail::Fuel,
            StateFail::Overflow,
        ]
        .get(usize::from(c).checked_sub(1)?)
        .copied()
    }

    pub fn label(self) -> &'static str {
        match self {
            StateFail::NoState => "has no saved settings",
            StateFail::Rejected => "did not accept its saved settings",
            StateFail::Fuel => "ran out of time handling its settings",
            StateFail::Overflow => "could not fit its settings in its buffer",
        }
    }
}

struct CellData {
    /// Last save, `out[..out_len]`, valid while `ready`.
    out: Vec<u8>,
    out_len: usize,
    out_version: u32,
    ready: bool,
    /// Settings waiting to be loaded, `inp[..inp_len]`.
    inp: Vec<u8>,
    inp_len: usize,
    inp_version: u32,
}

/// Hand-over point for the settings of one running code-module instance. The control thread
/// asks and collects; the audio thread, in [`CodeModule`]'s `process`, does the wasm call at the
/// start of its next block. Both buffers are allocated up front, so the audio side never
/// allocates; if the control side holds the lock the request waits for the next block.
pub struct StateCell {
    save_req: AtomicBool,
    load_req: AtomicBool,
    /// The module refused the settings it was given: its state is not the song's, so it must
    /// not be saved over them.
    rejected: AtomicBool,
    fail: AtomicU8,
    data: Mutex<CellData>,
}

impl StateCell {
    /// A cell for a module whose transfer buffer is `cap` bytes.
    pub fn new(cap: usize) -> StateCell {
        StateCell {
            save_req: AtomicBool::new(false),
            load_req: AtomicBool::new(false),
            rejected: AtomicBool::new(false),
            fail: AtomicU8::new(0),
            data: Mutex::new(CellData {
                out: vec![0; cap],
                out_len: 0,
                out_version: 0,
                ready: false,
                inp: vec![0; cap],
                inp_len: 0,
                inp_version: 0,
            }),
        }
    }

    /// Queues settings for the module to load at the start of its next block.
    pub fn queue_load(&self, s: &SavedState) -> bool {
        let mut d = self.data.lock().unwrap_or_else(|e| e.into_inner());
        if s.bytes.len() > d.inp.len() {
            return false;
        }
        d.inp[..s.bytes.len()].copy_from_slice(&s.bytes);
        d.inp_len = s.bytes.len();
        d.inp_version = s.state_version;
        self.load_req.store(true, Release);
        true
    }

    /// Asks the module to save its settings at the start of its next block.
    pub fn request_save(&self) {
        self.save_req.store(true, Release);
    }

    /// The settings the module saved since the request, once it has.
    pub fn take_saved(&self) -> Option<SavedState> {
        let mut d = self.data.lock().unwrap_or_else(|e| e.into_inner());
        if !d.ready {
            return None;
        }
        d.ready = false;
        Some(SavedState {
            state_version: d.out_version,
            bytes: d.out[..d.out_len].to_vec(),
        })
    }

    /// Why the last save or load failed, once.
    pub fn take_fail(&self) -> Option<StateFail> {
        StateFail::from_code(self.fail.swap(0, AcqRel))
    }

    /// A save request the audio thread has not served yet.
    pub fn save_pending(&self) -> bool {
        self.save_req.load(Acquire)
    }

    /// Anything waiting for the audio thread.
    pub fn pending(&self) -> bool {
        self.save_req.load(Acquire) || self.load_req.load(Acquire)
    }

    /// Audio thread, at the top of a block. Runs under `fuel`; `Err` is a trap (the module dies
    /// like any other fault). No allocation.
    pub(super) fn service(&self, inst: &mut dyn WasmInstance, fuel: u64) -> Result<(), Fault> {
        if !self.pending() {
            return Ok(());
        }
        let Ok(mut d) = self.data.try_lock() else {
            return Ok(()); // the control side is busy with the cell: next block
        };
        let Some(x) = inst.state_xfer() else {
            self.load_req.store(false, Release);
            self.save_req.store(false, Release);
            self.fail.store(StateFail::NoState as u8, Release);
            return Ok(());
        };
        if self.load_req.swap(false, AcqRel) {
            let n = d.inp_len.min(x.cap);
            inst.memory()[x.addr..x.addr + n].copy_from_slice(&d.inp[..n]);
            inst.set_fuel(fuel);
            match inst.call_state_load(n as u32, d.inp_version) {
                Ok(1) => {}
                Ok(_) => {
                    self.rejected.store(true, Release);
                    self.fail.store(StateFail::Rejected as u8, Release);
                }
                Err(Fault::OutOfFuel) => {
                    self.rejected.store(true, Release);
                    self.fail.store(StateFail::Fuel as u8, Release);
                }
                Err(f) => return Err(f),
            }
        }
        if self.save_req.swap(false, AcqRel) {
            if self.rejected.load(Acquire) {
                return Ok(()); // keep the song's bytes; the load failure was already noted
            }
            inst.set_fuel(fuel);
            match inst.call_state_save() {
                Ok(n) if n >= 0 && n as usize <= x.cap && n as usize <= d.out.len() => {
                    let n = n as usize;
                    let (mem, out) = (&inst.memory()[x.addr..x.addr + n], &mut d.out[..n]);
                    out.copy_from_slice(mem);
                    d.out_len = n;
                    d.out_version = x.state_version;
                    d.ready = true;
                }
                Ok(_) => self.fail.store(StateFail::Overflow as u8, Release),
                Err(Fault::OutOfFuel) => self.fail.store(StateFail::Fuel as u8, Release),
                Err(f) => return Err(f),
            }
        }
        Ok(())
    }
}
