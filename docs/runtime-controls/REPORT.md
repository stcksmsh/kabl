# D03-R2 + D04 report

```text
Batch: Kosta's combined assignment. D03-R2 (Why no sound? names an unplugged audio input;
  the stream-error log line gives the message's age), then D04 (runtime controls without
  recompilation). Brief: docs/product-research/briefs/D04.md; launch: D04-LAUNCH.md.
Starting commit: 4f7e729 (origin/master; planning on top of product 6404b21, the PR #5 merge)
Product commits: 9adac68 (D03-R2); 4bd84b3, 9a4864a, 07b2607, 7e5d908, b20e758 (examples),
  5e148f1 (opt-in latency measurement), e7dca9a (clippy); b2e9fd6 (a test moved to its own
  file). Everything after b2e9fd6 is docs/evidence.
Tested head: b2e9fd6. cargo test --workspace 540 passed, 0 failed, 16 ignored; clippy clean
  (evidence/test-workspace.txt, evidence/clippy.txt)
Evidence build: release e7dca9a (perf, walkthrough, screenshots, package). The runtime
  behaviour is identical to b2e9fd6, which only moved a test.
Reviewed head: NONE. The independent reviewer subagent was stopped by a platform rate limit
  before writing any finding (REVIEW.md). This is an open gap, not a pass.
Submitted head: the branch head that contains this file (product code = b2e9fd6)
Branch / PR: claude/d02-musical-controls-knjmgu, which the cloud environment requires. It was
  reset to master 4f7e729 for this assignment. New PR for Kosta (the merged PR #5 is not reused).
Engineering status: implemented and evidence produced. NOT complete: independent review pending.
Owner review: pending (CHECKLIST.md; D03-R2 items 13–15 in signal-inspection/CHECKLIST.md)
```

## What now works (user-facing)

- **Runtime controls.** Turning a rack knob, a Perform slider or macro, a pinned control, or a
  mapped controller knob no longer builds a graph. Notes, envelopes, sequencer positions and
  tails continue, and the inspector keeps measuring.
- **Structural edits still compile.** That covers module and cable edits, route bypass, and a
  MIDI In's mode, priority and glide.
- **MIDI without the editor.** Mapped CCs (with pickup) and MIDI buttons (Run/Stop, Restart,
  banks, cues) are applied by a control thread whether or not a window frame is drawn.
- **Status bar.** It shows "N change(s) waiting for audio" and "recompile failed: …". When a
  compile fails, the last good graph keeps playing.
- **D03-R2.** *Why no sound?* names a stage whose audio input has no cable. It leaves optional
  inputs alone, and it offers unconnected audio outputs as a possibility. The stream-error
  line gives the age of the last delivered message.

## Acceptance matrix (D04.md)

