# D03 report

```text
Batch ID / outcome: D03 — Signal inspection and three listening recipes (+ owner-requested
  operational logging). A musician can inspect one signal, tell a broken connection from an
  unobserved trigger, and try three short listening experiments without losing work.
Starting commit: e19a6ff (origin/master; product = 4eb4104 D02 merge, plus the D03 brief)
Submitted head commit: superseded by the D03-R1 follow-up below (tested head 3c8872e)
Branch / pushed remote / PR if environment-required: claude/d02-musical-controls-knjmgu on
  origin (the cloud environment requires this branch name); PR for Kosta:
  https://github.com/stcksmsh/kabl/pull/5 (not merged by the agent).
Engineering status: submitted
Owner-review status: pending
```

## D03-R1 follow-up (supervisor correction, brief `docs/product-research/briefs/D03-R1.md`)

```text
Batch: D03-R1, which bounds stream-error ownership (R1) and refreshes the affected evidence
  on the final head (R2)
Starting point: PR #5 head 08f9d06 (reviewed product 05734dd); master planning docs merged
  in with 55430ad
Product commits:
  - 25a9f78: error hand-off
  - 99c28d3: probe_cost gains parameter-edit and topology modes (example only)
  - f1edf70: comments/docs
  - review fixes (R1-1..R1-7): kabl-ui drops the stream before the error queue (R1-5),
    plus comments/docs; see the review section
Tested head: see "Verification after the review" below.
Evidence build: release 99c28d3. Product behaviour equals the tested head except R1-5's
  teardown order, which none of the recorded flows exercises.
Engineering status: submitted. R1 includes a contract adjustment that needs a decision.
Owner review: pending
```

**R1 disposition: bounded, with an explicit contract adjustment.**

- **Where the callback runs.** On cpal 0.18.2 ALSA, the only Linux backend built, the error
  callback runs on the stream worker (audio) thread. cpal allocates `BackendError`
  (`alsa::Error` text) on that thread immediately before calling it. `RealtimeDenied` would
  own a message only with cpal's `realtime` feature, which kabl does not enable. Every other
  error on that path owns no heap memory.
  Source references are in design.md, "Stream-error ownership".
- **The incompatibility.** Once the hand-off queue is full, no policy can both avoid freeing
  on the audio thread and keep a total bound.
- **What the code does now** (`hand_off_stream_error`):
  - counts every error by kind (14 kinds plus "unknown") in relaxed atomics;
  - queues it in a 16-slot rtrb queue;
  - on overflow, drops it in the callback and counts it as undelivered;
  - never has more than 16 outstanding;
  - still never allocates, formats, logs, locks or blocks.

  The `mem::forget` leak is gone. The drop frees only a message the backend allocated on
  that thread in the same call. That is the **proposed contract adjustment** for
  Kosta/supervisor. Rejected alternatives: a larger queue, a deferred-free list, a blocking
  writer.
- **Test.** `crates/standalone/tests/stream_error_overflow.rs` drives 1000 owned errors plus
  other kinds through a paused consumer and checks:
  - live heap blocks never exceed 16;
  - no allocation, and exactly one free per overflow;
  - exact counts per kind and undelivered;
  - drain and recovery;
  - zero outstanding after shutdown.

  `rt_with_logging.rs` still checks the no-allocation paths. Both binaries call this one
  function. Logs report per-kind counts, the last delivered message and the undelivered
  count.
- **Not tested.** No real device failure was injected. The test calls the hand-off
  directly.

**R2 disposition: done on the final head.**

| Check | Result | Evidence |
|---|---|---|
| Live signal; continuous drag keeps readings; removing the cable into the tapped VCA rejects old readings (new window 0.6 s, silence); undo recovers | pass (scripted, real app) | `r1/r1-app.mp4`, `r1/img/1440x900-{1-live,2-mid-drag,3-after-drag,4-just-removed,5-new-silence,6-why-removed,7-undone}.png`, `r1/r1-app-kabl-debug.log` (about 4 600 parameter swaps, ≈50/s, during the ≈90 s scripted drag; readings stay live) |
| Default-gain VCA not called silent | pass | `r1/img/1440x900-9-default-vca-why.png` (peak −0.6 dBFS; no "passes nothing") |
| Logging: INFO default, DEBUG, sink failure, save failure without paths, bad load | pass (release binary) | `r1/logging/*.txt`; `r1/img/1440x900-{10-log-sink-failure,11-log-sink-hover}.png` ("logging: 8 lines dropped, 1 write failures (last: create log dir: Not a directory (os error 20))") |
| Overflow ownership regression | pass (direct hand-off test, labelled; not a device failure) | `stream_error_overflow.rs` in `r1/test-workspace.txt` |
| Inspection cost: simple and dense, tap off/on, parameter swaps, topology change, sizes | measured (cloud VM, no target) | `evidence/perf.md` "D03-R1 re-measurement", `r1/probe-cost-*.txt` |
| Real app, dense piece, off/on/busy × 3 (+1) | measured. The **cloud stream stall** hit twice in this R2 session: perf busy-1 at 24 s, and the logging "blocked" run ("stalled … after 2428"; its recording has 0 frames). Both were logged and neither recovered (D05). | `r1/perf-app-summary.txt`, `r1/perf-busy-1-stall-kabl.log`, `r1/logging/blocked.txt` |
| Package outside the repo: recipes found, log in `~/.local/state` | pass (cloud). Built from `b298eb6` with an uncommitted docs-only edit (`perf.md`), so the log reads `b298eb65381d+modified`. Product code was that of f1edf70. | `r1/package.txt`, `r1/img/1440x900-package-*.png` |
| `cargo test --workspace` / clippy at 3c8872e | 520 passed, 0 failed, 15 ignored; clippy clean | `r1/test-workspace.txt`, `r1/clippy.txt` |

