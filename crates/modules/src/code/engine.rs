//! The engine seam (D11 decision 1): everything kabl needs from a WebAssembly runtime, so a
//! second runtime can be added later as one change. PR 2 brings the wasmi implementation; the
//! tests use `fake::FakeEngine`.
//!
//! Threading: `compile` and `instantiate` run off the audio thread (control or background
//! thread). The audio thread only calls `set_fuel`, `call_block`, `reset` and reads/writes
//! `memory` on an instance that already exists.

use super::abi::{Layout, DEFAULT_MEMORY_CAP};
use super::meta::CodeMeta;
use std::sync::Arc;

/// What goes wrong while a block runs. All of these end up as a crash except `OutOfFuel`, which
/// is an overrun (8 in a row is a crash too).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// The module trapped (unreachable, out of bounds, divide by zero, `memory.grow` ...).
    Trap,
    /// The block used up its fuel.
    OutOfFuel,
    /// Memory over the per-instance cap.
    MemoryOverCap,
    /// NaN or infinity in an output buffer.
    NonFinite,
    /// Eight consecutive overruns (fuel or wall clock).
    Overrun,
    /// Layout or block size the module cannot serve.
    BadLayout,
}

impl Fault {
    pub fn code(self) -> u8 {
        self as u8 + 1
    }
    pub fn from_code(c: u8) -> Option<Fault> {
        [
            Fault::Trap,
            Fault::OutOfFuel,
            Fault::MemoryOverCap,
            Fault::NonFinite,
            Fault::Overrun,
            Fault::BadLayout,
        ]
        .get(usize::from(c).checked_sub(1)?)
        .copied()
    }
    /// Short text for the face badge and toast.
    pub fn label(self) -> &'static str {
        match self {
            Fault::Trap => "trapped",
            Fault::OutOfFuel => "ran out of time budget",
            Fault::MemoryOverCap => "used too much memory",
            Fault::NonFinite => "produced NaN or infinity",
            Fault::Overrun => "kept overrunning its time budget",
            Fault::BadLayout => "has an unusable buffer layout",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LoadError {
    Invalid(String),
    ForbiddenImport(String),
    TooLarge {
        bytes: usize,
        cap: usize,
    },
    AbiMismatch {
        module: u32,
        host: u32,
    },
    /// The module needs a newer revision of the ABI than this host has.
    AbiMinor {
        module: u32,
        host: u32,
    },
    MemoryOverCap {
        bytes: usize,
        cap: usize,
    },
    Layout(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Invalid(m) => write!(f, "invalid code module: {m}"),
            LoadError::ForbiddenImport(m) => {
                write!(f, "code module imports `{m}`; only kabl's ABI is allowed")
            }
            LoadError::TooLarge { bytes, cap } => {
                write!(f, "code module is {bytes} bytes; the cap is {cap}")
            }
            LoadError::AbiMismatch { module, host } => write!(
                f,
                "code module targets ABI v{module}; this kabl runs v{host}"
            ),
            LoadError::AbiMinor { module, host } => write!(
                f,
                "code module needs ABI revision {module}; this kabl supports up to {host}. \
                 Update kabl, or ask the module's author for a build for this version"
            ),
            LoadError::MemoryOverCap { bytes, cap } => write!(
                f,
                "code module wants {bytes} bytes of memory; the cap is {cap}"
            ),
            LoadError::Layout(m) => write!(f, "code module layout: {m}"),
        }
    }
}

impl std::error::Error for LoadError {}

#[derive(Debug, Clone, Copy)]
pub struct InstanceConfig {
    pub sample_rate: f32,
    pub max_block: u32,
    pub memory_cap: usize,
}

impl InstanceConfig {
    pub fn new(sample_rate: f32, max_block: u32) -> Self {
        InstanceConfig {
            sample_rate,
            max_block,
            memory_cap: DEFAULT_MEMORY_CAP,
        }
    }
}

/// Where a module exchanges its saved settings with the host: a buffer in its own memory,
/// declared in the `layout_ext` table, plus the version of the layout its `state_save` writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateXfer {
    pub addr: usize,
    pub cap: usize,
    pub state_version: u32,
}

pub trait WasmEngine: Send + Sync {
    /// Validates and fully compiles `wasm` (no lazy translation: nothing may compile on first
    /// call). Rejects any import and any ABI mismatch. Blob size is checked by the caller side
    /// too, against `MAX_BLOB_BYTES`.
    fn compile(&self, wasm: &[u8]) -> Result<Arc<dyn CompiledModule>, LoadError>;
}

pub trait CompiledModule: Send + Sync {
    /// Metadata the module declared (the owned form; `CodeMeta::info` leaks it for the registry).
    fn meta(&self) -> &CodeMeta;
    /// A fresh instance with its own linear memory, `init` already run. Control/background
    /// thread only.
    fn instantiate(&self, cfg: &InstanceConfig) -> Result<Box<dyn WasmInstance>, LoadError>;
}

pub trait WasmInstance: Send {
    fn layout(&self) -> Layout;
    /// The instance's linear memory. Fixed size: modules may not grow it while running.
    fn memory(&mut self) -> &mut [u8];
    fn set_fuel(&mut self, fuel: u64);
    /// Runs `process(frames)`. Returns the fuel used.
    fn call_block(&mut self, frames: u32) -> Result<u64, Fault>;
    /// Runs the module's `reset()` under the fuel the caller set with `set_fuel`, exactly like
    /// `call_block` (audio thread, no allocation on out-of-fuel).
    fn reset(&mut self) -> Result<(), Fault>;

    /// The settings transfer buffer, when the module exports the state calls.
    fn state_xfer(&self) -> Option<StateXfer> {
        None
    }
    /// Whether the module exports `migrate`.
    fn has_migrate(&self) -> bool {
        false
    }
    /// `state_save() -> len`: the module writes its settings at the transfer buffer and returns
    /// their length, or -1 when they do not fit. Control thread; a trap leaves the instance
    /// unusable (drop it).
    fn call_state_save(&mut self) -> Result<i32, Fault> {
        Err(Fault::BadLayout)
    }
    /// `state_load(len, version) -> 1|0`: the host has put `len` bytes at the transfer buffer.
    fn call_state_load(&mut self, _len: u32, _version: u32) -> Result<i32, Fault> {
        Err(Fault::BadLayout)
    }
    /// `migrate(len, old_version) -> new_len|-1`: old settings in, new ones out, both at the
    /// transfer buffer.
    fn call_migrate(&mut self, _len: u32, _old_version: u32) -> Result<i32, Fault> {
        Err(Fault::BadLayout)
    }
}
