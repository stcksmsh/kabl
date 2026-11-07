//! Time budgets for code modules (D11 decision 5 and its follow-up).
//!
//! wasmi cannot measure wall-clock time inside a call, so a block's time limit is fuel. Fuel is
//! converted from time with a calibration done on the user's machine at startup and after an
//! audio-device change. The calibration uses the **slowest fuel-per-microsecond** code mix of
//! several probes (plain ALU, float divide/sqrt, integer divide, scattered memory, calls), so the
//! fuel granted for a time cap never takes longer than the cap. The wall-clock backstop in
//! `CodeModule` covers whatever the probes miss (an overrun is an overrun even with fuel left).
//!
//! Caps: each module gets at most 25 % of the block period, and all custom modules together at
//! most 50 %; when the requests add up to more, every module is scaled down in proportion.

use super::engine::LoadError;
use super::module::Health;
use super::wasmi_engine::WasmiEngine;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};
use wasmi::{Linker, Module, Store, TrapCode};

pub const PER_MODULE_SHARE: f64 = 0.25;
pub const TOTAL_SHARE: f64 = 0.5;
const PROBES: [&str; 5] = ["spin", "fdiv", "idiv", "mem", "call"];
const PROBE_FUEL: u64 = 2_000_000;
const PROBE_REPEATS: usize = 5;
static CALIB_WASM: &[u8] = include_bytes!("calib.wasm");

#[derive(Debug, Clone)]
pub struct Calibration {
    /// The worst case (lowest) fuel per microsecond over all probes: what budgets use.
    pub fuel_per_us: f64,
    /// Median fuel per microsecond of each probe, for the report.
    pub probes: Vec<(&'static str, f64)>,
}

impl Calibration {
    /// A fixed value, for tests and as a conservative fallback before the first calibration.
    pub fn fixed(fuel_per_us: f64) -> Calibration {
        Calibration {
            fuel_per_us,
            probes: vec![],
        }
    }

    /// Fuel that, at the worst-case rate, lasts at most `us` microseconds.
    pub fn fuel_for_us(&self, us: f64) -> u64 {
        (self.fuel_per_us * us).max(1.0) as u64
    }

    /// Measures this machine. Takes a few milliseconds per probe; run it on a background thread
    /// (never on the audio thread), at startup and when the audio device changes.
    pub fn measure(engine: &WasmiEngine) -> Result<Calibration, LoadError> {
        let e = engine.engine();
        let module = Module::new(e, CALIB_WASM).map_err(|x| LoadError::Invalid(x.to_string()))?;
        let mut probes = Vec::new();
        for name in PROBES {
            let mut rates = Vec::new();
            for _ in 0..PROBE_REPEATS {
                let mut store = Store::new(e, ());
                let inst = Linker::<()>::new(e)
                    .instantiate_and_start(&mut store, &module)
                    .map_err(|x| LoadError::Invalid(x.to_string()))?;
                let f = inst
                    .get_typed_func::<(), ()>(&store, name)
                    .map_err(|x| LoadError::Invalid(x.to_string()))?;
                store
                    .set_fuel(PROBE_FUEL)
                    .map_err(|x| LoadError::Invalid(x.to_string()))?;
                let t = Instant::now();
                let r = f.call(&mut store, ());
                let us = t.elapsed().as_secs_f64() * 1e6;
                match r {
                    Err(x) if x.as_trap_code() == Some(TrapCode::OutOfFuel) => {
                        rates.push(PROBE_FUEL as f64 / us.max(1e-3))
                    }
                    _ => {
                        return Err(LoadError::Invalid(format!(
                            "calibration probe `{name}` did not run out of fuel"
                        )))
                    }
                }
            }
            rates.sort_by(|a, b| a.partial_cmp(b).unwrap());
            probes.push((name, rates[rates.len() / 2]));
        }
        let fuel_per_us = probes.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
        Ok(Calibration {
            fuel_per_us,
            probes,
        })
    }
}

/// Share of the block period each module may use: its request (normally 25 %), scaled down in
/// proportion when all requests together would pass 50 %.
pub fn scaled_shares(requests: &[f64]) -> Vec<f64> {
    let sum: f64 = requests.iter().sum();
    let scale = if sum > TOTAL_SHARE {
        TOTAL_SHARE / sum
    } else {
        1.0
    };
    requests.iter().map(|r| r * scale).collect()
}

struct Entry {
    health: Weak<Health>,
    request: f64,
}

struct State {
    cal: Calibration,
    period_us: f64,
    entries: Vec<Entry>,
}

/// Owns the time budget of every code module in the running graph. Control thread only; it
/// writes each module's fuel and wall-clock cap into its [`Health`] (atomics the audio thread
/// reads), so applying a new budget never blocks audio.
pub struct Budget {
    state: Mutex<State>,
}

impl Budget {
    pub fn new(cal: Calibration, sample_rate: f32, block_frames: u32) -> Budget {
        Budget {
            state: Mutex::new(State {
                cal,
                period_us: period_us(sample_rate, block_frames),
                entries: vec![],
            }),
        }
    }

    /// Adds a module asking for the default 25 % share and rebalances everyone.
    pub fn register(&self, health: &Arc<Health>) {
        self.register_with(health, PER_MODULE_SHARE);
    }

    pub fn register_with(&self, health: &Arc<Health>, request: f64) {
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        s.entries.push(Entry {
            health: Arc::downgrade(health),
            request: request.clamp(0.0, PER_MODULE_SHARE),
        });
        apply(&mut s);
    }

    /// New calibration and/or block period (audio-device change); rebalances everyone.
    pub fn update(&self, cal: Option<Calibration>, sample_rate: f32, block_frames: u32) {
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(c) = cal {
            s.cal = c;
        }
        s.period_us = period_us(sample_rate, block_frames);
        apply(&mut s);
    }

    pub fn calibration(&self) -> Calibration {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .cal
            .clone()
    }
}

fn period_us(sample_rate: f32, block_frames: u32) -> f64 {
    block_frames as f64 / sample_rate as f64 * 1e6
}

fn apply(s: &mut State) {
    s.entries.retain(|e| e.health.strong_count() > 0);
    let shares = scaled_shares(&s.entries.iter().map(|e| e.request).collect::<Vec<_>>());
    for (e, share) in s.entries.iter().zip(shares) {
        if let Some(h) = e.health.upgrade() {
            let us = s.period_us * share;
            h.set_budget(s.cal.fuel_for_us(us), Duration::from_secs_f64(us / 1e6));
        }
    }
}
