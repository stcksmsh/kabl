# D08 context handover — 2026-10-04

## Resume here

User requested context checkpoint, then continuation after clearing chat. D08 is authorized and **implementing**, not accepted or submitted. Owner approval pending. No PR, merge, D09, or messages to other chats. Continue autonomously within original authorization.

- Branch: `codex/d08-host-production`.
- Worktree: `/home/stcksmsh/Programming/Github/kabl/.worktrees/d08` (physical `/secondary/Programming/Github/kabl/.worktrees/d08`). Always set this working directory; root checkout is not implementation checkout.
- Exact base: `c6bb8ccdab50dfe91a2b1f002d8c2766351697fc` (D07 merged PR #10).
- Current tested product head: `2bf509d03d4e9f86934a3aa0c52e697703f75be6`. Following checkpoint commit changes documentation/scripts/evidence only. Final submitted/tested/reviewed/packaged heads remain to be established.
- Previous source increment: `86a3fa2241350b47988aca59ef0d82202f9a792f`; scope commit `8f38e8a`.
- Original complete user request: `/home/stcksmsh/.codex/attachments/37be9c83-c4e8-4926-a5c7-1d352061927a/Pasted text.txt`. Read it, `BRIEF.md`, this file, then relevant workflow/code. User requested caveman ultra + ponytail ultra; read their skills if new context.
- Root user untracked `.ai/`, `.worktrees/`, `docs/OVERSEER-HANDOFF.md`, `docs/OVERSEER-LEDGER.md` preserved. No `.ai/state.json`; no AIW initialization. Other D06/D07 worktrees preserved.

## Implemented source

`automation.rs`: 16 stable normalized native slots; target identity = module ID/kind/parameter, pins/macros prioritized, only continuous runtime parameters. Deleted targets retire lanes; layout/undo do not silently retarget. Explicit rack mapping. `output_gain` native identity preserved; slot IDs `slot_1`…`slot_16`; final Bool `host_clock` defaults Free. REAPER native indexes: gain 0, slots 1–16, Host 17, host-added Bypass/Wet/Delta 18–20.

Version-2 `sound_state.rs` envelope persists mappings, slot values and clock mode. Version-1 missing additions loads Free/unassigned. Validation rejects invalid/duplicate mappings; complete session/bank prepared off callback and atomically swapped. Fixed bank ring retries when full. Host automation overlays DSP without editing patch history. Rack setters emit native gestures; CC worker forwards values/gestures with editor closed. Actual host gesture/CC recording remains unverified.

`schedule.rs`: fixed 2048-event storage, raw transport/slot events merged, 64-frame execution grid (up to 63 samples quantization). Host requires valid tempo + beats + seconds. Missing transport stops Host clocks. Play/seek reanchors timeline, resets DSP seeds; stop releases Host MIDI/cancels launches while tails run. Free behavior retained. Quarter-note host clock follows host beat/tempo; 4/4 bar launch remains initial policy. Runtime overlays use existing ramps across active/incoming/pending engines.

Latest source wraps CLAP start_processing: distinguish nice-plug implicit Plugin.reset on each thread start from explicit CLAP reset. Skip implicit reset only during already-playing Host epoch. Explicit reset/activation still reset. Pinned nice-plug source proves start invokes reset. Ordinary render repeat failure persists; do not claim this fixed it or is final validated design.

## Verification and failures

- Plugin unit tests: 14 passed, 0 failed, including state migration/identity/deletion, atomic rejection, Host/Free/missing transport and partition-independent explicit schedules under allocation guards.
- Strict plugin Clippy passed on latest source; release build passed. Logs: `evidence/start-policy-tests.txt`, `clippy-start-policy.txt`, `build-start-policy.txt`.
- Earlier workspace tests all passed (`workspace-tests-first.txt`); rerun final product later. Initial failing development logs retained, not acceptance results.
- Real REAPER 7.75 inserted two instances and saved version-2 arrangement. Synthetic played MIDI + host sequenced patch. PipeWire/JACK observed 48kHz/256. No physical controller claim.
- Completed ordinary 36-second stereo 48kHz float32 renders differ before and after latest reset change. See `render-first-comparison.json`, `render-start-policy-comparison.json`. Preserve failures.
- Provisional serial policy: audio device closed, both tracks I_PERFFLAGS=3 (no anticipation), discard one full warm-up render per fresh process. Renders 12/13/15/16 match exactly, PCM SHA256 `fcf577142b2f11660409a0ba21289c61f4be5466e508a19a770fb760d63e0b11`. Warm-ups 11/14 excluded explicitly. `render-policy-confirmation.json` records match; `render-serial-comparison.json` includes warm-up and fails. This is diagnostic-proxy evidence, not final unwrapped/head acceptance.
- Transport telemetry flags 15 stopped/31 playing include needed fields; Host parameter 1.0. Host song-time jumps during pre-roll; early missing-fields theory disproven. Saved actual tempo markers 120@0, 140@8, 100@16.5714 (host snapped requested16). Proxy process tempo remains120, native transport updates empty: actual tempo-follow acceptance unresolved.
- Latest `transport.lua` driver did NOT exercise playback reliably: time=0 after requested tempo/play, immediate Free readback remained1.0. Do not infer host bug or mark play/seek/loop/Free pass. Fix real action/wait/readback first.

## Durable raw artifacts and tools

All current `scratch/host` copied to `/home/stcksmsh/.codex/visualizations/2026/10/04/01a10707-3ccc-73d2-b3af-ee17bddde50b/d08-checkpoint/host`; hashes in `/home/stcksmsh/.codex/visualizations/2026/10/04/01a10707-3ccc-73d2-b3af-ee17bddde50b/d08-checkpoint/manifest.json`. Includes all failed and matching full WAVs, actual saved projects/profile, proxy telemetry, screenshots, logs and release binary. This is checkpoint evidence, **not owner portable bundle**. Original scratch also remains in worktree; scratch/proxy holds external diagnostic proxy.

Scripts in `scripts/`: make-project.py, prepare-host.lua, render.lua, repeat.lua, serial-repeat.lua, diagnostic.lua, transport.lua, run-host.py, compare-renders.py, timing-proxy.c. Read script interfaces before running. `run-host.py` owns Xvfb :110, metacity and isolated REAPER lifecycle; no persistent owned runner/display remained at checkpoint. Use escalation for X11 sockets; standalone long-lived exec sessions lost displays before, so keep ownership integrated. Build external proxy with `cc -O2 -shared -fPIC ... -ldl`; never package proxy as production plugin. It flushes fixed telemetry on instance destruction. Proxy .csv/.trace/.transport may contain appended runs; preserve provenance before fresh captures.

Git metadata needs sandbox escalation for commit/push; prior operations approved. Normal pushes only. Do not change user desktop/device settings outside isolated profile. Native GUI MCP disabled; shell-driven isolated GUI works.

## Next actions, ordered

1. Repair real host transport test driver (start audio/playback, await host state, await parameter readback). Verify actual play/stop/seek/loop, tempo map changes and missing transport, time signatures/launch/MIDI/tails. Inspect current telemetry without assuming tempo support passed.
2. Resolve acceptance gaps: mapped live automation readout, coherent Host clock rack controls (currently Run/Restart enabled but scheduler overrides), CC/buttons/editor-closed host recording/replay, undo independence and gesture lifecycle.
3. Validate deterministic declared render policy on exact final unwrapped production head in fresh processes. Retain ordinary failures + discarded warm-ups. Verify musical sections/tails; no oscillator-per-note resets, no Free changes.
4. Actual host automation write/read, save/full restart recall, multi-track piece; portable source-free relocated profile with unavailable factory path; curated 1440×900 and1280×800 light/dark screenshots; light/dense one/two-instance measurements with actual editor handles.
5. Finish README/design/REPORT/REVIEW/CHECKLIST, raw evidence provenance, curated media, owner bundle, root HANDOFF/STATUS/decisions. Focused commits + normal push.
6. Final appropriate checks. Fresh independent reviewer subagent explicitly authorized **at end**, exact base/head/evidence, resolve findings and independent final recheck. None spawned yet. Create draft PR and attach artifact. No merge or D09; owner approval remains pending.

## Specific code risks to inspect

- Raw transport/slot producer overflow errors ignored; schedule dropped counter not surfaced.
- GUI gesture enqueued before play-position reanchor can be cleared.
- CC output begin/end push failures not independently retried; host recording not verified.
- State validation reserves only1024 envelope bytes, potentially insufficient for16 mappings near2MiB. Preserve valid v1 compatibility when fixing.
- Fixed sorted event insertion bounded but up to2048 shifts; dense performance unmeasured.
- Pinned framework already takes process/reset Mutex; claim only no added callback locks, not whole framework lock-free.
- Start-reset distinction needs final necessity/behavior review; matching serial policy alone does not prove ordinary behavior fixed.
