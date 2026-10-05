# Kosta's D08 owner checklist

## R6 engineering prerequisite — 2026-10-05

The investigation did not repair R6. Read [RT-OWNERSHIP](RT-OWNERSHIP.md) before interpreting the retained build/host results as real-time acceptance. A strict CLAP framework profile or dedicated wrapper must correct ownership, fallback locks, state rendezvous and retrying handoffs together. This is an unresolved engineering requirement, not an owner checkbox that can prove locks absent. Updated record bundle: sibling d08-r6-owner-test; old owner bundle remains intact. All hands-on checks below remain pending.

Owner review: **pending**. Engineering submission does not approve sound, feel or production beta.

Bundle: `/home/stcksmsh/.codex/visualizations/2026/10/04/01a10707-3ccc-73d2-b3af-ee17bddde50b/d08-owner-test`. Close other test REAPER instances, then run `./launch.sh`. It opens an isolated profile and embedded two-track project. Keep edits in a copy. Prerequisites: Linuxx86_64, REAPER7.75 and PipeWire JACK. No source/build/factory files are required.

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
- [ ] Review the engineering R6 disposition; whole-callback no-lock acceptance requires a verified ownership/handoff correction.
- [ ] Record production-beta disposition: accepted / changes requested / not accepted, date and notes.

Optional global install: close host; preserve any existing ~/.clap/kabl.clap; copy bundle/plugins/kabl.clap only into an unused target. Optional uninstall: close host, remove only that installed file. Isolated removal: close bundle REAPER and remove this bundle directory. No global changes were made during D08 verification.

Owner notes:
