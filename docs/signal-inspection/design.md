# D03 design: tap, identity, comparison, recipes, logging

Written before implementation (first increment of the D03 brief). The later sections of
README.md record what was built and any deviation from this note.

## 1. The tap

**What is measured.** One output jack of one module: `(module id, output port)`. That is the
only primary selection surface. A cable can navigate to its source; the UI then says
"source output", because the destination may read it one block late (feedback), through a
voice average (voice → global) or as a scaled modulation route. Inputs and effective
parameter values are not measured.

**Where it is captured.** Inside `CompiledPatch::process_block_with`, right after the
selected module instance's `Step::Process` has written its outputs, from that instance's own
output buffer. That is the only point where the physical buffer is known to hold this
signal: `coalesce_buffers` may give the slot to another buffer at any later step, and the
pool after the block is not a map of signals. A feedback cable reads a separate delay copy
(`Step::CopyToDelay`); the tap reads the live output of this block, so the DSP, schedule and
buffer lifetimes are unchanged. The tap only reads.

**Voices and channels.** A port is one mono channel; stereo modules have separate ports
(`left`/`right`, `lp`/`bp`/`hp` are separate signals). A module compiled per voice has one
instance per lane (8 by default): the tap keeps separate statistics per lane (at most
`PROBE_LANES`), and reports the lane count. The UI shows lanes individually and aggregates
without averaging activity away: level = maximum over lanes; gate edges = sum over lanes;
CV/pitch = range over lanes plus each lane's last value. A voice lane is an engine voice
slot, not "the chord". A module compiled once (global, or a voice-rate chain driven only by
global modules) has one lane, labelled "one shared instance". The voice average that a
voice → global cable applies is not the tap's signal and is not shown as such.

**Statistics per lane and window** (fixed-size, `Copy`): min, max, peak |x|, sum of squares,
rising edges through 0.5 (the engine's gate threshold, `> 0.5`), samples above 0.5, last
value, non-finite samples. The window is a whole number of 64-sample blocks, about 50 ms
(38 blocks at 48 kHz). Short gate pulses inside a block are counted per sample, so a
one-sample pulse is an edge.

**Lifecycle and bounds.**

- The UI sends `Command::Inspect(Some(ProbeTarget))` or `Command::Inspect(None)` through the
  existing bounded command queue. `ProbeTarget` is `Copy`: selection token, module id,
  output port index, module kind (`&'static str`).
- `PatchEngine` keeps the target. It resolves it in the graph it measures by a linear scan
  of that graph's instances (the same bounded pass `carry_state` already makes at the
  same moment), when the target changes, when a fade starts and when a pending graph is
  promoted. Resolution checks id, kind and port index; a mismatch reports "not in this
  graph". No allocation: the resolved lanes are a fixed array.
- Which graph: the one fading in during a fade, else the active one. Only that graph
  collects; the other has collection off. A window that spans a fade start continues in the
  new graph only when the swap changed values, not wiring (same modules, kinds and cable
  endpoints: `CompiledPatch::topology`), and the target resolves to the same lanes and
  port there, so a knob turned continuously (a swap per frame) still yields reports,
  flagged `fading`. After a wiring change the window starts over, matching the UI's floor.
  *(Changed after review F3 and recheck N1; the first version always restarted.)*
- Closed inspection: target `None`, each schedule step pays one boolean test.
- When a window completes, the engine holds one `ProbeReport` (fixed size, `Copy`). The
  audio callback pushes it into a dedicated `rtrb` queue of 8; a full queue drops the
  report. Reports carry a sequence number, so the UI can count drops.

**Identity and staleness (UI side).**

- A selection token changes on every selection change, close and document replacement.
  A report with another token is ignored.
- Every rebuild attempt gets a new graph generation (`CompiledPatch::generation`, set by
  `main.rs`). A report names the generation it measured.
- Reports older than a floor are ignored, and reports already kept from before it are
  dropped (review F1). The floor rises to the current generation on: a failed compile (the
  measured graph is then not the patch on screen; the UI says so), and a topology change
  (modules or cables added/removed).
- Reports carry the engine's rendered-sample count; the UI ages them against the frames
  delivered to the device, so a report that waited in the queue is not "live" (review F4). Parameter-only edits keep the floor,
  so the meter keeps moving while a knob turns; the panel labels such a report as
  "measured on the previous version" until the new graph reports.
