# Decisions

Every non-trivial choice + the alternative rejected. Newest last.

## 2026-09-21 — Workspace layout

Set up per brief section 5: `crates/{core,engine,modules,cables,pedals,learn,ui,standalone,clap}`,
`benches/`, `tests/golden/`, `lessons/`, `docs/`. `core` has zero audio/UI deps — verified by
its `Cargo.toml` carrying no dependency outside `serde`/`serde_json`/`toml`.

`Cargo.toml` release profile: `panic = "abort"`. Alternative: default unwind. Chose abort because
the eventual CLAP wrapper (`crates/clap`, nih-plug) crosses an FFI boundary with the host;
unwinding across that boundary is UB regardless, so abort-on-panic is the safer default. Revisit
if a crate in the graph needs unwind (e.g. `catch_unwind` for host robustness) — that would need
`panic = "unwind"` and an explicit catch at the FFI boundary instead.

Session note: the harness that ran this session mandates a single working branch
(`claude/new-session-53hkvz`), which conflicts with the brief's "each spike in its own branch"
(section 11). Resolved by doing spike work as separate, clearly-labelled commits on the one
branch instead of separate git branches. Functionally equivalent for review purposes (each
commit is bisectable and has its own decisions.md entry); flagging as a deviation from the
brief's literal instruction.

## 2026-09-21 — Dependency versions (checked via `cargo info`, crates.io, this date)

| Crate | Version | Used by | Why |
|---|---|---|---|
| `serde` | 1.0.229 | core | op log / state (de)serialization |
| `serde_json` | 1.0.151 | core | `log.jsonl` |
| `toml` | 1.1.6+spec-1.1.0 | core | `meta.toml` |
| `rtrb` | 0.4.0 | engine | wait-free control→audio queues |
| `basedrop` | 0.1.3 | engine | deferred drop of old compiled graphs off the audio thread |
| `assert_no_alloc` | 1.1.2 | engine (tests) | proves `process()` allocates nothing |
| `proptest` | 1.11.0 | core (dev) | op-log replay/undo property tests |
| `hound` | 3.5.1 | engine (dev) | writing spike renders to WAV for inspection |
| `tempfile` | 3.27.0 | core (dev) | file-format round-trip test needs a scratch directory |
| `criterion` | 0.8.2 | engine (dev, added 2026-09-21 for spike S2) | ns/block measurement for the control-rate-tier comparison; brief section 13 names it for the real `tiny`/`classic`/`potato` benches anyway, so bringing it forward rather than hand-rolling timing for S2 and redoing it later |
| `wide` | 1.7.1 | engine (added 2026-09-21 for spike S3) | portable `f32x4` SIMD, per brief section 14's explicit suggestion; see "Spike S3" entry for why over `std::simd`/`pulp` |
| `cpal` | 0.18.2 | standalone (added 2026-09-21) | cross-platform audio output, brief section 14's named choice. Needed `libasound2-dev` installed in this container just to build (not run) on Linux — see "Standalone binary" entry |
| `midir` | 0.11.0 | standalone (added 2026-09-21) | cross-platform MIDI input, brief section 14's named choice |

Not yet added (deferred to the milestone that needs them, per brief section 14): `cpal`, `midir`
(standalone, v1 UI milestone), `egui` (ui crate), `nih-plug` (crates/clap, v5), `fundsp`
(optional, not needed yet).

Structural note: the brief's tree (section 5) shows a top-level `benches/` directory. Cargo only
auto-discovers bench harnesses inside each crate's own `benches/` folder, so the actual criterion
harness lives at `crates/engine/benches/s2_potato.rs`. Reading the top-level `benches/` as "where
benchmark *patches* (fixtures/data) might live later" rather than where harness code goes — not
a deviation requiring a decision, just how Cargo physically requires it.

## 2026-09-21 — Spike S1 (graph swap): scope and result

Question (brief section 11): can compiled graphs be swapped with state carry-over and no click?
Pass criterion: swap gate passes on a sustained saw→SVF chord while toggling a cable 100×.

**Implementation** (`crates/engine/src/dsp.rs`, `graph.rs`, `swap.rs`):
- 4-voice chord, each voice a PolyBLEP saw (Välimäki et al.), summed and scaled by a "cable"
  depth parameter, into one shared TPT state-variable lowpass filter (Zavalishin, *The Art of
  VA Filter Design*).
- `CompiledGraph` is the audio-thread-owned unit. `recompiled_with_depth` builds a new graph
  instance carrying over oscillator phase and filter state (`ic1eq`/`ic2eq`) from the old one —
  this is the "ringing filter survives a repatch" requirement (brief section 7.5), simplified:
  full `ModuleId`-keyed save/load state doesn't exist yet since there's no module registry yet
  (that's v1 milestone work); the spike only needs to prove *a* graph can carry state across
  a swap, not the general mechanism.
- `Engine::request_swap` builds the new graph and holds both old and new for a fixed crossfade
  window (constant `CROSSFADE_SAMPLES`, currently 15 ms at 48 kHz = 720 samples, per brief
  section 7 "10–20 ms equal-power crossfade"), running both in parallel and blending with an
  equal-power (sin/cos quarter-wave) curve, exactly as section 7 "Swap" specifies.
- Old graph is held in a `basedrop::Shared`; dropping the engine's reference to it on what the
  test treats as the audio thread does not deallocate — deallocation only happens when
  `Collector::collect()` runs (simulating the control thread reaping garbage). Verified with a
  drop-counting wrapper: 0 drops observed while `assert_no_alloc` scope is active and before
  `collect()`, exactly 1 drop after.
- Whole `Engine::process_block` (including the crossfade blend) runs inside
  `assert_no_alloc::assert_no_alloc!`, wired to `assert_no_alloc::AllocDisabler` as the test
  binary's global allocator. No allocation occurred across 100 swaps × ~50 blocks each.

**Result — spike passed.** See `docs/benchmarks.md` for numbers. Two things had to be measured,
not assumed:

- **No bleed outside the crossfade window.** Compared the crossfaded output against a
  hard-switch (instant, no crossfade) reference applying the identical 100 depth toggles at the
  identical samples. Outside each ~15 ms crossfade window the two renders are bit-for-bit
  identical (residual = 0.0, i.e. -infinity dBFS, against the -80 dBFS gate) — not approximately
  close, exactly equal. This isn't a coincidence: with this DSP shape (oscillator phase doesn't
  depend on `cable_depth`, only the mix→filter stage does), the crossfaded graph and the hard-
  switch reference are provably running the identical arithmetic from the identical starting
  state once a swap finishes, so bit-exactness is the expected result, not a lucky pass.
- **No click at the boundary.** First attempt compared raw sample-to-sample delta at the
  transition sample against the hard-switch reference's delta there, expecting the hard switch
  to show an obviously bigger jump. It didn't reliably — ratio ranged 0.1×-1.3× across the 100
  toggles, i.e. sometimes the "hard switch" delta was *smaller*. Root cause: at this filter's
  cutoff (2 kHz at 48 kHz SR), the TPT SVF's `a3` coefficient (the one that lets a sudden input
  change show up in the output) is ~0.014, so a 0.3 depth-jump's instantaneous contribution to
  the output is ~0.004 — the same order of magnitude as ordinary per-sample waveform movement in
  a moving chord. Comparing raw deltas against a synthetic reference wasn't a stable "click"
  signal for this cutoff; it wasn't a bug in the swap mechanism, it was a bad choice of metric.
  Replaced it with a self-contained curvature check: a click is an anomalous *kink* (a jump in
  slope beyond what the waveform produces on its own), so the test compares engine's own
  second-difference at each swap boundary against the worst curvature the sustained chord
  produces elsewhere on its own. Result: boundary curvature (0.032) came in *below* typical
  in-chord curvature (0.055) — 0.58×, well inside the 1.5× tolerance — meaning the crossfade
  introduces no detectable kink at all, not just one small enough to pass a loose bound.

**Known simplification, flagged for v1 proper:** state carry-over here is hand-written for this
specific 2-node graph shape. The general mechanism (`ModuleId`-keyed `save_state`/`load_state`
dispatched through a module registry, brief section 8) is v1 milestone scope, not spike scope.
The spike proves the *swap+crossfade+deferred-drop* mechanism works; it does not yet prove the
general per-module state carry-over will scale to an arbitrary graph — that's the compiler's job
in v1.

## 2026-09-21 — Spike S2 (control-rate tier): scope, method, and the hardware gap

Question (brief section 11): does block-held scalar classification meet the potato gate?
Pass criterion: 20-module, 4-voice patch < 50% of one Pi-4-class core.

