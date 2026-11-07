# Agent 2 handover (2026-11-03)

Tasks came one at a time from "Kabl overseer": match kabl UI pieces to the G4/G5 mockups (measure from mockup source, theme tokens not literals, capture graphite and paper next to the mockup, look at each capture), draft PRs only, never merge, no sound changes. Worktrees: `.worktrees/g4-kit` (#90, #95, #96, #97) and `.worktrees/sound-engines-2` (#75).

## PRs (all draft, master merged in on 11-03; merge order #95 -> #88 -> #90 -> #96 -> #97 -> #98)
- #95 `claude/logo-swap` 918958d: rebuilt logo (toolbar 32 px text colour via `brand()`, window icon, hicolor icons, small mark below 32 px). Captures `g4/logo/captures/`.
- #90 `claude/ui-g5-perform2` 6ec25d9 (taken over from agent 1): Play/Stop/Restart (new `pstop:<id>`; `prun` only starts), mockup card titles and spacing (16 margins/gaps), knob row at 160/390, sans mappings, skip-back icon. Lanes 8 steps (mockup 16). Captures `g5/perform3/`.
- #96 `claude/ui-seq-header` 5e74995: full seq face (header EDIT/PLAY groups, left Length/Gate length column, per-step dot/number/knob/gate pill/velocity bar, playing column). PAGE space kept empty. Captures `g4/seq-header/` (`KABL_CAPTURE_STEP` in face_capture).
- #97 `claude/ui-jacks` 1e07d5f: round jacks, mockup plug, mockup cable curve. Plugs sit on the ropes since #87. Captures `g4/jacks/`.
- #75 `claude/seq-32` b0ad9e6: master merged (rack/mod.rs conflict), workspace tests + clippy green. Rule: engine 32 steps and the groove restore ship together.

## Queued after #75 merges
1. #96: PAGE chips (third header group, room already kept in `BANKS_W`) and Length up to 32; recapture.
2. #90: Perform lanes at the real length (`kit::seq_lane` pitch cap 48 keeps mockup cell size).
Then merge master into the remaining PRs again.

## Open
- Recovery-check CI flake "kabl window did not become focusable": rerun requested, not a code fault found.
- #93 conflict choice: kept G4 good/warn/bad over #91's in `solid.rs` (Kosta to confirm).
- Knob caps have tick rings the mockup lacks (reported, unchanged). Tray box vs mockup rule is #91's recipe.

## Facts that cost effort
- Mockup pages: `g4-surfaces.html` (rack faces, surfaces; select via `#surf`, `#app`) and `directions.html` (Perform: replace `const S={...}`); render with playwright from `/secondary/Programming/Github/EmojiType/node_modules`, `executablePath: '/usr/bin/google-chrome'`. g4 rack faces render at ~0.976 scale: scale the crop to 660x340 before comparing.
- Captures: `face_capture`, `cable_capture`, `chrome_capture` examples under `xvfb-run`, `XDG_CONFIG_HOME` in /tmp, `PIPEWIRE_NODE/PULSE_SINK=kabl_agents`.
- `gh pr edit` fails (Projects classic): use `gh api -X PATCH repos/stcksmsh/kabl/pulls/N`.
- Release `cargo test` cannot build `tests/functional_cables_control.rs` (assert_no_alloc); use debug for the full suite.
- Default look `line_solid` = mockup "hybrid" ("Signal/Drafting solid"); jack recipe `hex` now draws round (name kept for saved themes).

## Rules
Run `cargo fmt --all -- --check` and clippy `--all-targets -D warnings` before every push. Report to "Kabl overseer", do not merge, `cargo clean` when stopping, stop when done.
