# kabl G4 spec: surfaces with no previous target

Answers `kabl-ui-g4-brief` (base fdf78d5). Mockup: `mockups/g4-surfaces.html` (24 surfaces × graphite/paper × 1440×900/1180×680; real px; egui primitives only).
Look: default theme (`graphite`/`paper` × `line_solid`), **directions.html tokens** (not the captured colours), accent per owner update 2026-11-02.
Marked **[owner]** = needs Kosta's verdict before code. Everything else follows locked decisions.

---

## 1. Decisions on the brief's open questions

| # | Question | Decision |
|---|---|---|
| Q1 | Frames | Three kinds, one family. **Modal**: blocks input, scrim, title 17/600 at (24, 38), footer 64 px with top rule, actions right-aligned, primary last; widths S 400 / M 520 / L 760 (clamped to window − 80). **Tool window**: non-blocking, 44 px title bar (`bar` fill, drag grip, title 14/600, optional tag chip), only for advanced tools. **Popover**: menus, dropdowns, tooltips; no scrim. All: radius 12 (menus 10), 1 px `line` border, 3-step stacked shadow, close 28 px hit at top-right, Esc closes, focus starts on the primary field or button. |
| Q2 | Editor kit | One **drawer editor kit**: header (title 15/600 + mono kind chip + accent "Explain" link right), condensed section labels, rows `label | control | mono value`. One **step-grid** widget shared by seq editor, cable pattern editor and Perform lanes. Cues are cards. |
| Q3 | Wavetable | **Bottom dock**, full rack width, 330 px (280 at 1180), drag handle on top edge, rack visible above, drawer still usable. Same dock hosts future large editors. **[owner]** |
| Q4 | Developer tools | Same kit, **ADVANCED** tag in title bar, no bespoke layout. Tool windows anchor left at 40 px (fixes 1180 clipping). **Edit face merges into More controls** as tab 2 ("Controls / Face layout"). **[owner]** |
| Q5 | Empty / error | **Empty**: 56 px circle with icon, title 16/600, one line 13 `text2`, ≤ 2 actions. **Error**: toast top-right of rack area (16 px inset), 380 wide, 4 px severity bar + icon + title 13.5/600 + mono detail + text actions; errors persist until dismissed, warnings 8 s, success 3 s; stack newest on top, max 3. Status bar shows a red "N error(s)" pill while any error toast exists. Form errors = banner inside the dialog (tinted bad, icon, title, mono detail). The toolbar grey caption is removed. **[owner: timings]** |
| Q6 | File browser | **Style the built-in browser** (works in plugin hosts). Native OS dialogs not in 1.0. |
| Q7 | Scrim / shadow | Graphite: scrim `rgba(0,0,0,.55)`, shadow black. Paper: scrim `rgba(26,25,23,.28)` (never white), shadow at 18 %. Shadow = 3 stacked rects (offset 2/6/12, alpha .5/.3/.18 × base), no blur. |
| — | Drawer at 1180 | ≥ 1280 px docks at 320. Below: 44 px rail; panels open as a **320 px overlay** with left shadow; rack zoom unchanged. (Brief defect: 360 px drawer covering faces.) |

## 2. Tokens added for G4

