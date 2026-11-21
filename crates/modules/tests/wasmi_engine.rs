//! D11 PR 3: the wasmi backend. Kernels are WAT text turned into wasm bytes here; nothing reads
//! the .kabl format.

use kabl_modules::code::{
    scaled_shares, Budget, Calibration, CodeGroup, CodeModule, Fault, Health, InstanceConfig,
    LoadError, WasmEngine, WasmiEngine, MAX_BLOB_BYTES, OVERRUN_LIMIT, PER_MODULE_SHARE,
    TOTAL_SHARE,
};
use kabl_modules::{Module, ProcessIo, QualityConfig, QualityTier, Signal};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
use std::time::Duration;

struct CountingAlloc;
thread_local! {
    /// Per thread: another test's trap (wasmi allocates its error value) must not count here.
    static VIOLATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static NO_ALLOC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
fn watched() -> bool {
    NO_ALLOC.with(|c| c.get()) || kabl_modules::code::in_code_call()
}
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if watched() {
            VIOLATIONS.with(|c| c.set(c.get() + 1));
        }
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if watched() {
            VIOLATIONS.with(|c| c.set(c.get() + 1));
        }
        System.realloc(p, l, n)
    }
}
#[global_allocator]
static ALLOCATOR: CountingAlloc = CountingAlloc;

const SR: f32 = 48_000.0;
const N: usize = 64;
const FADE: usize = 240;

const META: &str = "kind test.biquad\\nversion 1\\nname Biquad\\nexplain Test kernel.\\ncategory filter\\nrate voice\\nwidth 4\\n\
port in audio in 1\\nport out audio out 1\\n\
param b0 -4 4 1 linear 0 x\\nparam b1 -4 4 0 linear 0 x\\nparam b2 -4 4 0 linear 0 x\\n\
param a1 -4 4 0 linear 0 x\\nparam a2 -4 4 0 linear 0 x\\nparam fb -1 1 0 linear 0 x\\n";

/// Biquad (transposed direct form II) with a one-sample feedback loop, as in the D11 gate.
/// Layout: input 64.., output 320.., params 576 (6 f32), state 600 (z1 z2 prev).
/// Skips the rest of `process` unless parameter 0 is above 2 (the priming block uses 0).
const WHEN_LOUD: &str = "(if (f32.le (f32.load (i32.const 576)) (f32.const 2.0)) (then return))";

const BIQUAD: &str = "
 (local $i i32) (local $x f32) (local $y f32) (local $z1 f32) (local $z2 f32) (local $prev f32)
 (local $b0 f32) (local $b1 f32) (local $b2 f32) (local $a1 f32) (local $a2 f32) (local $fb f32)
 (local.set $b0 (f32.load (i32.const 576))) (local.set $b1 (f32.load (i32.const 580)))
 (local.set $b2 (f32.load (i32.const 584))) (local.set $a1 (f32.load (i32.const 588)))
 (local.set $a2 (f32.load (i32.const 592))) (local.set $fb (f32.load (i32.const 596)))
 (local.set $z1 (f32.load (i32.const 600))) (local.set $z2 (f32.load (i32.const 604)))
 (local.set $prev (f32.load (i32.const 608)))
 (block $done (loop $l
  (br_if $done (i32.ge_u (local.get $i) (local.get $n)))
  (local.set $x (f32.add (f32.load (i32.add (i32.const 64) (i32.shl (local.get $i) (i32.const 2))))
                         (f32.mul (local.get $fb) (local.get $prev))))
  (local.set $y (f32.add (f32.mul (local.get $b0) (local.get $x)) (local.get $z1)))
  (local.set $z1 (f32.add (f32.sub (f32.mul (local.get $b1) (local.get $x)) (f32.mul (local.get $a1) (local.get $y))) (local.get $z2)))
  (local.set $z2 (f32.sub (f32.mul (local.get $b2) (local.get $x)) (f32.mul (local.get $a2) (local.get $y))))
  (f32.store (i32.add (i32.const 320) (i32.shl (local.get $i) (i32.const 2))) (local.get $y))
  (local.set $prev (local.get $y))
  (local.set $i (i32.add (local.get $i) (i32.const 1)))
  (br $l)))
 (f32.store (i32.const 600) (local.get $z1)) (f32.store (i32.const 604) (local.get $z2))
 (f32.store (i32.const 608) (local.get $prev))";

