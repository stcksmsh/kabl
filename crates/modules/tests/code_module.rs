//! D11 PR 1: the code-module seam, driven by the fake engine (no WebAssembly yet).

use kabl_modules::code::fake::{fake_meta, Behavior, FakeEngine};
use kabl_modules::code::{
    CodeGroup, CodeModule, Fault, InstanceConfig, Layout, LayoutError, LoadError, WasmEngine,
    MAX_BLOB_BYTES, OVERRUN_LIMIT,
};
use kabl_modules::{registry, Module, ProcessIo, QualityConfig, QualityTier, Signal};
use std::sync::Arc;
use std::time::Duration;

#[cfg(debug_assertions)]
#[global_allocator]
static ALLOCATOR: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

const SR: f32 = 48_000.0;
const N: usize = 64;
const FADE: usize = 240; // 5 ms at 48 kHz

fn cfg() -> InstanceConfig {
    InstanceConfig::new(SR, N as u32)
}

fn group(
    kind: &str,
    behaviors: Vec<Behavior>,
    voices: usize,
    wall_cap: Duration,
) -> Arc<CodeGroup> {
    let compiled = FakeEngine::new(kind, behaviors).compile(&[0u8; 8]).unwrap();
    let g = CodeGroup::new(compiled, cfg(), 1_000_000, wall_cap).unwrap();
    g.fill(voices).unwrap();
    g
}

fn voice(g: &CodeGroup) -> CodeModule {
    let mut m = g.voice().unwrap();
    m.prepare(
        SR,
        N,
        &QualityConfig {
            tier: QualityTier::Live,
        },
    );
    m
}

/// One block with constant input 1.0 and gain 0.5 (so valid output is 0.5).
fn run(m: &mut CodeModule) -> Vec<f32> {
    let input = [1.0f32; N];
    let sig = [Signal::Buffer(&input)];
    let ps = [Signal::Scalar(0.5)];
    let mut out = vec![0f32; N];
    {
        let mut outs: [&mut [f32]; 1] = [&mut out];
        let mut io = ProcessIo::new(&sig, &mut outs, &ps, N);
        m.process(&mut io);
    }
    out
}

fn run_many(m: &mut CodeModule, blocks: usize) -> Vec<f32> {
    (0..blocks).flat_map(|_| run(m)).collect()
}

#[test]
fn healthy_module_processes_without_allocating() {
    let g = group("t.ok", vec![Behavior::Gain], 1, Duration::from_secs(1));
    let mut m = voice(&g);
    let input = [1.0f32; N];
    let sig = [Signal::Buffer(&input)];
    let ps = [Signal::Scalar(0.5)];
    let mut out = vec![0f32; N];
    for _ in 0..100 {
        {
            let mut outs: [&mut [f32]; 1] = [&mut out];
            let mut io = ProcessIo::new(&sig, &mut outs, &ps, N);
            // The whole process call, not just the module's inner call, must not allocate.
            #[cfg(debug_assertions)]
            assert_no_alloc::assert_no_alloc(|| m.process(&mut io));
            #[cfg(not(debug_assertions))]
            m.process(&mut io);
        }
        assert!(out.iter().all(|&v| v == 0.5));
    }
    assert!(!g.health().is_crashed());
}

#[test]
fn trap_fades_to_zero_in_5ms_then_stays_silent() {
    let g = group(
        "t.trap",
        vec![Behavior::Trap { after: 3 }],
        1,
        Duration::from_secs(1),
    );
    let mut m = voice(&g);
    for _ in 0..3 {
        assert!(run(&mut m).iter().all(|&v| v == 0.5));
    }
    // The crash block and the following ones form one linear ramp from the last valid sample.
    let tail = run_many(&mut m, 8); // 512 frames, longer than the 240-frame fade
    assert_eq!(g.health().fault(), Some(Fault::Trap));
    for (i, &v) in tail.iter().enumerate() {
        let want = if i < FADE {
            0.5 * (FADE - i) as f32 / FADE as f32
        } else {
            0.0
        };
        assert!((v - want).abs() < 1e-6, "frame {i}: {v} vs {want}");
    }
    assert!(g.health().take_notice(), "toast raised once");
    assert!(!g.health().take_notice());
    // Later blocks stay silent and the module does not retry.
    assert!(run_many(&mut m, 4).iter().all(|&v| v == 0.0));
}