| Token | graphite | paper |
|---|---|---|
| accent fill (both) | `#F2A33A`, ink `#1C1206` (8.9:1) | same |
| accent text (small text, links) | `#F2A33A` | `#A94F00` (4.6:1 on bg, 5.3:1 on surf) |
| bad / good / warn | `#F06A5F` / `#5CCB8A` / `#E3A93C` | `#C2362B` / `#2E7D4F` / `#9A6200` (all ≥ 4.5:1 on surf) |
| tint bg for banners | state colour at 12 % | state colour at 8 % |
| scrim | `rgba(0,0,0,.55)` | `rgba(26,25,23,.28)` |
| field bg | `inset` | `raised` (#FFF) |
| paper signal colours | — | audio `#B66518`, cv `#11897F`, gate `#7651D6`, pitch `#2E7CD2` (verdict 2026-11-01) |

Type: titles Plex Sans 600 (modal 17, drawer 15, tool 14, card 14); body 13–13.5; section labels Plex Sans Condensed 600 12 uppercase +0.08 em `text2`; numbers, paths, shortcuts Plex Mono 11.5–12.5.

## 3. Kit parts to add (agent A, `kit/`)

| Part | Spec |
|---|---|
| `button` | h 32 (30 compact toolbar, 28 inline), radius 7, padding 14, label 13/600. Kinds: primary (accent fill), secondary (`raised` + `line2`), ghost (text `text2`), danger (ghost in bad). Optional 16 px leading icon. Focus ring 2 px accent text colour, 3 px offset. |
| `icon_button` | 28 hit, 16 icon, `text2`; on-state `raised` + `line2`. |
| `field` | h 32, radius 7, pad 12, optional leading icon and trailing mono suffix; focus = 1.5 px accent; error = 1.5 px bad. |
| `select` | field + chevron. |
| `slider` | track 4 px (`line2` dark / `line` light), fill accent or signal colour, bipolar mode with centre tick, thumb 15 px solid with shadow. |
| `segmented` | h 28, radius 7, `inset` track, selected = `raised` + `line` (or accent fill for state like A–D banks). |
| `checkbox` / `toggle` | 18 px radius 5 / 34×20 pill; replaces stock egui checkboxes everywhere. |
| `chip` | h 22 pill, optional dot (signal colour), optional mono. |
| `menu` | width 200–260, row 30, icon column 40, shortcut mono right, submenu chevron, header row (12/600 `text2`, optional cable swatch), separators, danger row in bad. Hover = `line` (dark) / `inset` (light). |
| `modal`, `tool_window`, `popover` | §1 Q1. |
| `empty_state`, `toast`, `banner` | §1 Q5. |
| `step_grid` | columns = steps; per lane: bars in `inset`/`raised` cells radius 3–4, value fill height, bipolar mode (pitch), pill toggles (gate), playhead = accent outline across lanes, tap toggles, drag sets value; lane labels condensed left 56 px. |
| `range_bar` | 10 px track, base marker 3×18 `text`, modulation span in cv at 45 %, mono min/now/max below. |
| `dock` | bottom panel, top drag handle 36×4, 6 px shadow strip, resizable 220–60 % height. |

New icons (vector, `kit/icons.rs`): close, folder, file, warn, err, ok, chevron-right, back, up, home, copy, grip/drag, pin, invert, bypass, trash, plug, wave, clock, external, star.

## 4. Surfaces

| Surface (capture id) | Target |
|---|---|
| Wavetable (`wavetable-editor`) | Dock. Header: title + kind chip + file mono; right: undo, redo, Audition (play icon), Import…, close. Body: frames stack (300 px, selected frame in accent) · Draw/Harmonics editor with glowing trace · right column (1440 only): Frame Insert / Duplicate / Delete, Process Normalise / DC / Fade. Footer: Position slider + "Fill between frames [4] new [Interp|Morph]". At 1180 the right column folds into an overflow menu. Path field moves into Import…. |
| Seq editor (`seq-pattern-editor`) | Drawer kit. Edit A–D + Page 1–4 segmented; "Playing B · bar 17" + launch quantise. Step grid lanes: Pitch (bipolar, st), Gate (pills), Vel, Prob, Ratchet (×n text). Below: Length, Gate length, Transpose sliders. Shows 8 steps of the current page (seq-32). |
| Cues editor (`cues-editor`) | Launch segmented (Step / Bar / 2 bars). Cue cards (drag handle, number, name 14/600, bank summary 12 `text2`, Playing/Queued chip). "Add cue from what plays" full-width secondary. One-line explainer. |
| Cable pattern (`cable-pattern-editor`) | Header: signal swatch + "Cable 01" + route line. Warn banner if no clock. Steps step-grid (16) with hint line. Chance / Morph A→B / Glide: label + mono value, slider, "Moved by" chip or "nothing", "+ Source" ghost. Replaces "Add source" combos. |
| Routes (`cable-routes-drawer`) | Header target + Explain. Base field + Default link. Range bar. Route cards: source chip, invert / bypass / remove icon buttons, bipolar depth slider, mono value; selected card accent border. "Add source" full width. Footnote on ranges. |
| Explain (`explain-panel`) | Back link; condensed context label; title 22/600; body 13.5 (≤ 3 lines per paragraph); key/value list (label `text2`, value mono); Moved by chips; Signal path chips with chevrons; "Show in rack" card. |
| Inspect (`signal-inspector`) | Empty state "Inspect a signal" + "Why no sound?" secondary + tip line. With selection: existing content in kit rows. |
| Compare (`compare-panel`) | Explainer; A/B large segmented (40 px); reference card (captured time, counts, session-only note); Capture again / Restore B… |
| Learn (`recipes-panel`) | Recipe cards (title, one line, step count, done check). |
| Module menu (`context-menu-module`) | Header "Sequencer #2"; Explain, Inspect output ▸ (submenu lists outputs with plug icon); More controls…, Edit face…, Reset face; Pin to Perform ▸, MIDI button ▸, MIDI learn ▸; Remove module (danger, Del). |
| Cable menu (`context-menu-cable`) | Header with cable swatch + route; Pattern & probability…, Focus this cable, Explain this cable; Remove cable (danger, Del). |
| More controls (`more-controls-modal`, `more-controls-and-edit-face`) | Modal L. Tabs Controls / Face layout. Controls: "On the face" pinned chips (pin icon), "More" knob grid (solid knobs, pin toggle top-right). Face layout tab = old Edit face (reorder list with drag handles, show toggles, Restore defaults). Footer: Keyboard entry… (ghost), Done. |
| Composites / Panel authoring (`composites-instance-tools`, `panel-authoring`) | Tool window + ADVANCED tag; form rows (label 140 px column), status chips (missing = warn dot, placed = good dot), Place / Import… secondary buttons; footer Export package… (ghost), Reload, Apply. |
| File browser (`file-open-dialog`, `file-save-dialog`, `file-empty-folder`) | Modal L. Back / up / home + breadcrumb (last segment 600) + Filter field. Places sidebar 164 px (Home, Patches, Sounds, Recent). List: Name / Modified, folder (accent icon, chevron) and file rows, selected = accent border. Open: footer filter caption + Cancel / Open. Save: Name field with `.kabl` suffix + Cancel / Save. Empty: empty state + Go up. |
| Dialogs (`dialog-*`) | Unsaved: S, body 2 lines, Don't save (danger, left), Cancel, Save. Save as: M, Name / Category select / Tags as removable chips + "Add tag". Error: banner on top, actions Cancel / Choose folder… / Try again. Rename: S, focused field. Restore: M, body + undo reassurance line with icon. |
| About (`about-dialog`) | M. 56 px mark + wordmark 28/600 + mono version·commit; tagline; licence 3 lines; source row (mono URL, copy button); Third-party notices link; Close. |
| Empty rack (`empty-rack`) | Rails visible; centred empty state "Empty rack / Add a module, or start from a sound" + Add module (primary) + Browse sounds; tip line about right-click. |
| Error (`load-stopped-error`) | Toast stack + status-bar error pill (§1 Q5). |

## 5. Defects from the brief (route to owners, not G4)

- Seq face "Edit" label over the title, Output "out" caption over the title → G3 agent D / B; checker should already catch it (overlap rule).
- Jack tray labels overlapping (`Cut CV`, `In`) at 1440 → G3 face owners; verify against the 273/316 jack rows.
- Drawer 360 px at 1180 → F (§1, overlay rule).

## 6. Acceptance per PR

- Captures for every touched surface: graphite + paper × 1440×900 + 1180×680, same ids as the brief, side by side with `g4-surfaces.html`.
- Geometry checker extended to chrome surfaces: no overlap, no clipped text, modal fully inside window at both sizes.
- Contrast: all text pairs ≥ 4.5:1 with directions tokens (load-time check).
- No stock egui widgets left on touched surfaces (checkbox, combo, slider, window frame).
- Keyboard: Esc closes, Tab order follows reading order, primary on Enter.

## 7. Not covered (no capture or no state)

Settings (no surface exists), record panel, MIDI learn overlay, plugin host frame, Help tab, hover tooltips, drum-grid and FM editing states. Use the same kit; mock on request.

## 8. Owner questions

1. Wavetable as bottom dock (vs floating window)?
2. Merge Edit face into More controls as a second tab?
3. Toast timings: errors persist, warnings 8 s, success 3 s?
4. Native OS file dialogs out of 1.0?