const RESET_STATE: &str =
    "(f32.store (i32.const 600) (f32.const 0)) (f32.store (i32.const 604) (f32.const 0)) (f32.store (i32.const 608) (f32.const 0))";

fn module_wat(pages: u32, imports: &str, process: &str, reset: &str) -> String {
    format!(
        r#"(module {imports}
 (@custom "kabl.meta" "{META}")
 (memory (export "memory") {pages})
 (func (export "init") (param f32 i32) (result i32)
  (i32.store (i32.const 0) (i32.const 64)) (i32.store (i32.const 4) (i32.const 320))
  (i32.store (i32.const 8) (i32.const 576)) (i32.store (i32.const 12) (i32.const 600))
  (i32.store (i32.const 16) (i32.const 12)) (i32.store (i32.const 20) (i32.const 64))
  (i32.store (i32.const 24) (i32.const 1)) (i32.store (i32.const 28) (i32.const 1))
  (i32.store (i32.const 32) (i32.const 6))
  (i32.const 0))
 (func (export "reset") {reset})
 (func (export "process") (param $n i32) {process}))"#
    )
}

fn wasm(pages: u32, imports: &str, process: &str) -> Vec<u8> {
    wat::parse_str(module_wat(pages, imports, process, RESET_STATE)).unwrap()
}

fn wasm_with_reset(process: &str, reset: &str) -> Vec<u8> {
    wat::parse_str(module_wat(1, "", process, reset)).unwrap()
}

fn cfg() -> InstanceConfig {
    InstanceConfig::new(SR, N as u32)
}

fn engine() -> WasmiEngine {
    WasmiEngine::new()
}

