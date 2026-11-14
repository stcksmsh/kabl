# G5 agent 1 handover (worktree fix-flaky-alloc, checkpoint 2026-11-03 at 51 % context, third checkpoint)

Task as received from "Kabl overseer": polish G5 PRs against the mockup `docs/design/redesign/mockups/directions.html`. Never merge. Report to "Kabl overseer" (reply to its `from` socket), one line per step. Audio only on `kabl_agents`. `cargo clean` before stopping. Chat caveman and ponytail ultra, commits and docs normal English. Before every push: `cargo fmt --all -- --check`, `cargo clippy --all-targets -p kabl-ui` (no warnings), tests with a real pass count (check compile errors, not only FAILED lines).

## Done (all pushed)
- #87 drawer, #86, #91, #93: merged to master.
- #88 `claude/ui-g5-sounds` `8fda7d4`: All sounds and Recent rows, category verdict, master merged, fmt/clippy/tests clean.
- #98 `claude/ui-caps-tray` `20b4ee8` (draft PR): plain caps, hairline tray, mockup solid cap (skirt, 0.78 shaded cap, 2.5 px quarter-ink track), "Pulse width" in full; captures and 2x crops in `g5/caps-tray/`; master merged, clean.

## Remaining, in order
1. `claude/ui-drawer-content` (checked out here, from origin/master 5e870c7, only "before" captures `g5/drawer-content/before/` added, uncommitted until you commit). Brief: match the mockup Rack drawer content.
   - Routing tab: numbered cable list with colour bars and aligned "source -> dest" columns; MODULATION list with source chips, value bars, percentages; the "Why it sounds like this" card. `drawer.rs` already has `tabs`, `lists` (line 174) and `why_card` (276): compare the "before" captures with the mockup (`/home/stcksmsh/.claude-alt/jobs/85818718/tmp/mock/rack__*__1440.png`, source `directions.html` near lines 380-430) and close the gaps.
   - Drop the loose Help/Compare/Learn row, the inline "SVF Filter #3 Explain" sliders and the Remove module button (move to Inspect or the module menu).
   - Status bar as in the mockup: output meter, sample rate, buffer, CPU, voices, MIDI device; BPM plus song position on the right (mockup bottom row).
   - Rack must fit the width left by the docked drawer, not be cut off under it.
   - Capture 1440 and 1180, light and dark, next to the mockup; look at each; open a DRAFT PR; report one line.
2. `cargo clean` in this worktree before stopping.

## Facts
- `git push --force-with-lease` is denied by the auto-mode classifier: push fast-forwards only.
- Root `CLAUDE.md` is untracked; read it by absolute path. Use plain commands in this worktree.
- Capture: `PIPEWIRE_NODE=kabl_agents PULSE_SINK=kabl_agents xvfb-run -a -s "-screen 0 1440x900x24" cargo run --release -p kabl-tools --example chrome_capture -- OUTDIR rack`. Scene `drawer` opens the drawer in compact windows. 2x crop recipe: PIL crop of ours and the mockup, paste side by side, resize x2.
- Drawer width `DRAWER_W` is 320; compact (below 1280) overlay has a left shadow.
- Never use `git apply` of `-U0` diffs (it once appended hunks at end of a file).

## Open for Kosta
Audition button disabled without keys or clock. The Sounds library keeps All sounds and Recent rows though the mockup lists only Factory, My sounds, Favourites.
