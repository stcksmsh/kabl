# Benchmarks

History of measured numbers. Every PR touching the engine reports ns/block for the benchmark
patches before and after (brief section 3). Filled in as benchmark patches exist (v1 milestone);
until then, spike-specific measurements are logged here under their own heading.

## Spike S1 — graph swap / crossfade

See `docs/decisions.md` 2026-09-21 "Spike S1" entry for method and the click-metric dead end.
Numbers from `crates/engine/tests/spike_s1_swap.rs` (`cargo test -p kabl-engine -- --nocapture`),
run on this session's container (not the Pi-4-class reference machine — no Pi available here;
potato-gate emulation is for the v1 milestone's S2 spike, not this one).

Scenario: 4-voice saw chord (A2/C3/E3/A3) through one shared TPT SVF lowpass (2 kHz, Q≈0.3
equiv.), cable depth toggled 1.0 ↔ 0.7, 100 times, 15 ms equal-power crossfade per swap, 48 kHz /
64-sample blocks, ~2.67 s total render (128,000 samples).

| Metric | Value | Gate | Result |
|---|---|---|---|
| Steady-state residual (engine vs. hard-switch reference, outside crossfade windows) | 0.0 (bit-exact) | < -80 dBFS | pass, by a wide margin |
| Boundary curvature (max \|2nd difference\| at a swap boundary) | 0.0318 | ≤ 1.5× typical in-chord curvature | pass — came in *below* typical (0.58×) |
| Typical in-chord curvature (max \|2nd difference\| elsewhere in the render) | 0.0546 | — | reference value |
| Diagnostic: excess jump a naive hard switch injects at the transition, isolated from ordinary waveform movement | avg 0.00107, max 0.00312 | — (diagnostic only, see decisions.md) | small at this filter cutoff; not used as a gate |
| Allocation during audio-thread path (100 swaps × 20 blocks each, `assert_no_alloc`-wrapped) | 0 | 0 | pass |
| `basedrop::Owned<T>` drop timing (separate isolated test, 8 drops) | 0 destructors run before `Collector::collect()`, 8 after | inline drop count == 0 | pass |

## Spike S2 — control-rate tier (block-held scalars) vs. potato gate

See `docs/decisions.md` 2026-09-21 "Spike S2" entry — includes why this does **not** produce a
potato-gate pass/fail verdict (no Pi-4 hardware, no `cpufreq` in this cloud container; owner
chose "report raw + flag approximate" over a synthetic IPC conversion factor).

Patch: `crates/engine/src/potato.rs`'s `PotatoPatch` — 4 voices × (osc.va, filter.svf, env.adsr,
vca, ringmod) = 20 voice-rate modules + global lfo/mixer/out = 3 more. `cargo bench -p
kabl-engine --bench s2_potato`, `taskset -c 0` (shared cloud VM — not a dedicated core; criterion
reported 23-31% outliers across runs from neighbor contention), 5 runs:

| Run | naive (µs/block) | optimized (µs/block) |
|---|---|---|
| 1 | 4.01 | 3.13 |
| 2 | 3.80 | 3.30 |
| 3 | 3.51 | 3.48 |
| 4 | 4.08 | 3.46 |
| 5 | 3.93 | 3.48 |
| **mean** | **3.87** | **3.37** (~13% lower) |

Correctness (`cargo test -p kabl-engine optimized_converges`): naive vs. optimized converge to
-41.4 dB relative error once the ADSR settles — the optimization doesn't change what the patch
sounds like.

Both paths: well under 0.3% of one pinned Xeon @2.1GHz core at 48kHz/64-sample blocks — not a
meaningful comparison to the 50%-of-a-Pi-4-core gate; flagged, not claimed as a pass.

**Open item:** run the same bench on real Raspberry Pi 4 hardware for a number the gate can
actually be checked against.

## Spike S3 — SIMD voice batching (f32x4) vs. scalar

See `docs/decisions.md` 2026-09-21 "Spike S3" entry — **missed its >=2.5x target**, reported
honestly rather than picked favorably. Correctness is solid (bit-exact vs. scalar); the
shortfall is in raw speedup, with a plausible-but-unproven hypothesis (PolyBLEP's branchless
form pays for both regions unconditionally vs. scalar's near-free predicted branch) — see
decisions.md for the failed isolation attempt and why it wasn't trustworthy enough to report as
a root cause.

`cargo bench -p kabl-engine --bench s3_simd_voices`, `taskset -c 0`, same shared-VM noise caveat
as S2. Two granularities (added per-block after per-sample to rule out call-overhead as the
explanation — it wasn't, same ratio both ways):

| Granularity | runs | scalar mean (ns) | SIMD mean (ns) | mean ratio | range |
|---|---|---|---|---|---|
| per-sample | 6 | 8.57 | 5.57 | 1.54x | 1.31x-1.78x |
| per-block (64 samples) | 5 | 579.4 | 349.0 | 1.66x | 1.53x-1.77x |

Correctness (`cargo test -p kabl-engine simd_matches_scalar`): max \|scalar - SIMD\| = 0.0 over
48,000 samples — bit-exact, as it should be (SIMD batching is a pure implementation strategy,
not an approximation, unlike S2's control-rate tier).

**Not measured:** aarch64 (brief section 11 asks for "x86 and aarch64 if available" — only
x86_64 available in this container, same hardware gap as S2's Pi-4 number).

## First real-Module patch (`patch_demo.rs`) — integration spike

See `docs/decisions.md` 2026-09-21 "First patch built from real Module trait objects." 4 voices
x (midi.in, osc.va, filter.svf, env.adsr, vca) + mixer + out = 22 module instances, wired through
`ProcessIo`, not hand-rolled `dsp::` calls like S1/S2/S3.

`cargo bench -p kabl-engine --bench patch_integration`, `taskset -c 0`, 3 runs:

| Run | ns/block |
|---|---|
| 1 | 3585 |
| 2 | 3114 |
| 3 | 3321 |

Correctness (`cargo test -p kabl-engine --test patch_integration`): C-major chord (C4/E4/G4/C5)
sustains at RMS 0.22, zero NaN/Inf across a 2s render, decays to RMS 0.008 (<5% of sustain)
within 1s of release. WAV sent to the owner.

**Not measured:** `dyn Module` dispatch cost — this patch uses concrete typed fields
(static dispatch), not the heterogeneous `Box<dyn Module>` collection the real compiler will
need. See decisions.md's "Real open item" for why that matters before trusting this number as
predictive of the real compiler's cost.

## Not yet measured

v1 milestone, needs the real compiler/module registry, not spike-scoped hand-rolled graphs:
ns/block for the brief's actual `tiny`/`classic`/`potato` benchmark patches (as opposed to these
spikes' stand-ins), a real potato-gate CPU percentage on Pi-4 hardware, jitter-gate soak, S3's
aarch64 numbers, S4 (wasmtime, optional).
