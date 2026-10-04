You are implementing Kosta’s authorized D07 batch for `stcksmsh/kabl`, locally on his Linux laptop through remote control.

**Outcome:** insert kabl as a CLAP instrument in REAPER, play and record MIDI, edit its actual rack, save the project, reopen it with the same sound, and render while its editor is closed.

Complete implementation, local host verification, demonstrations, independent reviewer-subagent review, fixes and final recheck. Proceed through internal steps without approval pauses. Stop after D07.

## Starting state and reading

Fetch current `origin/master`. Expected prerequisite: D06 merged through `91777165cadba5ce81a1d7fdbc4de78563f95ca4`. Preserve any newer changes.

Inspect git status/remotes and preserve unrelated edits. Work on a dedicated D07 branch, using an isolated worktree if needed. Do not reset or clean the user’s checkout, force-push or merge.

Read, in order:

1. Applicable `AGENTS.md` and `docs/product-research/AGENT-WORKFLOW.md`.
2. Active entries in `docs/HANDOFF.md`, `docs/STATUS.md` and relevant decisions.
3. D07/D08 in `docs/PRODUCT-PLAN.md`.
4. `docs/midi-timing/{design,REPORT,REVIEW}.md`.
5. `docs/reaper-clap-prototype/{REPORT,REVIEW,CHECKLIST}.md` and the actual prototype.
6. Relevant D04/D05 runtime-control and lifecycle contracts.
7. Production engine, editor, state serialization and CLAP integration code.

This prompt authorizes D07 and supersedes older “do not start D07” instructions. Prior owner checks remain pending unless Kosta explicitly reports acceptance.

Record the actual starting commit, toolchains, installed REAPER version, desktop session, audio configuration, hardware and negotiated sample rate/buffer. Discover the working local setup; use the existing PipeWire/JACK configuration where appropriate.

## Build the production instrument

Reuse kabl’s production engine, D06 timeline, patch model, browser and actual egui rack/editor. Implement production integration in the appropriate workspace crate; the isolated prototype is a reference and regression fixture.

The prototype already proved scanning and offline output, but did not establish usable editor embedding, changed-instance recall or live playback. It also has these known gaps:

- State loading can partly apply parameters before rejecting the patch under queue saturation.
- Retired graphs are not guaranteed to be collected while the editor stays closed.
- MIDI mapping discards source/channel/identity and bypasses D06 timing.
- Host parameter changes did not reliably read back during the REAPER proof.
- Editor operation and repeated host lifecycle were not demonstrated.

Close those gaps where required for D07.

Start with a small real-host vertical slice: actual editor, production timeline, one persistent editable control, project recall and editor reopen. Then complete the full acceptance scope. This is an internal increment, not an approval gate.

Evaluate the pinned nice-plug prototype against the lifecycle/state/GUI requirements. Keep it if suitable; fix or replace the integration when concrete evidence requires it. Use existing research and official documentation to answer specific gaps. Pin dependencies, check licenses, and record the decision. Do not silently ship the prototype’s state shim as a complete production state contract.

## Ownership, timing and controls

REAPER owns audio devices, MIDI delivery and host lifecycle. The plugin must not start CPAL/midir backends, a device watchdog or standalone retry machinery.

Use D06’s production timeline for arbitrary host buffers and sample-offset MIDI. Preserve modulation grids, feedback delay and note-release ownership. Advertise only supported note dialects; do not advertise CLAP-native note support while discarding its required identity.

Support the MIDI expression delivered by D06: bend, modulation wheel, channel routing, sustain and cleanup. Define deactivation/reset/reconfiguration behavior so stopped, replaced or reactivated instances cannot replay held notes unexpectedly.

Report plugin latency accurately. Do not carry standalone’s live-input scheduling delay into host processing accidentally. Define tail reporting and render-end behavior; avoid truncating effects or claiming an infinite tail without reason.

Controls and audio must work with the editor closed. Host and rack changes must converge on one coherent sound state. Demonstrate at least one persistent host control with a stable identity and correct readback; do not expand into D08’s full automation-slot system.

Compilation, file access and retired-graph destruction belong outside the audio callback. Guarantee bounded collection and orderly shutdown even without editor frames. Preserve normal callback allocation/deallocation and lock constraints.

## Complete and atomic project state

Embed the complete sound state and necessary dependencies in the REAPER project. A source patch path is not a sufficient state representation.

Define a versioned, bounded state format. Validate and prepare changes under the host’s permitted lifecycle/thread rules. State acceptance must be atomic: rejection preserves the previous complete sound and parameters. Test malformed/truncated/oversized state, invalid graphs, queue saturation and repeated state changes.

