# Status

**Read this first.** Unlike `decisions.md` (append-only rationale log) and `benchmarks.md`
(append-only numbers log), this file is overwritten to reflect the current state of the project.
If you're a human or an agent picking this up cold, this is where you find out what's real,
what's a stand-in, and what's next — before reading any code.

Last updated: 2026-09-21, after the first patch built from real `Module` trait objects.

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
`osc.va`, `filter.svf`, `env.adsr`, `lfo`, `vca`, `ringmod`, `mixer`, `out`, `midi.in`. Two are
partial (`osc.va`: saw only, no hard sync; `lfo`: all waveforms but no sync) — tracked, not
hidden, see decisions.md. `filter.svf` and `env.adsr` directly apply spike S3's and S2's
findings (cache coefficients when nothing's actually modulating per-sample) in real module code,
not just as spike war stories. **A 4-voice patch built entirely from real `Module` trait objects now plays** —
`crates/engine/src/patch_demo.rs`: `midi.in -> osc.va -> filter.svf -> env.adsr/vca -> mixer ->
out`, 22 module instances, wired by hand (no cable system or compiler yet). Renders a real,
correct C-major chord with a working release envelope — caught one real integration bug (vca
gain-staging that silently defeated the envelope) before ever running anything, exactly the
class of bug component-level tests can't catch. WAV sent to the owner. This is the last thing
standing between "9 modules exist" and "the compiler can be trusted to assemble them" — proof
the trait design actually composes.

Still missing before anything is playable *by a person*: the flat-schedule compiler that builds
graphs like `patch_demo.rs`'s from patch ops instead of hand-written Rust, cables, a UI, a
standalone binary with real MIDI input. `core` (the op log) is real, production-shaped code,
already at v1 quality. `engine`'s S1/S2/S3 spike code is still hand-rolled fixed-topology graphs
using raw `dsp` primitives directly, separate from `patch_demo.rs`'s real-`Module` approach —
expect both to be absorbed into real compiler work, not extended indefinitely.

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
| `engine`: flat-schedule compiler | **not built.** S1/S2 use hand-rolled fixed 2-node / 23-node graphs, not a general compiler. |
| `engine`: voice allocator | **not built.** |
| `engine`: SIMD batching | **prototyped in spike S3 only** (`simd_voices.rs`), not integrated into a real compiler; measured ~1.5-1.7x speedup (target was 2.5x, missed — see decisions.md). |
| `engine`: control-rate tier | **prototyped in spike S2 only** (`potato.rs`), not integrated into a real compiler. |
| `engine`: swap + crossfade | **mechanism proven in S1** (`swap.rs`, `graph.rs`), but only for S1's specific 2-node shape — needs generalizing when the real compiler exists. |
| `engine`: quality tiers (Live/Render) | **not built.** |
| `cables`: depth only | **not built.** `crates/cables` is an empty stub. |
| `modules`: 9 v1 built-ins + metadata | **9 of 9 have a `Module` impl.** `Module` trait, `ModuleInfo`, `ProcessIo`/`Signal` all built and tested. `osc.va` (saw only) and `lfo` (no sync) are partial — see decisions.md. No registry/catalog struct yet (nothing has needed to enumerate "all modules" as a collection so far; tests just import each type directly). |
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
                                ringmod, mixer, out, midi_in.
  engine/      SPIKE CODE ONLY so far (depends on kabl-modules).
                 graph.rs  - S1's fixed 2-node (4-voice chord -> cable depth -> filter) graph.
                 swap.rs   - S1's Engine: crossfade swap + basedrop deferred drop.
                 potato.rs - S2's fixed 20+3-module patch, naive vs. control-rate-optimized.
                 simd_voices.rs - S3's SawX4/SvfX4 (wide::f32x4) vs. scalar ScalarVoices/SimdVoices.
                 patch_demo.rs  - first patch built from real Module trait objects (not raw dsp
                                  calls like the other 3): 4 voices, 22 module instances, hand-
                                  wired. Correctness+bench in tests/patch_integration.rs and
                                  benches/patch_integration.rs.
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
- **`osc.va` is saw-only, no hard sync** — brief section 8's table entry wants square/tri/sine
  and a hard-sync input too. Tracked, not forgotten.
- **No module registry/catalog struct** — every test imports each `Module` type directly by
  name. Fine for 9 hand-known modules; will matter once the UI needs to enumerate "everything
  available" or `learn`'s unlock flags need to filter a list. Not built until something needs it.
- **`dyn Module` dispatch cost unmeasured** — `patch_demo.rs` uses concrete typed fields
  (static dispatch), not the `Box<dyn Module>` heterogeneous collection the real compiler needs.
  Its ns/block number doesn't predict the real compiler's cost until someone measures the
  vtable-indirection overhead specifically. Flagged for whoever builds the compiler.

## What to read next, depending on what you're about to do

- **Continuing spike/engine work:** `docs/decisions.md`'s S1/S2/S3 entries (method + results),
  then the code in `crates/engine/src/`.
- **Modifying a built-in module or adding a composite/pedal later:** `crates/modules/src/
  builtins/*.rs` — any of the 9 works as a template; `filter_svf.rs` and `env_adsr.rs` are the
  most complete examples (fast/slow-path split, real state carry-over). Test files
  `crates/modules/tests/*.rs` show the expected shape (matches-direct-dsp-call, buffer input,
  reset, save/load-state round-trip).
- **Starting the real compiler:** read this file's "v1 milestone checklist" above first — it's
  the actual gap list. Brief section 7 is the spec. `Module`/`ModuleInfo`/`ProcessIo` already
  exist (`crates/modules/`) for the compiler to build graphs out of, and `crates/engine/src/
  patch_demo.rs` shows a working topology hand-wired the way the compiler needs to do it
  automatically from ops — read that first, it's the shape to generalize. Measure `dyn Module`
  dispatch cost before assuming `patch_demo.rs`'s static-dispatch ns/block predicts anything.
- **Just want to know if it works:** `cargo test --workspace` and the commands above. If they're
  not all green, the repo is mid-edit — check `git log` for the last commit's message.
