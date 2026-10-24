# Visual polish proposal: one integration language

Proposal for Kosta. Nothing here is built. The mockups are **drawn** (SVG, by `mockups/make_mockups.py`, using the
built-in theme colours), not rendered by the app: the app cannot yet draw the new frames, and a prototype behind
an environment switch would have meant changing `kit/face.rs` and `rack.rs`, which this task is not for. The
"current" row of every board is a real capture, the two option rows are drawings at the same 1:1 pixel scale
and the real face sizes (210, 240, 300, 660 px wide, 340 tall). What a drawing cannot show: egui has no blur,
so glows and inner shadows in the build are layered strokes and gradient meshes, not the SVG filters used here;
expect them to be a little harder-edged.

Boards: [`media/mockups/board-<theme>-<mode>.png`](media/mockups/) for `kabl.satin` and `kabl.lacquer`, light
and dark (four boards, six faces each: oscillator with display, filter, ADSR, drum grid, compressor with meter,
mixer as the utility). Close-ups at 2x of the oscillator and the ADSR: `closeup-<theme>-<mode>.png`.

## The idea in one paragraph

Today there are two kinds of thing on a face: parts of the panel (the knobs, the jacks, the face colour) and
objects dropped on it (displays, the grid, editors, stock widgets). The fix is a rule, not a skin: **every
element is in exactly one of three layers, and every layer has one recipe.**

| Layer | What lives there | Recipe |
|---|---|---|
| **Panel** | face colour, title, labels | one material, lit from the top |
| **Well** | anything that shows or edits data: displays, the drum grid, the wavetable stack, pattern bars, the port tray | cut into the panel: same frame recipe, same inner margin, same corner radius, wherever it appears |
| **Hardware** | knobs, jacks, selectors, buttons | stands proud of the panel or the well; own shadow; never directly on a bare slab |

The same rule fixes the grid (a well with pads in it), the ADSR (a well with a curve in it), the editors (wells in
a window) and the stock combos (hardware that matches the selector).

## Shared by both options

These are the same in Option 1 and Option 2; they are most of the improvement and need no choice.

**Spacing system.** 4 px base. Face margin 12 px. Header 40 px (title row plus caption row). A single display is
60 px high, full width inside the margins; a well that holds an editor (grid, stack) takes the middle of the
face. The **port tray** is 88 px high at the bottom of every face with two rows: inputs on top, outputs below
(row centres 19 and 59 px into the tray, labels under the jacks). The knob zone is what is left, and the knob
block is centred in it vertically. Gaps between a label, its knob and its value are always 10 and 17 px.

**Knob size follows count.** Radius 24 for 1 to 2 knobs, 20 for 3, 17 for 4 and more, 18 when a selector shares
the zone. Value readout directly under the knob, label directly above, so one-knob faces stop looking unfinished
and eight-knob faces stop colliding. Faces with no display (mixer) lay out in a 2x2 or 3x2 block with the larger
knob.

**Type, four steps, one rule.** Title 14 px SemiBold, left aligned, section ink. Caption 11 px mono, `kind · #id`,
`ink2`, under the title. Labels 11 px Medium, section ink. Values and selector states 11 px mono. The rule: mono
means a number, an id or a state; sans means a name. The 11 px floor stays. Titles are left aligned so the `+N`
chip (the "more controls" count) sits at the right of the same row and can no longer collide with a long name;
the title shrinks one step before it truncates.

