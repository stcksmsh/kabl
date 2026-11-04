# G4 owner F (chrome) handover (worktree fix-flaky-alloc, checkpoint 3 on 2026-11-04)

Task from "Kabl overseer": resume G4-F UI work, groups 4 (file browser) and 5 (dialogs and About), from this file. UI only: no sound or preset changes, no D11/D12. Also keep #109 applying cleanly on origin/master. One PR per group from origin/master, draft, not stacked. Scope F: `filebrowser.rs`, `browser.rs` dialogs, `about.rs`, `rack_editor.rs` modals; agent 2 (E) owns `tables.rs` (the wavetable editor embeds the file browser inline). Spec: `G4-SPEC.md` §4 rows "File browser", "Dialogs", "About"; mockup functions `fileBrowser`, `dlg-*`, `about` in `mockups/g4-surfaces.html` (around line 662).

Acceptance per PR (G4-SPEC §6): graphite and paper captures at 1440x900 and 1180x680 beside the mockup; no overlap or clipping; contrast >= 4.5:1; no stock egui widgets; Esc/Tab/Enter work. Before every push: `cargo fmt --all -- --check`, `cargo clippy --all-targets -p kabl-ui` (zero warnings), `cargo test -p kabl-ui`.

## Done
- #109 `claude/g4-f-explain`: merged origin/master (after #110) at 1637d66f, one conflict in `chrome_capture.rs` SCENES (union, 18 scenes); fmt, clippy, all kabl-ui tests green; pushed. Awaits review.
- Group 4 WIP on `claude/g4-f-filebrowser` (from origin/master 7ec28c93), commit in the CHECKPOINT line: `filebrowser.rs` rebuilt as G4 modal L via `g4::modal_framed` (icon Back/Up/Home, clickable breadcrumb cut from the front, Filter field, 164 px places column of Home plus remembered folders, Name/Modified list with 32 px rows, accent-border selection in Save, `g4::empty_state` with Go up, warn/bad banners, footer Cancel + Choose this folder / Save with Name field and suffix / Replace). Inline use (wavetable) gets a close icon instead of the footer. `record::civil_date` shared for the date column. Scenes `file-open`, `file-save`, `file-empty` in `chrome_capture.rs` (temp HOME tree with fixed dates). file_browser and wavetable_editor tests pass; clippy clean before the capture scene edits.

## Remaining, in order
1. Run fmt check, clippy, full `cargo test -p kabl-ui` on `claude/g4-f-filebrowser`.
2. Capture: `PIPEWIRE_NODE=kabl_agents PULSE_SINK=kabl_agents xvfb-run -a -s "-screen 0 1440x900x24" cargo run --release -p kabl-ui --example chrome_capture -- OUT file-open,file-save,file-empty`. First captures were taken but NOT looked at yet. Mockup shots: `python3 docs/design/redesign/g4/tools/shot_mockup.py OUT file-open,file-save,file-empty`; sheet with `tools/sheet.py`. Look at every capture; fix overlap (breadcrumb vs Filter, footer field width at 1180).
3. Draft PR for group 4, report PR number, commit and capture paths to "Kabl overseer".
4. Group 5 (dialogs `dlg-unsaved`, `dlg-save-as`, `dlg-save-error`, `dlg-rename`, `dlg-restore`; About) on a new branch from origin/master. Start: `grep -n "Modal\|egui::Window" crates/ui/src/browser.rs crates/ui/src/about.rs`.

## Facts
- Patches are folders, so "Open patch" is `Mode::Folder` with `FOLDERS`: the list shows folders only, not the mockup's `.kabl` files. Open mode keeps click-to-pick (tests rely on it), so its footer has Cancel only, no Open button.
- Modified dates are UTC dates without time (no time-zone dependency); marked `ponytail:` in `row()`.
- The `kabl_agents` null sink was missing and was recreated this session.
- `g4::Sev` variants: Bad, Warn, Good. Kit icons include Back, Up, Home, Folder, File, Search, Right, Wave, Rec, Panel.
- `git push --force-with-lease` is denied: push fast-forwards only, so merge master instead of rebasing. Plain single commands in this worktree (the guard rejects compound shell and heredocs); write scripts with the Write tool.

## Open for Kosta
Third rail row under a two-row patch (rows stay width-limited). Open mode without an Open button (click picks).

## Standing rules
Report to "Kabl overseer", do not merge, stop when done. Apply Style (caveman and ponytail ultra) on restart, read only this file.
