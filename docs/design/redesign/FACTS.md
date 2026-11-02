# Facts checked before G0 (2026-10-31, against origin/master 6e92737)

The brief and plan mark four items as unverified. Findings:

1. **`bench_chrome` exists.** `crates/ui/examples/bench_chrome.rs` has been in the tree since #35. Usage per `docs/themes/README.md`: `KABL_BENCH_THEME=<id> cargo run --release -p kabl-ui --example bench_chrome 300 live` (dense Composition, 1440x900, rack only). Other benches: `bench_composition`, `bench_palette`, `bench_palette_parts`, `bench_performance`, `bench_reference`, `bench_echo`. PLAN §9.8's "A creates it if missing" is not needed; A may need to add a 1180x680 mode.
2. **Theme file format** (`crates/ui/src/style/load.rs`): TOML, at most 64 KiB, at most 64 user files, `ui_api = 1`. A file is an overlay on a built-in named by `extends` (`kabl.satin` or `kabl.lacquer`, default satin). Unknown, wrong-type or out-of-range values are dropped with a note. A mode whose text contrast is below 3.0 is dropped whole; below 4.5 is only reported. Tables: `[light|dark.roles]`, `[light|dark.sections.<family>]` (base, ink, ink2), `[light|dark.slots.<part>]` (recipe choices, for example knob `body`, `scale`). Example: `docs/themes/example/midnight-rose.toml`. The 3-layer model (app theme × face style × accent) needs a new vocabulary version or new tables; `ui_api` 1 files must keep loading.
3. **PR #55** (visual polish stage 2a, `claude/visual-polish-2`, e71ce2c) is open as a draft, last updated 2026-10-28, parked by the owner. The redesign supersedes it; do not build on it. Close or leave it as the owner decides.
4. **seq-32** (`claude/seq-32`, dd33766, pushed, no PR yet): 32 steps per pattern, length 1 to 32, face shows pages of 8; seq param cap 655; per-step params are never automation-lane targets. Agent 2 is finishing it. G3 timing/drums (agent D) must build on it after it merges.

Also present on master: an older design set in `docs/design/` (BRIEF.md, app-target/, directions/, revision-2/, REVIEW.md, QA.md). `docs/design/redesign/` is the current one.

## Toolbar metrics in `mockups/directions.html` (read from `chrome()`, 2026-11-02)

The mockup draws SVG, so these are the drawn values (1 unit = 1 px at the 1440 frame; the sheet frame is 1392 wide, so measured screenshots read about 3 % smaller):

- Toolbar 48 px; 1 px bottom line. Logo tile 24x24, radius 6, at x 16; wordmark "kabl" 17 px, weight 600.
- Sound selector 28 px tall, 210 wide (170 below 1300 px), radius 6: kicker mono 10 px weight 500, name 13 px weight 500, dirty dot r 3, chevron.
- View switch: track 28 px tall, 3 x 86 = 258 wide, radius 7; selected segment 24 px tall inside it (2 px inset), radius 5; label 13 px, weight 600 selected, 500 otherwise.
- Add module: 180x28, radius 6, search icon, 13 px placeholder, key hint 20x16 radius 4 with a mono 10 px letter. Hidden below 1300 px.
- Icon buttons 28 wide (16 px icon), 4 px apart. Cable tools: 3 x 30 = 90 wide, 28 tall track, 26x24 segments. Zoom text mono 12 px weight 500.
- Save: 28 tall, label width + 28, 13 px weight 600, radius 6.
- Perform shows one icon (MIDI), Sounds one icon (heart), both left of Save.
