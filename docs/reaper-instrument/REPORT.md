# D07 implementation checkpoint

Engineering: implementing. Owner review: pending. D08 is not authorized.

Base: `91777165cadba5ce81a1d7fdbc4de78563f95ca4`, fetched origin/master on 2026-09-30.
Branch: `codex/d07-reaper-instrument`. Worktree: `/tmp/kabl-d07`.
User checkout remains on `c41420c`; its untracked `.ai/` is untouched.
Authorized brief: [BRIEF.md](BRIEF.md). User requested regular committed checkpoints.

## Completed

Read workflow, current handoff/status/decisions, D07/D08 plan, D06 design/report/review,
prototype report/review/checklist and source, D04/D05 contracts, production timeline/control/
patch engine/editor/serialization interfaces and pinned framework state/lifecycle hooks.
No implementation or acceptance claims yet.

## Environment

Actual laptop: ThinkBook16, Linux `7.0.0-34-generic`, x86_64. Default Rust 1.93.0;
1.98.1 also installed. X11 `DISPLAY=:1`, runtime `/run/user/1000`.
REAPER `/usr/local/bin/reaper`; exact version/audio/hardware pending measurement.

## Integration decision in progress

Keep pinned nice-plug for CLAP lifecycle, MIDI conversion and egui embedding if real-host
vertical slice succeeds. Its upstream loader applies params before persistent fields and
reactivates on state loads. Production needs a separate bounded transactional state extension;
the prototype shim is insufficient. Advertise MIDI only (D06 has no CLAP note-id contract).
Use production Timeline (64-frame latency, no standalone callback-period delay).

## Next action

Implement production `crates/clap` vertical slice: embedded rack/browser, coherent persistent
host control, transactional complete sound state and editor-independent collector. Test in
scratch REAPER profile on existing PipeWire setup. Then finish acceptance, fresh independent
review/fixes/recheck and draft/ready PR according to actual evidence. Commit/push increments.

## 2026-10-01 recovery checkpoint

The initial `/tmp/kabl-d07` and scratch host directory disappeared across the interruption.
Only the documentation checkpoint `85a0db8` had been committed/pushed; the uncommitted source
was recovered exactly from this chat's tool-call records. The worktree now lives persistently
at `/secondary/Programming/Github/kabl/.worktrees/d07`. Do not use `/tmp` for resumable source.

Recovered implementation: production `crates/clap`, pinned nice-plug wrapper, transactional
complete state, independent control/collector worker, MIDI-only note port, production Timeline,
engine DSP reset. Prior five targeted tests passed and debug cdylib built; rerunning now, then
commit the source. Host acceptance was not established: initial scratch scan opened a Wine
bridge desktop and never produced the host-info script output. Preserve this as failed evidence,
not successful scan/embedding. New scratch profile disables unrelated VST scanning.

Remaining: strict Clippy/workspace/regression tests, atomic/callback/lifecycle review, real GUI
vertical slice, all acceptance/demos/measurements, fresh independent review and final recheck,
packaging and PR. No engineering completion claimed. `.ai/` remains untouched.

## 2026-10-01 review fixes checkpoint

Product checkpoint before these fixes: `b2fef9e`. Independent reviewer found eight substantive
issues; final acceptance and reviewer recheck remain pending. Fixed metadata-only recall,
complete editor seeding (cable parameters, patterns and labels), state identity overflow,
and consistent editor/load resource bounds. Failed editor documents remain visible but never
replace the accepted persistent sound; subsequent worker ticks cannot silently accept them.
Checks: seven production CLAP unit tests pass; two focused UI seeding regressions pass.
Restored preexisting `.gitignore` rules and added only `/scratch/`.

Persistent host scratch: `.worktrees/d07/scratch/host`; REAPER 7.75/linux-x86_64,
PipeWire/JACK negotiated 48 kHz / 128 frames. Fresh scratch-profile insertion succeeds.
Host gain readback currently reports 1.0 despite the script requesting 0.4: unresolved.
Do not claim host-control acceptance yet. Actual embedded rack observed; fresh final-build
screenshots and full recall/live/render evidence remain required.

