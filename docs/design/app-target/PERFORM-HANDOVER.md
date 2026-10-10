# Perform view (task B): handover

Branch `claude/perform`, stacked on `claude/widget-set` (PR #18, merged with master dfe0ef6). Worktree `.worktrees/widget-set`. Draft PR #21, base `claude/widget-set`. Report to "Kabl overseer", do not merge, then stop.

## Task as received (condensed)
1. Perform on the widget kit (A's tokens, B's layout): transport, scenes, macros with arc knobs, banks with step lanes, every old control kept. Rack|Perform switch. Keep `ui_state.record` keys; fit 1440x900, 1280x800 and the plugin's 1180x680, light and dark.
2. Polish pass (from the overseer): fill the width; pads and tempo readable from a distance; keep most-used controls above the fold; close the Routing drawer on entering Perform unless pinned; bar/beat readout and queued-pad progress via the smallest audio-thread report; one hue per step lane from theme slots.

## Done
- Perform view and switch (`cabd1c8`), polish pass (`ad91f8e`); app_target clippy fix is in PR #18 now.
- Layout (`perform_view.rs::panel`): widths from the room (`split`), heights from the window (`FIXED_H` budget; pads 84 to 140 px, knobs 64 to 110 px). Rows: transport + scenes; macros + sequence cards (weights, `BANK_WEIGHT`); control cards in equal columns. No scenes: the transport joins the macro row when sequences exist. Macros without sequences keep `TILE_W` and the controls flow beside them (the docked 340 px strip needs it).
- Drawer rule (`lib.rs::show`): entering the full view closes the drawer unless `drawer_pinned`; leaving reopens it if Perform closed it (`drawer_auto_closed`). Pin chip "Keep Routing open" in the Perform header (key `drawer-pin`). A hand-opened drawer is left alone.
- Clock position: `Clock::position()` (16ths since the last restart, counted as launch boundaries are), `clocks(|id, running, pos|)` in `compile.rs` and `patch_engine.rs`, drained into `UiState::clock_pos` by `main.rs` (ring item `(ModuleId, bool, f64)`) and `clap/src/lib.rs` (`RuntimeReport::Clock`). UI: `bar_beat`, `kit::bar_meter`, pad progress for queued cues from the cue's own clock.
- Lane hues: `roles.audio`, `cv`, `gate` by sequencer order.
- Tests: `crates/engine/tests/clock_position.rs` (no allocation, tempo, monotonic, Stop, Restart, host mode), `crates/ui/tests/perform.rs` (drawer rule, width and fold), unit tests for `split` and `bar_beat`.

## Verification at `ad91f8e` plus docs
`cargo test --workspace --no-fail-fast` 828 passed, 0 failed, 28 ignored; strict clippy clean. Captures and recording in `media/perform/` (and `plugin/`); frame times in the PR body.

## Facts that cost effort
- sp() scale is [2,4,8,12,16,24,32,48]: card margin is 12, gap 8.
- A macro tile cannot be narrower than about 150 px unless its MIDI row wraps (`horizontal_wrapped`); a bank card's inner width needs about 200 px (pads in one row, the "playing: ..." label on its own row).
- The drawer header must stay one row: a second row shrank the explain panel and clipped its Show buttons (explain tests).
- Capture tooling in the job tmp dir: `cap/perf.sh` (matrix), `cap/rec.sh` + `rec.txt` (ffmpeg x11grab recording), `host/start.sh` (REAPER, display :148, `import -window root`; the factory sounds are not available there, so the plugin capture shows the default patch). Never `pkill`; `touch host/run/quit`. The Bash guard rejects variables and compound commands in a worktree session: use literal paths.
- `docs/design/app-target/media/perform/composition-1440x900-light-docked.webp` is from the earlier revision.

## Remaining / open
- Bar/beat in a real host: engine test only; not watched in REAPER with a moving clock.
- In host mode bars count from the clock's last restart (as launches do), so after a host seek the bar differs from the project's.
- Bank pads have no progress (no reference clock is known there); only cue pads do.
- Decisions for Kosta: Rack|Perform switch opens the full view (built) or the docked strip; drawer rule as described.
