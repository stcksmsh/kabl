//! The wasmi implementation of [`WasmEngine`] (D11 decision 1; ABI v0, decision 8).
//!
//! - `CompilationMode::Eager`: every function is translated at `compile`, never on first call
//!   (which would be on the audio thread). Compile on a background thread with
//!   [`WasmiEngine::compile_async`].
//! - Imports: none. A module that imports anything (WASI included) is rejected at load; the
//!   kabl ABI is exports only ([`ALLOWED_IMPORTS`] is empty by design).
//! - Memory: `StoreLimits` caps an instance's memory (4 MiB by default). After `init` the memory
//!   is frozen: `memory.grow` during `process` traps and is reported as `MemoryOverCap`.
//! - Time: fuel (`consume_fuel`) bounds a call; the caller sets it before every block.
//! - Metadata: a custom section named `kabl.meta`, line based text (see [`parse_meta`]).

use super::abi::{Layout, MAX_BLOB_BYTES};
use super::engine::{CompiledModule, Fault, InstanceConfig, LoadError, WasmEngine, WasmInstance};
use super::meta::{CodeMeta, CodeParam, CodePort};
use crate::info::{Category, PortDirection, PortType, Rate, Taper};
use std::sync::Arc;
use std::thread::JoinHandle;
use wasmi::{
    CompilationMode, Config, Engine, Linker, Memory, Module, ResourceLimiter, Store, StoreLimits,
    StoreLimitsBuilder, TrapCode, TypedFunc, TypedResumableCall,
};

/// Imports a code module may declare, as `(module, name)`. Empty: ABI v0 is exports only.
pub const ALLOWED_IMPORTS: &[(&str, &str)] = &[];
/// The custom section carrying the module's metadata.
pub const META_SECTION: &str = "kabl.meta";
/// Fuel for `init` and the start function (once, off the audio thread).
const INIT_FUEL: u64 = 50_000_000;
const REQUIRED_EXPORTS: [&str; 4] = ["init", "process", "reset", "memory"];

pub struct WasmiEngine {
    engine: Engine,
}

impl Default for WasmiEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmiEngine {
    pub fn new() -> WasmiEngine {
        let mut c = Config::default();
        c.compilation_mode(CompilationMode::Eager);
        c.consume_fuel(true);
        WasmiEngine {
            engine: Engine::new(&c),
        }
    }

    pub(crate) fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Compiles on a background thread (the audio thread never compiles).
    pub fn compile_async(
        self: &Arc<Self>,
        wasm: Vec<u8>,
    ) -> JoinHandle<Result<Arc<dyn CompiledModule>, LoadError>> {
        let e = Arc::clone(self);
        std::thread::spawn(move || e.compile(&wasm))
    }
}

impl WasmEngine for WasmiEngine {
    fn compile(&self, wasm: &[u8]) -> Result<Arc<dyn CompiledModule>, LoadError> {
        if wasm.len() > MAX_BLOB_BYTES {
            return Err(LoadError::TooLarge {
                bytes: wasm.len(),
                cap: MAX_BLOB_BYTES,
            });
        }
        let module =
            Module::new(&self.engine, wasm).map_err(|e| LoadError::Invalid(e.to_string()))?;
        for i in module.imports() {
            if !ALLOWED_IMPORTS.contains(&(i.module(), i.name())) {
                return Err(LoadError::ForbiddenImport(format!(
                    "{}.{}",
                    i.module(),
                    i.name()
                )));
            }
        }
        for want in REQUIRED_EXPORTS {
            if !module.exports().any(|e| e.name() == want) {
                return Err(LoadError::Invalid(format!("missing export `{want}`")));
            }
        }
        let section = module
            .custom_sections()
            .find(|s| s.name() == META_SECTION)
            .ok_or_else(|| LoadError::Invalid(format!("missing `{META_SECTION}` section")))?;
        let text = std::str::from_utf8(section.data())
            .map_err(|_| LoadError::Invalid("metadata is not UTF-8".into()))?;
        let abi = abi_of(text).map_err(LoadError::Invalid)?;
        if abi != super::abi::ABI_VERSION {
            return Err(LoadError::AbiMismatch {
                module: abi,
                host: super::abi::ABI_VERSION,
            });
        }
        let meta = parse_meta(text).map_err(LoadError::Invalid)?;
        meta.validate().map_err(LoadError::Invalid)?;
        Ok(Arc::new(WasmiCompiled {
            engine: self.engine.clone(),
            module,
            meta,
        }))
    }
}