Remaining review fixes: accurate effect tails, runtime UI feedback, collector final cleanup,
and bounded raw CLAP MIDI before the framework's event queue. Then rebuild and refresh host
artifacts, validator, full workspace checks, demos/package, independent final recheck and PR.

## Review fixes: callback, feedback, tails and shutdown

Previous head: `b993a95`. Eleven production unit tests pass; strict plugin Clippy passes.
New exported-entry test drives the actual CLAP wrapper with 4,097 raw MIDI events, including
an unseen release, then reset/deactivation/reactivation at 96 kHz. Every process/reset is
allocation/deallocation guarded. Intake reads at most 2,048 events, holds 1,024 MIDI events
and forwards at most 128 parameter events. Overflow schedules Host source loss; D06 retains
full host offsets across parameter subblocks. The outer CLAP object delegates lifecycle and
GUI using unchanged framework plugin_data, without a global map lock on audio.

Runtime clock/sequence/LFO/delay/probe reports use a fixed queue; the control worker updates
rack feedback with the editor closed. Probe age uses the actual engine sample clock. State
replacement discards reports from the prior session. Collector storage now calls try_cleanup
only after queued values and Handles drop. Targeted cleanup and closed-editor feedback tests
pass. The finite-tail policy recognizes MIDI-envelope-gated output routes and sums conservative
release/effect settling times; other routes advertise KeepAlive with an explicit free-sound
reason. FullAdsr release is a time constant, so its settling bound uses ten release periods.
A 30-second reverb no longer receives an arbitrary eight-second tail.

These fixes need independent recheck and new real-host evidence. Earlier debug plugin remains
loaded in scratch REAPER; close that task-created scratch profile before replacing its binary.
Host control readback, final workspace/validator/lifecycle evidence, demonstrations, packaging
and PR remain outstanding. Owner acceptance remains pending.

## Follow-up independent recheck fixes

Reviewed head: `94a22bf5c4675a859f4f708ccdc41ebe81e4b902`. Reviewer resolved R1/R4–R8;
R2/R3 and new R9/R10 are detailed in REVIEW.md. Follow-up code/tests fix all four scenarios.
Eleven plugin tests and 21 standalone runtime-control tests pass; strict plugin Clippy passes.
Standalone intentionally lets existing runtime controls operate after a bad structural edit;
that contract is preserved. The plugin additionally compiles recovery candidates before
accepting their complete state. Gain base remains persistent; host monophonic modulation now
reaches audio. Saturated automation preserves the final value/modulation independently.
Release build is running; scratch desktop restarted on DISPLAY=:100 after prior processes
ended across interruption. Persistent profile/projects remain available. Host and package
acceptance are still incomplete; no final reviewer or owner approval claimed.

## Real-host acceptance checkpoint

Current product before the sample-rate change: `043708d`. The rate guard now covers
1,000..768,000 Hz (finite only), including the official validator's unusual fractional rates;
tails larger than CLAP's finite u32 representation use KeepAlive rather than truncation.
The first validator run rejected 384 kHz and is preserved as validator-first-*. The refreshed
release passes 35 tests, with nine explicit skips and no failures/crashes. Raw JSON/stderr are
retained. Full workspace tests and strict workspace Clippy pass (final-head rerun still due).

Actual scratch REAPER: output gain set/readback 0.40000000596; real embedded rack operated.
Saved actual knob edit to cutoff 20 kHz and cable replug from left output to right. Opened the
factory Expressive Lead through the browser, confirming the rack's unsaved-change dialog
only after the edited snapshot had been saved. The two-instance project contains distinct
complete states: 6 modules/6 cables/gain 0.4 and 15 modules/30 cables/gain 0.55. Full process
quit/restart with KABL_FACTORY_DIR pointing to a nonexistent scratch path preserved both
states exactly, including topology. Ten complete editor open/close cycles during playback
retain gain and playing state; final editor closed. Virtual keyboard recording through
REAPER's input queue records five notes and six CC/bend/cleanup events. No physical controller
was present; listening/feel acceptance remains Kosta's.

