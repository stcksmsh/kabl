# Layout and interaction model (P2)

Proposed behaviour for the rack UI. None of it is built. Where this needs more than UI work, the
dependency is marked **[engine]** or **[core]**. Layout geometry lives in one place,
[`render.py`](render.py). Every mockup is drawn from it, so the geometry cannot drift between
directions.

## Layout at 1280×800 (minimum design viewport)

| Region | Size | Contents |
|---|---|---|
| Top bar | full width × 48 | patch name + save state, Undo, Redo, Cables `All / Focus / Hidden`, Zoom `− % + Fit`, Modules, Routing, Out level |
| Rack | the rest | two rack rows. Panel height 340, rails 10, width unit 30 px. Empty rail shows `+ Add module` |
| Routing drawer | 336 wide, right side, collapsible | only when opened. Takes width from the rack. Never overlays it |
| Module browser | collapsible, left side | not drawn in the mockups. Opens from `Modules` |
| Footer | 24 | concept label (mockups only) and a one-line hint for the current mode |

- There is no permanent inspector. Parameters live on the panels. The drawer holds routing only.
- Module positions never change between cable presentations. The drawer shrinks the visible rack.
  If the selection would be covered, the rack pans to keep it visible.
- Reference patch order: row 1 has Oscillator, LFO, Filter; row 2 has MIDI In, ADSR, VCA, Output.
  This order keeps the two cables that cross between rows (MIDI pitch up, Filter LP down) clear of
  knobs.
- Widths come from `ModuleInfo::width_units` × 30. The concept proposes one change: **VCA 6u → 7u**.
  At 6u, the hidden-mode badges of the three VCA jacks collide. All other widths are unchanged.

### Panel grammar (shared by all three directions)

- Header zone (top 52 px): name at 15 px semibold, with kind in 11 px mono underneath. This zone is
  also the **drag handle** for moving the module. Long names wrap to two lines, then end in an
  ellipsis; the kind line is dropped when the name needs two lines. The full name appears in a
  tooltip.
- Knob: label above, value below, 13 px. Knobs on the same row share label and value baselines.
  Large knob radius 22, small knob radius 17. The value text is always drawn by the UI and is never
  part of the artwork.
- Discrete params use a segmented selector (osc/LFO waveform, VCA response), not a knob. The
  engine stores these as stepped floats. That is invisible to the user.
- Jack: the label sits **above** the jack, so a hanging cable never covers it. Outputs sit on a
  contrasting plate (the VCV convention). The port-type colour appears only on cables and on
  hidden-mode rings, always as a two-tone (colour + dark edge).
- Parameter visualisations (envelope shape, LFO wave) are drawn from params. They are not
  telemetry.
- Artwork (skin PNG, or printed art in A/B) sits under controls. In C, every control group gets a
  plain "clear zone" plate so the art never touches text.

## Cable presentations

| | All | Focus | Hidden |
|---|---|---|---|
| Cables | drawn with sag, plugs, shadow | cables touching the selection solid, the rest at 13% | not drawn |
| Jacks | the plug covers a connected jack | same | two-tone ring in the port-type colour + badge naming the other end (`→ Filter`, `← MIDI`, or `→ 2 routes`) |
| Modulation ring on a destination knob | faint | faint, strong when its source is selected | same |
| Editing | drag jacks, drawer optional | same | drawer rows + badges. Every connection stays editable, audio included |

The same cable ID (`C1`…`C8`) is behind the plug, the badge, and the drawer row. Cable
presentation is a view setting. It is never patch data, and it is not an undoable op.

## Actions

| Action | Entry | Feedback | Commit | Cancel | Undo |
|---|---|---|---|---|---|
| Connect | press on a jack and drag. **Alternative:** click jack A, then click jack B; or drawer `+ New route…` with source/destination menus | ghost cable follows the pointer. Suitable inputs get a blue ring. Outputs and the source dim. Hovering a target shows a tooltip `Filter · Cutoff CV — release to connect` | release over an input | Esc, or release over empty space | one `Connect` op |
| Repatch | drag an existing plug off its jack | the old cable stays live and draws ghosted until release | release on the new input | Esc puts the plug back | `Disconnect` + `Connect`. **[core]** needs op grouping for this to be one undo step |
| Occupied input | drop on an input that already has a cable | tooltip `Replace ← ADSR?`. The old cable ghosted | release replaces | Esc | same as repatch. Summing several sources is **[engine]** work. Today only the first cable counts, so the UI must never allow a second one silently |
| Adjust knob | vertical drag. Shift = fine (×0.1). Double-click the value = numeric entry with units (`1.35 kHz`, `1350`, `-6 dB`). `Reset` in the context menu, or Alt-click | the skirt lights while dragging. The value text updates live | release / Enter | Esc during the drag or entry restores the old value | one `SetParam` per gesture (commit on release, not every frame) |
| Discrete value | click a segment. Arrow keys when focused | selected segment filled | click | — | one `SetParam` |
| Inspect cable | click a plug or an exposed stretch of cable. Or a drawer row | both endpoints ring blue. The drawer row highlights | — | Esc clears | — |
| Wire over a knob | pointer over a knob that a cable crosses | the knob wins hit-testing. A cable is only selectable at its plugs and in empty rack space | — | — | — |
| Several cables on one jack | click a jack that has more than one cable (an output with fan-out) | a list of named connections next to the jack, e.g. `VCA out → Output L`, `→ Output R` | pick one to select it | Esc | — |
| Select mod source | click a source module or its output | its destinations' modulation rings turn strong. The drawer (if open) shows `SELECTED SOURCE` with its route cards | — | Esc | — |
| Set mod amount | with the source selected, drag the destination knob's **ring**; or the drawer slider; or the drawer value box | tooltip `LFO → Cutoff +0.75 oct · sweeps 714 Hz – 2.02 kHz` | release | Esc | one cable `SetParam`. **[engine]** the amount is ignored today |
| Inspect destination | click the modulation ring (or context menu `Modulation sources`) | the drawer switches to `Filter · Cutoff`: base value, then every source with amount, Bypass, Remove | — | Close / Esc | — |
| Bypass vs Remove | route card buttons | Bypass keeps the cable, drawn dashed / greyed, and its amount is remembered. Remove deletes it | click | — | Bypass: cable `SetParam` **[engine]**. Remove: `Disconnect` |
| Hide cables | `Cables: Hidden` | same layout. Badges appear. The drawer is suggested but not forced | — | — | view only |
| Navigate | scroll/drag empty rack = pan. Ctrl+wheel / `− +` = zoom. `Fit` = whole patch. `F` = focus the selection | a scrollbar on the bottom rail while panned | — | — | view only |
| Overview (< 75% zoom) | zoom out | panels simplify to name + bare controls (labels would be unreadable). The selected module opens a **100% popover** with live controls | — | Close | — |
| Move module | drag the header zone | a snap preview on the rail. Neighbours shift to show where it will be inserted. Overlap is never allowed | release | Esc | one `MoveModule` per module moved. Several moves need **[core]** grouping |

Keyboard: Tab walks modules and then their controls in reading order. Arrow keys adjust a
focused control (Shift = fine). Enter opens numeric entry. Esc cancels any gesture. Focus is shown
with a dashed frame around the whole control, which is distinct from the hover/drag skirt.

## Modulation rules for the example route (design level)

- **One model.** A modulation route *is* a cable into a CV input. The drawer row, the badge, the
  plug and the knob ring all show the same `CableState`. There is no second modulation table.
- **Amount.** A proposed cable param `amount` is a multiplier on the source signal, range
  −2…+2, default **1.0**. A default of 1.0 keeps every existing patch sounding the same. The
  amount is shown in the destination's own unit. `cutoff_cv` is exponential (`cutoff × 2^cv`), so
  1.0 = 1 octave: `+0.75 oct`. Linear inputs (`resonance_cv`, `vca.cv`) show plain numbers.
  **[engine]** the compiler must apply the amount. **[core/modules]** per-input display units need
  a small metadata field.
- **Polarity** = the sign of the amount. The ring draws the real range. A bipolar source (LFO,
  ±1) gives base ± amount. A unipolar source (ADSR, 0…1) gives base → base + amount.
- **Limits.** The engine clamps the result to the param range (cutoff 20 Hz–20 kHz). The ring
  stops at the clamp and shows a flat cap there.
- **Combining sources.** Proposed: an input holds one cable (the occupied-input rule above).
  Summing several sources into one input is later **[engine]** work. When it lands, the destination
  inspector already lists several sources.
- **Scope.** The LFO is voice-rate: each voice has its own LFO. The route card says
  `per voice`. When a voice-rate source feeds a global-rate input, the card says
  `averaged across voices` (the compiler's current rule).
- **Base value vs modulation.** The knob pointer is always the base value. The ring outside the
  knob is the range. A live "current value" dot needs **[engine]** telemetry and is not drawn.

## Storyboard

[`storyboard.png`](storyboard.png) (frames are in `directions/a-warm/story-1…5.png`):

1. Drag from LFO out. CV inputs light up, and outputs dim.
2. Connected. With LFO selected, drag Cutoff's ring to set `+0.75 oct`.
3. Cables hidden. The layout is unchanged, and the badges name each jack's other end.
4. Click the modulation ring. The drawer inspects `Filter · Cutoff`: base value, sources, amount,
   Bypass, Remove.
5. Undo. The amount returns to `+1.00 oct` (the default on connect), and a toast offers Redo.