fn group(bytes: &[u8], voices: usize, fuel: u64, cap: Duration) -> Arc<CodeGroup> {
    let compiled = engine().compile(bytes).unwrap();
    let g = CodeGroup::new(compiled, cfg(), fuel, cap).unwrap();
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

const COEF: [f32; 6] = [0.0674552, 0.1349104, 0.0674552, -1.1429805, 0.4128015, 0.5];

fn block(m: &mut CodeModule, input: &[f32; N], params: [f32; 6]) -> [f32; N] {
    let sig = [Signal::Buffer(input)];
    let ps = params.map(Signal::Scalar);
    let mut out = [0f32; N];
    {
        let mut outs: [&mut [f32]; 1] = [&mut out];
        let mut io = ProcessIo::new(&sig, &mut outs, &ps, N);
        m.process(&mut io);
    }
    out
}

fn noise(seed: &mut u32) -> [f32; N] {
    let mut v = [0f32; N];
    for x in v.iter_mut() {
        *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        *x = (*seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5;
    }
    v
}

#[test]
fn real_kernel_round_trip_matches_native_bit_for_bit() {
    let g = group(&wasm(1, "", BIQUAD), 1, 1_000_000, Duration::from_secs(1));
    assert_eq!(g.info().kind, "test.biquad");
    assert_eq!(g.info().params.len(), 6);
    let mut m = voice(&g);
    let [b0, b1, b2, a1, a2, fb] = COEF;
    let (mut z1, mut z2, mut prev) = (0f32, 0f32, 0f32);
    let mut seed = 3;
    for _ in 0..200 {
        let inp = noise(&mut seed);
        let out = block(&mut m, &inp, COEF);
        for i in 0..N {
            let x = inp[i] + fb * prev;
            let y = b0 * x + z1;
            z1 = b1 * x - a1 * y + z2;
            z2 = b2 * x - a2 * y;
            prev = y;
            assert_eq!(out[i].to_bits(), y.to_bits(), "frame {i}");
        }
    }
    assert!(!g.health().is_crashed());
}

#[test]
fn forbidden_imports_are_rejected_at_load() {
    let imp = r#"(import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32)))"#;
    let e = engine().compile(&wasm(1, imp, BIQUAD)).err().unwrap();
    assert_eq!(
        e,
        LoadError::ForbiddenImport("wasi_snapshot_preview1.fd_write".into())
    );
    let host = r#"(import "env" "log" (func (param i32)))"#;
    assert!(matches!(
        engine().compile(&wasm(1, host, BIQUAD)),
        Err(LoadError::ForbiddenImport(_))
    ));
}

#[test]
fn bad_modules_are_refused_with_a_reason() {
    assert!(matches!(
        engine().compile(b"not wasm"),
        Err(LoadError::Invalid(_))
    ));
    assert!(matches!(
        engine().compile(&vec![0u8; MAX_BLOB_BYTES + 1]),
        Err(LoadError::TooLarge { .. })
    ));
    let no_meta = wat::parse_str(r#"(module (memory (export "memory") 1) (func (export "init") (param f32 i32) (result i32) (i32.const 0)) (func (export "process") (param i32)) (func (export "reset")))"#).unwrap();
    assert!(
        matches!(engine().compile(&no_meta), Err(LoadError::Invalid(m)) if m.contains("kabl.meta"))
    );
    let no_reset = wat::parse_str(r#"(module (memory (export "memory") 1) (func (export "init") (param f32 i32) (result i32) (i32.const 0)) (func (export "process") (param i32)))"#).unwrap();
    assert!(
        matches!(engine().compile(&no_reset), Err(LoadError::Invalid(m)) if m.contains("reset"))
    );
}

#[test]
fn compile_runs_on_a_background_thread() {
    let e = Arc::new(engine());
    let compiled = e
        .compile_async(wasm(1, "", BIQUAD))
        .join()
        .unwrap()
        .unwrap();
    assert_eq!(compiled.meta().kind, "test.biquad");
}

#[test]
fn memory_over_the_cap_is_refused_at_load_and_growth_is_a_crash() {
    // 65 pages = 4.06 MiB, over the 4 MiB cap.
    let big = engine().compile(&wasm(65, "", BIQUAD)).unwrap();
    let g = CodeGroup::new(big, cfg(), 1_000_000, Duration::from_secs(1)).unwrap();
    assert!(matches!(g.fill(1), Err(LoadError::MemoryOverCap { .. })));

    // memory.grow while processing traps and is reported as memory over cap.
    // (After load: instantiation primes each instance with a zero-parameter block, so a module
    // that grows on its very first block is refused at load instead; see the next test.)
    let grow = format!("{WHEN_LOUD} (drop (memory.grow (i32.const 1)))");
    let g = group(&wasm(1, "", &grow), 1, 1_000_000, Duration::from_secs(1));
    let mut m = voice(&g);
    let mut seed = 1;
    block(&mut m, &noise(&mut seed), COEF);
    assert!(!g.health().is_crashed());
    let mut loud = COEF;
    loud[0] = 3.0;
    for _ in 0..8 {
        block(&mut m, &noise(&mut seed), loud);
    }
    assert_eq!(g.health().fault(), Some(Fault::MemoryOverCap));
}

#[test]
fn a_trap_is_a_crash_that_fades_and_silences_every_voice() {
    let trap_later = "(if (f32.gt (f32.load (i32.const 576)) (f32.const 2.0)) (then unreachable)) (f32.store (i32.const 320) (f32.const 0.5))";
    let g = group(
        &wasm(1, "", trap_later),
        2,
        1_000_000,
        Duration::from_secs(1),
    );
    let (mut a, mut b) = (voice(&g), voice(&g));
    let mut seed = 1;
    let inp = noise(&mut seed);
    block(&mut a, &inp, COEF);
    block(&mut b, &inp, COEF);
    let mut bad = COEF;
    bad[0] = 3.0;
    block(&mut a, &inp, bad);
    assert_eq!(g.health().fault(), Some(Fault::Trap));
    let tail: Vec<f32> = (0..6).flat_map(|_| block(&mut b, &inp, COEF)).collect();
    assert!(
        tail[FADE..].iter().all(|&v| v == 0.0),
        "the other voice is silenced too"
    );
}

#[test]
fn an_infinite_loop_runs_out_of_fuel_and_eight_in_a_row_crash() {
    let spin = format!("{WHEN_LOUD} (loop $l (br $l))");
    let g = group(&wasm(1, "", &spin), 1, 200_000, Duration::from_secs(5));
    let mut m = voice(&g);
    let mut seed = 1;
    let inp = noise(&mut seed);
    let mut loud = COEF;
    loud[0] = 3.0;
    let coef = loud;
    let mut news = vec![];
    for k in 1..OVERRUN_LIMIT {
        block(&mut m, &inp, coef);
        assert!(!g.health().is_crashed(), "{k} overruns are not a crash");
        news.extend(g.health().take_overrun_news());
    }
    block(&mut m, &inp, coef);
    news.extend(g.health().take_overrun_news());
    assert_eq!(g.health().fault(), Some(Fault::Overrun));
    assert_eq!(g.health().overrun_count(), OVERRUN_LIMIT);
    assert_eq!(
        news,
        [1, 2, 4, 8],
        "told at first, then each time it doubles"
    );
}

#[test]
fn a_modules_measured_rate_moves_its_fuel_between_floor_and_ceiling() {
    let us = Duration::from_micros;
    let h = Health::new(1, Duration::ZERO);
    // A 100 us share: floor at 1000 fuel/us, ceiling at 4000 fuel/us.
    h.set_budget_range(100_000, 400_000, us(100));
    assert_eq!(h.fuel_range(), (100_000, 400_000));
    assert_eq!(
        h.fuel_budget(),
        400_000,
        "a new module starts at the ceiling"
    );

    // One late call (10x slower, e.g. preempted) costs a quarter at most.
    h.observe(4_000, us(10));
    assert!(h.fuel_budget() >= 300_000, "{}", h.fuel_budget());
    for _ in 0..64 {
        h.observe(40_000, us(10));
    }
    assert!(h.fuel_budget() >= 390_000, "recovers: {}", h.fuel_budget());

    // Code that burns fuel at the floor rate is reined in within a few calls.
    for _ in 0..6 {
        h.observe(10_000, us(10));
    }
    assert!(h.fuel_budget() < 160_000, "{}", h.fuel_budget());
    // Slower than the floor: never below the floor (the old, worst-case budget).
    for _ in 0..64 {
        h.observe(1_000, us(10));
    }
    assert_eq!(h.fuel_budget(), 100_000);

    // Faster than the ceiling: never above it (the hard cap). Calls too short to time count
    // for nothing.
    for _ in 0..256 {
        h.observe(1_000_000, us(10));
    }
    assert_eq!(h.fuel_budget(), 400_000);
    h.observe(1, Duration::from_nanos(500));
    assert_eq!(h.fuel_budget(), 400_000);

    // A new share (device change) keeps the measured rate: the fuel scales with the share.
    for _ in 0..64 {
        h.observe(20_000, us(10));
    }
    let before = h.fuel_budget();
    h.set_budget_range(200_000, 800_000, us(200));
    assert!(h.fuel_budget().abs_diff(before * 2) <= 2, "{before}");
}

#[test]
fn slow_per_fuel_code_gets_less_fuel_than_dsp_code() {
    let cal = Calibration::measure(&engine()).unwrap();
    let cap = Duration::from_millis(1);
    let (floor, ceil) = (cal.fuel_for_us(1000.0), cal.max_fuel_for_us(1000.0));
    // Thousands of calls per block: the `call` probe's mix, the slowest per unit of fuel.
    let leaf = "(func $leaf (param i32) (result i32) (i32.add (local.get 0) (i32.const 1)))";
    let calls =
        "(local $i i32) (block $d (loop $l (br_if $d (i32.ge_u (local.get $i) (i32.const 4000)))
        (local.set $i (call $leaf (local.get $i))) (br $l)))";
    let slow = group(&wasm(1, leaf, calls), 1, 1, cap);
    let dsp = group(&wasm(1, "", BIQUAD), 1, 1, cap);
    let mut seed = 1;
    for g in [&slow, &dsp] {
        g.health().set_budget_range(floor, ceil, cap);
        let mut m = voice(g);
        for _ in 0..32 {
            block(&mut m, &noise(&mut seed), COEF);
        }
        assert!(!g.health().is_crashed());
    }
    let (s, d) = (slow.health().fuel_budget(), dsp.health().fuel_budget());
    println!("fuel range {floor}..{ceil}: call-heavy {s}, biquad {d}");
    // Release builds show the probes' full spread; a debug build's interpreter overhead narrows
    // it, so only the order is checked.
    assert!(s < d, "call-heavy {s} vs biquad {d}");
    assert!(s >= floor && d <= ceil);
}

#[test]
fn overruns_and_crashes_reach_the_user_as_notes() {
    let spin = format!("{WHEN_LOUD} (loop $l (br $l))");
    let g = group(&wasm(1, "", &spin), 1, 200_000, Duration::from_secs(5));
    kabl_modules::code::register(Arc::clone(&g)).unwrap();
    let mut m = voice(&g);
    let mut loud = COEF;
    loud[0] = 3.0;
    let scope = kabl_modules::code::CodeScope::new();
    let mut seed = 1;
    block(&mut m, &noise(&mut seed), loud);
    scope.collect();
    let notes = scope.take_notes();
    assert!(
        notes
            .iter()
            .any(|n| n.contains("test.biquad") && n.contains("CPU time budget in 1 block")),
        "{notes:?}"
    );
    for _ in 1..OVERRUN_LIMIT {
        block(&mut m, &noise(&mut seed), loud);
    }
    scope.collect();
    let notes = scope.take_notes();
    assert!(
        notes
            .iter()
            .any(|n| n.contains("test.biquad stopped: kept overrunning")),
        "{notes:?}"
    );
    scope.collect();
    assert!(scope
        .take_notes()
        .iter()
        .all(|n| !n.contains("test.biquad")));
    kabl_modules::code::unregister("test.biquad");
}

/// The capacity the budget policy is sized for: `demo.bitfilter` (a stereo resonant filter with
/// drive) at 8 voices, at 48 kHz with 64-frame blocks, with the rates REAPER's calibration
/// measured on an i7-13700H (PR #181: worst 927 fuel/us, fastest 4309).
#[test]
fn eight_code_filters_at_eight_voices_fit_the_budget() {
    let ex = &kabl_modules::code::host::EXAMPLES[1];
    assert_eq!(ex.kind, "demo.bitfilter");
    let compiled = engine().compile(ex.wasm).unwrap();
    let params: Vec<f32> = compiled.meta().params.iter().map(|p| p.default).collect();
    let mut inst = compiled.instantiate(&cfg()).unwrap();
    let layout = inst.layout();
    inst.set_fuel(u64::MAX / 2);
    inst.reset().unwrap();
    let mut seed = 3;
    let mut need = 0;
    for _ in 0..16 {
        let sig = noise(&mut seed);
        let mem = inst.memory();
        for s in 0..layout.in_slots as usize {
            for (i, v) in sig.iter().enumerate() {
                let at = layout.input_range(s).start + i * 4;
                mem[at..at + 4].copy_from_slice(&v.to_le_bytes());
            }
        }
        for (p, v) in params.iter().enumerate() {
            mem[layout.param_range(p)].copy_from_slice(&v.to_le_bytes());
        }
        inst.set_fuel(u64::MAX / 2);
        need = need.max(inst.call_block(N as u32).unwrap());
    }
    // Twice the cost measured here, for other settings and input than these defaults.
    let voices = 8u64;
    let need = 2 * need;
    let cal = Calibration::range(927.2, 4309.0);
    let period_us = N as f64 / SR as f64 * 1e6;
    let fits = |modules: usize, fuel_for: &dyn Fn(f64) -> u64| {
        let share = scaled_shares(&vec![PER_MODULE_SHARE; modules])[0];
        voices * need <= fuel_for(period_us * share)
    };
    let capacity =
        |fuel_for: &dyn Fn(f64) -> u64| (1..64).take_while(|&n| fits(n, fuel_for)).count();
    let old = capacity(&|us| cal.fuel_for_us(us));
    let new = capacity(&|us| cal.max_fuel_for_us(us));
    println!(
        "demo.bitfilter: {} fuel per voice-block (x2 margin {need}); at {voices} voices {old} fit \
         the worst-case budget, {new} the measured-rate ceiling",
        need / 2
    );
    assert!(new >= 8, "{new}");
    assert!(new >= 4 * old, "old {old}, new {new}");
}

#[test]
fn a_module_that_traps_loops_or_grows_on_its_first_block_is_refused_at_load() {
    for (name, body) in [
        ("trap", "unreachable"),
        ("loop", "(loop $l (br $l))"),
        ("grow", "(drop (memory.grow (i32.const 1)))"),
    ] {
        let compiled = engine().compile(&wasm(1, "", body)).unwrap();
        let g = CodeGroup::new(compiled, cfg(), 1_000_000, Duration::from_secs(1)).unwrap();
        assert!(
            matches!(g.fill(1), Err(LoadError::Invalid(m)) if m.contains("first block")),
            "{name}"
        );
    }
}

#[test]
fn nan_output_from_real_code_is_a_crash() {
    let nan = "(f32.store (i32.const 320) (f32.div (f32.const 0) (f32.const 0)))";
    let g = group(&wasm(1, "", nan), 1, 1_000_000, Duration::from_secs(1));
    let mut m = voice(&g);
    let mut seed = 1;
    let out = block(&mut m, &noise(&mut seed), COEF);
    assert!(out.iter().all(|v| v.is_finite()));
    assert_eq!(g.health().fault(), Some(Fault::NonFinite));
}

#[test]
fn calibration_reports_worst_case_fuel_per_microsecond() {
    let cal = Calibration::measure(&engine()).unwrap();
    println!("calibration (fuel/us, median of 5 runs of 2e6 fuel):");
    for (name, rate) in &cal.probes {
        println!("  {name:<5} {rate:>9.1}");
    }
    println!(
        "  worst case used for budgets: {:.1} fuel/us",
        cal.fuel_per_us
    );
    assert_eq!(cal.probes.len(), 5);
    assert!(cal.probes.iter().all(|p| p.1 > 0.0));
    let min = cal.probes.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    assert_eq!(cal.fuel_per_us, min, "the budget uses the slowest mix");
    // Fuel for a time cap lasts at most that long at the worst rate.
    assert!((cal.fuel_for_us(333.0) as f64 / cal.fuel_per_us) <= 333.0 + 1e-6);
}

#[test]
fn budgets_are_25_percent_each_and_50_percent_in_total_with_proportional_scaling() {
    assert_eq!(scaled_shares(&[0.25]), vec![0.25]);
    assert_eq!(scaled_shares(&[0.25, 0.25]), vec![0.25, 0.25]);
    let four = scaled_shares(&[0.25; 4]);
    assert!(four.iter().all(|s| (s - 0.125).abs() < 1e-12));
    let mixed = scaled_shares(&[0.25, 0.25, 0.1]);
    assert!((mixed.iter().sum::<f64>() - 0.5).abs() < 1e-12);
    assert!((mixed[0] / mixed[2] - 2.5).abs() < 1e-9, "proportional");

    // Applied to Health objects: a 64-frame block at 48 kHz is 1333.3 us; 1000 fuel/us.
    let budget = Budget::new(Calibration::fixed(1000.0), SR, N as u32);
    let one = Health::new(1, Duration::ZERO);
    budget.register(&one);
    assert_eq!(one.fuel_budget(), 333_333, "alone: 25 % of the block");
    assert_eq!(one.wall_cap().as_micros(), 333);
    let rest: Vec<_> = (0..3).map(|_| Health::new(1, Duration::ZERO)).collect();
    for h in &rest {
        budget.register(h);
    }
    for h in std::iter::once(&one).chain(&rest) {
        assert_eq!(
            h.fuel_budget(),
            166_666,
            "four modules: 12.5 % each, 50 % in total"
        );
    }
    // Audio-device change: new period and a new calibration rebalance everyone.
    budget.update(Some(Calibration::fixed(2000.0)), 48_000.0, 128);
    assert_eq!(one.fuel_budget(), 666_666);
    drop(rest);
    budget.update(None, 48_000.0, 128);
    assert_eq!(
        one.fuel_budget(),
        1_333_333,
        "dropped modules free their share"
    );
}

#[test]
fn sixteen_voices_times_four_modules_never_allocate_on_the_audio_thread() {
    let bytes = wasm(1, "", BIQUAD);
    let groups: Vec<_> = (0..4)
        .map(|_| group(&bytes, 16, 1_000_000, Duration::from_secs(1)))
        .collect();
    let mut mods: Vec<CodeModule> = groups
        .iter()
        .flat_map(|g| (0..16).map(|_| voice(g)).collect::<Vec<_>>())
        .collect();
    assert_eq!(mods.len(), 64);
    let mut seed = 9;
    let inputs: Vec<[f32; N]> = (0..4).map(|_| noise(&mut seed)).collect();
    // No warm-up: instantiation already primed every instance off the audio thread.
    let before = VIOLATIONS.with(|c| c.get());
    for r in 0..50 {
        for m in mods.iter_mut() {
            let sig = [Signal::Buffer(&inputs[r % 4])];
            let ps = COEF.map(Signal::Scalar);
            let mut out = [0f32; N];
            let mut outs: [&mut [f32]; 1] = [&mut out];
            let mut io = ProcessIo::new(&sig, &mut outs, &ps, N);
            NO_ALLOC.with(|c| c.set(true));
            m.process(&mut io);
            NO_ALLOC.with(|c| c.set(false));
        }
    }
    assert_eq!(
        VIOLATIONS.with(|c| c.get()) - before,
        0,
        "allocated while processing"
    );
    assert!(groups.iter().all(|g| !g.health().is_crashed()));
}

/// Allocations made inside `m.process` for one block with the given params.
fn allocs_in_block(m: &mut CodeModule, params: [f32; 6]) -> usize {
    let mut seed = 5;
    let inp = noise(&mut seed);
    let sig = [Signal::Buffer(&inp)];
    let ps = params.map(Signal::Scalar);
    let mut out = [0f32; N];
    let mut outs: [&mut [f32]; 1] = [&mut out];
    let mut io = ProcessIo::new(&sig, &mut outs, &ps, N);
    let before = VIOLATIONS.with(|c| c.get());
    NO_ALLOC.with(|c| c.set(true));
    m.process(&mut io);
    NO_ALLOC.with(|c| c.set(false));
    VIOLATIONS.with(|c| c.get()) - before
}

#[test]
fn an_out_of_fuel_block_allocates_nothing_and_a_trap_at_most_once() {
    let mut loud = COEF;
    loud[0] = 3.0;

    // Overruns: seven in a row (one short of the crash limit) allocate nothing at all.
    let spin = format!("{WHEN_LOUD} (loop $l (br $l))");
    let g = group(&wasm(1, "", &spin), 1, 100_000, Duration::from_secs(5));
    let mut m = voice(&g);
    assert_eq!(allocs_in_block(&mut m, COEF), 0);
    for k in 1..OVERRUN_LIMIT {
        assert_eq!(allocs_in_block(&mut m, loud), 0, "overrun block {k}");
    }
    assert_eq!(g.health().overrun_count(), OVERRUN_LIMIT - 1);
    assert!(!g.health().is_crashed());
    // A good block in between still runs, with no allocation either.
    assert_eq!(allocs_in_block(&mut m, COEF), 0);

    // A real trap builds wasmi's error: at most one allocation, then the module is off.
    let trap = format!("{WHEN_LOUD} unreachable");
    let g = group(&wasm(1, "", &trap), 1, 100_000, Duration::from_secs(5));
    let mut m = voice(&g);
    assert!(allocs_in_block(&mut m, loud) <= 1);
    assert!(g.health().is_crashed());
    assert_eq!(
        allocs_in_block(&mut m, loud),
        0,
        "a disabled module runs nothing"
    );
}

/// Reset only misbehaves once a loud block has run (the priming reset at load is clean).
fn bad_reset(body: &str) -> String {
    format!("(if (f32.gt (f32.load (i32.const 576)) (f32.const 2.0)) (then {body}))")
}

fn reset_counted(m: &mut CodeModule) -> usize {
    NO_ALLOC.with(|c| c.set(true));
    let before = VIOLATIONS.with(|c| c.get());
    m.reset();
    NO_ALLOC.with(|c| c.set(false));
    VIOLATIONS.with(|c| c.get()) - before
}

#[test]
fn reset_runs_under_the_block_budget_without_allocating() {
    let mut loud = COEF;
    loud[0] = 3.0;
    let mut seed = 1;
    let inp = noise(&mut seed);

    // A clean reset: no allocation, no overrun.
    let g = group(&wasm(1, "", BIQUAD), 1, 1_000_000, Duration::from_secs(5));
    let mut m = voice(&g);
    block(&mut m, &inp, COEF);
    assert_eq!(reset_counted(&mut m), 0);
    assert_eq!(g.health().overrun_count(), 0);

    // A reset that never ends is cut off by the voice's fuel (not a fixed 1e6), allocates
    // nothing, counts as an overrun, and eight in a row disable the module like a block would.
    let spin = bad_reset("(loop $l (br $l))");
    let g = group(
        &wasm_with_reset(BIQUAD, &spin),
        1,
        100_000,
        Duration::from_secs(5),
    );
    let mut m = voice(&g);
    block(&mut m, &inp, loud);
    for k in 1..OVERRUN_LIMIT {
        assert_eq!(reset_counted(&mut m), 0, "reset overrun {k}");
        assert!(
            !g.health().is_crashed(),
            "{k} reset overruns are not a crash"
        );
    }
    assert_eq!(g.health().overrun_count(), OVERRUN_LIMIT - 1);
    m.reset();
    assert_eq!(g.health().fault(), Some(Fault::Overrun));

    // A trapping reset disables the module.
    let g = group(
        &wasm_with_reset(BIQUAD, &bad_reset("unreachable")),
        1,
        100_000,
        Duration::from_secs(5),
    );
    let mut m = voice(&g);
    block(&mut m, &inp, loud);
    m.reset();
    assert_eq!(g.health().fault(), Some(Fault::Trap));
}

#[test]
fn sixteen_voices_share_their_modules_budget_and_stay_within_both_caps() {
    let bytes = wasm(1, "", BIQUAD);
    let cal = Calibration::fixed(1000.0);
    let budget = Budget::new(cal.clone(), SR, N as u32);
    let period_us = N as f64 / SR as f64 * 1e6;

    // One module with 16 voices: all voices together within 25 % of the block.
    let g = group(&bytes, 16, 1, Duration::from_secs(1));
    let one: Vec<CodeModule> = (0..16).map(|_| voice(&g)).collect();
    budget.register(g.health());
    let h = g.health();
    assert_eq!(h.voices(), 16);
    let module_cap_fuel = cal.fuel_for_us(period_us * PER_MODULE_SHARE);
    assert!(
        16 * h.voice_fuel() <= module_cap_fuel,
        "{} > {module_cap_fuel}",
        16 * h.voice_fuel()
    );
    assert!(
        16 * h.voice_fuel() > module_cap_fuel * 9 / 10,
        "voices do get the budget, not a sliver"
    );
    assert!(h.voice_wall_cap() * 16 <= Duration::from_secs_f64(period_us * PER_MODULE_SHARE / 1e6));

    // Four such modules (64 voices): each within 12.5 %, all together within 50 %.
    let groups: Vec<_> = (0..3)
        .map(|_| group(&bytes, 16, 1, Duration::from_secs(1)))
        .collect();
    let more: Vec<CodeModule> = groups
        .iter()
        .flat_map(|g| (0..16).map(|_| voice(g)).collect::<Vec<_>>())
        .collect();
    for g in &groups {
        budget.register(g.health());
    }
    let total_cap_fuel = cal.fuel_for_us(period_us * TOTAL_SHARE);
    let mut total = 0;
    for g in std::iter::once(&g).chain(&groups) {
        let spent = u64::from(g.health().voices()) * g.health().voice_fuel();
        assert!(
            spent <= cal.fuel_for_us(period_us * 0.125),
            "module over its scaled share"
        );
        total += spent;
    }
    assert!(total <= total_cap_fuel, "{total} > {total_cap_fuel}");
    drop((one, more));
    assert_eq!(
        g.health().voices(),
        0,
        "dropping voices releases their count"
    );
}

#[test]
fn a_param_without_a_unit_is_accepted_and_fewer_than_six_fields_are_not() {
    use kabl_modules::code::wasmi_engine::parse_meta;
    let ok = parse_meta(
        "kind x.y\nversion 1\nparam gain 0 4 1 linear 5\nparam f 20 2000 440 exp 5 Hz\n",
    )
    .unwrap();
    assert_eq!(ok.params[0].unit, "");
    assert_eq!(ok.params[1].unit, "Hz");
    assert!(parse_meta("kind x.y\nversion 1\nparam gain 0 4 1 linear\n").is_err());
}

#[test]
fn calibration_module_matches_its_wat_source() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/code");
    let built = wat::parse_file(dir.join("calib.wat")).unwrap();
    if std::env::var("KABL_REGEN_CALIB").is_ok() {
        std::fs::write(dir.join("calib.wasm"), &built).unwrap();
    }
    assert_eq!(
        std::fs::read(dir.join("calib.wasm")).unwrap(),
        built,
        "run with KABL_REGEN_CALIB=1"
    );
}