Scratch artifacts: `scratch/host/{two-instances,reopened,after-cycles,virtual-recording}.rpp`,
actual rack/browser screenshots, operation-1.mp4 and editor-cycles.mp4. Offline musical render
is 52 s at 48 kHz stereo float32; peak 0.138675, activity at -60 dB RMS 0.5..30.5 s and at
-90 dB 0.5..32.2 s. Last-second RMS 2.25e-22. The longer ending is deliberate tail settling
verification. A 36 s listening copy and 34.6 s REAPER-node-only live capture are trimmed and
scaled uniformly by 6.423 (+16.15 dB); original raw files remain retained. Screen videos have
no audio; offline and live audio are separate and clearly labeled.

The first repeated render differs: preserved render-repeat.json, not a reproducibility pass.
Testing an explicit host fresh-instance reset (offline/online) before each render, with no
concurrent project mutation until a completion marker. Remaining: reproducibility diagnosis,
measurements, viewport/theme evidence, package outside source/relocation launch, final tests,
independent final source/evidence recheck, documentation/handoff/PR. No completion claimed.

## 2026-10-03 continuation state

Product source and pushed branch head: `c9541bcf9caaa377767caf6907c29ebbd694369e`.
Reviewer source recheck at that exact head is recorded in REVIEW.md. No new product source
has been edited since. One pending reproduction script, render-fresh-second.lua, is saved
with this checkpoint. First explicit fresh-instance render completed in 3.276793095 s and
produced scratch/host/d07-fresh.wav. Second fresh render has no completion marker/file
verification; reproducibility remains unresolved. Earlier nonidentical repeated renders are
preserved, not relabeled as passing. Proposed performance instrumentation was not implemented.

Next action: inspect scratch REAPER state and finish the controlled second fresh render,
compare PCM and resolve/document reset behavior. Then measure light/dense one/two instances
with editors open/closed, capture second viewport/theme evidence, prepare and verify an
outside-checkout installable bundle/relocated project, finish README/design/CHECKLIST, run
final-head checks, obtain final independent acceptance/package recheck, update HANDOFF/STATUS/
decisions and submit PR. No PR exists yet; no merge or D08 work authorized. Owner listening,
physical-controller feel and acceptance remain pending.

## 2026-10-04 closeout: draft submission, acceptance incomplete

Starting continuation head: `0af399f` (original prerequisite base `91777165`). Final product
and tested/reviewed source: `ff11434c6502d2cce5a3894544bec10f5ceea6d6`, on
`codex/d07-reaper-instrument`. Product increments: `7d111f4` satisfies strict Rust 1.93
Clippy for asynchronous browser save; `ff11434` fixes actual embedded toolbar overlap by
placing Save/Save As and Perform/Routing on a separate row. No D08 changes or merge.

Engineering is submitted as a draft with incomplete acceptance. Owner review remains
pending. The user's time-limited closeout request stops further diagnosis in this run;
the following gaps must not be relabeled as completion.

| Criterion | Result | Evidence/provenance |
|---|---|---|
| Final workspace checks | Pass: 597 passed, 0 failed, 22 ignored; strict Clippy and release build exit 0 | evidence/final-{summary.json,workspace-tests.txt,workspace-clippy.txt,release-build.txt}; ff11434 |
| Final CLAP validator | 35 success, 9 explicit skips, no failures/crashes | evidence/validator-final{.json,-stderr.txt}; ff11434 |
| Complete recall, independent gains, browser/routing edits | Pass in prior actual host; final relocated states exactly equal | host/reopen logs; evidence/package-final-state-comparison.json |
| Outside-checkout final binary/package | Pass: missing factory path, gains 0.4/0.55, 52-second stereo render, saved complete states | evidence/package-final-{check.txt,manifest.json,state-comparison.json} |
| Virtual MIDI and editor cycles | Virtual recording and ten lifecycle transitions/readback pass | Existing c9541bc host logs/videos; no physical controller; no uninterrupted-audio claim from transport flags alone |
| Repeated render | Fail/unresolved: fresh-process and offline/online policies differ | evidence/render-{fresh,process}-comparison.json; earlier failure preserved |
| Performance | Eight 12-second real-host cases at negotiated 48 kHz/256; editor-open requested, not handle-verified | evidence/host-timing.json; product 7d111f4; DSP unchanged at ff11434, final toolbar not benchmarked |
| Backend/device | Cumulative device error 26 stable during the sampled light-case window; REAPER node error 0 | evidence/pipewire-measurement-window.txt; does not cover every case or physical latency |
| Real knob Undo | Actual cutoff 20 kHz → 1.26 kHz → Undo 20 kHz observed | scratch/host/undo-1280-{edited,restored}.png; 7d111f4 widgets unchanged |
| Owner sound/controller/feel | Pending | CHECKLIST.md |

