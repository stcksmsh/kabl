# G5 chrome: handover (agent F, worktree fix-flaky-alloc)

Task as received from "Kabl overseer": redesign G5 chrome (lib.rs toolbar / drawer / status bar, perform*.rs, browser.rs) to `mockups/directions.html` Rack / Perform / Sounds, faces "Line + solid", both app themes, 1440x900 / 1280x800 / 1180x680. Drawer collapses to a rail below 1280 px, rack auto-fits. Kit and tokens only, old satin / lacquer themes keep working. Out of scope: rack and face files, editors (`tables.rs`, `pattern.rs`, `drum_grid.rs`), the sequencer-title overlap (G3 D). Audio only on `kabl_agents`. Every PR: draft, capture sheet (mockup vs app, like-for-like), tests, perf line, one line to Kabl overseer. Never merge, no stacking (base master unless told), about 800 lines per PR.

## Done (draft PRs, all open)
- #86 `claude/ui-g5-chrome` (toolbar view switch with Sounds, primary Save, status bar in `kabl_ui::show`, `show_with`), #87 `claude/ui-g5-drawer` stacked on #86 (compact overlay drawer, cable-aware fit anchored top-left, drawer tabs / cable list / modulation list / why card, ropes above all content). Overseer retargets #87 when #86 merges.
- #88 `claude/ui-g5-sounds` (Sounds scene: library, list, detail with macros), #89 `claude/ui-g5-perform` (two-column Perform composition), #90 `claude/ui-g5-perform2` (card styling, `Button::large`). All on master.
- Tools: `crates/ui/examples/chrome_capture.rs` (scenes x 3 sizes x 2 modes; `KABL_CAPTURE_PATCH=DIR`), sheet scripts in the job tmp dir (not in repo).

## Remaining, in order
1. #88: use `Button::large()` for Load sound / Open in rack once #90 (kit) has merged; note: do not edit #88's branch from #90.
2. #86 gaps: centred tabs, search field, sound dropdown with category, cable-tool icons, zoom in the toolbar, glyph wordmark (all mockup items still missing).
3. #87 gaps: cable list excludes param routes (ADSR to Cut CV shows under Modulation), selected-row highlight, card text is the module description.
4. #90 gaps: one "Sequences" card with a row per sequencer, centred macro names.
5. Perf gate: line_solid was 1-10 % above satin on the dense patch in 3 interleaved rounds (docs/design/redesign/g5/pr2/perf-ab-raw.txt); sent to Kosta by the overseer, not mine to resolve.

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