struct WasmiCompiled {
    engine: Engine,
    module: Module,
    meta: CodeMeta,
}

/// Per-store host state: the memory cap, and the stricter limits that apply once `init` is done
/// (no growth at all; a denied `memory.grow` traps with `GrowthOperationLimited`).
struct Host {
    limits: StoreLimits,
    frozen_limits: StoreLimits,
    frozen: bool,
}

impl CompiledModule for WasmiCompiled {
    fn meta(&self) -> &CodeMeta {
        &self.meta
    }

    fn instantiate(&self, cfg: &InstanceConfig) -> Result<Box<dyn WasmInstance>, LoadError> {
        // Declared minimum memory over the cap is refused before anything is allocated.
        if let Some(mt) = self
            .module
            .exports()
            .find(|e| e.name() == "memory")
            .and_then(|e| e.ty().memory().cloned())
        {
            let bytes = (mt.minimum() as usize).saturating_mul(65536);
            if bytes > cfg.memory_cap {
                return Err(LoadError::MemoryOverCap {
                    bytes,
                    cap: cfg.memory_cap,
                });
            }
        }
        let limits = StoreLimitsBuilder::new()
            .memory_size(cfg.memory_cap)
            .memories(1)
            .tables(4)
            .table_elements(1024)
            .instances(1)
            .build();
        let frozen_limits = StoreLimitsBuilder::new()
            .memory_size(0)
            .trap_on_grow_failure(true)
            .build();
        let mut store = Store::new(
            &self.engine,
            Host {
                limits,
                frozen_limits,
                frozen: false,
            },
        );
        store.limiter(|h| -> &mut dyn ResourceLimiter {
            if h.frozen {
                &mut h.frozen_limits
            } else {
                &mut h.limits
            }
        });
        store
            .set_fuel(INIT_FUEL)
            .map_err(|e| LoadError::Invalid(e.to_string()))?;
        let linker = Linker::<Host>::new(&self.engine);
        let inst = linker
            .instantiate_and_start(&mut store, &self.module)
            .map_err(|e| LoadError::Invalid(e.to_string()))?;
        let bad = |what: &str, e: wasmi::Error| LoadError::Invalid(format!("export `{what}`: {e}"));
        let init: TypedFunc<(f32, i32), i32> = inst
            .get_typed_func(&store, "init")
            .map_err(|e| bad("init", e))?;
        let process: TypedFunc<i32, ()> = inst
            .get_typed_func(&store, "process")
            .map_err(|e| bad("process", e))?;
        let reset: TypedFunc<(), ()> = inst
            .get_typed_func(&store, "reset")
            .map_err(|e| bad("reset", e))?;
        let memory = inst
            .get_memory(&store, "memory")
            .ok_or_else(|| LoadError::Invalid("`memory` is not a memory".into()))?;
        let at = init
            .call(&mut store, (cfg.sample_rate, cfg.max_block as i32))
            .map_err(|e| LoadError::Invalid(format!("init failed: {e}")))? as u32
            as usize;
        let mem = memory.data(&store);
        if mem.len() > cfg.memory_cap {
            return Err(LoadError::MemoryOverCap {
                bytes: mem.len(),
                cap: cfg.memory_cap,
            });
        }
        let record = mem
            .get(at..at + 36)
            .ok_or_else(|| LoadError::Layout("init returned an address outside memory".into()))?;
        let mut w = [0u32; 9];
        let (words, _) = record.as_chunks::<4>();
        for (k, c) in words.iter().enumerate() {
            w[k] = u32::from_le_bytes(*c);
        }
        let layout = Layout::from_words(w);
        // Declared memory is final from here: growing it while running is a crash.
        store.data_mut().frozen = true;
        let mut inst = WasmiInstance {
            store,
            memory,
            process,
            reset,
            layout,
            fuel_set: 0,
        };
        inst.prime(cfg.max_block)?;
        Ok(Box::new(inst))
    }
}

pub struct WasmiInstance {
    store: Store<Host>,
    memory: Memory,
    process: TypedFunc<i32, ()>,
    reset: TypedFunc<(), ()>,
    layout: Layout,
    fuel_set: u64,
}

impl WasmiInstance {
    /// One throwaway block then `reset`, run here (control/background thread) so wasmi sizes its
    /// value stack and call stack now, not on the first block on the audio thread.
    fn prime(&mut self, max_block: u32) -> Result<(), LoadError> {
        let frames = max_block.min(self.layout.max_block) as i32;
        let _ = self.store.set_fuel(INIT_FUEL);
        self.process
            .call(&mut self.store, frames)
            .map_err(|e| LoadError::Invalid(format!("first block failed: {e}")))?;
        let _ = self.store.set_fuel(INIT_FUEL);
        self.reset
            .call(&mut self.store, ())
            .map_err(|e| LoadError::Invalid(format!("reset failed: {e}")))
    }

    fn classify(e: &wasmi::Error) -> Fault {
        match e.as_trap_code() {
            Some(TrapCode::OutOfFuel) => Fault::OutOfFuel,
            Some(TrapCode::GrowthOperationLimited) => Fault::MemoryOverCap,
            _ => Fault::Trap,
        }
    }
}

impl WasmInstance for WasmiInstance {
    fn layout(&self) -> Layout {
        self.layout
    }

    fn memory(&mut self) -> &mut [u8] {
        self.memory.data_mut(&mut self.store)
    }

    fn set_fuel(&mut self, fuel: u64) {
        self.fuel_set = fuel;
        let _ = self.store.set_fuel(fuel);
    }

    fn call_block(&mut self, frames: u32) -> Result<u64, Fault> {
        // `call_resumable` reports running out of fuel as a plain return value, where `call` would
        // build an allocating error. The handle is dropped at once (abandoning the block); that
        // hands wasmi's stack back to the engine's cache without allocating. A real trap still
        // builds an error: one allocation, after which the module is disabled.
        match self.process.call_resumable(&mut self.store, frames as i32) {
            Ok(TypedResumableCall::Finished(())) => Ok(self
                .fuel_set
                .saturating_sub(self.store.get_fuel().unwrap_or(0))),
            Ok(TypedResumableCall::OutOfFuel(_)) => Err(Fault::OutOfFuel),
            // No host functions exist (all imports are refused), so this cannot happen.
            Ok(TypedResumableCall::HostTrap(_)) => Err(Fault::Trap),
            Err(e) => Err(Self::classify(&e)),
        }
    }

    /// Same path as `call_block`: the caller sets the fuel first (the module's per-call budget),
    /// out-of-fuel is a plain return value (no allocation), a trap builds one error.
    fn reset(&mut self) -> Result<(), Fault> {
        match self.reset.call_resumable(&mut self.store, ()) {
            Ok(TypedResumableCall::Finished(())) => Ok(()),
            Ok(TypedResumableCall::OutOfFuel(_)) => Err(Fault::OutOfFuel),
            Ok(TypedResumableCall::HostTrap(_)) => Err(Fault::Trap),
            Err(e) => Err(Self::classify(&e)),
        }
    }
}

/// The ABI major a module was built for: its `abi N` meta line, or the host's own when absent.
fn abi_of(text: &str) -> Result<u32, String> {
    match text.lines().map(str::trim).find(|l| l.starts_with("abi ")) {
        Some(l) => l[4..]
            .trim()
            .parse()
            .map_err(|_| "bad `abi` line".to_owned()),
        None => Ok(super::abi::ABI_VERSION),
    }
}

/// Parses the `kabl.meta` text. One item per line; `#` starts a comment line.
///
/// ```text
/// kind demo.gain          module id stored in patches
/// version 1
/// name Gain               rest of line
/// explain Scales it.      rest of line
/// category utility        source|filter|modulator|sequencer|utility|effect
/// rate voice              voice|global
/// width 4                 panel width in rack units
/// port in audio audio_in 1      in|out, audio|cv|unipolar|gate|pitch, name, width 1 or 2
/// param gain 0 4 1 linear 5 x   name min max default linear|exp|stepped|cubic smoothing_ms [unit]
/// advanced gain
/// ```
pub fn parse_meta(text: &str) -> Result<CodeMeta, String> {
    let mut m = CodeMeta {
        kind: String::new(),
        version: 0,
        name: String::new(),
        explain: String::new(),
        category: Category::Utility,
        rate: Rate::Voice,
        ports: vec![],
        params: vec![],
        advanced: vec![],
        width_units: 4,
    };
    let num = |s: &str, what: &str| {
        s.parse::<f32>()
            .map_err(|_| format!("bad number `{s}` for {what}"))
    };
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, rest) = line.split_once(' ').unwrap_or((line, ""));
        let rest = rest.trim();
        let f: Vec<&str> = rest.split_whitespace().collect();
        let err = |m: &str| format!("meta line {}: {m}", n + 1);
        match key {
            // Checked by `abi_of` at load; accepted here so the key is not "unknown".
            "abi" => {}
            "kind" => m.kind = rest.to_owned(),
            "version" => m.version = rest.parse().map_err(|_| err("bad version"))?,
            "name" => m.name = rest.to_owned(),
            "explain" => m.explain = rest.to_owned(),
            "category" => {
                m.category = match rest {
                    "source" => Category::Source,
                    "filter" => Category::Filter,
                    "modulator" => Category::Modulator,
                    "sequencer" => Category::Sequencer,
                    "utility" => Category::Utility,
                    "effect" => Category::Effect,
                    _ => return Err(err("unknown category")),
                }
            }
            "rate" => {
                m.rate = match rest {
                    "voice" => Rate::Voice,
                    "global" => Rate::Global,
                    _ => return Err(err("rate must be voice or global")),
                }
            }
            "width" => m.width_units = rest.parse().map_err(|_| err("bad width"))?,
            "port" => {
                let [dir, ty, name, width] = f[..] else {
                    return Err(err("port needs: in|out type name width"));
                };
                m.ports.push(CodePort {
                    name: name.to_owned(),
                    port_type: match ty {
                        "audio" => PortType::Audio,
                        "cv" => PortType::Cv,
                        "unipolar" => PortType::UnipolarCv,
                        "gate" => PortType::Gate,
                        "pitch" => PortType::Pitch,
                        _ => return Err(err("unknown port type")),
                    },
                    direction: match dir {
                        "in" => PortDirection::Input,
                        "out" => PortDirection::Output,
                        _ => return Err(err("port direction must be in or out")),
                    },
                    width: width.parse().map_err(|_| err("bad port width"))?,
                });
            }
            "param" => {
                if f.len() < 6 {
                    return Err(err(
                        "param needs: name min max default taper smoothing_ms [unit]",
                    ));
                }
                m.params.push(CodeParam {
                    name: f[0].to_owned(),
                    min: num(f[1], "min").map_err(|e| err(&e))?,
                    max: num(f[2], "max").map_err(|e| err(&e))?,
                    default: num(f[3], "default").map_err(|e| err(&e))?,
                    taper: match f[4] {
                        "linear" => Taper::Linear,
                        "exp" => Taper::Exponential,
                        "stepped" => Taper::Stepped,
                        "cubic" => Taper::Cubic,
                        _ => return Err(err("unknown taper")),
                    },
                    smoothing_ms: num(f[5], "smoothing").map_err(|e| err(&e))?,
                    unit: f[6..].join(" "),
                });
            }
            "advanced" => m.advanced.extend(f.iter().map(|s| s.to_string())),
            _ => return Err(err("unknown key")),
        }
    }
    if m.version == 0 {
        return Err("meta: `version` is required and starts at 1".into());
    }
    Ok(m)
}