Measurements exclude instances with fewer than 1,000 callbacks from the steady-case summary
(startup/shutdown rows remain raw). Per-instance means are 0.124–0.162 ms for light sounds,
0.346–0.426 ms for dense sounds; worst observed callback across all cases is 2.265132 ms.
The 48 kHz/256 buffer interval is 5.333333 ms. These are wall-clock callback durations with
clock/atomic overhead, not physical latency or proof of long-run stability. No historical
freeze cause was inferred. Measurement proxy never replaces the packaged product.

Actual environment: ThinkBook16, i7-13700H (20 logical CPUs), Linux 7.0.0-34-generic,
Rust 1.93.0, REAPER 7.75/linux-x86_64, PipeWire/JACK negotiated 48 kHz/256 during new
measurements. Previous musical/live proofs used 48 kHz/128. Raw renders and failed runs
remain in scratch; listening copies and silent operation videos remain clearly labeled.

Fresh reviewer `/root/final_review` rechecked ff11434, reported no new substantive source
finding and independently ran 24 plugin/UI library tests successfully. REVIEW.md records
its acceptance limitations. Later final package/validator refresh changes no product source.

Persistent owner bundle:
`/home/stcksmsh/.codex/visualizations/2026/10/03/01a10304-4ddb-7850-bdcf-4509411a23d5/d07-owner-test`.
Launch `./launch.sh`; no global install was performed. See README.md and CHECKLIST.md.
The complete two-instance MIDI project, offline/live listening copies and silent operation
videos are present. The final packaged binary manifest names ff11434. Earlier /tmp test
bundle is disposable and is not the durable handoff. `.ai/` and user checkout remain untouched.

Next action: diagnose render reset/pre-roll timing with a confirmed complete render and
bounded callback telemetry, then establish a repeatable policy. Refresh editor-open timing
with actual GUI-handle verification and complete remaining owner/controller acceptance.
The timing proxy's optional trace run did not produce a verified complete render and is not
used as successful diagnosis. Stop at D07; no merge or D08 authorization.

Final toolbar visually verified at 1280×800 in A-light/A-dark: evidence/screenshots/final-1280-{light,dark}.png. Save/Save As and Perform/Routing no longer overlap. Earlier 1440×900 evidence predates this toolbar change; a refreshed final-head 1440 view remains pending.

Submitted draft PR: https://github.com/stcksmsh/kabl/pull/10. Initial submitted evidence head: `894d83592721bae1a1415f82166109d489b19517`; subsequent PR-link commit is documentation only. Tested/reviewed product stays `ff11434c6502d2cce5a3894544bec10f5ceea6d6`. Scratch REAPER was closed after final evidence.

## 2026-10-04 bounded diagnostic and measurement refresh

Product remains `ff11434c6502d2cce5a3894544bec10f5ceea6d6`; initial submitted head was
`dd8fa7f70040ba0df3d1d2018cd377cefeeab733`. Base remains merged master
`91777165cadba5ce81a1d7fdbc4de78563f95ca4`. This continuation changes evidence/scripts
and documentation only. Engineering remains incomplete; PR remains open draft and unmerged.
Owner acceptance remains pending. No D08 work or `.ai` changes.

### Render diagnosis: failure retained

All valid runs use the identical saved `scratch/host/after-cycles.rpp` and final binary;
SHA256 provenance is in `evidence/render-diagnostic/provenance.json`. The new runner verifies
synchronous render return, atomic completion marker, clean host exit and exact decoded
52-second/48-kHz/stereo float32 payload. A marker publication race invalidated fresh-1;
its WAV/log remain retained and it is excluded from passing completion claims.
Fresh-2/3 complete but differ in 4,914,929 samples (maximum absolute difference 0.18606399).
Serial-1/2 additionally close the audio device, recreate instances and disable track buffering
and anticipative FX; both complete but differ in 4,915,249 samples (maximum 0.2427147552371025).
Device-closed alone has one completed exploratory run, not a repeatability result.

