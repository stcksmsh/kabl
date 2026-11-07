//! Code modules (D11): sandboxed, distributed DSP behind the ordinary `Module` trait.
//!
//! This PR is the seam only: owned metadata, the registry path, the `WasmEngine` trait with a
//! fake test backend, the `CodeModule` shell with its failure rules, the per-voice instance pool
//! and the ABI v0 layout types. The wasmi engine, embedding in `.kabl` and fuel calibration come
//! in later PRs. Built-in modules and their lookup are untouched.
//!
//! Wiring note: the compiler instantiates one module per voice lane through `registry::create`.
//! A [`CodeGroup`] hands out those lanes and gives them one shared [`Health`], so a crash in one
//! voice disables the module everywhere. `registry::create(kind)` on a registered group takes the
//! next pre-built instance; per-node grouping in the compiler is a follow-up.

pub mod abi;
pub mod budget;
pub mod engine;
pub mod fake;
pub mod meta;
pub mod module;
pub mod wasmi_engine;

pub use abi::{Layout, LayoutError, ABI_VERSION, DEFAULT_MEMORY_CAP, MAX_BLOB_BYTES};
pub use budget::{scaled_shares, Budget, Calibration, PER_MODULE_SHARE, TOTAL_SHARE};
pub use engine::{CompiledModule, Fault, InstanceConfig, LoadError, WasmEngine, WasmInstance};
pub use meta::{CodeMeta, CodeParam, CodePort};
pub use module::{in_code_call, CodeModule, Health, FADE_MS, OVERRUN_LIMIT};
pub use wasmi_engine::{WasmiEngine, ALLOWED_IMPORTS, META_SECTION};

use crate::info::{ModuleInfo, PortDirection};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;

/// A compiled module plus its pre-built pool of instances (one per voice) and shared health.
pub struct CodeGroup {
    compiled: Arc<dyn CompiledModule>,
    info: &'static ModuleInfo,
    cfg: InstanceConfig,
    health: Arc<Health>,
    pool: Mutex<Vec<Box<dyn WasmInstance>>>,
}

impl CodeGroup {
    /// `fuel` and `wall_cap` are the per-block budgets (calibration and the 50 % total cap are
    /// applied by the caller via [`Health::set_budget`]).
    pub fn new(
        compiled: Arc<dyn CompiledModule>,
        cfg: InstanceConfig,
        fuel: u64,
        wall_cap: Duration,
    ) -> Result<Arc<CodeGroup>, LoadError> {
        let meta = compiled.meta();
        meta.validate().map_err(LoadError::Invalid)?;
        Ok(Arc::new(CodeGroup {
            info: meta.info(),
            compiled,
            cfg,
            health: Health::new(fuel, wall_cap),
            pool: Mutex::new(Vec::new()),
        }))
    }

    pub fn info(&self) -> &'static ModuleInfo {
        self.info
    }

    pub fn health(&self) -> &Arc<Health> {
        &self.health
    }

    fn build_one(&self) -> Result<Box<dyn WasmInstance>, LoadError> {
        let inst = self.compiled.instantiate(&self.cfg)?;
        let mut inst = inst;
        let layout = inst.layout();
        let mem_len = inst.memory().len();
        if mem_len > self.cfg.memory_cap {
            return Err(LoadError::MemoryOverCap {
                bytes: mem_len,
                cap: self.cfg.memory_cap,
            });
        }
        layout
            .validate(
                mem_len,
                self.info.slots(PortDirection::Input),
                self.info.slots(PortDirection::Output),
                self.info.params.len(),
                self.cfg.max_block as usize,
            )
            .map_err(|e| LoadError::Layout(e.to_string()))?;
        Ok(inst)
    }

    /// Builds `n` instances into the pool on the calling thread (control thread only).
    pub fn fill(&self, n: usize) -> Result<(), LoadError> {
        for _ in 0..n {
            let inst = self.build_one()?;
            self.pool
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(inst);
        }
        Ok(())
    }

    /// Builds `n` instances on a background thread; the audio thread never instantiates.
    pub fn fill_async(self: &Arc<Self>, n: usize) -> JoinHandle<Result<(), LoadError>> {
        let g = Arc::clone(self);
        std::thread::spawn(move || g.fill(n))
    }

    pub fn pooled(&self) -> usize {
        self.pool.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// One voice lane, from the pool; if the pool ran dry (more voices than planned) it builds a
    /// fresh instance here, so call this from the control thread, never the audio thread.
    pub fn voice(&self) -> Result<CodeModule, LoadError> {
        let pooled = self.pool.lock().unwrap_or_else(|e| e.into_inner()).pop();
        let inst = match pooled {
            Some(i) => i,
            None => self.build_one()?,
        };
        Ok(CodeModule::new(
            self.info,
            inst,
            Arc::clone(&self.health),
            &self.cfg,
        ))
    }
}

static REGISTRY: RwLock<BTreeMap<String, Arc<CodeGroup>>> = RwLock::new(BTreeMap::new());

/// Registers a code module under its kind. Refuses a kind that belongs to a built-in.
pub fn register(group: Arc<CodeGroup>) -> Result<(), LoadError> {
    let kind = group.info.kind;
    if crate::registry::all_infos().iter().any(|i| i.kind == kind) {
        return Err(LoadError::Invalid(format!("`{kind}` is a built-in module")));
    }
    REGISTRY
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .insert(kind.to_owned(), group);
    Ok(())
}

pub fn unregister(kind: &str) {
    REGISTRY
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .remove(kind);
}

pub fn group(kind: &str) -> Option<Arc<CodeGroup>> {
    REGISTRY
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .get(kind)
        .cloned()
}

/// A fresh instance of a registered code module (the fallback after the built-in table).
pub fn create(kind: &str) -> Option<Box<dyn crate::module::Module>> {
    let g = group(kind)?;
    g.voice()
        .ok()
        .map(|m| Box::new(m) as Box<dyn crate::module::Module>)
}

pub fn info_for(kind: &str) -> Option<&'static ModuleInfo> {
    group(kind).map(|g| g.info)
}

pub fn all_infos() -> Vec<&'static ModuleInfo> {
    REGISTRY
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .values()
        .map(|g| g.info)
        .collect()
}
