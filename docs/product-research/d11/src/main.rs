//! D11 gate benchmark: wasmi (CompilationMode::Eager) vs native Rust on a real kernel
//! (biquad + one-sample feedback), 64-sample blocks, 16 voices, one wasmi Store per voice.
//! Per-block cost includes the host copies in/out and the param write (ABI v0 shape).
use std::hint::black_box;
use std::time::Instant;
use wasmi::{CompilationMode, Config, Engine, Linker, Memory, Module, Store, TypedFunc};

const N: usize = 64;
const VOICES: usize = 16;
const SR: f64 = 48_000.0;
const BLOCK_US: f64 = N as f64 / SR * 1e6; // 1333.3 us
const WASM: &[u8] = include_bytes!("../kernel.wasm");
// Mem layout in the kernel: input[64], output[64], params[6], state[3]
const OFF_OUT: usize = N * 4;
const OFF_PARAMS: usize = 2 * N * 4;
const OFF_STATE: usize = OFF_PARAMS + 6 * 4;

/// Low-pass biquad (RBJ, fc 1 kHz, Q 0.707) as b0 b1 b2 a1 a2, plus feedback gain.
fn params(fb: f32) -> [f32; 6] {
    let w = 2.0 * std::f64::consts::PI * 1000.0 / SR;
    let alpha = w.sin() / (2.0 * 0.707);
    let a0 = 1.0 + alpha;
    let b1 = (1.0 - w.cos()) / a0;
    [
        (b1 / 2.0) as f32,
        b1 as f32,
        (b1 / 2.0) as f32,
        (-2.0 * w.cos() / a0) as f32,
        ((1.0 - alpha) / a0) as f32,
        fb,
    ]
}

// ---- native reference: same arithmetic as kernel/src/lib.rs ----
#[derive(Clone, Copy)]
struct Native {
    p: [f32; 6],
    s: [f32; 3],
}
impl Native {
    #[inline(never)]
    fn process(&mut self, inp: &[f32; N], out: &mut [f32; N]) {
        let [b0, b1, b2, a1, a2, fb] = self.p;
        let [mut z1, mut z2, mut prev] = self.s;
        for i in 0..N {
            let x = inp[i] + fb * prev;
            let y = b0 * x + z1;
            z1 = b1 * x - a1 * y + z2;
            z2 = b2 * x - a2 * y;
            out[i] = y;
            prev = y;
        }
        self.s = [z1, z2, prev];
    }
}

