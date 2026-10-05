# Kosta's D08 owner checklist

## D08 strict CLAP repair — 2026-10-05

Engineering complete; strict-profile repair submitted for owner review. Owner review pending. PR #11 remains draft and unmerged. No D09/D10.

R6 is repaired for the declared Kabl strict CLAP profile. Checked exclusive plugin
ownership, immutable activation snapshots, scalar tail publication and bounded SPSC
handoffs replace the reachable callback locks, retrying queues and generic state
rendezvous. State load publishes complete recalled controls without writing native
caches; serialized callbacks own those writes. Pending getters/editor/state observations
retain canonical recall until cache synchronization. Native IDs, v1/v2 state, graph
retirement, automation/flush ordering and Host/Free behavior are preserved.

Starting submitted investigation: `790f0c40a479bdaa55753bc8d63b7dec3729784b`.
Tested/reviewed/packaged product: `f7157619836075d0d7704e59be5d66e6224890b1`. Binary SHA256: `3baa8ee3cc0f2f99e1823e0a741d7301713098f99e92dc84f7556b8d0483aca4`.
Final submitted documentation head is recorded in the bundle's submission.json after
normal push; it changes no product source. Prior product `133fdb89ac56a396db0c7c961c74067782b34efb`
and its original evidence/bundles remain retained with that provenance.

Final checks: workspace612 passed/0 failed/22 ignored; plugin25; framework primitives4;
strict workspace/all-target Clippy, release and validator35 success/9 skips pass.
Actual REAPER Write/replay, closed-editor virtual CC/buttons, full fresh-process recall,
transport and GUI/plugin destruction/recreation are refreshed. All18 native values and
both complete states survive recreation. Two fresh unwrapped production processes each
retain three full36-second renders: first is warm-up, second/third match across processes
with PCM SHA256 `c535792b70919e23b5f8b1f25dd92333f397b6bcbb5e9c3ccb2e5bb1c7e2cb7b`.
Three ordinary renders still differ and remain retained. Additional source-free packaged
fresh recall and policy renders pass, with verified bundle mapping/cwd and unavailable
factory path; packaged warm-up is also retained.

Evidence: [strict-profile index](evidence/strict-profile/index.json),
[callback audit](evidence/strict-profile/callback-audit.md),
[timing summary](evidence/strict-profile/timing-summary.json).
Owner bundle: `/home/stcksmsh/.codex/visualizations/2026/10/04/01a10707-3ccc-73d2-b3af-ee17bddde50b/d08-strict-owner-test`; run `./launch.sh`.
Full raw archive: sibling `d08-strict-evidence`. Previous archives and failures remain intact.

Limits: valid CLAP host lifecycle contracts; off-audio producer backlog can grow when a
host permanently rejects delivery; no exhaustive DSP/host/machine-code proof. Dense two
open editors still show backend ERR=2; worst measured individual process5491.27µs exceeds
the5333.33µs JACK period. Physical xruns and round-trip latency are unmeasured. Reported
plugin latency remains64frames (1.333ms at48kHz). Physical controller, listening, feel,
latency and production-beta approval remain Kosta's checks. Ordinary-render nondeterminism
and dense-editor errors are not resolved by this repair.

Maintenance: retained ISC snapshot at upstream263b16877d0b0ad30921868338a6df46eadc9a46,
original/final source manifests, reproducible patch and upgrade audit checklist in
[vendor/nice-plug/STRICT-PROFILE.md](../../vendor/nice-plug/STRICT-PROFILE.md).
Repeat ownership/ring/state/editor audit and full regression/host evidence on upstream
changes. No Cargo cache mutation, merge, global plugin install or other-chat messages.

Owner review: **pending**. Engineering submission does not approve sound, feel or production beta.

Bundle: `/home/stcksmsh/.codex/visualizations/2026/10/04/01a10707-3ccc-73d2-b3af-ee17bddde50b/d08-strict-owner-test`. Close other test REAPER instances, then run `./launch.sh`. It opens an isolated profile and embedded two-track project. Keep edits in a copy. Prerequisites: Linuxx86_64, REAPER7.75 and PipeWire JACK. No source/build/factory files are required.

- [ ] Listen to media/arrangement.wav and play both voices. Record sound/release/tail problems.
- [ ] Read/Write/Touch: move a mapped rack knob or Automation slots value, record, stop, switch Read and replay.
- [ ] Close floating and chain editors; replay recorded envelopes. Connect your physical controller through REAPER, test pickup + one mapped button.
- [ ] Reorder panels, save, fully quit REAPER, relaunch the saved copy. Verify mapping identities, gain, Host/Free and both complete patches.
- [ ] Delete a mapped target. Verify deleted lane stays unassigned through Undo. Explicitly reassign only when intended.
- [ ] Host: play/stop, change tempo, seek and loop. Expect synchronized pulse phase and restart policy described in design.md. Check4/4 launches. Free should follow document BPM independently.
- [ ] Run ./render-policy.sh twice in fresh processes. Retain each first warm-up; compare completed second/third PCM with scripts/compare-renders.py.
- [ ] Check light/dark rack fitting at1440×900 and1280×800 on your desktop scaling.
- [ ] Measure physical round-trip latency and controller feel.64frames reported plugin latency is not the measured total.
- [ ] Evaluate dense two-instance open-editor load at your intended buffer. Dummy-backend errors were observed; no hard real-time guarantee.
- [ ] Review the engineering R6 disposition; declared strict-profile ownership/handoff correction has engineering evidence; this is not a hardware approval.
- [ ] Record production-beta disposition: accepted / changes requested / not accepted, date and notes.

Optional global install: close host; preserve any existing ~/.clap/kabl.clap; copy bundle/plugins/kabl.clap only into an unused target. Optional uninstall: close host, remove only that installed file. Isolated removal: close bundle REAPER and remove this bundle directory. No global changes were made during D08 verification.

Owner notes:
