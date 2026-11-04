# G4 owner E (editors) handover, 2026-11-03

## Task as received (from "Kabl overseer")
Agent 2, G4 owner E. Scope: wavetable bottom dock (`tables.rs`), seq pattern editor and cues editor (`seq_editor.rs`, split out of `lib.rs`), cable pattern editor (`pattern.rs`), routes (`routing.rs`). One surface group per PR, new branches from `origin/master`, draft PRs, never merge. Acceptance per PR (G4-SPEC §6): graphite and paper captures at 1440x900 and 1180x680 next to `g4-surfaces.html` in the mockup's state, no overlap or clipping, contrast at least 4.5:1, no stock egui widgets, Esc/Tab/Enter, perf gate. Run fmt --check, clippy --all-targets -D warnings and `cargo test -p kabl-ui` before every push. Report one line per PR. `lib.rs` is shared with F (chrome): keep hunks there small.

## Done
- #100 split seq/cues editors into `seq_editor.rs`: merged.
- #101 wavetable bottom dock: merged.
- #103 seq pattern editor in the drawer: merged.
- #104 cues editor: merged. Cue cards, Launch for all cues, per-cue settings when a card is tapped; `cues::standing` keeps the cue you hear Playing while another is queued.
- #105 cable pattern editor: merged.

- #107 `claude/ui-routes` d8cc81b9, draft: routes drawer (route cards, `kit::slider_bipolar`, range bar, Add source menu; depth routes, cable routes and lists behind More while a control is inspected), sheets added.
- #108 `claude/ui-cues-face` e31198f6, draft: G3 cues face, seq Cancel off the header (the queued bank cancels; Cancel in the drawer), cue drag reorder (`cues::reorder`), `Timing::NextTwoBars` ("2 bars"). render_hash over 129 patches identical.

## Remaining, in order
1. Stopped by Kosta: answer review on draft PRs #107 (routes, lanes kept and animated, clips added) and #111 (G4-F group 3: menus, More controls with Face layout tab), merging `origin/master` in when they conflict (usually `SCENES` in `chrome_capture.rs`: keep every scene).

## Facts that cost effort
- Sheets: `/tmp/sheetjs/sheet.py MOCK APP OUT` (mockup scaled to the app size, side by side). Not in the repo.
- A kit slider with a step snaps a value set elsewhere and reports `changed` every frame: only treat a press or arrow key as an edit.
- egui `Frame` size includes the stroke width: keep one width for selected and plain cards.
- `cargo test` stops at the first failing test binary: use `--no-fail-fast`.
- Retargeting a PR base via `gh api` was blocked by the permission check.
- Mockup renders: `/tmp/mockjs/shot.js` (Playwright from `/secondary/Programming/Github/EmojiType/node_modules`, `executablePath: '/usr/bin/google-chrome'`). Call `window.__set({surf, app, size: '1440'|'1180', mode: 'one'})`, screenshot `#out svg`, then resize to the window size. Surface keys: `wavetable`, `seq-editor`, `cues-editor`, `cable-pattern`, `routes`.
- Captures: `xvfb-run -a -s "-screen 0 1440x900x24" cargo run --release -p kabl-ui --example chrome_capture -- OUT <scene>` with `XDG_CONFIG_HOME=/tmp/wtcfg PIPEWIRE_NODE=kabl_agents PULSE_SINK=kabl_agents`. The mockup's rack zoom: set `self.ui.zoom` at `f == 6` (0.95, or 0.78 below 1280). Put extra modules at y 640 so they sit below the view.
- On master, the drawer shows `show_param_panel` only under Inspect. The seq and cues editors also show under Routing through `module_editor` in `drawer()`, and take the whole drawer.
- `cargo test -p kabl-ui` takes about 10 min after a clean: run it in the background, not under a 590 s timeout.
- Perf: `bench_chrome 300` with `KABL_BENCH_SIZES=1` and `KABL_BENCH_THEME`. line_solid against satin swings ±7 % between rounds on master itself.
- `text3` is 3.3:1 on surface and fails the 4.5 gate: use `text2` for secondary text.
- There is no `explain::Subject` for cables.
- `gh pr edit` fails: use `gh api -X PATCH repos/stcksmsh/kabl/pulls/N -F body=@file`.

## Open decisions for Kosta
- #107: the range bar centre prints the live value (kit spec), the mockup the span; cards keep a Pattern icon.
- #108: LAUNCH keeps Now as a fourth option (the mockup shows three).
- Pitch lane: bars span ±12 st (the mockup's scale). Steps beyond ±12 show as a full bar.
- Cable pattern: the mockup's single bar is the chance; here the height is the level and the chance sets the bar's strength. The Steps length slider stays.
- The cable pattern keeps two bars per step (level, and chance below), where the mockup draws one.

## Standing rules
Report to "Kabl overseer", draft PRs only, do not merge, stop when done.