- Accepted generations are monotonic (overlapping swaps cannot move it backwards).
- A deleted target, an undo that removes it, or another document: shown as not in the patch.
  Reused ids in another patch cannot inherit a report: the token changes on replacement.
- Waiting (no report yet), unavailable (no audio engine, not compiled), stale (no report for
  0.5 s: the callback stopped or the queue is starved) and measured-zero are distinct
  states. Stale values are greyed with their age, never presented as live.
- The UI re-sends its current target when it has waited for 0.5 s (a full command queue
  cannot strand the selection).

Selection, opening and closing inspection never touch the editor, the undo log, the saved
state, the transport or the voice allocation.

## 2. "Why no sound?"

Pure function of the current `PatchState`, the compile result (for the output the engine
actually plays: the last `out` module in schedule order), runtime transport status already
reported by the audio thread, the final output meter and the selected tap's recent accepted
reports. It returns three separate lists: graph facts, measurements (with tap, threshold and
interval) and possibilities (where to look next). No speaker, device, MIDI hardware or intent
claim; a bounded path search that hits its limit says unknown. It runs on demand when the
aid is open, never in the background, and never edits.

## 3. Comparison reference

Interaction: **capture / restore as one undo step**, the explicit restore/undo form the brief
allows. No audition toggle and no hidden second document.

- *Capture reference* copies the complete `PatchState` (modules, params including face,
  pins and MIDI mappings stored as params, cables with route settings and step patterns,
  labels, positions). It does not touch the log. Session-local; said in the panel.
- *Restore reference…* asks first, naming the scope, then appends one `Op::Group` that
  turns the current state into the reference (a computed difference, checked to reproduce
  the reference exactly). Undo reverts it in one step and brings every working edit back;
  redo re-applies it: that pair is the A/B. The earlier history is untouched; like any edit,
  a restore replaces the redo tail (the dialog says so when there is one).
- What is heard, edited and saved is always the one current state, so Save and Save As
  target it unambiguously, and the ordinary unsaved-changes protection applies. MIDI CC and
  buttons edit the current state as always; notes, sustain and transport are runtime and
  unaffected. The graph after a restore is an ordinary edit swap (state carried,
  crossfaded), not a fresh load, so held notes and tails continue.
- Document replacement (a different editor instance) drops the reference with a message.
  Failed load/save and cancelled dialogs do not replace the editor, so the reference stays.
- No normalization or gain change. Loudness differences are shown by the existing output
  meter only.

## 4. Recipes

Three recipes compiled into the binary (`recipes.rs`), each tied to one factory teaching
patch in `patches/recipes/*` (generated by a test, shipped by the package like every factory
sound). State: `{recipe, step, editor instance}`, view only.

- Start = `browser::request(Pending::Open(recipe sound))`: the ordinary unsaved question.
  The recipe becomes active only when that open completed (a new editor instance with that
  library origin). Cancel leaves both the song and recipe untouched.
- On start, a comparison reference of the starting patch is captured (reversible, stated).
- Steps name explicit targets `(module id, kind, param)`; a target that is missing or has
  another kind in the open patch is shown as unavailable, never guessed by name. Show/Back
  use D02's `explain::go`/`back`. An optional "Inspect" selects a tap.
- Another document replacement detaches the recipe (it says so; it does not follow the new
  patch). Close keeps the current patch and edits. Restarting offers the ordinary open.

## 5. Logging

`log` facade (0.4, MIT/Apache-2.0, already in the dependency tree) and one small backend in
`kabl-standalone::applog`, initialized once in each binary's `main`. Why not a stock
backend: the brief needs bounded buffering with drop counts, size rotation with a fixed cap
and a non-blocking sink-failure notice; `env_logger` writes synchronously to stderr,
`tracing-appender` rotates by time only and `flexi_logger`'s async mode neither bounds nor
counts drops. The backend is a `log::Log` impl + one writer thread.

- Level: INFO default; `KABL_LOG=trace|debug|info|warn|error|off` or `--log-level` in the
  same release binary (no compile-time level features).
- Location: `$KABL_LOG_DIR`, else `$KABL_USER_DIR/logs`, else
  `$XDG_STATE_HOME/kabl/logs`, else `~/.local/state/kabl/logs`; `kabl.log` rotated at
  1 MiB to `kabl.1.log`…`kabl.3.log` (4 MiB cap). The path is printed at start and shown in
  the status bar tooltip / Help menu.
