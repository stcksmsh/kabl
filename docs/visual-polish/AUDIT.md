# Visual polish audit

Audit of the current release editor against the bar Kosta set (Vital, Arturia Pigments). Everything below was
looked at in real captures of the release `kabl-ui` at the head of this branch (origin/master `ad0b0e3`), not
in code alone. Captures are in [`media/`](media/); how they were made is in [`README.md`](README.md).

Kosta's words for the problem: the grid and the ADSR-style displays "look like they're glued on top of
everything". The audit finds that this is true, that it is one root cause repeated, and that most of the other
problems are smaller cases of the same cause: some things are drawn as part of the panel (knobs, jacks, the
face material) and some are drawn as separate objects placed on it (displays, grids, editors, stock widgets),
and the two groups share no edge, depth or colour language.

## What was captured

| Set | Files | Contents |
|---|---|---|
| Faces | `media/faces/faces-<theme>-<mode>-z100.webp`, `-z50.webp` | All 51 module kinds, each alone in a patch, theme A (satin) and B (lacquer), light and dark, at 100% and at 58% (the app's third zoom-out step; "50%" in the brief). 8 contact sheets, 1280x800 release editor. |
| Scenes | `media/scenes/<size>-<scene>-<shot>.webp` | 2x2 sheets (A light, A dark, B light, B dark) at 1440x900 and 1180x680: simple rack, dense rack (routing drawer open and closed), Perform, cable pattern editor, Sounds browser, Save As, Diagnostics, settings menu, About, drum grid, wavetable editor. |
| Plugin | not captured | The plugin editor uses the same renderer at the same size; a standalone capture at 1180x680 is what the scenes show. A REAPER run was not made in this task. |

Face sheet reading order is alphabetical by module kind; the label above each tile is the kind.

## Ranked problems

Ranked by how much each costs the impression, highest first. "Where" names the code that draws it.

### 1. Displays are neutral black slabs dropped onto tinted faces (every face with a display)

Seen in every `z100` face sheet. The display is a flat near-black rectangle with a 1 px dark outline and a
70-alpha strip on its top edge (`kit/face.rs: display_frame`, `DisplayFrame::Glass`). It has no lip where the
panel meets it, no bezel, no inner shadow on the sides, no highlight on the lower edge, and its colour is the
theme's single `disp_bg`, unrelated to the hue of the face it sits in. On a pale salmon, green or lavender
face (theme A light) it reads as a hole punched through to a different object; on the dark faces it reads as
a sticker. The trace colour (amber in A, cyan in B) is good; the frame is what looks glued on.
The wavetable editor and the pattern editor's step bars use the same recipe on a plain window, so the same
slab appears there too.

### 2. Theme B's gloss band is a hard horizontal step through the content (every B face)

`FaceMaterial::Lacquer` paints a lighter band over the top 36% of the face (`surface()` in `kit/face.rs`). Its
lower edge is a straight line at a fixed fraction of the height, so it cuts through knob labels, displays and
the first knob row at different places on every face (see Hi-Hat, Delay, Clap, Compressor in
`faces-lacquer-*-z100`). In A the same area is a smooth gradient and does not show. This is the single most
visible theme-B defect and it is a one-function fix.

### 3. The drum grid is a separate widget on the face (drum.grid)

Captured in `faces-*-drum.grid` and the `drum` scenes. The grid draws its own black cells directly on the
face, with its own pastel velocity colours (salmon, yellow, green, cyan), saturated blue selection chips
(`BRUSH 30 55 78 100`, `100% 75% 50% 25%`) that appear nowhere else in the palette, and its own small
captions at 9.5 px (`BRUSH`, row names). It does not use `display_frame` or the display roles at all
(`drum_grid.rs` reads the panel theme `th`), so it is not in a well; 128 black rectangles sit straight on a pale
face. Playhead and selection are different in A and B. This is the clearest example of Kosta's complaint.

### 4. Three selector styles and stock combo boxes on the same face

Compare Arpeggiator, Delay, Sample & Hold, Random, Quantizer, ADSR:

- dark inset pill with a light active segment (`CONT KEY`, `SIN TRI SAW SQR`),
- light inset pill with a dark active segment (`SLEW FOLLOW`, `SAMPLE TRACK`, `OFF ON` in A),
- stock egui combo boxes with a triangle (`UP`, `1/16`, `MAJOR C`, `x1`, algorithm and table pickers).

`SAMPLE` is clipped by its own pill on Sample & Hold. Selector heights and corner radii differ between the three styles. Text is mono in
some selectors and sans in others. The combo boxes are the one stock-looking control left on the faces.

### 5. Empty and crowded faces (layout rhythm)

- Empty: Comparator, Logic, Clock Divider, Ring Modulator, Sample & Hold, Quantizer, Random, Output, Cues
  and the drum voices between knob row and jacks. A third to half of the face is flat colour.
- Crowded: FM Voice (osc.fm6) has operator rows where `Att 4`, `Dec 4`, `300 ms` touch each other and the
  operator boxes' dark outlines are drawn over the face rather than part of it; the EQ's eight knob labels nearly touch.
- Knob sizes are one size everywhere (about 34 px), so one-knob faces look unfinished and eight-knob faces look
  squeezed.

### 6. The ports area has no structure

Inputs, outputs and the divider rule form two loosely aligned rows with the rule covering only part of the
width (Compressor, Drum Grid, Pan: the rule spans only part of the width). Jack holes are
flat black on every face; they have no seat. `Edit face` is printed at the foot of every face at all times, repeated 51 times. The `+N` chip sits in the title row and collides with long titles:
`Comparato`, `Wavetable Oscillat`, `Sample & Hold` are cut or crowded.

### 7. Type hierarchy is flat

Centred 14 px title, a 10 px mono sub-caption in the low-contrast `ink2` (`osc.va · #1`), 12 px labels, mono
12 px values, 9.5 px captions in the grid, and a mono face for selectors. There are four levels but they are
not distinguished by weight or colour in a consistent way, and the sub-caption is the faintest text on the face
for the least useful information.

### 8. Low zoom drops the content and leaves unlabelled shapes (`-z50` sheets)

At 58% text under 9.5 px is hidden (by design, `FACES-HANDOVER`). What remains: unlabelled dark knobs, a plain
bar with a black block where a selector was, and jacks with no names. The faces keep their colour but read as
a pattern of dots. Displays stay legible (they are the best-looking part of the low-zoom view). Nothing replaces
the hidden text, such as a value arc on the knob or an icon on the selector.

### 9. Editors and inspector use stock-style widgets in a plain window

Wavetable editor (`wt` scenes): an egui window with a title bar, a row of small outlined buttons that look
disabled (Insert, Duplicate, Move earlier, ...), two black slabs (stack, wave) of different heights, a thin stock slider. Cable pattern editor and the routing inspector
(`pattern` scenes): stock slider handles, stock combo boxes (`Add source`), outlined fields, tiny captions.
These are the other place where "glued on" is most obvious, but they are less frequent than faces.

### 10. Theme B specific gaps

- Black jack holes on dark lacquer faces have almost no contrast; the signal rim colour only appears when
  cabled.
- Neon display frame halo touches the label above it (SVF, Formant, Delay) because the halo is outside the
  rect and labels are placed flush.
- Two accents: blue on selection chips, cyan on displays.
- Slots no drawing code reads yet (from `docs/themes/HANDOVER.md`): `plug`, `face.out_plate`, `chrome.navigator`,
  `controls.row`, rack surface, icons. A theme author sees no effect from them.
- At 1180x680 the dark theme B rack and the routing drawer have the same near-black; the drawer edge is
  only a hairline.

### 11. Chrome is in good shape; small things

Perform is the most finished surface (cards, hierarchy, large pads). The browser, About, Save As and
Diagnostics dialogs are consistent and legible in all four themes. Small items: At 1180x680 the routing drawer
takes one third of the width and leaves two rack rows (`1180x680-rack-*`);
the About box is plain text with no layout hierarchy; focus ring and hover
states were not judged (stills only).

## What is already good (keep)

Knob design (cap with skirt in A, glass in B), the face colour families, display trace colours and live
behaviour, Perform, the toolbar, the rack rails, the cable look. The proposal changes how things sit on the
face, not what they are.

## Limits of this audit

Stills only: motion, hover, press and drag states were not judged. The plugin editor was not captured in a
host. "50%" zoom is the app's third zoom-out step (58%). Contrast was not re-measured; the theme tests already
assert the text pairs.
