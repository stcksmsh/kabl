# D03 report

```text
Batch ID / outcome: D03 — Signal inspection and three listening recipes (+ owner-requested
  operational logging). A musician can inspect one signal, tell a broken connection from an
  unobserved trigger, and try three short listening experiments without losing work.
Starting commit: e19a6ff (origin/master; product = 4eb4104 D02 merge, plus the D03 brief)
Submitted head commit: see "Commits" below
Branch / pushed remote / PR if environment-required: claude/d02-musical-controls-knjmgu on
  origin (the cloud environment requires this branch name); PR for Kosta: see "Commits".
Engineering status: submitted
Owner-review status: pending
```

## Commits

- Starting: `e19a6ff`. Implementation commits `34bd347` … `496a5c2` (reviewed), review
  fixes `f28d262`, docs `2967b06` and later.
- Tested / reviewed / submitted: filled in at the end of this file ("Final state").

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
| RT safety: no alloc on changed paths; saturated queue and stalled consumer don't block | pass (automated) | `the_audio_thread_path_never_allocates_with_the_tap_on` (full report queue), `crates/standalone/tests/rt_with_logging.rs` (TRACE logger installed, full report and error queues); kabl-ui callback closure itself not under the check (its added work: relaxed atomic adds) |
| Sound preservation: inspection off/on identical | pass (automated) | `inspection_on_or_off_renders_the_same_samples` (every output of composition, palette/pad, echo; bit-identical) |
| Diagnostics: disconnected, missing trigger, valid quiet/stopped | pass (automated + scripted real app) | UI tests `why_no_sound_*`; screenshots `img/*-why-missing-trigger.png`, `img/*-dark-why-disconnected.png`; walkthrough 0:13–0:42 and the stopped piece at 2:35–2:40 |
| Work preservation: restore/undo, unrelated edits, MIDI, Save/Open/New/quit/cancel/failure, history, origin | pass (automated) / partly scripted | `a_restore_is_one_undo_step_and_keeps_the_working_version`, `the_restore_difference_reproduces_every_factory_sound` (all factory pairs), `another_sound_drops_the_reference_and_a_failed_open_keeps_it`, `save_targets_the_current_patch_while_comparing`, recipe start/cancel tests; MIDI CC edits go through the same current patch (no hidden version exists); quit uses the unchanged D01 path (logging runs show `shutdown result=ok` after Ctrl+Q) |
| Recipes: all three complete in the real app with audio, reveal, comparison, exit/keep, unguided task | pass (scripted; not learning evidence) | `walkthrough.mp4`, `evidence/walkthrough-drive.txt`, `evidence/walkthrough-kabl.log` (recipe/compare transitions) |
| Layout/focus: 1440×900 and 1280×800, A-light/A-dark, browser/Perform/drawer/dialog, off-face pin | pass (scripted screenshots) | `img/` (both sizes; light and dark); D02 gaps: `*-lead-glide-{before,shown,back}`, `*-dark-browser-perform-{inspect-search,after-escape}`; UI test `escape_in_a_dialog_over_inspection_and_comparison_closes_only_the_dialog` (Unsaved and Rename) |
| Logging: INFO default, DEBUG override, rotation/queues, sink failure, no callback logging | pass (automated + real binary) | `crates/standalone` unit tests (rotation cap, unwritable dir), `applog_stalled_sink`, `applog_full_disk`, `applog_levels`; `evidence/logging-{default,debug,blocked,badload}.txt`; `img/1440x900-light-log-sink-failure.png` |
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

Commands and totals: "Final state" below. Earlier runs: 512/0/15 at `496a5c2` (reviewer's
own run too), 518/0/15 at `f28d262`.

## Subagent review

Fresh reviewer subagent; base `e19a6ff`, head `496a5c2`; [REVIEW.md](REVIEW.md). Findings:
F1–F3 major, F4–F8 minor, F9–F10 nits; all fixed in `f28d262` (responses in REVIEW.md).
Recheck: see REVIEW.md "Recheck".

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
- Evidence media predates the review fixes except the knob-drag screenshot.

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