| Criterion | Part | Result | Evidence |
|---|---|---|---|
| D03 cable diagnosis | D03-R2 | pass (automated + scripted real app) | `signal_inspection.rs`: `why_no_sound_names_an_unplugged_intermediate_audio_input`, `why_no_sound_leaves_optional_inputs_alone`, `needed_inputs_are_real_audio_inputs`; walkthrough 55.6–68 s (VCA #14, audible removal, named input, undo); `img/*-why-audio-input.png`, `*-why-undone.png` (Init Keyboard, both sizes/themes) |
| Error-message age | D03-R2 | pass (automated) | `stream_error_line.rs` |
| Control classification | D04 | pass (generated + tests) | `classification.md` (every built-in param with its reason), `only_runtime_values_are_runtime_changes`, `only_continuous_unsmoothed_values_ramp` |
| No routine recompilation | D04 | pass (automated + real app) | `routine_controls_never_build_a_graph`. perf: knob 1075–1148 → 0 builds, cc 1230–1266 → 0; package run 0 graphs / 137 values; walkthrough log (only cable edits compile) |
| Compatibility | D04 | pass for fixed settings. Transitions differ by design and are documented | `render_hash`: all 17 factory sounds give bit-identical hashes to 6404b21 (engine unchanged since b20e758); `a_set_value_renders_like_a_compile_of_it`; `evidence/transition-compare.txt` |
| Ordering and identity | D04 | pass (automated) | `values_and_graphs_keep_revision_order`, `stale_and_reused_targets_are_rejected`, `a_structural_edit_while_controls_move_keeps_every_value`, `comparison_restore_and_recipe_like_edits_converge_on_the_document`, `deletion_recreation_and_a_new_document_reject_old_values` |
| Backpressure and failure | D04 | pass (automated) | `a_paused_audio_thread_bounds_the_queue_and_gets_the_last_value`, `draining_is_bounded_ordered_and_allocation_free`, `a_failed_compile_keeps_the_graph_and_runtime_values_still_apply`; refused discrete commands are counted ("actions not delivered"), not reported as success |
| Editor independence | D04 | pass (automated + real app) | `midi_buttons_and_pickup_work_without_a_frame_and_the_ui_catches_up`; perf cc runs (control thread; CC to callback p50 ≈3 ms) |
| RT safety | D04 | pass for the checked paths | The ui production-path tests forbid allocation in every audio callback, through saturation and swaps. The narrowly accepted D03-R1 backend-error exception is unchanged. The source review of the `Core` mutex and callback destruction was the implementer's own: **not independently reviewed** |
| D02/D03 integration | both | pass (automated + real app) | `inspector_readings_survive_runtime_edits_but_not_rewiring`; walkthrough 46–53 s (inspect while controls move), 29–44 s (comparison restore without a compile) |
| Performance | D04 | measured (cloud VM) | README "Performance", `evidence/perf-table.md`: 3 runs per workload per build, interleaved; two censored stall runs, one per build |
| Delivery | both | pass (cloud) | workspace tests/clippy at b2e9fd6; `evidence/package.txt` (outside the checkout, clean shutdown) |
| Independent review | both | **not done** | REVIEW.md: the reviewer subagent was rate-limited before any finding |
| Owner | both | **pending** | CHECKLIST.md |

## Changes

See README "How it works (code)". In brief:

- **Engine:** `crates/engine/src/runtime.rs` (new), plus `compile.rs` and `patch_engine.rs`.
- **UI:** `crates/ui/src/control.rs` (new), plus `main.rs`, `perform.rs` and `lib.rs`.
- **D03-R2:** `inspect.rs`, and `standalone/src/lib.rs` and `main.rs`.
- **Tests:** engine and ui `runtime_controls.rs`, additions to `signal_inspection.rs`, and
  `stream_error_line.rs`.
- **Examples:** `render_hash`, `transition_compare`.

## Decisions

These are recorded in `docs/decisions.md`, "2026-09-26 — D03-R2 and D04 runtime controls
(implementation rationale)". Summary, with rejected alternatives there:

- the audio side converges to the document by a diff;
- one FIFO with revisions;
- only `midi.in` mode, priority and glide are structural;
- ramps last 15 ms and end on the exact value;
- bounded queues and held values;
- a control thread for MIDI.

## Verification

Environment: cloud container, Ubuntu 24.04, 4 vCPU, rustc 1.94.1, Xvfb, PipeWire 1.0.5 null
sink, fifo MIDI stand-in, no RT priority.

- `cargo test --workspace` at b2e9fd6 gives 540 passed, 0 failed, 16 ignored (exit 0).
- Before b2e9fd6, one workspace run failed in `stream_error_overflow`. Cause: the D03-R2 test
  had been added to the same binary, and its parallel allocations shifted the process-wide
  heap count. The test was moved to its own file, after which 12/12 focused runs passed.
  The failure is disclosed in `signal-inspection/REPORT.md`.
- `cargo clippy --workspace --all-targets` is clean at b2e9fd6.
- The package was built from the checkout, installed outside it, and run with a fresh HOME.

## Measurements

README "Performance": Composition at 48 kHz / 256, base `6404b21` (plus the measurement-only
patch) against final `e7dca9a`, 3 runs each, interleaved.

- **Routine controls:** knob and CC graph builds drop to 0. The execution p99 bin drops from
  ≤1000–1100 µs to ≤550 µs, the same as steady play.
- **CC to audio callback:** p50 49–50 ms → 2.9–3.1 ms; max 76–167 ms → 16–24 ms.
- **Topology swaps:** unchanged, 201 builds in both.
- **Tails:** worst-case execution, arrival and xruns vary more between runs than between builds.
- **Stalls:** the known stream stall censored one baseline run (swaps-base-2) and one final
  run (knob-final-1). No cause is assigned; recovery belongs to D05.

## Compatibility

- The patch format, saved mappings and the op log are unchanged.
- Fixed-setting renders of all factory sounds are bit-identical to the baseline.
- A knob change mid-note now transitions through a 15 ms ramp or the module's own smoothing,
  where it used to crossfade two graphs. The difference is confined to the transition, and is
  small where envelope or tail state carries it (`transition-compare.txt`).

## Limits / deviations

- **No independent review** (REVIEW.md). This work must not be treated as reviewed or
  merge-ready until a fresh reviewer has inspected it and rechecked the final head.
- All evidence is cloud and scripted. There is no listening, controller-feel, beginner or
  laptop evidence.
- Two perf runs were censored by the cloud stream stall. It happened in both builds, with no
  cause known.
- Rules from design.md §12:
  - past 32 simultaneous ramps per graph, further targets step instead of ramping;
  - commands refused by a full queue are reported, not retried;
  - CC latency while the editor draws includes up to one layout pass.
- The evidence media was recorded on the `e7dca9a` build. `b2e9fd6` only moved a test.

## Owner checklist

[CHECKLIST.md](CHECKLIST.md): 11 items, and D03-R2 items 13–15 in
`docs/signal-inspection/CHECKLIST.md`. Everything is pending.

## Follow-ups (not implemented)

- The independent review and recheck described above, once reviewer capacity returns.
- D05: stream stall recovery. The stall appeared again in both builds here.
- Two routes to a lower CC latency: a MIDI-thread fast path, or not taking the lock during
  layout. Neither is needed for correctness.

## Repo status

- Clean at the submitted head. `.ai/` is untouched.
- rustfmt was run only on changed files.
- No force push. The branch was reset to master for this assignment, and its one unpushed
  commit was recovered from the reflog.
