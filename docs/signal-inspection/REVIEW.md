# D03 review: signal inspection, silence aid, comparison, recipes, logging

- **Reviewer:** a new reviewer subagent (Claude). It had no part in the implementation and did not change product code.
- **Reviewed base:** `e19a6ff` (origin/master = product master `4eb4104` plus docs)
- **Reviewed head:** `496a5c2` (branch `claude/d02-musical-controls-knjmgu`)
- **Contract:** `docs/product-research/briefs/D03.md`. The format follows AGENT-WORKFLOW's "Reviewer subagent before submission" section.
- **Status:** these are review findings only. This is not approval, and it is not Kosta's hands-on review.

## Coverage

What I read, in the code rather than in the implementer's description:

- **Engine.** I read `engine/src/probe.rs` in full. In `compile.rs` I read the whole diff: the tap hook in `process_block_with`, `set_probe`, `probe_block_end`, `output_module`, and `coalesce_buffers` and its pinning rules. In `patch_engine.rs` I read `inspect`, `move_probe`, `receive_swap`, pending promotion and the probe section of `process_block`. I also read `engine/tests/probe.rs` and `examples/probe_cost.rs` (skimmed the second).
- **UI crate.**
  - Read in full: `inspect.rs`, `compare.rs` and `recipes.rs`.
  - Diff only: `main.rs` (audio callback, error callback, rebuild/generation, health logging, histogram, stats file), `lib.rs`, `browser.rs` (the restore dialog and doc logging), `editor.rs` (`restore_to`), `record.rs` and `explain.rs`.
  - Checked against these: `core/src/state.rs` (`inverse_for`, `apply`, the fields of `PatchState`), module defaults in `modules/src/builtins/{vca,filter_svf,filter_ladder}.rs`, and `ui/tests/signal_inspection.rs`.
- **Standalone crate.** I read `applog.rs` in full, plus the diffs of `build.rs`, `main.rs` and `lib.rs`, and the three `tests/applog_*.rs` files.
- **Factory patches.** I checked the params of every VCA across `patches/**`, and the `interlocking` wiring that the recipe text describes.
- **Evidence.** I read `README.md`, `design.md` and `evidence/perf.md`, and skimmed `evidence/logging-*.txt`. I only listed `walkthrough.mp4`, `img/*` and the walkthrough logs; I did not watch or view them.

What I ran at `496a5c2`, in the cloud container:

- `cargo test --workspace`: exit 0, **512 passed, 0 failed, 15 ignored**.
- `cargo clippy --workspace --all-targets`: exit 0, no warnings in the output tail.
- Two throwaway programs in a scratch crate outside the repo, which used the public `kabl_ui::inspect` and `kabl_engine` APIs to reproduce F1, F2 and F3. Their output is quoted under the findings.

## Findings

