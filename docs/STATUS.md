# Status

**Read this first.** Unlike `decisions.md` (append-only rationale log) and `benchmarks.md`
(append-only numbers log), this file is overwritten to reflect the current state of the project.
If you're a human or an agent picking this up cold, this is where you find out what's real,
what's a stand-in, and what's next — before reading any code.

Last updated: 2026-09-22, after landing Eurorack view v1 (`kabl-ui`'s second, grid-snapped layout
mode) — see decisions.md's "Eurorack view v1" entry for the full design. This file's own history
above this line, up through the reconciliation note two paragraphs down, is a second session's
that worked this backlog in parallel; `git log` is the merged truth from here on.

**MIDI hardware verified live for the first time**: a real controller (KL Essential 61 mk3)
connected and played through `kabl-standalone`, confirmed by the owner. Needed a small fix first
(`--midi <substring>`, standalone/main.rs) — the binary was silently connecting to ALSA's own
virtual "Midi Through" loopback port instead of the real controller.

**Eurorack view v1 is built and screenshot/drag-verified** (not live-audio-verified — see the
real-hardware caveat in the handover note below): a `ViewMode::Eurorack` toggle in `kabl-ui`'s
toolbar, sharing the exact same `PatchEditor`/`show_canvas` as the existing free-form Patchbay
view, not a separate renderer. Modules snap to a grid (`ModuleInfo::width_units`, a new per-module
field, times a fixed cell size); dragging commits the snapped position, not a raw pixel one.
Canvas pan/scroll came back too (needed for a wide rack row). **Rough, not polished**: first-pass
`width_units` values, untuned grid color, the one skinned module (`osc.va`) doesn't yet respect
`width_units` for its own width (position snaps, width doesn't). See decisions.md for the full
list of what's deliberately left rough vs. what's real.

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

**Context handover note (2026-09-22, after commit `617da9d`)**: owner is clearing context on
purpose and starting a fresh agent from a tight resume prompt (same pattern as earlier in this
project — read this file cold, continue from it, no recap needed). **No `send_later` trigger is
currently active** — the previous one-shot trigger already fired (it's how the buffer-pool-reuse
work above happened) and was not rescheduled, since the owner is taking over the handoff
manually this time. If continued unattended autonomous work is wanted again, the new session
should set that up itself (see the pattern in git history around commits `7fe161d`..`617da9d`:
`send_later` ~20-30 min out, instruction to read this file's "Handover" section, pick the next
self-contained item, do it with full rigor, then reschedule) rather than assuming one is already
running.

**Context handover note (2026-09-22, after commit — check `git log -1` for the exact hash at
push time)**: owner asked to checkpoint and clear context after Eurorack view v1 landed. Read this
file cold, continue from it — no recap needed, same pattern as every prior handoff in this
project. Concretely, in priority order:

1. **Owner has real audio hardware, a real display, and a real MIDI controller on this machine**
   (confirmed working this session) — but was mid-way through live-testing the Eurorack view
   (drag-tested via a screenshot-only headless harness, never actually run with audio +
   interactively clicked by a person) when context was cleared. If the owner is at the machine,
   offer to run `cargo run -p kabl-ui` for real and get their eyes/hands on it — that's the
   natural next step, not more headless iteration.
2. **Eurorack view polish**, per decisions.md's "known rough edges": tune `width_units` per
   built-in against how they actually look at real size (current values are first-pass guesses);
   make the skinned module (`osc.va`) respect `width_units` for width, not just position; tune
   grid line color/spacing; no zoom yet.
3. ~~`PatchEngine` Mutex-window shrink~~ done (`2cf3d29`: `kabl-ui` compiles unlocked, locks
   only for `PatchEngine::finish_swap`). Also fixed (`2fd0aef`): `kabl-ui` had its own MIDI
   connect that picked ALSA's "Midi Through" loopback — now shares standalone's port selection.
   Still open: native file picker (`rfd`) for the Save/Load path field (idea reference on branch
   `backup-local-before-reconcile`, not a literal patch).
4. Real audio/MIDI hardware verification is still only partial: `kabl-standalone` confirmed
   playing through a real controller; `kabl-ui` has not been run with real audio at all yet (only
   the audio-free screenshot harness). See item 1.

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
(averaging not summing for voice->global, cycle-as-compile-error, params as compile-time
constants — buffer-pool reuse, the fourth, is now done, see below).

**Cycles now compile via brief section 7.1's implicit 1-block delay**, instead of a hard
`CompileError::Cycle`: `crates/engine/src/compile.rs` runs a DFS over the cable graph to find a
feedback-arc set (back edges relative to that DFS — provably enough to leave the rest of the
graph acyclic), excludes those edges from the topo-ordering graph, and wires each one's reader to
a *delay buffer* holding its source's previous-block output (refreshed by a new
`Step::CopyToDelay` at the end of the schedule) instead of the live one. `CompileError::Cycle` is
now a defensive, should-be-unreachable fallback rather than the normal outcome for a cyclic patch.
Delay buffers are pinned live-forever in `coalesce_buffers` (their value must survive to the
*next* block, not just to the end of this one); they now carry across `recompile()` too, keyed by
a `DelaySlotKey` (source port + lane) independent of physical buffer index rather than resetting
to silence — see "Cycle handling: carrying delay-buffer memory across recompile" below. Tested
end-to-end in
`crates/engine/tests/compile.rs`'s `cycle_compiles_with_implicit_one_block_delay`: an external
oscillator feeding a 2-`vca` cycle, asserted bit-exact against an independent reference generator
over 5 blocks. See decisions.md "Cycle handling: implicit 1-block delay".

