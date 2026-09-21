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
