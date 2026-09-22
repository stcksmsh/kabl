# Visual QA (P4)

Static review of concept mockups. Each check is labelled as one of:

- **measured**: computed by `render.py --qa` from the same geometry the mockups are drawn from.
- **inspected**: rendered and looked at, at full size or as a close crop.
- **specified**: described in INTERACTIONS.md, not drawn.
- **unverified**: static images cannot establish it.

Targets are the proposed project targets from PLAN.md: essential labels 13–14 px, principal hit
regions ≥ 28×28, text contrast ≥ 4.5:1, and ≥ 3:1 for non-text indicators.

## Measured

Run `python3 docs/design/render.py --qa`. The results below are from the committed state.

| Check | Result |
|---|---|
| Hit regions, reference patch | 47 regions (knobs, selector segments, jacks, module headers). Minimum 28×28. **0 overlapping pairs** |
| Essential text overlap / panel overflow | 0 findings. Text boxes use estimated Plex widths (0.55 em sans, 0.60 em mono) with a 4 px minimum gap |
| Route identity across presentations | holds by construction: one cable list (`C1`–`C8`) drives plugs, badges and drawer rows in all six mockups |
| Font sizes | module names 15 px; knob / port / selector labels and values 13 px (selector options 12.5 px mono caps). Exceptions below |

Contrast (WCAG ratio):

| Pair | A | B | C |
|---|---|---|---|
| panel label (ink on panel) | 11.75 | 14.9 | 13.25 |
| module kind line (secondary) | 5.61 | 7.8 | 6.15 |
| output plate label | 11.93 | 14.74 | 13.28 |
| selector, selected | 13.25 | 17.06 | 13.63 |
| selector, unselected | 9.98 | 12.75 | 10.95 |
| chrome text / secondary | 12.47 / 6.91 | 15.86 / 7.72 | 13.06 / 7.24 |
| button / active button | 10.05 / 13.2 | 13.2 / 15.96 | 10.31 / 14.18 |
| audio two-tone vs panel / plate (non-text) | 5.31 / 5.73 | 8.23 / 4.92 | 5.93 / 6.09 |
| CV two-tone vs panel / plate (non-text) | 6.14 / 4.61 | 7.64 / 5.18 | 6.86 / 4.9 |
| gate two-tone vs panel / plate (non-text) | 7.49 / 3.37 | 5.89 / 6.26 | 8.37 / 3.58 |
| selection vs panel / rack (non-text) | 3.84 / 3.51 | 6.47 / 7.64 | 4.29 / 3.66 |

The signal colours by themselves are only 1.7–2.4:1 against a panel of similar lightness (for
example, amber on A's off-white panel). So every ring and cable is **two-tone**: the colour plus
an edge 45% darker. The table lists the better of the two, and that one always clears 3:1. Colour
is never the only cue: hidden-mode badges and drawer rows carry text.

For C, text contrast is measured against the clear-zone plate. The plate has 96% opacity, so the
art under it shifts the real background slightly. The shift was **inspected**, not measured.

## Inspected, with defects fixed

Every artifact was rendered and looked at, and changed outputs were rendered again. Problems
found and fixed:

1. Cables covered jack labels. Labels were below the jacks, and cables hang down. **Fix:** labels
   moved above the jacks, for every direction.
2. The inter-row gate and pitch cables crossed the ADSR knobs and the MIDI header. **Fix:** module
   order changed (Oscillator/LFO/Filter over MIDI/ADSR/VCA/Output). MIDI and Output jacks moved to
   the left edge, with their labels to the right.
3. The Filter-to-VCA cable crossed the VCA response switch. **Fix:** VCA controls moved right of
   centre, which leaves a cable lane.
4. Hidden-mode badges on the three VCA jacks overlapped at 6u. **Fix:** proposed VCA width 7u.
5. C: clear zones were drawn over neighbouring labels ("Attack", "240 ms" were clipped), and
   zones stuck out past panel edges. **Fix:** all zones are drawn first, and they are clamped to
   the panel.
6. Hover and focus rings on knobs overlapped the label and value. **Fix:** hover lights the
   skirt; focus is a dashed frame around the whole control.
7. The modulation ring touched the knob label, and value text touched the selector label.
   **Fix:** labels raised and selectors lowered. Labels and values share baselines along a row.
8. Crowded hidden view: badges showing full module names overlapped each other and the panel
   edge. **Fix:** badges use a short instance name (`Osc 1`, `Filt Env`), capped at 64 px.
9. Signal colours on light panels and B's light plates were below 3:1. **Fix:** two-tone rings.
   The selection blue was darkened for A/C.
10. The undo storyboard frame restored an amount that had never existed. **Fix:** it now
    restores the connect default, +1.00.

## Reference tasks (static walkthrough, A)

| Task | Where it is answered | Status |
|---|---|---|
| Find output, adjust cutoff, enter exact value, identify waveform | Output on row 2, far right; Cutoff labelled with its value; numeric entry on the component sheet; waveform selector plus LFO wave display | inspected |
| Trace audio source to output; list selected LFO's destinations | All: follow the amber cables. Hidden: badges (`← Osc`, `→ VCA`, `→ 2 routes`) and the drawer's `ALL CONNECTIONS` | inspected |
| Change modulation amount with cables hidden | `hidden.png`: drawer route card, slider + value box. Storyboard frame 2: ring drag | inspected (the amount itself needs engine work) |
| Pick one of several connections on a jack; find its other end | VCA out `→ 2 routes` badge opens a list; plug click highlights both ends | specified |
| Base value vs modulation range; bypass vs remove | knob pointer = base, outside ring = range; route card Bypass / Remove; INTERACTIONS.md | inspected / specified |
| Open drawer without losing selected controls | drawer takes width from the rack, and the rack pans to keep the selection visible (the reference patch fits at 944 px) | inspected |

Stress specimens: `focus.png`; `crowded-working.png` (14 modules and 21 connections at 100%,
panned, focus on); `crowded-hidden.png`; `crowded-overview.png` (50% overview with the selection at
100%); `long-names.png`; `all-200pct.png` (2560×1600, vector-crisp); `layout-hit-regions.png`.

## Known exceptions (left as-is, deliberately)

- **All mode, reference patch:** C3 (Osc → Filter in) and C6 (LFO → Cutoff CV) pass over the
  Filter's `LP` label. Physical cables will always cover something. Focus and Hidden solve it, and
  cable opacity could become a view setting, as it is in VCV. No layout tried avoided it without
  moving another cable across a knob.
- **Crowded All/Focus:** dimmed cables still cross panels. This is expected at 14 modules. Focus
  keeps the relevant ones solid.
- Text below 13 px, all non-essential or duplicated elsewhere: module kind line (11 px mono),
  badges (11 px, and the drawer repeats the full names), drawer secondary lines and chips
  (12–12.5 px), footer (12 px). The overview scale hides labels on purpose, and the 100% popover
  brings them back.
- The patch name in the top bar ellipsizes at 150 px (`long-names.png`). The full name belongs in
  a tooltip.
- Direction C art is a procedural vector placeholder. No bespoke art exists yet
  (IMAGEGEN_PROMPTS.md). C's clear zones make its panels busier than A's; that is the trade-off
  C is meant to test.

## Unverified (deferred to the interactive prototype after G1)

Real hit-testing and gesture feel, keyboard traversal, how the drawer pans the rack when a
selection would be covered, frame rate with cable rendering, audio continuity during edits, and
whether real people find any of this usable. Text widths are estimates; a real renderer may
differ by a few pixels.