- **What the older evidence still covers.** Media under `img/`, `evidence/` and
  `walkthrough.mp4` from before the fixes stays as the record of those commits (see
  "Limits" below). They are kept, not regenerated, because nothing changed the lesson and
  layout screenshots or the 212 s tutorial. Since `db29445`, `recipes.rs` and `lib.rs` lost
  only F9's unused `via_cable`/`midi_connected` fields (no text or layout change). The
  behaviour changes in `inspect.rs` (F1–F4, N1) are what the new R2 captures above cover.
- **New follow-up, not fixed here.** After the cable into VCA #4 `in` is pulled, "Why no
  sound?" doesn't name the unplugged audio input. It reports measured silence and points at
  the ADSR/cv instead (`r1/img/1440x900-6-why-removed.png`). `diagnose` checks unconnected
  gate inputs but not audio inputs on the path. This weakens the "intentionally broken
  connection" diagnosis in the real app; the automated disconnected-output case still
  passes. Suggested scope: a small diagnosis addition, in a later repair or D05.

## Commits

- Starting: `e19a6ff` (origin/master at start; product code = `4eb4104`).
- First reviewed head: `496a5c2`. Review fixes: `f28d262` (F1–F10); recheck at `2967b06`
  found N1–N3; fixes `05734dd`; second recheck at `05734dd`: all resolved, no new findings.
- **Tested product head: `05734dd`** (`cargo test --workspace` 519/0/15, clippy clean:
  `evidence/test-workspace.txt`, `evidence/clippy.txt`). **Reviewed head: `05734dd`.**
  **Submitted head:** the next commit(s) change only docs (`REPORT.md`, `README.md`,
  `HANDOFF.md`, `STATUS.md`, evidence text); product code equals `05734dd`.

## What now works (user-facing)

See [README.md](README.md) "What it does": Inspect one output (per voice lane, interval and
age, distinct waiting/stale/unavailable/missing states); "Why no sound?" (graph facts,
measurements, possibilities); Compare (capture; restore as one undo step; Undo/Redo as A/B);
Learn (Pluck to pad, Filter movement, Interlocking sequences); operational logging with a
**Log** button in the status bar.

## Acceptance matrix (brief §E)

