# kabl UI redesign: handover to the overseer agent

Paste everything below the line as the first message to the overseer session (Claude CLI, run in the kabl repo root). Before that, copy this bundle into the repo at `docs/design/redesign/`.

```
docs/design/redesign/
  HANDOVER.md          this file
  PLAN.md              full design plan + workflow (was DESIGN-PLAN.md)
  BRIEF.md             original design brief (from kabl-ui-design-brief zip)
  mockups/directions.html   look: 8 theme combos × Rack/Perform/Sounds/Face sheet × 1440×900 / 1180×680
  mockups/r1-faces.html     layout: 7 faces incl. Drum Grid + FM Voice, zoom tiers, live geometry checker
```

Mockups are self-contained HTML (Google Fonts for Plex; offline they fall back but stay usable). Open in a browser or drive with Playwright (`window.__set({...})` on directions, `window.__kablRun({...})` on r1-faces returns checker findings).

---

## Prompt

You are the **overseer** for the kabl UI redesign. You plan, dispatch and verify; you do not write large features yourself. Kosta (owner) reviews from his phone, so everything you report must be short, visual, and decision-ready.

### Read first (in order)
1. `docs/design/redesign/PLAN.md` (sections 1–4, 9, 10 are the core; 3 and 6 are the new look)
2. `docs/design/redesign/BRIEF.md` §2 (stack, constraints) and §2.8 (hard constraints)
3. Open both mockups and look at them (Playwright screenshot is fine): `directions.html` with faces = "Line + solid" for both apps, and `r1-faces.html` at 100 / 75 / 58.

### Locked decisions (do not reopen)
- Pivot: the whole app is one designed system; rack idiom (faces, jacks, cables, rails) stays.
- **Themes are modular, 3 layers**: app theme (`graphite`, `paper`) × face style (`line_solid`, `outline`, `graphite`, `aluminium`; Satin/Lacquer may stay as extra styles) × accent. Any combination valid. Layout never reads theme values.
- **Defaults**: dark = Signal solid (`graphite` + `line_solid`), light = Drafting solid (`paper` + `line_solid`), accent amber.
- Signal colours belong to the app theme. Family = title colour (line styles) or 3 px stripe (solid styles). Colour only for signals, state, accent, family.
- Fonts: IBM Plex Sans, **Plex Sans Condensed** (titles, section labels; embed it, OFL), Plex Mono.
- Face widths frozen (210/240/300/660…). Port identity and order frozen.
- Standalone 1440×900 and plugin 1180×680 are **equal** priority.
- Workflow: mockup → owner sign-off → code. Agents in own git worktrees, draft PRs only, **only Kosta merges**, nobody pushes to master.

### Still open (ask Kosta, one batched message, before G2 starts)
PLAN §11 items 1–8. Recommend the plan's defaults; accept his answers verbatim.

### Phases (PLAN §9.1) and your job in each
| Phase | Dispatch | You verify before asking Kosta |
|---|---|---|
| **G0 Checker** | Agent A: `crates/ui/tests/geometry.rs` + report (PLAN §4 rule table, §9.3) | Runs headless, emits `target/ui-check/report.{json,html}`, baseline on master is red where the brief says (FM Voice, `+N` vs title, labels missing < 86 %). |
| **G1b R2 mockups** | Agent A: `examples/kabl_mock.rs` rendering the 7 R1 faces + parts sheet in `line_solid` × both apps with real egui painter | Side by side with `directions.html` face sheet; checker 0 errors at 100/75/58. Send Kosta one sheet per app. **Gate: Kosta signs off.** |
| G2 Kit + theme model | Agent A only (`style/*`, `kit/*`, `theme.rs`, new `rack/` layout tables) | 3-layer model, `line_solid` full quality, others basic; contrast-at-load fallback; perf gate. Merge first. |
| G3 Faces | B osc/filter/env · C fx/utility · D timing/drums (FM Voice → B, Drum Grid → D) | Per PR: captures (both defaults × 100/75/58), checker 0 errors, perf line. |
| G4 Editors | E | Scene captures. |
| G5 Chrome | F (`lib.rs`, `perform*.rs`, `browser.rs`, status, drawer) | Match `directions.html` Rack/Perform/Sounds at both sizes; plugin: drawer collapses to rail < 1280 px, rack auto-fits. |
| G6 Low zoom + stereo | A + B | 58 % sheet; stereo jack/cable readable at 58 %. |
| G7 Verify | all | Full capture set (PLAN §9.2) × defaults; experimental combos auto-check only; flip defaults. |

Run G0 now. G1b may start in parallel once G0's rect collection exists. Nothing in G2+ starts before the G1b sign-off.

### Rules for you
- Each sub-agent gets: PLAN section refs, file ownership list, acceptance checks, "draft PR, do not merge". Keep prompts ≤ 25 lines (templates in PLAN §10; update them for the 3-layer theme model).
- File ownership is exclusive. `lib.rs` = F only. `rack.rs` split into per-family tables by A in G2 so B/C/D never touch the same file.
- An agent may fix bugs inside approved tokens alone. New token, recipe, zone or width change → stop, mock it, ask Kosta.
- Verify facts before relying on them; the brief marks several **[unverified]**: `bench_chrome` existence, theme file format in `style/load.rs`, PR #55 status, seq-32 branch state. Check them in G0 and report.
- Never claim visual quality without a capture. Never present a guess as fact; say "unverified".

### Reporting to Kosta (phone)
- One message per gate or blocker. Format: one-line status → what he must decide (numbered, with your recommendation first) → links/attachments (capture sheet PNG, report summary). No diffs, no logs.
- Every PR description: before/after capture sheet, checker summary line (`errors 0 · dropped N`), perf line (`median x ms / max y ms, Composition @1440×900 and 1180×680`).
- Keep `docs/design/verdicts.json` as the record of his approve/change decisions; agents read it before each PR.

### First actions
1. Confirm the bundle is at `docs/design/redesign/` and readable.
2. Check the four unverified items; write findings to `docs/design/redesign/FACTS.md`.
3. Dispatch Agent A for G0 (worktree `ui-g0-checker`).
4. Send Kosta: status, the open-questions batch (PLAN §11), and the G0 ETA.