Saved state must represent the audible settings when the editor is closed. Loading must update host parameter values and the rack consistently. Keep transient held keys, sustain state, pending commands and session machinery out of persistent sound state unless explicitly justified.

Standalone and plugin must represent the same patch sound. Preserve existing standalone save/load, undo, runtime controls and recovery.

## Actual laptop and REAPER acceptance

Use the installed REAPER and real desktop/audio setup. Temporary REAPER profiles and scratch projects are encouraged. Preserve existing projects, presets and configuration. Ordinary app operation, reversible builds and demos are authorized; ask before privileged installation or global configuration changes.

Verify and record:

- Fresh scan and insertion of the built production CLAP.
- Live MIDI playback and recording.
- Two instances with different patches/settings, independent controls and correct recall.
- Browser patch loading, knob edits, structural routing edits and undo.
- At least ten editor open/close cycles; audio and controls continue while closed.
- Save, quit REAPER completely, restart and reopen.
- Project relocation with original patch directories unavailable, using a temporary copy rather than modifying user assets.
- Changed controls and topology surviving recall, rather than two identical default instances.
- Offline rendering with editors closed, including release/effect tails.
- Repeated renders under a documented seed/reset policy.
- Supported sample-rate/buffer changes, deactivation/reactivation and instance deletion.
- Correct latency, MIDI timing and channel expression.
- Installed plugin/package operation outside the source checkout.

Run CLAP validator against the production plugin and retain raw results, including skips. Add targeted tests for atomic state rejection, callback bounds, lifecycle ownership and the D06 adapter. Rerun workspace tests and strict Clippy on the final product head.

At 48 kHz/256 where supported, measure light and dense patches, one and two instances, editors open and closed. Distinguish plugin execution time, backend xruns and host/device latency. Do not infer the cause of historical stalls from an unrelated successful run.

If the previous dense-patch freeze recurs, capture diagnostics and determine whether host audio, plugin processing or GUI progress stopped. Do not discard failed runs.

## Demonstrations and owner test package

Create `docs/reaper-instrument/` containing:

- `README.md`, `design.md`, `REPORT.md`, `REVIEW.md`, `CHECKLIST.md`.
- Reproduction scripts, validator results, focused logs and exact build/host versions.
- A portable REAPER project with two independently edited kabl instances.
- A 30–60-second musical render demonstrating sustained notes, bend/wheel, chords and tails. Measure active duration and explain intentional silence.
- A short recording of actual REAPER operation showing the real embedded editor, editing, close/reopen and project recall.
- Necessary screenshots at both viewport sizes/themes where changed UI requires verification.
- An installable CLAP build/test bundle outside the checkout, with simple install, launch and uninstall instructions.

Label offline renders, live app audio, scripted MIDI, physical-controller tests and owner observations accurately. Do not substitute offline audio for a live demonstration without labeling it.

Send useful WAV/video/project files through the available interface. Otherwise give exact local paths and simple commands.

Prepare a short hands-on checklist for Kosta. Once the concrete package is ready, request listening/controller observations while continuing independent work. If hardware is absent, use clearly labeled virtual MIDI and leave physical checks pending. Do not approve sound or feel on Kosta’s behalf.

## Review, commits and completion

Commit small coherent increments frequently and push normally. Checkpoint progress before context limits in `REPORT.md`: exact head, completed behavior, results, remaining failures, artifact paths and next action. Leave `.ai/` untouched and format only changed files.

Use a fresh independent reviewer subagent. Supply this prompt, repository instructions, exact base/head, source/diff and evidence. Require scrutiny of:

- Host lifecycle, thread ownership and editor-independent processing.
- Atomic state loading and accurate project recall.
- D06 timing, MIDI identity, expression and latency.
- Graph collection, queue saturation and shutdown.
- Callback bounds and standalone regressions.
- Whether actual-host evidence supports the acceptance claims.

Fix findings, rerun affected checks and obtain its final recheck against the actual final product head. Refresh affected evidence after fixes or justify reused artifacts against unchanged behavior. Independent review unavailable means an explicit remaining gap and draft PR.

Update HANDOFF, STATUS and decisions accurately. Return exact starting/tested/reviewed/submitted commits, branch/PR, acceptance matrix, verification results, reviewer recheck, measurements, demo paths/links, owner checklist and limitations.

D08 owns the full host automation bank and Host/Free transport synchronization. Record required seams without implementing those features now.

Submit D07 for review. Do not merge or start D08.