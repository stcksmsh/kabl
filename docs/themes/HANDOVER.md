# Theme loader, theme B, font, capture tooling: handover

Branch `claude/theme-b` (from origin/master), worktree `.worktrees/functional-cables`. Report to "Kabl overseer", do not merge, stop when done. Caveman and Ponytail ultra in chat; code, commits and PR text normal English. Stay under 50% context.

## Task as received (condensed)
Build a theme loader (declarative files, untrusted input: validate, cap size, fall back per value with a message, partial themes work); live theme switching in the interface, remembered across launches for standalone and plugin; theme B "Lacquer and Light" built to `docs/design/app-target/media/b-lacquer/` with every slot filled, then check every surface under B and list hard-coded spots (fix small Style substitutions; report, do not fix, anything that means reworking drawing in `rack.rs`, `kit/face.rs` or cable drawing); contrast test (WCAG AA) for both built-ins; drop the egui default fonts and use an OFL typeface (overseer: keep IBM Plex, no alternative candidates unless Kosta asks); fix the capture script's dark step ("no target theme:dark") so any theme can be captured by name and re-take the effect-module dark captures. Out of scope: theme editor, a third theme, cable pattern editor restyle beyond following the theme, cable management, effect-module live displays, presets. Constraints: frame time unchanged (median < 8 ms, max < 16 ms), behaviour unchanged, no new dependency unless needed, CI green (fmt, strict clippy, tests, package, validator, licenses), check in the standalone and in the plugin at 1180x680 in REAPER. Agents 1 (cables, About box) and 2 (wavetable editor) work in `rack.rs`, `kit/face.rs`, cable drawing: keep edits there minimal; merge origin/master before the end.

## Done
See `git log claude/theme-b`. Loader and theme B (`crates/ui/src/style/{load,builtin}.rs`), switching and persistence (`UiState::use_user_prefs`, settings menu), default fonts off and glyph test (`theme_tests.rs`), tabs and accent-pill recipes, theme-aware face surfaces, capture tooling (`docs/themes/scripts/`), docs (`THEMING.md` section 13, `docs/themes/README.md`), example user theme.

## Remaining
Nothing assigned beyond the final report to the overseer: what was built, format and folder, hard-coded list (fixed and not), contrast, bench, test totals, capture paths, CI link, gaps. Then stop.

## Facts that cost effort
- The "no target theme:dark" failure did not reproduce on current master: `click view-menu` then `click theme:dark` works with the release binary. The robust route is `KABL_THEME=ID:dark` (no menu), used by `docs/themes/scripts/capture.sh`.
- Bash guard in a worktree session refuses heredocs and compound commands: use Write/Edit and plain commands. Python heredocs sometimes pass, sometimes not.
- egui `Fonts::has_glyph` is unreliable with a single embedded font (the replacement face is the first face), so the glyph test reads the TTF cmaps with `skrifa` (dev-dependency, already in the lock file).
- REAPER host check: copy of the earlier faces job's `start.sh` using Xvfb :248, a throwaway `KABL_CONFIG_DIR` with a `settings.toml`; close the VST scan dialog (Cancel) and the update notice with xdotool.

## Working assumptions and open decisions for Kosta
Theme choice is global per user (not per plugin instance, not in the patch). Slots no drawing code reads yet: `cable.*` (recipe, width, sag, glow, dimmed), `plug`, `face.out_plate`, `chrome.navigator`, `controls.row`, `rack` surface, `icons`. They are filled for B, but changing them has no visible effect until the drawing code reads them (cable drawing is agent 1's).
