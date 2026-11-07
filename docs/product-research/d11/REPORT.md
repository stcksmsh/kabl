# D11 gate: wasmi on a real kernel

Date: 2026-11-07. Decisions 1, 5, 10 of `briefs/D11-DECISIONS.md`. Benchmark only; no product integration.
Reproduce: `docs/product-research/d11/build.sh` (needs `rustup target add wasm32-unknown-unknown`; `kernel.wasm` is committed, 749 bytes).

## Setup

- Kernel (`kernel/src/lib.rs`, Rust `no_std` to wasm32): transposed direct-form-II biquad (low-pass 1 kHz, Q 0.707) with a one-sample feedback loop (`x + fb * previous output`). ABI-v0 shape: `init`, `process(n)`, `reset`; audio, params and state live in linear memory.
- 64-sample blocks, 16 voices, one wasmi `Store` and memory per voice, release build, `CompilationMode::Eager`, wasmi 2.0.0 (newest that resolves on the local rustc 1.93).
- Per-block cost includes the host copy of 64 input floats in, 6 params in, 64 output floats out, and (fuel on) one `set_fuel`. Median of 7 runs of 20,000 rounds x 16 voices. Dev machine, x86_64, 20 threads.
- Block period at 48 kHz = 1333.3 us. Item-5 budget: 25 % = 333 us per module, 50 % = 667 us for all custom modules.
- "16 voices" = 16 instances of one module; the table gives the cost of all 16 together.

## Result: PASS, by about 20x

| | us/block/voice | % of block, one voice | % of block, 16 voices |
|---|---|---|---|
| native Rust | 0.179 | 0.01 | 0.21 |
| wasmi Eager, fuel off | 0.980 | 0.07 | 1.18 |
| wasmi Eager, fuel on | 1.034 | 0.08 | 1.24 |

- wasmi is 5.5x native (fuel off), 5.8x (fuel on). Fuel overhead is 5.5 %. This measures one kernel; the "2.6x native" figure from the earlier toy spike is superseded, and 5.5x is in line with an interpreter's expected range.
- Against the budget: 1.24 % for 16 voices of the module vs 25 % allowed (20x headroom). Even 10x the work would stay under 25 %.
- Fuel used: 2,933 per 64-frame block (45.8 per frame).

### Fuel calibration caveat (item 5)

A spin loop ran 1,327 fuel/us (2e6 fuel stopped after 1.5 ms; trap works, store reusable). The real kernel ran 2,933 fuel in ~1 us, about 3,000 fuel/us. So fuel to wall-clock varies by about 2x with instruction mix. Calibrating on a spin loop is conservative for this kernel (it would grant ~442k fuel for the 25 % budget where the kernel needs 2.9k) but a kernel heavy on memory access or division could cost more time per fuel. Recommendation: calibrate with a representative arithmetic-plus-memory kernel and keep the 8-consecutive-overrun rule as the backstop.

## Denormals (item 10): verified, and it matters

Host SSE flush-to-zero/denormals-are-zero (MXCSR bits 15 and 6) applies to wasmi, because it executes float ops on the host CPU.

| persistent denormal input (1e-40), fb 0.5 | us/block/voice | % for 16 voices |
|---|---|---|
| native, FTZ/DAZ off | 9.706 | 11.65 |
| wasmi fuel off, FTZ/DAZ off | 12.167 | 14.60 |
| native, FTZ/DAZ on | 0.179 | 0.22 |
| wasmi fuel off, FTZ/DAZ on | 0.932 | 1.12 |

- Denormals cost wasmi 13x (12.2 vs 0.93 us). With FTZ/DAZ on the host audio thread the cost disappears, so no module-side DC offset is needed.
- Decaying loop (impulse, fb 0.999, then silence, wasmi, fuel off): with FTZ/DAZ off the output never flushed to zero in 400,000 blocks and 384,406 of them were denormal (11.6 us/block average); with FTZ/DAZ on it reached silence in 14,579 blocks, no denormal blocks, 0.83 us/block.
- Even the slow case (14.6 % for 16 voices) fits the 25 % cap; the real risk is several modules at once approaching the 50 % total.
- **The engine does not set FTZ/DAZ today** (no `_mm_setcsr` or equivalent anywhere in `crates/`). Recommended: set FTZ/DAZ at the top of the audio callback (and restore it after in plug-in hosts) for builds that run custom modules, and keep the fuel budget as the safety net. Note that aarch64 needs the FPCR FZ bit instead; not tested here.

## Tests (debug build)

`cargo test` in `d11/`: 2 passed, 0 failed (`wasm_matches_native_bit_exact`: wasm output equals the native reference bit for bit over 200 blocks; `fuel_stops_spin`: an endless loop traps on fuel and the instance runs again after refuelling). This crate is outside the kabl workspace and touches no workspace code.

## Not covered

REAPER/CLAP run (acceptance, later PR); real-world heavier kernels (FM, delay lines with large memory); aarch64; fuel calibration at startup; wasmtime (not added, per decision 1).
