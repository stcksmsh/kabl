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

Not yet added (deferred to the milestone that needs them, per brief section 14): `cpal`, `midir`
(standalone, v1 UI milestone), `egui` (ui crate), `criterion` (once real benchmark patches exist
in v1, not needed for a one-off spike test), `nih-plug` (crates/clap, v5), `fundsp` (optional,
not needed yet).

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