| Criterion | Result | Evidence |
|---|---|---|
| Correct tap: known audio/CV/gate/pitch fixtures, before buffer reuse, short edges, voice/channel, feedback/source | pass (automated) | `crates/engine/tests/probe.rs` (DC chain through coalesced buffers, lanes/pitch/held gate, clock edge rate), `probe.rs` unit test (one-sample pulses, NaN); README "Voice, channel and feedback meaning" |
| Lifecycle: selection change, deletion/undo, reused ids, failed compile, overlapping swaps | pass (automated) | engine `swaps_name_the_graph_and_a_missing_target_says_so`, `overlapping_swaps_report_the_newest_graph_and_never_go_back`, `a_knob_being_turned_does_not_stop_the_measurement`; UI `only_current_measurements_are_shown_as_current`, `a_wiring_change_drops_what_was_measured_before_it`, `another_sound_never_inherits_a_measurement_by_reused_id`, `a_report_that_waited_in_the_queue_is_not_live`, `a_stuck_command_queue_is_retried` |
| RT safety: no alloc on changed paths; saturated queue and stalled consumer don't block | pass (automated), **except one path under a proposed contract adjustment (D03-R1).** On overflow the stream-error callback frees a backend-allocated `BackendError` message, and the allocator may lock. `stream_error_overflow.rs` counts that exactly: one free, no allocation, at most 16 outstanding. | `the_audio_thread_path_never_allocates_with_the_tap_on` (full report queue), `crates/standalone/tests/rt_with_logging.rs` (TRACE logger installed, full report and error queues); kabl-ui callback closure itself not under the check (its added work: relaxed atomic adds) |
| Sound preservation: inspection off/on identical | pass (automated) | `inspection_on_or_off_renders_the_same_samples` (every output of composition, palette/pad, echo; bit-identical) |
| Diagnostics: disconnected, missing trigger, valid quiet/stopped | pass (automated + scripted real app) | UI tests `why_no_sound_*`; screenshots `img/*-why-missing-trigger.png`, `img/*-dark-why-disconnected.png`; walkthrough 0:13–0:42 and the stopped piece at 2:35–2:40 |
| Work preservation: restore/undo, unrelated edits, MIDI, Save/Open/New/quit/cancel/failure, history, origin | pass (automated) / partly scripted | `a_restore_is_one_undo_step_and_keeps_the_working_version`, `the_restore_difference_reproduces_every_factory_sound` (all factory pairs), `another_sound_drops_the_reference_and_a_failed_open_keeps_it`, `save_targets_the_current_patch_while_comparing`, recipe start/cancel tests; MIDI CC edits go through the same current patch (no hidden version exists); quit uses the unchanged D01 path (logging runs show `shutdown result=ok` after Ctrl+Q) |
| Recipes: all three complete in the real app with audio, reveal, comparison, exit/keep, unguided task | pass (scripted; not learning evidence) | `walkthrough.mp4`, `evidence/walkthrough-drive.txt`, `evidence/walkthrough-kabl.log` (recipe/compare transitions) |
| Layout/focus: 1440×900 and 1280×800, A-light/A-dark, browser/Perform/drawer/dialog, off-face pin | pass (scripted screenshots) | `img/` (both sizes; light and dark); D02 gaps: `*-lead-glide-{before,shown,back}`, `*-dark-browser-perform-{inspect-search,after-escape}`; UI test `escape_in_a_dialog_over_inspection_and_comparison_closes_only_the_dialog` (Unsaved and Rename) |
| Logging: INFO default, DEBUG override, rotation/queues, sink failure, no callback logging | pass (automated + real binary) | `crates/standalone` unit tests (rotation cap, unwritable dir), `applog_stalled_sink`, `applog_full_disk`, `applog_levels`; `evidence/logging-{default,debug,blocked,badload}.txt`; `img/1440x900-light-log-sink-failure.png` (historical, ef9ed7b). **Final head (D03-R1):** `r1/logging/*.txt`, `r1/img/1440x900-{10-log-sink-failure,11-log-sink-hover}.png` |
| Package: launch outside checkout; recipes discoverable | pass (cloud) | `evidence/package.txt`, `img/1440x900-light-package-interlocking-stopped.png` |
| Regression: focused tests, `cargo test --workspace`, clippy at head | pass | "Final state" below |
| Measurements: off/on, simple and dense, distributions, memory, lateness/xruns apart, rapid selection + edits | done (cloud; no target) | `evidence/perf.md` |
| Owner: laptop audio/controller/feel/learning | **pending** | CHECKLIST.md |

## Changes

- `crates/engine/src/probe.rs` (new), `compile.rs` (tap hook, `set_probe`, `probe_block_end`,
  `continue_probe_from`, `generation`, `output_module`), `patch_engine.rs`
  (`Command::Inspect`, measured graph, report hand-off), `examples/probe_cost.rs`,
  `tests/probe.rs`.
- `crates/ui/src/inspect.rs`, `compare.rs`, `recipes.rs` (new); `lib.rs` (drawer tools row,
  sections, recipe area, module menu), `main.rs` (probe/fault queues, generations, logging,
  health, histogram), `browser.rs` (`Dialog::Restore`, doc log lines), `editor.rs`
  (`restore_to`), `library.rs` (`LibError::log_text`), `record.rs` (log lines),
  `explain.rs` (`can_go_back`); `tests/signal_inspection.rs`, `tests/library.rs`.
- `crates/standalone/src/applog.rs` (new), `build.rs` (commit id), `lib.rs`
  (`hand_off_stream_error`), `main.rs` (logger, fault drain); tests `applog_*`,
  `rt_with_logging.rs`.
- `patches/recipes/{pluck-to-pad,filter-movement}` (new factory sounds).
- `packaging/linux/package.sh` (README.txt names the log).
- Docs: `docs/signal-inspection/*`, `decisions.md`, `HANDOFF.md`, `STATUS.md`.

## Decisions

