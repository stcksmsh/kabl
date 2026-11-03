# G4 owner E (editors) handover, 2026-11-03

## Task as received (from "Kabl overseer")
Agent 2, G4 owner E. Scope: wavetable bottom dock (`tables.rs`), seq pattern editor and cues editor (`seq_editor.rs`, split out of `lib.rs`), cable pattern editor (`pattern.rs`), routes (`routing.rs`). One surface group per PR, new branches from `origin/master`, draft PRs, never merge. Acceptance per PR (G4-SPEC §6): graphite and paper captures at 1440x900 and 1180x680 next to `g4-surfaces.html` in the mockup's state, no overlap or clipping, contrast at least 4.5:1, no stock egui widgets, Esc/Tab/Enter, perf gate. Run fmt --check, clippy --all-targets -D warnings and `cargo test -p kabl-ui` before every push. Report one line per PR. `lib.rs` is shared with F (chrome): keep hunks there small.

## Done
- #100 split seq/cues editors into `seq_editor.rs`: merged.
- #101 wavetable bottom dock: merged.
- #103 seq pattern editor in the drawer: merged.
- #104 cues editor: merged. Cue cards, Launch for all cues, per-cue settings when a card is tapped; `cues::standing` keeps the cue you hear Playing while another is queued.
- #105 `claude/ui-cable-pattern` 2eb0247c, draft, master merged in. The cable pattern editor: one bar per step (height is the level, strength the chance, Shift-drag sets the chance), Pattern B and the patch routes behind More, and `cable_param_routes` with "Moved by" chips and a "+ Source" menu. The cable list is hidden while a cable is open.

## Remaining, in order
1. If #105 conflicts again, merge `origin/master` into it. A conflict is usually the `SCENES` list and the `drawer_open`/`f == 6` lines in `crates/ui/examples/chrome_capture.rs`: keep every scene.
2. Routes PR (`cable-routes-drawer`, last surface group). Branch from master. Target in `g4-surfaces.html` (`SURF['routes']`, render with the script below): header target + Explain; Base field + Default link; `kit::g4::range_bar`; route cards (source chip, invert/bypass/remove icon buttons, bipolar depth slider, mono value, selected card in an accent border); full-width "Add source"; footnote on ranges. Code: `routing::drawer`, `routing::cable_routes`, `cable_route_row` in `crates/ui/src/routing.rs`. Add a `cable-routes-drawer` scene to `chrome_capture`.
3. Follow-up PR, which Kosta approved through the overseer:
   - restyle the Cues face (`draw_cues`/`pad` in `lib.rs`) to the G3 kit, with accent for queued and playing and neutral chips;
   - move the seq face's "Cancel" chip out of its header (`draw_banks`);
   - cue reordering: drag grips on the cards, a reorder op in `cues.rs`, one undo step;
   - a "2 bars" launch quantise in the engine (`kabl_engine::patch_engine::Timing`, `banks::TIMINGS`), with LAUNCH reading Now / Step / Bar / 2 bars. The audio of existing patches must stay bit-identical: prove it with render hashes before and after.
4. `cargo clean` in the worktree when stopping.

## Facts that cost effort
- Mockup renders: `/tmp/mockjs/shot.js` (Playwright from `/secondary/Programming/Github/EmojiType/node_modules`, `executablePath: '/usr/bin/google-chrome'`). Call `window.__set({surf, app, size: '1440'|'1180', mode: 'one'})`, screenshot `#out svg`, then resize to the window size. Surface keys: `wavetable`, `seq-editor`, `cues-editor`, `cable-pattern`, `routes`.
- Captures: `xvfb-run -a -s "-screen 0 1440x900x24" cargo run --release -p kabl-ui --example chrome_capture -- OUT <scene>` with `XDG_CONFIG_HOME=/tmp/wtcfg PIPEWIRE_NODE=kabl_agents PULSE_SINK=kabl_agents`. The mockup's rack zoom: set `self.ui.zoom` at `f == 6` (0.95, or 0.78 below 1280). Put extra modules at y 640 so they sit below the view.
- On master, the drawer shows `show_param_panel` only under Inspect. The seq and cues editors also show under Routing through `module_editor` in `drawer()`, and take the whole drawer.
- `cargo test -p kabl-ui` takes about 10 min after a clean: run it in the background, not under a 590 s timeout.
- Perf: `bench_chrome 300` with `KABL_BENCH_SIZES=1` and `KABL_BENCH_THEME`. line_solid against satin swings ±7 % between rounds on master itself.
- `text3` is 3.3:1 on surface and fails the 4.5 gate: use `text2` for secondary text.
- There is no `explain::Subject` for cables.
- `gh pr edit` fails: use `gh api -X PATCH repos/stcksmsh/kabl/pulls/N -F body=@file`.

## Open decisions for Kosta
- Pitch lane: bars span ±12 st (the mockup's scale). Steps beyond ±12 show as a full bar.
- Cable pattern: the mockup's single bar is the chance; here the height is the level and the chance sets the bar's strength. The Steps length slider stays.
- The cable pattern keeps two bars per step (level, and chance below), where the mockup draws one.

## Standing rules
Report to "Kabl overseer", draft PRs only, do not merge, stop when done.
