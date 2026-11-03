# Handover to the implementation overseer: G4 surfaces + logo (2026-11-02)

Copy this folder into the repo at `docs/design/redesign/g4/`, replace `docs/design/redesign/verdicts.json` with the one here, then paste the Prompt section into the overseer session.

```
g4/
  HANDOVER.md                 this file
  G4-SPEC.md                  design spec for the 24 G4 surfaces (decisions, tokens, kit parts, per-surface targets, acceptance)
  verdicts.json               full verdict log incl. 15 new entries dated 2026-11-02 (replaces the repo copy)
  mockups/g4-surfaces.html    24 surfaces × graphite/paper × 1440x900/1180x680 (window.__set({surf,app,size,mode}))
  mockups/logo.html           logo comparison: original vs rebuilt frame, small sizes, app icon, toolbar
  logo/kabl-lockup-rebuilt.svg     viewBox 272x100, rebuilt loop + plugs, Kosta's original letters
  logo/kabl-mark-rebuilt.svg       viewBox 100x100, rebuilt loop + plugs, Kosta's original k
  logo/kabl-mark-small.svg         16-24 px variant (plugs as two blocks)
  logo/kabl-app-icon.svg           orange tile #F2A33A + mark in #1C1206
  logo/kabl-app-icon-small.svg     same with the small mark, for 16-24 px
```

All logo SVGs draw in `currentColor` with `color="#020202"` on the root, so they tint the same way as the current `kabl-logo.svg`. The letter and k paths inside them are copied verbatim from `crates/ui/assets/logo/kabl-logo.svg` (subpaths 8-12) and `kabl-mark.svg` (subpaths 6-7).

---

## Prompt

You are the implementation overseer for the kabl UI redesign. New design input has landed for G4 and the logo. Read `docs/design/redesign/g4/G4-SPEC.md` and `verdicts.json` (entries dated 2026-11-02), and open `mockups/g4-surfaces.html` at both sizes and both apps before dispatching.

### What changed (all approved by Kosta unless marked open)
1. **G4 surfaces have targets.** 24 surfaces: wavetable editor, seq pattern editor, cues editor, cable pattern editor, routes drawer, Explain / Inspect / Compare / Learn panels, module and cable context menus, More controls (with Edit face merged in as tab 2), developer tools, file browser (open / save / empty), five dialogs, About, empty rack, error toasts. Capture ids match the G4 brief.
2. **Owner answers to G4-SPEC §8:** wavetable = bottom dock; Edit face merged into More controls; toasts: errors persist, warnings 8 s, success 3 s; no native file dialogs in 1.0.
3. **Tokens:** accent fill `#F2A33A` in both themes (paper `#A97229` rejected); small accent text on paper `#A94F00`; state colours and scrims per G4-SPEC §2; app theme tokens = directions.html values. Correct `style/builtin.rs` before capturing anything new, since the G4 captures still show the rejected brown and the greyer panels.
4. **Drawer:** below 1280 px it is a 44 px rail plus a 320 px overlay; it never pushes the rack or changes zoom.
5. **Logo:** keep Kosta's logo. Letters and k are his paths, never redrawn. **Open:** rebuilt loop/plugs (`logo/*-rebuilt.svg`) vs his original artwork. Do not swap toolbar or packaging assets until he picks. Small mark, app icon and toolbar colour rule (logo in text colour; orange only for app icon and splash) apply to either choice.

### Work split (extend the existing ownership table)
| Agent | Files | Scope |
|---|---|---|
| A (kit) | `kit/*`, `style/*` | G4-SPEC §3 parts first: button, icon_button, field, select, slider, segmented, checkbox, toggle, chip, menu, modal / tool_window / popover, empty_state, toast, banner, step_grid, range_bar, dock; ~20 icons; G4 tokens. Merges before E/F use them. |
| E (editors) | `tables.rs`, `pattern.rs`, `lib.rs` seq_panel + cues sections (coordinate with F), `routing.rs` | Wavetable dock, seq editor, cues editor, cable pattern, routes. |
| F (chrome) | `lib.rs` (menus, empty rack, drawer, toasts), `explain.rs`, `inspect.rs`, `compare.rs`, `recipes.rs`, `browser.rs` dialogs, `filebrowser.rs`, `about.rs`, `rack_editor.rs` modals | Panels, menus, modals, file browser, dialogs, About, empty and error states, drawer overlay. |
| C or A | `composites.rs`, `panels.rs` | Developer tools in kit frame + ADVANCED tag; fix 1180 clipping. |
| Logo (after Kosta picks) | `crates/ui/assets/logo/`, `kit::logo`, `packaging/linux/logo/` | Swap assets, render masks and hicolor icons, small mark below 32 px. |

`lib.rs` is shared between E and F: split the seq/cues editor code into its own module first (E), then F continues. One surface group per PR.

### Acceptance per PR (G4-SPEC §6)
Captures for every touched surface (graphite + paper × 1440x900 + 1180x680, brief ids) next to `g4-surfaces.html`; geometry checker extended to chrome (no overlap, no clipping, modal inside window at both sizes); contrast ≥ 4.5:1; no stock egui widgets left on touched surfaces; Esc / Tab / Enter behaviour; perf gate unchanged (line_solid not slower than satin, interleaved A/B).

### Route back to G3 owners (not G4)
Seq face "Edit" label over the title; Output "out" caption over the title; jack tray labels overlapping (`Cut CV`, `In`) at 1440.

### Report to Kosta
One message when kit parts land (parts sheet in both apps), then one per surface group with capture pairs. Ask him once, batched, for the logo choice (rebuilt frame vs original). Show `mockups/logo.html` sections 1 and 4 as images.
