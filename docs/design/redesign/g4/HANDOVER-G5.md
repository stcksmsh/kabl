# G4 group 5 (dialogs and About) handover

Branch `claude/g4-group5` from origin/master. Draft PR. UI only; `lib.rs` untouched, `filebrowser.rs` untouched (agent 1, #112).

## Done
- `kit/g4.rs` `modal_framed`: new `lead` closure (left footer slot, for a danger action); empty `title` drops the header row (About).
- `browser.rs` `dialogs()`: Unsaved (S, Don't save danger at left, Cancel, Save), Save as (M, Name field, Category dropdown, Tags as removable chips + "Add tag" field, banner for errors, primary reads "Try again" after an error, "Replace it" on a name clash), Rename (S, error banner), Restore (M, undo reassurance line with icon). All via `g4::modal_framed`, `g4::banner`, `g4::chip_label`, `Button::g4(32)`. Hit keys unchanged (`dlg:*`).
- `about.rs`: 56 px accent mark drawn with the painter, wordmark 28, mono `Version x · commit abcdef0`, tagline, licence text (kept in full, legal wording), source row (inset strip, mono URL, copy icon button), accent "Third-party notices" link (toggles the scroll list), Close. Hit keys `about:close`, `about:notices`, `about:copy`.
- Scenes `dlg-unsaved`, `dlg-save-as`, `dlg-save-error`, `dlg-rename`, `dlg-restore`, `about` in `chrome_capture` (SCENES 28).
- Test helper `save_as_named` settles two extra frames (the modal re-centres when a banner grows it).

## Captures
`g4/g5-dialogs/ours/<scene>__<graphite|paper>__<1440x900|1180x680>.png`, mockup shots in `g4/g5-dialogs/mockup/` (same names). Every PNG viewed.

## Gaps
- "Choose folder…" in the save-error dialog is not there: the library has no folder picker.
- Rename/Save as name field: focus ring missing in captures (`request_focus` when nothing is focused; the capture harness has the rack focused). Works live.
- "Add tag" is a kit field, not bare placeholder text as in the mockup.
- Rack-editor modals: More controls already uses `g4::surface_frame` (agent 2's work); nothing changed there. `panels.rs` panel-fallback modal is stock `egui::Modal` and out of scope (not in the group 5 list).
- Backdrops differ from the mockup (ours: live rack at 107 %/77 %, mockup 109 %/78 %): same as earlier groups.
- No Enter-activates-primary on Unsaved/Restore.

## Checks
fmt --check, clippy --all-targets -p kabl-ui -D warnings, `cargo test -p kabl-ui` all green.
