# D03 — Signal inspection and three listening recipes

Owner-authorized batch (brief: [`../product-research/briefs/D03.md`](../product-research/briefs/D03.md)).
Engineering: submitted (tested and reviewed product head `05734dd`), see
[REPORT.md](REPORT.md) and [REVIEW.md](REVIEW.md). **Owner review:
pending** ([CHECKLIST.md](CHECKLIST.md)). Design note written first: [design.md](design.md).

Built in a cloud session on branch `claude/d02-musical-controls-knjmgu` (the environment
requires this branch name; it holds D03 on top of master `e19a6ff`). Exact commits: REPORT.md.

## What it does

- **Inspect one signal.** Routing drawer → **Inspect**, or a module's right-click menu →
  *Inspect output "…"*. One output of one module is measured at a time, with the module,
  port, type and voice scope named ("8 voice lanes, each measured apart" or "one shared
  instance"). Audio: peak dBFS of the loudest lane and lane bars. Gate: rising edges through
  0.5 (one-sample pulses count) and lanes held high. CV: range against its nominal ±1 or
  0…1. Pitch: semitones from note 60 with the note name per lane; 0 st is a valid pitch.
  Every reading states its interval ("over the last 1.0 s (20 windows of 51 ms)") and age.
  Waiting, not arriving, stale (greyed, with age), not compiled, not in the playing graph,
  no audio engine and "no longer in the patch" are separate states, never a zero.
- **Why no sound?** In the inspector. Three lists kept apart: *From the patch* (no Output
  module; which of several Outputs the engine actually plays; unplugged Output inputs; the
  signal path from the inspected module to the played Output or its absence, with a search
  limit stated as unknown; envelope gates with no cable; clocks the transport stopped),
  *Measured* (the tap over its interval and the final output meter, "before the audio
  device"), *Could be (check next)* (a VCA at 0 with no cv, a very low cutoff, no key started a
  note, where to inspect next with a button). It never claims anything about speakers, the
  device, the controller or your intent, and never edits.
- **Compare with a reference.** Drawer → **Compare**. *Capture reference* copies the whole
  patch (values, cables, routes, labels, Perform pins and MIDI mappings) in memory only.
  *Restore reference…* asks first, naming that scope, then replaces the current patch as
  **one undo step**: *Back to my version (Undo)* returns your version with every edit made
  since, *Hear the reference again (Redo)* switches back. What you hear, edit and save is
  always the one current patch. Opening another sound drops the reference (said once).
- **Three recipes.** Drawer → **Learn**: *Pluck to pad*, *Filter movement*, *Interlocking
  sequences*. Each opens its own factory patch through the ordinary unsaved-changes question,
  captures a reference of that starting point, and has five short steps with Show/Back to the
  exact control, an optional Inspect, a fixed-length *Play test note* (middle C, velocity 100)
  where a keyboard patch needs one, a Compare step and a final task without guidance.
  Close keeps the patch and edits. Opening another sound detaches the recipe.
- **Operational logging.** `~/.local/state/kabl/logs/kabl.log` (see Logging).

## Decisions

Rationale and rejected alternatives: `decisions.md`, "D03: signal inspection, silence aid,
comparison, recipes, logging". In short: the tap reads the module's own output buffer inside
the schedule (before buffer reuse); selection is a runtime command; comparison is
capture + restore-as-one-undo-step (the brief's allowed restore/undo form, no hidden second
document); recipe text is compiled in, recipe patches are ordinary factory sounds; logging is
the `log` facade with a small bounded backend.

## How it works (code)

- `crates/engine/src/probe.rs`: `ProbeTarget`, `ProbeReport`, per-lane `LaneStats`, the
  fixed-size tap. `compile.rs`: the tap hook in `process_block_with`, `set_probe`,
  `probe_block_end`, `generation`, `output_module`. `patch_engine.rs`: `Command::Inspect`,
  measuring the graph fading in, `take_probe_report`.
- `crates/ui/src/inspect.rs` (selection, acceptance, panel, `diagnose`), `compare.rs`
  (`diff`, `restore`, panel), `recipes.rs` (the three recipes, their patch builders,
  panel), `browser.rs` (`Dialog::Restore`, doc log lines), `editor.rs` (`restore_to`),
  `main.rs` (probe queue, generations, fault queue, health logging, histogram).
- `crates/standalone/src/applog.rs` (+ `build.rs` for the commit id); both binaries call it
  once in `main`.
- Patches: `patches/recipes/pluck-to-pad`, `patches/recipes/filter-movement` (regenerate with
  `cargo test -p kabl-ui --test library write_factory_library -- --ignored`, then revert
  the unrelated timestamp churn it writes into `patches/init-keyboard/log.jsonl`).

## Voice, channel and feedback meaning

A port is one mono channel (a stereo module has separate ports). A module compiled per voice
has one lane per voice slot (8); lanes are measured separately and shown separately, the
level is the loudest lane and edges are summed; nothing is averaged. A lane is a voice slot,
not "the chord". The measured signal is the source output of this block: a feedback input
reads it one block (64 samples) later, a global module reading a voice chain gets the voice
average, and a modulation route scales it — none of those are shown as the tap. During a
crossfade the measurement is of the graph fading in, and says so.

## Logging

- Level: INFO by default in the release binary. `KABL_LOG=debug` (or `trace`, `warn`,
  `error`, `off`) or `--log-level debug` changes it at launch; nothing is compiled out.
- File: `$KABL_LOG_DIR`, else `$KABL_USER_DIR/logs`, else `$XDG_STATE_HOME/kabl/logs`,
  else `~/.local/state/kabl/logs`: `kabl.log`, rotated at 1 MiB to `kabl.1.log` …
  `kabl.3.log` (at most 4 MiB). The status bar's **Log** button shows the path and copies
  it; it turns into **Log ⚠** with a one-time message when lines were dropped or writing
  failed. The first line of each run prints the path to stderr too.
- Line: UTC time, seconds since start, level, subsystem, session id, message with
  `key=value` fields. The start line has version and commit (`unavailable` outside git).
- INFO: start/shutdown, audio backend, requested and actual rate/buffer, RT outcome, MIDI
  connect/disconnect/reconnect, open/save completion, recorder start and outcome, audio
  health counts when xruns or late executions happened (per 10 s). WARN/ERROR: failed
  open/save/compile/recording, RT refusal, stream errors (counted), stalls, unknown log level.
  DEBUG: graph compile with generation and duration, inspection/recipe/comparison
  transitions, arrival-lateness counts, a full timing line per minute.
- Not logged: audio, patch contents, notes, search text, typed names (user sounds are
  logged as `origin=user`; a failed library open/save logs the error kind and the system's
  reason, e.g. `io (No space left on device)`, without its paths). The start line, the
  library directories, a failed `--patch` path and device names appear because they help
  diagnosis, so a log can contain local details.
- The audio callback and the stream error callback never call the logger; see
  `decisions.md`. The error callback counts each `cpal::Error` by kind and moves it into a
  16-slot queue for the UI thread (the standalone `kabl` drains it every second). When that
  queue is full, the error is dropped in the callback and counted as "not delivered". At
  most 16 errors are outstanding. Dropping an error there frees its message only if cpal
  allocated one on that same thread (on this build: `BackendError` only). This
  is a proposed contract adjustment, described in design.md, "Stream-error ownership".
  The log line gives the counts per kind, the last delivered message and the number not
  delivered. `KABL_STATS_FILE` keeps its first line; a second line adds the histogram.

## Evidence (cloud VM; not laptop)

**D03-R1** added evidence on the final product head in [`r1/`](r1/): the real-app sequence
(live signal, knob drag, cable removal, undo, default-gain VCA), the logging runs including
the sink failure, re-measured cost (`evidence/perf.md`, last section) and the package smoke
test. REPORT.md, "D03-R1 follow-up", indexes it. Media outside `r1/` is historical: it was
recorded before the review fixes.

Environment: see REPORT.md. Scripted real X input (xdotool) on the release binary, audio
into a PipeWire null sink, MIDI through the fifo stand-in (`KABL_MIDI_PIPE`). No RT
priority (no system D-Bus).

- **Walkthrough** [`walkthrough.mp4`](walkthrough.mp4) (212 s, 1440×900, recorded at
  `ef9ed7b`): inspect while playing; missing trigger; both Output plugs pulled, then undone;
  the three recipes with Show, inspection, test notes, restore/undo comparison, the unguided
  step and Close; the stopped piece as the valid-quiet case. Script
  [`scripts/walkthrough.txt`](scripts/walkthrough.txt), recorder
  [`record-walkthrough.sh`](record-walkthrough.sh). Step times
  [`evidence/walkthrough-drive.txt`](evidence/walkthrough-drive.txt); the run's DEBUG log
  [`evidence/walkthrough-kabl.log`](evidence/walkthrough-kabl.log); stats
  [`evidence/walkthrough-stats.txt`](evidence/walkthrough-stats.txt).
- **Gap check** ([`scripts/gaps.py`](scripts/gaps.py) →
  [`evidence/walkthrough-gaps.txt`](evidence/walkthrough-gaps.txt)): no sample jumps above
  0.5; five 5 ms dips (40.8, 90.5 ×2, 194.3, 211.2 s). The run had 6 xruns and 30 late
  executions (stats file). The dip at 194.3 s is 0.2 s before the "restore applied" log line
  of the third recipe; the others have no script event nearby. These are correlations on a
  VM without RT priority, not a proven cause, and the same kind of dips were reported in
  earlier batches' cloud runs.
- **Screenshots** (`img/`, both sizes): `*-light-inspect-chord` (lanes while a chord is held),
  `*-light-why-missing-trigger`, `*-dark-why-disconnected`, `*-dark-recipe-step`,
  `*-light-restore-dialog`, `*-light-restored-undo-offered`, `*-light-lead-glide-{before,shown,back}`
  and `*-dark-browser-perform-{inspect-search,after-escape}` (the two D02 carried gaps), and
  `1440x900-light-log-sink-failure` (log location blocked). Script:
  [`scripts/shots.sh`](scripts/shots.sh).
- **Logging** ([`scripts/logging.sh`](scripts/logging.sh)): `evidence/logging-default.txt`
  (INFO/WARN only, injected Save As and recorder failures, recipe open, shutdown),
  `evidence/logging-debug.txt` (the same with `KABL_LOG=debug`: compile generations and
  durations, recipe/compare/inspect transitions), `evidence/logging-blocked.txt` (log
  directory blocked: the app runs; the status bar shows it), `evidence/logging-badload.txt`
  (a missing `--patch`: ERROR line written before exit 1).
- **Knob turned while inspecting** (review F3/N1): [`scripts/knob-drag.txt`](scripts/knob-drag.txt),
  `img/1440x900-light-knob-drag-live.png` — VCA #4 out stays live while Filter #3 Cutoff is
  dragged continuously (a rebuild per frame), labelled as measured on the edited version.
- **Tests / clippy at `05734dd`**: [`evidence/test-workspace.txt`](evidence/test-workspace.txt)
  (519/0/15), [`evidence/clippy.txt`](evidence/clippy.txt).
- **Measurements**: [`evidence/perf.md`](evidence/perf.md).
- **Package**: [`evidence/package.txt`](evidence/package.txt).

## Known limits

- Owner hands-on checks, controller feel, laptop timing and learning observations are all
  pending. Scripted recipe completion is not learning evidence; `docs/confusions.md` is
  unchanged because no person was observed.
- Measurement is of outputs only (not inputs or effective parameter values), one at a time.
  A voiced module shows its first 16 lanes.
- The comparison is not a sample-exact A/B: a restore rebuilds like any edit (crossfade;
  held notes, tails and phases continue). No loudness matching.
- Like any edit, a restore replaces the redo tail (the dialog says so when there is one).
- No deterministic audio-device fault hook exists; the stream-error hand-off is tested
  directly (`crates/standalone/tests/rt_with_logging.rs`, `stream_error_overflow.rs`), not
  by an injected device failure.
- A stream error that finds the 16-slot queue full is dropped in the error callback and
  counted by kind. This bounds outstanding error memory. It also means a message that cpal
  allocated on the audio thread is freed there, which is the contract adjustment for the
  supervisor/owner (design.md). Messages of undelivered errors are not kept.
- Audio dips in the cloud walkthrough remain unexplained VM observations.
- The cloud stream stall hit twice during the D03-R1 runs (perf busy-1 at 24 s, and the
  logging "blocked" run). It was logged each time; nothing recovers the stream (D05).
- "Why no sound?" does not name an unplugged audio input on the path; it checks gate inputs
  only (D03-R1 follow-up).

## Reproduce

Container audio/display (as in earlier batches): a session D-Bus, `pipewire`,
`wireplumber`, `pipewire-pulse`, `pactl load-module module-null-sink sink_name=kabl_rec`,
`Xvfb :97 -screen 0 1600x1000x24 &`. Then:

    cargo build --release -p kabl-ui --bin kabl-ui --example midi_player
    DISPLAY=:97 docs/signal-inspection/record-walkthrough.sh
    python3 docs/signal-inspection/scripts/gaps.py target/signal-inspection-walkthrough
    DISPLAY=:97 docs/signal-inspection/scripts/shots.sh
    DISPLAY=:97 docs/signal-inspection/scripts/logging.sh
    DISPLAY=:97 RUNS=3 docs/signal-inspection/scripts/perf.sh
    cargo run --release -p kabl-engine --example probe_cost -- patches/composition 60 3
    cargo test --workspace && cargo clippy --workspace --all-targets
