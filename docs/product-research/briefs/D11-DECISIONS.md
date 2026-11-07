# D11 custom-code modules: owner decisions (2026-11-07)

Scope unchanged: D11 proves that WASM modules run safely and fast enough inside kabl. The developer SDK is D12.
Record each numbered decision in `docs/design/redesign/verdicts.json`. Items marked **[verify]** are facts to confirm in code or by measurement before relying on them.

---

## Decisions

### 1. Engine: wasmi only
- Build only wasmi. Drop the "build both, pick later" step.
- Wrap the engine behind a trait (`WasmEngine`: compile, instantiate, call_block, set_fuel, reset) so wasmtime can be added later as a single change.
- **Gate before locking:** benchmark a real kernel, not a toy: biquad + one-sample feedback loop, 64-sample blocks, 16 voices, release build. Report µs per block per voice and % of the block budget at 48 kHz. Treat the "2.6× native" figure as unproven: Wasmtime's own docs put its interpreter (Pulley) at about 10× native for compute-heavy code. If wasmi exceeds the budget in item 5, stop and report; don't add wasmtime on your own.
- Reason for no JIT: on macOS a plug-in inherits JIT permission from the host app and can't grant it to itself, so a JIT engine can fail inside some hosts.
- Use `CompilationMode::Eager`. The wasmi default (`LazyTranslation`) translates functions on first call, which would happen on the audio thread. Compile on a background thread.

### 2. Storage: code embedded in the .kabl
- Embed the `.wasm` bytes in the patch, using the extra-data support from #124.
- Each embedded blob has a content hash (SHA-256), a module id and a module version. Instances reference the hash, so identical code is stored once.
- Enforce a size cap per blob (propose 2 MiB) and refuse to load anything larger, with a clear error.
- **No host imports except kabl's own ABI.** No WASI, not even read-only. Reject modules that import anything else at load time.
- First time a patch with custom modules opens: a one-time notice, "This patch contains custom code modules," with Open / Cancel. Remember the choice per content hash.

### 3. One instance per voice
- Yes. Each voice gets its own instance and linear memory.
- Cap memory per instance with wasmi `StoreLimits` (propose 4 MiB). Over the cap counts as a crash (item 4).
- Never instantiate on the audio thread. Keep a pre-built pool sized to the max voice count, and rebuild it off-thread when voice count or module changes.

### 4. Crash handling
- Trap, out-of-memory or invalid output → fade the module's outputs to zero over ~5 ms, then silence. Warning badge on the face, plus a toast (G4 toast pattern).
- A crash in any voice disables the module in **all** voices.
- **Invalid output counts as a crash:** any NaN or ±Inf in an output buffer. Never pass it downstream.
- Stays off until the user resets or reloads it. No automatic retry.

### 5. Time limit via fuel
- wasmi can't measure wall-clock time inside a call; use fuel metering (`consume_fuel = true`, off by default).
- At startup and after an audio-device change, calibrate fuel per µs on the user's machine and convert the time budget to fuel.
- Per-module cap: 25 % of the block period.
- **Global cap:** all custom modules together ≤ 50 % of the block period. When the total would exceed it, reduce each module's per-block fuel proportionally.
- Out of fuel → that block's output is silence (with the same short fade as item 4).
- **Repeated overruns = crash:** 8 consecutive out-of-fuel blocks trigger item 4. One-off overruns only log.
- **[verify]** fuel overhead: measure the benchmark from item 1 with fuel on and off and report both.

### 6. Faust: optional, cut first
- One optional PR: a Faust effect compiled to WASM, running through a thin adapter to kabl's ABI. It's useful as a test of whether the ABI is general enough. First thing cut if time is short.

### 7. Module version changes
- A patch saved with module v3 opened with v4 → defaults plus a notice ("Custom module X updated from v3 to v4: settings reset").
- **Keep the old state blob in the patch** alongside the new state when saving, so reopening with v3 restores it and D12 can add a `migrate(old_version, state)` export later without data loss.

---

## Additional decisions (not in the original list)

### 8. ABI is per block
- One call per audio block, never per sample.
- Audio buffers live in the module's linear memory. kabl copies inputs in and outputs out around each call; the module exports pointers/offsets at init.
- Parameters are read from a shared array in module memory, written by kabl before each block. Modulation and smoothing are applied by kabl exactly as for built-in modules, so custom modules feel the same.
- Minimal exports: `init(sample_rate, max_block) -> layout`, `process(n_frames)`, `reset()`; optional `state_save` / `state_load`. Document as ABI v0 in the PR; D12 freezes it.

### 9. Nothing allocates on the audio thread
- Compile, instantiate, grow memory and load state all happen off-thread. The audio thread only calls `process`.
- Modules may not grow memory during `process` (`memory.grow` → trap → item 4). They declare their memory size up front.
- Add a debug-build assertion (or allocator hook) that panics on allocation inside the audio callback while custom modules run.

### 10. Denormals **[verify]**
- The WASM spec requires full denormal handling, which is slow. wasmi likely runs float ops on the host CPU, so the host thread's flush-to-zero setting may apply. Test: feed a decaying feedback loop into silence and compare CPU time with flush-to-zero on vs off. Report the result; if denormals slow it down, add a tiny DC offset or document the module-side fix for D12.

---

## Acceptance for D11

- Benchmark report (item 1) with fuel on/off (item 5), on the dev machine and inside REAPER as a CLAP plugin.
- Tests: trap, infinite loop, memory over cap, NaN output, oversize blob, forbidden import, version bump. Each one: the synth keeps playing and the right warning shows.
- Audio-thread allocation check passes with 16 voices × 4 custom modules.
- Patch round-trip: save, reopen, identical audio (bit-exact for the same input).
- No regression in the existing perf gate.

---

# D11 follow-up (owner, 2026-11-07)

Gate: passed. wasmi only is confirmed (5.7× native, fuel +5 %, 16 voices = 1.24 % of block).

Flush-to-zero: option (a), engine-wide.
- RAII guard at audio-callback entry/exit, standalone and CLAP: set FTZ+DAZ (x86 MXCSR) / FZ (aarch64 FPCR) via inline asm, restore the previous state on exit. Never leave the host thread modified. Do NOT use _mm_setcsr (deprecated since Rust 1.75; Rust documents FP-env changes as UB because LLVM assumes the default state).
- Add a code comment explaining that UB caveat and that the golden renders (release build, FTZ on) are the guard against miscompiles.
- Before re-recording golden hashes: render every golden patch with FTZ off vs on and report the max |diff| per patch in dBFS. Accept if ≤ −140 dBFS. Any patch above that, stop and report.
- Explicitly test self-oscillating patches (resonant filter at max resonance, no input; feedback delays): confirm they still start and sound the same with FTZ on. If one relies on denormal noise to start, add a deterministic tiny excitation in that module instead.
- Remove any per-module denormal workarounds only after this passes.

Fuel calibration:
- Calibrate with the slowest fuel-per-µs code mix (worst case), so the fuel budget never exceeds the time cap.
- Add a wall-clock backstop: time each process() call; exceeding the per-module cap counts as an overrun even if fuel remains (same 8-in-a-row → crash rule).
- Report calibration values on the dev machine and inside REAPER.
