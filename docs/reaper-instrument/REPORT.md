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
