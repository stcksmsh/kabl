# Status

**Read this first.** Unlike `decisions.md` (append-only rationale log) and `benchmarks.md`
(append-only numbers log), this file is overwritten to reflect the current state of the project.
If you're a human or an agent picking this up cold, this is where you find out what's real,
what's a stand-in, and what's next — before reading any code.

Last updated: 2026-09-22, after persistence (save/load) was wired into both `standalone`/`kabl-ui`.

## Autonomous overnight work (started 2026-09-21)

Owner: fully autonomous, work the backlog, conserve tokens, self-restart if usage runs out, no
further check-in expected until morning. Originally scoped to self-contained items only (nothing
needing hardware, a UI, or a human ear) — owner then explicitly said "do the UI yourself, do
everything, we can easily fix it later," authorizing the `standalone`/`ui` crates too. Built
standalone first (cpal+midir — the "hear it" milestone), then `kabl-ui` (the patchbay). **This
container has no audio hardware (`/dev/snd` doesn't exist) and no display server** — checked, not
assumed. For the UI specifically, a virtual X server (`Xvfb`) + Mesa's software GL renderer
(`llvmpipe`, already installed) turned out to be enough to actually *run* it headlessly and
capture a real screenshot (sent to owner) — so unlike `standalone`'s audio path, the UI's
rendering and one real interaction (a simulated "Add module" click) are genuinely verified here,
not just built and logic-tested. Audio playback and MIDI hardware still need a real machine. See
each item's decisions.md entry for what was picked and why. If you're reading this mid-run: check
`git log` for the latest commit and this file's "Where we are" below for the current real state;
nothing here should be stale by more than one work chunk.

**Context handover note (2026-09-22, after commit `7a3793c`)**: owner cleared context here on
purpose — same pattern as earlier in this project (read this file cold, continue from it, no
recap needed). A `send_later` self-continuation trigger is active in this session (fires roughly
every 20-30 min with an instruction to read this file's "Handover" section and pick the next
self-contained backlog item, same rigor as every prior chunk: plan, implement, test, verify
clean, document, commit, push, reschedule). If you're the session that trigger just woke: just
follow its instructions directly, this paragraph is only here so a human glancing at this file
understands why work might keep happening with no one watching.

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

**The standalone binary now exists**: `crates/standalone`, `cargo run -p kabl-standalone` (binary
name `kabl`). Opens the default audio output device + first MIDI input port, compiles
`default_patch()` (a full `midi.in -> osc.va -> filter.svf -> env.adsr/vca -> out` polysynth,
`DEFAULT_VOICE_COUNT`=8 voices via `VoiceAllocator`), and plays it live through `PatchEngine`.
MIDI resolution (which needs `VoiceAllocator`'s `HashMap`, not RT-safe) happens on the MIDI
thread; only resolved `VoiceEvent`s cross an `rtrb` channel to the audio callback, applied with
no allocation. **Unverified in this environment** — no `/dev/snd`, no MIDI hardware here — the
binary correctly detects that and falls back to rendering a self-test WAV instead (sent to
owner), proving the synthesis path but not real device I/O. Run it on a real machine to confirm
actual playback. See decisions.md "Standalone binary" for the full design.

**`kabl-ui` (the patchbay) now exists**: `crates/ui`, binary `kabl-ui`. A real node-graph editor —
modules as draggable positioned boxes, click-a-port-then-click-a-port to cable, per-module param
sliders, undo/redo — built on `editor.rs::PatchEditor`, which drives `kabl_core::PatchLog`
directly (every edit is a real `Op`, so undo/redo is the existing log machinery, not reinvented).
`main.rs` embeds the same `cpal`/`midir`/`PatchEngine` audio path `standalone` uses, recompiling
and hot-swapping on every edit. **Actually run and screenshotted in this container** (Xvfb +
software GL) — caught and fixed a real bug this way: `default_patch()`'s modules all shared
position `(0,0)` and rendered stacked on top of each other; now staggered. See decisions.md
"`kabl-ui`: the patchbay" for the full design, including a flagged, deliberate compromise (the
audio callback and UI thread share one `PatchEngine` behind a `Mutex` with `try_lock` on the
audio side — real glitch risk on every edit, not a crash risk; the textbook fix needs restructuring
`PatchEngine` around a lock-free state-snapshot publish, not attempted blind).

**Persistence now exists**: `standalone --patch <dir>` loads a saved patch (`--save-default <dir>`
writes `default_patch()` out as a starting point); `kabl-ui`'s toolbar has a "patch dir" field with
real Save/Load buttons, wired to `core::{save, load}`. Load replaces the live patch and correctly
triggers a recompile+swap (`PatchEditor::mark_dirty()` — a real bug caught and fixed while
building this, see decisions.md). **Verified actually saving/loading in this container**: ran
`kabl-ui` under Xvfb, clicked the real Save button, confirmed the file landed on disk with the
right shape and the toolbar showed "saved to my-patch". See decisions.md "Persistence" for the
full design (why a text field not a native file picker, why a hand-rolled 2-flag CLI parser not a
new dependency).

Still missing before it's the *complete* "person can open, patch, and hear" product: cables (v2
params beyond simple wiring), canvas panning in `kabl-ui` (modules placed off the default window
width are simply clipped, no scroll/zoom), and — the big one — nothing here has run on real audio
hardware, a real MIDI controller, or a real display yet. `core` (the op log) is real, production-
shaped code, already at v1 quality. `engine`'s S1/S2/S3 spike code is still hand-rolled fixed-
topology graphs using raw `dsp` primitives directly — expect it to be absorbed into/replaced by
the real compiler, not extended indefinitely.

## Handover: next session starts here

The compiler is built, RT-safe, swappable, tested, and committed. A standalone binary and a
patchbay UI both exist, both embed live audio, and both can now save/load a real patch file.
Every crate in brief section 12's table now has *something* real in it except
`cables`/`pedals`/`learn`/`clap` (still stubs, later milestones). What's NOT done, in rough
priority order for "something a person can actually patch and hear live":

1. **Nothing has been run on real audio/MIDI hardware or a real display.** This is the single
   biggest remaining unknown — everything here was built against API docs/source and verified as
   much as a headless container allows (Xvfb screenshots, real clicks via `xdotool`, WAV self-
   tests, `assert_no_alloc`, a real save-to-disk check), but never actually heard through speakers
   or played from a real MIDI controller. Run `cargo run -p kabl-standalone` and `cargo run -p
   kabl-ui` on a real machine first, before trusting any of the audio claims beyond "it compiles
   and the logic tests pass."
2. **`kabl-ui`'s `Mutex`-sharing compromise** (see decisions.md) — works, flagged as not
   textbook-RT-safe, real follow-up work if edit-time glitches turn out to be audible/annoying in
   practice on real hardware.
3. **Buffer-pool reuse**: every port gets its own fresh buffer right now; fine for correctness,
   wasteful for anything beyond test-sized patches.
4. Cycle handling still hard-errors instead of brief section 7.1's implicit 1-block delay — not
   needed by any v1 accept-test patch, but real feedback patches will hit it.
5. Overlapping swaps (a second `build_swap` while one is still crossfading) aren't handled by
   either `Engine` or `PatchEngine` — a pre-existing S1 gap, not new.
6. No canvas pan/scroll in `kabl-ui` — a patch wider than the window is simply clipped.
7. `kabl-ui`'s Save/Load uses a plain text path field, not a native file picker (deliberate, see
   decisions.md) — a nicer picker is cosmetic follow-up, not a functional gap.

