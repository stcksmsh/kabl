# G5 agent 1 handover (worktree fix-flaky-alloc, checkpoint 2026-11-03 at 51 % context)

Task as received from "Kabl overseer": polish G5 PRs against the mockup `docs/design/redesign/mockups/directions.html`. Never merge. Report to "Kabl overseer" (reply to its `from` socket), one line per step. Audio only on `kabl_agents`. `cargo clean` before stopping. Chat caveman and ponytail ultra, commits and docs normal English. Out of scope: rack and face files, editors, kit parts.

## Done (all pushed)
- #91 `claude/ui-g5-tokens`: merged by Kosta. Tray plate, accents to #F2A33A, colour audit (`g5/tokens/COLOR-AUDIT.md`), warn strings and routing pill on roles (`d50e311`).
- #88 `claude/ui-g5-sounds`: detail pane polish, waveform removed, full description (`713d636`, `310eedb`). Captures `g5/sounds-detail/`.
- #86 `claude/ui-g5-chrome`: toolbar already in mockup order; icons from mockup, switch height, 95 % capture (`bfb8c55`). Strip `g5/toolbar-fix/`.

## Remaining, in order
1. #87 `claude/ui-g5-drawer` (checked out in this worktree, clean at 7684ca7): `git merge origin/claude/ui-g5-chrome` then `git merge origin/master`. First merge conflicts in `crates/ui/src/lib.rs` (toolbar and drawer areas) and touches `crates/ui/tests/explain.rs`; resolve keeping both.
2. #87 work per G4-SPEC: drawer docks at 320 px at 1280 and above (code `DRAWER_W` in lib.rs; check the value), below 1280 a 44 px rail plus a 320 px overlay with a left shadow, rack zoom unchanged. Add the verdict entry in `docs/design/verdicts.json` (see the entry format there).
3. Recapture rack at 1440x900 and 1180x680, light and dark: `PIPEWIRE_NODE=kabl_agents PULSE_SINK=kabl_agents xvfb-run -a -s "-screen 0 1440x900x24" cargo run --release -p kabl-ui --example chrome_capture -- OUT rack`. Sheet script: `/home/stcksmsh/.claude-alt/jobs/85818718/tmp/tksheet.py APPDIR OUTDIR` (mockup screenshots in the same job tmp `mock/`). Look at every capture, push, one line to the overseer.
4. #90 `claude/ui-g5-perform2` gaps (from g5/HANDOVER.md on that branch): 16-step lanes, Stop button, Perform header; #88 All and Recent filters.

## Facts
- `git push --force-with-lease` is denied by the auto-mode classifier: after a rebase, rebuild the branch on top of the remote and push a fast-forward instead.
- Root `CLAUDE.md` is untracked; read by absolute path. Compound shell commands can trip the worktree guard; use plain commands.
- `chrome_capture` on #90 has the mockup-like Perform patch; on #86 it sets zoom 0.95 for rack at frame 6. `KABL_CAPTURE_PATCH=DIR` captures another patch.
- Mockup toolbar and Sounds metrics are in `docs/design/redesign/FACTS.md`.

## Open for Kosta
Audition button is disabled when the rack sound has no keys or clock. Buttons keep default height (`Button::large` is on #90).