| ID | Severity | File / function | Concrete failure scenario | Supporting evidence | Requested correction |
|---|---|---|---|---|---|
| F1 | major | `ui/src/inspect.rs` `Inspect::rebuilt` / `accept` / `summary` / `status` | A topology edit raises `floor`, but reports already accepted into `reports` stay there. `summary()` combines every accepted window from the last `SUMMARY_SECS` (1 s) by arrival time, whatever graph produced it. Example: the user inspects VCA #2 out (−6 dBFS) and deletes the cable into the VCA. For up to 1 s the panel shows **Live**, "peak −6.0 dBFS", with the label "updated … s ago", even after the new graph has reported silence. The Why panel uses the same summary, so it reports −6 dBFS "Measured" at the VCA while the output meter reads below −90 dBFS. The brief requires that deletion/undo and similar changes "cannot show stale measurements as current". The prepared removed-connection case is exactly this edit. | Scratch program: accept gen 10 at 0.5 peak → `rebuilt(11)` after removing the cable → `status` = `Live`, `summary.peak_all()` = 0.5, `generation` = 10. Accept gen 11 at peak 0 → still `Live`, peak 0.5, generation 11, 2 windows. `measurement_lines` gave "At VCA #2 out: peak -6.0 dBFS over the last 0.1 s". `only_current_measurements_are_shown_as_current` only tests `accept`'s return value, never the summary or status after a floor rise. | When `floor` rises (topology change or failed compile), drop the accepted reports with `generation < floor`, or filter `summary()` and `status()` by `generation >= floor`. Until a report from the new lineage arrives, show "Waiting". Add a test: report, topology edit, then a new-graph report; the summary must contain only the new report. |
| F2 | major | `ui/src/inspect.rs` `diagnose`, "Parameters that may hold the path at silence" | `p("gain", 0.0)` treats a missing `gain` as 0. Module params are sparse, though: `Op::AddModule` inserts an empty map, and the engine's VCA default is **1.0**. So a VCA added from the rack, or one in `patches/crowded` (both VCAs have `params {}`), makes the aid say "VCA #n gain is 0 and its cv input has no cable: it passes nothing" while the VCA passes full signal. The check also ignores modulation routes into `gain` (`PortRef::Param`), which the engine applies. This is a false, confidently worded diagnosis, which the brief rules out ("no invented certainty"). | The scratch program built osc → VCA (default params) → out. `diagnose` → possible: "VCA #2 gain is 0 and its cv input has no cable: it passes nothing." An offline render of the same patch had a left peak of **0.989**. `vca.rs`: `default: 1.0`. `state.rs` `AddModule` → `params: BTreeMap::new()`. | Read the effective base value with the registry `ParamInfo::default` as the fallback (or the existing `explain::param_of`). Treat a param with an incoming route as modulated and word that case as a possibility. Add a test with a default-param VCA and one with a route into gain. The `cutoff_hz` fallback (1000) happens to match the default, but it should use the same helper. |
| F3 | major | `engine/src/patch_engine.rs` `move_probe` (from `receive_swap` / pending promotion), `compile.rs` `set_probe` | Every fade start rebuilds the tap in the incoming graph (`set_probe`), which restarts the 50 ms window and resets `prev` to NaN. `main.rs` rebuilds on every frame in which the editor is dirty, so a continuous knob drag starts a new fade every 16–33 ms (crossfade 15 ms). No window ever completes, so **no reports arrive for the whole gesture**. The inspector drops to Stale / "No measurement is arriving" and re-sends `Inspect` every 0.5 s, which restarts the window again. This contradicts design.md ("Parameter-only edits keep the floor, so the meter keeps moving while a knob turns"). It also defeats recipe steps that ask the user to inspect while turning a control ("Inspect LFO #7 … Set Rate", ADSR steps). The "busy" perf run only rebuilt at about 2.5 Hz, so it could not show this. An edge exactly at a fade start is also not counted. | Scratch program on `init-keyboard`, tap on VCA #4, param-only swaps: every 6 blocks (8 ms) → **0 reports in 2 s**; every 12 blocks (16 ms) → **0**; every 24 blocks (32 ms) → **0**; every 40 blocks (53 ms) → 37; no swaps → 40. | Keep the accumulated window and `prev` when the target resolves to the same lanes in the incoming graph. For example, copy the `Tap` accumulator in `move_probe` when the resolved lane count and port match, and mark the window `fading`. Alternatively, publish the partial window, flagged, when a fade starts. Add an engine test that swaps every 12 blocks and still gets reports, and add a real-app run that drags a knob continuously. |
| F4 | minor | `ui/src/main.rs` audio callback (`probe_tx.push`) and `Inspect::accept(r, now)` | An `rtrb` push fails when the queue is full, so the 8-slot queue keeps the **oldest** reports. The UI stamps each report with its own drain time. When the UI stops updating (window minimized or occluded, a long frame, a blocking dialog), the first frame afterwards accepts up to 8 windows that may be seconds old. They are timestamped `now`, count as Live and are merged into the 1 s summary, so old gate edges or levels appear live for up to 1 s. | Code reading: `let _ = probe_tx.push(r)` drops the newest report when full. `accept()` records `now` as the arrival time. The design note says "only the newest few matter". The seq-gap counter shows drops but does not age the reports. | Stamp reports with an audio-side position (a block counter or callback count, which is already available) and compute age from it. Or discard accepted reports that come just before a seq gap. At minimum, do not treat a backlog drained in one frame as fresh. |
| F5 | minor | `ui/src/main.rs` and `standalone/src/main.rs` stream error callbacks | The error callback runs on cpal's stream thread, which is the audio thread for ALSA. When the 8-slot fault queue is full, `faults_tx.push(err)` returns the error and it is dropped there. A `cpal::Error` can carry an owned `Cow` message, so this can **deallocate on the audio thread**. In the standalone binary the queue is drained only every 10 s, so a burst of 9 or more non-xrun errors within 10 s reaches this path. The brief forbids allocation/deallocation in the new RT path, and the design says the error callback hands errors over "unformatted". | `cpal-0.18.2/src/error.rs`: `Error { kind, message: Cow<'static, str> }`. `let _ = faults_tx.push(err);` in both binaries. | Push only the `ErrorKind` (Copy) and a count, keeping the first message by some non-dropping route. Or let the full case forward into a pre-sized slot rather than dropping. Document it. |
| F6 | minor | Evidence / tests: RT safety | `the_audio_thread_path_never_allocates_with_the_tap_on` covers `PatchEngine` only. The new code in the kabl-ui callback closure (probe `take`/`push`, histogram `fetch_add`) and the error callback are not under an allocation check. No RT test runs with the logger installed at DEBUG/TRACE, which the brief asks for ("RT tests must include diagnostics enabled"). No log call reaches the engine today, so the risk is low. The acceptance row is still only partly evidenced. | `engine/tests/probe.rs` L314–353. There is no `assert_no_alloc` around the `main.rs` closure or the error callback, and no test calls `applog::init` at Debug/Trace alongside rendering. | Factor the callback body (engine step plus probe hand-off plus timing record) into a testable function and run it under `assert_no_alloc` with `applog::init(.., Trace, ..)` active. Cover the fault-queue-full path, which also exposes F5. |
| F7 | minor | `ui/src/inspect.rs` `why_panel` → `diagnose` | While "Why no sound?" is open, `diagnose` runs a full `kabl_engine::compile::compile(state, 48000, 1)` on the UI thread **every frame** (at least 20 Hz while audio runs), only to find `output_module()`. That allocates every module's state, including delay and reverb lines, on each frame. It is not RT, but it contradicts "runs on demand … never in the background" in spirit, and its UI cost on dense patches was not measured. | `why_panel` is called from `panel` on every frame while `why` is set. The repaint is `request_repaint_after(50 ms)` while audio runs. I could not time it: see Limits. | Cache the diagnosis per (editor instance, log seq), or take the output module from the last successful `rebuild` (for example, store `output_module()` in `AudioHost` next to `generation`). |
| F8 | minor | `ui/src/browser.rs` doc logging (`save failed … error={e}`, `open failed … error={err}`), README "Logging" | README says typed names are not logged. `LibError::Io` includes the path (`"{what} {path}: {e}"`), and user sound paths come from typed names. `LibError::ReplaceMismatch` prints the typed `name`. So a failed save or open of a user sound can write typed names to the default INFO/WARN log. | `library.rs` `impl Display for LibError` L128–140 and `fn io` L145–146. | Either log only the error kind or `io::ErrorKind` for user-origin failures, or correct the README to say that user paths and names can appear in failure lines. |
| F9 | nit | `ui/src/inspect.rs` `Sel::via_cable`, panel text; `lib.rs` `UiState::midi_connected` | `via_cable: true` is never constructed anywhere, so cable → source navigation (optional in the brief) does not exist. The "Source output of the cable…" label is dead code. `midi_connected` is written in `Default` only and never read. The README does not claim cable navigation, so this is only dead code. | `grep "via_cable: true"`: no hits. `grep midi_connected`: only the declaration and the default. | Remove the dead code, or implement cable → source navigation with the existing label. |
| F10 | nit | `standalone/src/main.rs` | In the `kabl` binary, fatal failures (patch load failure → exit 1, default patch compile failure → exit 1) are logged at WARN. In `kabl-ui`, the same `--patch` load failure is ERROR. The brief lists WARN/ERROR together, so this is only an inconsistency. | Diff of `standalone/src/main.rs`. | Use ERROR for failures that end the process. |

Checked with no finding:

- **Tap placement.** The tap reads the module's own `output_bufs[port]` right after that step's `process()` and before any later step. `coalesce_buffers` only reuses slots after a buffer's last use, so this is before reuse. Feedback delay copies (`CopyToDelay`) and `SumVoices` are untouched. The tap is read-only. The engine test compares off/on renders bit for bit on three factory pieces.
- **Voice lanes and edges.** Voice lanes are resolved in `module_origin` order, and `lanes_total` beyond 16 is disclosed. Gate edges within a block and across window boundaries are counted per sample, because `prev` carries over across `reset_window`.
- **Graph identity.** Report generation equals the measured graph's generation. A failed compile still advances the generation and raises the floor. A reused id with another kind reports `NotInGraph`. The token changes on selection change and on editor replacement. Pending graphs that are replaced never measure.
- **RT path.** `set_probe` / `probe_block_end` do no allocation, formatting or logging. The `set_probe` pass is linear in the graph, and runs only on a target change or a fade start.
- **Comparison.** `compare::diff` covers every `PatchState` field (modules with kind, pos and params; cables with endpoints, params and steps; labels). Its final equality check makes a restore refuse rather than land somewhere else. `restore_to` is one `edit()` group, so the undo inverse restores the whole previous state. The next module and cable ids are raised above the restored ids. A restore replaces the redo tail, and the dialog says so.
- **Recipes.**
  - A recipe starts only after the ordinary unsaved-changes flow; cancel or failure drops the pending start with a note.
  - Opening another sound detaches the recipe and drops the reference. Close keeps the patch.
  - Targets are checked by id and kind. The factory `interlocking` wiring matches the recipe text (clock → seq 3; clock → div /2 → seq 4; lengths 7 and 5).
- **Logging.**
  - The level is INFO by default and set at runtime through `KABL_LOG` / `--log-level`.
  - Buffering is bounded: a 1024-line `sync_channel` with `try_send`, and drops are counted.
  - Rotation keeps 1 MiB × (1+3) files. Sink failures are counted, retried after 5 s and shown in the UI.
  - Shutdown waits at most 500 ms. No logger call is reachable from the engine or the audio callbacks.
  - The first line of `KABL_STATS_FILE` is unchanged.

## Limits of this review

- **Real app.** I did not run the real app, watch `walkthrough.mp4` or view the screenshots. Layout and focus, audible recipe outcomes, package launch and the D02 carried gaps are unverified by me and rest on the implementer's evidence.
- **Measurements.** I did not rerun the perf or logging scripts. The numbers in perf.md are unverified.
- **Build incident.** A release build of a throwaway timing program (for F7) ran the disk out of space. I then deleted the files in `target/release` newer than my scratch source file. Those should be only that build's partial outputs. I did not verify how much space was freed, because a follow-up `df` was refused by the permission classifier. The implementer's existing `target/release` artifacts should be intact, but if a release binary or example is missing, rebuild it (`cargo build --release …`). F7's cost remains unmeasured.
- **Scratch reproductions.** F1–F3 were reproduced with scratch programs that drive the public APIs directly: `Inspect`, `diagnose`, `PatchEngine`, `compile`. They were not reproduced in the GUI. F3's real-app behaviour depends on the frame rate during a drag, which I infer from `main.rs` (a rebuild on every dirty frame) and did not observe.
- **Beyond scope.** I did not audit cpal/ALSA thread behaviour beyond reading the `cpal::Error` type, and did not audit third-party crates' logging (`audio_thread_priority` logs at most one WARN from within the first callback on invalid input). The comparison `diff` was checked by reading and by the existing factory-sound test, not by randomized property tests.
- **No approval.** This review does not approve sound, feel, learning value or owner acceptance.

## Implementer responses

Fixes are in `f28d262` ("D03 review fixes F1-F10"); verification at that commit:
`cargo test --workspace` 518 passed, 0 failed, 15 ignored; `cargo clippy --workspace
--all-targets` clean.

| ID | Resolution |
|---|---|
| F1 | **Fixed.** `Inspect::rebuilt` drops kept reports older than the raised floor (and restarts "waiting"). Test `a_wiring_change_drops_what_was_measured_before_it`: report, cable removed, summary empty and status Waiting; a new-graph report is then the only one. |
| F2 | **Fixed.** `diagnose` reads the effective base (stored, else the registry default via `explain::param_of`) and counts non-bypassed routes into the param. "passes nothing" only when gain is 0 and neither cv nor a route drives it; otherwise worded as "opens only as far as its cv input or the routes into Gain move it". Cutoff uses the same helper. Test `why_no_sound_reads_defaults_and_routes_for_the_vca` (default-param VCA, route into gain, and the true case). |
| F3 | **Fixed.** `CompiledPatch::continue_probe_from` carries the open window (accumulators, `prev`, counts) into the graph fading in when the target resolves to the same lanes and port; the window is flagged `fading`. Engine test `a_knob_being_turned_does_not_stop_the_measurement`: swaps every 6/12/24 blocks for 2 s each, ≥35 reports each. Real app: `scripts/knob-drag.txt`, Filter #3 Cutoff dragged continuously with a note held while VCA #4 out is inspected (2 578 rebuilds in 66 s); the panel stayed live, labelled "measured on the edited version while it crossfades in" (`img/1440x900-light-knob-drag-live.png`). |
| F4 | **Fixed.** Reports carry `end_sample` (samples the engine had rendered); `main.rs` ages them against `CallbackTiming::frames_total` and `Inspect::accept(r, now, age)` stores the audio-clock time. A backlog read after a stall falls outside the 1 s summary. Test `a_report_that_waited_in_the_queue_is_not_live`. |
| F5 | **Fixed.** `kabl_standalone::hand_off_stream_error`: every error (xruns too) is moved into a 256-slot queue; when full it is `mem::forget`-ten (never freed on the audio thread) and counted (`errors_lost`, logged as "not delivered"). Xruns carry no message in cpal 0.18, so forgetting them leaks nothing; an owned message is leaked only when 256 errors arrive between two drains (UI: every frame; standalone: now every second). Documented in README. |
| F6 | **Fixed (partly as asked).** `crates/standalone/tests/rt_with_logging.rs` runs the engine with the tap on (swaps, selection changes), the report push into a full queue and the stream-error hand-off into a full queue with owned messages, under `assert_no_alloc`, with `applog` installed at TRACE. The kabl-ui callback closure itself is still not factored out; its remaining new work is relaxed atomic adds (histogram, `frames_total`), stated in the test's doc. |
| F7 | **Fixed.** `Inspect` caches `engine_output` per topology signature; the open aid compiles once per wiring change (`Context::output`). |
| F8 | **Fixed.** `LibError::log_text` (kind + system reason, no paths/names) is used for every library open/save failure line, including folder opens. README corrected: the start line, library directories, a failed `--patch` path and device names can appear. Test `library_errors_are_logged_without_paths_or_names`. |
| F9 | **Fixed** by removal (`Sel::via_cable`, its label, `UiState::midi_connected`). Cable → source navigation stays unimplemented (optional in the brief, not claimed). |
| F10 | **Fixed.** Process-ending failures in `kabl` log at ERROR. |

Evidence recorded before `f28d262` (walkthrough at `ef9ed7b`, screenshots/logging/perf/package at `ef9ed7b`–`db29445`) predates these fixes; only `img/1440x900-light-knob-drag-live.png` was taken at `f28d262`.


## Recheck

- **Recheck head:** `2967b06` (product fixes in `f28d262`). I reviewed `git diff 496a5c2..2967b06 -- crates docs/signal-inspection/{design,README}.md` and checked each resolution in the code.
- **Tests run at `2967b06`, debug target, in the cloud container:**
  - `cargo test -p kabl-engine --test probe`: 8 passed, including `a_knob_being_turned_does_not_stop_the_measurement`.
  - `cargo test -p kabl-ui --test signal_inspection`: 21 passed.
  - `cargo test -p kabl-standalone --test rt_with_logging`: 1 passed.
  - `cargo test --workspace`: exit 0, **518 passed, 0 failed, 15 ignored**.
  - `cargo clippy --workspace --all-targets`: exit 0, no warnings.
- **Scratch reproduction:** a throwaway program outside the repo, built into the existing debug `target/` with no release build. It drives `PatchEngine`, `compile` and `Inspect` directly. Its output is quoted under N1.

### Per-finding status

| ID | Status | Evidence |
|---|---|---|
| F1 | **Partly** | The UI side is fixed: `rebuilt` now drops the kept reports with `generation < floor` and restarts "waiting", and `a_wiring_change_drops_what_was_measured_before_it` covers that. The F3 fix brings the stale measurement back from the engine side, though, for the same removed-connection case. See N1. |
| F2 | Resolved | `base()` reads the stored value, falling back to the registry default through `explain::param_of`. `routes_into()` counts routes into the param that are not bypassed. "Passes nothing" is only said when the base is ≤ 0 and neither cv nor a route drives the param. Cutoff uses the same helpers. Covered by `why_no_sound_reads_defaults_and_routes_for_the_vca`. |
| F3 | Resolved (with N1) | `continue_probe_from` copies `acc`, `prev`, `samples` and `blocks`, and sets `fading`, but only when both taps are `found` and target (token included), lane count `n` and `port` are equal. So a changed selection, an editor replacement, a module that is gone or a different lane count restarts the window. `move_probe` continues from `active`, which is correct on both paths: on a direct `receive_swap`, `active` is the graph measured so far; on pending promotion, `active` is the just-promoted graph that was measured during the fade. Nothing allocates (fixed-size copy). The engine test gets ≥ 35 reports per 2 s with swaps every 6, 12 or 24 blocks. I did not look at the real-app run (`scripts/knob-drag.txt`, screenshot). |
| F4 | Resolved | `end_sample` is the engine's `rendered` count, advanced by `BLOCK` on every `process_block`. The UI ages each report against `frames_total`, which `record` adds after each callback. Both counters are created in the same `AudioHost::new`, so they share an origin. The engine renders at most one block ahead of what is delivered, so the difference saturates at 0 rather than going negative. The UI stores `now - age`. `a_report_that_waited_in_the_queue_is_not_live` covers `accept` with an age. The age computation in `main.rs` itself has no test. |
| F5 | Resolved | `hand_off_stream_error` pushes into a 256-slot queue. When the queue is full it calls `mem::forget` on the error and counts it, so nothing is freed on the audio thread. Both binaries use it. The standalone binary now drains the queue every 1 s. The README documents the bounded leak. See also N3. |
| F6 | Partly (accepted) | `rt_with_logging.rs` installs `applog` at TRACE, then runs engine swaps, selection changes, a report push into a full queue and a stream-error hand-off into a full queue with owned messages, all under `assert_no_alloc` (which also catches deallocation). The kabl-ui callback closure itself is still untested; its remaining additions are relaxed atomic adds, as the implementer states. |
| F7 | Resolved | `why_panel` caches `engine_output` per `topology()` signature. `output_module()` and every `CompileError` variant depend only on modules, kinds and endpoints, so the cache cannot go stale on a parameter edit. `topology()` still formats one string per cable each frame while the aid is open, which is cheap and has no RT impact. |
| F8 | Resolved | Every `doc` failure line uses `LibError::log_text`, which gives the kind plus the text after the last `": "`, i.e. the system's reason. No `doc` log line carries an id, name or path now; I grepped every `target: "doc"` line. The README is corrected for the lines that still carry paths (start, library directories, `--patch`, devices). Covered by `library_errors_are_logged_without_paths_or_names`. |
| F9 | Resolved | `via_cable`, its label and `midi_connected` are removed. There are no remaining references. |
| F10 | Resolved | Both process-ending paths in `standalone/src/main.rs` now use `log::error!`. |

### New findings

| ID | Severity | File / function | Concrete failure scenario | Supporting evidence | Requested correction |
|---|---|---|---|---|---|
| N1 | major | `engine/src/compile.rs` `continue_probe_from`, called from `patch_engine.rs` `move_probe` | The carry checks the target, lane count and port, but not whether the swap changed topology. When a cable into the inspected module is removed, the open window, with the old graph's peaks and RMS, continues into the new graph and is reported with the **new** generation. The UI raised its floor to that generation, so it accepts the report. `summary()` then shows the old peak as **Live** for 1 s (`SUMMARY_SECS`), while the new graph measures silence. This is F1's symptom again, and the Why panel reads the same summary. This happens on every topology edit made while inspecting, whatever point the window has reached. design.md is now self-contradictory: "reports already kept from before [the floor] are dropped" vs. "a window that spans a fade start continues". | Scratch program with `chain()` (macro 0.3 → VCA ×0.5 → VCA #3 ×0.5 → out), tapping VCA #3 out. Cable 2 (into VCA #3) is removed 5, 20 or 37 blocks into a window, the new graph gets gen 2, and `Inspect::rebuilt(2, …)` is called. Every case printed: first new report `gen 2 fading true peak 0.075 last 0.000`, `UI accepted true, summary peak 0.075 status Live`; second report peak 0.000; summary still 0.075 half a second later, cleared only after 1 s. | Carry only across swaps that keep the topology. For example, give `CompiledPatch` a topology signature computed in `compile` (or a `param_only` flag set by the builder) and require it to match in `continue_probe_from`. Otherwise restart the window as before. Add an engine test: tap on, remove the cable into the tapped module mid-window, and the first new-generation report must not contain the old level. A UI-level test that feeds real engine reports would also catch it. |
| N2 | nit | `ui/src/main.rs` `log_health`, `standalone/src/main.rs` loop | The count in "not delivered (queue full): N" is cumulative since start, but it sits on a line that reports "stream errors=… in 10s / in {secs}s". A reader will take it for a per-period count. | `get(&t.errors_lost)` and `lost.load(..)` are never reset or diffed. | Log the difference since the last line, or label it "since start". |
| N3 | nit (plausible) | `ui/src/main.rs` error callback → `hand_off_stream_error` | Xruns now go into the fault queue too (before this change only non-xrun errors did). If the UI does not run `update` for a while (e.g. a minimized window on a platform that stops repainting), a burst of 256 or more xruns fills the queue. A later real device error is then forgotten: its message is leaked, and it is never shown in `fault_text`, only counted. I have not observed this. | The diff queues every error in the kabl-ui callback. `update` filters xruns only after draining. | Keep counting xruns on the audio side (they are already counted in `t_err.error`) and queue only non-xrun errors, as the standalone binary could too. |

### Limits of this recheck

- I did not run the real app, look at `scripts/knob-drag.txt` or the new screenshot, or rerun the walkthrough, perf or logging evidence. As the implementer notes, that evidence predates `f28d262`.
- N1 was reproduced by calling the engine and `Inspect` APIs directly, not in the GUI. The GUI goes through the same `rebuilt`/`accept`/`summary` path, so I expect the same visible result, but I did not see it.
- F4's aging in `main.rs` was checked by reading only. So was the claim that the standalone `kabl` binary drains its queue every second.
- This recheck is not approval, and it is not Kosta's hands-on review.

## Implementer responses to the recheck

Fixes in the commit after `2967b06` ("D03 recheck fixes N1-N3"); `cargo test --workspace`
519 passed, 0 failed, 15 ignored; clippy clean.

| ID | Resolution |
|---|---|
| N1 | **Fixed.** `CompiledPatch` carries a `topology` signature (modules with kinds, cable ids with endpoints; computed in `compile`). `continue_probe_from` continues the window only when it is equal, so a wiring change restarts the measurement exactly when the UI raises its floor. Engine test `a_wiring_change_mid_window_starts_the_measurement_over`: the cable into the tapped VCA is removed 20 blocks into a window; the first report of the new graph has peak 0. design.md corrected. |
| N2 | **Fixed.** Both binaries log the "not delivered (queue full)" count for that interval (difference since the last line). |
| N3 | **Fixed.** `hand_off_stream_error` returns early for an xrun without a message (it owns nothing; callers count xruns themselves), so xruns never fill the queue ahead of a real device error. |

## Second recheck

- **Second recheck head:** `05734dd`. I reviewed `git diff 2967b06..05734dd -- crates docs/signal-inspection/design.md`.
- **Tests run at `05734dd`, debug target only:**
  - `cargo test -p kabl-engine --test probe`: 9 passed, including `a_wiring_change_mid_window_starts_the_measurement_over` and `a_knob_being_turned_does_not_stop_the_measurement`.
  - `cargo test --workspace`: **519 passed, 0 failed, 15 ignored**.
  - `cargo clippy --workspace --all-targets`: no warnings.
- **N1 reproduction rerun:** the same scratch program as in the recheck, unchanged, against this head.

| ID | Status | Evidence |
|---|---|---|
| N1 | Resolved | `compile_inner` sets `topology_of(patch)`, a hash of module ids with kinds plus cable ids with `from`/`to`. `continue_probe_from` now also requires `old.topology == self.topology`. These are the same ingredients the UI's `inspect::topology()` uses to raise its floor, so the engine restarts a window exactly when the UI starts rejecting older generations. The hash is computed on the compile (control) thread; the audio-thread check is one `u64` compare. The scratch rerun, with the cable into VCA #3 removed 5, 20 or 37 blocks into a window, printed for every case: `first new report gen 2 fading true peak 0.000`, `summary peak 0.000`. In the recheck it was 0.075 Live. The knob-drag test still passes, so param-only swaps still continue. design.md now matches. |
| N2 | Resolved | Both binaries log `errors_lost - lost_seen` and then update `lost_seen`, with the line reading "in that time". The kabl-ui baseline only moves when a WARN line is written. That cannot under-report, because a lost error implies a full queue, and a full queue means some errors were delivered, which triggers the line. |
| N3 | Resolved | `hand_off_stream_error` returns before pushing when the error is an xrun with no message. The returned error is dropped with nothing owned, so nothing is freed. Both callers count xruns before the call (kabl-ui in `t_err.error`, standalone in `xruns_cb`). An xrun that does carry a message still goes through the queue and is forgotten if the queue is full, which is correct. |
| F1 | **Resolved** | The UI part was already fixed at `2967b06`; the engine part (N1) is now fixed too. The removed-connection case shows no stale level as Live, as the N1 scratch run shows through `Inspect::rebuilt` / `accept` / `summary`. |

New findings: none.

Limits: I did not run the real app. N1 was verified through the engine and `Inspect` APIs, not the GUI. Only removing a cable was exercised; adding a cable and changing a module's kind go through the same signature check but were not run. This is not approval, and it is not Kosta's hands-on review.

## D03-R1 review

- **Reviewer:** a fresh reviewer subagent (Claude). It had no part in the D03-R1 correction and changed no product code; this section is its only edit.
- **Reviewed base:** `08f9d06` (PR #5 head the brief was prepared against).
- **Reviewed head:** `68f6cee` (branch HEAD). Product commits `25a9f78` (hand-off), `99c28d3` (`probe_cost` example), `f1edf70` (comments only; I checked that `git diff 99c28d3 f1edf70 -- crates` has no non-comment line). `git diff f1edf70 68f6cee -- crates Cargo.toml Cargo.lock packaging` is empty, so the product code at the head equals `f1edf70`.
- **Contract checked against:** `docs/product-research/briefs/D03-R1.md`, and the original RT clause in `briefs/D03.md` (line 155: "No logger calls, text formatting, file I/O, locks or allocation/deallocation in the new RT path"; acceptance row "No-allocation/deallocation checks on changed callback paths").

### Coverage

- **Code, read directly:**
  - `crates/standalone/src/lib.rs`: `hand_off_stream_error`, `StreamErrorCounts`, `STREAM_ERROR_KINDS`, `STREAM_ERROR_QUEUE`.
  - `crates/standalone/src/main.rs`: the error callback and the 1 s drain / 10 s log loop.
  - `crates/ui/src/main.rs`: the error callback, `CallbackTiming::error`, `AudioHost` field order, `log_health` (drain plus WARN line) and where `update` calls it.
  - Tests: `crates/standalone/tests/stream_error_overflow.rs` and `rt_with_logging.rs`.
  - Diff: `crates/engine/examples/probe_cost.rs`.
- **Pinned backend source**, `~/.cargo/registry/src/*/cpal-0.18.2`:
  - `src/error.rs`: `Error`, `ErrorKind`, `From<AudioThreadPriorityError>`, `ResultExt::context`.
  - `src/host/alsa/mod.rs`: `new_output`/`new_input`, `output_stream_worker`/`input_stream_worker`, `poll_for_period`, `try_resume`, `process_output`/`process_input`, `From<alsa::Error>`, `Drop for Stream`.
  - cpal's `[features]` in its `Cargo.toml`, `cargo tree -e features -i cpal` and `Cargo.lock`.
- **Docs:**
  - design.md, "Stream-error ownership";
  - REPORT.md, "D03-R1 follow-up", plus the older acceptance matrix, Verification and Commits sections;
  - README.md diff, HANDOFF/STATUS diff;
  - evidence/perf.md, last section.
- **Evidence opened:**
  - `r1/perf-app-summary.txt` (every row checked against the perf.md table), `r1/perf-busy-1-stall-kabl.log`, `r1/probe-cost-composition.txt`;
  - `r1/logging/{default,debug,blocked,badload}.txt`, `r1/r1-sink.stderr`, `r1/package.txt`;
  - `r1/r1-app-drive.log` and `r1/r1-app-kabl-debug.log` (4 593 `compiled generation` lines; the drag runs from 160.6 s to 252.0 s of drive time), `r1/r1-app-stats.txt`, `r1/test-workspace.txt`;
  - `scripts/r1-app.sh`, `r1-default-vca.txt`, `r1-sink.txt`;
  - screenshots `r1/img/1440x900-{2-mid-drag,4-just-removed,5-new-silence,7-undone,9-default-vca-why,10-log-sink-failure}.png`;
  - `ffprobe`: `r1/r1-app.mp4` is 117.8 s.
- **Commands run at `68f6cee`** (debug profile, cloud container):
  - `cargo test -p kabl-standalone`: exit 0. All suites pass, including `stream_error_overflow` 1/1 and `rt_with_logging` 1/1.
  - `cargo clippy --workspace --all-targets`: exit 0, 0 warnings.
  - `cargo test --workspace`: exit 0, **520 passed, 0 failed, 15 ignored** (summed over every `test result` line). This matches the implementer's 3c8872e totals.

### Findings

| ID | Severity | File / function | Concrete failure scenario | Evidence | Requested correction |
|---|---|---|---|---|---|
| R1-1 | minor | design.md "Stream-error ownership"; README "Logging"/limits; REPORT "D03-R1 follow-up"; HANDOFF; `lib.rs` `hand_off_stream_error` doc; `stream_error_overflow.rs` module doc | The docs say that on this build cpal allocates **`RealtimeDenied`** messages on the audio thread "once, when the worker starts". Kabl does not enable cpal's `realtime` feature, so that code is compiled out. On this build, cpal never produces `RealtimeDenied`, and **`BackendError` is the only kind with a heap message** on the error-callback path. The "real-time priority refused" lines in every R2 log come from kabl-ui's own `audio_thread_priority` call in the *data* callback. They are not a cpal stream error. A reader of the contract decision will overstate what the backend does. | `cpal-0.18.2/Cargo.toml`: `default = []`, `realtime = ["dep:audio_thread_priority", …]`. `cargo tree -e features -i cpal` shows only `cpal feature "default"`, and Cargo.lock's `cpal` entry has no `audio_thread_priority`. `alsa/mod.rs` lines 916/973 and `error.rs`'s `From<AudioThreadPriorityError>` are `#[cfg(feature = "realtime")]`. `crates/ui/src/main.rs` ~L421 promotes the thread itself. | State that on this build only `BackendError` (from `From<alsa::Error>` for an unmapped errno) owns a heap message. `RealtimeDenied` would too if cpal's `realtime` feature were enabled. Keep the provenance-based clause, which already covers both. |
| R1-2 | minor | REPORT "D03-R1 follow-up" R2 table; perf.md last section; README limits; commit `3c8872e` message | The R2 evidence contains **a second stream stall that is not disclosed**. The release sink-failure logging run (`r1/logging/blocked.txt`, commit 99c28d3) logs `stalled: no callbacks for over 1.5 s after 2428`. Its recording then ends `take complete frames=0`. The REPORT, perf.md, README and STATUS all say that *one* busy perf run hit the stall. The brief asks for the observed stall to be labelled honestly. As written, a reader concludes the stall happened once in about 11 app runs, but it happened twice. Logging and sink-failure behaviour are still shown correctly by that run. | `r1/logging/blocked.txt` lines 20 and 31. `grep -n "2428\|stall"` over REPORT/README/perf.md finds only the busy-1 case. | Add the blocked-logging stall to the R2 table and the README/STATUS limit ("two of the R2 app runs stalled: perf busy-1 at 24 s, logging-blocked after callback 2428; the recorder there captured 0 frames"). No rerun is needed. |
| R1-3 | minor | REPORT.md acceptance matrix, "RT safety" row (and the "Logging" row) | The main acceptance matrix still reads **"RT safety: no alloc on changed paths … pass (automated)"**, citing `rt_with_logging.rs` "full … error queues". At `25a9f78` the stream-error overflow path intentionally **deallocates** an owned message on the audio thread, pending a supervisor/owner decision. `rt_with_logging` was changed to a static-message error, so it no longer exercises that path. The matrix therefore reports an unconditional pass for a clause that the correction itself proposes to relax. The Logging row still cites only the historical `evidence/logging-*` files. The brief asks the matrix and identities to be updated. | REPORT.md acceptance matrix rows (unchanged by `git diff 08f9d06..HEAD -- REPORT.md`, which only edits the header and appends the follow-up). The `rt_with_logging.rs` diff uses `"Device disconnected"` (a static `&str`). | Mark the RT row "pass, except the stream-error overflow free (contract adjustment pending; `stream_error_overflow.rs`)". Point the Logging row at `r1/logging/*` too. Or add one sentence above the matrix saying the follow-up table supersedes the matching rows. |
| R1-4 | minor | design.md "Contract adjustment"; `lib.rs` `hand_off_stream_error` doc ("Never allocates, formats, logs, locks or blocks"); REPORT | The adjusted clause is not fully precise, for two reasons. (a) The overflow drop calls the global allocator's `free`. glibc's `free` is usually served from the per-thread tcache, but it can take the arena mutex (tcache bin full, or a larger block). So "never … locks" is not guaranteed on the overflow path. (b) The "original clause" is quoted as "the error callback never frees on the audio thread". The D03 brief actually says "no logger calls, text formatting, file I/O, locks or allocation/deallocation in the new RT path". The adjustment should name the clause it amends. It is fair to note that cpal's own `malloc` of the same message, moments earlier on the same thread, has the same lock exposure. | `D03.md` L155. `lib.rs` doc comment on `hand_off_stream_error` and design.md "Policy (R1)" bullet: "The hand-off itself never allocates, formats, logs, locks or blocks". | Quote the brief's clause. Word the exemption as "may deallocate (through the global allocator, which may briefly lock as cpal's own allocation of it may) exactly one message that cpal allocated on this thread in the same call, and only when the hand-off queue is full". Qualify "never locks" in the doc comment accordingly. |
| R1-5 | nit | `crates/ui/src/main.rs` `AudioHost` field order; design.md clause "only while the hand-off queue is full" | At teardown, `AudioHost` drops `faults_rx` (declared first) before `_stream`. `Stream::drop` then joins `cpal_alsa_out`, and that thread drops the error closure, which holds the last `Producer`. So the rtrb buffer and up to 16 queued errors, with any owned messages, are freed **on the audio worker thread** as it exits. This is outside the "queue full" case the clause names. It is harmless for audio, because no more periods are processed. The shutdown step in `stream_error_overflow.rs` drops both ends on one test thread, so it does not model this ordering. | `crates/ui/src/main.rs` L62 (`faults_rx`) before L67 (`_stream`); cpal `new_output` moves `error_callback` into the spawned closure. `Drop for Stream` joins it. | Either declare `_stream` before `faults_rx`, so the consumer outlives the stream and drops the leftovers on the UI thread, or add "and at stream teardown, after the last period" to the clause. |
| R1-6 | nit | REPORT "D03-R1 follow-up" header / package row; `r1/package.txt` | The package smoke test's binary reports `commit b298eb65381d+modified`, which means it was built from a working tree with modified tracked files. The REPORT gives the evidence build as "release 99c28d3" and doesn't mention this identity. No crate file changes after `f1edf70` in any commit, so the product code is very likely the same. But a "+modified" package is not an exact identity. | `r1/package.txt` line 24. `crates/standalone/build.rs` marks "+modified" when `git status --porcelain --untracked-files=no` is non-empty. | Record the package's identity (b298eb6 plus which files were modified, presumably docs), or rebuild the package from a clean tree and refresh `r1/package.txt`. |
| R1-7 | nit | `standalone/src/main.rs` loop; `ui/src/main.rs` `log_health` | "last delivered" can belong to another interval. The hand-off counts before it pushes, and the consumers drain before they call `since`. So an error counted just after a drain is reported in interval *k*'s kinds, while its text arrives in interval *k+1*. If *k+1* has no new errors, the text is held and shown later next to unrelated counts. The counts themselves are exact. | Order of `fetch_add` then `push` in `hand_off_stream_error`, and drain then `since` in both callers. | Cosmetic. Either call `since` before draining, or clear the text when an interval has no errors. |

Checked with no finding:

- **Bound.** Every non-bare-xrun error is either in the 16-slot rtrb queue or dropped in the same call. Nothing is forgotten (`mem::forget` is gone from the code; `grep` finds it only in design.md's history). So outstanding error ownership is at most `STREAM_ERROR_QUEUE` = 16 errors, each with at most one heap block, whatever the consumer does. This holds for a paused consumer, for kabl-ui with no UI frames (drain only in `log_health` ← `update`, so the queue just stays full), and for `kabl`'s 1 s drain. Both binaries size the queue from `kabl_standalone::STREAM_ERROR_QUEUE` and call the same function.
- **Counts.**
  - `STREAM_ERROR_KINDS` lists all 14 variants of cpal 0.18.2's `#[non_exhaustive] ErrorKind`, so "unknown" (index 14) is unreachable today and exists only for the future.
  - `seen` has `len()+2` = 16 entries: indices 0–14 for kinds, 15 for undelivered. The indexing is correct.
  - Every undelivered error is also counted by kind, so the WARN line (printed only when kinds is non-empty) can never hide a non-zero undelivered count.
  - A bare `Xrun` returns before counting, and both callers count xruns themselves.
  - An `Xrun` with a message (cpal's `try_resume` static "Device does not support suspend/resume") is queued, and it is counted both as a stream error (`Xrun=`) and in the xrun counters. That is consistent and owns nothing.
  - The subtractions can't underflow because the counters are monotonic.
- **Where the callback runs.** Confirmed: `new_output`/`new_input` spawn `cpal_alsa_out`/`_in` and move the error closure into it. `output_stream_worker` calls it between periods, on the same thread as the data callback. Every error on that path is built on that thread just before the call. `ResultExt::context` (which would `format!` for any kind) is not used on the worker path.
- **Is the incompatibility real?** Yes. cpal allocates a fresh `String` per `BackendError`, and a persistent unmapped ALSA errno can repeat it every poll. With no consumer progress, a total bound requires that some of these be freed. The only thread that holds them and still runs is the audio thread. Every alternative the brief excludes (a larger queue, a deferred-free list, blocking, forgetting) or that I considered (a fixed "graveyard" slot, a capped count of forgets) either only moves the limit or eventually frees on that thread. A dedicated drain thread in kabl-ui, instead of the UI frame, would make a full queue much rarer: today it fills whenever the UI stops running frames. But it would not remove the need for the clause. Avoiding the allocation would need a patched cpal, which the brief does not ask for. The provenance-based clause is the smallest adjustment, subject to R1-4 and R1-5.
- **Test.** `stream_error_overflow.rs` checks bounded outstanding ownership directly: a process-wide live-block counter stays ≤ 16 after every one of 1000 overflowing hand-offs, there are 0 allocations and exactly 1 free per overflowing call (per-thread counters), and exact per-kind and undelivered counts, plus the drain contents and order, recovery and shutdown. It checks both the bound and ownership, not RSS alone or a no-alloc assertion alone. "Unknown" can't be constructed, so it is untested.
- **No leftover "bounded leak" claim.** README, design, REPORT, HANDOFF and STATUS now describe a drop, not a leak. `mem::forget` appears only as the rejected D03 behaviour. I found no universal "never frees on the audio thread" claim left, apart from R1-3's matrix row and R1-4's "never locks".
- **R2 artifacts checked:**
  - **Drag, removal, undo** (`r1-app`, release `99c28d3` = product `f1edf70` behaviour):
    - mid-drag the reading stays live, "measured on the edited version while it crossfades in";
    - just after removing the cable into VCA #4 `in`, the panel shows a new 0.6 s window below −90 dBFS with 0 lanes active, not the old level;
    - 1.5 s later it shows a full 1.0 s window of silence;
    - after undo it shows −3.7 dBFS with 1 lane active, and "Why no sound?" gives the correct gain-0/cv explanation;
    - about 4 600 compile lines in the DEBUG log.
  - **Default-gain VCA:** gain 1.00 with no cv, peak −0.6 dBFS, 8 lanes active, and no "passes nothing" wording.
  - **Logging:** INFO default (no DEBUG lines), DEBUG override, a bad `--patch` exits 1 at ERROR, and the sink-failure toolbar text and Log ⚠ are shown.
  - **Re-measurement:**
    - `probe_cost` has off/on edits+topology modes: every 10th swap removes the first module-to-module cable, and the next swap restores it. That changes the topology signature and restarts the window.
    - I checked the perf.md tables against the raw `probe-cost-*.txt` and `perf-app-summary.txt`, and they match.
    - Execution, arrival and xruns are kept apart.
    - The historical sections are labelled as such.
  - **Package:** run outside the checkout with the checkout's patches hidden. The recipes are found and the log goes to `~/.local/state/kabl/logs`.

### Verdict on R1

**Bounded: yes.** Outstanding stream-error ownership is at most 16 errors in all states I examined: a paused consumer, kabl-ui with no frames, `kabl`'s drain, and shutdown. The regression test verifies this directly. The `mem::forget` escape is gone. The incompatibility between a total bound and never deallocating on the audio thread is real for unmodified cpal 0.18.2 ALSA. The provenance-based adjustment is the smallest one that works.

The adjustment is **precise in substance but needs three wording fixes before the supervisor/owner decides**:
- name the right owned kinds (R1-1): on this build only `BackendError`;
- quote the actual original clause and disclose the allocator-lock exposure of the free (R1-4);
- cover or remove the teardown free (R1-5).

None of these changes the bound or the behaviour. The adjustment itself still needs a supervisor/owner decision. This review does not approve it.

### Verdict on R2

**Supported, with disclosure gaps.** The artifacts in `r1/` and the last perf.md section support the R2 table:
- the real-app drag, cable removal, new silence and undo;
- the default-gain VCA;
- INFO, DEBUG and sink failure;
- a re-measurement that includes the topology change;
- a package smoke test;
- historical evidence labelled as historical.

Three gaps:
- the second observed stream stall is undisclosed (R1-2);
- the acceptance matrix's RT and Logging rows were not updated (R1-3);
- the package's `+modified` build identity is unrecorded (R1-6).

These are documentation corrections. None needs a rerun.

### Limits of this review

- I did not run the real app, the perf harnesses, the logging runs or the package. I read their artifacts and six screenshots. I did not watch `r1-app.mp4`; I only checked its length.
- No device failure was injected. The ALSA error paths were checked by reading cpal's source, not by running them.
- The glibc `free` lock point (R1-4) comes from knowledge of glibc's allocator design, not from a trace on this machine.
- I did not check whether eframe stops calling `update` for a minimized window on the target desktop. That affects how often kabl-ui's queue fills, not the bound.
- Tests were run in the debug profile only.
- This is not approval, and it is not Kosta's hands-on review.

## D03-R1 implementer responses

Fixes are in `d21f42b` (code and comments) and the docs commit that follows it.
`cargo test --workspace` at `d21f42b`: 520 passed, 0 failed, 15 ignored; clippy is clean
(`r1/test-workspace.txt`, `r1/clippy.txt`). A real-app start/quit on the release build of
`d21f42b` logged `shutdown result=ok` (`r1/quit-d21f42b.log`).

| ID | Resolution |
|---|---|
| R1-1 | **Fixed.** Confirmed with `cargo tree -e features -i cpal`: only cpal's `default` feature, which is empty, is enabled; `realtime` is off. design.md, README, REPORT, HANDOFF and the comments in `lib.rs`/the test now say that `BackendError` is the only owned message on this build. `RealtimeDenied` is described as owning a message only with that feature, and the "RT priority refused" line is attributed to kabl-ui's own promotion call. |
| R1-2 | **Fixed.** The second stall (the logging "blocked" run, after callback 2428, with a 0-frame recording) is now reported in the REPORT R2 table, perf.md, README limits and STATUS. |
| R1-3 | **Fixed.** The main acceptance table's RT-safety row now names the exception under the proposed adjustment. The Logging row cites the final-head evidence in `r1/logging` and marks the older files as historical. |
| R1-4 | **Fixed.** design.md now quotes the D03 brief's clause word for word ("No logger calls, text formatting, file I/O, locks or allocation/deallocation in the new RT path…"). The proposed adjustment adds that the overflow free may take the allocator's internal lock. The claim "never locks" is now "takes no lock of its own" (design.md and the `lib.rs` doc comment). |
| R1-5 | **Fixed in code.** `AudioHost` declares `_stream` first, so the stream and the producer in its error callback drop before `faults_rx`. Queued errors are then freed on the UI thread, not on the exiting audio thread. The standalone `kabl` runs until killed (it never tears down), which design.md records. |
| R1-6 | **Fixed.** REPORT records that the package was built from `b298eb6` with an uncommitted docs-only edit (`+modified`), and that its product code is `f1edf70`'s. |
| R1-7 | **Documented.** design.md says the "last delivered" message can come from an earlier interval than the counts beside it. The code is unchanged: the brief asks for classification and counts, and the counts are exact. |

The evidence builds (99c28d3, and b298eb6+modified for the package) predate R1-5's
teardown order. None of the recorded flows exercises teardown with queued errors, so no
evidence was rerun, as the reviewer noted.
