# Wavetable import and editor: handover

Branch `claude/wavetable-editor` (from origin/master), worktree `.worktrees/sound-engines-2`, draft PR #30. Report to "Kabl overseer"; do not merge; stop when done. Caveman/Ponytail ultra in chat; code, commits, PR text normal English. Stay under 50% context.

## Task as received (condensed)
Wavetable import and a table editor for `osc.wt`, opened from its face: import a user .wav (single-cycle, multi-frame at 2048; say how frame size is detected, what happens when ambiguous, a control to correct it; a longer file sliced into frames allowed, nothing further without asking); editor with all frames shown, select/reorder/duplicate/delete/insert, pen and harmonic bars, normalise/remove DC/fade edges/interpolate-morph, audition while editing, undo through the existing undo; persistence (embed vs reference, justify; schema bump only if needed; prove old patches and the factory bank render unchanged); the same in the CLAP build with table data in the plugin state, checked in REAPER (state how the file dialog works in a host). Constraints: untrusted input (limits, no crash/hang/unbounded allocation, broken-file tests); nothing on the audio thread allocates or locks, swap without click, tested; frame time median < 8 ms and max < 16 ms with the editor open; Style/kit only, no theme or font change, no cable or rack changes; licensing (no new dependency needed); CI green on the final head. Out of scope: warp modes, spectral effects, table browser, resynthesis, presets, other oscillators. Deliver: a clip per audible feature, captures light and dark standalone and plugin at 1180x680, final report to the overseer.

## Done
Everything in the task. See `README.md` here (use, formats, limits, decisions, frame time, host run) and PR #30. Commits on the branch: modules import and edit ops; core log merge; ui window; tests; clips; bench modes; host evidence and media.

## Remaining
1. Confirm CI is green on the final head of PR #30 (`gh pr checks 30`); fix if not.
2. Send the final report to "Kabl overseer" (what was built, decisions, tests and totals 928 passed / 0 failed / 32 ignored, bench numbers, media paths, CI link, known gaps). Do not merge.

## Facts that cost effort
- Import, embedding (`Op::SetTable`, schema 5), `PatchEditor::import_table` and CLAP state with tables existed before this task; schema stays 7.
- REAPER offline render of a patch with an embedded table is silent for the first seconds (pre-existing; `imported-morph` does it): put notes at 4 s or later. Renders are not sample-identical between runs (free-running oscillators); compare level and spectrum per window.
- Host tooling: `host-*.py/.lua` here; REAPER needs its update notice and About window closed with `wmctrl -c`; plugin editor sits at (298,172) in the FX window; never `pkill -f` with a pattern that appears in your own command.
- Bash guard: plain single commands, Write tool for scripts, `python3 -I file.py`.

## Open for Kosta
Path field instead of a native file dialog; table undo log grows by one table per discrete edit; a shared slot edits every oscillator that uses it; the offline-render silence above.
