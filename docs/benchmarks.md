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

## `dyn Module` dispatch cost — static vs. `Box<dyn Module>`

See `docs/decisions.md` 2026-09-21 "Spike: `dyn Module` dispatch cost" entry. Same topology as
`patch_demo.rs` (4 voices x 5 modules + mixer + out, 22 instances), only difference is
`Box<dyn Module>` fields instead of concrete typed fields.

`cargo bench -p kabl-engine --bench dyn_dispatch_spike`, `taskset -c 0`, 3 runs:

| Run | static (µs/block) | dyn (µs/block) | ratio |
|---|---|---|---|
| 1 | 5.221 | 6.056 | 1.16x |
| 2 | 5.194 | 6.113 | 1.18x |
| 3 | 5.147 | 6.070 | 1.18x |

Correctness (`cargo test -p kabl-engine --test dyn_dispatch_spike`): bit-exact agreement over 200
blocks — dispatch mechanism doesn't change the signal.

**Note:** this run's static-dispatch number (~5.15-5.22µs) doesn't match the `patch_integration`
entry above (3.1-3.6µs) for the same `Patch::process_block` — re-ran `patch_integration` in the
same session and it now also reads ~5.15-5.18µs, confirming a container-baseline shift between
sessions (not a code regression). Trust the ratio, not either absolute number across sessions.

## Not yet measured

v1 milestone, needs the real compiler/module registry, not spike-scoped hand-rolled graphs:
ns/block for the brief's actual `tiny`/`classic`/`potato` benchmark patches (as opposed to these
spikes' stand-ins), a real potato-gate CPU percentage on Pi-4 hardware, jitter-gate soak, S3's
aarch64 numbers, S4 (wasmtime, optional).

## Modulation slice: reference patch, before/after (2026-09-23)

`cargo run --release -p kabl-ui --example bench_reference` (current) and the same `routeless`/
`bench` code built at e6db377 (baseline), runs interleaved. i7-13700H, 8 voices all held, median
ns per 64-sample block over 25 × 4000 blocks. See `docs/modulation-slice/README.md`.

| Run | baseline routeless | now routeless | now, 6 routes | now, 6 routes, key-trigger |
|---|---|---|---|---|
| 1 | 16 709 | 14 203 | 17 906 | 18 606 |
| 2 | 15 231 | 15 525 | 17 913 | 18 324 |
| 3 | 13 967 | 15 420 | 17 780 | 19 798 |
| 4 | 15 157 | 15 428 | 17 699 | 19 637 |

Audio-thread state carry per swap (`receive_swap`, 200 swaps): 2 829 ns median, 7 331 ns max.
Routeless difference is within run-to-run noise; routes cost ~2.4 µs/block here.

## Echo slice: audio callback with delays (2026-09-23)

`cargo run --release -p kabl-ui --example bench_echo [frames]`, `taskset -c 2`, i7-13700H,
48 kHz, 8 voices, 4 s delay lines full of signal. A new graph arrives every 4th callback, three
at once every 40th (compiled outside the timing). Worst observed callback, µs. Details:
`docs/echo/README.md`.

| frames (budget) | patch | steady max | max with swaps |
|---|---|---|---|
| 64 (1333) | interlocking / one delay / two delays | 137 / 64 / 46 | 171 / 294 / 546 |
| 256 (5333) | same | 251 / 297 / 339 | 619 / 738 / 955 |
| 1024 (21333) | same | 906 / 1019 / 1133 | 1422 / 1954 / 2203 |

One delay: about +11 µs median per 256 frames steady; about +80 µs per swap that carries it
(1.5 MB line copy). Pi 4 unmeasured.

## 2026-10-05 — D08 strict CLAP refresh

Product `f7157619836075d0d7704e59be5d66e6224890b1`, binary `3baa8ee3cc0f2f99e1823e0a741d7301713098f99e92dc84f7556b8d0483aca4`. JACK48k/256, Xvfb, PipeWire Dummy-Driver;12s scripted MIDI per case. External proxy process intervals only.

| Case | Process p99 upper µs per active instance | Process max µs per active instance | Backend ERR max |
| --- | --- | --- | --- |
| light-1-closed | 170.0 | 442.52 | 0 |
| light-1-open | 190.0 | 412.9 | 0 |
| light-2-closed | 190.0, 180.0 | 394.19, 394.98 | 0 |
| light-2-open | 210.0, 210.0 | 1927.54, 428.83 | 0 |
| dense-1-closed | 1730.0 | 2027.54 | 0 |
| dense-1-open | 2030.0 | 2708.06 | 0 |
| dense-2-closed | 1780.0, 1760.0 | 2677.4, 2104.53 | 0 |
| dense-2-open | 2540.0, 2460.0 | 4969.91, 5491.27 | 2 |

