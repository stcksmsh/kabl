//! `cargo kabl test`: the offline harness. Builds the module, then renders it through the real
//! `WasmiEngine` under its own `FtzGuard` (the host sets FTZ/DAZ in the audio callback; an
//! offline tool must do the same or denormals cost 13x, D11) and runs the checks: golden hash,
//! parameter sweep, reset, determinism, state round trip, 16 voices, the fault matrix and a cost
//! report. The smoke render of `cargo kabl validate` is reused, not duplicated.

use crate::build;
use crate::validate::{render, write_inputs, Pass, FRAMES, FUEL};
use kabl_engine::ftz::FtzGuard;
use kabl_modules::code::engine::StateXfer;
use kabl_modules::code::{
    CodeMeta, CompiledModule, Fault, InstanceConfig, WasmEngine, WasmInstance, WasmiEngine,
};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;

const VOICES: usize = 16;
/// Block size and repeat count of the cost measurement.
const COST_FRAMES: usize = 64;
const COST_ROUNDS: usize = 500;
/// Fuel per microsecond measured in D11: a tight spin and the real kernel (they differ ~2x).
const FUEL_PER_US: (f64, f64) = (1_327.0, 3_000.0);
/// Fault builds the template provides as cargo features.
const FAULTS: [(&str, Fault); 3] = [
    ("fault-trap", Fault::Trap),
    ("fault-spin", Fault::OutOfFuel),
    ("fault-nan", Fault::NonFinite),
];

type Check = (&'static str, Result<String, String>);
type Module = Arc<dyn CompiledModule>;

pub struct Opts {
    build: build::Args,
    /// Overwrite the golden hash with the current output.
    bless: bool,
}

pub fn parse(args: &[String]) -> Result<Opts, String> {
    let rest: Vec<String> = args.iter().filter(|a| *a != "--bless").cloned().collect();
    Ok(Opts {
        build: build::parse(&rest)?,
        bless: rest.len() != args.len(),
    })
}

/// Deterministic noise in [-1, 1), indexed by frame.
fn noise(seed: u32) -> impl Fn(usize) -> f32 {
    move |i| {
        let mut x = (i as u32).wrapping_add(seed).wrapping_mul(0x9E37_79B9);
        x ^= x >> 15;
        x = x.wrapping_mul(0x85EB_CA6B);
        x ^= x >> 13;
        x as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}

fn instance(c: &Module) -> Result<Box<dyn WasmInstance>, String> {
    c.instantiate(&InstanceConfig::new(48_000.0, 2048))
        .map_err(|e| e.to_string())
}

fn defaults(m: &CodeMeta) -> Vec<f32> {
    m.params.iter().map(|p| p.default).collect()
}

/// A clean render: a fault or a non-finite sample is an error.
fn clean(i: &mut dyn WasmInstance, seed: u32, params: &[f32]) -> Result<Pass, String> {
    let frames = FRAMES.min(i.layout().max_block as usize);
    let p = render(i, &noise(seed), params, frames).map_err(|e| format!("module {e}"))?;
    if p.non_finite {
        return Err("output is not finite".into());
    }
    Ok(p)
}

fn golden(path: &Path, kind: &str, hash: u64, bless: bool) -> Result<String, String> {
    let line = format!("{kind} {hash:016x}\n");
    match std::fs::read_to_string(path) {
        Ok(old) if !bless => {
            if old == line {
                Ok(format!("{hash:016x} matches {}", path.display()))
            } else {
                Err(format!(
                    "output hash is {hash:016x}, {} has `{}`: the module's output changed; \
                     rerun with --bless if that is intended",
                    path.display(),
                    old.trim()
                ))
            }
        }
        _ => {
            std::fs::write(path, line).map_err(|e| format!("{}: {e}", path.display()))?;
            Ok(format!("{hash:016x} recorded in {}", path.display()))
        }
    }
}

fn sweep(c: &Module, meta: &CodeMeta) -> Result<String, String> {
    let mut i = instance(c)?;
    for (k, p) in meta.params.iter().enumerate() {
        for step in 0..=4 {
            let mut v = defaults(meta);
            v[k] = p.min + (p.max - p.min) * step as f32 / 4.0;
            clean(i.as_mut(), 2, &v).map_err(|e| format!("param `{}` = {}: {e}", p.name, v[k]))?;
        }
    }
    Ok(format!(
        "{} params x 5 points, all finite",
        meta.params.len()
    ))
}

fn reset(c: &Module, meta: &CodeMeta) -> Result<String, String> {
    let (mut i, d) = (instance(c)?, defaults(meta));
    let first = clean(i.as_mut(), 3, &d)?.hash;
    clean(i.as_mut(), 4, &d)?;
    if clean(i.as_mut(), 3, &d)?.hash != first {
        return Err(
            "the same input after other input gives different output: reset leaves state behind"
                .into(),
        );
    }
    Ok("same output after a reset".into())
}

fn saved(i: &mut dyn WasmInstance, x: &StateXfer) -> Result<Vec<u8>, String> {
    i.set_fuel(FUEL);
    let len = i
        .call_state_save()
        .map_err(|f| format!("save: module {}", f.label()))?;
    match usize::try_from(len) {
        Ok(n) if n <= x.cap => Ok(i.memory()[x.addr..x.addr + n].to_vec()),
        _ => Err(format!("save returned {len} for a {} byte buffer", x.cap)),
    }
}

fn state(c: &Module, meta: &CodeMeta) -> Result<String, String> {
    let mut a = instance(c)?;
    let Some(x) = a.state_xfer() else {
        return Ok("no saved settings, skipped".into());
    };
    clean(a.as_mut(), 5, &defaults(meta))?;
    let bytes = saved(a.as_mut(), &x)?;
    let mut b = instance(c)?;
    let xb = b
        .state_xfer()
        .ok_or("second instance has no state buffer")?;
    b.memory()[xb.addr..xb.addr + bytes.len()].copy_from_slice(&bytes);
    b.set_fuel(FUEL);
    let ok = b
        .call_state_load(bytes.len() as u32, x.state_version)
        .map_err(|f| format!("load: module {}", f.label()))?;
    if ok != 1 {
        return Err("load refused the bytes its own save wrote".into());
    }
    if saved(b.as_mut(), &xb)? != bytes {
        return Err("save after load differs from the saved bytes".into());
    }
    Ok(format!(
        "{} bytes saved, loaded, saved again identically",
        bytes.len()
    ))
}

fn voices(c: &Module, meta: &CodeMeta) -> Result<String, String> {
    let (mut solo, d) = (instance(c)?, defaults(meta));
    let mut all = (0..VOICES)
        .map(|_| instance(c))
        .collect::<Result<Vec<_>, _>>()?;
    for (v, i) in all.iter_mut().enumerate() {
        let seed = 100 + v as u32;
        let want = clean(solo.as_mut(), seed, &d)?.hash;
        let got = clean(i.as_mut(), seed, &d).map_err(|e| format!("voice {v}: {e}"))?;
        if got.hash != want {
            return Err(format!(
                "voice {v} differs from a solo instance on the same input"
            ));
        }
    }
    Ok(format!("{VOICES} voices render like a solo instance"))
}

/// Median of `COST_ROUNDS` timed calls of `f`, in microseconds.
fn median_us(mut f: impl FnMut() -> Result<(), String>) -> Result<f64, String> {
    let mut t = Vec::with_capacity(COST_ROUNDS);
    for _ in 0..COST_ROUNDS {
        let s = Instant::now();
        f()?;
        t.push(s.elapsed().as_secs_f64() * 1e6);
    }
    t.sort_by(f64::total_cmp);
    Ok(t[t.len() / 2])
}

fn cost(c: &Module, meta: &CodeMeta) -> Result<String, String> {
    let d = defaults(meta);
    let mut all = (0..VOICES)
        .map(|_| instance(c))
        .collect::<Result<Vec<_>, _>>()?;
    for i in &mut all {
        i.set_fuel(FUEL);
        i.reset()
            .map_err(|f| format!("reset: module {}", f.label()))?;
        write_inputs(i.as_mut(), &noise(7), 0, &d, COST_FRAMES);
    }
    let block = |i: &mut Box<dyn WasmInstance>| {
        i.set_fuel(FUEL);
        i.call_block(COST_FRAMES as u32)
            .map_err(|f| format!("module {}", f.label()))
    };
    let fuel = block(&mut all[0])?;
    let one = median_us(|| block(&mut all[0]).map(drop))?;
    let many = median_us(|| all.iter_mut().try_for_each(|i| block(i).map(drop)))?;
    let budget = COST_FRAMES as f64 / 48_000.0 * 1e6;
    let mut s = format!(
        "per {COST_FRAMES}-sample block at 48 kHz ({budget:.0} us): 1 voice {one:.2} us = {:.2} %, \
         {VOICES} voices {many:.1} us = {:.2} %; {fuel} fuel per voice-block, about {:.2} to {:.2} us \
         by D11's fuel rates (fuel to wall-clock varies ~2x with the instruction mix: trust a \
         measurement on your own machine, not the fuel)",
        one / budget * 100.0,
        many / budget * 100.0,
        fuel as f64 / FUEL_PER_US.1,
        fuel as f64 / FUEL_PER_US.0
    );
    if cfg!(debug_assertions) {
        s.push_str("; debug build of cargo-kabl, times are inflated");
    }
    Ok(s)
}

/// What the host would report for this build of the module on the first block.
fn classify(c: &Module) -> Result<String, String> {
    let mut i = instance(c)?;
    let frames = FRAMES.min(i.layout().max_block as usize);
    let d = defaults(c.meta());
    Ok(match render(i.as_mut(), &noise(1), &d, frames) {
        Err(e) => e,
        Ok(p) if p.non_finite => Fault::NonFinite.label().into(),
        Ok(_) => "was not stopped".into(),
    })
}

/// The package's cargo features, from `cargo metadata`.
fn features(args: &build::Args) -> Result<Vec<String>, String> {
    let manifest = build::manifest(args)?;
    let out = Command::new("cargo")
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--manifest-path",
        ])
        .arg(&manifest)
        .output()
        .map_err(|e| format!("cargo metadata: {e}"))?;
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("cargo metadata: {e}"))?;
    let pkg = v["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| match &args.package {
            Some(n) => p["name"] == n.as_str(),
            None => p["manifest_path"].as_str().map(Path::new) == Some(manifest.as_path()),
        });
    Ok(pkg
        .and_then(|p| p["features"].as_object())
        .map(|f| f.keys().cloned().collect())
        .unwrap_or_default())
}