**Hardware blocker, raised to the owner before building anything.** This container is a 4-core
x86_64 Xeon @ 2.1GHz cloud VM: no ARM, no `cpufreq` (no scaling driver exposed — checked
`/sys/devices/system/cpu/cpu0/cpufreq`, doesn't exist), no `cpupower`. The brief's own fallback
("`taskset` + `cpufreq`, document method") needs `cpufreq`, which isn't available here either.
Asked the owner how to handle it; chose "pin with `taskset`, report raw numbers, flag as
approximate" over deriving a synthetic Xeon:Cortex-A72 IPC conversion factor. **Result: this
spike does not produce a potato-gate pass/fail verdict.** It answers S2's actual engineering
question (does the optimization reduce CPU, and does it stay correct) and reports raw numbers;
turning that into a Pi-4 percentage needs real Pi-4 hardware, still open.

**Scope grew past S1's shape, confirmed with the owner first.** S1 reused a 2-node graph;
answering S2 for real needs signals that are actually classifiable as slow (an LFO, a
sustain-held envelope) sitting next to genuinely audio-rate ones. Built (`crates/engine/src/
potato.rs`) a stand-in for the brief's `potato` benchmark patch: 4 voices × (`osc.va`,
`filter.svf`, `env.adsr`, `vca`, `ringmod`) = 20 voice-rate modules, one global `lfo` (2 Hz, well
under the brief's named "~100 Hz" control-rate threshold) feeding both the filter's cutoff
modulation and the ring-mod depth, plus `mixer` and `out` = 3 global-rate modules. Still a
hand-rolled fixed topology, not the general flat-schedule compiler (that's v1 milestone scope) —
same simplification S1 made for state carry-over.

**Method:** two process paths share the same DSP building blocks (`Saw`, `Svf` from S1, new
`Lfo`/`Adsr` in `dsp.rs`). `process_naive` computes the LFO into a full 64-sample buffer every
block (64 `sin()` calls) and re-runs the ADSR's one-pole recurrence every sample even once it's
converged. `process_optimized` computes the LFO once per block (1 `sin()` call, shared by all 4
voices) and skips the ADSR recurrence entirely once `|level - target| < 1e-5`, holding the
converged scalar instead — exactly brief section 7's two named examples ("LFOs below ~100 Hz",
"envelopes in sustain").

**Correctness gate (checked before trusting any benchmark number):** with the patch's gate held
throughout (a sustained chord, matching the brief's own swap-gate scenario), naive and optimized
outputs converge to -41.4 dB relative error once the ADSR settles (`crates/engine/tests/
spike_s2_potato.rs`) — the optimization changes *how* the patch is computed, not what it sounds
like. This is the Live-tier approximation brief section 7 allows explicitly ("quality tiers ...
change fidelity, never character") one level down, at the compiler's signal-classification tier.

**Benchmark numbers — noisy, reported honestly rather than cherry-picked.** `taskset -c 0`
pins the process to one core but this is a shared cloud VM, not a dedicated machine — neighbor
contention shows up as 23-31% "outliers" in criterion's own report. Five runs:

| Run | naive (µs/block) | optimized (µs/block) |
|---|---|---|
| 1 | 4.01 | 3.13 |
| 2 | 3.80 | 3.30 |
| 3 | 3.51 | 3.48 |
| 4 | 4.08 | 3.46 |
| 5 | 3.93 | 3.48 |
| **mean** | **3.87** | **3.37** |

Optimized is consistently faster or at worst tied (run 3, ~1% apart — within this container's
noise floor); mean reduction ~13%, individual runs ranged 1-25%. At 48kHz/64-sample blocks
(1333µs/block budget), both paths are under 0.3% of one pinned Xeon core — nowhere close to
exercising this container's headroom, which is expected and uninformative: a modern server core
at 2.1GHz is a different machine class from a Cortex-A72 at 1.5GHz, not just a clock-speed
difference, and per the owner's answer this spike isn't extrapolating one number from the other.
**Open item, needs the owner or real Pi-4 hardware:** run `taskset -c 0 cargo bench -p
kabl-engine --bench s2_potato` (or the eventual real `potato` benchmark patch once the v1
compiler exists) on an actual Raspberry Pi 4 to get a number the gate can actually be checked
against.

## 2026-09-21 — Spike S3 (SIMD voice batching): missed its target, reporting honestly

Question (brief section 11): does f32x4 voice batching beat scalar by >=2.5x on osc+filter?

**Result: no. Measured ratio ~1.5-1.7x (range 1.3-1.8x across 11 runs), consistently below the
2.5x target.** Reporting this straight rather than picking a favorable run — brief section 17,
"measure, then claim."

**`wide` crate chosen** (brief section 14: "portable SIMD crate or std::arch behind a small
abstraction, document the choice") over `std::simd`: this workspace runs stable Rust (`rustc
1.94.1`, verified via `rustc --version`, no `-nightly` suffix) and `core::simd`/`std::simd`
(`#![feature(portable_simd)]`) is nightly-only. `wide` v1.7.1 (checked via `cargo info`, this
date), MSRV 1.89, picks SSE2/NEON/wasm128 per target behind a safe API — exactly what section 14
asks for. Over `pulp` (also considered, `cargo info` checked same date): `wide`'s `f32x4` lane
model maps directly onto "4 voices per lane" with less API surface to learn for this narrow use.

**Method** (`crates/engine/src/simd_voices.rs`): `SawX4`/`SvfX4` batch the same PolyBLEP-saw/TPT-
SVF math as `dsp.rs`'s scalar `Saw`/`Svf`, one `f32x4` lane per voice. Filter cutoff/resonance
are shared across all 4 voices (same as S1/S2's patches), so their coefficients — which need
`tan()` — stay plain scalars broadcast with `splat`; only oscillator phase and filter state
(genuinely per-voice) are SIMD lanes. PolyBLEP's branch (`t < dt` / `t > 1-dt` / neither) has no
scalar equivalent in SIMD — `poly_blep_x4` computes both regions unconditionally and selects
with a mask.

**Correctness gate (checked first, tighter bar than S2's):** unlike S2's control-rate tier —
which brief section 7 explicitly allows to trade fidelity — SIMD batching is a pure
implementation-strategy change; it should reproduce the scalar path, not approximate it.
`crates/engine/tests/spike_s3_simd_voices.rs` confirms **bit-exact** agreement (`max_diff =
0.0`) over 48,000 samples. This part fully succeeded: batching is safe to use.

**A confound found and fixed along the way, additive, zero risk to S1/S2:** the first pass
recomputed SVF coefficients (the `tan()` call) every sample in both the scalar and SIMD paths,
identically wastefully in both — this doesn't change the fair comparison (both sides paid the
same tax) but it's not how a real engine would do it, so it was cleaned up regardless: split
`dsp::Svf::process_lowpass` into `SvfCoeffs::compute` (once, when cutoff/resonance actually
change) + `process_with_coeffs` (every sample, reusing them) — `process_lowpass` itself is now a
thin wrapper calling both, bit-identical behavior, `cargo test --workspace` confirms S1/S2
unaffected. Re-measuring after this split changed nothing about the ratio (see below) — the
compiler had apparently already hoisted the invariant `tan()` call in the original benchmark, so
this was a real code-quality improvement, not the fix for the shortfall.

**Ruled out call-overhead as the explanation.** First measurements were per-sample
(`Engine::next`-style, one call per sample) — worried that criterion's own per-call overhead at
a ~5-10ns granularity was compressing the ratio toward 1x. Added `process_block` (64
samples/call, matching how the real engine actually calls into voice processing per brief
section 7's flat schedule) to both paths and re-measured: **same ratio.** Not a granularity
artifact.

**Investigated but inconclusive: PolyBLEP's branchless cost.** Leading hypothesis is that
`poly_blep_x4` computing both regions' divisions unconditionally trades away some of the 4-lane
win against scalar's branch predictor, which skips both regions almost for free (the correction
only applies within `dt` of a phase wrap — roughly 2 samples out of a ~400-sample period at
these frequencies, so the scalar branch is >99% predictable and "no correction" is nearly free
there). Tried to isolate this by benchmarking the SVF filter alone (no oscillator, fixed
constant input, both scalar and SIMD) to separate "pure branchless arithmetic SIMD win" from
"PolyBLEP's SIMD branch tax." That micro-benchmark measured **both** the scalar and SIMD
filter-only paths at ~15-20x slower than their share of the combined osc+filter number — which
isn't physically sensible (isolating a cheaper part of a pipeline shouldn't make it slower than
the whole pipeline) and reads as a benchmarking artifact, not a real signal. Didn't chase it
further: that's `perf stat`/`perf record` territory, not a criterion micro-benchmark, and
disproportionate time for a spike. **So the PolyBLEP-branch hypothesis is plausible and
unproven** — flagging honestly rather than reporting a root cause I don't actually have evidence
for.

**Numbers** (`taskset -c 0`, same noise caveat as S2 — shared cloud VM):

| Granularity | runs | scalar (ns, mean of medians) | SIMD (ns, mean of medians) | mean ratio | range |
|---|---|---|---|---|---|
| per-sample | 6 | 8.57 | 5.57 | 1.54x | 1.31x-1.78x |
| per-block (64 samples) | 5 | 579.4 | 349.0 | 1.66x | 1.53x-1.77x |

**What this means for v1:** brief section 11 only names a fallback for S2 failing ("tighten the
tier, cap default poly at 4"); it's silent on what to do if S3 falls short. My read: SIMD
batching is still a real, verified-correct, ~1.5-1.7x win — worth keeping in the real compiler's
voice-rate processing even though it misses the 2.5x bar, since "smaller win than hoped" isn't
the same as "not worth it." Flagging as a call for the owner rather than deciding silently: keep
SIMD voice batching in v1's design at this ratio, or is 2.5x load-bearing for the potato gate
budget in a way that changes the decision? Also open: S3's brief text asks for "x86 and aarch64
if available" — only x86_64 is available in this container (same hardware gap as S2); no aarch64
measurement exists yet.

## 2026-09-21 — Skipping S4 (wasmtime), starting real v1 work

Brief section 11 marks S4 ("is a WASM sine oscillator RT-safe in the callback?") explicitly
deferrable, and section 10 already sequences sandboxed WASM modules last ("later, gated") behind
composite pedals (v4) and Faust->Rust (v4) — S4 only matters once that milestone is actually
being built, which is v4, three milestones away. Flagged this call in STATUS.md after S3 landed;
proceeding on it now rather than blocking further progress on a check-in for a spike whose
result won't change any v1 decision. Reversible: nothing in v1's design depends on S4's answer,
and it can be run whenever v4 (pedals) actually starts.

Starting real v1 (Engine) milestone work now: the module registry + `Module`/`ModuleInfo`
(brief section 8) first, since the compiler, cables, and UI all need it to exist before they're
buildable — it's the one piece on the v1 checklist nothing else is downstream-independent of.

## 2026-09-21 — Module registry: `dsp` relocated from `engine` to `modules`

`Saw`/`Svf`/`Lfo`/`Adsr`/`SvfCoeffs`/`poly_blep` lived in `crates/engine/src/dsp.rs` because
S1-S3 needed *some* real DSP to spike against, and `engine` was the only crate with content at
the time. Per brief section 5, `modules` is where "built-in modules + metadata" belong — `engine`
is the compiler/scheduler/swap layer, not a DSP owner. Now that `modules` is about to hold the
real `Module` trait and built-in implementations, leaving the primitives in the wrong crate would
mean either duplicating them or having `modules` depend on `engine` (backwards — `engine` is
supposed to depend on `modules`, not the other way around).

Moved `dsp.rs` to `crates/modules/src/dsp.rs` unchanged (`git mv`, pure relocation, zero logic
changes), added `kabl-modules` as an `engine` dependency, updated `graph.rs`/`potato.rs`/
`simd_voices.rs` imports from `crate::dsp::` to `kabl_modules::dsp::`. `cargo test --workspace`
confirms bit-identical behavior — this is a move, not a rewrite.

## 2026-09-21 — `Module` trait + `ModuleInfo` + `ProcessIo`, and `osc.va` as first real module

Built brief section 8 close to verbatim in `crates/modules/src/{info,io,module}.rs`:

- `ModuleInfo` (`info.rs`): all fields from the brief's sketch — `kind`, `name`, `category`,
  `rate`, `explain`, `lesson`, `requires`, `ports`, `params`, `quality`. `Category`/`Rate`/
  `PortType`/`PortDirection`/`Taper` are the enums the brief's comments name inline.
- `ProcessIo`/`Signal` (`io.rs`): `Signal::at(i)` is exactly the helper brief section 8 asks for
  ("provide helpers so module authors handle both [scalar and buffer] without branching per
  sample") — same idea S2's spike hand-rolled per-signal in `potato.rs`, now a reusable type any
  module can use instead of every module reinventing it.
- `Module` trait (`module.rs`): `info`/`prepare`/`process`/`reset`/`save_state`/`load_state`
  match the brief's signatures. `QualityConfig`/`QualityTier` and `StateWriter`/`StateReader` are
  deliberately minimal — brief section 7's full per-lever quality table and section 7.5's
  `ModuleId`-keyed save/load both need a compiler that doesn't exist yet to actually drive them.
  Building those out now would be designing ahead of their only caller. `QualityTier` has the
  brief's three tiers (Live/Studio/Render); `StateWriter`/`StateReader` are minimal key/value
  float traits, enough for a module to say "here's my float state" — matches what S1's spike
  hand-rolled (`CompiledGraph::recompiled_with_depth` cloning phase/filter state directly)
  through a trait instead of bespoke per-graph-shape code.

**`osc.va`** (`crates/modules/src/builtins/osc_va.rs`) is the first real `Module` impl, wrapping
`dsp::Saw`. **Only the saw waveform, no hard sync** — brief section 8's table entry is
"saw/square/tri/sine, PolyBLEP, hard sync input"; this proves the trait/`ProcessIo` design works
end-to-end, it doesn't finish the table entry. Square/tri/sine and hard sync are real, tracked
gaps (see STATUS.md), not a silent partial implementation.

Design choice worth flagging: `osc.va` takes frequency via a `pitch` input port (semitones, 1V/
oct per brief section 8's `PortType::Pitch` semantics) combined with a `base_hz` param (the
frequency at 0 semitones), rather than a single `freq_hz` param. This is what lets `midi.in`'s
future `pitch` output (brief's built-in table: "per-voice gate, pitch, velocity") drive an
oscillator by cable, and what a `cable` (v2) would modulate for pitch-bend/vibrato — a `freq_hz`-
only param wouldn't support either. Reversible: nothing else depends on this shape yet.

Tests (`crates/modules/tests/osc_va.rs`, 5 passing): output matches calling `dsp::Saw` directly
byte-for-byte (the trait plumbing doesn't change the math), `Signal::Buffer` inputs are read
per-sample correctly (not just `Signal::Scalar`), `reset()` zeroes phase, and — the one that
actually matters for brief section 7.5 — saving a module's state and loading it into a *fresh*
instance continues identically to the original, proving the save/load-state mechanism generalizes
what S1 hand-rolled.

## 2026-09-21 — Remaining 8 built-in modules

All 9 of brief section 8's v1 built-ins now have a `Module` impl (`crates/modules/src/
builtins/`). Two design choices worth recording, plus what's still partial:

- **`filter.svf`** exposes all three simultaneous outputs (LP/BP/HP), not a mode switch — the
  TPT topology computes all three from the same recurrence, so there's no cost to giving all
  three. Cutoff/resonance are each a param (base value) + an optional CV input: `cutoff_cv` is
  1V/oct-style (exponential, matching `osc.va`'s pitch handling), `resonance_cv` is linear
  additive. Not brief-mandated (it just says "cutoff + resonance CV"), a design choice.
  **Applies spike S3's finding for real**, not just as a war story: if both CV inputs (and both
  params) are `Signal::Scalar` for the block, coefficients are computed once and cached via
  `process_with_coeffs_multi`; only a genuinely per-sample-varying CV falls back to recomputing
  `tan()` every sample via `process_multi`. Test (`per_sample_cv_path_matches_scalar_path_when_
  cv_is_actually_constant`) confirms the fast/slow paths agree — the optimization doesn't change
  behavior, matching how S2's control-rate tier was verified.
- **`env.adsr`** reads its attack/decay/sustain/release params once per block (`.at(0)`), not
  per sample — same reasoning as the filter's fast path: these are knob values, not audio-rate
  CV, in the overwhelming common case, and `FullAdsr::set_params` computes 3 `exp()` calls each
  time it's invoked. A new `dsp::FullAdsr` (4-stage, real attack/decay/sustain/release) was added
  rather than extending `dsp::Adsr` in place — the latter's exact attack-only-toward-sustain
  behavior is what S1's `CompiledGraph` and S2's `PotatoPatch` are tested against; changing it
  would risk their already-passed, already-reported numbers for no reason.
- **`lfo`** implements all 5 waveforms (sine/tri/saw/square/S&H) — cheap since none need
  anti-aliasing at LFO rates, so this one built-in table entry is actually complete except for
  "sync," which the brief itself defers (no clock exists until v3). Waveform selection is a
  stepped float param (0..=4, rounded) since `ParamInfo` has no enum variant — the standard
  modular-software convention for a discrete choice, documented in the module's own doc comment
  since it's not obvious from `ParamInfo`'s shape.
- **`vca`**'s "exponential response" is approximated as `gain * gain` — cheap, common, not a
  precise psychoacoustic curve. Flagged as an approximation in the module's doc comment.
- **`mixer`** sums 4 channels with per-channel level knobs (default unity) — the brief only says
  "4 inputs"; per-channel levels are the minimal, universally-expected reading of "mixer," not
  scope creep.
- **`out`** has no `Module` output ports (nothing in the graph consumes the final mix) — instead
  exposes `left()`/`right()` accessors for the eventual audio callback (`standalone`, not built)
  to read.
- **`midi.in`** has no `Module` input ports either — its `gate`/`pitch`/`velocity` state is set
  by `note_on`/`note_off`, called directly for now since there's no real MIDI plumbing
  (`standalone`'s `midir` integration) yet. Monophonic-per-instance; a voice allocator (not built)
  would eventually route real MIDI across instances for polyphony. This is a stand-in with the
  same relationship to the real thing that S1's hand-rolled graphs have to the real compiler.
- **Still partial, tracked, not hidden:** `osc.va` (saw only, no hard sync/square/tri/sine) and
  `lfo` (all waveforms, no sync) — see their own module doc comments.

`dsp.rs` gained `SvfOutputs`/`Svf::process_with_coeffs_multi`/`process_multi` (additive, `Svf`'s
existing lowpass-only API untouched), `FullAdsr`/`AdsrStage` (new type, `Adsr` untouched),
`FullLfo`/`LfoWaveform` (new type, `Lfo` untouched) — nothing S1/S2/S3 depend on changed.
`FullLfo`'s S&H uses an inline xorshift32 PRNG (~5 lines) rather than adding the `rand` crate as
a real dependency for one PRNG call.

6 new test files (`crates/modules/tests/{dsp_extensions,filter_svf,env_adsr,lfo,vca_ringmod,
mixer_out_midi_in}.rs`), 21 tests total for this batch. Two caught real bugs before they shipped
— see the "Add DSP primitives" commit message: the ADSR convergence test needed far more samples
than first guessed, and the LFO range test needed a higher rate for enough S&H draws to be
statistically meaningful. Both were test-parameter bugs, not DSP bugs, but worth recording as a
reminder that a new test failing isn't automatically the code's fault.

## 2026-09-21 — First patch built from real `Module` trait objects

The thing S1-S3, the DSP extensions, and all 9 built-in modules were building toward: a patch
assembled from actual `Module` trait objects wired through `ProcessIo`, not hand-rolled `dsp::`
calls. Topology (`crates/engine/src/patch_demo.rs`), 4 voices: `midi.in` -> `osc.va` (pitch-
driven) -> `filter.svf` (lowpass tap) -> `vca` (gain from `env.adsr`, gated by the same
`midi.in`) -> `mixer` (4 channels) -> `out`. Same chord shape S1/S2 used, but every node this
time is a real module, and the wiring (which buffer feeds which port) is hand-written Rust
standing in for what the compiler will eventually generate from ops — same relationship S1's
`CompiledGraph` has to the real compiler, made explicit in the file's own doc comment so nobody
mistakes it for the real thing.

**Result: it plays, first real run, no debugging needed.** `cargo test -p kabl-engine --test
patch_integration`: a held C-major chord (C4/E4/G4/C5) renders with real sustained energy (RMS
0.22), no NaN/Inf anywhere in a 2-second render (brief section 13's robustness-fuzz idea,
applied to the real module chain for the first time), and decays to nearly silent (RMS 0.008,
<5% of sustain) within 1s of release — the `FullAdsr`'s release stage actually working through
the real `vca` gain-staging, not just in `dsp_extensions.rs`'s isolated test. WAV sent to the
owner.

**A real bug this caught, worth recording:** the first version of `patch_demo.rs`'s `vca` wiring
set the gain *param* to `1.0` and fed the envelope in as `cv` — but `vca`'s effective gain is
`clamp(gain_param + cv, 0, 1)`, so `1.0 + envelope` clamps to `1.0` regardless of what the
envelope is doing, silently defeating the envelope entirely (constant full volume, gate or no
gate). Caught before running anything, by re-reading the gain formula while wiring the params —
fixed by setting `gain` to `0.0` so the envelope's `cv` is the *only* thing setting amplitude.
Exactly the kind of integration bug component-level tests (which all passed) can't catch — each
piece was individually correct, the combination wasn't. This is the concrete case for why this
spike existed at all: proving composition, not just units.

**Numbers** (`crates/engine/benches/patch_integration.rs`, `taskset -c 0`, 3 runs): 3.08-3.60µs/
block for the whole 22-module-instance patch (4 voices x 5 modules + mixer + out) — cheaper than
S2's 20-module hand-rolled `potato` patch (3.37-3.87µs), plausibly because this patch's `env.adsr`
and `filter.svf` both apply the coefficient-caching fast path from decisions.md's "Remaining 8
built-in modules" entry, while `potato.rs`'s naive/optimized comparison was deliberately testing
the *without*-caching case for half of it. Not a controlled comparison (different topologies,
different module counts per voice) — a data point, not a benchmark claim. Same hardware caveat
as S1-S3: no Pi-4, no percentage claim.

**Real open item this surfaces:** every module call here is statically dispatched — `Voice` holds
concrete typed fields (`MidiIn`, `OscVa`, ...), not `Box<dyn Module>`. The real compiler needs a
heterogeneous collection of module instances (a patch has an arbitrary mix of module kinds), which
means `dyn Module` trait objects and virtual dispatch — a real cost (vtable indirection, no
inlining across the call) this benchmark doesn't capture at all. Flagging for whoever builds the
compiler: measure `dyn Module` dispatch overhead before assuming this patch's ns/block number
predicts anything about the real compiler's.

## 2026-09-21 — `core`: op log inverse simplifications

`Entry.inverse` is a single `Op`, per the brief's exact struct (section 6) — no new `Op` variant
was added for this (scope discipline, section 2).

- `RemoveModule`'s inverse is `AddModule { id, kind, pos }` using the module's state at removal
  time. This restores the module and its position but **not** its accumulated `SetParam`
  history — a single `Op` can't carry an arbitrary param map back. Acceptable for v1: removing a
  module mid-session and undoing it will restore it with default params, not the params it had.
  Flagging as an open item — if this turns out to matter in practice (owner removes/undos
  modules with tweaked params often), the fix is a dedicated snapshot-carrying inverse, which
  needs either a new (internal-only, not user-authored) `Op` variant or a side-table keyed by
  seq. Parked, not decided.
- `Annotate` and `Snapshot` are no-ops against `PatchState` (the graph: modules/cables/params).
  They exist in the log for display/history purposes (construction replay can show "user typed
  a note here") but don't affect the replayable graph state, so their inverse is themselves and
  they're excluded from the `PatchState` equality the property tests check. This keeps the
  property test (`replay(log) == state`) meaningful without inventing removal semantics for
  free-text annotations that the brief doesn't specify.
- `SetParam`'s inverse for a param that never had a prior value (first `SetParam` on a fresh
  target) falls back to `0.0` — `Op::SetParam` has no variant for "unset", so there's no way to
  express "restore to nonexistent." Undoing a param's very first `SetParam` therefore leaves it
  present at `0.0` rather than removed from the map. Matches how most synth params treat 0 as a
  sane center/off default; flagging in case a param exists where 0.0 isn't a safe default (e.g.
  a param whose range doesn't include 0) — would need per-param default metadata from
  `ModuleInfo` (brief section 8) to fix properly, which doesn't exist yet in `core` since `core`
  has no module registry (by design, section 5: "no audio deps").
- File format (`crates/core/src/format.rs`): `checkpoint.json` is written on save but **not**
  consulted on load yet — load always full-replays `log.jsonl` from the start. Checkpoint-based
  partial replay (brief section 6, "replay starts from nearest checkpoint") is deferred until a
  real patch's log is long enough for full replay to be measurably slow. Correct-but-unoptimized
  now; measure before optimizing (brief section 17).

## 2026-09-21 — Spike: `dyn Module` dispatch cost, measured before the compiler

STATUS.md's handover left this open: does `dyn Module` dispatch need its own spike before the
real compiler is built around it, or is measuring-as-you-go for the compiler enough? Chose the
spike — cheap to answer, and the compiler's central data structure (a heterogeneous collection of
module instances, arbitrary kind mix per patch) has no static-dispatch alternative, so this isn't
a design choice to revisit, just a cost to know up front.

**Method** (`crates/engine/src/dyn_dispatch_spike.rs`): `DynPatch`/`DynVoice` are a byte-for-byte
copy of `patch_demo.rs`'s `Patch`/`Voice` topology and wiring (same 5-module voice chain, same
params, same `ProcessIo` calls) — the *only* change is `Voice`'s concrete typed fields (`MidiIn`,
`OscVa`, ...) become `[Box<dyn Module>; 5]`. Correctness gate first, same reasoning as S3 (a
dispatch-mechanism change should reproduce the original exactly, not approximate it):
`tests/dyn_dispatch_spike.rs` runs both patches for 200 blocks and asserts bit-exact equality —
passed first try, as expected (identical arithmetic, only the call mechanism differs).

**Numbers** (`benches/dyn_dispatch_spike.rs`, `taskset -c 0`, 3 runs each; also cross-checked
against a fresh `patch_integration` re-run for consistency):

| Run | static (µs/block) | dyn (µs/block) |
|---|---|---|
| 1 | 5.221 | 6.056 |
| 2 | 5.194 | 6.113 |
| 3 | 5.147 | 6.070 |

Ratio: dyn dispatch costs **~17-19% more** than static dispatch for this 22-module-instance patch
(mean ~1.176x). Tight across all 3 runs (range 1.16x-1.19x) — much less noisy than S2/S3's numbers,
plausibly because this measures a fixed, purely-CPU-bound code-shape difference rather than a
scheduler-classification win that depends on signal content.

**Cross-check note:** this run's static-dispatch number (~5.15-5.22µs) is noticeably higher than
the `patch_integration` entry logged earlier this session (3.1-3.6µs) for the *identical*
`Patch::process_block`. Re-ran `patch_integration`'s own bench just now to check: it now reports
~5.15-5.18µs too — i.e. the container's baseline shifted between sessions (shared cloud VM, same
noise caveat as S2/S3), not a regression in `patch_demo.rs`. The internal comparison (static vs.
dyn, same run, same container state) is what's trustworthy here, not either absolute number in
isolation.

**What this means for the compiler:** `Box<dyn Module>` is the only viable representation for a
patch with an arbitrary mix of module kinds — this was never going to change the plan. The number
says the real compiler's ns/block will run ~15-20% higher than `patch_demo.rs`'s static-dispatch
figure predicted, not a different order of magnitude. Not a blocker, not a reason to look for an
alternative (an enum-of-module-kinds dispatch table was considered and rejected: it would need a
match arm added by hand for every new module kind including pedals/composites in v4, defeating
brief section 4.1's "one module interface for built-in, composite, and code modules" — trading a
~17% ns/block cost for permanently coupling the compiler to a closed module-kind list is the wrong
trade). Proceeding with `Box<dyn Module>` in the compiler design.

## 2026-09-21 — Melody through `patch_demo.rs`, not just a held chord

Owner asked to hear the real-module patch play a melody rather than a static chord, before
continuing to the compiler. `crates/engine/tests/melody.rs`: same `Patch` as
`patch_integration.rs`, but `Patch::note_on`/`note_off` (new, additive — `note_on_chord`/
`note_off_chord` untouched) address a single voice instead of all four, called in sequence.
"Twinkle Twinkle Little Star"'s opening phrase (C C G G A A G, semitones `[0,0,7,7,9,9,7]` from
`BASE_HZ`), ~280ms hold + ~150ms gap per note on voice 0, voices 1-3 silent.

No real MIDI input exists (`standalone` is still a stub) — this is the same `note_on`/`note_off`
stand-in `midi_in.rs` already documents, just sequenced instead of held once. Confirms nothing
about `midi.in`'s design assumed "one chord, held once": per-voice control, re-triggering, and a
gap-then-new-note sequence all worked without changes to any of the 9 modules.

Test asserts each note's hold window has real energy and each gap dips measurably below it —
evidence of distinct notes, not one smeared tone (gaps don't fully silence: 150ms against a
300ms release time constant is expected legato, not a bug). WAV sent to the owner.

## 2026-09-21 — Module registry (`crates/modules/src/registry.rs`)

Needed before the compiler could turn a `ModuleState.kind: String` into a `Box<dyn Module>` —
nothing previously mapped kind strings to constructors; every test imported each type directly.
`KNOWN_KINDS: &[&str]`, `create(kind) -> Option<Box<dyn Module>>` (a `match` over the 9 kinds,
`None` for unknown — a normal, expected outcome for a not-yet-patched or future kind, not a bug
to panic on), `all_infos() -> &'static [&'static ModuleInfo]`, `info_for(kind)`. Plain `match`
over dynamic registration/`inventory`-style linkme tricks: 9 fixed kinds, no plugin loading in
v1, and v4's composite/code modules will need a different registration path anyway (they're
authored patches, not Rust types) — no reason to pay dynamic-dispatch-at-startup complexity now
for a problem v4 will have to solve differently regardless.

`all_infos()` initially tried to build `[&ModuleInfo; 9]` fresh inside the function body — failed
(E0515: can't return a reference to a temporary). Fixed by hoisting to a `static ALL_INFOS`.

## 2026-09-21 — `Module: Send + Any`, `as_any`/`as_any_mut`

The compiler needs to call module-specific methods (`MidiIn::note_on`, later `Out::left/right`)
through a type-erased `Box<dyn Module>` — the only way back from `&dyn Module` to a concrete type
is `std::any::Any` downcasting. Added `Module: Send + std::any::Any` and two required methods,
`as_any`/`as_any_mut`.

Tried a default body (`fn as_any(&self) -> &dyn Any { self }`) first — doesn't compile without
`where Self: Sized`, and `Sized` makes the method uncallable through `dyn Module` (defeats the
purpose; confirmed via the actual E0277, not assumed). So both methods are required, not
defaulted, and got the same 2-line body copy-pasted into all 9 `builtins/*.rs` impls. Mechanical
and repetitive, but there's no trait-level way around it in current Rust.

## 2026-09-21 — Flat-schedule compiler v1 (`crates/engine/src/compile.rs`)

The actual compiler: `compile(&PatchState, sample_rate, voice_count) -> Result<CompiledPatch,
CompileError>`, turning ops-authored patches into a running graph the way `patch_demo.rs` was
hand-wired. Proven end-to-end in `crates/engine/tests/compile.rs`: the same 5-stage voice chain
`patch_demo.rs` hard-coded (`midi.in -> osc.va -> filter.svf -> env.adsr/vca -> out`), built from
`PatchState` ops instead, compiled, driven via `as_any_mut` downcast MIDI triggering — output
matches `patch_demo.rs`'s numbers exactly (`sustain_rms=0.2243, tail_rms=0.007959` both ways).
WAV sent to owner (`compiler_chord.wav`).

Mechanism: resolve each module's `ModuleInfo` via the new registry (unknown kind ->
`CompileError::UnknownKind`); build cable adjacency from `PatchState.cables`; Kahn's-algorithm
topo sort over modules (leftover un-orderable nodes -> `CompileError::Cycle`, see below);
validate every cable's destination port resolves to a real input (`CompileError::UnknownPort`);
walk topo order, instancing each module once per voice if `Rate::Voice` or once total if
`Rate::Global`, wiring each input to its source's buffer (unconnected -> silence) or, for
voice-output-into-global-input, a `SumVoices` step. `out`'s inputs are special-cased into
`out_left`/`out_right` buffer indices directly (avoids needing to downcast `Out` just to read
two buffer handles the compiler already has).

Four design decisions made here, not specified by the brief, each documented at the top of
`compile.rs` itself as well as here:

1. **Voice-to-global signal combination is averaging, not summing.** Brief section 7 doesn't say
   which. Summing would make an N-voice patch N times louder than a 1-voice patch through any
   global-rate module downstream (e.g. a shared filter or output stage) — clearly wrong for a
   "just add more voices" workflow. Averaging keeps loudness stable as voice count changes.
   Tested two ways: `averaging_n_identical_voices_reproduces_a_single_voice_exactly` (4 identical
   voices average back to bit-exact the single-voice signal — exact under IEEE754 because 4 is a
   power of 2, no rounding) and `voice_count_does_not_change_loudness_for_identical_voices`
   (4-voice vs. 8-voice, same patch, asserted within `1e-4` rather than bit-exact — sequential
   summation of 8 equal values passes through non-power-of-2 partial sums (3v, 5v, 6v, 7v) that
   round slightly differently than summing to 4v; both are correct to float precision, only
   bit-exactness across different N would be the wrong thing to assert).

2. **A cycle is a compile error, not brief section 7.1's implicit 1-block delay.** Kahn's
   algorithm already detects cycles for free (nodes left over once the queue empties) — turning
   that into a hard `CompileError::Cycle(Vec<ModuleId>)` was the zero-extra-work option. Breaking
   cycles with an implicit delay is real work (need to choose *which* edge in the cycle gets
   delayed, thread an extra block of latency through state) that doesn't block anything in v1's
   accept test (brief section 12's test patch has no feedback loop). Deferred, not forgotten —
   flagged in STATUS.md's open items.

3. **No buffer-pool reuse yet.** Every output port on every module/voice instance gets its own
   fresh `[f32; BLOCK]` in `CompiledPatch.buffers`. Correct but wasteful (a real patch's buffer
   count grows with module-count × voice-count, all live for the process's lifetime, no reuse
   even for buffers whose consumers have already finished reading them within the same block).
   Kept simple to get a correct compiler first; buffer-pool reuse is a pure optimization on top
   of the same schedule shape, not a design change, so it's safe to defer.

4. **Params are compile-time constants**, resolved once from `ModuleState.params` (falling back
   to `ParamInfo.default`) when a `Step::Process` is built, not re-read from `PatchState` every
   block. Matches how `patch_demo.rs` and the 9 built-ins already treat most params (block-rate,
   not sample-rate, values — see `env_adsr.rs`'s doc comment for the same reasoning applied at
   the module level). A live-patching UI changing a knob mid-play will need a real
   recompile-or-patch path either way (see `recompile()` below); this isn't a regression from
   anything that worked before.

**Not yet RT-safe**: `process_block()` allocates scratch `Vec`s per call (building `Signal`
arrays for `ProcessIo::new`) — fine for the correctness tests here, but this cannot run on the
actual audio thread yet without that allocation removed. Flagged explicitly in `compile.rs`'s
module doc and in STATUS.md's open items; not silently swept under "it works."

**State carry-over**: `recompile(old: &mut CompiledPatch, patch, sample_rate, voice_count) ->
Result<CompiledPatch, ...>` compiles a fresh `CompiledPatch` against a (possibly changed) patch,
then for every `(ModuleId, voice)` origin present in both old and new, round-trips
`save_state`/`load_state` through a new reusable `HashMapState` (factors out the
`HashMap<String,f32>` `StateWriter`/`StateReader` pattern every module's own test file had been
hand-rolling locally). Tested (`recompile_carries_over_oscillator_phase_and_filter_state`):
render 50 blocks, recompile against the same patch, confirm the next block from the recompiled
graph matches what the original would have produced — phase/envelope state isn't reset. This is
the piece `swap.rs`'s crossfade mechanism (proven in spike S1) will need to call around; the two
aren't wired together yet (`recompile()` returns a plain new `CompiledPatch`, not yet routed
through `swap.rs`'s `Engine`).

## 2026-09-21 — `process_block` made allocation-free

First follow-up chosen off the compiler's own open-items list (RT-safety, swap.rs wiring, buffer
pool reuse, cycle handling — picked RT-safety first since it's the one thing standing between "the
compiler is correct" and "the compiler could run on a real audio thread"). Owner said "continue"
without picking a specific item; RT-safety was next in the priority order STATUS.md's last
handover already laid out, so proceeded on that basis rather than asking.

Replaced `process_block`'s four per-call `Vec` allocations (copied input buffers, `Signal` arrays
for inputs/params, the output-slice array) with fixed-size stack arrays: `[[f32; BLOCK];
MAX_INPUTS]` for input copies, `[Signal; MAX_INPUTS]` / `[Signal; MAX_PARAMS]` for the signal
views, and per-arity stack-literal output slices (`[&mut [f32]; N]`, N known at each match arm —
this was already implicit in the existing 0/1/2/3-output match, just routed through an
unnecessary intermediate `Vec` before).

`MAX_INPUTS = 4`, `MAX_OUTPUTS = 3`, `MAX_PARAMS = 4` — measured, not guessed: grepped every
built-in's `PORTS`/`PARAMS` table (`mixer`: 4 inputs; `filter.svf`: 3 outputs; `env.adsr`: 4
params; nothing else comes close). A module exceeding these bounds would either need truncated
scratch (silently wrong) or a runtime panic on the audio thread — neither acceptable — so
`compile()` (control thread, safe to fail loudly) now checks every module's port/param counts
against these constants and returns a new `CompileError::TooManyPorts` instead. The old
`process_block` match already had a `_ => panic!(...)` for >3 outputs as a latent audio-thread
risk; it's now `unreachable!()`, true by construction because `compile()` already rejected that
patch.

Proof, not assertion: `crates/engine/tests/compile_rt_safety.rs`, same pattern spike S1's
`tests/spike_s1_swap.rs` established (`assert_no_alloc::{assert_no_alloc, AllocDisabler}` as
`#[global_allocator]`, panics on any alloc/dealloc while active). Compiles the real 5-stage
`chord_patch` at voice_count=4 (exercises voice-rate instancing, `SumVoices` averaging, and
`filter.svf`'s 3-output arm — not a trivial single-module patch), triggers all 4 voices, then
runs 200 blocks of `process_block()` inside the guard. Passes; output still finite and non-silent
afterward (a no-alloc pass that also produced silence would be a hollow proof).

Not done by this pass, still open: wiring `CompiledPatch`/`recompile()` into `swap.rs`'s `Engine`
so a live repatch can actually reach the audio thread, and buffer-pool reuse (still one fresh
buffer per output port — a memory footprint problem, not an RT-safety one, so lower priority than
this was).

## 2026-09-21 — `PatchEngine`: wiring the compiler into S1's swap mechanism

Next item off the compiler's open-items list after RT-safety. New type,
`crates/engine/src/patch_engine.rs::PatchEngine`, rather than generalizing `swap::Engine` in
place — the two wrap genuinely different shapes (S1's `CompiledGraph` is mono, takes
`sample_rate` per `process_block` call, rebuilds via a single `cable_depth: f32` knob;
`CompiledPatch` is stereo, owns its `sample_rate`, rebuilds via `recompile()` against a whole
`PatchState`). Forcing both through one trait/generic for exactly two call sites would be
abstracting over the difference, not removing duplication — the "don't design for hypothetical
future requirements" case. The actual shared thing, the equal-power crossfade formula itself
(`swap::equal_power`, now `pub(crate)`), *is* reused verbatim rather than re-derived, since that
part genuinely is the same math, not just similar-looking code.

`PatchEngine::build_swap` (control thread) calls `compile::recompile()` against the current
`active` graph and wraps the result in `Owned` for handoff; `receive_swap`/`process_block`
(audio thread) mirror `swap::Engine`'s shape exactly — same `Owned<T>` single-slot-channel
pattern, same elapsed-sample crossfade math, generalized to stereo.

**Caught a real state-carry-over bug while proving this, not a hypothetical one.**
`crates/engine/tests/patch_engine_swap.rs` swaps the same patch onto itself 20 times (chosen
specifically because it gives an exact null test: since nothing about the patch changes and
`recompile()` was already proven to carry state exactly in `compile.rs`'s own single-recompile
test, a `hard` reference that never swaps at all should match `PatchEngine`'s output bit-exactly
outside every crossfade window — a repeated-swap stress test that a single recompile can't
exercise). First run failed: residual grew with each swap, engine consistently *louder* than the
never-swapped reference, worsening over the 20 swaps. Root cause: `dsp::FullAdsr`'s
`gate_was_high: bool` (the field `next()` uses to edge-trigger entering `Attack`/`Release`) was
never part of `env.adsr`'s `save_state`/`load_state` — only `level`/`stage` were. A freshly
`registry::create()`d `EnvAdsr` always starts with `gate_was_high: false` (via `FullAdsr::new`),
so on every recompile, a continuously-held gate looked like a brand-new note-on: the envelope
re-entered `Attack` from whatever `level` it had already reached, instead of continuing
`Sustain`. One recompile's worth of this is a small, easy-to-miss bump; 20 in a row compounded
into an obviously-wrong, steadily-diverging signal — exactly the kind of bug that only shows up
under repeated-recompile stress, not a single before/after check.

Fixed: made `FullAdsr::gate_was_high` `pub` (matching `stage`/`level`'s existing visibility) and
added it to `env_adsr.rs`'s `save_state`/`load_state` as a third `f32` (`0.0`/`1.0`). Test now
passes with `max_steady_residual == 0.0` (bit-exact, not just close) across all 20 swaps. Also
proves `PatchEngine::process_block` itself adds no allocation on top of `CompiledPatch::
process_block`'s existing allocation-freedom (same `assert_no_alloc` pattern as
`compile_rt_safety.rs` and spike S1).

Not done by this pass: overlapping swaps (a second `build_swap` while one is still crossfading)
aren't accounted for in either `Engine` — `PatchEngine` inherits the same documented gap
`swap::Engine` already had, not a new one. No hookup yet from a real control surface (no
UI/standalone binary exists to actually call `build_swap` from user input) — `PatchEngine` is
proven correct and ready, not yet reachable by a person.

## 2026-09-21 — Autonomous overnight work begins

Owner: "Continue working in perpetuity, make/test spikes you can do yourself when possible, I'll
check up on the work in the morning, you're fully autonomous here now, make sure to conserve
tokens and when you run out restart yourself when you get them again." No further user input
expected for hours. Working the compiler's/modules' open-items list in priority order for
self-contained, self-verifiable items only — nothing needing hardware (Pi-4, aarch64), a UI, or a
human ear to judge (those stay flagged, not attempted blind). Self-rescheduling via
`send_later` (short delay, message: continue the backlog) after each chunk, so the session keeps
picking up work across the night without needing a person to re-prompt it; each chunk still gets
the full existing rigor (plan, implement, test, clippy/fmt, decisions.md, STATUS.md, commit,
push) — autonomy is not a license to skip verification, if anything it raises the bar since
nobody's watching in real time to catch a mistake.

## 2026-09-21 — `osc.va`: square/triangle/sine waveforms + hard sync

First autonomous-session item, picked because it's the clearest remaining gap in brief section
8's v1 built-in table (`osc.va` was saw-only since the very first module pass) and is fully
self-testable — no external ground truth needed beyond DSP first-principles and internal
consistency.

Added `dsp::FullOsc` (parallel to `FullAdsr`/`FullLfo`): `OscWaveform::{Sine, Triangle, Saw,
Square}`, `next(freq, sample_rate, waveform)`, `hard_sync()`. Saw and square are PolyBLEP-
corrected (square applies the same `poly_blep` correction `Saw` already used, at both of its two
discontinuities — phase 0's rising edge, phase 0.5's falling edge — the standard two-correction
extension). **Triangle is naive, not band-limited** — a properly anti-aliased triangle needs
"polyBLAMP" (an integrated correction at its two corner discontinuities, not the single-edge
correction PolyBLEP handles), a distinct algorithm not implemented here. Chose to ship a correct-
shape, honestly-labeled-as-aliased triangle now over blocking on polyBLAMP, which needs its own
derivation/verification pass; `ModuleInfo.quality.anti_aliasing` stays `true` since it's a
per-module not per-waveform flag, with the nuance documented in `FullOsc`'s doc comment instead
(the coarsest-level-available limitation `lfo.rs`'s waveform-as-stepped-param comment already
flagged for the same `ParamInfo`-shape reason).

`osc_va.rs`: added a `waveform` stepped-float param (`0..=3`, same convention as `lfo.rs`,
same relative ordering `Sine/Triangle/Saw/Square` for cross-module consistency) and a `sync`
audio-rate input port, edge-detected per sample against the same `>0.5` "gate" threshold
`env.adsr`/`midi.in` use. Default waveform is `2.0` (Saw) specifically to preserve this module's
pre-existing sound, not because saw is otherwise privileged — every existing hand-wired caller
(`patch_demo.rs`, `dyn_dispatch_spike.rs`) was updated to pass `waveform=2.0` and an unconnected
`sync` input, keeping their numbers bit-exact (confirmed: their existing correctness tests still
pass unchanged).

**Applied the `gate_was_high` lesson proactively**: added `sync_was_high` to `osc_va.rs`'s
`save_state`/`load_state` from the start, rather than waiting to hit the identical bug class
`env.adsr`'s state carry-over just had (a continuously-held level looking like a fresh edge to a
freshly-constructed instance). A test (`save_and_load_state_round_trips_sync_edge_flag`) exercises
this directly: sync held high across a save/load boundary must not falsely re-trigger a hard sync.

12 tests total in `crates/modules/tests/osc_va.rs`: the four pre-existing correctness tests
(saw-vs-`dsp::Saw`, pitch shift, buffer input, reset, state round-trip) plus 8 new ones —
square/triangle/sine each checked against `FullOsc` called directly (exact, not approximate,
since the module wraps `FullOsc` with no extra transformation), a square-wave sanity check
(bounded amplitude allowing PolyBLEP's small correction overshoot, correct zero-crossing count
for a known frequency), a triangle sanity check (bounded exactly to [-1,1], no sample-to-sample
step exceeding its known maximum slope), hard sync (a sync buffer with a single rising-edge
sample snaps phase to 0 at that exact sample, verified against a fresh `FullOsc`), and the
sync-state-carry-over test above. All passing; workspace build/test/clippy/fmt clean.

Skipped for this pass, not silently: an audible WAV render (the owner won't hear it until
morning regardless, and the waveform-vs-reference tests already prove correctness more precisely
than an ear could at 1x speed) — can render one on request rather than spending effort on it now
given the "conserve tokens" instruction.

## 2026-09-21 — Voice allocator (`crates/engine/src/voice_allocator.rs`)

Second autonomous-session item. Brief section 7's "voice allocator" was the one remaining v1
milestone-checklist row nothing at all existed for — every test/demo that played more than one
note (`patch_demo.rs`'s chord, `compile.rs`'s `chord_patch` tests) hand-picked which voice index
each note went to, which only works because those are fixed test chords, not real playing.

`VoiceAllocator` is deliberately decoupled from `CompiledPatch`/`midi.in`/pitch representation:
`note_on(note_id: u32) -> usize` / `note_off(note_id: u32) -> Option<usize>`, where `note_id` is
whatever the caller wants it to mean (a MIDI note number, in practice). Kept pitch-agnostic and
graph-agnostic on purpose — it doesn't need to know semitones vs. MIDI note numbers vs. anything
else, and testing the allocation policy shouldn't require compiling a patch. A caller (a MIDI
router, not built yet) takes the returned voice index and calls the corresponding `MidiIn::
note_on` itself — same "manual glue" shape `patch_demo.rs` already established for driving
`midi.in`, just automated instead of hand-picked.

Policy: lowest-index free voice first; steal the *oldest* still-held voice when all are busy
(standard, simple voice-stealing — not velocity/envelope-stage-aware, which would need to inspect
each voice's envelope state for a "steal the quietest" refinement; flagged as a real, deferred
improvement, not attempted blind). A repeated `note_on` for an already-held note reuses its
existing voice and refreshes its age, rather than leaking a second voice for one logical note —
handles a real MIDI-source edge case (retriggered/stuck-key note-on) that a naive "always find a
free voice" implementation would get wrong.

9 tests in `crates/engine/tests/voice_allocator.rs`: 7 pure-logic (fill order, steal-oldest,
stealing continues in oldest-first order across repeated steals, a stolen note's own `note_off`
resolves to `None` and doesn't disturb the voice that stole it, unknown-note `note_off` is a
no-op, repeated `note_on` reuses rather than steals, and refreshes age so it isn't immediately
re-stolen) plus one integration test compiling a real minimal patch and confirming the allocator's
returned voice indices land pitch changes on the exact right `midi.in` instance, including a
free-before-steal check after an explicit release. All passing; workspace build/test/clippy/fmt
clean.

## 2026-09-21 — Standalone binary (`crates/standalone`): cpal + midir, "something to hear"

Owner: "Do the UI yourself, do everything, we can easily fix it later" — explicit authorization
to build brief section 12's `standalone`/`ui` crates autonomously, superseding this session's
earlier self-imposed caution about needing a person for anything UI/hardware-shaped. Built the
standalone binary first (not the UI) since it's the actual "a person can hear it" milestone —
a patchbay UI without real audio/MIDI I/O underneath it still can't produce sound.

**Environment reality, checked not assumed**: this container has no `/dev/snd`, no ALSA cards,
and no display server. `cpal`'s Linux backend needed `libasound2-dev` even to *build* (not just
run) — installed via `apt-get` (a safe, reversible dev-header install, not a destructive system
change). The binary itself was run here and correctly detects "no usable audio device," proving
the fallback path (see below) rather than the real `cpal` playback path, which cannot be verified
in this environment. Real audio/MIDI I/O needs to be checked on the owner's own machine — flagged
in STATUS.md, not glossed over.

`kabl-standalone` splits into a testable `lib.rs` (default patch, MIDI-byte resolution, a non-
allocating ring buffer, RT-safe event application — all provable without hardware) and a thin
`main.rs` (the actual `cpal`/`midir` wiring, which by construction can't be unit-tested here).
Design:

- **`default_patch()`**: one voice-rate chain (`midi.in -> osc.va -> filter.svf -> env.adsr/vca
  -> out`), the same 5-stage shape already proven throughout this session. The compiler instances
  it once per voice (`Rate::Voice`) automatically — no per-voice wiring needed, it's already a
  full `DEFAULT_VOICE_COUNT`-voice (8) polysynth once paired with `VoiceAllocator`.
- **MIDI resolution split across two threads by design, not incidentally**: `resolve_midi_message`
  (MIDI thread) turns raw bytes into a `VoiceEvent` using `VoiceAllocator` — deliberately *not*
  called from the audio thread, since `VoiceAllocator`'s `HashMap` can allocate on insert/resize
  and would violate brief section 3's RT rule. Only the resolved, `Copy`-able `VoiceEvent` crosses
  an `rtrb` single-producer/single-consumer channel to the audio thread, where `apply_voice_event`
  (proven allocation-free via `assert_no_alloc`) applies it — the same "resolve off the audio
  thread, apply cheaply on it" split `PatchEngine::build_swap`/`receive_swap` already established
  for graph swaps, now applied to MIDI events too.
- **`RingBuffer`**: bridges `PatchEngine::process_block`'s fixed `BLOCK`=64-sample output against
  `cpal`'s callback, which can request any number of frames per call (host/device/buffer-size
  dependent, not a multiple of 64 in general). Fixed capacity (`BLOCK * 256` =~ 340ms headroom),
  allocated once at stream setup (control thread), never grown after — `push`/`pop` are proven
  allocation-free the same way. Overflowing it is an `assert!` panic, not a silent drop: it would
  mean a host buffer size far outside anything reasonable, a real bug to see loudly, not paper
  over.
- **Fallback self-test render**: no output device (this container) or no usable `f32` config ->
  instead of exiting with nothing to show, renders a short chord straight through `compile()`
  (bypassing `PatchEngine`/`cpal` entirely) to `target/spike-renders/standalone_selftest.wav`.
  Proves the synthesis path end-to-end even where real playback can't be verified. Sent to owner.

13 tests in `crates/standalone/tests/lib_logic.rs`: `RingBuffer` round-trip/wraparound/overflow-
panics/no-alloc, `resolve_midi_message` (correct voice+pitch+velocity, note-on-velocity-0-as-
note-off MIDI convention, explicit note-off, unheld-note note-off resolves to `None`, non-note
messages resolve to `None`), `apply_voice_event` (reaches the exact right compiled voice, other
voices untouched, no allocation), and `default_patch()` actually compiles and produces audible,
finite output when driven through a full note-on/render/verify cycle. All passing; workspace
build/test/clippy/fmt clean (clippy now also covers `cpal`/`midir`/`alsa`'s dependency tree,
clean).

Not done by this pass: a `--midi <port>`/`--list-midi` CLI for picking among multiple MIDI ports
(connects to the first available one only); no persistence (always plays `default_patch()`, no
loading a saved `.kabl` file yet — `core`'s file format already exists, just not wired here);
a mono output device gets the left channel only (not an L+R downmix) since channel 0 is always
written from `left_ring` and channels 1+ from `right_ring` — a real but minor gap for the
uncommon mono-output case, most devices are stereo. None of these block "can a person plug in a
MIDI keyboard and hear the synth," which was the actual goal.

## 2026-09-22 — `kabl-ui`: the patchbay

Owner: "Do the UI yourself, do everything, we can easily fix it later" — building brief section
12's `ui` crate. Split the same way `standalone` was: `editor.rs`'s `PatchEditor` (patch-editing
logic over `kabl_core::PatchLog` — every mutation is a real `Op`, so undo/redo comes from the
existing log machinery, not a UI-layer reimplementation) is fully hardware-independent and
tested headlessly; `lib.rs`'s `show()` is the actual `egui` widget tree; `main.rs` wires live
audio (same `cpal`/`midir`/`PatchEngine` shape as `standalone`) plus `eframe` windowing.

**egui/eframe version note**: latest (0.36.2) needs rustc 1.95; this toolchain has 1.94.1, so
pinned to 0.35.0 — checked by trying to build, not guessed. 0.35's `Panel` API is a bigger
change than a version bump suggests: `TopBottomPanel`/`SidePanel` were unified into one
`egui::Panel::{top,bottom,left,right}(id)`, and `eframe::App::update(&mut self, ctx: &Context,
...)` became `App::ui(&mut self, ui: &mut Ui, ...)` — panels now nest inside a `&mut Ui` the
integration hands you, not a `&Context` you thread through yourself. Found by reading the
installed crate source directly (`~/.cargo/registry/.../egui-0.35.0/src/containers/panel.rs`)
after the pattern from memory didn't compile, rather than guessing at API shapes.

**Also switched off the default `wgpu` renderer to `glow`** (`eframe = { default-features =
false, features = ["glow", ...] }`). Checked, not assumed: this container has no `/dev/snd`
*and* no display server, so before writing 500 lines of UI code the plan was verified against
reality first — installed `libxkbcommon-x11-0` (a missing shared library `winit` needed just to
open an X11 event loop), started a virtual X server (`Xvfb`) with `LIBGL_ALWAYS_SOFTWARE=1`
(Mesa's `llvmpipe` software rasterizer, already present), and tried running the built binary.
`wgpu`'s default backend selection failed outright (`CreateSurfaceError`) under Xvfb's software
GL; `glow` (classic OpenGL via `glutin`) worked. This is a real, load-bearing finding, not a
guess: it means this specific container can validate the UI actually renders, not just compiles.

**Ran it and looked at the result** — `xwd`+`imagemagick` for a screenshot, `xdotool` to simulate
a click, none of which existed in the container until installed (all safe, reversible dev-tool
installs, same category as `libasound2-dev` earlier). This caught a real bug the same session's
established "measure before claiming" rule exists for: `kabl_standalone::default_patch()` gave
every module the identical `pos: Vec2{0,0}` (nobody had ever needed positions to be distinct
before — `standalone`'s binary doesn't render anything). In the UI, all 6 modules rendered
stacked exactly on top of each other; only the topmost (`out`, drawn last) was visible. Fixed by
giving `default_patch()` real staggered positions (left-to-right in signal-flow order) — this is
shared with `kabl-standalone`, so both crates now start from a patch that actually looks like
what it is. A second, smaller issue found the same way: newly-added modules all spawned at the
same fixed `(40,40)`, so repeated "Add module" clicks stacked new boxes on each other too — fixed
with a cheap cascade offset (`40 + (count%10)*24`) rather than solving real auto-layout, which is
follow-up work if it matters once dragging is the normal way to place things.

Design choices in `editor.rs`/`lib.rs`:
- **Snapshot-then-mutate, every frame.** `show_canvas`/`show_param_panel` clone the module/cable
  data they need to draw *before* touching `editor` mutably (a click handled mid-draw calls
  `editor.add_module`/`connect`/etc. immediately — immediate-mode UI, no separate "apply queued
  actions" pass). Holding `editor.state()`'s borrow across a later `&mut editor` call doesn't
  borrow-check otherwise; cloning a small `Vec` of positions/kinds once per UI frame is cheap
  (this is the UI thread, not the audio thread — nothing here claims or needs RT-safety).
- **Drags commit once, on release, not every frame.** Dragging a module updates only `UiState`'s
  local `live_pos` each frame; a single `MoveModule` op is appended to the log on
  `drag_stopped()`. Committing every frame would flood the undo history with one entry per
  rendered frame for a single drag gesture — wrong grain for "undo" to operate at.
- **Port-click-to-connect, not drag-a-cable.** Click an output port (arms it, shown in the
  toolbar), then click an input port to complete the connection, or click the same output again
  to cancel. Simpler to implement and test correctly than cable-dragging with hit-testing against
  a moving cursor; a real "drag from port to port" interaction is nicer and can replace this
  later without changing `PatchEditor`'s API at all (the UI layer is the only thing that would
  change).
- **No canvas panning/scrolling yet** — `default_patch()`'s modules already run off the visible
  width at default window size (visible in the screenshot: `vca`/`out` are clipped by the side
  panel). A real gap, not hidden — flagged in STATUS.md.
- **`kind`/port validity isn't re-checked by the editor** (`add_module`/`connect` accept whatever
  string/`PortRef` they're given). `compile()` already rejects an unknown kind or port
  (`CompileError::UnknownKind`/`UnknownPort`) — the same error path a corrupted saved file would
  hit — so the editor doesn't need a second, UI-layer copy of that validation. In practice `show()`
  only ever offers valid kinds (from `registry::KNOWN_KINDS`) and valid ports (from the clicked
  module's own `ModuleInfo`) as choices anyway.

**Known, flagged compromise in `main.rs`, not textbook RT-safe**: the audio callback and the
UI/control thread share one `PatchEngine` behind a `Mutex`. The callback only ever `try_lock`s
(silence for that block on contention, never a blocking wait); the control thread takes a real
lock only when an edit landed (`PatchEditor::take_dirty()`), for the duration of `build_swap`
(which calls `compile()` — real, non-trivial work). Any audio block landing in that window gets
silence, not corruption or a panic — a real, audible glitch risk on every edit, not a crash risk.
The textbook fix is restructuring `PatchEngine` so the control thread reads a lock-free-published
state snapshot instead of sharing the live instance — real follow-up work, not attempted blind
tonight without a way to exercise actual concurrent audio+UI threads here to verify it.

**Testing**: 16 tests in `crates/ui/tests/editor_and_ui.rs` — `PatchEditor`'s full surface (add/
remove/connect/disconnect/move/set_param, undo/redo including no-op edge cases, `seed_from`
reproducing the source patch exactly and being undoable back to empty, fresh IDs never colliding
with seeded ones, the dirty flag's take-once semantics) plus two headless `show()` smoke tests
proving the actual widget tree runs across several frames and with a module selected, without
panicking — using `egui::Context::begin_pass`/`end_pass` directly (pure Rust, no window/GPU
needed for this, confirmed by reading `egui::Context::run_ui`'s own doc example). Beyond the
smoke test, the interactive/visual correctness (does a click really land, does it look right) was
checked by hand this session (screenshot + a real `xdotool` click, both included above) rather
than automated — a real gap between "proven" and "eyeballed once," flagged, not conflated.

Workspace build/test/clippy/fmt all clean, including the full `egui`/`eframe`/`winit`/`wgpu`-free-
`glow` dependency tree now pulled into the workspace.

## 2026-09-22 — Persistence: `--patch`/`--save-default` and `kabl-ui`'s Save/Load

Fourth autonomous-session item. `core::{load, save}` have existed and been tested since the very
first session (the op-log file format), but nothing ever called them — both `standalone` and
`kabl-ui` always started from `default_patch()` with no way to get a save file onto disk or back
off it. Picked as the natural next item per STATUS.md's own handover note ("probably the next
real feature: it's what turns 'always the same demo patch' into an actual instrument").

`PatchEditor` gained two additions for this: `from_log(PatchLog) -> Self` (wraps an already-built
log directly, preserving its *real* history — unlike `seed_from`, which replays a bare
`PatchState` as fresh ops and is right only for a hardcoded starting patch that never had real
history) and `log() -> &PatchLog` (what `core::save` actually needs — a bare `state()` snapshot
would lose everything but the current values, discarding undo history a saved file should keep).

**`kabl-standalone`**: `--patch <dir>` loads via `kabl_core::load` instead of playing
`default_patch()`; `--save-default <dir>` writes `default_patch()` out (replayed as real ops, not
a bare state dump — same reasoning as `PatchEditor::seed_from`, in miniature) and exits, as a
starting point to then edit in `kabl-ui`. A tiny hand-rolled two-flag parser
(`flag_value(&args, "--flag")`) rather than a CLI-parsing crate dependency — two optional flags
each taking one path argument doesn't clear the bar for a new dependency (this project's standing
rule: no new dependency without a decisions.md line justifying it).

**`kabl-ui`**: a "patch dir" text field plus Save/Load buttons in the toolbar, not a native file-
picker dialog (`rfd` or similar) — deliberately: a picker dialog is exactly the kind of thing this
container's Xvfb setup *can't* verify (no real file-manager chrome to click through), while a
plain text field's Save/Load logic is fully real and independently provable. Load replaces
`*editor` wholesale with `PatchEditor::from_log(...)`, which (correctly) starts with `dirty:
false` — right for the app's own startup seed, whose caller compiles the seeded patch directly,
but wrong here: a Load mid-session replaces a *live* patch, and the audio host only ever
recompiles by checking `take_dirty()`. Missing this would have meant Load silently doing nothing
audible. Fixed with a new, explicit `PatchEditor::mark_dirty()` — a deliberate escape hatch for
exactly this "I replaced state some other way, the host still needs to notice" case, not a
workaround bolted onto `from_log` itself (which should stay clean for its own real use, the
startup seed).

**Verified running, not just tested**: rebuilt the same Xvfb + software-GL setup from `kabl-ui`'s
own session, clicked the real "Save" button via `xdotool`, and confirmed the file landed on disk
with the expected `log.jsonl`/`checkpoint.json`/`meta.toml` shape and the toolbar's own "saved to
my-patch" confirmation message rendered. (A first attempt at also exercising the text field itself
via synthetic key events didn't land the edit — `xdotool`'s keyboard synthesis under `winit`'s
X11 focus handling is finicky; the earlier "Add module" click already proved click-driven
mutations reach the render loop correctly, so this is a text-input-simulation gap in the test
harness, not a signal about the Save/Load code path itself, which the direct file-on-disk check
confirms independently.)

24 new tests: 4 in `crates/standalone/tests/persistence.rs` (`default_patch()` round-trips
through save/load exactly; a loaded patch compiles and renders bit-identical output to the
original over 20 blocks; loading preserves real op-log entry count and `can_undo()`, not just a
snapshot; a guard that `default_patch()` still has params worth exercising `SetParam` ops) plus 4
new ones in `crates/ui/tests/editor_and_ui.rs` (`from_log` preserves state and starts clean,
`mark_dirty` sets the flag without an op, a full save-then-load round-trip through a real
temp-directory file, fresh IDs after `from_log` never collide with loaded ones). Workspace
build/test/clippy/fmt all clean.

## 2026-09-22 — Compiler: buffer-pool reuse

Fifth autonomous-session item, from STATUS.md's own handover list (buffer-pool reuse, cycle
handling, overlapping-swap handling, `kabl-ui` canvas pan/scroll were the remaining self-
contained candidates — picked this one first: real memory cost today, most self-contained of the
four, no design ambiguity to resolve).

`coalesce_buffers` (new function in `compile.rs`): standard greedy linear-scan register
allocation, run once at compile time after the schedule (`steps`) is fully built. For each buffer
index, computes `[first_def, last_use]` in schedule-step terms (the step that produces it, the
last step that reads it as an input or `SumVoices` source); sorts buffers by `first_def`; walks
that order handing out physical slots, reusing one whose previous occupant's `last_use` is
already behind the new buffer's `first_def`, allocating a fresh slot only when nothing's free.
Three buffers are pinned "live forever" and excluded from reuse entirely: `silence_buf` (a shared
sentinel every unconnected input reads, not a normal single-producer value), and `out_left`/
`out_right` (read by the *caller* of `process_block`, i.e. after every step has already run — a
range ending at "the last step" would let another buffer's write stomp on it before the caller
gets to `.left()`/`.right()`).

Deliberately did **not** attempt same-step reuse (a buffer whose last read and a new buffer's
first write happen at the identical step index), even though `process_block`'s existing
copy-inputs-before-writing-outputs ordering would make it safe: correctness here shouldn't depend
on a subtle invariant of a separate function that could change independently later. One extra
buffer's worth of slack across the whole schedule is a trivial cost against that fragility.

Measured, not assumed (a throwaway `cargo run --example`, deleted after — see the git history if
the exact script matters again): the 5-stage chord chain (`midi.in -> osc.va -> filter.svf ->
env.adsr/vca -> out`) at increasing voice counts:

| voice_count | buffers before (1 per output port + 1) | buffers after coalescing |
|---|---|---|
| 1 | 10 | 7 |
| 4 | 37 | 13 |
| 8 | 73 | 21 |
| 16 | 145 | 37 |

Roughly linear growth either way, but coalesced grows at ~2 buffers/voice instead of ~9/voice
(each voice's chain has 9 output ports total) — about a 4x reduction at 16 voices, growing with
scale since the *shape* of what's simultaneously live per voice stays constant while the naive
count scales with total port count.

Three new tests in `crates/engine/tests/compile.rs` (now 10, was 7): `buffer_count()` (new,
trivial accessor added to `CompiledPatch` so this is observable at all, not just trusted) is
below the naive upper bound for the chord patch; 8 voices don't need double the 4-voice pool
(a loose bound proving sub-linear-with-total-ports growth without pinning exact scheduler
output, which would make the test brittle against unrelated scheduling changes); two independent
`compile()` calls of the same patch, driven identically, produce bit-exact identical output over
50 blocks (a belt-and-suspenders check — if coalescing ever aliased two buffers still actually
live at the same time, this is the kind of corruption that would show up as nondeterminism or a
wrong render, not a clean crash). All of the compiler's existing tests (the averaging/cycle/
state-carryover/RT-safety/swap ones) also still pass unchanged, since they were already exercising
the one and only `compile()` path — there's no separate "coalescing on/off" mode to test against.

Workspace build/test/clippy/fmt all clean.

## 2026-09-22 — Cycle handling: implicit 1-block delay (`crates/engine/src/compile.rs`)

Sixth autonomous-session item off STATUS.md's own handover list (the Mutex-sharing compromise
needs real hardware to judge whether it matters in practice; overlapping swaps and canvas
pan/scroll are UI/engine polish with no correctness question attached — cycle handling was the
next item with a real, self-contained design decision still open). Was previously a hard
`CompileError::Cycle` (see "Flat-schedule compiler v1" above) — brief section 7.1 asks for an
implicit 1-block delay instead, which the original entry flagged as "real work, deferred not
forgotten."

**Mechanism**: a DFS over the cable graph (`patch.cables`, iterated in `CableId` order —
`BTreeMap`, deterministic) classifies every cable as tree/forward/cross or *back* (destination
already on the current recursion stack). A cycle must contain at least one back edge relative to
any single DFS of the graph — standard result, not something specific to this graph shape — so
excluding every back edge found this way from the topo-ordering graph always leaves a DAG. Kahn's
algorithm then always consumes every module; `CompileError::Cycle`'s "leftover nodes" branch is
now a defensive fallback that should be unreachable, not the normal outcome for a cyclic patch.
Each back-edge cable's reader gets wired to a *delay buffer* instead of the source's live output
buffer — a separate buffer refreshed by a new `Step::CopyToDelay` placed at the end of the
schedule, so this block's own reads (earlier in the step list) still see *last* block's value.
First block after compile reads silence on that input (no history yet, same convention as every
other buffer starting zeroed).

**Design calls made here, not spelled out by the brief**:

1. **Which edge gets delayed, for a multi-edge cycle**: whichever cable the DFS marks a back edge
   — not chosen for musical sensibility (there's no principled way to pick "the right" edge from
   inside the compiler; that's a patch-design decision, not a compiler one), just the standard,
   deterministic, always-correct choice. Cable-id-ordered DFS makes it reproducible: the same
   patch always gets the same cable delayed.
2. **Delay buffers are pinned live-forever in `coalesce_buffers`**, alongside `silence_buf`/
   `out_left`/`out_right` (that function's signature changed from three named exceptions to a
   `pinned: &[BufIdx]` slice to fit this in without special-casing a fourth category). A delay
   buffer's whole purpose is surviving from one `process_block()` call to the next — the existing
   `[first_def, last_use]` analysis only reasons about one block's schedule, so it has no way to
   know a "last use" inside this block isn't actually the last use overall. Reusing its slot for
   something else would silently corrupt the next block's feedback read.
3. **Delay buffers reset to silence on `recompile()`**, same as every other buffer — `recompile()`
   calls `compile()` fresh, and `compile()` always zero-initializes `buffers`. Only module state
   (via `save_state`/`load_state`) carries across a recompile; a feedback loop's one-block memory
   doesn't, so a live edit to a patch with a cycle momentarily "forgets" the loop's last value.
   Not attempted here — no accept-test patch recompiles a cyclic patch, and carrying it over would
   mean threading delay-buffer identity through `CompiledPatch`'s public surface the way
   `module_origin` does for modules, real extra design work for an unexercised case. Flagged in
   STATUS.md's open items if it turns out to matter once real feedback patches exist.
4. **Cross-rate delayed cables reuse the existing voice/global/sum machinery**, just against a
   parallel set of delay-buffer maps (`voice_delay_buf`/`global_delay_buf`/`summed_delay_buf`
   mirroring `voice_output_buf`/`global_output_buf`/`summed_buf`) rather than a special case: a
   delayed voice-rate source feeding a global-rate destination still averages across voices (via
   `SumVoices` reading the delay buffers instead of the live ones), a delayed global source
   broadcasts to every voice lane the same way a live one does. Not separately proven against a
   real patch (no v1 built-in module combination needs a cross-rate feedback loop), but it falls
   out of the same trichotomy the non-delayed path already used, not new logic.

**Tested**: `crates/engine/tests/compile.rs`'s `cycle_is_a_compile_error_not_silently_wrong`
replaced with `cycle_compiles_with_implicit_one_block_delay` — `osc.va` (external driver, outside
the cycle) feeds two `vca`s wired into a 2-cycle (`3.out -> 2.in` normal, `2.out -> 3.cv` delayed;
worked out by hand which edge the DFS picks — see the test's own doc comment for the full trace).
Both `vca`s keep default `gain=1.0`/`exponential=off`/unconnected `cv`, which makes `vca`'s output
exactly `in * clamp(1.0 + cv, 0, 1)` — the test computes each block's expected output directly
from an independent `Saw` reference generator (same pattern `single_osc_patch`'s tests already
use) and the *previous* block's own expected output (the value the delayed `cv` should read),
asserted bit-exact over 5 blocks. This exercises the real mechanism end-to-end (DFS edge choice,
delay-buffer pre-creation ahead of the topo loop, `CopyToDelay` timing, `coalesce_buffers` pinning
all live together), not just the DFS classification in isolation.

Workspace build/test/clippy/fmt all clean.

## 2026-09-22 — `PatchEngine`: overlapping swaps (`crates/engine/src/patch_engine.rs`)

Seventh autonomous-session item off STATUS.md's handover list. Picked over canvas pan/scroll
(`kabl-ui`, needs the Xvfb harness) and the delay-buffer-across-recompile gap (smaller, cosmetic
correctness note rather than a real risk) because this one is a genuine live-editing hazard: brief
section 7.6 describes a live-patchable synth, and a person dragging a knob twice within 15ms (the
crossfade window) is an entirely ordinary interaction, not an edge case.

**The bug**: `receive_swap` unconditionally did `self.incoming = Some((new_patch, 0))`. If a fade
was already in flight, this replaced `incoming` outright — the blended output would snap from
"blend of `active` and the first new patch" straight to "blend of `active` (still the *original*
pre-first-swap graph, since `finished` never got a chance to fire) and the second new patch,
restarted at `elapsed=0`" in one process_block call. Two problems at once: the first edit's
graph is thrown away before ever becoming steady-state (a queued knob-turn silently vanishes), and
the sudden change in what "new" means mid-blend is exactly the audible discontinuity the
equal-power crossfade exists to prevent — the brief's own words, "not just fewer clicks."

**Fix**: a `pending: Option<Owned<CompiledPatch>>` slot. `receive_swap` queues into `pending`
instead of overwriting `incoming` when a fade is already running; `process_block`'s existing
"fade finished, promote incoming to active" branch now also promotes `pending` into `incoming`
(fresh `elapsed=0`) in the same step, so a queued swap starts the instant the one ahead of it
finishes — no dropped edits, no restarted fades, at the cost of up to one crossfade's worth
(15ms) of extra latency before the second edit's audio actually starts moving. Only one slot, not
a queue: a *third* overlapping arrival replaces `pending` (last-request-wins) rather than growing
unboundedly — nothing in a live-patching UI needs every intermediate knob position to survive,
only the final one. `is_swapping()` now reports true while a swap is either blending or queued,
not just blending, since both are "a swap is happening" from the caller's point of view.

**Known remaining staleness, not fixed here**: `build_swap` always recompiles against `self.active`
(the confirmed graph), never against an in-flight `incoming` — so a second `build_swap` called
before the first fade finishes carries module state (oscillator phase, envelope stage, filter
memory) snapshotted from *before* the first edit, not from wherever the first edit's graph will be
once promoted. This is a staleness bound, not a correctness bug — `active`'s state was already
only ever a control-thread snapshot of "as of whenever `build_swap` ran," never synced to the
audio thread's exact position even in the non-overlapping case — but it does mean back-to-back
edits can carry state that's up to one extra crossfade behind. Not attempted: recompiling
against a hypothetical future-`active` (the not-yet-promoted `incoming`) would need a new API
shape (what does "current state" even mean while two graphs are blending?) for a staleness window
measured in tens of milliseconds, real over-engineering for what it buys. Flagged, not hidden.

**Scope**: only `PatchEngine` (the real engine `standalone`/`kabl-ui` embed) got this fix.
`swap.rs::Engine` — S1's spike, fixed 2-node mono graph — has the identical gap and was
deliberately left alone: the repo map already documents spike code as "expect it to be
replaced/absorbed into the real compiler, not extended indefinitely," and `patch_engine.rs`'s own
module doc already treats the two engines' state-machine logic as intentionally unshared (only the
crossfade *math*, `equal_power`, is factored out). Extending the soon-superseded spike engine to
match would be exactly the kind of code this project has already decided not to keep investing in.

**Tested**: new `crates/engine/tests/patch_engine_overlapping_swap.rs`. Three patches, identical
except `vca`'s `gain` (1.0/0.5/0.0 — a compile-time-constant param, `vca` has no state to carry, so
nothing about state-carryover ambiguity muddies the result), one shared `osc.va` at a fixed
`base_hz` so every instance's phase stays bit-identical to the others throughout (same reasoning
`compile.rs`'s own recompile test already establishes) — this lets two independently-`compile()`d
references stand in for "what the engine's internal patch_b/patch_c instances should read at any
given moment," the same pattern `patch_engine_swap.rs` already uses for the single-swap case,
without needing to hand-derive any waveform. Fades: patch_a (active) -> patch_b (fade 1) ->
[patch_c requested mid-fade-1, must queue] -> patch_c (fade 2, starts the instant fade 1
finishes). Asserts: `is_swapping()` true right after the overlapping request; the window between
the two fades matches patch_b's own reference bit-exact (proof the queued swap didn't skip or
corrupt the first one — a buggy "just overwrite `incoming`" alternative would also eventually
reach patch_c, so this window is what actually distinguishes correct sequencing from the old bug,
not just the final state); the window after fade 2 matches patch_c (silence) bit-exact (proof the
queued swap wasn't dropped); the handoff sample between the two fades matches patch_b's reference
exactly (equal-power's `t=0` is `cos(0)=1, sin(0)=0` exactly, so it's provably an exact
continuation, not a coincidence of rounding). Verified the test actually catches the bug it's
named for: reverted the fix, confirmed this exact test fails (steady window between fades read as
silence — patch_b never became real steady-state active, exactly the "queued edit gets dropped"
failure mode described above), then restored the fix and reconfirmed green.

Workspace build/test/clippy/fmt all clean.