Dense two-open backend ERR2 remains; physical xruns and latency unmeasured. P99 bins10µs; arrivals include startup/teardown and are not execution time. Raw strict-profile timing-summary.json retains scopes.

## 2026-10-09 — Functional cables

`taskset -c 2 ./target/release/examples/cable_cost <patch>` (`crates/engine/examples/cable_cost.rs`), commit of this PR, i7-13700H, rustc 1.93.0, 48 kHz, 64-sample blocks, 8 voices, 40 000 blocks per run, fastest of 9 runs (the machine was shared with other builds; medians are printed too and run up to 2× higher on the dense patch). Offline `process_block` time, no audio device. Pi 4 unmeasured.

| Case | Plain | Functional | Difference |
|---|---|---|---|
| Minimal (clock, lfo, out), one cable, length 8 | 946–1 051 ns/block | 920–1 090 ns/block | within noise (−24…+39 ns) |
| `patches/sequence`, all 9 jack cables (length 8, chance 80 %) | 5 375–5 662 ns | 7 320–7 516 ns | +1.9 µs: about 210 ns per cable |
| `patches/composition`, all 57 jack cables (113 nodes with voice lanes) | 43 472–47 667 ns | 60 415–66 005 ns | +17–18 µs: about 300 ns per cable, 1.3 % of the 1 333 µs block budget |
| `patches/composition`, same cables but every step open and never rejected | 43 235 ns | 48 264 ns | +5 µs: about 45 ns per node, 88 ns per cable |

The node itself costs tens of nanoseconds (one 64-sample copy and a multiply or slew). The rest of the difference with real gating is downstream: a closed step feeds silence to filters and delays and changes what they compute. A plain cable costs nothing (no node). Making every jack cable of the densest shipped patch functional is far beyond real use; ten functional cables is under 1 % of the budget.

### Morph and glide

Same tool and machine (`KABL_MORPH=1 taskset -c 2 ./target/release/examples/cable_cost patches/composition`), one run of 9 on 2026-10-10 at the head of the morph-standard commit (a36e63c plus the test and blend-exactness change). `KABL_MORPH=1` gives every functional cable a second pattern of 5 steps, a morph of 0.5 and 20 ms of glide, so each pulse blends two steps and every level change slews.

| Case | Pattern only | Pattern, pattern B, morph 0.5, glide 20 ms |
|---|---|---|
| `patches/composition`, 57 jack cables, difference to plain | +16 494 ns/block (289 ns per cable) | +15 473 ns/block (271 ns per cable) |
| Minimal, one cable, node alone | +11 ns/block | +17 ns/block |

Morph and glide add nothing measurable: the blend is two table reads and a multiply at each pulse (not per sample), and glide only changes the slew rate of the existing audio path. The difference between the columns is inside run-to-run noise (the plain baseline moved 44 015 to 44 057 ns). The morph and glide edits are runtime values and `audio_path_does_not_allocate_with_morph_and_glide_running` asserts no allocation while they change.

### Routes into cable parameters

2026-10-10, the commit that adds `PortRef::CableParam`. Same tool and machine as above, with the new switch: `KABL_MORPH=1 KABL_ROUTES=1 taskset -c 2 ./target/release/examples/cable_cost patches/composition`. `KABL_ROUTES=1` routes one macro (amount 0.5) into the morph of every functional jack cable (57 routes), and in the minimal patch swaps a jack from a second lfo for a route from it into the cable's morph. One run of 9, fastest. The machine was shared: the plain baseline moved between 44 000 and 63 000 ns/block across runs, so read the differences with that spread.

| Case | Without routes | With routes | Difference |
|---|---|---|---|
| `patches/composition`, 57 functional cables with morph and glide | 73 949 ns/block (cables +18 008 over plain) | 79 358 ns/block | about +5 µs, 95 ns per route (a −55 ns result in another run: inside the noise) |
| Minimal, one cable, an lfo to `out.right` as a jack against the same lfo as a route into morph | 1 529 ns/block | 1 535 ns/block | +6 ns |

A route costs a source buffer read per pulse (not per sample) plus one 64-sample copy in its cable's step, and nothing when the cable has none. Fifty-seven routes, one in every functional cable of the densest shipped patch, are well under 0.5 % of the 1 333 µs block budget. The audio path stays allocation-free while an amount is edited (`audio_path_does_not_allocate_with_routes_running_and_edited`).