**A cycle's delay-buffer memory now survives `recompile()`**: `CompiledPatch` gained
`delay_slots: HashMap<DelaySlotKey, BufIdx>`, keying each delay buffer by what it holds (source
port + voice lane) instead of where it lives. `recompile()` copies the old compile's buffer
content into the new compile's buffer for every key present in both — a live edit no longer
momentarily silences an in-flight feedback loop. Summed (voice-into-global) delay buffers are
deliberately not carried — they're recomputed every block from the per-lane buffers, which are
carried. New test in `compile.rs` (now 11): `recompile_carries_over_a_cycles_delay_buffer_memory`,
same two-track bit-exact-agreement pattern as the oscillator-phase carry-over test. See
decisions.md "Cycle handling: carrying delay-buffer memory across recompile".

**Buffer-pool reuse is done**: `coalesce_buffers` remaps buffers to a smaller set of physical
slots via greedy linear-scan register allocation over each buffer's compile-time-computed
`[first_def, last_use]` schedule interval — real reuse, not just fewer allocations. Measured on
the 5-stage chord chain: 37 -> 13 buffers at 4 voices, 145 -> 37 at 16 voices (~4x reduction at
scale, growing since naive cost scales with total port count while coalesced cost tracks what's
actually simultaneously live). `silence_buf`/`out_left`/`out_right` pinned live-forever, excluded
from reuse. 3 new tests in `compile.rs` (now 10). See decisions.md "Compiler: buffer-pool reuse".

**`process_block` is allocation-free**, proven the same way spike S1 proved it for the swap
mechanism: `crates/engine/tests/compile_rt_safety.rs` runs 200 blocks of the real chord patch
inside `assert_no_alloc!`. Got there by replacing 4 per-call `Vec`s with fixed-size stack arrays
sized to `MAX_INPUTS`/`MAX_OUTPUTS`/`MAX_PARAMS` (measured off every built-in's port/param
counts); `compile()` now rejects a module exceeding those bounds with `CompileError::TooManyPorts`
instead of the audio thread ever truncating or panicking.

**`PatchEngine` now handles overlapping swaps**, brief section 7.6: a second `build_swap`/
`receive_swap` arriving while a fade is still in flight used to overwrite `incoming` outright —
the first edit's graph never became steady-state active before being discarded, and the blended
output snapped to a different signal mid-crossfade (the exact click equal-power blending exists to
prevent). Fixed with a single `pending: Option<Owned<CompiledPatch>>` slot: `receive_swap` queues
instead of overwriting when a fade is already running; `process_block` promotes `pending` into
`incoming` (fresh `elapsed=0`) the instant the current fade finishes, so queued edits are never
dropped and never cause a discontinuity, just delayed by up to one crossfade's worth of latency
(15ms). A third+ overlapping arrival replaces `pending` (last-request-wins, no unbounded queue).
Known remaining gap: `build_swap` always recompiles against `active`, never a not-yet-promoted
`incoming`, so back-to-back edits can carry state up to one extra crossfade stale — documented,
not fixed (see decisions.md for why). `swap.rs`'s S1 spike engine has the same original gap and
was deliberately left alone (soon-superseded spike code). Tested in new
`crates/engine/tests/patch_engine_overlapping_swap.rs`: two references independently prove the
queued swap neither got dropped nor corrupted the first one, bit-exact in the steady windows
between and after both fades — verified to actually fail against the pre-fix code before being
left in place. See decisions.md "`PatchEngine`: overlapping swaps".

