# D04 design: runtime controls without recompilation

Batch D04 (with the D03-R2 closeout). Brief: [D04](../product-research/briefs/D04.md).
Code: `crates/engine/src/runtime.rs`, `compile.rs` (slots, ramps), `patch_engine.rs`
(`set`, `drain`), `crates/ui/src/control.rs`, `crates/ui/src/main.rs` (`Core`, `pump`).
The full parameter inventory is generated: [classification.md](classification.md).

## 1. What changed

Before D04 every audio-affecting edit, a knob drag frame or a CC message included, compiled a
whole new graph on the UI thread and crossfaded to it over 15 ms. Mapped CCs and MIDI
buttons were applied inside the editor frame (`show` called `perform::apply_cc`), so nothing
happened while no frame was drawn.

Now:

```
MIDI thread ──CC──▶ cc queue ──▶ control thread (pump) ─┐
UI frame (mouse, undo, restore, recipes, load) ─────────┤  both under the Core mutex
                                                        ▼
                PatchEditor (the document) ──diff──▶ Delivery ──ToAudio FIFO──▶ audio callback
                                                     (values or a graph)        PatchEngine::drain
```

- The document (`PatchEditor` and its op log) stays the only authority. `Delivery` keeps the
  state the audio side converges to (`sent`) and diffs the document against it with
  `runtime::runtime_changes`. Only runtime params or route amounts changed: those values go
  as `ParamSet`s. Anything else: one compile, as before.
- `Core { editor, ui, delivery }` sits behind one `std::sync::Mutex`. The UI takes it for a
  frame; the control thread (`pump`, `main.rs`) takes it to apply CCs and to retry what waits.
  The audio thread never takes it.

## 2. Classification

Source-grounded: every built-in reads its params in `process` through `ProcessIo::param`,
once per block (`compile.rs` writes them into each step's `params` before the block). A
module's `prepare` receives no params. So a param is runtime unless something outside
`process` reads it. Exactly one module has such params:

- **`midi.in` `mode`, `priority`, `glide` — structural.** `MidiIn::configure` sets them at
  compile, and the engine's `Keyboard` (outside every graph) reads them from the graph playing
  in `PatchEngine::sync`, which runs when a graph is installed: voice assignment and the release
  a mode change makes. Changing them in place would leave the keyboard on the old settings
  until the next key event. Kept on the compile path, with their existing semantics.
- `midi.in` `glide_ms` is runtime: `process` reads it every block.
- `seq` `bank` (startup bank) is runtime in the classification's sense but acts only once:
  the sequencer reads it on its first block. A recompile never re-applied it either
  (`load_state` marks the carried sequencer as started), so the runtime value and the old
  rebuild agree: it takes effect at the next load.
- No param sizes or precomputes state: delay lines, reverb tanks and chorus lines are sized
  from the sample rate in `prepare`; unison uses fixed `MAX_UNISON` arrays; sequencer banks are
  fixed arrays.

Defaults and aliases: `runtime_changes` compares the value the compiler uses (stored under the
param's name, else its legacy alias through `registry::legacy_param`, else the default), so
removing a stored value (back to the default) is a runtime value and a legacy-named stored
value is honoured. Values compare bitwise: an edit that writes the same value sends nothing.

Cables: a route's `amount` is runtime (the step's route scale). `bypass` is structural: a
bypassed route is left out of the compiled graph (`route_bypassed`), which changes scheduling
and cycle breaking. Creating or deleting any cable, adding, removing or changing the kind of a
module: structural. A bypassed route's amount changes nothing and sends nothing; the next
compile reads it. Jack cables have no params the compiler reads.

Presentation (`face.*`, `pin.*`, `cc.*`, `btn.*`, `launch.*`, `cue*`, labels, positions) never
reached the compiler and now also never reaches the diff: `runtime_changes` walks
`ModuleInfo::params` only.

Stepped, discrete controls keep their semantics: they are set at once and rounded by the
module as before. Nothing about modulation changes: stored base versus routes, taper space,
signed amount, one clamp, stepped rounding (`ParamInfo::from_norm`) and the block-rate grid
are the compiler's code, applied to the new base (`write_param` sets `params[i]` and, for a
modulated param, `ParamMod::base_norm = to_norm(value)`, exactly what `compile` computes).