/// Builds the module once per fault feature the crate has; each must be reported as its fault.
fn fault_matrix(args: &build::Args, engine: &WasmiEngine) -> Result<Vec<Check>, String> {
    let have = features(args)?;
    let mut out = vec![];
    for (feat, want) in FAULTS {
        let r = if have.iter().any(|f| f == feat) {
            let a = build::Args {
                features: Some(feat.into()),
                manifest: args.manifest.clone(),
                package: args.package.clone(),
                locked: args.locked,
                allow_reserved: args.allow_reserved,
            };
            build::blob(&a).and_then(|(blob, ..)| {
                let c = engine.compile(&blob).map_err(|e| e.to_string())?;
                let got = classify(&c)?;
                if got == want.label() {
                    Ok(format!("reported as: module {got}"))
                } else {
                    Err(format!(
                        "expected the module to be reported as `{}`, got `{got}`",
                        want.label()
                    ))
                }
            })
        } else {
            Ok(format!(
                "skipped: this crate has no `{feat}` feature (the template does)"
            ))
        };
        out.push((feat, r));
    }
    Ok(out)
}

fn checks(c: &Module, golden_path: &Path, bless: bool) -> Vec<Check> {
    let meta = c.meta().clone();
    let first = instance(c).and_then(|mut i| clean(i.as_mut(), 1, &defaults(&meta)));
    let hash = first.as_ref().ok().map(|p| p.hash);
    vec![
        (
            "golden hash",
            first.and_then(|p| golden(golden_path, &meta.kind, p.hash, bless)),
        ),
        (
            "determinism",
            instance(c).and_then(|mut i| {
                let h = clean(i.as_mut(), 1, &defaults(&meta))?.hash;
                match hash {
                    Some(f) if f != h => Err("two instances gave different bytes".into()),
                    _ => Ok("two instances give identical bytes".into()),
                }
            }),
        ),
        ("parameter sweep", sweep(c, &meta)),
        ("reset", reset(c, &meta)),
        ("state round trip", state(c, &meta)),
        ("16 voices", voices(c, &meta)),
        ("cost", cost(c, &meta)),
    ]
}

