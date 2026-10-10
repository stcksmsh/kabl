# Widget-set build: checkpoint and handoff

Branch `claude/widget-set` (stacked on `claude/design-target`), worktree `.worktrees/widget-set`. Task from the Kabl overseer (relaying Kosta): build part 1 of the redesign in the real app: theme model and custom chrome widget set in theme A. **Scope, requirements and report format are in the "Task" section below; read it first.**

## State at checkpoint

Done and committed (51a1b0c, plus this checkpoint commit):

- `crates/ui/src/style/` (`mod.rs`, `builtin.rs`): the theme model from `THEMING.md`: `Style` = colour `Roles` (29), `Sections` palettes, `TypeScale` (7 roles + `section`), spacing, `Radii`, `Shadow`s, `Motion`, recipe `Slots` (all of face, knob, jack, cable, display, controls, chrome), non-themable `Metrics` (hit areas). Pure data, serde-derivable; built-in theme A (light and dark) via `builtin_a(dark)`. Unit tests: TOML round-trip, contrast pairs (4.5), 11 px floor (`TEXT_FLOOR`, `Style::font()` clamps).
- `crates/ui/src/kit/` (`mod.rs`, `controls.rs`, `containers.rs`, `icons.rs`): widgets reading only from `&Style`: `Button` (Primary/Secondary/Ghost, icon, selected, small, enabled; hover/press/focus/disabled), `icon_button`, `segmented`+`seg`, `toggle`, `slider`, `field` (themed frame around `egui::TextEdit`), `tag`, `bar`, `dropdown`+`menu_item`, `menu_button`, `context_menu`, `Tip` trait (`.tip(st, text)`), `modal`, `card`, `panel_frame`/`bar_frame`/`popover_frame`, `shadow`, `label`/`paragraph`/`section`/`rule`/`gap`, stroke icon set `Ic`, and `apply(ctx, &Style)` which installs the theme into egui visuals (once per theme change).
- `UiState.style: Arc<Style>` (rebuilt in `show()` when `dark` flips); `theme::install_fonts` now registers Plex Medium, SemiBold, Mono Medium (files in `crates/ui/assets/`); `theme::ensure_fonts` is called from `show()` so the plugin editor gets them too. The old `theme::Theme` (rack faces) is untouched.
- `crates/ui/examples/bench_chrome.rs`: frame-time bench of `show()` (simple default patch and dense `patches/composition`, both sizes, light/dark, browser + drawer open, includes tessellation).

Compiles. `cargo test -p kabl-ui --lib style` passes. **Not yet wired**: `kit` is not used by any chrome code, so behaviour is unchanged; full test suite not re-run after the `style`/fonts/`apply` hook in `show()`.

## Baselines

