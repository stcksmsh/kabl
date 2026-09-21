# Status

**Read this first.** Unlike `decisions.md` (append-only rationale log) and `benchmarks.md`
(append-only numbers log), this file is overwritten to reflect the current state of the project.
If you're a human or an agent picking this up cold, this is where you find out what's real,
what's a stand-in, and what's next — before reading any code.

Last updated: 2026-09-21, after commit `8a0fdad` (Spike S2) + the master-branch switch below.

## Workflow (changed 2026-09-21)

Owner (Kosta) said: push directly to `master`, no feature branches, no PRs. Commit frequently,
each commit meaningful (a working state + a real result, not a checkpoint). This repo has no
`main`/`master` before this commit — it was created here from the prior work done on a
throwaway session branch. Follow this workflow for everything from here on unless told
otherwise.

## Where we are, one paragraph

Pre-milestone: working through the brief's "week-one spikes" (section 11) before starting real
v1 (Engine) milestone work. S1 (graph swap/crossfade) and S2 (control-rate tier) are done and
passed. Nothing playable or audible exists yet — no module registry, no compiler, no UI, no
standalone binary. `core` (the op log) is real, production-shaped code, already at v1 quality.
Everything in `engine` so far is spike-scoped: hand-rolled fixed-topology graphs built to answer
one architectural question each, not the general compiler. That's intentional (brief section 2:
scope discipline) but means don't mistake spike code for the real engine.

## Spike checklist (brief section 11)

| Spike | Question | Status | Result |
|---|---|---|---|
| S1 | Can compiled graphs swap with state carry-over and no click? | **done, passed** | Steady-state residual bit-exact (0.0) outside crossfade; boundary curvature *below* typical in-chord curvature (0.58x); 0 allocations across 100 swaps. See `docs/decisions.md` "Spike S1". |
| S2 | Does block-held scalar classification meet the potato gate? | **done, mechanism validated; gate itself unverified** | Optimization is correct (-41.4dB vs. naive once settled) and measurably cheaper (~13% mean, noisy). **No pass/fail on the actual gate** — this container has no Pi-4 hardware and no `cpufreq`; owner chose not to fake a conversion factor. Open item: run `crates/engine/benches/s2_potato.rs` on real Pi-4 hardware. See `docs/decisions.md` "Spike S2". |
| S3 | Does f32x4 voice batching beat scalar by >=2.5x? | **not started** | Next up. |
| S4 | Is a WASM sine oscillator RT-safe in the callback? | **not started, optional** | Brief marks this deferrable; likely skipped in favor of going straight to v1 real work once S3 lands — confirm with owner before skipping outright. |

## v1 (Engine) milestone checklist (brief section 12)

What actually exists vs. what's still spike-scoped or missing:

| Piece | State |
|---|---|
| `core`: op log, undo/redo, coalescing, checkpoints, file format w/ schema version, property tests | **done, real.** `crates/core/`. |
| `engine`: flat-schedule compiler | **not built.** S1/S2 use hand-rolled fixed 2-node / 23-node graphs, not a general compiler. |
| `engine`: voice allocator | **not built.** |
| `engine`: SIMD batching | **not built** — this is S3. |
| `engine`: control-rate tier | **prototyped in spike S2 only** (`potato.rs`), not integrated into a real compiler. |
| `engine`: swap + crossfade | **mechanism proven in S1** (`swap.rs`, `graph.rs`), but only for S1's specific 2-node shape — needs generalizing when the real compiler exists. |
| `engine`: quality tiers (Live/Render) | **not built.** |
| `cables`: depth only | **not built.** `crates/cables` is an empty stub. |
| `modules`: 9 v1 built-ins + metadata | **not built as a real module system.** DSP exists ad hoc inside spike code (`Saw`, `Svf`, `Lfo`, `Adsr` in `engine/src/dsp.rs`) — correct signal processing, but no `ModuleInfo`, no registry, no `Module` trait impls. `crates/modules` is an empty stub. |
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
  engine/      SPIKE CODE ONLY so far.
                 dsp.rs    - Saw (PolyBLEP), Svf (TPT/Zavalishin), Lfo, Adsr. Real DSP, reusable.
                 graph.rs  - S1's fixed 2-node (4-voice chord -> cable depth -> filter) graph.
                 swap.rs   - S1's Engine: crossfade swap + basedrop deferred drop.
                 potato.rs - S2's fixed 20+3-module patch, naive vs. control-rate-optimized.
               None of this is the general compiler. Expect it to be replaced/absorbed, not
               extended indefinitely, once real compiler work starts.
  modules/, cables/, pedals/, learn/, ui/, standalone/, clap/
               STUBS. `//! Stub — not yet implemented.` One-line lib.rs each, empty Cargo.toml
               deps. Scaffolded so the workspace builds; no logic.
benches/       (top-level, per brief's tree) — not where Cargo benches actually live; see
               docs/decisions.md's "Structural note" under the dependency table. Real criterion
               harness: crates/engine/benches/s2_potato.rs.
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

# S2's benchmark specifically (see docs/benchmarks.md for why taskset, and its caveats):
taskset -c 0 cargo bench -p kabl-engine --bench s2_potato
```

All green as of commit `8a0fdad`.

## Open items (not decided, not blocking, but real)

- **S2's potato-gate number** needs real Pi-4 hardware — this container can't produce one
  honestly (see decisions.md "Spike S2").
- **S4 (wasmtime)** — optional per brief; leaning toward skipping to get to real v1 work faster,
  but that's a call to confirm with the owner, not decide silently.
- **Brief section 16's open questions** (voice-rate/global-rate module reassignment, feedback
  loop semantics, standalone-vs-CLAP-first timing, sharing the op-log model with Hysteresis,
  MIDI learn/MPE timing) — none hit yet in practice; will surface once real module/compiler work
  starts. Not yet raised because nothing has forced a decision.
- **`RemoveModule` undo losing param history** and **`SetParam`'s 0.0 fallback for a
  never-set param** — both documented, intentional-for-now simplifications in `core`. See
  decisions.md "core: op log inverse simplifications." Revisit if they cause a real problem.

## What to read next, depending on what you're about to do

- **Continuing spike/engine work:** `docs/decisions.md`'s S1 and S2 entries (method + results),
  then the code in `crates/engine/src/`.
- **Starting real v1 compiler/module work:** read this file's "v1 milestone checklist" above
  first — it's the actual gap list. The brief's sections 6-8 (patch model, engine, module
  interface) are the spec; nothing here implements the module registry or compiler yet.
- **Just want to know if it works:** `cargo test --workspace` and the commands above. If they're
  not all green, the repo is mid-edit — check `git log` for the last commit's message.
