# G3 agent B handover (osc / filter / env), next: FM Voice

Standing rules: report to session "Kabl overseer" (SendMessage to its `from` address), never merge, draft PRs only, never push to master. Chat style: caveman + ponytail ultra; code, commits, PR text normal English. The worktree guard rejects compound shell commands that mention git/gh or use shell variables: use plain single commands and the Write tool + `python3 script.py` for multi-line edits; shell loops with a variable that reaches a program are refused.

## State (2026-11-01)
- Stack: #80 hooks (`claude-ui-g3-hooks`), #81 flow, #83 osc/filter/env (`claude/ui-g3-osc`, base `claude/ui-g3-flow`). #77-#79 (G2) accepted.
- #83 covers 10 of the 11 kinds with the generic flow plus family hooks (`rack/osc.rs`, `filter.rs`, `env.rs`): `on_face`, `knob_size`, `order`, `in_header`, `short_name`. Checker: none of the 11 kinds has an error except `env.adsr` at 0.5 (`timing / title` gap 1.5 px < 2 px).
- **Next PR: FM Voice (`osc.fm6`)**. It passes the checker but is the old card layout (ratio as a dropdown, no algorithm well, no editable curves). Target: PLAN §4 "Representative faces" FM Voice 660 and the G1b sheet (`docs/design/redesign/g1b/sheet-{graphite,paper}.png`, rendered by `crates/ui/examples/kabl_mock.rs`; its FM Voice spec is the layout reference).
  - Left column: algorithm well (operator graph, carriers filled, feedback loop), Freq / Fine / Fdbk / Index, OVERSAMPLE 2x/4x.
  - Six operator cards: `OP n` + carrier dot, ratio field, Level knob, editable envelope curve (A/D/S/R as draggable points). Fine + velocity per operator behind `+12` which opens the operator drawer.
  - Code: `rack/osc.rs::fm6_face` (layout, `Decor::Operators`), `displays.rs::fm6_scope` and `fm6` drawing in `lib.rs`/`rack_editor`; envelope editing needs a new widget (kit) and writes the four params per operator (`attack1..` etc.), so undo/save paths stay as for knobs.
  - Keep face width 660 (face_width test), port identity and order, saved racks. fm6 face jacks are on top left today (`rack/mod.rs` fm6 branch), the PLAN wants the single-row wide tray.

## Known diffs vs G1b in #83
- Plate is full tray width now; bold labels (target) vs regular weight, filter response shape (target has a bump), SVF shows LP only.
- Stereo jack double ring is drawn (`kit::face::jack_ch`), VA saw shows two cycles.
- `env.adsr` 0.5 title gap (see above).

## How to work
- Captures: `xvfb-run -a -s "-screen 0 1000x520x24" target/release/examples/face_capture OUT kind,kind [theme-id]`; sheets `OUT/<kind>__sheet.png`; side-by-side script is in the job tmp, recreate from `docs/design/redesign/g3/osc/side-by-side-*.png` (crop boxes: VA (24,46,258,388), SVF (280,46,492,388), ADSR (506,46,718,388) in the G1b sheet).
- Checker: `cargo test -p kabl-ui --test geometry` then read `target/ui-check/report.json` (rule, face, zoom, sev). Full `cargo test -p kabl-ui` takes ~4 min.
- Perf A/B: `KABL_BENCH_SIZES=1|1180 KABL_BENCH_THEME=kabl.satin|kabl.line_solid target/release/examples/bench_chrome 200 live`, interleave A B, 6 rounds, report median of medians and paired difference. Machine load is high (7-9): absolute numbers inflated.
- Builds are large (`cargo clean` at the end of a session).