- Tests (debug profile; `--release` breaks `runtime_controls` because `assert_no_alloc` is disabled in release): `cargo test -p kabl-ui` at the branch point = **273 passed, 0 failed, 21 ignored**.
- Frame-time **before** numbers: run `cargo run --release -p kabl-ui --example bench_chrome` from the **design-target** worktree (it has master's UI; the bench file was copied there untracked at `crates/ui/examples/bench_chrome.rs`; delete it afterwards, it is not part of that branch). The run was started at the checkpoint; if its output is lost, rerun. Then run the same in widget-set for "after".

## Facts learned about the real app

- Chrome lives in: `lib.rs` `show()` (panels), `toolbar()` (row 1: wordmark, Sounds, Composites, kind ComboBox, Add, Undo/Redo, Cables All/Focus/Hidden, zoom −/%/+/Fit/Focus, A-light/A-dark, View menu with checkboxes; row 2: `browser::toolbar` doc name, Save, Save As, then right-to-left Routing, Perform toggles, REC indicator, last message), the Routing drawer header inline in `show()` (Help, Close, Inspect/Compare/Learn chips), `browser.rs` `panel()` (header, search TextEdit, filter chips, Category ComboBox, sound list rows, selected-sound block, `play_section` audition: −8/note ComboBox/+8, chord chips, Velocity and Length sliders, Play/Stop, Start/Stop for sequences, Patch folder CollapsingHeader), `browser::dialogs` (`egui::Modal` dialogs: Unsaved, SaveAs, Rename, Restore), and the standalone status bar in `main.rs` `App::ui` (meter, audio status, phase/session, output ComboBox, Retry audio, Log, timing summary).
- Tests drive `kabl_ui::show` with real egui events and find controls through `ui_state.record(key, rect)` / `ui_state.hits`. Keep every recorded key (`doc-name`, `save`, `save-as`, `browser`, `routing`, `perform`, `search`, `filter:*`, `category`, `category:*`, `fav:*`, `sound:*`, `open`, `rename`, `new`, `browser-close`, `note`, `note-down`, `note-up`, `chord:*`, `velocity`, `length`, `play`, `stop-preview`, `start`, `stop`, `patch-path`, `load`, `save-folder`, `folder-header`, `dlg:*`, `view:*`, `zoom:*`, `theme:*`, `menu:*`, `view-menu`, `help`, `inspect-open`, `compare-open`, `learn-open`, `audio-retry`, `log-path`, ...). `tool()` in lib.rs is the toolbar helper. Dialogs' Escape handling is in `browser::dialogs`.
- The CLAP plugin editor calls `kabl_ui::show` only (no status bar there); the status bar is standalone-only (`main.rs`).
- Toolbar is two rows (64 px) with "Perform" a bottom-panel toggle, not a view switch: the mockup's "Rack | Perform" segmented control must stay a toggle chip (behaviour unchanged).
- `egui::Popup::from_toggle_button_response`, `Popup::menu`, `Popup::context_menu`, `egui::Modal` (with `.frame`, `.backdrop_color`) exist in egui 0.35 and are used by the kit. `ctx.global_style_mut` compiles.
- Tooling quirk: the Bash guard refuses compound commands that mention the worktree path with `cd`/`&&`; use simple commands, or run scripts from files in `/home/stcksmsh/.claude/jobs/4129379d/tmp/`.

## Remaining work (in order)

1. **Wire the chrome to the kit** (keep all hit keys and behaviour):
   - `lib.rs` toolbar rows and `tool()` -> `kit::Button` chips / `icon_button` / `segmented`; kind picker -> `kit::dropdown`; View menu -> `kit::menu_button` with `kit::toggle`; Undo/Redo/zoom as icon buttons with `.tip()`; panels get `kit::panel_frame`/`bar_frame`; Routing drawer header (title, Help, Close, Inspect/Compare/Learn) -> kit; central rack fill from `st.roles.rack`.
   - Side rail (44 px icon rail from the mockup): optional; if it breaks canvas-size assumptions in tests, defer and say so.
   - `browser.rs`: header, search `kit::field`, filter chips, Category `kit::dropdown`, sound rows (card rows, star ghost button, category pill, `kit::tag`), selected-sound block, audition block (`kit::slider`, chips, dropdown, Play primary), Patch folder disclosure, notes; dialogs -> `kit::modal`, `kit::field`, `kit::Button`.
   - Module context menus / add menu in `lib.rs` -> `kit::context_menu`. Hover texts -> `.tip(st, ..)` in chrome code.
   - Status bar (`main.rs`): meter restyled from `Style` (`record::meter_ui` takes a `&Style`), audio and MIDI state, output `kit::dropdown`, Retry button, Log; **telemetry** (callback worst, late, xruns, RT, session/phase, change(s) waiting, last take) moves behind a **Diagnostics** popover (button in the status bar).
   - Apply type roles and the 11 px floor across the chrome (no `RichText::small()`, no raw `size()`).
2. Out of scope, leave stock but themed via `kit::apply`: module faces/knobs/jacks/cables, live displays, Perform panel, routing/cable pattern editor (PR #15 is changing it), composites/panel-authoring windows, explain/inspect/compare/recipes drawer bodies. List them in the report as follow-ups.
3. Verify: `cargo test -p kabl-ui` (debug) and workspace tests; `cargo clippy` strict (`-D warnings`, the workspace's own config) and `rustfmt` on touched files only; update tests that encode old widget structure without weakening them.
4. Real captures: build an example or reuse the app under Xvfb (scripts from `docs/factory-performance-bank/scripts/capture.py` and `docs/find-play-save/repair-1/record-dialogs.sh` are precedents) at 1440x900 and 1280x800, light and dark, plus one scaled display (`pixels_per_point` 1.5), with Sounds open; compare against `docs/design/app-target/media/a-satin/*rack-sounds*` and list departures with reasons. Look at the images yourself.
5. Frame time before/after (`bench_chrome`), simple and dense patch.
6. Rebase expectation: PR #15 (cable pattern editor) and PR #17 (oscillators) will merge to master under this branch.
7. Open a **separate draft PR** for `claude/widget-set` (do not push implementation to PR #16). Send one report to "Kabl overseer": PR link, what is built, verification, frame-time numbers, departures from mockups, unverified, decisions for Kosta. Do not merge. Then stop.

## Working assumptions to restate in the report (not individually signed off by Kosta)

Deep declarative theming (THEMING.md); A default, B second built-in (next part: loader, package format, B); themes not saved in songs; no fonts or code in themes in v1. Each is a contained change: the model is data, widgets read only `&Style`, `UiState.style` is the one switch point.

## Task (as received, condensed)

Build the theme model in code (slot structure from THEMING.md, A light/dark built in; no hard-coded colour, size, radius, shadow or timing in the new widgets; no loader, no package format, no B). Replace every stock egui widget in the application chrome per `stock-widget-inventory.md`: toolbar/transport (patch name, undo, view controls), Sounds browser (search, filters, cards, tags, audition), side rail and panel frames, status bar with telemetry behind a Diagnostics view, dialogs, menus, tooltips, text entry. Type scale and 11 px floor across the chrome. Behaviour must not change; every action stays reachable; existing interface tests keep passing (update structure-encoding tests without weakening). Works in standalone and CLAP editor, at 1440x900 and 1280x800, light and dark, and a scaled display. Focus, hover, disabled states on every widget. Measure frame time before/after (simple and dense patch). Compare real captures against the A mockups, list departures. Workspace tests, strict clippy, rustfmt on touched files. Short comments only. Out of scope: face/knob/jack polish, live displays, Perform redesign, dense-patch and cable legibility, wavetable/step-cable visuals, the routing cable pattern editor.

## Addendum

The "before" bench run (design-target worktree) writes to `/tmp/claude-1000/-secondary-Programming-Github-kabl--worktrees-design-target/4129379d-ce4b-4c8b-a7f7-dbffc0122a92/tasks/bgswjkkr4.output`; copy its numbers here if present, else rerun. Draft PR for this branch: #18 (base claude/design-target).

### Frame-time BEFORE (master UI, release, `bench_chrome`, browser + drawer open, 300 frames incl. tessellation)

```
simple  1440x900 light median 762 us  p95 795  max 847
dense   1440x900 light median 4642 us p95 5037 max 5261
simple  1440x900 dark  median 512 us  p95 540  max 560
dense   1440x900 dark  median 4627 us p95 5377 max 5819
simple  1280x800 light median 496 us  p95 532  max 565
dense   1280x800 light median 4135 us p95 4924 max 5296
simple  1280x800 dark  median 503 us  p95 540  max 582
dense   1280x800 dark  median 3689 us p95 4302 max 4699
```

The untracked bench copy in the design-target worktree was deleted.

## Checkpoint 2 (chrome wired)

Committed and pushed on `claude/widget-set`. `cargo test -p kabl-ui` (debug): **276 passed, 0 failed, 21 ignored** (baseline 273 plus the 3 style tests).

### Done
- `lib.rs`: toolbar (rows, `tool`/`tool_small`/`tool_icon`, kind dropdown, segmented cables and theme, View menu with toggles), panel frames, Routing drawer header (one row: title, Help, Inspect, Compare, Learn, close icon), module/bank/cable context menus, rack fill from `st.roles.rack`.
- `browser.rs`: toolbar items, header, `kit::field` search, filter chips, category dropdown, card rows (`sound_row`), selected-sound block, audition (two rows of chips, `kit::slider`, Play primary), patch-folder disclosure (`Browser.folder_open`), all four dialogs on `kit::modal`.
- `main.rs`: status bar (meter from `Style`, audio status, output dropdown, Retry, Log, Diagnostics popover holding phase/session, waiting changes, output settings, last take, timing).
- `perform.rs::module_menu`: Pin / MIDI submenus use `kit::submenu`.
- Fonts: `Style.fonts_ready` falls back to plain families until egui applies the installed fonts (fixes a first-frame panic in tests).
- Layout facts: `toolbar_h` stays 64 (row 2 uses small 24 px buttons); `bar_frame` vertical margin `sp(1)`; browser list reserve is `available_height - 440`.

### Not done
- Side rail (deferred: would change canvas size assumptions).
- Captures: release binary builds; scripts in the job tmp dir (`cap/run.sh`, `drive.py` + Xvfb + openbox) worked once. First capture found and fixed: clipped toolbar row 2, tofu star glyph, slider readout overflow, audition chip overflow, field text alignment. Needs a re-run at 1440x900, 1280x800 light/dark and a scaled display (`WINIT_X11_SCALE_FACTOR=1.5`), compared with `media/a-satin/*rack-sounds*`.
- Frame times after (`cargo run --release -p kabl-ui --example bench_chrome`).
- Strict clippy and rustfmt on touched files.
- Draft PR #18 exists; overseer report not sent.
- Out of scope, still stock: module faces, drawer bodies (explain, inspect, compare, recipes, param panel), Perform panel, seq/cue combo boxes, Composites windows.

## Checkpoint 3 (verified)

- Tests (debug): 276 passed, 0 failed, 21 ignored. `cargo clippy -p kabl-ui --lib --bins -- -D warnings` clean. rustfmt run on touched files.
- Captures (real app, Xvfb): `media/widget-set/` at 1440x900, 1280x800 (light/dark: rack-sounds, diagnostics, save-as) and one scaled (1.5) light.
- Frame time AFTER (release, same bench as BEFORE):

```
simple  1440x900 light median 908 us  p95 1277 max 1640   (before 762)
dense   1440x900 light median 6628 us p95 8326 max 10883  (before 4642)
simple  1440x900 dark  median 635 us  p95 902  max 1463   (before 512)
dense   1440x900 dark  median 5785 us p95 8036 max 12005  (before 4627)
simple  1280x800 light median 564 us  p95 740  max 997    (before 496)
dense   1280x800 light median 4439 us p95 4761 max 5522   (before 4135)
simple  1280x800 dark  median 529 us  p95 571  max 924    (before 503)
dense   1280x800 dark  median 4369 us p95 4749 max 5779   (before 3689)
```

Departures from the A mockups: no side rail; no Rack|Perform switch (Perform stays a toggle chip); no prev/next sound arrows in the doc name; doc name is a plain label beside small Save buttons; no patch-folder row visible at 1280x800 with Perform open; the rack, faces, knobs, Perform panel and drawer bodies are unchanged (out of scope).

## Checkpoint 4 (overseer review fixes)

- Toolbar row 2 was clipped at 64 px: row gap now 0 (28 + 24 px rows + 8 px margins). Verified in a fresh capture.
- Doc name is a framed control (inset fill, border, 160 px).
- Sounds panel body is a vertical scroll area, so "Patch folder (advanced)" stays reachable at any height; the duplicate audition help paragraph is gone (the Play tooltip carries it); list height is `body_h - 520`.
- Short list at 1280x800 (about 2.5 rows, min 120 px) is a trade-off: the existing test `every_control_is_reachable_at_both_sizes` needs every control on screen without scrolling. Decision for Kosta: relax that test to allow scrolling and give the list more room.
- Box glyphs in the Perform panel are egui's painted checkboxes (stock, unchanged from master), not missing font glyphs.
- Captures with Composition/Echo open Perform and Routing by themselves (the app opens them for pieces); that is app behaviour.
- Tests 276 passed, 0 failed, 21 ignored; clippy clean.

## Checkpoint 5 (task A complete, rebased on master)

Branch rebased onto `origin/master` (bb70204: PR #16, #17, #19 merged); PR #18 retargeted to master.

- Toolbar is one 48 px row: logo, Perform toggle, doc name, Save, Save As, undo/redo, Cables All/Focus/Hidden, then Sounds and Routing on the right (last message and REC indicator truncate to the room left).
- 44 px right rail (`rail()` in `lib.rs`): Composites toggle, Add module popover (kind dropdown + Add), zoom +, %, -, Fit, Focus, and at the bottom a settings popover (A-light/A-dark and the View toggles). Hit keys unchanged except `add-menu` (opens the Add popover) and `view-menu` (now the gear). `kit::icon_menu` is the new popover-button widget.
- Sounds list height is `body_h - 330` (min 300); audition and patch folder sit below it in the scrolling panel body. `browser-body` is recorded so tests can scroll to controls. `every_control_is_reachable_at_both_sizes` now scrolls panel-body controls into view and operates them; the toolbar/header keys must still be on screen unscrolled. The test harness `click` scrolls the body first.
- Canvas-size tests subtract the rail width (`Metrics::default().rail_w`).
- Logo: Kosta's SVGs under `crates/ui/assets/logo/` (README there). The wide lockup, rendered white to a PNG mask and tinted from `roles.text`, is used at 30 px height (`kit::logo`); the small mark SVG is stored, unused.
- Verification at this head: `cargo test --workspace` 728 passed, 0 failed, 28 ignored. Strict clippy (`-D warnings`) clean for all lib, bin, test and example targets except `crates/ui/examples/app_target/*`, which already fails on master (mockup renderer from PR #16, not touched here). Plugin editor checked in REAPER 7.75 under Xvfb: media/widget-set/plugin/.
- Plugin: editor is fixed at 1180x680 logical (resizing the host window does not resize it), so 1440x900 and 1280x800 apply to the standalone only. No status bar in the plugin (standalone `main.rs` only): audio and device state belong to the host; the last message and REC indicator are in the toolbar. Light and dark both render; Sounds panel opens; settings popover works.
- Frame times after (release, `bench_chrome`, browser + drawer open): simple 1440x900 light 507 us, dark 514; dense 1440x900 light 4464, dark 4865; simple 1280x800 light 522, dark 498; dense 1280x800 light 2919, dark 2901. (Before: see above. Not optimised; that follows Perform.)
- osc.fm6 face (PR #19): the Algorithm and Ratio selector strips draw overlapping labels at 100% zoom. The same overlap appears on a master build (bb70204), so it is not from this branch. Face is wider than the canvas at the 50% minimum zoom.

### Departures from the A mockups (rack-sounds scenes)
- No Rack|Perform segmented control: Perform stays a toggle chip (bottom panel, not a view switch).
- No prev/next arrows beside the document name.
- Rail icons differ: mockup shows panel, routing, composites, learn; ours holds composites, add, zoom, settings (Sounds and Routing stay in the toolbar; Learn stays in the Routing drawer header).
- At 1280x800 the Audition block is below the fold of the Sounds panel (scroll); the mockup fits list and audition together. Audition is not a bordered card; Note/Major/Minor chips and the extra rows differ from the mockup's Note/Chord pair.
- Wide lockup in the toolbar is 90x30 px: the plugs are small details at that size.
- Rack, faces, knobs, Perform panel, drawer bodies are unchanged (out of scope).
- The standalone status bar has no MIDI device line (the mockup shows "MIDI · KeyLab 49" and a voice count).

### Unverified
- The scaled (1.5) run is one capture at light; dark scaled not looked at.
- Plugin on other hosts and the Wayland path.
- Real MIDI/audio devices (captures run without them).
