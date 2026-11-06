# G4 owner F (chrome) handover (group 4 done, 2026-11-06)

Task from "Kabl overseer": G4-F group 4 (file browser) only. UI only: no sound or preset changes. Group 5 (dialogs `dlg-*`, About) is a separate later task on a new branch from origin/master. Agent 2 owns `tables.rs`, #111 and its worktree: do not touch. Scope F: `filebrowser.rs`, `browser.rs` dialogs, `about.rs`, `rack_editor.rs` modals. Spec: `G4-SPEC.md` §4 "File browser"; mockup `fileBrowser` in `mockups/g4-surfaces.html`.

## Done
- #109 (`claude/g4-f-explain`) and #110 merged. Merged origin/master into `claude/g4-f-filebrowser` (4903c5ee).
- `filebrowser.rs` is G4 modal L (see PR). Scenes `file-open`, `file-save`, `file-empty`.
- `chrome_capture.rs`: `raw.screen_rect` now follows the job size, so modals centre at 1180x680 (before, the footer clipped off-screen); `reset_areas` per job.
- fmt, clippy (zero warnings), `cargo test -p kabl-ui` green.
- Captures: `docs/design/redesign/g4/file-browser/` (dark = graphite, light = paper; 1440x900, 1280x800, 1180x680) and `file-browser/mockup/` (mockup shots).

## Remaining
Group 5 on a new branch from origin/master. Start: `grep -n "Modal\|egui::Window" crates/ui/src/browser.rs crates/ui/src/about.rs`.

## Facts
- Patches are folders with meta.toml: listed as `name.kabl` items (file icon, display only; on disk no suffix); real folders first with chevron. Open mode: click selects, Open button or double-click picks. Sounds place = user_root/sounds. Recent place left out: no recent-patches list exists (library recents are sound ids).
- Modified is UTC "2 Nov 18:40" (no time-zone data); real folders show no date.
- Capture ctx is shared across jobs; modals centre on the screen rect.
- `git push --force-with-lease` is denied: merge master, do not rebase.

## Open for Kosta
Recent place missing: needs a recent-patches list (none exists). Patch save writes the folder without ".kabl"; the suffix is display-only.

## Standing rules
Report to "Kabl overseer", do not merge, stop when done. Apply Style (caveman and ponytail ultra) on restart, read only this file.
