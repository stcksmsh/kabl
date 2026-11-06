# G4 owner F (chrome) handover (worktree fix-flaky-alloc, checkpoint 2 on 2026-11-03)

Task from "Kabl overseer": G4 owner F. Read `g4/HANDOVER.md` (work split row F) and `G4-SPEC.md` per group. Scope F: `lib.rs` menus, empty rack, drawer, toasts; `explain.rs`, `inspect.rs`, `compare.rs`, `recipes.rs`; `browser.rs` dialogs, `filebrowser.rs`, `about.rs`, `rack_editor.rs` modals. Agent 2 (E) owns `tables.rs`, `pattern.rs`, `routing.rs`, `seq_editor.rs`, and the cues editor (#104, cue reordering is theirs, skip it). Keep `lib.rs` edits to drawer, status bar, toasts and menus. One surface group per PR from origin/master, draft PRs, report one line each to "Kabl overseer".

Acceptance per PR (G4-SPEC §6): graphite and paper captures at 1440x900 and 1180x680 beside `g4-surfaces.html`, same frame and scale; no overlap or clipping; contrast >= 4.5:1; no stock egui widgets on touched surfaces; Esc/Tab/Enter work; perf gate holds. Look at every capture. Before every push: `cargo fmt --all -- --check`, `cargo clippy --all-targets -p kabl-ui` (zero warnings), `cargo test -p kabl-ui` (check pass count and compile errors, not only FAILED).

## Done
- Group 1: PR #102 `claude/g4-f-rack-fill` (3f9ed8e5 plus this handover). Toasts with actions (Show details, Open another...) and Esc, error pill, empty rack with `.g4(32.0)` Browse sounds, rails to window bottom, `FIT_MAX_ZOOM` 1.5, scenes `empty` and `error`. Merged with master twice; fmt/clippy/tests green.
- PR #90 `claude/ui-g5-perform2` merged with master, f6a54596.

## Remaining, in order
Stopped 2026-11-04 on Kosta's order: #109 (claude/g4-f-explain, d33f0018) and #110 (claude/g4-empty-rack-fix) await review, merge #110 first; then group 4 (file browser) and 5 (dialogs and About); context menus and More controls moved to agent 2.

## Facts
- Kit parts in `kit/g4.rs`; use them, no stock egui widgets. `Button::g4(h)` is the mockup button look (master's); `outlined` was dropped.
- Mockup shots: `python3 docs/design/redesign/g4/tools/shot_mockup.py OUTDIR surf[,surf]`. Sheets: `python3 docs/design/redesign/g4/tools/sheet.py OURS MOCK OUT scene:surf[,...]`. Captures: `PIPEWIRE_NODE=kabl_agents PULSE_SINK=kabl_agents xvfb-run -a -s "-screen 0 1440x900x24" cargo run --release -p kabl-ui --example chrome_capture -- OUTDIR scene[,scene]`; scenes: rack, perform, sounds, drawer, wavetable-editor, seq-pattern-editor, empty, error. Stored sheets: `docs/design/redesign/g4/f-rack/` (not recaptured after the last master merge).
- Rack mockup scene has the drawer docked (95 %); the error and toasts mockup scene has none (109 %, rail only). Ours at 95 % is width-limited because faces are wider than the mockup's: the reason for the face-widths PR.
- Toast API: `Toast` has `actions`, `expanded`; `g4::toasts` returns `(toast index, action index)`; Esc dismisses the newest.
- Merge conflicts: after `git merge` grep all files for `<<<<<<<` (include `examples/`), a conflict in chrome_capture.rs was once committed by mistake.
- Glyphs outside the embedded fonts fail `theme_tests::embedded_fonts_cover_every_character_the_interface_draws`.
- `git push --force-with-lease` is denied: push fast-forwards only. Plain commands in this worktree. `cargo clean` before stopping.

## Open for Kosta
Third rail row under a two-row patch (rows stay width-limited, not stretched).

## Standing rules
Report to "Kabl overseer", do not merge, stop when done. Apply Style (caveman and ponytail ultra) on restart, read only this file.
