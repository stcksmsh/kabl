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
