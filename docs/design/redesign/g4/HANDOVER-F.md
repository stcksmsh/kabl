# G4 owner F (chrome) handover (worktree fix-flaky-alloc, checkpoint 2026-11-03)

Task from "Kabl overseer": G4 owner F. Read `g4/HANDOVER.md` (work split row F) and `G4-SPEC.md` per group. Scope F: `lib.rs` menus, empty rack, drawer, toasts; `explain.rs`, `inspect.rs`, `compare.rs`, `recipes.rs`; `browser.rs` dialogs, `filebrowser.rs`, `about.rs`, `rack_editor.rs` modals. Agent 2 (E) owns `tables.rs`, `pattern.rs`, `routing.rs`, `seq_editor.rs` (#100). Keep `lib.rs` edits to drawer, status bar, toasts and menus. One surface group per PR from origin/master, draft PRs, report one line each to "Kabl overseer".

Acceptance per PR (G4-SPEC §6): graphite and paper captures at 1440x900 and 1180x680 beside `g4-surfaces.html`, same frame and scale; no overlap or clipping; contrast >= 4.5:1; no stock egui widgets on touched surfaces; Esc/Tab/Enter work; perf gate holds. Look at every capture. Before every push: `cargo fmt --all -- --check`, `cargo clippy --all-targets -p kabl-ui` (zero warnings), `cargo test -p kabl-ui` (check the pass count and compile errors, not only FAILED).

## Done
- Group 1 (rack fill, empty rack, toasts): PR #102 `claude/g4-f-rack-fill`, draft. Toasts from `last_message` (`toast_for`, `show_toasts` in lib.rs), error pill in status bar, empty rack state, rails to the window bottom, capture scenes `empty` and `error`, tools below.

## Remaining, in order (each from origin/master, own branch)
2. Explain / Inspect / Compare / Learn panels (`explain.rs`, `inspect.rs`, `compare.rs`, `recipes.rs`): targets in G4-SPEC §4 rows Explain, Inspect, Compare, Learn. Surf ids in the mockup: `explain`, `inspect`, `compare`, `learn`.
3. Module and cable context menus, More controls with Edit face as tab 2 (`menu-module`, `menu-cable`, `more-controls`).
4. File browser: open, save, empty (`file-open`, `file-save`, `file-empty`), no native dialogs.
5. The five dialogs (`dlg-unsaved`, `dlg-save-as`, `dlg-save-error`, `dlg-rename`, `dlg-restore`) and About (`about`).

## Facts
- Kit parts already exist in `kit/g4.rs` (toast, toasts, empty_state, banner, step_grid, ...); use them, do not add stock egui widgets.
- Mockup screenshots: `python3 docs/design/redesign/g4/tools/shot_mockup.py OUTDIR surf[,surf]` (headless google-chrome, 1:1). Side by side: `python3 docs/design/redesign/g4/tools/sheet.py OURS MOCK OUT scene:surf[,...]` (crops the mockup frame, scales to ours). Our captures: `PIPEWIRE_NODE=kabl_agents PULSE_SINK=kabl_agents xvfb-run -a -s "-screen 0 1440x900x24" cargo run --release -p kabl-ui --example chrome_capture -- OUTDIR scene[,scene]`; scenes: rack, perform, sounds, drawer, empty, error. Add scenes there for new surfaces (the example sets up state in `prepare`).
- Branch #102 is cut from master before #99 (drawer content, status readouts) merged: when #99 lands, `status_bar` in lib.rs will conflict (I replaced the grey `last_message` caption with the error pill; keep both the readouts and the pill).
- Font check: glyphs outside the embedded fonts fail `theme_tests::embedded_fonts_cover_every_character_the_interface_draws` (use "•", not "●").
- `last_message` stays the producer API (47 call sites, tests read it); the toast is derived each frame from changes to it, so the same text twice in a row gives one toast.
- Toast actions (Show details, Open another) and Esc-dismiss are not done.
- `git push --force-with-lease` is denied: push fast-forwards only. Root `CLAUDE.md` is untracked. Plain commands in this worktree. `cargo clean` before stopping.

## Open for Kosta / overseer
Whether toasts need the mockup's text actions now; the third rail row under a two-row patch (rows stay width-limited, not stretched).