#[test]
fn a_crash_in_one_voice_disables_every_voice() {
    // Voice 0 traps on its 2nd block; voices 1 and 2 would run forever.
    let g = group(
        "t.all",
        vec![Behavior::Trap { after: 1 }, Behavior::Gain],
        3,
        Duration::from_secs(1),
    );
    let mut vs: Vec<CodeModule> = (0..3).map(|_| voice(&g)).collect();
    for v in vs.iter_mut() {
        assert!(run(v).iter().all(|&x| x == 0.5));
    }
    for v in vs.iter_mut() {
        run(v);
    }
    assert!(g.health().is_crashed());
    for v in vs.iter_mut() {
        let tail = run_many(v, 6);
        assert!(
            tail[FADE..].iter().all(|&x| x == 0.0),
            "every voice ends silent"
        );
        assert!(tail.iter().all(|&x| x <= 0.5));
    }
}

#[test]
fn nan_output_counts_as_a_crash_and_never_leaves_the_module() {
    let g = group(
        "t.nan",
        vec![Behavior::NaN { after: 2 }],
        1,
        Duration::from_secs(1),
    );
    let mut m = voice(&g);
    let all = run_many(&mut m, 8);
    assert!(
        all.iter().all(|v| v.is_finite()),
        "no NaN or Inf downstream"
    );
    assert_eq!(g.health().fault(), Some(Fault::NonFinite));
    assert_eq!(g.health().nonfinite_count(), 1);
    // Crash at block 2 (frame 128); the 240-frame fade ends by frame 368.
    assert!(all[N * 6..].iter().all(|&v| v == 0.0));
}

#[test]
fn memory_over_cap_is_a_crash_at_run_time_and_a_load_error_at_build_time() {
    let g = group(
        "t.cap",
        vec![Behavior::OverCap { after: 1 }],
        1,
        Duration::from_secs(1),
    );
    let mut m = voice(&g);
    run(&mut m);
    run_many(&mut m, 6);
    assert_eq!(g.health().fault(), Some(Fault::MemoryOverCap));

    let mut engine = FakeEngine::new("t.big", vec![Behavior::Gain]);
    engine.extra_memory = 5 << 20;
    let compiled = engine.compile(&[0u8; 8]).unwrap();
    let g = CodeGroup::new(compiled, cfg(), 1_000_000, Duration::from_secs(1)).unwrap();
    assert!(matches!(g.fill(1), Err(LoadError::MemoryOverCap { .. })));
}

#[test]
fn eight_consecutive_overruns_are_a_crash() {
    let g = group(
        "t.eight",
        vec![Behavior::OutOfFuel {
            from: 0,
            count: 1000,
        }],
        1,
        Duration::from_secs(1),
    );
    let mut m = voice(&g);
    for k in 1..OVERRUN_LIMIT {
        assert!(
            run(&mut m).iter().all(|&v| v == 0.0),
            "overrun block is silent"
        );
        assert!(!g.health().is_crashed(), "{k} overruns are not a crash");
    }
    run(&mut m);
    assert_eq!(g.health().fault(), Some(Fault::Overrun));
}

#[test]
fn a_one_off_overrun_only_logs() {
    // Overruns at call 3 and call 8, with good blocks between: never 8 in a row.
    let g = group(
        "t.once",
        vec![Behavior::OutOfFuel { from: 3, count: 1 }],
        1,
        Duration::from_secs(1),
    );
    let mut m = voice(&g);
    let out: Vec<Vec<f32>> = (0..12).map(|_| run(&mut m)).collect();
    assert!(
        out[3].iter().all(|&v| v <= 0.5) && out[3][N - 1] < 0.5,
        "the overrun block fades down"
    );
    assert!(out[5].iter().all(|&v| v == 0.5));
    assert!(
        out[4].iter().all(|&v| v == 0.5),
        "the next block runs normally"
    );
    assert!(!g.health().is_crashed());
    assert_eq!(g.health().overrun_count(), 1);
    assert!(!g.health().take_notice());
}

#[test]
fn overrun_blocks_fade_instead_of_cutting() {
    let g = group(
        "t.fade",
        vec![Behavior::OutOfFuel { from: 1, count: 1 }],
        1,
        Duration::from_secs(1),
    );
    let mut m = voice(&g);
    run(&mut m);
    let o = run(&mut m); // overrun block: ramp from 0.5 down
    assert!((o[0] - 0.5).abs() < 1e-6);
    assert!(o.windows(2).all(|w| w[1] <= w[0]));
    assert!(o[N - 1] > 0.0 && o[N - 1] < 0.5);
}

