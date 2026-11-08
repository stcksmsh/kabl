//! The process-wide side of code modules: the one wasmi engine, the one time [`Budget`], and the
//! table of registered code blobs (so a save can embed exactly the code a patch uses).
//!
//! The example modules (`demo.logic`, `demo.bitfilter`) are committed as wasm blobs built from
//! `crates/modules/code-examples` (see its `build.sh`). They are registered like any other
//! blob, through [`register_blob`]; nothing about them is special after that.

use super::standin::Report;
use super::{
    Budget, Calibration, CodeGroup, CodeMeta, InstanceConfig, LoadError, WasmEngine, WasmiEngine,
    PER_MODULE_SHARE,
};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
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
    /// The sample rate the group's instances were built for.
    rate: f32,
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
    let rate = latest().lock().unwrap_or_else(|e| e.into_inner()).0;
    {
        let table = blobs().lock().unwrap_or_else(|e| e.into_inner());
        if table
            .get(&kind)
            .is_some_and(|e| *e.wasm == *wasm && e.rate == rate)
            && super::group(&kind).is_some()
        {
            return Ok(Registered { kind, version });
        }
    }
    // `init` receives this rate, so a module's coefficients match the running device.
    let cfg = InstanceConfig::new(rate, MAX_BLOCK);
    let group = CodeGroup::new(compiled, cfg, 1, std::time::Duration::from_secs(1))?;
    budget().register_with(group.health(), PER_MODULE_SHARE);
    super::unregister(&kind);
    super::register(group)?;
    blobs().lock().unwrap_or_else(|e| e.into_inner()).insert(
        kind.clone(),
        Entry {
            version,
            wasm: Arc::from(wasm),
            rate,
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

/// Registers every `(module id, version, wasm)` of a patch's embedded code, so the patch can
/// build. A blob that cannot load (invalid, forbidden import, other ABI, or metadata naming
/// another module or version than the file says) becomes a stand-in instead of an error, so the
/// patch still opens; the returned reports say why. Used by the command-line `--patch` paths,
/// which have no trust notice: the user named the file.
pub fn register_all<'a>(blobs: impl IntoIterator<Item = (&'a str, u32, &'a [u8])>) -> Vec<Report> {
    let mut failed = Vec::new();
    for (id, version, bytes) in blobs {
        let cause = match register_blob(bytes) {
            Ok(r) if r.kind == id && r.version == version => continue,
            Ok(r) => format!("the code says it is {} v{}", r.kind, r.version),
            Err(e) => e.to_string(),
        };
        failed.push(register_stand_in(id, version, Some(bytes), &cause, None));
    }
    failed
}

/// Makes `kind` a stand-in (see [`super::standin`]). Ports and params come from the blob's own
/// metadata when it can be read, else from `fallback` (how the patch uses the module), else
/// there are none. The original blob is kept so a save writes it back unchanged.
pub fn register_stand_in(
    kind: &str,
    version: u32,
    wasm: Option<&[u8]>,
    cause: &str,
    fallback: Option<CodeMeta>,
) -> Report {
    let from_blob = wasm.and_then(|w| engine().read_meta(w).ok());
    let mut meta = from_blob.or(fallback).unwrap_or(CodeMeta {
        kind: String::new(),
        version,
        name: String::new(),
        explain: String::new(),
        category: crate::info::Category::Utility,
        rate: crate::info::Rate::Voice,
        ports: vec![],
        params: vec![],
        advanced: vec![],
        width_units: 4,
    });
    meta.kind = kind.to_owned();
    if meta.name.is_empty() {
        meta.name = kind.to_owned();
    }
    let report = Report {
        kind: kind.to_owned(),
        version,
        cause: cause.to_owned(),
    };
    // A real module registered later wins (`code::register` clears the stand-in).
    super::standin::register(&meta, report.clone());
    if let Some(w) = wasm {
        let rate = latest().lock().unwrap_or_else(|e| e.into_inner()).0;
        blobs().lock().unwrap_or_else(|e| e.into_inner()).insert(
            kind.to_owned(),
            Entry {
                version,
                wasm: Arc::from(w),
                rate,
            },
        );
    }
    report
}

fn latest() -> &'static Mutex<(f32, u32)> {
    static L: Mutex<(f32, u32)> = Mutex::new((DEFAULT_SAMPLE_RATE, DEFAULT_BLOCK));
    &L
}

/// A new sample rate: every registered module gets a fresh group built for it (graphs built
/// from now on use it; a running graph is rebuilt by the app when the device restarts).
fn rebuild_for_rate() {
    let all: Vec<Arc<[u8]>> = blobs()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        // Stand-ins stay stand-ins: only modules that are running get rebuilt.
        .filter(|(kind, _)| super::group(kind).is_some())
        .map(|(_, e)| Arc::clone(&e.wasm))
        .collect();
    for wasm in all {
        let _ = register_blob(&wasm);
    }
}

static CALIBRATING: AtomicBool = AtomicBool::new(false);
static CALIBRATED: AtomicBool = AtomicBool::new(false);

/// True once a measured calibration (not the fallback) is in use.
pub fn is_calibrated() -> bool {
    CALIBRATED.load(Ordering::Acquire)
}

/// Call at app startup and whenever the audio device changes (sample rate or block size). The
/// block period is applied at once; the fuel-per-microsecond calibration runs on a background
/// thread (never the audio thread) and rebalances every module's budget when it finishes.
/// Until then budgets use the conservative fallback. A call while a measurement is running only
/// updates the period.
pub fn device_changed(sample_rate: f32, block_frames: u32) {
    let block = block_frames.max(1);
    let old_rate = std::mem::replace(
        &mut *latest().lock().unwrap_or_else(|e| e.into_inner()),
        (sample_rate, block),
    )
    .0;
    if old_rate != sample_rate {
        rebuild_for_rate();
    }
    budget().update(None, sample_rate, block);
    if CALIBRATING.swap(true, Ordering::AcqRel) {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("kabl-calibrate".into())
        .spawn(|| {
            if let Ok(cal) = Calibration::measure(engine()) {
                // The device may have changed again while measuring: use the latest period.
                let (sr, blk) = *latest().lock().unwrap_or_else(|e| e.into_inner());
                // For benchmark runs inside a host (REAPER): set the variable to a file path.
                if let Some(path) = std::env::var_os("KABL_CODE_CALIBRATION_REPORT") {
                    let mut text = format!("worst_fuel_per_us {:.1}\n", cal.fuel_per_us);
                    for (name, rate) in &cal.probes {
                        text.push_str(&format!("{name} {rate:.1}\n"));
                    }
                    text.push_str(&format!("period_us {:.1}\n", blk as f64 / sr as f64 * 1e6));
                    let _ = std::fs::write(path, text);
                }
                budget().update(Some(cal), sr, blk);
                CALIBRATED.store(true, Ordering::Release);
            }
            CALIBRATING.store(false, Ordering::Release);
        });
    if spawned.is_err() {
        CALIBRATING.store(false, Ordering::Release);
    }
}