**`kabl-ui` got a real panel aesthetic + a working module-skin mechanism**, owner feedback ("too
simple and soulless"), not a backlog item. Two pieces: (1) every module's default rendering
(`crates/ui/src/lib.rs`) now has a category-colored accent strip, port-type-colored jack rings
with on-canvas name labels (there were none before), curved multi-colored cables instead of one
flat straight yellow line for every connection, and on-panel draggable knobs (270° sweep) in
addition to the existing side-panel sliders — no new dependency, pure rendering rewrite. (2) a new
`ModuleSkin` mechanism (`crates/modules/src/skin.rs`) lets a module declare custom background art
(raw embedded PNG bytes, kept out of `kabl_modules`' otherwise UI-framework-agnostic dependency
graph) plus an explicit normalized position for every jack/knob — the owner's actual ask ("custom
modules can use their own images as their background and specify where to put their
jacks/ins/outs/switches/readouts"). `osc.va` is the one built-in that uses it, with an honest
placeholder panel image (generated via ImageMagick, literally labeled "PLACEHOLDER PANEL ART" —
no image-generation tool was available to produce real designed artwork, and none was faked as
such); the other 8 built-ins still use the improved procedural panel from (1). `Switch`/`Readout`
control kinds are declared (matching the owner's wording) but not rendered — no built-in has a
discrete toggle or numeric readout yet, nothing real to wire them against. Verified beyond a code
read: ran under the existing Xvfb+`xdotool` setup, screenshotted the skinned panel rendering
correctly, and did a real interactive check — dragged the on-panel `waveform` knob and confirmed
the side panel's numeric value actually changed (which is also what caught the knob's initially-
backwards drag direction before shipping it). 2 new tests in `crates/modules/tests/skin.rs` (every
skin's controls match real ports/params and cover all of them; every embedded image decodes).
**No "rack" container/canvas-background concept was built** — the owner's other word — `kabl-ui`'s
canvas is still free 2D placement, not rack slots; a real rack metaphor is a bigger design
question flagged for the owner, not guessed at. See decisions.md "`kabl-ui`: real panel aesthetic
+ module skins".

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
3. No canvas pan/scroll in `kabl-ui` — a patch wider than the window is simply clipped.
4. `kabl-ui`'s Save/Load uses a plain text path field, not a native file picker (deliberate, see
   decisions.md) — a nicer picker is cosmetic follow-up, not a functional gap.
5. `PatchEngine.build_swap`'s state snapshot can be up to one extra crossfade stale for a second
   overlapping edit (see "Where we are" above) — a staleness bound, not a correctness bug; not
   fixed, real follow-up if it turns out to matter in practice.

No new open question from the owner to resolve first — everything on the originally-scoped
autonomous list, plus the UI/standalone/persistence/buffer-pool work, is now done. The remaining
items are either genuinely needing real hardware (#1) or smaller polish (#2-6) — see the end of
this file for what the autonomous session picks next.

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
| `engine`: flat-schedule compiler | **built, correctness-tested, RT-safe, buffer-pool reuse, cycles handled (incl. delay-buffer memory across recompile).** `compile.rs`: topo sort, voice/global instancing, `recompile()` w/ state carry-over. Cycles get brief 7.1's implicit 1-block delay (DFS feedback-arc set) instead of a hard error, and the delay buffer itself now survives `recompile()` (keyed by source port + lane, not physical index). `process_block` proven allocation-free (`tests/compile_rt_safety.rs`). `coalesce_buffers` reuses non-overlapping buffer slots (~4x fewer at scale, measured). |
| `engine`: voice allocator | **built.** `voice_allocator.rs::VoiceAllocator` — lowest-free-first, oldest-steal-when-full. Pitch/graph-agnostic (`note_id: u32` -> voice index); nothing routes real MIDI into it yet. `voice_count` is still a fixed `compile()` parameter. |
| `engine`: SIMD batching | **prototyped in spike S3 only** (`simd_voices.rs`), not integrated into the compiler; measured ~1.5-1.7x speedup (target was 2.5x, missed — see decisions.md). |
| `engine`: control-rate tier | **prototyped in spike S2 only** (`potato.rs`), not integrated into the compiler. |
| `engine`: swap + crossfade | **wired to the real compiler, overlapping swaps handled.** `patch_engine.rs::PatchEngine` generalizes S1's `swap.rs::Engine` mechanism (equal-power crossfade, `basedrop` deferred drop) to arbitrary-topology `CompiledPatch`es via `recompile()`. Proven in `tests/patch_engine_swap.rs` (bit-exact outside crossfade, no allocation) and `tests/patch_engine_overlapping_swap.rs` (a swap queued mid-crossfade is never dropped or clicked). Driven live by both `standalone` and `kabl-ui`. |
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
                 skin.rs      - ModuleSkin/ControlSkin/ControlKind (owner ask, not a brief
                                feature): optional custom panel art (embedded PNG bytes) + explicit
                                normalized jack/knob positions, on ModuleInfo.skin. None for 8 of 9
                                built-ins; osc.va carries a real (placeholder-art) demo. Only
                                Jack/Knob are rendered by kabl-ui; Switch/Readout are declared, not
                                wired to anything yet. Tested in tests/skin.rs (2 tests: every
                                skin's controls match and cover its module's real ports/params;
                                every embedded image decodes). See decisions.md "kabl-ui: real
                                panel aesthetic + module skins".
                 assets/osc_va_panel.png - the one skin demo's background image, an honest
                                generated placeholder (ImageMagick), not designed art.
  engine/      Real compiler + spike code (depends on kabl-modules, kabl-core).
                 compile.rs - THE COMPILER. compile(&PatchState, sample_rate, voice_count) ->
                                CompiledPatch: topo sort (Kahn's algorithm), voice/global-rate
                                instancing, voice->global averaging (SumVoices steps), recompile()
                                with save_state/load_state carry-over via HashMapState,
                                coalesce_buffers (greedy linear-scan register allocation over
                                buffer live ranges -- ~4x fewer buffers measured at scale). Cycles
                                get brief 7.1's implicit 1-block delay: a DFS over the cable graph
                                finds a feedback-arc set (back edges), excluded from the topo-
                                ordering graph so Kahn's always completes; each back-edge cable
                                reads a pinned delay buffer (last block's value, via a
                                Step::CopyToDelay at schedule's end) instead of the live one --
                                CompileError::Cycle is now a should-be-unreachable safety net, not
                                the normal outcome. A delay buffer's value is keyed by source
                                port + lane (delay_slots) so it survives recompile() too, not
                                reset to silence. Correctness-tested end-to-end in tests/
                                compile.rs (11 tests) against patch_demo.rs's numbers, an
                                independent reference generator for the cycle case, and a
                                recompile-carry-over test for delay-buffer memory.
                                process_block is allocation-free (fixed-size stack scratch sized
                                to MAX_INPUTS/MAX_OUTPUTS/MAX_PARAMS; compile() rejects a module
                                exceeding them) -- proven in tests/compile_rt_safety.rs via
                                assert_no_alloc, same pattern as spike S1. Wired to swap.rs's
                                mechanism via patch_engine.rs (below) and live in both
                                standalone/ and ui/. See decisions.md "Flat-schedule compiler
                                v1", "process_block made allocation-free", "Compiler:
                                buffer-pool reuse", and "Cycle handling: implicit 1-block delay".
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
                 lib.rs    - show(): the egui widget tree. Node panels positioned by ModuleState
                                .pos, category-colored accent strip, port-type-colored jack rings
                                with on-canvas name labels, curved multi-colored cables (a
                                quadratic-bezier droop, not a straight line), on-panel draggable
                                knobs per param (plus the existing side-panel sliders for precise
                                entry), click-a-port-then-click-a-port cabling, drag-to-move
                                (commits one MoveModule op on release, not per-frame), a "patch
                                dir" text field with real Save/Load buttons (kabl_core::save/load,
                                not a native file picker -- deliberate, see decisions.md). A module
                                with ModuleInfo.skin set (currently just osc.va) renders via
                                draw_skinned_module instead: its own panel_size, its background
                                image (decoded once, cached as an egui::TextureHandle in
                                UiState.image_cache), every control at its exact declared
                                position. Snapshots editor state before mutating mid-frame
                                (immediate-mode borrow-checker reality). See decisions.md "kabl-ui:
                                real panel aesthetic + module skins".
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

- **No "rack" container/canvas concept** — owner mentioned "racks where you place what you need"
  alongside the module-skin ask; only the skin mechanism (custom art + control placement per
  module) was built, not a rack-slot canvas layout. Open question for the owner: fixed HP-width
  slots like real Eurorack, or free placement with a rack-styled canvas background? Not guessed
  at. See decisions.md "kabl-ui: real panel aesthetic + module skins".
- **Only `osc.va` has a real skin** — proves the mechanism, not a finished aesthetic pass. The
  other 8 built-ins use the improved procedural panel (category accent, colored jacks, on-panel
  knobs) but no custom art. Real hardware-quality panel art for more/all modules is real design
  work this container has no tool to produce (ImageMagick placeholder only) — needs either a
  human designer or an image-generation tool this session doesn't have.
- **`Switch`/`Readout` skin control kinds are declared, not rendered** — no built-in has a
  discrete toggle or live numeric readout to bind one to yet; add the `kabl-ui` rendering when a
  real module needs one.
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
- ~~**No buffer-pool reuse in the compiler**~~ — fixed. `coalesce_buffers` (greedy linear-scan
  register allocation over compile-time buffer live ranges); ~4x fewer buffers measured at 16
  voices on the chord patch. See decisions.md "Compiler: buffer-pool reuse".
- ~~**Compiler treats a cycle as a hard compile error**~~ — fixed. Brief section 7.1's implicit
  1-block delay: DFS feedback-arc set, delayed cables read a pinned delay buffer instead of the
  live one. See decisions.md "Cycle handling: implicit 1-block delay".
- ~~**A cyclic patch's delay-buffer memory resets to silence on `recompile()`**~~ — fixed.
  `CompiledPatch::delay_slots` keys each delay buffer by source port + lane instead of physical
  index; `recompile()` copies old values forward by key. See decisions.md "Cycle handling:
  carrying delay-buffer memory across recompile".
- ~~**No voice allocator**~~ — built, `crates/engine/src/voice_allocator.rs`. See decisions.md
  "Voice allocator". Now fed by real MIDI in both `standalone` and `kabl-ui` (via
  `resolve_midi_message`), unverified against real hardware in this container.
- ~~**Overlapping swaps unhandled**~~ — fixed in `PatchEngine`: a `pending` slot queues a swap
  that arrives mid-crossfade instead of overwriting `incoming` (which used to both drop the
  in-flight edit and click). See decisions.md "`PatchEngine`: overlapping swaps". Remaining gap:
  `build_swap` snapshots state from `active`, never a not-yet-promoted `incoming`, so a queued
  edit's state can be up to one extra crossfade stale — documented, not fixed. `swap::Engine`
  (S1 spike) still has the original gap, left alone deliberately (soon-superseded spike code).
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
- **Extending the compiler** (quality tiers, cable-depth/param modulation, v2+ features):
  `crates/engine/src/compile.rs`'s module doc comment + decisions.md's "Flat-schedule compiler
  v1", "`process_block` made allocation-free", "Compiler: buffer-pool reuse", "Cycle handling:
  implicit 1-block delay", and "Cycle handling: carrying delay-buffer memory across recompile"
  entries lay out what's built, what's deferred, and why. `crates/engine/
  tests/compile.rs` shows the expected external shape (build a `PatchState` from ops, `compile()`,
  drive it, `recompile()`); `tests/compile_rt_safety.rs` shows the `assert_no_alloc` pattern to
  keep any future change RT-safe.
- **Extending `PatchEngine`** (the `build_swap`-snapshot staleness on a queued overlapping swap):
  `crates/engine/src/patch_engine.rs`'s module doc + decisions.md's "`PatchEngine`: wiring the
  compiler into S1's swap mechanism" and "`PatchEngine`: overlapping swaps" entries. `tests/
  patch_engine_swap.rs` and `tests/patch_engine_overlapping_swap.rs` show the expected external
  shape and the independent-reference-comparison pattern used to test crossfade correctness
  without needing to replicate `swap::equal_power` (`pub(crate)`, not visible to integration
  tests).
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
