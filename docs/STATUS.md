# Status

**Read this first.** Unlike `decisions.md` (append-only rationale log) and `benchmarks.md`
(append-only numbers log), this file is overwritten to reflect the current state of the project.
If you're a human or an agent picking this up cold, this is where you find out what's real,
what's a stand-in, and what's next — before reading any code.

Last updated: 2026-09-21, after `osc.va` gained square/triangle/sine + hard sync.

## Autonomous overnight work (started 2026-09-21)

Owner: fully autonomous, work the backlog, conserve tokens, self-restart if usage runs out, no
further check-in expected until morning. Working self-contained items only (nothing needing
hardware, a UI, or a human ear) — see each item's decisions.md entry for what was picked and why.
If you're reading this mid-run: check `git log` for the latest commit and this file's "Where we
are" below for the current real state; nothing here should be stale by more than one work chunk.

## Workflow (changed 2026-09-21)

Owner (Kosta) said: push directly to `master`, no feature branches, no PRs. Commit frequently,
each commit meaningful (a working state + a real result, not a checkpoint). This repo has no
`main`/`master` before this commit — it was created here from the prior work done on a
throwaway session branch. Follow this workflow for everything from here on unless told
otherwise.

## Where we are, one paragraph

Week-one spikes (brief section 11) are done: S1 (graph swap/crossfade) and S2 (control-rate
tier) passed; S3 (SIMD voice batching) landed with solid correctness but missed its 2.5x target
(~1.5-1.7x measured) — reported honestly rather than massaged. S4 (wasmtime) deliberately
skipped — doesn't gate any v1 decision, revisit at v4. **All 9 of brief section 8's v1 built-in
modules now have a real `Module` trait implementation**, in `crates/modules/src/builtins/`:
`osc.va`, `filter.svf`, `env.adsr`, `lfo`, `vca`, `ringmod`, `mixer`, `out`, `midi.in`. `osc.va`
now has all four waveforms (sine/triangle/saw/square) plus hard sync (triangle is naive, not
PolyBLEP/BLAMP-corrected — documented, not hidden). `lfo` still has all waveforms but no sync
(needs a clock, v3 scope) — tracked, not hidden, see decisions.md. `filter.svf` and `env.adsr`
directly apply spike S3's and S2's
findings (cache coefficients when nothing's actually modulating per-sample) in real module code,
not just as spike war stories. **A 4-voice patch built entirely from real `Module` trait objects now plays** —
`crates/engine/src/patch_demo.rs`: `midi.in -> osc.va -> filter.svf -> env.adsr/vca -> mixer ->
out`, 22 module instances, wired by hand (no cable system or compiler yet). Renders a real,
correct C-major chord with a working release envelope — caught one real integration bug (vca
gain-staging that silently defeated the envelope) before ever running anything, exactly the
class of bug component-level tests can't catch. WAV sent to the owner. This is the last thing
standing between "9 modules exist" and "the compiler can be trusted to assemble them" — proof
the trait design actually composes.

**The flat-schedule compiler now exists and works**, `crates/engine/src/compile.rs`:
`compile(&PatchState, sample_rate, voice_count) -> Result<CompiledPatch, CompileError>` turns an
ops-authored patch into a running graph — topo sort, voice/global rate instancing, cable wiring,
cycle detection, a `recompile()` that carries module state across a rebuild. Proven end-to-end:
the same 5-stage voice chain `patch_demo.rs` hand-wired, built from `PatchState` ops instead,
compiled, and driven via MIDI — output matches `patch_demo.rs`'s numbers exactly. WAV sent to
owner. See decisions.md's "Flat-schedule compiler v1" entry for the four design calls made here
(averaging not summing for voice->global, cycle-as-compile-error, no buffer-pool reuse yet,
params as compile-time constants).

**`process_block` is allocation-free**, proven the same way spike S1 proved it for the swap
mechanism: `crates/engine/tests/compile_rt_safety.rs` runs 200 blocks of the real chord patch
inside `assert_no_alloc!`. Got there by replacing 4 per-call `Vec`s with fixed-size stack arrays
sized to `MAX_INPUTS`/`MAX_OUTPUTS`/`MAX_PARAMS` (measured off every built-in's port/param
counts); `compile()` now rejects a module exceeding those bounds with `CompileError::TooManyPorts`
instead of the audio thread ever truncating or panicking.

**The compiler is now wired into S1's swap mechanism**: `crates/engine/src/patch_engine.rs::
PatchEngine`, a stereo/arbitrary-topology counterpart to `swap::Engine`, reusing the same
equal-power crossfade curve and `basedrop` deferred-drop pattern. `build_swap` calls
`compile::recompile()` (control thread), `process_block` blends active+incoming exactly like S1
(audio thread, no allocation). Proven in `tests/patch_engine_swap.rs`: swaps the same 5-stage
chord patch onto itself 20 times and checks the output against a never-swapped reference — bit-
exact outside every crossfade window. **Caught a real bug doing this**: `env.adsr`'s state
carry-over was missing `FullAdsr::gate_was_high`, so a continuously-held gate looked like a fresh
note-on on every recompile, re-triggering `Attack` each time — invisible in a single recompile,
glaringly wrong after 20 in a row. Fixed; see decisions.md's "`PatchEngine`: wiring the compiler
into S1's swap mechanism" entry.

Still missing before anything is playable *by a person*: cables (v2 params beyond simple
wiring), a UI, a standalone binary with real MIDI input to actually call `build_swap` from. `core`
(the op log) is real, production-shaped code, already at v1 quality. `engine`'s S1/S2/S3 spike
code is still hand-rolled fixed-topology graphs using raw `dsp` primitives directly — expect it
to be absorbed into/replaced by the real compiler, not extended indefinitely.

## Handover: next session starts here

The compiler is built, RT-safe, swappable, tested, and committed — every item on the "make the
compiler runnable" list from the last few handovers is now done. What's NOT done, in rough
priority order for "something a person can actually patch and hear live":

1. **No control surface exists to actually drive `PatchEngine`** — no UI, no standalone binary
   with real MIDI/audio I/O. `PatchEngine::build_swap`/`receive_swap` are proven correct but
   nothing outside a test calls them yet. This is arguably the next big milestone (brief section
   12's `standalone`/`ui` crates, both still empty stubs).
2. **Buffer-pool reuse**: every port gets its own fresh buffer right now; fine for correctness,
   wasteful for anything beyond test-sized patches.
3. Cycle handling still hard-errors instead of brief section 7.1's implicit 1-block delay — not
   needed by any v1 accept-test patch, but real feedback patches will hit it.
4. No voice allocator — `voice_count` is a fixed `compile()` parameter, not dynamically assigned
   from incoming MIDI note-on/off.
5. Overlapping swaps (a second `build_swap` while one is still crossfading) aren't handled by
   either `Engine` or `PatchEngine` — a pre-existing S1 gap, not new.

No new open question from the owner to resolve first — the natural next chunk is #1 above (a real
milestone: standalone binary or UI) — ask before picking one un-prompted since this is a
milestone boundary (brief section 17).

## Spike checklist (brief section 11)

| Spike | Question | Status | Result |
|---|---|---|---|
| S1 | Can compiled graphs swap with state carry-over and no click? | **done, passed** | Steady-state residual bit-exact (0.0) outside crossfade; boundary curvature *below* typical in-chord curvature (0.58x); 0 allocations across 100 swaps. See `docs/decisions.md` "Spike S1". |
| S2 | Does block-held scalar classification meet the potato gate? | **done, mechanism validated; gate itself unverified** | Optimization is correct (-41.4dB vs. naive once settled) and measurably cheaper (~13% mean, noisy). **No pass/fail on the actual gate** — this container has no Pi-4 hardware and no `cpufreq`; owner chose not to fake a conversion factor. Open item: run `crates/engine/benches/s2_potato.rs` on real Pi-4 hardware. See `docs/decisions.md` "Spike S2". |
| S3 | Does f32x4 voice batching beat scalar by >=2.5x? | **done, target missed** | Bit-exact correctness vs. scalar (not an approximation). Measured ~1.5-1.7x (range 1.3-1.8x across 11 runs, two granularities), below the 2.5x bar. Root cause of the shortfall investigated but inconclusive (leading hypothesis: PolyBLEP's branchless form vs. scalar's near-free predicted branch — see decisions.md for the failed isolation attempt). aarch64 not measured (no ARM hardware in this container). Open call for the owner: still use SIMD batching in v1 at this ratio, or does missing 2.5x change the decision? See `docs/decisions.md` "Spike S3". |
| S4 | Is a WASM sine oscillator RT-safe in the callback? | **skipped, deliberately** | Doesn't gate any v1 decision — brief itself sequences sandboxed WASM modules behind composite pedals and Faust->Rust, both v4. Revisit when v4 (pedals) actually starts. See decisions.md "Skipping S4". |

## v1 (Engine) milestone checklist (brief section 12)

What actually exists vs. what's still spike-scoped or missing:

| Piece | State |
|---|---|
| `core`: op log, undo/redo, coalescing, checkpoints, file format w/ schema version, property tests | **done, real.** `crates/core/`. |
| `engine`: flat-schedule compiler | **built, correctness-tested, RT-safe.** `compile.rs`: topo sort, voice/global instancing, cycle detection, `recompile()` w/ state carry-over. `process_block` proven allocation-free (`tests/compile_rt_safety.rs`). No buffer-pool reuse yet. |
| `engine`: voice allocator | **not built.** `voice_count` is a fixed compile-time parameter, not dynamically assigned from incoming MIDI notes. |
| `engine`: SIMD batching | **prototyped in spike S3 only** (`simd_voices.rs`), not integrated into the compiler; measured ~1.5-1.7x speedup (target was 2.5x, missed — see decisions.md). |
| `engine`: control-rate tier | **prototyped in spike S2 only** (`potato.rs`), not integrated into the compiler. |
| `engine`: swap + crossfade | **wired to the real compiler.** `patch_engine.rs::PatchEngine` generalizes S1's `swap.rs::Engine` mechanism (equal-power crossfade, `basedrop` deferred drop) to arbitrary-topology `CompiledPatch`es via `recompile()`. Proven in `tests/patch_engine_swap.rs` (bit-exact outside crossfade, no allocation). No control surface calls it yet — see open items. |
| `engine`: quality tiers (Live/Render) | **not built.** |
| `cables`: depth only | **not built.** `crates/cables` is an empty stub. |
| `modules`: 9 v1 built-ins + metadata | **9 of 9 have a `Module` impl.** `Module` trait, `ModuleInfo`, `ProcessIo`/`Signal` all built and tested. `osc.va` now has all 4 waveforms + hard sync (triangle naive, not BLEP/BLAMP-corrected). `lfo` has no sync (needs a clock, v3 scope) — see decisions.md. **Registry now exists** (`registry.rs`: `create(kind)`, `all_infos()`, `info_for(kind)`) — the compiler uses it to turn `ModuleState.kind` strings into instances. |
| `learn`: unlock flags filter catalog | **not built.** `crates/learn` is an empty stub. |
| `ui`: egui patchbay | **not built.** `crates/ui` is an empty stub. |
| `standalone`: cpal + midir + JACK | **not built.** `crates/standalone` is an empty stub. |
| **v1 accept test** (play a 4-voice MIDI chord, repatch live no click, undo, save, reload, replay construction log; potato gate passes) | **not achievable yet** — no UI, no standalone binary, no MIDI input, no real module system to patch. |

Bottom line: the *risky architectural bets* (event-sourced patch, cable-as-node swap mechanism,
control-rate optimization) are de-risked. The *product* — something Kosta can open, patch, and
hear — doesn't exist yet. That's the next phase of work.

## Repo map

```
crates/
  core/        REAL. Op log, PatchState, PatchLog, file format. No audio deps. Fully tested.
  modules/     REAL, v1 built-ins done. Brief section 8's module system.
                 info.rs      - ModuleInfo, Category, Rate, PortInfo, ParamInfo, QualitySupport.
                 io.rs        - ProcessIo, Signal (scalar-or-buffer, with the .at(i) helper).
                 module.rs    - the Module trait, QualityConfig/Tier, StateWriter/StateReader.
                 dsp.rs       - Saw/Svf/Lfo/Adsr (S1-S3's originals) + SvfOutputs/FullAdsr/
                                FullLfo (added for the 8 newer modules, originals untouched).
                 builtins/    - all 9 Module impls: osc_va, filter_svf, env_adsr, lfo, vca,
                                ringmod, mixer, out, midi_in. All 9 also implement `as_any`/
                                `as_any_mut` (required, not defaulted — see decisions.md) so the
                                compiler can downcast `Box<dyn Module>` back to a concrete type.
                 registry.rs  - kind string -> Module factory. KNOWN_KINDS, create(kind),
                                all_infos(), info_for(kind). Tested in tests/registry.rs.
  engine/      Real compiler + spike code (depends on kabl-modules, kabl-core).
                 compile.rs - THE COMPILER. compile(&PatchState, sample_rate, voice_count) ->
                                CompiledPatch: topo sort (Kahn's algorithm, doubles as cycle
                                detection), voice/global-rate instancing, voice->global averaging
                                (SumVoices steps), recompile() with save_state/load_state carry-
                                over via HashMapState. Correctness-tested end-to-end in
                                tests/compile.rs (7 tests) against patch_demo.rs's numbers.
                                process_block is allocation-free (fixed-size stack scratch sized
                                to MAX_INPUTS/MAX_OUTPUTS/MAX_PARAMS; compile() rejects a module
                                exceeding them) -- proven in tests/compile_rt_safety.rs via
                                assert_no_alloc, same pattern as spike S1. NOT yet wired to
                                swap.rs. See decisions.md "Flat-schedule compiler v1" and
                                "process_block made allocation-free".
                 patch_engine.rs - PatchEngine: generalizes swap.rs's Engine (equal-power
                                crossfade, basedrop deferred drop) to CompiledPatch (arbitrary
                                topology, stereo, recompile()-based rebuild instead of S1's single
                                cable_depth knob). Proven in tests/patch_engine_swap.rs: swaps a
                                real chord patch onto itself 20x, bit-exact vs. a never-swapped
                                reference outside every crossfade window, no allocation. See
                                decisions.md "PatchEngine: wiring the compiler into S1's swap
                                mechanism" -- also where env.adsr's gate_was_high state-
                                carry-over bug was caught and fixed.
                 graph.rs  - S1's fixed 2-node (4-voice chord -> cable depth -> filter) graph.
                 swap.rs   - S1's Engine: crossfade swap + basedrop deferred drop.
                 potato.rs - S2's fixed 20+3-module patch, naive vs. control-rate-optimized.
                 simd_voices.rs - S3's SawX4/SvfX4 (wide::f32x4) vs. scalar ScalarVoices/SimdVoices.
                 patch_demo.rs  - first patch built from real Module trait objects (not raw dsp
                                  calls like the other 3): 4 voices, 22 module instances, hand-
                                  wired. Correctness+bench in tests/patch_integration.rs and
                                  benches/patch_integration.rs; a sequenced melody (not just a
                                  held chord) in tests/melody.rs via Patch::note_on/note_off.
                 dyn_dispatch_spike.rs - same topology as patch_demo.rs, Box<dyn Module> fields
                                  instead of concrete typed fields; measures vtable dispatch cost
                                  (~17-19% over static). Correctness+bench in
                                  tests/dyn_dispatch_spike.rs and benches/dyn_dispatch_spike.rs.
               None of this is the general compiler. Expect it to be replaced/absorbed, not
               extended indefinitely, once real compiler work starts.
  cables/, pedals/, learn/, ui/, standalone/, clap/
               STUBS. `//! Stub — not yet implemented.` One-line lib.rs each, empty Cargo.toml
               deps. Scaffolded so the workspace builds; no logic.
benches/       (top-level, per brief's tree) — not where Cargo benches actually live; see
               docs/decisions.md's "Structural note" under the dependency table. Real criterion
               harnesses: crates/engine/benches/{s2_potato,s3_simd_voices}.rs.
tests/golden/  Empty, scaffolded. No golden-file renders exist yet (needs real modules first).
lessons/       Empty, scaffolded. Content is v5 scope.
docs/
  STATUS.md       This file. Current state, overwritten each update.
  decisions.md    Append-only: every non-trivial choice + alternative rejected, with dates.
  proposals.md    Append-only: out-of-scope ideas, parked. Empty so far.
  confusions.md   Append-only: owner's beginner confusions, lesson material. Empty so far.
  benchmarks.md   Append-only: measured numbers, by spike/benchmark-patch, with dates.
```

## How to build / test / bench

```
cargo build --workspace          # everything, including stub crates
cargo test --workspace           # core's property tests + engine's spike tests
cargo clippy --workspace --all-targets
cargo fmt --all -- --check

# S2/S3 benchmarks specifically (see docs/benchmarks.md for why taskset, and their caveats):
taskset -c 0 cargo bench -p kabl-engine --bench s2_potato
taskset -c 0 cargo bench -p kabl-engine --bench s3_simd_voices
```

All green as of the latest commit on `master` — check `git log -1` to confirm you're reading
this against the commit it was last updated for.

## Open items (not decided, not blocking, but real)

- **S2's potato-gate number** needs real Pi-4 hardware — this container can't produce one
  honestly (see decisions.md "Spike S2").
- **S3 missed its 2.5x target** (~1.5-1.7x measured) — root cause of the shortfall not fully
  pinned down (see decisions.md "Spike S3"). Open call: still use SIMD voice batching in v1 at
  this ratio, or does the miss change the decision? Also no aarch64 measurement (brief section
  11 asks for it "if available" — not available in this container).
- ~~**S4 (wasmtime)**~~ — skipped, see decisions.md. Not blocking, revisit at v4.
- **Brief section 16's open questions** (voice-rate/global-rate module reassignment, feedback
  loop semantics, standalone-vs-CLAP-first timing, sharing the op-log model with Hysteresis,
  MIDI learn/MPE timing) — none hit yet in practice; will surface once real module/compiler work
  starts. Not yet raised because nothing has forced a decision.
- **`RemoveModule` undo losing param history** and **`SetParam`'s 0.0 fallback for a
  never-set param** — both documented, intentional-for-now simplifications in `core`. See
  decisions.md "core: op log inverse simplifications." Revisit if they cause a real problem.
- ~~**`osc.va` is saw-only, no hard sync**~~ — fixed. All 4 waveforms + hard sync, see
  decisions.md "`osc.va`: square/triangle/sine waveforms + hard sync". Triangle stays naive
  (not PolyBLEP/BLAMP-corrected) — a real gap, just a smaller one, flagged in `dsp::FullOsc`'s
  doc comment.
- ~~**No module registry/catalog struct**~~ — built, `crates/modules/src/registry.rs`.
- ~~**Compiler's `process_block()` is not RT-safe**~~ — fixed. Fixed-size stack scratch, no
  per-call `Vec`s; proven allocation-free via `assert_no_alloc` in
  `tests/compile_rt_safety.rs`. See decisions.md "`process_block` made allocation-free".
- ~~**Compiler not wired to `swap.rs`'s `Engine`**~~ — fixed. `patch_engine.rs::PatchEngine`;
  proven in `tests/patch_engine_swap.rs`. Along the way, fixed a real bug: `env.adsr` wasn't
  carrying `FullAdsr::gate_was_high` across recompile, so a held gate re-triggered `Attack` on
  every swap. See decisions.md "`PatchEngine`: wiring the compiler into S1's swap mechanism".
- **No control surface drives `PatchEngine` yet** — no UI, no standalone binary. It's proven
  correct in tests but nothing outside a test calls `build_swap`/`receive_swap` from real input.
- **No buffer-pool reuse in the compiler** — every port gets a fresh `[f32; BLOCK]`. Correct,
  wasteful; deferred as a pure optimization on the same schedule shape.
- **Compiler treats a cycle as a hard compile error**, not brief section 7.1's implicit 1-block
  delay. No v1 accept-test patch has a feedback loop, so not currently blocking; will matter for
  real feedback patches.
- **No voice allocator** — `voice_count` is a fixed parameter passed to `compile()`, not
  dynamically assigned from incoming MIDI note-on/off.
- **Overlapping swaps unhandled** — a second `build_swap` while one is still crossfading isn't
  accounted for in `swap::Engine` or `PatchEngine`. Pre-existing S1 gap, not new.
- ~~**`dyn Module` dispatch cost unmeasured**~~ — measured. `crates/engine/src/
  dyn_dispatch_spike.rs`: `Box<dyn Module>` costs ~17-19% more than static dispatch on the same
  22-instance patch (bit-exact correctness, tight ratio across runs). Not a blocker — `Box<dyn
  Module>` is the only representation that fits an arbitrary module-kind mix; the compiler build
  proceeds on that basis, budgeted for ~1.2x over `patch_demo.rs`'s static numbers, not a
  different order of magnitude. See decisions.md.

## What to read next, depending on what you're about to do

- **Continuing spike/engine work:** `docs/decisions.md`'s S1/S2/S3 entries (method + results),
  then the code in `crates/engine/src/`.
- **Modifying a built-in module or adding a composite/pedal later:** `crates/modules/src/
  builtins/*.rs` — any of the 9 works as a template; `filter_svf.rs` and `env_adsr.rs` are the
  most complete examples (fast/slow-path split, real state carry-over). Test files
  `crates/modules/tests/*.rs` show the expected shape (matches-direct-dsp-call, buffer input,
  reset, save/load-state round-trip).
- **Extending the compiler** (buffer-pool reuse, cycle handling, voice allocator): `crates/engine/
  src/compile.rs`'s module doc comment + decisions.md's "Flat-schedule compiler v1" and
  "`process_block` made allocation-free" entries lay out what's built, what's deferred, and why.
  `crates/engine/tests/compile.rs` shows the expected external shape (build a `PatchState` from
  ops, `compile()`, drive it, `recompile()`); `tests/compile_rt_safety.rs` shows the
  `assert_no_alloc` pattern to keep any future change RT-safe.
- **Starting the standalone binary or UI** (the actual next milestone): `patch_engine.rs::
  PatchEngine` is the thing to drive — `build_swap(handle, patch)` on the control thread,
  `receive_swap`/`process_block` on the audio thread, same `Owned<T>`-over-a-channel handoff as
  spike S1. `tests/patch_engine_swap.rs` is a complete worked example of the whole lifecycle
  (minus real MIDI/audio I/O, which is what a standalone binary would add).
- **Just want to know if it works:** `cargo test --workspace` and the commands above. If they're
  not all green, the repo is mid-edit — check `git log` for the last commit's message.