pub fn run(o: &Opts) -> Result<(), String> {
    let wasm_path = build::run(&o.build)?;
    // The guard lives to the end: every render below runs with FTZ/DAZ on, like the host.
    let _ftz = FtzGuard::new();
    let bytes = std::fs::read(&wasm_path).map_err(|e| format!("{}: {e}", wasm_path.display()))?;
    let engine = WasmiEngine::new();
    let c = engine.compile(&bytes).map_err(|e| e.to_string())?;
    let golden_path: PathBuf = build::manifest(&o.build)?.with_file_name("kabl-golden.txt");
    let mut all = checks(&c, &golden_path, o.bless);
    all.extend(fault_matrix(&o.build, &engine)?);
    let mut failed = 0;
    for (name, r) in all {
        match r {
            Ok(m) => println!("ok    {name}: {m}"),
            Err(e) => {
                failed += 1;
                println!("FAIL  {name}: {e}");
            }
        }
    }
    if failed > 0 {
        return Err(format!("{failed} check(s) failed"));
    }
    println!("ok");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::tests::{blob, PASS, SILENT};

    fn module(process: &str) -> Module {
        WasmiEngine::new()
            .compile(&blob("alice.t", "", "", 16448, process))
            .unwrap()
    }

    fn failures(v: &[Check]) -> Vec<&str> {
        v.iter().filter(|c| c.1.is_err()).map(|c| c.0).collect()
    }

    fn tempdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kabl-test-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_good_module_passes_and_the_golden_hash_is_checked() {
        let _g = FtzGuard::new();
        let path = tempdir("golden").join("g.txt");
        let c = module(PASS);
        assert_eq!(
            failures(&checks(&c, &path, false)),
            Vec::<&str>::new(),
            "first run records"
        );
        assert!(path.exists());
        assert_eq!(failures(&checks(&c, &path, false)), Vec::<&str>::new());
        // A different module under the same golden file fails that one check; --bless repairs it.
        let other = module(SILENT);
        assert_eq!(failures(&checks(&other, &path, false)), ["golden hash"]);
        assert!(failures(&checks(&other, &path, true)).is_empty());
    }

    #[test]
    fn faults_are_classified() {
        let _g = FtzGuard::new();
        let label = |body: &str| classify(&module(body)).unwrap();
        // Like the template's fault features: only on non-zero input, so the load-time priming
        // block (silent) passes.
        let on_input = |body: &str| {
            format!("(if (f32.ne (f32.load (i32.const 64)) (f32.const 0)) (then {body}))")
        };
        assert_eq!(label(&on_input("unreachable")), Fault::Trap.label());
        assert_eq!(label(&on_input("(loop (br 0))")), Fault::OutOfFuel.label());
        assert_eq!(
            label("(f32.store (i32.const 8256) (f32.const nan))"),
            Fault::NonFinite.label()
        );
        assert_eq!(label(PASS), "was not stopped");
    }

    #[test]
    fn leftover_state_fails_the_reset_check() {
        // Outputs the previous block's input: state in memory that `reset` does not clear.
        let leaky = "(memory.copy (i32.const 8256) (i32.const 40000) (i32.const 8192)) \
                     (memory.copy (i32.const 40000) (i32.const 64) (i32.const 8192))";
        let c = module(leaky);
        let _g = FtzGuard::new();
        assert!(reset(&c, &c.meta().clone()).is_err());
    }
}
