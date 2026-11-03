# G5 agent 1 handover (worktree fix-flaky-alloc, checkpoint 2026-11-03 at 51 % context, second checkpoint)

Task as received from "Kabl overseer": polish G5 PRs against the mockup `docs/design/redesign/mockups/directions.html`. Never merge. Report to "Kabl overseer" (reply to its `from` socket), one line per step. Audio only on `kabl_agents`. `cargo clean` before stopping. Chat caveman and ponytail ultra, commits and docs normal English. Out of scope: editors, kit parts beyond the items below. #90 moved to agent 2.

## Done (all pushed)
- #91 tokens, #86 chrome, #93: merged to master.
- #88 `claude/ui-g5-sounds`: detail pane polish; All sounds and Recent rows in the library column plus test; category-mapping verdict recorded (`1d93adc`).
- #87 `claude/ui-g5-drawer`: merged origin/master (conflicts in `lib.rs` resolved: kept `fn drawer`, dropped unused `tool`, took master's rail toggle), `DRAWER_W` 320, left shadow on the compact overlay, `explain.rs` test windows made taller, `chrome_capture` fixed after merge, captures in `g5/drawer/` (`6bc19df`). Verdict entry for the drawer already existed. kabl-ui tests green.

## Remaining, in order (queued by the overseer, branch names are fixed)
1. `claude/ui-caps-tray` from origin/master: knob caps exactly as the mockup (plain caps, no tick rings) and port trays as a single hairline rule (not a filled or boxed plate). Change them in the shared kit (`kit::parts` and the cap recipe) so every face follows. Measure from the mockup source. Capture the rack light and dark plus 2x module crops next to the mockup, look at each capture, open a DRAFT PR, report one line.
2. `claude/ui-drawer-content` from master, only after #87 merges: Routing tab as numbered cable list with colour bars and aligned "source -> dest" columns; MODULATION list with source chips, value bars, percentages; "Why it sounds like this" card. Remove the loose Help/Compare/Learn row and the inline "SVF Filter #3 Explain" sliders and Remove module button (move to Inspect or the module menu). Status bar: output meter, sample rate, buffer, CPU, voices, MIDI device, BPM plus song position on the right. Rack must fit the width left by the docked drawer. Capture 1440 and 1180, light and dark, next to the mockup; draft PR; one line.
3. `cargo clean` in this worktree before stopping.

## Facts
- `git push --force-with-lease` is denied by the auto-mode classifier: rebuild on the remote and push a fast-forward.
- Root `CLAUDE.md` is untracked; read it by absolute path. Use plain commands in this worktree.
- Capture: `PIPEWIRE_NODE=kabl_agents PULSE_SINK=kabl_agents xvfb-run -a -s "-screen 0 1440x900x24" cargo run --release -p kabl-ui --example chrome_capture -- OUTDIR rack` (writes 1440x900, 1280x800, 1180x680, light and dark). At 1180 the drawer is closed in compact mode, so the shadow does not show; open it in the example to see it.
- Sheet script: `/home/stcksmsh/.claude-alt/jobs/85818718/tmp/tksheet.py APPDIR OUTDIR`; mockup screenshots in that job's tmp `mock/`.
- Adding rows to the Sounds library column pushes "New" below the viewport at 900 px; the `browser.rs` test now uses a 1000 px window.
- Mockup toolbar and Sounds metrics: `docs/design/redesign/FACTS.md`.
- This worktree currently sits on `claude/ui-g5-drawer`; create the new branch with `git checkout -b claude/ui-caps-tray origin/master` after `git fetch origin`.

## Open for Kosta
Audition button is disabled when the rack sound has no keys or clock. Buttons keep default height (`Button::large` is on #90). The Sounds library keeps All sounds and Recent rows although the mockup lists only Factory, My sounds, Favourites.