Fixed-capacity callback telemetry observes reset/start positions and first MIDI note offsets.
The pinned framework resets DSP during start_processing. For serial runs, the second start
occurs at cumulative frame 8960 vs 4352. This establishes differing reset/pre-roll sequences,
not that timing alone explains the PCM. The framework requests restart when render mode
changes; a diagnostic proxy suppressing that forwarding still observes second starts.
That hypothesis is insufficient; no product render-mode override was introduced.

Declared policies fresh-process and serial-offline both FAIL repeatability. No repeatable host
policy is established. Preserving free-running DSP while finding a reliable host reset boundary
remains the next engineering investigation. Resetting phase at transport/MIDI boundaries would
change product behavior and needs a precise separate decision; it is not silently adopted here.
Raw failed PCM now also resides durably in `d07-closeout-evidence` beside the owner bundle;
`evidence/render-diagnostic/durable-wavs.json` indexes absolute paths and hashes.

### Refreshed real-host measurement

Eight final-product light/dense × one/two instances × open/closed cases complete with exit0.
Playback durations are 12.001–12.030 seconds. Negotiation is 48 kHz/256 frames/JACK.
Actual floating GUI handles are verified at start, ~6 seconds and end; two-open cases have
separate handles. Inactive/closed instances have nil handles. Selected light/dense graph is
copied into both tracks with unique FX IDs; measured-project.rpp is retained per case.
Raw callbacks/device/GUI/PipeWire files and summary live in `evidence/host-refresh/`,
with the complete runner output in `evidence/host-timing-refresh.json`.

Callback totals span instance lifetime including startup/teardown, not precisely the playback
window. Active rows have 2280–2408 callbacks; arrival spans are 12.128–12.661 seconds.
Rows below1000 remain raw and are excluded from active-instance summaries. Light mean
execution is 0.122831–0.159417 ms; dense 0.323895–0.461315 ms. Worst execution is
3.602444 ms vs nominal buffer interval 5.333333 ms. Maximum callback arrival interval is
64.188876 ms; arrival gaps include lifecycle transitions and cannot be called execution xruns.
Proxy calibration: 100000 noop calls, added67.471 ns/call on this run. Execution timing
includes its clock edge; lookup/arrival bookkeeping/atomics contribute separate proxy overhead.
Calibration is one local short microbenchmark, not a precision hardware latency measurement.

Per-case PipeWire snapshots cover the whole run. Device counter goes 0 while inactive to1
on activation, then remains1 in active snapshots; REAPER remains0. Raw first/last/delta are
retained separately. No assertion of xrun-free physical playback, uninterrupted cycle audio,
physical latency or long-duration stability follows from these short observations.

### Final views, package and checks

Final 1440×900 A-light/A-dark: `evidence/screenshots/final-1440-{light,dark}.png`.
The same ff11434 packaged binary is used; final1280 views remain valid. No visual redesign.
Product binary, saved state and package contents are unchanged; no bundle rebuild is needed.
Fresh independent manifest recheck validates all21 files and binary equality with target:
SHA256 `998020860c7cb6f027fa74462ed4cbd0afa02d52167ebe40fdc6111e36d21bd4`.
Complete relocated recall evidence remains valid for unchanged product (6/6/gain0.4 and
15/30/gain0.55, unavailable factory directory, outside-source render).
Prior final workspace597 passed/0 failed/22 ignored, strict Clippy/release and validator35
success/9 skips retain exact product provenance. New Python scripts compile; strict C11
`-Wall -Wextra -Werror` proxy build and `git diff --check` pass. No full suite repeated because
product source did not change. Independent review dispositions are in REVIEW.md.

Next action: resolve repeat-render acceptance on D07, then obtain Kosta's listening,
physical-controller/latency/feel and hands-on host acceptance. Submission is not completion.