// ---- wasmi voice ----
struct Voice {
    store: Store<()>,
    mem: Memory,
    process: TypedFunc<i32, ()>,
    spin: TypedFunc<(), ()>,
    base: usize,
}
impl Voice {
    fn new(engine: &Engine, module: &Module) -> Voice {
        let mut store = Store::new(engine, ());
        let _ = store.set_fuel(u64::MAX / 2); // errors (harmlessly) when fuel is off
        let linker = Linker::<()>::new(engine);
        let inst = linker.instantiate_and_start(&mut store, module).unwrap();
        let init = inst.get_typed_func::<(f32, i32), i32>(&store, "init").unwrap();
        let base = init.call(&mut store, (SR as f32, N as i32)).unwrap() as usize;
        Voice {
            mem: inst.get_memory(&store, "memory").unwrap(),
            process: inst.get_typed_func(&store, "process").unwrap(),
            spin: inst.get_typed_func(&store, "spin").unwrap(),
            store,
            base,
        }
    }
    #[inline(always)]
    fn block(&mut self, inp: &[f32; N], out: &mut [f32; N], p: &[f32; 6], fuel: Option<u64>) {
        if let Some(f) = fuel {
            self.store.set_fuel(f).unwrap();
        }
        let d = self.mem.data_mut(&mut self.store);
        let b = self.base;
        for (i, v) in inp.iter().enumerate() {
            d[b + i * 4..b + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        for (i, v) in p.iter().enumerate() {
            d[b + OFF_PARAMS + i * 4..b + OFF_PARAMS + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        self.process.call(&mut self.store, N as i32).unwrap();
        let d = self.mem.data(&self.store);
        for (i, o) in out.iter_mut().enumerate() {
            let a = b + OFF_OUT + i * 4;
            *o = f32::from_le_bytes([d[a], d[a + 1], d[a + 2], d[a + 3]]);
        }
    }
    fn set_state(&mut self, s: [f32; 3]) {
        let b = self.base + OFF_STATE;
        let d = self.mem.data_mut(&mut self.store);
        for (i, v) in s.iter().enumerate() {
            d[b + i * 4..b + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
    }
}

fn engine(fuel: bool) -> Engine {
    let mut c = Config::default();
    c.compilation_mode(CompilationMode::Eager);
    c.consume_fuel(fuel);
    Engine::new(&c)
}

#[allow(deprecated)]
fn set_ftz(on: bool) {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use std::arch::x86_64::{_mm_getcsr, _mm_setcsr};
        let c = _mm_getcsr();
        _mm_setcsr(if on { c | 0x8040 } else { c & !0x8040 });
    }
}

fn noise(seed: &mut u32) -> f32 {
    *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    (*seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// us per block per voice: median of `reps` timed runs of `rounds` rounds x 16 voices.
fn time_native(inp: &[f32; N], fb: f32, rounds: usize, reps: usize) -> f64 {
    let mut vs = [Native { p: params(fb), s: [0.0; 3] }; VOICES];
    let mut out = [0f32; N];
    let mut r = vec![];
    for _ in 0..reps {
        let t = Instant::now();
        for _ in 0..rounds {
            for v in vs.iter_mut() {
                v.process(black_box(inp), &mut out);
                black_box(&out);
            }
        }
        r.push(t.elapsed().as_secs_f64() * 1e6 / (rounds * VOICES) as f64);
    }
    median(&mut r)
}

fn time_wasmi(fuel: bool, inp: &[f32; N], fb: f32, rounds: usize, reps: usize) -> (f64, u64) {
    let e = engine(fuel);
    let m = Module::new(&e, WASM).unwrap();
    let mut vs: Vec<Voice> = (0..VOICES).map(|_| Voice::new(&e, &m)).collect();
    let p = params(fb);
    let mut out = [0f32; N];
    let budget = fuel.then_some(u64::MAX / 2);
    // fuel actually used by one block
    let used = if fuel {
        vs[0].block(inp, &mut out, &p, budget);
        budget.unwrap() - vs[0].store.get_fuel().unwrap()
    } else {
        0
    };
    let mut r = vec![];
    for _ in 0..reps {
        let t = Instant::now();
        for _ in 0..rounds {
            for v in vs.iter_mut() {
                v.block(black_box(inp), &mut out, &p, budget);
                black_box(&out);
            }
        }
        r.push(t.elapsed().as_secs_f64() * 1e6 / (rounds * VOICES) as f64);
    }
    (median(&mut r), used)
}

fn pct(us_per_voice: f64) -> (f64, f64) {
    // all 16 voices of one module together, as % of the block period
    (us_per_voice / BLOCK_US * 100.0, us_per_voice * VOICES as f64 / BLOCK_US * 100.0)
}

fn row(name: &str, us: f64) {
    let (one, all) = pct(us);
    println!("{name:<34} {us:>8.3} us/block/voice   {one:>6.2} % per voice   {all:>6.2} % for 16 voices");
}

fn main() {
    let mut seed = 1u32;
    let mut inp = [0f32; N];
    for v in inp.iter_mut() {
        *v = noise(&mut seed);
    }
    println!("block {N} frames @ {SR} Hz = {BLOCK_US:.1} us; budget 25 % = {:.1} us per module, 50 % = {:.1} us all", BLOCK_US * 0.25, BLOCK_US * 0.5);
    let (rounds, reps) = (20_000, 7);

    println!("\n== steady noise input, fb 0.5, FTZ off ==");
    set_ftz(false);
    let nat = time_native(&inp, 0.5, rounds, reps);
    let (on, used) = time_wasmi(true, &inp, 0.5, rounds, reps);
    let (off, _) = time_wasmi(false, &inp, 0.5, rounds, reps);
    row("native Rust", nat);
    row("wasmi Eager, fuel OFF", off);
    row("wasmi Eager, fuel ON", on);
    println!("fuel used per block: {used} ({:.1} per frame); wasmi/native: off {:.1}x, on {:.1}x; fuel overhead {:.1} %", used as f64 / N as f64, off / nat, on / nat, (on / off - 1.0) * 100.0);

    // Fuel to wall-clock for the budget: calibrate on the spin loop.
    {
        let e = engine(true);
        let m = Module::new(&e, WASM).unwrap();
        let mut v = Voice::new(&e, &m);
        v.store.set_fuel(2_000_000).unwrap();
        let t = Instant::now();
        let r = v.spin.call(&mut v.store, ());
        let us = t.elapsed().as_secs_f64() * 1e6;
        println!("spin loop: trapped={} after 2e6 fuel in {us:.0} us => {:.1} fuel/us; 25 % budget = {:.0} fuel by spin, kernel block needs {used}", r.is_err(), 2e6 / us, 2e6 / us * BLOCK_US * 0.25);
    }

    println!("\n== denormals: persistent denormal input 1e-40, fb 0.5 ==");
    let mut dn = [0f32; N];
    dn.fill(1e-40);
    for ftz in [false, true] {
        set_ftz(ftz);
        let n = time_native(&dn, 0.5, rounds / 4, 5);
        let (w, _) = time_wasmi(false, &dn, 0.5, rounds / 4, 5);
        row(&format!("native, FTZ/DAZ {}", if ftz { "on" } else { "off" }), n);
        row(&format!("wasmi fuel off, FTZ/DAZ {}", if ftz { "on" } else { "off" }), w);
    }

    println!("\n== denormals: decaying loop (impulse, fb 0.999, then silence) ==");
    // one cycle: reset, impulse block, then silence until the state flushes to exactly 0
    let silence = [0f32; N];
    let mut imp = [0f32; N];
    imp[0] = 1.0;
    for ftz in [false, true] {
        set_ftz(ftz);
        let e = engine(false);
        let m = Module::new(&e, WASM).unwrap();
        let mut v = Voice::new(&e, &m);
        let p = params(0.999);
        let mut out = [0f32; N];
        let mut best = (f64::MAX, 0usize, 0usize);
        for _ in 0..3 {
            v.set_state([0.0; 3]);
            v.block(&imp, &mut out, &p, None);
            let (mut blocks, mut dnorm) = (1usize, 0usize);
            let t = Instant::now();
            loop {
                v.block(&silence, &mut out, &p, None);
                blocks += 1;
                let last = out[N - 1];
                if last != 0.0 && last.abs() < f32::MIN_POSITIVE {
                    dnorm += 1;
                }
                if (last == 0.0 && blocks > 4) || blocks > 400_000 {
                    break;
                }
            }
            let us = t.elapsed().as_secs_f64() * 1e6;
            if us < best.0 {
                best = (us, blocks, dnorm);
            }
        }
        println!(
            "wasmi fuel off, FTZ/DAZ {}: {} blocks to silence, {} with denormal output, {:.3} us/block avg",
            if ftz { "on " } else { "off" }, best.1, best.2, best.0 / best.1 as f64
        );
    }
    set_ftz(false);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wasm kernel and the native reference compute the same samples, bit for bit.
    #[test]
    fn wasm_matches_native_bit_exact() {
        let e = engine(true);
        let m = Module::new(&e, WASM).unwrap();
        let mut v = Voice::new(&e, &m);
        let mut nat = Native { p: params(0.5), s: [0.0; 3] };
        let (mut seed, mut a, mut b) = (7u32, [0f32; N], [0f32; N]);
        for _ in 0..200 {
            let mut inp = [0f32; N];
            inp.iter_mut().for_each(|x| *x = noise(&mut seed));
            v.block(&inp, &mut a, &params(0.5), Some(1_000_000));
            nat.process(&inp, &mut b);
            assert_eq!(a.map(f32::to_bits), b.map(f32::to_bits));
        }
    }

    /// An endless loop is stopped by fuel and the instance's store stays usable after refuelling.
    #[test]
    fn fuel_stops_spin() {
        let e = engine(true);
        let m = Module::new(&e, WASM).unwrap();
        let mut v = Voice::new(&e, &m);
        v.store.set_fuel(100_000).unwrap();
        assert!(v.spin.call(&mut v.store, ()).is_err());
        let mut out = [0f32; N];
        v.block(&[0.0; N], &mut out, &params(0.5), Some(1_000_000));
    }
}