- Records go through a bounded queue (1024) to the writer thread: a full queue drops and
  counts; a write failure is counted, retried on the next rotation check, never panics or
  blocks. The UI shows a rate-limited notice when records were dropped or the sink failed.
  Shutdown flush waits at most 500 ms.
- Lines: UTC time, seconds since start, level, target (subsystem), session id, message with
  `key=value` fields. The startup line adds version, commit (build script; "unavailable"
  outside git) and the log path.
- The audio callback and the stream error callback never call the logger. They update
  atomics (existing `CallbackTiming`) and a bounded error queue; the UI thread reports
  changes at most every 10 s (INFO/WARN with counts), DEBUG summaries every 60 s.

### Stream-error ownership (D03-R1)

**Where the error callback runs.** Source is cpal 0.18.2 (`Cargo.lock`), ALSA, the only
Linux backend built.

- `Stream::new_output`/`new_input` start one worker thread per stream (`cpal_alsa_out` /
  `cpal_alsa_in`). The closure passed as the error callback is moved into that thread and
  only called there, by `output_stream_worker` / `input_stream_worker`
  (`src/host/alsa/mod.rs`, around lines 909–1021). This is the same thread that calls the
  data callback, between periods. So the error callback runs on the audio thread.
- `stream.play()` and `pause()` return their errors to the caller; they don't go through
  the callback.

**Which errors own heap memory.** cpal builds each error on that worker thread immediately
before calling us. `cpal::Error` is `{ kind, message: Option<Cow<'static, str>> }`.

- **Owned (heap) messages:**
  - `From<alsa::Error>` for an errno that has no specific kind becomes
    `BackendError(err.to_string())` (`mod.rs` around line 1722). This can happen on any
    failed poll/avail/prepare/start.
  - `From<AudioThreadPriorityError>` becomes `RealtimeDenied(format!(…))` (`error.rs`
    around line 199). This happens once, when the worker starts, before the first period.
- **No allocation:** every other error on this path is either message-less (`Xrun` from
  EPIPE, `DeviceBusy`, …) or carries a `&'static str` (`Cow::Borrowed`, e.g.
  `DeviceNotAvailable` "Device disconnected").

**What that means for the RT contract.** The backend itself allocates the owned messages
on the audio thread. The application can't prevent that allocation. It can only decide
where the memory is freed, and how much can be outstanding at once. There are only three
places the error can end up:

1. Freed in the callback.
2. Handed to another thread through a bounded queue, which is full when the consumer
   stalls.
3. Kept forever (D03's `mem::forget`), which is unbounded while the consumer stalls.

So when the queue is full, no policy can meet both "never frees on the audio thread" and
"bounded memory".

**Policy (R1).**

- `hand_off_stream_error` counts every error by `ErrorKind` in relaxed atomics: 14 known
  kinds plus "unknown". It then pushes the error into a 16-slot `rtrb` queue that the UI
  thread (kabl-ui) or the main loop (`kabl`) drains.
- A bare `Xrun` is not queued (it owns nothing); callers count xruns separately.
- If the queue is full, the error is dropped in the callback and counted as "undelivered".
  At most 16 errors are outstanding. Only a `BackendError` or `RealtimeDenied` message is
  freed there, and it is a block cpal allocated on this same thread moments earlier. The
  hand-off itself never allocates, formats, logs, locks or blocks.
- The count is kept by kind. The message text of an undelivered error is lost; the last
  delivered message is logged.

**Contract adjustment (for the supervisor/owner).**

- Original clause: "the error callback never frees on the audio thread".
- Proposed clause: "the error callback frees on the audio thread only a message that the
  backend allocated on that thread in the same call, and only while the hand-off queue is
  full".
- Rejected alternatives: keeping `mem::forget`, which has no bound; a larger queue, which
  moves the limit but keeps it; a deferred-free list, which is unbounded; and a blocking
  or locking writer.
- The data callback's contract (no allocation or free) is unchanged.
- Regression test: `crates/standalone/tests/stream_error_overflow.rs`. It sends 1000
  owned errors plus other kinds through a paused consumer, then checks the live heap
  blocks (never more than 16 outstanding), the frees per call (exactly one on overflow,
  never an allocation), the per-kind and undelivered counts, drain and recovery, and that
  shutdown leaves nothing behind. Both binaries call this same function.
- Not logged by default: audio, patch contents, notes, typed search/rename text.
