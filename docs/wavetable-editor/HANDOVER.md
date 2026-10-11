# Wavetable import and editor: handover

Branch `claude/wavetable-editor` (from origin/master), worktree `.worktrees/sound-engines-2`. Report to "Kabl overseer"; do not merge; stop when done. Caveman/Ponytail ultra in chat; code, commits, PR text normal English. Stay under 50% context.

## Task as received (condensed)
Build for `osc.wt`: (1) import a user .wav (single-cycle, multi-frame at 2048 like Serum/Vital; say how frame size is detected, what happens when ambiguous, control to correct; resample + band-limit like bundled tables; a longer audio file sliced into frames is an allowed extra, nothing further without asking); (2) table editor from the osc.wt face: all frames (stack + single view), select/reorder/duplicate/delete/insert, edit by drawing and harmonic bars, normalise/remove DC/fade edges/interpolate-morph between two frames, audition while editing, undo/redo through the existing undo; (3) persistence: embed vs file reference, decide and justify, bump schema (7) with a migration only if it changes; prove old patches and the factory bank render unchanged; (4) plugin: same in CLAP, table in plugin state, check save/quit/reopen/render in REAPER, state how the file dialog works in a host. Constraints: untrusted input (limits documented, no crash/hang/unbounded alloc, test broken files); nothing on the audio thread allocates or locks, swap without click, tested like the existing no-alloc tests; frame time median < 8 ms, max < 16 ms with the editor open; all drawing from Style/kit, no theme/font changes (claude/theme-b), no cable/rack changes (claude/cables-legibility); licensing: GPL, cargo-deny + THIRD-PARTY-NOTICES, bundle no third-party tables; CI (fmt, clippy 1.99 strict all targets, tests, package, validator, licenses) green on final head. Out of scope: warp modes, spectral effects, table browser/library, resynthesis, presets, other oscillators. For Kosta: one clip per audible feature (imported table swept, drawn table, morph-filled) with a one-line listening note; editor captures light and dark, standalone and plugin editor at 1180x680. Final report to the overseer: built, decisions (formats and frame detection, embed vs reference, limits, schema), tests and totals, bench numbers, clip and capture paths, CI run link, known gaps.

## Done
- modules: `import_wav_with` (frame length detect/choose, `ImportInfo`, `Detected`), FFT resample for power-of-two cycles, `tableedit.rs` (normalise, remove_dc, fade_edges, harmonics/set_harmonic, interpolate, morph, fill_between, fit). Tests `crates/modules/tests/tableedit.rs`.
- core `log.rs`: continuing `SetTable` merges into the gesture's entry (also inside a Group).
- ui: `editor.rs` `put_table` (gesture-aware, optional bind of the module's `user`), `tables.rs` (window), `lib.rs` Edit chip on the osc.wt display, tests `crates/ui/tests/wavetable_editor.rs` (9 + ignored `write_clips`), `bench_chrome` modes `editor` / `draw`.
- engine test `crates/engine/tests/table_swap.rs` (no alloc, no click across 20 swaps).
- docs `docs/wavetable-editor/README.md` (use, formats, limits, decisions), `make_demo_table.py`.
- Bench (release, dense Composition 1440x900 light, live, editor open): median 6.0 ms, max 7.7 ms; `draw` mode median 5.4 ms, max 6.5 ms.

## Remaining, in order
1. `cargo test --workspace` totals (log at `~/.claude/jobs/1e2b91f5/tmp/ws.log` if still there), fmt, clippy `-D warnings --all-targets`, `cargo deny`/notices unchanged (no new dependency).
2. Real host: build `cargo build --release -p kabl-clap` (see CI for the cdylib step), project from `docs/host-production/scripts/make-project.py` style (state JSON + 8-byte length, base64), REAPER tooling in `~/.claude/jobs/4129379d/tmp/host/start.sh` (Xvfb :148); edit a table in the plugin editor at 1180x680, save, quit, reopen, render both, compare; captures light/dark.
3. Media into `docs/wavetable-editor/media/` (webp captures, clips as wav/mp3) from `~/.claude/jobs/1e2b91f5/tmp/o3,o4/shots` and `target/wt-clips`.
4. decisions.md entry, STATUS line, draft PR to master, CI link, report.

## Facts that cost effort
- Import, embedding (`Op::SetTable`, schema 5), `PatchEditor::import_table`, CLAP state with tables already existed before this task; schema is not bumped.
- Capture runner `~/.claude/jobs/1e2b91f5/tmp/run.sh OUT PATCH SIZE SCRIPT` (Xvfb :146, drive.py; hit keys `edit-table:ID`, `wt:path|import|use|cancel|wave|bars|stack|insert|dup|delete|left|right|normalise|dc|fade|interp|morph|undo|redo|audition|position|tab-draw|tab-bars|size-*`).
- Bash guard: plain single commands, Write tool for scripts, `python3 -I file.py`.
- Cargo target dir is shared by builds in this worktree; release builds take about 1.5 min.

## Working assumptions / open for Kosta
Path field instead of a native file dialog; table undo log keeps one table per gesture (log grows per discrete edit); editing a table used by two oscillators changes both (slot semantics).
