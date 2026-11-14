# G4 kit (agent A) handover

Task: build the G4-SPEC §3 kit parts, icons and tokens not covered by #91 (accent/theme tokens are agent 1's; do not touch). Parts sheet in both apps. Draft PRs only, Kosta merges, faces and G5 files out of scope.

## Done
- #93 `claude/g4-kit`: `kit/g4.rs` (checkbox, modal_framed S/M/L, tool_window, dock, empty_state, banner, toast + stack, step_grid, range_bar, stacked shadow, tokens), 14 icons, example `g4_parts`, sheets in `docs/design/redesign/g4/kit/`.
- Paper state colours set in `style/solid.rs` roles (spec hexes, good/warn darkened to reach 4.5 on bg); values in `redesign/FACTS.md`.
- #94 folded into #93 and closed. #92 (handover docs) merged.

## Remaining, in order
1. Logo swap PR from master (worktree `.worktrees/g4-logo` exists, nothing done): rebuilt logo files from `docs/design/redesign/g4/logo/` into `crates/ui/assets/logo/`, `kit::logo` (text colour in toolbar, orange only app icon and splash), `packaging/linux/logo/` (`render-icons.sh`, hicolor), small mark below 32 px. Captures: toolbar both apps, About, icon sizes.
2. G3 D (timing/drums faces) after #75 merges. Note: Seq "Edit" label overlaps the SEQU title; "out" caption overlaps OUTPUT title.
3. Surface PRs using the kit (G4-SPEC §4) once #93 is merged; modal_framed is not in the sheet (blocks input).

## Facts
- Existing kit already had button, icon_button, field, dropdown (select), slider, segmented, toggle, tag (chip), menu items, popover_frame; reused, not duplicated.
- `egui::Panel::bottom` must be added before `CentralPanel` or it renders nothing.
- Shadow layer for `tool_window` must be `Order::Background` or it paints over the window.
- Capture tool: `claude/g4-capture-tool` (`g4_capture.rs`), sheet: `cargo run --release -p kabl-tools --example g4_parts -- DIR` under xvfb.
- Paper accent text and accent fill are #91's; sheet shows brown paper accent until it merges.

## Rules
Report to "Kabl overseer", do not merge, stop when done.
