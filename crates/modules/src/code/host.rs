//! The process-wide side of code modules: the one wasmi engine, the one time [`Budget`], and the
//! table of registered code blobs (so a save can embed exactly the code a patch uses).
//!
//! The example modules (`demo.logic`, `demo.bitfilter`) are committed as wasm blobs built from
//! `sdk/examples` with `kabl-module-sdk` (see `examples/README.md`). They are registered like any other
//! blob, through [`register_blob`]; nothing about them is special after that.

use super::standin::Report;
use super::{
    hash_of, standin_key, Budget, Calibration, CodeGroup, CodeHash, CodeMeta, CodeScope,
    InstanceConfig, LoadError, WasmEngine, WasmiEngine, PER_MODULE_SHARE,
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
    /// The blob's hash: what a song's [`CodeScope`] maps the kind to.
    pub key: CodeHash,
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

/// Every blob kept, by key: its hash when it runs, the stand-in's key when it does not (so a
/// save writes back exactly the bytes that were loaded).
fn blobs() -> &'static Mutex<BTreeMap<CodeHash, Entry>> {
    static T: Mutex<BTreeMap<CodeHash, Entry>> = Mutex::new(BTreeMap::new());
    &T
}

/// Compiles `wasm`, registers it under its hash and makes it the latest of the kind its
/// metadata names, remembering the bytes. Registering the same bytes again is a no-op (but
/// makes them the latest again). Different bytes under the same kind do not replace the earlier
/// code: both run, each for the songs whose [`CodeScope`] names it. Control thread only.
pub fn register_blob(wasm: &[u8]) -> Result<Registered, LoadError> {
    install_blob(wasm, true)
}

fn install_blob(wasm: &[u8], make_latest: bool) -> Result<Registered, LoadError> {
    let compiled = engine().compile(wasm)?;
    let meta = compiled.meta();
    let (kind, version) = (meta.kind.clone(), meta.version);
    let key = hash_of(wasm);
    let rate = latest().lock().unwrap_or_else(|e| e.into_inner()).0;
    {
        let table = blobs().lock().unwrap_or_else(|e| e.into_inner());
        if table.get(&key).is_some_and(|e| e.rate == rate) && super::group_by_key(&key).is_some() {
            if make_latest {
                super::set_latest(&kind, key);
            }
            return Ok(Registered { kind, version, key });
        }
    }
    // `init` receives this rate, so a module's coefficients match the running device.
    let cfg = InstanceConfig::new(rate, MAX_BLOCK);
    let group = CodeGroup::keyed(compiled, cfg, 1, std::time::Duration::from_secs(1), key)?;
    budget().register_voiced(group.health(), PER_MODULE_SHARE);
    super::install(group, make_latest)?;
    blobs().lock().unwrap_or_else(|e| e.into_inner()).insert(
        key,
        Entry {
            version,
            wasm: Arc::from(wasm),
            rate,
        },
    );
    Ok(Registered { kind, version, key })
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

/// The latest registered code for `kind`: `(version, wasm bytes)`.
pub fn blob_for(kind: &str) -> Option<(u32, Arc<[u8]>)> {
    blob_for_in(&CodeScope::default(), kind)
}

/// The code `kind` has in the song `scope` describes (else the latest): what a save embeds.
pub fn blob_for_in(scope: &CodeScope, kind: &str) -> Option<(u32, Arc<[u8]>)> {
    let key = scope.key(kind).or_else(|| super::latest_key(kind))?;
    blobs()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&key)
        .map(|e| (e.version, Arc::clone(&e.wasm)))
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
    register_stand_in_keyed(kind, version, wasm, cause, fallback).0
}

/// [`register_stand_in`], also returning the key to put in the song's [`CodeScope`]. The
/// stand-in lives under a key of its own (derived from the blob's hash, or from how the song
/// uses a module that has no blob), so it never replaces a running group of the same kind that
/// another song uses; it only becomes what the kind resolves to without a scope.
pub fn register_stand_in_keyed(
    kind: &str,
    version: u32,
    wasm: Option<&[u8]>,
    cause: &str,
    fallback: Option<CodeMeta>,
) -> (Report, CodeHash) {
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
    let key = match wasm {
        Some(w) => standin_key(&hash_of(w)),
        None => standin_key(format!("{kind}\0{version}\0{meta:?}").as_bytes()),
    };
    let report = Report {
        kind: kind.to_owned(),
        version,
        cause: cause.to_owned(),
    };
    super::standin::register(key, &meta, report.clone());
    // Without a scope the kind now resolves to the stand-in (the failed module replaces the
    // example or the earlier patch's); a real module registered later wins again.
    super::set_latest(kind, key);
    if let Some(w) = wasm {
        let rate = latest().lock().unwrap_or_else(|e| e.into_inner()).0;
        blobs().lock().unwrap_or_else(|e| e.into_inner()).insert(
            key,
            Entry {
                version,
                wasm: Arc::from(w),
                rate,
            },
        );
    }
    (report, key)
}

fn latest() -> &'static Mutex<(f32, u32)> {
    static L: Mutex<(f32, u32)> = Mutex::new((DEFAULT_SAMPLE_RATE, DEFAULT_BLOCK));
    &L
}

/// A new sample rate: every running module gets a fresh group built for it (graphs built
/// from now on use it; a running graph is rebuilt by the app when the device restarts).
fn rebuild_for_rate() {
    let all: Vec<Arc<[u8]>> = blobs()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        // Stand-ins stay stand-ins: only modules that are running get rebuilt.
        .filter(|(key, _)| super::group_by_key(key).is_some())
        .map(|(_, e)| Arc::clone(&e.wasm))
        .collect();
    for wasm in all {
        // Which blob is the latest of its kind does not change with the rate.
        let _ = install_blob(&wasm, false);
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

/// The song the app has open. Single-song front ends (kabl-ui, the standalone) set it when a
/// song opens, so the entry points that take no [`CodeScope`] (`compile` for the live graph,
/// saving) find that song's code and settings. A host running several songs in one process
/// passes each song's scope explicitly instead.
static ACTIVE: Mutex<Option<CodeScope>> = Mutex::new(None);

pub fn set_active_scope(scope: CodeScope) {
    *ACTIVE.lock().unwrap_or_else(|e| e.into_inner()) = Some(scope);
}

/// The open song's scope (empty until one is set).
pub fn active_scope() -> CodeScope {
    ACTIVE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_default()
}
