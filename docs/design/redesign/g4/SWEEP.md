# G4 sweep: mockup vs app (origin/master after #113)

UI only, no code changed. 22 gaps: **3 high, 9 med, 10 low**. Ranked by severity, then by how much of the app it touches.

## How this was made
- Mockup: `tools/shot_mockup.py` on `mockups/g4-surfaces.html` (24 surfaces, 96 shots); Rack, Perform and Sounds from `mockups/directions.html` (Signal solid / Drafting solid, hybrid faces, amber), same 1:1 headless-Chrome method, state set through localStorage.
- App: `chrome_capture` (all 32 scenes, 3 sizes, both themes).
- Pairs: `tools/sheet.py` (app left, mockup right). Every crop below is the same box cut from both halves (`crops/<id>__<theme>__<size>.png`, app left, mockup right).
- Reviewed in full: all 23 surfaces plus Rack, Perform and Sounds in graphite at 1440x900. Spot-checked in paper and at 1180x680: rack (paper 1180), seq editor (paper 1180), Save as (graphite 1180). The other light and 1180 sheets were generated but not each read. Same gap shows at those sizes where checked; nothing was found that exists only there except G11 and G15.
- Not covered: `cues-face`, `cable-routes-rest`, `cable-routes-opening` (app scenes with no mockup surface), and `dev-tools` (mockup surface with no app scene, see G9).
- Ignored by instruction: cables drawn over faces. Capture state differences (zoom 91 vs 95, bar 17, sound names, seq length 8 vs 32, "Internal clock" vs "Host sync", version hash) are not counted unless noted.
- Counts and names were checked in the mockup source: drawer tabs `g4-surfaces.html:522` (Routing, Inspect, Compare, Learn, Help) and `directions.html:324` (Routing, Explain, Inspect); PAGE chips `directions.html:165`; Perform draws 16 steps per sequence `directions.html:385`; rail icons `g4-surfaces.html:544` (`route, target, info, grid`). Pixel sizes of dialog text, toast bars and similar are read off the shots, not the source, and are marked as such ("about", "looks"); none of them is used to rank a gap.

## Leads from the brief
| Lead | Verdict |
|---|---|
| Rail 3 icons vs 4 | **Confirmed**, G5 |
| Seq playhead column highlight missing | **Confirmed** on the face, G3 (the drawer editor already has the column outline) |
| No unsaved dot after the preset name | **Rejected.** The app draws it and shows it when the patch is dirty: [g21](sweep/crops/g21-dirty-dot__light__1180x680.png) (seq scene). The rack captures are not dirty, so [g20](sweep/crops/g20-logo__dark__1440x900.png) shows no dot while the mockup always shows it. Capture state, not a gap. |
| PAGE chips need #75 | **Confirmed**, G2 |

## Gaps (ranked)

