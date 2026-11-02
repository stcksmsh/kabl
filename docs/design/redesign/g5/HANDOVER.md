# G5 chrome: handover (agent F, worktree fix-flaky-alloc)

Task as received from "Kabl overseer": redesign G5 chrome (lib.rs toolbar / drawer / status bar, perform*.rs, browser.rs) to `mockups/directions.html` Rack / Perform / Sounds, faces "Line + solid", both app themes, 1440x900 / 1280x800 / 1180x680. Drawer collapses to a rail below 1280 px, rack auto-fits. Kit and tokens only, old satin / lacquer themes keep working. Out of scope: rack and face files, editors (`tables.rs`, `pattern.rs`, `drum_grid.rs`), the sequencer-title overlap (G3 D). Audio only on `kabl_agents`. Every PR: draft, capture sheet (mockup vs app, like-for-like), tests, perf line, one line to Kabl overseer. Never merge, no stacking (base master unless told), about 800 lines per PR.

## Done (draft PRs, all open)
- #86 `claude/ui-g5-chrome` (toolbar view switch with Sounds, primary Save, status bar in `kabl_ui::show`, `show_with`), #87 `claude/ui-g5-drawer` stacked on #86 (compact overlay drawer, cable-aware fit anchored top-left, drawer tabs / cable list / modulation list / why card, ropes above all content). Overseer retargets #87 when #86 merges.
- #88 `claude/ui-g5-sounds` (Sounds scene: library, list, detail with macros), #89 `claude/ui-g5-perform` (two-column Perform composition), #90 `claude/ui-g5-perform2` (card styling, `Button::large`). All on master.
- Tools: `crates/ui/examples/chrome_capture.rs` (scenes x 3 sizes x 2 modes; `KABL_CAPTURE_PATCH=DIR`), sheet scripts in the job tmp dir (not in repo).

## Review round 2026-11-02 (Kosta: match the mockup closer; cables #87 accepted)
Done and pushed: #86 toolbar (17b96f8, 2a6f70b, 56e4ccc; metrics in FACTS.md), #88 library column (f8550cf, d0617a9), #90 Perform (card kicker rows with hover tools, big macro knobs, 2x4 scene pads, one Sequences card with a row per sequencer, light-theme knob contrast; #89 merged into #90, close #89). Sheets: docs/design/redesign/g5/{toolbar-fix,sounds-fix,perform-fix}.

## Remaining
1. #87: add the verdicts.json entry (Kosta chose option A, drawer stays an overlay below 1280, no refit); gaps listed before still open (cable list excludes param routes, selected-row highlight, card text).
2. #90 gaps: Perform view header row (title, MIDI in, All notes off, Keep Routing open, Show rack) is not in the mockup, home for these actions undecided; sequencers have 8 steps (mockup 16); macro value inside the knob shows the param value, the mockup a percent; separate Stop button (the run button toggles).
3. #88: All and Recent library filters left the UI (Recent code kept); unmapped categories Basic, Sounds, Sequences, Performances show under All only.
4. Perf gate deferred by the overseer until the other machine jobs finish.

## Facts that cost effort
- Worktree guard rejects compound shell commands and heredocs: write scripts with the Write tool, run `python3 -I script.py`, plain single commands; `gh pr edit` fails (Projects classic), use `gh api -X PATCH repos/stcksmsh/kabl/pulls/N -F body=@file`.
- Root `CLAUDE.md` is untracked in the main checkout; read it by absolute path with cat, not `git -C`.
- `lift_foreground` used to paint module content over cables; removed in #87 (trays hid ropes).
- Fit: `frame_patch` then `refit_with_cables` after `draw_cables` (cable ends are world points); zoom floor is `MIN_ZOOM` 0.5.
- Mockup page screenshots: headless Chrome on a copy of `directions.html` with `const S=...` replaced (script in job tmp dir).
- Seq lanes read `v1..v8`, `r1..r8` via base values; factory sounds leave them at the 100 % default, so lanes look full.

## Working assumptions / open for Kosta
Perform columns apply only with one sequencer, at most four other controls and 520 px height; others keep the flowing rows. Scene pads show the number, the CC mapping is in the tooltip. Sheets use mockup scaled to window width; 1280 has no mockup.

## Standing rules
Report to the session named "Kabl overseer" (reply to its `from` socket), do not merge, stop when done. Style: caveman and ponytail at ultra in chat, normal English in commits, PRs and docs.