No new open question from the owner to resolve first — everything on the originally-scoped
autonomous list, plus the UI/standalone/persistence work the owner explicitly authorized, is now
done. The remaining items are either genuinely needing real hardware (#1) or smaller polish (#2-7)
— see the end of this file for what the autonomous session picks next.

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
| `engine`: voice allocator | **built.** `voice_allocator.rs::VoiceAllocator` — lowest-free-first, oldest-steal-when-full. Pitch/graph-agnostic (`note_id: u32` -> voice index); nothing routes real MIDI into it yet. `voice_count` is still a fixed `compile()` parameter. |
| `engine`: SIMD batching | **prototyped in spike S3 only** (`simd_voices.rs`), not integrated into the compiler; measured ~1.5-1.7x speedup (target was 2.5x, missed — see decisions.md). |
| `engine`: control-rate tier | **prototyped in spike S2 only** (`potato.rs`), not integrated into the compiler. |
| `engine`: swap + crossfade | **wired to the real compiler.** `patch_engine.rs::PatchEngine` generalizes S1's `swap.rs::Engine` mechanism (equal-power crossfade, `basedrop` deferred drop) to arbitrary-topology `CompiledPatch`es via `recompile()`. Proven in `tests/patch_engine_swap.rs` (bit-exact outside crossfade, no allocation). No control surface calls it yet — see open items. |
| `engine`: quality tiers (Live/Render) | **not built.** |
| `cables`: depth only | **not built.** `crates/cables` is an empty stub. |
| `modules`: 9 v1 built-ins + metadata | **9 of 9 have a `Module` impl.** `Module` trait, `ModuleInfo`, `ProcessIo`/`Signal` all built and tested. `osc.va` now has all 4 waveforms + hard sync (triangle naive, not BLEP/BLAMP-corrected). `lfo` has no sync (needs a clock, v3 scope) — see decisions.md. **Registry now exists** (`registry.rs`: `create(kind)`, `all_infos()`, `info_for(kind)`) — the compiler uses it to turn `ModuleState.kind` strings into instances. |
| `learn`: unlock flags filter catalog | **not built.** `crates/learn` is an empty stub. |
| `ui`: egui patchbay | **built, run, and screenshotted.** `crates/ui`, binary `kabl-ui`. Node-graph editor over `PatchEditor` (real op-log-backed undo/redo), live audio via the same `cpal`/`midir`/`PatchEngine` path as `standalone` (shared behind a flagged, non-textbook-RT-safe `Mutex` — see decisions.md). Verified actually rendering + handling a real click in this container via Xvfb; audio playback itself still unverified (no device here). |
| `standalone`: cpal + midir + JACK | **cpal + midir built** (`crates/standalone`, binary `kabl`). Default patch or `--patch <dir>`, live MIDI-in, RT-safe audio callback. Unverified on real hardware (none in this container) — falls back to a WAV self-test render instead. JACK not attempted (cpal's JACK backend needs a running jackd; deferred, not blocking — ALSA/default host covers the common case). `--save-default <dir>` bootstraps a save file. |
| **v1 accept test** (play a 4-voice MIDI chord, repatch live no click, undo, save, reload, replay construction log; potato gate passes) | **infrastructure complete, unverified end-to-end.** `kabl-ui` provides live repatch, undo/redo, and real save/reload now. Nothing here has been exercised on real audio/MIDI hardware or a real display by a person. |

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
                 voice_allocator.rs - VoiceAllocator: note_on/note_off(note_id: u32) -> voice
                                index within a fixed voice_count. Lowest-free-first, oldest-
                                steal-when-full. Pitch/graph-agnostic on purpose (see its own doc
                                comment); nothing feeds it a real MIDI stream yet. Tests in
                                tests/voice_allocator.rs (7 pure-logic + 1 driving a real
                                CompiledPatch's midi.in instances).
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
  standalone/  REAL, first version. Binary `kabl` (cargo run -p kabl-standalone).
                 lib.rs    - testable core: default_patch() (8-voice polysynth), RingBuffer
                                (non-allocating, bridges PatchEngine's BLOCK=64 output against
                                cpal's arbitrary callback size), resolve_midi_message (MIDI bytes
                                -> VoiceEvent, control thread, uses VoiceAllocator), apply_
                                voice_event (audio thread, no allocation). 13 tests in
                                tests/lib_logic.rs, all hardware-independent.
                 main.rs   - the actual cpal+midir wiring: opens default output device + first
                                MIDI port, runs PatchEngine live. `--patch <dir>` loads a saved
                                patch (kabl_core::load) instead of default_patch(); `--save-
                                default <dir>` writes default_patch() out and exits, a starting
                                point for kabl-ui. Falls back to rendering target/spike-renders/
                                standalone_selftest.wav if no usable audio device is found (this
                                container's case) instead of doing nothing observable. NOT
                                verified against real audio/MIDI hardware -- none exists in this
                                container. 4 tests in tests/persistence.rs (round-trip exactness,
                                bit-identical replay, real undo history preserved). See
                                decisions.md "Standalone binary" and "Persistence".
  ui/          REAL, first version. Binary `kabl-ui` (cargo run -p kabl-ui). Depends on
               kabl-standalone as a library (reuses default_patch/RingBuffer/
               resolve_midi_message/apply_voice_event -- no duplicated audio glue).
                 editor.rs - PatchEditor: patch-editing logic over kabl_core::PatchLog (every
                                edit is a real Op -- undo/redo is the log's, not reimplemented).
                                add_module/remove_module/connect/disconnect/move_module/
                                set_param/undo/redo/seed_from(existing PatchState)/from_log
                                (loads an existing PatchLog, preserving real history)/log()
                                (what core::save needs)/mark_dirty() (flags a recompile is due
                                after a wholesale state replacement, e.g. a Load). Hardware-
                                independent, fully unit-tested.
                 lib.rs    - show(): the egui widget tree. Node boxes positioned by ModuleState
                                .pos, click-a-port-then-click-a-port cabling, per-module param
                                sliders in a side panel, drag-to-move (commits one MoveModule op
                                on release, not per-frame), a "patch dir" text field with real
                                Save/Load buttons (kabl_core::save/load, not a native file
                                picker -- deliberate, see decisions.md). Snapshots editor state
                                before mutating mid-frame (immediate-mode borrow-checker reality).
                 main.rs   - eframe app: embeds the same cpal/midir/PatchEngine path standalone
                                uses, recompiling+hot-swapping on every edit. Known compromise:
                                audio callback and UI thread share one PatchEngine behind a
                                Mutex (try_lock on the audio side) -- flagged, not textbook RT-
                                safe, see decisions.md.
               20 tests in tests/editor_and_ui.rs: PatchEditor's full surface (including from_log/
               mark_dirty/save-load round-trip) plus two headless show() smoke tests
               (egui::Context::begin_pass/end_pass, no window/GPU needed). Also actually RUN in
               this container via Xvfb + software GL + real xdotool clicks (Add module, Save) --
               screenshots sent to owner, caught and fixed a real bug (default_patch()'s
               modules all shared pos (0,0), rendered stacked). See decisions.md "kabl-ui: the
               patchbay".
  cables/, pedals/, learn/, clap/
               STUBS. `//! Stub — not yet implemented.` One-line lib.rs each, empty Cargo.toml
               deps. Scaffolded so the workspace builds; no logic. Later milestones (v2+).
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

# Play it live, no UI (needs real audio output + optionally a MIDI controller; on Linux, cpal
# needs libasound2-dev installed to build at all -- see decisions.md "Standalone binary"):
cargo run -p kabl-standalone

# Play it live WITH the patchbay UI (same hardware needs as above, plus a display; on Linux also
# needs libxkbcommon-x11-0 -- see decisions.md "kabl-ui: the patchbay"):
cargo run -p kabl-ui
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
- ~~**No control surface drives `PatchEngine`**~~ — both `standalone` (`cargo run -p
  kabl-standalone`) and `kabl-ui` (`cargo run -p kabl-ui`) exist now. Real audio/MIDI I/O is
  unverified in this container (no hardware — see decisions.md "Standalone binary" and "`kabl-ui`:
  the patchbay"); `kabl-ui`'s rendering itself *is* verified (ran + screenshotted via Xvfb). No
  persistence in either binary (always starts from `default_patch()`); no `--midi <port>`
  selection (connects to the first port found).
- **`kabl-ui`'s audio/UI-thread `Mutex` sharing isn't textbook RT-safe** — a real, flagged
  compromise (audio callback `try_lock`s, silence on contention; control thread takes a real lock
  for the duration of `build_swap`). Fixing it properly needs `PatchEngine` to publish state over
  a lock-free channel instead of being shared directly. See decisions.md.
- ~~**No persistence wired into `standalone`/`kabl-ui`**~~ — fixed. `standalone --patch <dir>`/
  `--save-default <dir>`; `kabl-ui`'s toolbar Save/Load (text path field, real
  `kabl_core::save`/`load`). Verified: round-trip tests plus an actual Save click under Xvfb with
  the file confirmed on disk. See decisions.md "Persistence".
- **No canvas pan/scroll in `kabl-ui`** — a patch wider than the window is simply clipped
  (visible in the sent screenshot: `vca`/`out` cut off by the side panel).
- **No buffer-pool reuse in the compiler** — every port gets a fresh `[f32; BLOCK]`. Correct,
  wasteful; deferred as a pure optimization on the same schedule shape.
- **Compiler treats a cycle as a hard compile error**, not brief section 7.1's implicit 1-block
  delay. No v1 accept-test patch has a feedback loop, so not currently blocking; will matter for
  real feedback patches.
- ~~**No voice allocator**~~ — built, `crates/engine/src/voice_allocator.rs`. See decisions.md
  "Voice allocator". Now fed by real MIDI in both `standalone` and `kabl-ui` (via
  `resolve_midi_message`), unverified against real hardware in this container.
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
- **Extending `kabl-ui`** (a native file picker instead of the text-path field, cable-dragging
  instead of click-to-connect, canvas pan/zoom, fixing the `Mutex`-sharing compromise):
  `crates/ui/src/editor.rs`'s module doc + `lib.rs`'s module doc + decisions.md's "`kabl-ui`: the
  patchbay" and "Persistence" entries lay out what's built and why. `crates/ui/tests/
  editor_and_ui.rs` shows both the `PatchEditor` API (including `from_log`/`log`/`mark_dirty`)
  and the headless `show()` smoke-test pattern (`egui::Context::begin_pass`/`end_pass`, no window
  needed) for testing any new interaction logic without a display.
- **Extending `standalone`** (MIDI port selection, JACK): `crates/standalone/src/lib.rs`'s module
  doc + decisions.md's "Standalone binary" and "Persistence" entries lay out what's built vs.
  deferred. `tests/lib_logic.rs` and `tests/persistence.rs` show what's provable without hardware.
- **Just want to know if it works:** `cargo test --workspace` and the commands above. If they're
  not all green, the repo is mid-edit — check `git log` for the last commit's message.
