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
pub mod host;
pub mod install;
pub mod meta;
pub mod module;
pub mod package;
pub mod scope;
pub mod standin;
pub mod state;
pub mod wasmi_engine;

pub use abi::{Layout, LayoutError, ABI_VERSION, DEFAULT_MEMORY_CAP, MAX_BLOB_BYTES};
pub use budget::{scaled_shares, Budget, Calibration, PER_MODULE_SHARE, TOTAL_SHARE};
pub use engine::{CompiledModule, Fault, InstanceConfig, LoadError, WasmEngine, WasmInstance};
pub use meta::{CodeMeta, CodeParam, CodePort};
pub use module::{in_code_call, CodeModule, Health, FADE_MS, OVERRUN_LIMIT};
pub use scope::{hash_of, standin_key, CodeHash, CodeScope};
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
    /// The registry key: the blob's hash for a loaded blob.
    key: CodeHash,
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
        let key = hash_of(format!("unhashed:{}", compiled.meta().kind).as_bytes());
        Self::keyed(compiled, cfg, fuel, wall_cap, key)
    }

    /// [`CodeGroup::new`] for a blob with a known hash: songs find the group by it.
    pub fn keyed(
        compiled: Arc<dyn CompiledModule>,
        cfg: InstanceConfig,
        fuel: u64,
        wall_cap: Duration,
        key: CodeHash,
    ) -> Result<Arc<CodeGroup>, LoadError> {
        let meta = compiled.meta();
        meta.validate().map_err(LoadError::Invalid)?;
        Ok(Arc::new(CodeGroup {
            info: meta.info(),
            compiled,
            cfg,
            health: Health::new(fuel, wall_cap),
            pool: Mutex::new(Vec::new()),
            key,
        }))
    }

    pub fn info(&self) -> &'static ModuleInfo {
        self.info
    }

    pub fn key(&self) -> CodeHash {
        self.key
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
        Ok(self.voice_from(inst))
    }

    fn voice_from(&self, inst: Box<dyn WasmInstance>) -> CodeModule {
        CodeModule::new(self.info, inst, Arc::clone(&self.health), &self.cfg)
    }
}

/// Running groups by key (a blob's hash).
static REGISTRY: RwLock<BTreeMap<CodeHash, Arc<CodeGroup>>> = RwLock::new(BTreeMap::new());
/// The key a kind resolves to without a [`CodeScope`]: whatever was registered for it last
/// (a running group or a stand-in).
static LATEST: RwLock<BTreeMap<String, CodeHash>> = RwLock::new(BTreeMap::new());

fn builtin_clash(kind: &str) -> Result<(), LoadError> {
    if crate::registry::all_infos().iter().any(|i| i.kind == kind) {
        return Err(LoadError::Invalid(format!("`{kind}` is a built-in module")));
    }
    Ok(())
}

/// Registers a code module under its key and makes it the latest of its kind. Refuses a kind
/// that belongs to a built-in. Another blob of the same kind keeps running for the songs that
/// use it.
pub fn register(group: Arc<CodeGroup>) -> Result<(), LoadError> {
    install(group, true)
}

/// [`register`], optionally leaving the kind's latest alone (a rebuild of an older blob).
pub(crate) fn install(group: Arc<CodeGroup>, make_latest: bool) -> Result<(), LoadError> {
    let kind = group.info.kind;
    builtin_clash(kind)?;
    let key = group.key;
    REGISTRY
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .insert(key, group);
    if make_latest {
        set_latest(kind, key);
    }
    Ok(())
}

pub(crate) fn set_latest(kind: &str, key: CodeHash) {
    LATEST
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .insert(kind.to_owned(), key);
}

pub(crate) fn latest_key(kind: &str) -> Option<CodeHash> {
    LATEST
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .get(kind)
        .copied()
}

/// Forgets the latest module of `kind` (the running group, if it is one).
pub fn unregister(kind: &str) {
    let key = LATEST
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .remove(kind);
    if let Some(key) = key {
        REGISTRY
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&key);
    }
}

/// The running group with this key.
pub fn group_by_key(key: &CodeHash) -> Option<Arc<CodeGroup>> {
    REGISTRY
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .get(key)
        .cloned()
}

/// The running group `kind` resolves to in `scope` (the song's own blob, else the latest).
pub fn group_in(scope: &CodeScope, kind: &str) -> Option<Arc<CodeGroup>> {
    group_by_key(&scope.key(kind).or_else(|| latest_key(kind))?)
}

pub fn group(kind: &str) -> Option<Arc<CodeGroup>> {
    group_in(&CodeScope::default(), kind)
}

/// A fresh instance of a code module for the song `scope` describes (the fallback after the
/// built-in table): its running group, else its stand-in.
pub fn create_in(scope: &CodeScope, kind: &str) -> Option<Box<dyn crate::module::Module>> {
    let key = scope.key(kind).or_else(|| latest_key(kind))?;
    let Some(g) = group_by_key(&key) else {
        return standin::create(&key);
    };
    match g.voice() {
        Ok(m) => Some(Box::new(m) as Box<dyn crate::module::Module>),
        // Never vanish: a module that cannot make an instance now passes audio through.
        Err(e) => {
            scope.note(format!(
                "code module {kind} could not start ({e}); it passes audio through"
            ));
            Some(Box::new(standin::StandIn::new(g.info())) as Box<dyn crate::module::Module>)
        }
    }
}

/// A fresh instance of a registered code module, the latest of its kind.
pub fn create(kind: &str) -> Option<Box<dyn crate::module::Module>> {
    create_in(&CodeScope::default(), kind)
}

pub fn info_for_in(scope: &CodeScope, kind: &str) -> Option<&'static ModuleInfo> {
    let key = scope.key(kind).or_else(|| latest_key(kind))?;
    group_by_key(&key)
        .map(|g| g.info)
        .or_else(|| standin::info_for(&key))
}

pub fn info_for(kind: &str) -> Option<&'static ModuleInfo> {
    info_for_in(&CodeScope::default(), kind)
}

/// The latest running module of each kind.
pub fn all_infos() -> Vec<&'static ModuleInfo> {
    let latest = LATEST.read().unwrap_or_else(|e| e.into_inner());
    let groups = REGISTRY.read().unwrap_or_else(|e| e.into_inner());
    latest
        .values()
        .filter_map(|k| groups.get(k))
        .map(|g| g.info)
        .collect()
}
