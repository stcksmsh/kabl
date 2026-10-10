# Perform view (task B): handover

Branch `claude/perform`, stacked on `claude/widget-set` (PR #18). Worktree `.worktrees/widget-set` (same one). Draft PR base: `claude/widget-set`. Do not add to PR #18. Report to "Kabl overseer", do not merge, then stop.

## Task as received (condensed)
Redesign Perform on the widget kit: A's tokens, B's layout and value arcs (scenes under `media/b-lacquer/*perform*`, colour under `a-satin/`). Build: transport (large tempo, bar/beat, Run/Restart); scenes (large pads with playing, queued, idle, progress, Cancel); macros (kit arc knob recipe reading theme slots, value, names of what each moves, MIDI tag); sequences (bank launchers, live step lanes only if data exists); every other control keeps working on a kit widget (record + folder, MIDI in, All notes off, pins, direction, transpose, level, learn + soft-takeover, Taller/Shorter). Rack|Perform view switch in the toolbar if it does not lose rack+Perform together, else keep the toggle and ask Kosta with captures. Constraints: behaviour unchanged, keep `ui_state.record` keys, fit the plugin's fixed 1180x680 and 1440x900 / 1280x800, light and dark; check Composition, Echo Sequence and a plain keyboard sound. Also fix the app_target clippy errors (separate commit). Verify: workspace tests + strict clippy totals, captures (both sizes, themes, plugin), frame times with Perform open on a dense patch.

## Done
- `kit/recipes.rs`: `arc_knob`, `pad`, `step_lane` (read Style only). `Ic::More`.
- `perform_view.rs`: the view (header, transport, scenes, macros, banks + lanes, control cards). Old drawing code removed from `perform.rs`; `record::controls` now on the kit.
- Toolbar: Rack|Perform segmented switch (keys `rack`, `perform`). Perform opens the full view (`perform_tall` = full view, rack hidden); "Show rack" in the header docks it (old behaviour, `PANEL_H` 340). "Taller/Shorter" became "Full view"/"Show rack" (key `perform-size`).
- Commits: Perform view; app_target clippy fixes (separate).
- Tests: only `the_demo_controls_fit_together_at_1280` changed (opens the full view; dropped the rack-height line that encoded the docked strip).

## Facts
- Not available without engine work: bar/beat position and the time to the launch point (no clock phase reaches the UI), so no bar/beat readout and no pad progress bar (the `progress` arg of `kit::pad` is ready). Step lanes are live: `seq_steps` plus the bank's v/r/g params.
- Pin keys unchanged: `pcard:*`, `pslider:*`, `plabel*`, `pmenu`, `pexplain`, `plearn`, `pclear`, `ppickup`, `prun`, `prestart`, `pbank*`, `pcue*`, `midi-input`, `notes-off`, `learn-cancel`, `rec-*`.
- Gestures: `continuing()` in `perform_view.rs` keeps one undo step per press for sliders and knobs.
- Tooling: capture scripts in the job tmp dir (`cap/perf.sh`, `run1.sh`, `host/start.sh` for REAPER). Never `pkill`; touch `host/run/quit`.

## Remaining
1. Captures: Composition, Echo Sequence, Init Keyboard at 1440x900 and 1280x800, light and dark, plus plugin (REAPER, 1180x680). Look at each, fix.
2. Frame times: `bench_chrome` with Perform open on `patches/composition` (add a Perform-open case to the bench).
3. Full workspace tests + strict clippy totals; rustfmt touched files.
4. Update this file, push, draft PR (base claude/widget-set), one report.

## Open decisions for Kosta
- Default of the Perform switch: full view (built) or docked under the rack.
- Bar/beat and pad progress need a clock-phase report from the engine to the UI.