| # | Sev | Size | Dep | Surface | Mockup | App | Crop |
|---|---|---|---|---|---|---|---|
| G1 | high | M | none | Drawer tabs and panels | Tab strip Routing, Inspect, Compare, Learn, Help (Rack screen: Routing, Explain, Inspect); no close X, no "More"; each panel is its own tab and replaces the body; Back reads "Back to routing" | Routing, Explain, Inspect, More, X. Compare and Learn are not tabs: their panel renders at the top of the Routing tab with the full cables list stacked below it, and no tab is highlighted for them; Back is "Back" plus a separate "Close" | [g01](sweep/crops/g01-drawer-tabs__dark__1440x900.png), [g06](sweep/crops/g06-compare-panel__dark__1440x900.png) |
| G2 | high | M | #75 | Seq face and seq drawer | `PAGE 1 2 3 4` chips in the face header and in the drawer, "Next bar" | No PAGE chips on the face (header ends at PLAY A B C D) or in the drawer, "Bar" | [g02](sweep/crops/g02-page-chips__dark__1440x900.png) |
| G3 | high | S | none | Seq face playhead | Current-step column tinted on the face (step 5: warm fill behind the dot, knob and slider) | No tint on the face; only the drawer editor outlines the column | [g03](sweep/crops/g03-seq-playhead__dark__1440x900.png) |
| G4 | med | S | none | Toasts (Error scene) | Three toasts stacked at the right edge, error on top, severity bar and close X on each | One toast, placed beside the drawer (not at the screen edge); the warn and success toasts the scene pushes are not shown. Check whether the stack is capped or only the capture misplaces it | [g04](sweep/crops/g04-toast-stack__dark__1440x900.png) |
| G5 | med | S | none | Rail | 4 icons from the source: route, target, info, grid (route is the active one); no gear | 3 icons (overlapping circles, cable, sparkle) plus a gear at the bottom; different glyphs. The mockup's rail mirrors the drawer tabs; Kosta to say what grid stands for | [g05](sweep/crops/g05-rail__dark__1440x900.png) |
| G6 | med | M | #75 | Perform sequences | 16 step columns per sequence, lanes tinted by bar | 8 columns, plain tint | [g07](sweep/crops/g07-perform-seq__dark__1440x900.png) |
| G7 | med | M | sound work | Sounds detail pane | Waveform preview panel between description and macros; macro knobs smaller; Load sound, Audition, Open in rack | No preview panel (needs a stored or rendered preview per sound); long description fills the space | [g08](sweep/crops/g08-sounds-detail__dark__1440x900.png) |
| G8 | med | S | none | Sounds list | Active filter chips row (Bass, sequenced, dark), "Search 64 sounds" with the count, expand chevron per row, orange outline on the selected row | No chips row, plain placeholder, star per row, brown fill on the selected row | [g09](sweep/crops/g09-sounds-list__dark__1440x900.png) |
| G9 | med | M | none | Panel authoring modal | Surface `dev-tools`: G4 modal | Still an `egui::Window` (`panels.rs:154`); no `chrome_capture` scene, so it could not be compared | none (no app capture) |
| G10 | med | M | none | Routing drawer lists | Short names ("Seq Pitch" to "VA Osc Pitch"), swatch bar, selected row filled; Modulation as Source, Target, value bar ("Bite", "SVF Cutoff", +38 %) | Long names ("Sequencer Pitch"), plain lines, modulation as "ADSR Envelope #4 Filter Cutoff +25 %" | [g10](sweep/crops/g10-routing-list__dark__1440x900.png) |
| G11 | med | S | none | Face labels at 1180x680 | Short labels (Len, Gate, Freq, PW, Atk, Dec, Sus, Rel, Lv 1, Fdbk, Damp, Res) | Full labels, so "Pulse width" and "Feedback" crowd or drop (geometry report: `fit` info rows at z 0.58) | [g11](sweep/crops/g11-compact-labels__light__1180x680.png) |
| G12 | med | S | none | Status bar | Green meter, "-9.4 dB", "MIDI · KeyStep" (1180: "MIDI") | Meter, "CLIP" chip, "no signal"; no device readout. "no signal" is capture state; the missing MIDI device readout and dB value are real | [g12](sweep/crops/g12-status-bar__dark__1440x900.png) |
| G13 | low | S | none | About | 3-line copyright, close X in the header | About text runs about 8 lines (full GPL notice), no header X. Probably intentional for licensing; confirm | [g13](sweep/crops/g13-about__dark__1440x900.png) |
| G14 | low | S | none | More controls | On-face chips carry an accent sparkle; label pins are small solid glyphs | Chips carry an outline pin; different pin glyph beside each knob | [g14](sweep/crops/g14-more-controls__dark__1440x900.png) |
| G15 | low | S | none | Toolbar at 1180x680 | No tool group, no fullscreen, no search icon | App keeps all three | [g15](sweep/crops/g15-toolbar-1180__light__1180x680.png) |
| G16 | low | S | none | Save as, Tags | "Add tag" is plain text after the chips | "Add tag" is a boxed field; spacing a little tighter | [g16](sweep/crops/g16-save-as-tags__dark__1440x900.png) |
| G17 | low | S | none | File browser, empty folder | "This folder has no .kabl files." and an enabled Open | "This folder has no folders or patches." and Open disabled (the app is arguably right on Open) | [g17](sweep/crops/g17-empty-folder__dark__1440x900.png) |
| G18 | low | S | none | Unsaved changes | "Don't save" is red text only, body looks a size larger | "Don't save" has an outlined button, body slightly smaller | [g18](sweep/crops/g18-unsaved__dark__1440x900.png) |
| G19 | low | S | none | Rename | Field shows the focus ring and caret | Field unfocused in the capture (check live focus) | [g19](sweep/crops/g19-rename-focus__dark__1440x900.png) |
| G20 | low | S | none | Toolbar logo | Vector mark plus "kabl" wordmark | Bitmap wordmark logo. `g4/logo` exists, so this may be a decision; confirm | [g20](sweep/crops/g20-logo__dark__1440x900.png) |
| G21 | low | S | sound work | Learn recipes | 4 recipes (adds "Why no sound?"), done check on a finished recipe, one-line blurbs | 3 recipes, no done state, longer blurbs; Compare panel carries extra paragraphs and a footer note | [g06](sweep/crops/g06-compare-panel__dark__1440x900.png) |
| G22 | low | S | none | Cues launch | Launch chips Step, Bar, 2 bars | Extra "Now" chip | none (see `cues-editor` sheet) |

## Notes
- G1 comes with an inconsistency in the mockup itself: the Rack screen (`directions.html`) has 3 tabs, the surfaces (`g4-surfaces.html`) have 5. Pick the 5-tab set before building.
- G2, G6 depend on #75 (sequencer paging and length); G3 does not need it.
- G7 and G21 need sound work (previews, recipe content), not only UI.
- Suggested order: G1, G3, G5, G4 (small and visible), then G10, G11, G12; G2 and G6 with #75.
