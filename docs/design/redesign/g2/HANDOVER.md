# G2 handover (agent A, kit and style)

Standing rules: report to session "Kabl overseer" (SendMessage to its `from` address), never merge, draft PRs only, base on master, stop when done. Style: caveman + ponytail ultra in chat; code, commits, PR text normal English. Plain single shell commands only (the worktree guard rejects compound commands that mention git/gh with variables; use the Write tool plus `python3 script.py` for multi-line edits). `cargo clean` at the end. Measure context with the one-liner in CLAUDE.md; checkpoint at 50 %.

## Task as received
G2 (PLAN §3, §6, §7 incl. Cables, §9.1 G2, §9.6-9.8, §10 A-kit; verdicts.json). Own `style/*`, `kit/*`, `theme.rs`, `cable_look.rs` (paint only), `rack/` tables. Do not touch `perform*.rs`, `browser.rs`, editors; `lib.rs` only the approved 2-line title-ink edit. Three-layer themes, `ui_api` 2 vocabulary, recipes, old themes stay default until G7, acceptance: app parts sheet matches G1b within a pixel threshold, G0 checker no new errors, bench not worse, fmt/clippy/tests, saved racks and presets load unchanged. Cables: port `cable_look.rs` and theme it, no redesign (PLAN §7 "Cables").

## Done
- #73 (merged): `rack.rs` -> `rack/` with osc/timing/amp/fx tables.
- #76 (draft, `claude/ui-g2-kit`, base master): `style/solid.rs` (graphite/paper x line_solid/outline x accent), `ui_api` 2 keys `accent` / `face_style`, new role `pitch`, recipes (Line face, CondCaps title, Solid knob, Hex + stereo jack, family ink/stripe, `CompactText::Hold`, tray), `kit/parts.rs` (selector, toggle, chip, drum cell, tray, cable, beads), cable paint tokens on `CableSlots`, Condensed embedded app-wide, `examples/kit_parts.rs`, `g1b/parts_diff.py`. Parts sheet diff vs G1b mock 0.48 % graphite, 0.83 % paper. Images in this folder.

## Remaining, in order
1. **cable_look.rs wiring.** `CableSlots.style` (`Classic` | `Themed`) and tokens exist; `kit::parts::cable` is the themed paint. Make `cable_look.rs` read them when `Themed` (paint only; geometry, sag, drag, beads, plug logic unchanged): two passes (all glows, then all cores), core >= 2.5 px, stereo centre >= 1.5 px, stereo = twin strand + double-ring plug, functional = beads (filled on, hollow off, current step in the accent), Focus: others 25 %, selected glows (dark) or thickens (light). `Classic` must stay pixel-identical to today.
2. **Cable capture set** (PLAN §7 Cables): simple, Composition, stereo, functional x classic vs both defaults x 100 / 58 % x 1440x900 and 1180x680, for Kosta's side-by-side. If the new look loses anywhere, ship classic with the new tokens only.
3. **Mesh cache** per (cable, zoom, theme) and per (theme, width, zoom tier) for faces; then PLAN §9.8 gate (below).
4. **Face styles `graphite` and `aluminium`** (basic). They need a per-style face fill and ink (the faces use `roles.surface` and `roles.text` today), so add a `FaceSlots` fill/ink slot first; `theme.rs::theme()` maps it.
5. **Quiet-box bench re-run** (`KABL_BENCH_THEME=kabl.line_solid cargo run --release -p kabl-ui --example bench_chrome 300 live`; also `kabl.satin`). Gate: Composition median < 8 ms, max < 16 ms at 1440x900, and 1180x680. Baseline (G0, quiet): 8.7 / 16.7 ms and 5.3 / 9.2 ms (light/dark medians 8.7 / 9.1 and 5.3 / 5.3). My runs on 2026-11-01 ran at load average 20-60: line_solid 8.3 / 10.1 and 5.1 / 5.5 ms medians, not trustworthy.
6. Faces do not call the tray, stripe or stereo-jack recipes yet: that is G3 (family agents) and G6.

## Token proposals (all need Kosta; status)
- Approved (tentative, confirm at sign-off): paper amber `#A97229`; paper title-hue mix amp 45 % / fx 40 % / others 35 %; dark filled knob caps; graphite cap light end unchanged.
- **Pending Kosta:** paper signal colours on the paper rack (`#DCD9D2`) are below 3:1 (audio 2.24, cv 2.62, pitch 2.91); `solid.rs` darkens them to audio `#B66518`, cv `#11897F`, pitch `#2E7CD2` (gate `#7651D6` unchanged). Swatch: `paper-signals.png`. If Kosta rejects, change the `sig` closure in `style/solid.rs::roles` and the swatch.
- Jack nut uses `raised`/`line2` (graphite `#232529` vs the mock's `#2B2D32`): small diff, no decision needed unless Kosta wants the mock's value.

## Facts that cost effort
- Parts sheet capture: `xvfb-run -a -s "-screen 0 1100x800x24" target/release/examples/kit_parts graphite|paper out.png`; diff with `python3 docs/design/redesign/g1b/parts_diff.py graphite|paper` (needs `target/mock/kit-<theme>.png` and the committed `g1b/sheet-<theme>.png`; the mock's parts panel is at y 3503).
- New enum variants break exhaustive `match` in `kit/face.rs` and `kit/recipes.rs`; new `Roles` fields need all four literals in `style/builtin.rs`.
- The user theme overlay (`style/load.rs`) is generic over the serialised `Style`, so new slot fields need no vocabulary code except clamps in `clamp()`.
- Full `cargo test -p kabl-ui` takes about 4 minutes (one test is 195 s). Builds are 26 GB: `cargo clean` at the end.
- Machine load is high (a training job runs); do not trust bench numbers unless `uptime` shows load under 4.
