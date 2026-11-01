# G1b handover (agent A, kabl_mock)

Standing rules: report to session "Kabl overseer" (SendMessage to its `from` address), never merge, draft PR only, stop when done. Style: caveman + ponytail ultra in chat; code, commits, PR text normal English. No bare `git stash`; single plain shell commands (worktree guard rejects compound ones with `$PWD`, `cd &&` chains with variables). `cargo clean` at the end. Checkpoint at 50 % context.

## Task as received
Phase G1b (PLAN §9.1, §3, §4, §6; verdicts.json all approved). Branch `claude/ui-g1b-mock` from `origin/claude/ui-g0-checker` (created, worktree `.claude/worktrees/fix-flaky-alloc`).
- Add `crates/ui/examples/kabl_mock.rs`: render with the real egui painter the 7 R1 faces (PLAN §4 "Representative faces": VA Osc 240, SVF 210, ADSR 210, Mixer 240, Sequencer 660, Drum Grid 660, FM Voice 660 as operator cards) plus a parts sheet (knob, selector, toggle, jack mono/stereo, cable, family marker / stripe, tray two_row / single_row_wide, drum cell). Face style `line_solid` x app themes `graphite` (dark) and `paper` (light), accent amber, at zoom 100 / 75 / 58. Approved: jack rows y 273/316, single-row tray on wide faces, compact tier text held at 9.5 px (short names, values and captions dropped).
- Mock-only: tokens and layout live in the example (or a mock module behind it). Do NOT change `style/*`, `kit/*`, `theme.rs`, `rack.rs`, `lib.rs` (that is G2).
- Embed Plex Sans Condensed (OFL) only if the mock needs it; licence file + note in the PR.
- Run the G0 checker rules on the mock's rects: 0 errors at 100/75/58.
- Output: one PNG sheet per app theme (graphite, paper) plus side by side vs `docs/design/redesign/mockups/r1-faces.html` (headless Chrome screenshot at 100/75/58) so Kosta sees target | egui. Commit PNGs under `docs/design/redesign/g1b/`.
- PR body: the two sheets, checker line (`errors N · warnings M`), differences vs the HTML mockup that could not be matched and why. fmt + clippy -D warnings; push; DRAFT PR (base master); report PR number to the overseer. Kosta signs off on the sheets before any G2.

## Done
Nothing yet except branch + this file. G0 is PR #71 (merged state unknown; branch based on it).

## Remaining, in order
1. Read `mockups/r1-faces.html` lines 205-520 (`buildFace`: zone y values, knob/seg/jack/tray/trayWide helpers, `fit()` short-name rule, `check()`), 547-680 (defs, parts sheet). Layout constants are at line 137 (`Z`) and sizes at 305 (`R={L:24,M:20,S:16}`). Face specs (names, short names, values, ports) are at lines 172-196: copy them. NOTE: that HTML is Satin-styled (superseded look); take geometry from it, look from PLAN §3/§6 (`line_solid`, tokens in §6) and `mockups/directions.html` ("Signal solid" / "Drafting solid").
2. Write `kabl_mock.rs`: headless render is the problem. egui has no offscreen rasteriser here. Options: (a) run eframe window and screenshot via `ViewportCommand::Screenshot` (needs a display: use xvfb or the real X session, silent, no audio), or (b) emit `egui::Shape`s into an own software rasteriser. Pick (a): `eframe` glow is in the deps; screenshots arrive as `Event::Screenshot`; save PNG with the `image` crate (already a dep, png feature). Check `crates/ui/examples/` for an existing capture example (bench_*, `capture`?) first with `ls crates/ui/examples`; reuse if present.
3. Checker on mock rects: reuse the rule code from `crates/ui/tests/geometry.rs` (copy the small overlap/gap/edge functions into the example; do not import test code). Print `errors N · warnings M` per zoom.
4. PNGs: `target/mock/{graphite,paper}.png` then copy to `docs/design/redesign/g1b/`. HTML reference: `google-chrome --headless=new --no-sandbox --disable-gpu --hide-scrollbars --window-size=W,H --screenshot=OUT file:///.../r1-faces.html` (works; set zoom/theme via localStorage key `kabl-r1` or URL if the page supports it, else a tiny wrapper HTML). Build a side-by-side PNG (image crate or an HTML page + screenshot).
5. fmt, `cargo clippy -p kabl-ui --examples -- -D warnings`, commit, push `claude/ui-g1b-mock`, `gh pr create --draft --base master`, body per above, SendMessage to overseer, `cargo clean`.

## Facts that cost effort
- Headless text measure in egui 0.35: `ctx.begin_pass(RawInput{screen_rect,..})`, `theme::install_fonts(&ctx)`, run two empty passes so fonts apply, then `Painter::new(ctx.clone(), LayerId::background(), Rect::EVERYTHING).layout_no_wrap(text, FontId, color)` inside a pass. `Context::run` does not exist (`run_ui` does).
- Fonts: `theme::install_fonts`; families `kabl-sans-medium`, `kabl-sans-semibold`, `kabl-mono-medium`; Plex Regular/Medium/SemiBold sans + Regular/Medium mono embedded in `crates/ui/assets/`. Condensed is not embedded.
- Text boxes: use glyph extent (0.85 em tall), not the galley line box, or label/value pairs flag as touching.
- `bench_chrome` already has 1440x900, 1280x800, 1180x680. `google-chrome` is at /usr/bin. Audio: this example plays nothing; no sink needed.
- `cargo` builds fill disk (6 GB per worktree); `cargo clean` at the end.

## Assumptions / open for Kosta
- Sheets use the real egui painter via eframe screenshot (a software-rendered look differs slightly from GL AA).
- Condensed font omitted unless titles look wrong without it; if embedded, add OFL text to `crates/ui/assets/FONT-LICENSE.txt` and `ASSET-LICENSES.md`.