In `decisions.md` "2026-09-25 — D03: signal inspection, silence aid, comparison, recipes,
logging": tap inside the schedule (rejected: post-block buffer read, pinning); selection as a
runtime command (rejected: recompile per selection); voices kept as lanes (rejected: averaging);
token + generation staleness with parameter edits keeping the floor (rejected: invalidate per
rebuild); comparison as capture + restore-as-one-undo-step (rejected: audition toggle over a
hidden document; rebuilding from bare state); recipes as compiled-in text on factory patches
(rejected: a recipe format/framework); `log` facade + small bounded backend (rejected:
env_logger, tracing-appender, flexi_logger async).

## Verification

Environment: cloud container, Ubuntu 24.04.4, Linux 6.18.44 x86_64, 4 vCPU, rustc/cargo
1.94.1, Xvfb 1600×1000, PipeWire 1.0.5 with a null sink (`kabl_rec`) through ALSA, a session
D-Bus (for wireplumber) but no system D-Bus (RT priority refused), MIDI through the
`KABL_MIDI_PIPE` fifo stand-in. No audio or MIDI hardware.

- `cargo test --workspace` at `05734dd`: exit 0, **519 passed, 0 failed, 15 ignored**
  (`evidence/test-workspace.txt`); reviewer's own run at `05734dd`: same totals.
- `cargo clippy --workspace --all-targets` at `05734dd`: exit 0, no warnings
  (`evidence/clippy.txt`).
- Earlier: 512/0/15 at `496a5c2`, 518/0/15 at `f28d262`.
- Focused: `cargo test -p kabl-engine --test probe` (9), `-p kabl-ui --test
  signal_inspection` (21), `-p kabl-standalone` (applog unit 4, `applog_*` 3,
  `rt_with_logging` 1).

## Subagent review

Fresh reviewer subagent; base `e19a6ff`, head `496a5c2`; [REVIEW.md](REVIEW.md). Findings:
F1–F3 major, F4–F8 minor, F9–F10 nits, fixed in `f28d262`. Recheck at `2967b06`: F1
partly (N1 major: a measurement window carried across a wiring change), N2/N3 nits, F6
partly (accepted: the kabl-ui callback closure itself is not under an allocation check).
Fixed in `05734dd`; second recheck at `05734dd`: N1–N3 and F1 resolved, no new findings.

## Real-app evidence

`walkthrough.mp4` (212 s) and `img/` — list and reproduction commands in README "Evidence"
and "Reproduce". All scripted (xdotool on the release binary), cloud VM.

## Measurements

`evidence/perf.md`. Offline (engine only): a steady 8-lane tap adds ≈9–11 µs at the median
of a 256-frame callback (≈0.2 % of the 5333 µs budget); simple voice p50 107.8 → 116.9 µs,
dense piece 276.8 → 287.2 µs; memory 720 B per graph + 8 × 552 B queue. Real app on the
dense piece, 3 runs × off/on/busy: run-to-run spread (0–11 xruns with the inspector off) is
larger than any mode difference; execution, arrival lateness and xruns are reported
separately. No laptop extrapolation.

## Compatibility

Old patches load unchanged (no format change; recipe patches are ordinary v3 folders). Undo
history: comparison restores are ordinary undo entries; inspection and recipes add none.
Sound: bit-identical with the tap on or off. `KABL_STATS_FILE` first line unchanged (a second
line added; the `watch_stats.py` regex still matches). No plugin automation involved.

## Limits / deviations

- All evidence is cloud and scripted; no controller, laptop or human learning evidence.
- Short audio dips in the cloud walkthrough (5 × 5 ms) correlate with VM xruns; cause unknown.
- Comparison is restore/undo, not an instant audition toggle (allowed by the brief); a
  restore replaces the redo tail like any edit.
- Only outputs are measured; one at a time; up to 16 voice lanes.
- No injected audio-device fault; stream-error handling tested at the hand-off function.
- Evidence media (walkthrough, screenshots, logging and perf runs, package) was recorded at
  `ef9ed7b`–`db29445`, before the review fixes; the fixes change staleness handling, the
  aid's wording for VCAs/filters and log text, not the flows shown. The knob-drag check
  (`scripts/knob-drag.txt`, `img/1440x900-light-knob-drag-live.png`) is at `f28d262`.

## Owner checklist

[CHECKLIST.md](CHECKLIST.md): launch commands and 12 actions.

## Follow-ups (not implemented)

- Cable → source navigation for inspection (optional in the brief).
- A deterministic audio-stream fault hook (belongs with D05 recovery).
- The kabl-ui audio callback body is not factored into a testable function; its allocation
  freedom is shown for the engine and hand-off paths only.
- The build script records the commit when git's HEAD/index change; a dirty working tree
  built without a new commit is not marked "+modified".

## Repo status

Clean at the submitted head; `.ai/` untouched; no unrelated files reformatted
(`rustfmt` on changed files only).
