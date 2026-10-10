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

## Final state (task complete, report sent)
- Verification at the final head: `cargo test --workspace` 730 passed, 0 failed, 28 ignored; `cargo clippy --workspace --all-targets -- -D warnings` clean (the app_target errors are fixed in their own commit). New tests: the toolbar switch and Show rack; a macro knob edits the real parameter with one undo step.
- Captures: `media/perform/` (Composition, Echo Sequence, Init Keyboard at 1440x900 and 1280x800, light and dark; one docked capture) and `media/perform/plugin/` (REAPER, 1180x680 editor, light, dark, docked).
- Frame times (release `bench_chrome`, median us, Routing drawer open, Perform open on `patches/composition`): full view 1440x900 756 light / 650 dark, 1280x800 647 / 637; docked under the rack 1440x900 3547 / 3416, 1280x800 3183 / 3095. Dense rack alone for reference: 5459 / 5949 and 2981 / 3014.
- Where the old controls went: record, folder and MIDI in stay in the view header (kit field, buttons, dropdown); All notes off and Learn banner (with Cancel learn) in the header; Taller/Shorter is now Full view / Show rack; pin menu (Rename, Move left, Move right, Unpin) behind the ... button on each card; Explain behind ?; per-pin MIDI learn is the CC tag button (Learn / CC n . ch), Clear beside it, soft-takeover shown as "live" or an arrow with the target percentage; direction, transpose and other selectors are segmented controls, levels and the rest are kit sliders.
- Not built (needs engine work): bar and beat readout, pad progress to the launch point.

## Open decisions for Kosta
- Default of the Perform switch: full view (built) or docked under the rack.
- Bar/beat and pad progress need a clock-phase report from the engine to the UI.
