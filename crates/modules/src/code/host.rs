//! The process-wide side of code modules: the one wasmi engine, the one time [`Budget`], and the
//! table of registered code blobs (so a save can embed exactly the code a patch uses).
//!
//! The example modules (`demo.logic`, `demo.bitfilter`) are committed as wasm blobs built from
//! `crates/modules/code-examples` (see its `build.sh`). They are registered like any other
//! blob, through [`register_blob`]; nothing about them is special after that.

use super::{
    Budget, Calibration, CodeGroup, InstanceConfig, LoadError, WasmEngine, WasmiEngine,
    PER_MODULE_SHARE,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

/// An example module: its kind and the committed wasm.
pub struct Example {
    pub kind: &'static str,
    pub wasm: &'static [u8],
}

pub const EXAMPLES: [Example; 2] = [
    Example {
        kind: "demo.logic",
        wasm: include_bytes!("examples/demo_logic.wasm"),
    },
    Example {
        kind: "demo.bitfilter",
        wasm: include_bytes!("examples/demo_bitfilter.wasm"),
    },
];

/// Sample rate and block the default budget assumes until the audio device reports its own
/// (`budget().update(..)`).
const DEFAULT_SAMPLE_RATE: f32 = 48_000.0;
const DEFAULT_BLOCK: u32 = 512;
/// Fuel per microsecond used until calibration runs: below the slowest probe measured in the
/// D11 report (1,327), so budgets stay inside the time cap.
const DEFAULT_FUEL_PER_US: f64 = 1_000.0;
/// Memory cap and block the instances are built for.
const MAX_BLOCK: u32 = 2048;

pub struct Registered {
    pub kind: String,
    pub version: u32,
}

struct Entry {
    version: u32,
    wasm: Arc<[u8]>,
}

fn engine() -> &'static Arc<WasmiEngine> {
    static E: OnceLock<Arc<WasmiEngine>> = OnceLock::new();
    E.get_or_init(|| Arc::new(WasmiEngine::new()))
}

/// The shared time budget; register every group's health with it.
pub fn budget() -> &'static Budget {
    static B: OnceLock<Budget> = OnceLock::new();
    B.get_or_init(|| {
        Budget::new(
            Calibration::fixed(DEFAULT_FUEL_PER_US),
            DEFAULT_SAMPLE_RATE,
            DEFAULT_BLOCK,
        )
    })
}

fn blobs() -> &'static Mutex<BTreeMap<String, Entry>> {
    static T: Mutex<BTreeMap<String, Entry>> = Mutex::new(BTreeMap::new());
    &T
}

/// Compiles `wasm`, registers it under the kind its metadata names and remembers the bytes.
/// Registering the same bytes again is a no-op; different bytes under the same kind replace the
/// earlier code (existing voices keep running their old instances). Control thread only.
pub fn register_blob(wasm: &[u8]) -> Result<Registered, LoadError> {
    let compiled = engine().compile(wasm)?;
    let meta = compiled.meta();
    let (kind, version) = (meta.kind.clone(), meta.version);
    {
        let table = blobs().lock().unwrap_or_else(|e| e.into_inner());
        if table.get(&kind).is_some_and(|e| *e.wasm == *wasm) && super::group(&kind).is_some() {
            return Ok(Registered { kind, version });
        }
    }
    let cfg = InstanceConfig::new(DEFAULT_SAMPLE_RATE, MAX_BLOCK);
    let group = CodeGroup::new(compiled, cfg, 1, std::time::Duration::from_secs(1))?;
    budget().register_with(group.health(), PER_MODULE_SHARE);
    super::unregister(&kind);
    super::register(group)?;
    blobs().lock().unwrap_or_else(|e| e.into_inner()).insert(
        kind.clone(),
        Entry {
            version,
            wasm: Arc::from(wasm),
        },
    );
    Ok(Registered { kind, version })
}

/// Registers both example modules (compiled once per process; later calls return the result).
pub fn register_examples() -> Result<(), LoadError> {
    static DONE: OnceLock<Result<(), LoadError>> = OnceLock::new();
    DONE.get_or_init(|| {
        for e in &EXAMPLES {
            register_blob(e.wasm)?;
        }
        Ok(())
    })
    .clone()
}

/// The registered code for `kind`: `(version, wasm bytes)`.
pub fn blob_for(kind: &str) -> Option<(u32, Arc<[u8]>)> {
    blobs()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(kind)
        .map(|e| (e.version, Arc::clone(&e.wasm)))
}