#[test]
fn wall_clock_backstop_counts_overruns_even_with_fuel_left() {
    // Each call sleeps 3 ms against a 1 ms cap; the output is valid, but eight in a row crash.
    let g = group(
        "t.slow",
        vec![Behavior::Sleep(Duration::from_millis(3))],
        1,
        Duration::from_millis(1),
    );
    let mut m = voice(&g);
    for _ in 1..OVERRUN_LIMIT {
        assert!(
            run(&mut m).iter().all(|&v| v == 0.5),
            "valid output is kept while counting"
        );
    }
    assert!(!g.health().is_crashed());
    run(&mut m);
    assert_eq!(g.health().fault(), Some(Fault::Overrun));
}

#[test]
fn reset_does_not_revive_a_crashed_module_but_clear_does() {
    let g = group(
        "t.revive",
        vec![Behavior::Trap { after: 0 }],
        1,
        Duration::from_secs(1),
    );
    let mut m = voice(&g);
    run_many(&mut m, 6);
    assert!(g.health().is_crashed());
    m.reset();
    assert!(run_many(&mut m, 2).iter().all(|&v| v == 0.0));
    g.health().clear();
    assert!(!g.health().is_crashed());
}

#[test]
fn oversize_blob_is_refused() {
    let big = vec![0u8; MAX_BLOB_BYTES + 1];
    let e = FakeEngine::new("t.blob", vec![Behavior::Gain])
        .compile(&big)
        .err()
        .unwrap();
    assert!(matches!(e, LoadError::TooLarge { .. }));
}

#[test]
fn layout_validation_catches_bad_layouts() {
    let ok = Layout::packed(1, 1, 1, 64, 0);
    assert_eq!(ok.validate(ok.memory_bytes(), 1, 1, 1, 64), Ok(()));
    assert!(matches!(
        ok.validate(ok.memory_bytes(), 1, 1, 1, 128),
        Err(LayoutError::TooSmall { .. })
    ));
    assert!(matches!(
        ok.validate(ok.memory_bytes(), 2, 1, 1, 64),
        Err(LayoutError::CountMismatch(_))
    ));
    assert!(matches!(
        ok.validate(100, 1, 1, 1, 64),
        Err(LayoutError::OutOfMemory(_))
    ));
    let mut overlap = ok;
    overlap.output = 128;
    assert!(matches!(
        overlap.validate(ok.memory_bytes(), 1, 1, 1, 64),
        Err(LayoutError::Overlap(..))
    ));
    let mut skew = ok;
    skew.params += 2;
    assert!(matches!(
        skew.validate(ok.memory_bytes() + 8, 1, 1, 1, 64),
        Err(LayoutError::Misaligned(_))
    ));
    let w = Layout::from_words([0, 256, 512, 516, 0, 64, 1, 1, 1]);
    assert_eq!(w.max_block, 64);
}

#[test]
fn registry_serves_code_modules_and_leaves_built_ins_alone() {
    let g = group(
        "t.registry.gain",
        vec![Behavior::Gain],
        2,
        Duration::from_secs(1),
    );
    kabl_modules::code::register(g.clone()).unwrap();
    assert!(registry::create("t.registry.gain").is_some());
    assert_eq!(
        registry::info_for("t.registry.gain").unwrap().params[0].name,
        "gain"
    );
    assert_eq!(g.pooled(), 1, "create took one pre-built instance");
    assert!(registry::create("t.registry.unknown").is_none());
    // Built-ins resolve exactly as before and cannot be shadowed.
    assert!(registry::create("osc.va").is_some());
    let shadow = group("osc.va", vec![Behavior::Gain], 1, Duration::from_secs(1));
    assert!(kabl_modules::code::register(shadow).is_err());
    kabl_modules::code::unregister("t.registry.gain");
    assert!(registry::create("t.registry.gain").is_none());
}

#[test]
fn metadata_leaks_once_per_distinct_metadata() {
    let a = fake_meta("t.leak").info();
    let b = fake_meta("t.leak").info();
    assert!(std::ptr::eq(a, b));
    let mut changed = fake_meta("t.leak");
    changed.version = 2;
    changed.explain = "other".into();
    assert!(!std::ptr::eq(a, changed.info()));
    assert_eq!(a.slots(kabl_modules::PortDirection::Input), 1);
}

#[test]
fn instance_pool_is_built_off_thread() {
    let compiled = FakeEngine::new("t.pool", vec![Behavior::Gain])
        .compile(&[0u8; 8])
        .unwrap();
    let g = CodeGroup::new(compiled, cfg(), 1_000_000, Duration::from_secs(1)).unwrap();
    g.fill_async(16).join().unwrap().unwrap();
    assert_eq!(g.pooled(), 16);
    let vs: Vec<CodeModule> = (0..16).map(|_| g.voice().unwrap()).collect();
    assert_eq!(g.pooled(), 0);
    assert_eq!(vs.len(), 16);
}
