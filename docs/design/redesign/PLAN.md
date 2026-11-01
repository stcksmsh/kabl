# kabl UI redesign: design plan + workflow (R1, revised)

Status: R1 layout rules + revised look (owner chose a pivot on 2026-10-31). Sections 3, 6 and 11 supersede the earlier Satin-only version. Answers brief `kabl-ui-design-brief` (commit `6e92737`).
Companions: `mockups/directions.html` (look) and `mockups/r1-faces.html` (layout + live checker; Satin styling there is superseded).
Suggested repo path: `docs/design/redesign-r1/PLAN.md`.

Owner answers used: direction = evolve or change if better; themes = whatever looks best;
standalone and DAW must work **equally**; deliverable = plan + R1 mockups.

Marked **[decision]** = taken in R1, open to veto. **[unverified]** = not checked against code.

---

## 1. Diagnosis

"Looks broken" is mostly geometry, not style:

1. Positions are hand-tuned per face in `rack.rs` with no constraint checks → collisions (FM Voice, `+N` vs title, `Cutoff CV`/`Res CV`).
2. Text hides below 9.5 screen px; 11 px × zoom < 9.5 for any zoom < **0.864**. So at 65–75 % (normal in a 1180×680 plugin window) **every label vanishes**. This is the "text missing at 50 %" complaint, and it also hits the DAW case.
3. Objects dropped on panels (neutral black displays, drum grid widget, stock combos) instead of belonging to a layer.

Fix order: checker → layout rules → zoom tiers → look. Restyling alone fixes none of 1–2.

---

## 2. Principles (each testable)

| # | Principle | Test |
|---|---|---|
| P1 | Every element lives in one layer: **Panel** (material, title, labels), **Well** (shows/edits data, cut in), **Hardware** (knobs, jacks, selectors; stands proud). | Element registry has exactly one layer per kind. |
| P2 | No two leaf boxes overlap or sit closer than 3 px (2 px label → own control). | `overlap`, `gap` checks = 0. |
| P3 | Text never truncates. Full → short name → dropped by rule (listed). | `fit` = 0; dropped list reviewed. |
| P4 | Same structure at every width: header / well / controls / tray, fixed y bands. | Zone conformance check. |
| P5 | Legible at every supported zoom: text held at ≥ 9.5 screen px, never shrunk below. | Required-text-at-tier check. |
| P6 | Signal type readable without colour: stereo = double ring / twin strand; drum velocity = fill height, chance = pips. | Greyscale capture review. |

---

## 3. Visual language and theme model **[owner decision 2026-10-31]**

Pivot from Satin: the whole app is one designed system. Looks are fully modular.

**Theme = 3 independent layers, any combination valid**

| Layer | Owns | Built-ins |
|---|---|---|
| App theme | chrome surfaces, text, rack/rails, **signal colours** | `graphite` (dark), `paper` (light) |
| Face style | panel, well, knob, jack, selector, tray, cable recipes | `line_solid` (default), `outline`, `graphite`, `aluminium` (Satin/Lacquer may return as more styles) |
| Accent | one brand colour | amber (default), coral, cyan, custom |

**Defaults**: dark = **Signal solid** (`graphite` + `line_solid`), light = **Drafting solid** (`paper` + `line_solid`). Toggling light/dark swaps only the app theme; faces keep identity.

**`line_solid` face style**: panel = 1 px line art (title in family hue, no fill / near-transparent fill), wells = outlined, displays and cables may glow on dark (stacked wide low-alpha stroke under the core), **hardware solid**: filled knob caps with drop shadow, filled hex/round jacks, value arc in accent (glow underlay on dark), filled selector track with accent cap.

Rules:
1. Layout is look-independent: geometry (§4) never reads theme values.
2. Signal colours live in the app theme (contrast depends on the background behind cables).
3. Outline-panel styles inherit text ink from the app theme; solid-panel styles carry their own ink.
4. Every theme combination is contrast-checked at load; failing pairs fall back to defaults with a log warning.
5. Users may author app themes, face-style parameter sets and accents as files; new drawing recipes are code only.
6. Module family = title colour (line styles) or 3 px top stripe (solid styles). Never whole-face colour.
7. Colour only for: signals, live state, accent, family marker.

Review policy: the two defaults get full capture review + owner sign-off. Other built-ins: automatic checks only, flagged "experimental" until reviewed.

Mockups: `mockups/directions.html` (8 combos × Rack/Perform/Sounds/Face sheet × 1440×900 / 1180×680). `mockups/r1-faces.html` holds the R1 layout reference (Satin styling, superseded look, valid geometry incl. Drum Grid and FM Voice).

---

## 4. Face layout system (px at 100 %, face height 340)

```
M (side margin)          12
Header                   y 0–46   title 14/600 at (12, 12); caption 11 mono at y 32
  right cluster          from x = w−12 leftwards: +N chip (h 18, min w 26), gap 8, mode selector (h 18), gap 14 between header selectors
  title max width        cluster left edge − 12 − 8; fallback = short name; never ellipsis
Display well (optional)  y 52–116, x 12…w−12, r 4
Control zone             y 124–236 (52–236 without well)
  rows                   centred vertically, 8 gap
Port tray (w < 480)      y 244–330, x 8…w−8, r 5; two rows
  input row              jack centre y 273, label bottom = jack top − 3
  output plate           y 288–330, jack centre y 316
Port tray (w ≥ 480)      y 286–330, single row at y 316; inputs left on tray, outputs right on plate; pitch 56
  control zone grows     to y 278
Screws                   (7,7) r 2.4 at the four corners
```

Knobs: size by count in row: 1–2 → **L r 24**, 3 → **M r 20**, 4+ → **S r 16** (per-control override allowed, e.g. VA Frequency L next to two S). Value arc ring r + 3.5, 2.5 stroke, 270° sweep from −225°. Cell height = 2r + 34 (label 12, gap 3, ring, gap 3, value 12).
Knob pitch = (w − 24) / n. Label max width = pitch − 4.
Jack: hex nut r 11; signal ring r 7.6; hole r 5.2. Jack pitch (narrow) = min(72, (w − 24) / max(ins, outs, 3)), left-aligned so inputs sit above outputs.
Selector: h 20 in body (18 in header), track r 4, raised cap for selected (inset 1.5 px), mono 11 text.

Geometry rules for the checker (screen px):

| Rule | Full tier | Compact tier |
|---|---|---|
| leaf overlap | 0 | 0 |
| min gap, any two leaves | 3 | 2 |
| min gap, label ↔ own control | 2 | 1.5 |
| min margin to face edge | 4 | 4 |
| text wider than slot after fallback | error | dropped (info) |
| required label missing | error | info if dropped by rule |
| text contrast | ≥ 4.5 : 1 | ≥ 4.5 : 1 |

Leaves = text, knob (incl. arc ring), jack, selector, chip, pad. Containers (well, tray, card) are excluded from pairwise checks but children must sit inside them.

**Width policy** **[decision]**: widths unchanged (210/240/300/660). Jack rows move from y 248/296 to 273/316 (cables bind by port identity, so saved racks load; drawn positions shift). Needs owner OK.

### Representative faces (R1)

- **VA Oscillator 240**: saw well; Frequency L, Fine S, PW S; SIN TRI SAW SQR full-width selector; `+7`; Pitch, Sync / Out, Stereo (stereo jack).
- **SVF 210**: response well with cutoff marker; Cutoff L, Resonance L; In, Cut CV, Res CV / LP BP HP Notch.
- **ADSR 210**: CONT/KEY in header (title falls back to "ADSR"); envelope well with playhead; Atk Dec Sus Rel (S).
- **Mixer 240**: input meter strip well (new content), 4 level knobs aligned under meters → reads as channel strips.
- **Sequencer 660**: header EDIT A–D, PLAY A–D, PAGE 1–4 (playing page dot). Left: Length, Gate length. 8 columns: LED + step number, pitch knob + `+7 st`, gate pad, velocity lane, probability. Ratchet + gate mode behind `+9`. Pages of 8 cover seq-32.
- **Drum Grid 660**: header PAT / VEL brush / CHANCE brush; 8×16 well, 4-step groups, row names, playhead column, per-row length shown as unfilled outline cells; velocity = fill height, chance = 1–3 pips.
- **FM Voice 660**: left column: algorithm well (operator graph, carrier filled, feedback loop), Freq/Fine/Fdbk/Index, OVERSAMPLE 2×/4×. Six operator cards: OP n + carrier dot, ratio field, Level knob, editable envelope curve (A/D/S/R as draggable points). Fine + velocity per op behind `+12` → opens operator drawer.

---

## 5. Typography

IBM Plex only (embedded, OFL). Mono for numbers, ids, states; sans for names.

| Role | Size | Weight | Font | Case |
|---|---|---|---|---|
| Face title | 14 | 600 | Sans | Title |
| Caption (`kind · #id`) | 11 | 400 | Mono | as-is |
| Control label | 11 | 500 | Sans | Title |
| Value | 11 | 400 | Mono | as-is |
| Selector / chip | 11 | 500 | Mono | UPPER for modes |
| Perform tempo / pads | 40 / 14 | 600 | Sans / Mono | — |

Short names: per control, ≤ 6 chars (Atk, Dec, Sus, Rel, Res, Cut CV, Fdbk, Lv 1, Len, Lvl, BD SD CP CH OH TM P1 P2). Stored next to the param in module metadata. Full name in tooltip.

### Zoom tiers (replaces "hide below 9.5")

| Tier | Zoom | Behaviour |
|---|---|---|
| Full | ≥ 0.864 | Everything at design size. |
| Compact | 0.55–0.864 | Text drawn at max(design × z, 9.5) screen px. Labels use short name; dropped only if short name > slot width or vertical room. Values and captions hidden (shown on hover/drag tooltip). Title ≥ 10.5 px. Selector text hidden if any option won't fit; selection position still shown. |
| Overview | < 0.55 (proposed) | Title, display well, connected jacks only; knobs as value arcs. Not drawn in R1. |

R1 result (Satin, both modes): Full 0 errors; 75 % 0 errors, 26 drops (values, captions); 58 % 0 errors, 58 drops (all jack labels, values, captions, a few long labels).

---

## 6. Tokens (from mockups; contrast to be confirmed by the checker)

App themes:

| Token | graphite | paper |
|---|---|---|
| bg / bar / surf / raised / inset | `#111214 / #17181B / #1A1B1E / #232529 / #0F1012` | `#ECEAE5 / #F7F6F2 / #FBFAF7 / #FFFFFF / #E4E1DA` |
| line / line2 | `#2A2C31 / #393C42` | `#DCD8D0 / #C8C4BB` |
| text / text2 / text3 | `#ECEAE6 / #A29F99 / #6F6D68` | `#1A1917 / #5F5C56 / #8E8A83` |
| rack / rail / rail hole | `#0C0D0E / #1B1C1F / #060607` | `#DCD9D2 / #C9C5BC / #A39E94` |
| audio / cv / gate / pitch | `#F59E3B / #38C6B2 / #A68BFA / #5BA8F7` | `#D9781C / #12958A / #7651D6 / #2F7FD6` |

Accent: amber `#F2A33A` (ink on accent `#1C1206`), coral `#FF6B3D`, cyan `#33C1D8`.
Family hues: osc `#E07A5F`, filter `#7FB59A`, env `#9D8DF1`, timing `#5FA8D3`, amp `#E3BE5C`, fx `#B9B2A6`. Line styles draw titles with hue mixed 15 % to white (dark) / 35 % to black (light) **[contrast unverified]**.

`line_solid` values (dark / light): panel edge `rgba(236,234,230,.26)` / `rgba(26,25,23,.30)`; knob skirt `#141518` / `#D6D2C9`; cap gradient `#3C3F45→#1D1F23` / `#FFFFFF→#E2DED6`; pointer `#F2F0EB` / `#1A1917`; jack nut `#2B2D32` / `#FFFFFF`, nut edge `#45484E` / line2; hole `#050506` / `#24232A`; selector track inset / `#D6D2C8`; shadow alpha .45 / .14; glow alpha .14–.22 (dark only).

Type: IBM Plex Sans (UI), **IBM Plex Sans Condensed 600 uppercase +0.07 em** (face titles, section labels), IBM Plex Mono (numbers, states). Condensed is OFL, same family; must be embedded.

New slots / recipes: `face.style` (`line_solid | outline | graphite | aluminium | satin…`), `face.family_marker` (`title | stripe`), `glow` dial, `jack.stereo`, `cable.channels`, `drum.cell`, `tray.layout`, `type.compact_floor` (9.5, not themable).

---

## 7. Components and states

Knob S/M/L: default, hover (track 0.32 alpha), focus (2 px focus ring r + 7.5), modulated (cv arc from value to mod range + live dot r 3.2), disabled (40 %).
Jack: audio, cv, gate, pitch, stereo, connected (plug cap in signal colour), hover (halo r 14), disabled.
Cable: rest, lit (lighter core + highlight line), stereo (8 px dark edge, 6.4 colour, 1.6 dark centre line), functional (step beads: filled = on, hollow = off), dimmed in Focus view.
Selector, toggle pad, chip, cue pad (idle / queued with progress / playing), drum cell (off, 4 velocities, 3 chances).
Still to draw (R2): dropdown, button primary/ghost/icon, field, slider, step lane, meter, tooltip, menu, dialog, toast, card, error/unconnected.

Stereo at 58 % is weak in R1 (double ring reads, twin strand is thin). R2: hold stereo centre line ≥ 1.5 screen px at any zoom.

### Cables (planner decision, relayed by Kosta 2026-11-01)

Do not redesign cables. Port `cable_look.rs` as-is and theme it. Applies in G2 (cable paint tokens) and G6 (stereo, functional).

- Geometry, sag, drag, beads and plug logic are unchanged. The theme supplies paint only: core (signal colour from the app theme), edge darken, highlight alpha, shadow alpha and offset, glow alpha and width, plug style.
- The current look stays selectable as `cable.style = "classic"`.
- Dark default: a lit cable gets a glow underlay of about 12 px. Light default: no glow, stronger edge and shadow.
- Two passes: all glows first, then all cores. Core at least 2.5 screen px at any zoom; stereo centre line at least 1.5 px.
- Stereo = twin strand + double-ring plug. Functional = beads (filled on, hollow off, current step in the accent).
- Focus view: other cables at 25 %; the selected cable glows (dark) or thickens (light).
- Cache meshes per (cable, zoom, theme). Composition patch: median < 8 ms, max < 16 ms.
- Before/after capture set: simple, Composition, stereo, functional x classic vs both defaults x 100 / 58 % x 1440x900 and 1180x680.
- Gate: Kosta compares side by side. If the new look loses anywhere, ship the classic renderer with the new tokens only.

---

## 8. Scaling: standalone and DAW equally

Same rules, two targets, both gated:

| Window | Rack default | Tier | Must hold |
|---|---|---|---|
| 1440×900 standalone | 100 % | Full | 2 rack rows + drawer open |
| 1280×800 standalone | 86–100 % | Full | toolbar no overlap |
| 1180×680 plugin | Fit (≈ 65–75 %) | Compact | all control labels; toolbar no overlap; drawer as overlay not split |
| HiDPI 150 % | × scale | per effective px | same captures at 2× |

Plugin specifics **[decision]**: routing drawer becomes an overlay below 1280 width (today it takes a third of the window); empty band above the rack removed (rack top-aligns under toolbar).

---

## 9. Workflow

### 9.1 Phases and gates

| Phase | Owner of files | Exit gate (what Kosta sees) |
|---|---|---|
| **G0 Checker** | agent A: `crates/ui/tests/geometry.rs`, `rack.rs` read-only | JSON + HTML report of current master = red baseline. |
| **G1 Mockups R1** (this) | design | Review page; verdicts per face. |
| **G1b Mockups R2** | design + A | Same faces rendered by egui in a `kabl-mock` example binary using the new kit. Sign-off target = egui render. |
| G2 Kit + style | A: `style/*`, `kit/*`, `theme.rs` | Parts sheet captured from app; perf numbers. Merged first. |
| G3 Faces | B: osc/filter/env; C: fx/utility; D: timing/drums | Per family: captures + checker green + review page. |
| G4 Editors | E: `tables.rs`, `pattern.rs`, `drum_grid.rs` | Scene captures. |
| G5 Chrome | F: `lib.rs` toolbar/drawer/status, `perform*.rs`, `browser.rs` | 3 window sizes × 2 modes. |
| G6 Low zoom + stereo | A + B | 58 % sheet, stereo parts. |
| G7 Full verification | all | Full capture set, perf, sign-off. |

### 9.2 Fixed capture set

```
faces:  53 kinds × {light,dark} × {100,75,58} = 318 PNG
        + 7 R1 faces × 150 % HiDPI × {light,dark} = 14
scenes: {rack-simple, rack-dense(Composition), rack-dense+drawer, perform, browser, wavetable, pattern, drumgrid, dialogs}
        × {1440x900, 1280x800, 1180x680} × {light,dark}
plugin: REAPER, 1180x680, {light,dark}, rack + perform (manual or xdotool) 
name:   cap/<set>/<kind|scene>__<look>__<mode>__z<zoom>__<WxH>.png
fixed:  seed 1, transport stopped at bar 1 beat 1 except "live" variants (frame N of a fixed playback),
        fonts embedded, silent audio sink, deterministic patch files in tests/fixtures/capture/
```

### 9.3 Automatic checks

Geometry (no GPU, `cargo test`): overlap, gap, edge, fit (glyph advances from embedded Plex via `ab_glyph`/egui fonts headless), required-text-per-tier, grid alignment (x on 4 px), contrast from tokens, controls outside face, empty-zone ratio (control-zone area with no leaf / total; flag > 0.55).
Pixels (captures): golden diff vs approved, blank-text detection (labels expected by geometry but no ink in box), perf.
Report: `target/ui-check/report.json` (`{face, look, zoom, rule, boxes:[…], sev}`) + `report.html`.

### 9.4 Review page

Static HTML from capture set + mockups: target | real side by side, blink toggle, checker boxes overlaid, filter (failing / face / look / zoom), approve / change + note per item → `docs/design/verdicts.json` (committed). Published so it opens on phone. Agents read verdicts before each PR.

### 9.5 Autonomy

Agent alone: bug fixes inside approved tokens with checker green. Needs new mockup + owner OK: new token, new recipe, zone change, width change, anything that changes a captured image of an approved item beyond threshold.

### 9.6 PR rules

One family per PR, ≤ ~800 lines changed. Attach: capture sheet, checker report (0 errors), perf line. A merges first; others rebase on A. `rack.rs` conflicts avoided by per-family layout tables in separate files (`rack/osc.rs`, …) created by A in G2. `lib.rs` owned by F only.

### 9.7 Rollback

Each phase behind the theme switch: new theme model ships beside the current `kabl.satin` / `kabl.lacquer` (kept selectable) until G7; defaults flip to Signal solid / Drafting solid at G7. Saved racks/presets load unchanged (port identity + grid positions). Revert = drop theme.

### 9.8 Performance gate

Composition patch (41 modules, 76 cables) at 1440×900: median < 8 ms, max < 16 ms; also at 1180×680 compact. Cache face meshes per (theme, width, zoom tier). Tool: `bench_chrome` **[unverified: named in BUILD.md, not found in tree; A creates it if missing]**.

---

## 10. Agent prompts (short)

**A — checker (G0)**: "Add `crates/ui/tests/geometry.rs`. For every kind in `KNOWN_KINDS` × zoom {0.5, 0.58, 0.75, 1, 1.5, 2}, collect all leaf rects from `rack.rs` layout + measured text extents (embedded Plex). Implement rules in PLAN §4 table. Emit `target/ui-check/report.json` + `report.html`. Do not fix faces. Test must report, not fail, until G3 (feature flag `strict`). Draft PR with baseline report."

**A — kit (G2)**: "Implement the 3-layer theme model of PLAN §3 with tokens of §6: app themes graphite + paper, face styles line_solid (first, full quality) then outline/graphite/aluminium, accents; recipes: family marker, tray two_row/single_row_wide, jack stereo, cable channels, drum cell, compact tier text rule. Parts sheet capture must match approved R2 within threshold. Perf gate §9.8."

**B/C/D — faces (G3)**: "Port <family> faces to PLAN §4 zones using kit only. No magic numbers outside layout tables. Checker 0 errors at all zooms for your kinds. Attach face captures (light/dark × 100/75/58) and report."

**E — editors**, **F — chrome**: same shape, inputs = approved scene mockups, outputs = scene captures + report.

---

## 11. Open for owner

Decided: pivot; modular 3-layer themes; defaults Signal solid / Drafting solid; Plex + Plex Condensed.
Still open:
1. Jack rows to y 273/316 and single-row tray on wide faces.
2. Compact tier (text held at 9.5 px, short names, values on hover).
3. FM Voice operator cards; fine + vel behind `+12`.
4. Mixer meter strip (new content).
5. R2 sign-off target rendered by egui (recommended).
6. Overview tier below 0.55.
7. Hosts beyond REAPER.
8. Which built-ins besides the defaults ship in 1.0 (experimental flag OK?).