## 3. Identity and revisions

- **Target.** `RuntimeTarget::Param { id, kind, index }` (module id, its kind as a `&'static
  str` from the registry, the param's index in `ModuleInfo::params`) or `Route { cable }`.
  Not a panel order, voice index or buffer slot. Inside a graph the target resolves to
  compile-time slots (`param_slots` sorted by (id, index, step); `route_slots` by cable) by
  binary search; a kind mismatch (an id reused for another kind) resolves to nothing.
- **Revision.** `Delivery` hands out one increasing revision per value and per graph
  (`ParamSet::rev`, `CompiledPatch::rev`); the startup graph is revision 1. Revisions keep
  increasing across documents, so they also carry the document lifetime: a new document is a
  fresh graph with a higher revision than every value of the old one.
- **Rule.** A graph applies a value only when `value.rev > graph.rev`. A graph compiled from
  the document already contains every earlier value, so a delayed value cannot overwrite it,
  and a value from before a delete/recreate, an undo or a new document cannot land in the new
  graph.
- **Graph order.** `PatchEngine::receive_swap` refuses a graph whose revision is below the
  newest one received (it would undo newer edits). With one producer and one FIFO this does not
  happen in production; it guards the rule if a build ever completes out of order
  (`values_and_graphs_keep_revision_order`).

## 4. Active, fading and pending graphs

`PatchEngine::set` offers a value to every graph that exists: `active`, `incoming` (fading in)
and `pending` (queued behind the fade). Each applies it by the rule above: ramped in a graph
that has rendered a block, set in one that has not (`CompiledPatch::started`).

Shared-target policy during a fade: both the outgoing and the incoming graph take a newer
value, so the crossfade blends two graphs moving the same way; a graph that already has a
value (its revision is newer) keeps it. A pending graph compiled before the value takes it
before it plays, so its installation cannot overwrite a later edit. When a fade ends, the
outgoing graph is dropped (to the collector) and never receives anything again: there is no
path back to it.

## 5. Queue, bounds and backpressure

One `rtrb` SPSC queue `ToAudio` (capacity `control::QUEUE` = 256) carries graphs and values in
revision order. Commands (launch, preview, inspect) and transport keep their own queues: their
order relative to values does not matter (they act on sequencers, clocks, keys and the tap,
not on params).

What cannot be sent now waits in `Delivery`:

- at most one graph (`held_graph`); a newer compile replaces it (the older is freed on the
  control thread);
- at most one value per target (`held`, the newest; the replaced one is counted as coalesced),
  kept behind the held graph. A compile clears them: the graph carries them.
- Past `MAX_HELD` (1024) targets, a compile replaces the values (counted as a fallback).

Absolute values of one target may coalesce because the last one is what the document holds;
values are sent sorted by revision. Button actions are not values and never coalesce; undo,
redo, restores and loads are document edits whose resulting values (or graph) are what is
sent.

Outstanding ownership: at most `MAX_GRAPHS_QUEUED` (2) graphs in the queue (`Delivery`
counts graphs sent against `Feedback::graphs_taken`), one held, plus `active`, `incoming` and
`pending` on the audio side. Values own nothing.

Retry: `Delivery::flush` runs after every UI frame, after every CC batch and every 5 ms on the
control thread while something waits (`pump`), so the final value reaches the audio thread when
it resumes, with no editor frame. `a_paused_audio_thread_bounds_the_queue_and_gets_the_last_value`
fills the queue with 2000 CC values and 10 compiles while the consumer is paused, then checks
the bounds and that the engine ends on the document's values.

Per-callback work: `PatchEngine::drain` takes at most `control::QUEUE` messages per callback
(so a producer that keeps pushing cannot keep the callback draining); each value is two binary
searches and a write per voice lane; ramps are at most `MAX_RAMPS` (32) targets per graph per
block.

## 6. Requested, applied, failed

- `Delivery::pending()` counts what was requested and not yet taken by the audio thread
  (waiting here or still in the queue). The status bar shows "N change(s) waiting for audio"
  while it is non-zero with audio running.
- `Feedback` (relaxed atomics written by `drain`): graphs taken, stale graphs refused, values
  taken, values that resolved in no running graph (`unresolved`), the newest applied revision.
  `KABL_STATS_FILE` prints them with `Delivery::counts` (graphs compiled, failed, fallbacks,
  values, coalesced, actions not delivered).
- A failed compile keeps the last valid graph playing (nothing is sent), sets
  `Delivery::compile_error`, puts "recompile failed: …" in the status bar and hands the error to
  the inspector (`Inspect::rebuilt`). Runtime values for modules the playing graph has still
  apply; values for modules only in the failed document resolve nowhere and are counted.
  A later successful compile clears the error.
- A command the queue refuses (the audio thread stopped taking them) is reported in the
  status line and the log and counted, never treated as done. It is not retried: a launch or a
  Restart delivered late would land on a different beat.

## 7. Control and MIDI

`control::midi` = `perform::apply_cc` + `control::deliver`, called by the control thread for
each CC batch. The MIDI callback only pushes the CC and wakes the thread
(`Thread::unpark`); it never takes the core lock, so the MIDI sink lock and the core lock
never nest. Unchanged semantics (`perform.rs`): channel-specific mappings, soft takeover within
1.5/127 or by crossing, pickup reset when something else changes the value
(`sync_takeover`), one undo group per continuous CC gesture (gaps under 1 s, across
mappings), one action per low-to-high press, reserved CCs 64/120/123 as key events, learning
through `UiState::learn` (set by the UI; the first CC after it is taken by the learn).

Two details moved with the code:

- **Run/Stop button.** It used to choose Run or Stop from `UiState::clock_running`, a report
  only refreshed when a frame is drawn. It now sends `Transport::Toggle`, which
  `PatchEngine::transport` resolves from the clock in the graph playing, before the Stop-to-
  selection rule for pending launches.
- **Reconnect guard.** `perform::rearm` starts the 0.5 s button guard when the control
  thread sees `button_rearm` (within 5 ms of a connect), no longer when a frame happened to
  apply the next message.

## 8. Document, history and save

- Every source edits the document first (`PatchEditor`), under the lock: mouse drags and text
  entry in a frame, CCs on the control thread (`set_param_cc`, the existing gesture groups),
  undo/redo, recipe steps, comparison restore (`restore_to`, one undo step), Load
  (`replace_patch`). The delivery follows the document, so all of them converge on the same
  values; there is no second patch.
- Arbitration: edits are serialized by the lock; the last edit to the document wins, and the
  audio side follows the document in revision order. A mouse drag and a CC on the same knob
  interleave exactly as before (the CC's pickup drops when the mouse changes the value).
- Save writes the document, which already holds every accepted CC edit: nothing waits in a UI
  queue. A frame drawn after a pause replays nothing (the CC queue is consumed by the control
  thread; `midi_buttons_and_pickup_work_without_a_frame_and_the_ui_catches_up`).
- Mapping changes (`cc.*`, `btn.*`) are presentation edits: undoable, saved, no audio effect.

## 9. D02 and D03

- D02 explanations read the stored base and routes from the document: unchanged.
- D03 inspection measures the graph playing. Runtime values do not create a graph, so the
  measurement window continues through a knob turn (before, a turn rebuilt every frame and the
  window was carried across each swap). A structural edit still compiles a new generation;
  `main.rs` passes it to `Inspect::rebuilt`, which drops readings from another topology.
- Voices, note ownership, envelope phases, sequencer positions, seeds, delay and reverb tails
  and probe windows are untouched by a runtime value: nothing is compiled, carried or reset.

## 10. Smoothing

Existing DSP smoothing stays and is not duplicated. A runtime value in a playing graph either:

- **ramps** over `RAMP_MS` = 15 ms (the old crossfade's length; 11 blocks of 64 at 48 kHz) in
  knob travel (`to_norm`, so an exponential param ramps in its own taper; a route in its scale),
  block by block, ending on the exact value the compiler would have used; or
- **is set at once**: stepped choices (never through invalid values), sequencer step data
  (rounded when a step plays; a ramp could play a note between two values) and params the
  module smooths itself (`runtime::smoothed_by_module`: gain, macro, drive, ladder filter,
  reverb; chorus depth/mix/width; delay time/feedback/mix; noise level; oscillator pulse width).

Which params ramp is listed in [classification.md](classification.md) (20 of 95). Gates and
triggers are signals, not params, and are untouched. A graph that has not rendered a block
(a pending graph, a Load) takes values exactly: an initial load starts at the stored value,
never from zero. A new value for a target already ramping starts a new ramp from where it is.
Past `MAX_RAMPS` concurrent ramps in one graph a value is set at once (a `ponytail:` note marks
the ceiling).

Audible difference from before, by design of D04: an edit used to reach the sound through a
15 ms crossfade between two graphs; it now reaches it through the module's own smoothing or
the 15 ms ramp. The duration is the same; a crossfade blended two filter or envelope states,
the ramp moves one. This is the requested behaviour change (routine controls must not
recompile), not a hidden one; listening is for Kosta.

Compatibility: a sustained fixed setting renders as before because nothing changed when no
value moves (`compile`, the module code and the schedule are untouched; the ramp code runs only
while a ramp exists). A value applied before a graph plays produces a graph bit-identical to a
compile of that value (`a_set_value_renders_like_a_compile_of_it`). Baseline-versus-final
renders: README "Compatibility".

## 11. Real-time rules

Audio thread, changed code only: `drain` (pop, match, `receive_swap`, `set`), `set_runtime`
(binary search, writes to preallocated steps), `run_ramps` (a fixed array), `transport`
(Toggle). No allocation, deallocation, logging, formatting, I/O or locks: a refused or replaced
graph is dropped as `basedrop::Owned`, which queues it for the collector (the UI thread frees
it); values are `Copy`. Checked with `assert_no_alloc` around the production calls, including a
saturated queue and swaps (`draining_is_bounded_ordered_and_allocation_free`; every callback of
`crates/ui/tests/runtime_controls.rs`). The narrowly accepted D03-R1 exception (freeing a
backend error message when the 16-slot error queue is full) is untouched; the first-callback RT
priority request remains the older documented exception.

Off the audio thread: the core mutex is taken by the UI (a frame's layout) and the control
thread. A CC therefore waits at most for one frame's layout; with no frame it is applied at
once. Compiles happen on whichever of the two threads delivers a structural change (in practice
the UI; the control thread compiles only for the `MAX_HELD` fallback).

## 12. Limits and rejected alternatives

- Rejected: a second, audio-only patch model fed by commands (two sources of truth; save and
  undo would have to reconcile them). Rejected: an audio-side table of recent values re-applied
  to late graphs (unneeded with one FIFO and revisions; more audio-thread state).
- Rejected: separate queues for graphs and values (their relative order is what makes a
  queued graph safe).
- Rejected: CC processing in the MIDI callback (it would take the core lock while holding the
  sink lock that the UI also takes: a lock-order inversion).
- Kept: `SwapSender`/`drain_swaps` for the existing live-edit tests and the offline example.
- `MAX_RAMPS` is 32 per graph: a 33rd simultaneous target steps.
- Commands refused by a full queue are reported, not retried.
- A frame's layout holds the lock, so CC latency with the editor drawing includes up to one
  layout pass (measured in README "Performance").