**Selectors: one idiom.** A dark segmented control, 22 px high, in the header row for mode switches that the
knobs do not need space for (ADSR `CONT/KEY`, the drum grid's pattern, brush and chance), or under the knob
in the zone (oscillator waveform). The stock combo boxes on faces become a kit drop-down with the same
height and radius.

**Faces never print `Edit face`.** It appears on hover and in the context menu only.

**Theme B's gloss band becomes a continuous top light.** The step at 36% height goes; the whole face gets a
gradient. This one is independent of the options and is the first fix to build.

**Jack positions.** Inputs move down by about 11 px and outputs by about 3 px to sit on the tray's two row
centres (today 248 and 296 px from the top of the face). Port identities do not change; cables follow. This is
the same kind of move Kosta already accepted for faces with displays.

**Drum grid becomes a well.** The brush, pattern and chance controls move into the header row as three kit
segmented controls (the active segment uses the theme's one active colour, so the blue chips go).
Cells sit in the well on a 4-step grouping (extra gap every four steps), 16 steps fill the width, row names are
11 px mono inside the left gutter, the playhead is a column highlight. Pads take the row hue; off cells are the
well colour plus 6 to 8% light, not black.

## The real choice: how the well is made

### Option 1: Seated (hardware depth)

The Arturia Pigments direction. The face is a soft satin (gradient halved from today), with a 1 px light edge
on top and nuts. A well is physically cut in: 2 px dark bezel, a lower lip highlight, an inner shadow at the
top and sides, a faint diagonal sheen on the glass. The display content is lit (soft glow on the trace, glowing
pads in the grid). The port tray is a recess with a top inner shadow; jacks are metal nuts in it. Knobs keep the
cap with a tick scale. Light sits at the top; everything casts a short shadow down.

- Best: it is the closest to what the faces already are, so it reads as a finished version of the current look.
  It makes the "glued on" feeling go away by giving the join a believable edge.
- Cost: the most drawing (shadows, gradient meshes, glow strokes) and the most to tune; the largest risk for
  frame time on the dense patch. It also needs the most per-theme care in light mode, where shadows on pale faces
  can look dirty.

### Option 2: Tinted (flat panels, colour continuity)

The Vital direction. The face is almost flat (a 4% top light, a hairline edge, no nuts). A well is a deeper
tone of the **face's own hue** with a 1 px hairline: no bezel, no glow outside, no black. Trace and active
segments use the theme's trace colour so the data pops against the tint. The port tray is a darker flat panel;
jacks are flat donuts. Knobs lose the cap and become a flat disc inside a value arc (track and value stroke),
which also tells the value at low zoom without text.

- Best: cheapest to draw and to run, the most robust at low zoom (the arc survives when text is hidden), and
  the cleanest way to make the drum grid and the editors part of the panel: they are the same flat tint.
  Dark mode in particular looks calm and current.
- Cost: it leaves behind the "physical module" character Kosta recorded as preferred (continuous coloured satin
  faces, recessed hardware). Light mode can look flat on pale faces unless the tint depth is tuned.

### Mapping and recommendation

Both options apply to both themes (the theme still changes palette, type case and accents). **Recommended:
Option 1 for theme A (Satin Studio), Option 2 for theme B (Lacquer and Light).** A is the theme Kosta chose for
continuity, and Option 1 is its finished form; B is the dark-first, display-led theme, and Option 2 gives it
the Vital-style display-first calm and a cheaper per-frame cost. This needs no new theme architecture: the
slots already exist (`face.material`, `display.frame`, `knob.body`, `knob.scale`, `jack`, `selector`); each
option adds recipe values and a few dials (see `BUILD.md`). Kosta can also choose one language for both, or
choose Option 2 as the default and keep Option 1 as a third built-in theme later.

## What the boards show

| Board | Look at |
|---|---|
| `board-kabl.satin-light` | Option 1 against today: the oscillator display is seated, the grid is one well with lit pads, the jack tray is a recess. Compare "VA Oscillator" and "Drum Grid". |
| `board-kabl.satin-dark` | the amber trace and the glass at night; Option 2 is darker and calmer. |
| `board-kabl.lacquer-light` | the end of the gloss step (compare the current Compressor face with either option); neon halo replaced by the tinted well. |
| `board-kabl.lacquer-dark` | the lowest-risk-looking pair; Option 2's grid and ADSR. |
| `closeup-*` | 2x of the oscillator (display, one large knob, selector, tray) and the ADSR (mode selector in the header, four-knob row). |

Known weaknesses of the drawings, so they are not a surprise later:

- Narrow faces (210 px) have no `IN`/`OUT` tags; the rows are told apart by order and spacing. Wide faces show
  the tags. If that is not enough, a 1 px divider between the rows is the fix.
- The `Cutoff CV` and `Res CV` labels on the SVF are 2 px apart at 210 px; the real build wraps or shortens to
  two lines (decision for Kosta: labels may wrap to two lines on narrow faces).
- The compressor GR meter is a plain bar; the real display keeps its live value behaviour.
- The utility (mixer) has no display, so the drawings fill the zone with bigger knobs; an alternative is a
  tiny input meter strip, which would be new content and is not proposed.

## Things this proposal does not change

Face sizes and the rack grid, module positions, port identities, the colour families per section, cable look,
Perform, the toolbar and the status bar, anything on the audio path. The editors (wavetable, pattern, inspector)
change look only: their controls become kit controls and their displays become wells, and nothing about what
they edit changes.
