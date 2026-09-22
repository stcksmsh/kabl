# Interaction specification — revision 2

This file adds to and overrides [`../INTERACTIONS.md`](../INTERACTIONS.md) (revision 1). Where
the two disagree, this file wins. Nothing described here is built. Geometry lives in
[`render2.py`](render2.py), which extends [`../render.py`](../render.py).

Every rule is tagged with one of these labels:

- **[owner]**: confirmed by Kosta at G1 (decisions.md, 2026-09-22 G1 entries). Not reopened here.
- **[rec]**: the agent's recommendation. Not approved.
- **[open]**: explicitly left for later.
- **[engine]** / **[core]**: needs work beyond the UI.

## 1. Viewport

- **[rec]** Default design viewport is **1440×900**. At 900 px, the extra height becomes a 50 px
  cable channel between the two rack rows (rows at y 70 and 480). Cables that cross between
  rows sag into the channel instead of into panels.
- **[owner]** 1280×800 stays usable. It uses the revision-1 row positions (62 / 426). Measured:
  with the routing drawer open, 944 px of rack stays visible. The expanded LFO (600 px) fits
  whole, and the rest of the row pans.

## 2. Modulation model

- **[owner]** Any knob accepts a modulation cable. There is no CV jack per parameter.
- **[owner]** Amount, polarity and bypass belong to the **route**. The cable, the drawer row,
  the jack badge and the knob ring are four views of one route.
- **[rec]** Jacks remain only for signals that *are* the module's function: audio in/out,
  pitch, gate/trigger, and the VCA's CV (multiplicative gain, not a parameter offset). The
  filter's `Cutoff CV` and `Res CV` jacks disappear from the panel. The existing engine inputs
  (`cutoff_cv`, `resonance_cv`) become the implementation behind the Cutoff and Resonance knob
  destinations. **[engine]** The compiler must map knob routes onto them, or onto a generic
  param-modulation input. `ProcessIo::param` is already a `Signal`, so this is compiler work.

### 2.1 Anatomy of a modulated knob

| Element | Shows | Always visible? |
|---|---|---|
| Pointer on the cap | **base value** (what the knob itself is set to) | yes |
| Value text | base value, in units | yes; inside a **pill** with a coloured edge when modulated |
| Outer ring (r + 11) | **resulting range**: every active route summed, then clamped | yes. Faint when idle, strong when its source or the knob is selected |
| Flat cap bar at a range end | the clamp is reached there | only when clamped |
| Dashed faint arc past the cap | depth lost to the clamp | only when clamped |
| Dot on the ring | where the source's **positive peak** lands, which shows polarity | single-route knobs |
| Inner lanes (r + 18, r + 24 …) | **depth per source** | only while the knob is inspected and has ≥ 2 routes |
| Plug at 6 o'clock | the cable end (All/Focus) or a small dot (Hidden) | yes |
| Badge `← LFO` / `← 2 mods` | the source | Hidden mode only |

- **[rec]** The plug docks in the 6 o'clock gap that a 270° knob scale leaves empty. Up to
  3 plugs fan out, 15 px apart. A 4th or later route collapses into one `+N` plug.
- **[rec]** Knob routes draw as **mod leads**: 4 px instead of 5.5 px, at 72% opacity. They
  draw at full opacity when their source or destination is selected or inspected. The value
  pill draws **above** cables, so a lead never hides its own knob's value.
- **[rec]** Hidden-mode knob badge placement: below the pill. If that spot collides with
  another label, the badge goes beside the pill, on whichever side is clear. The renderer
  checks the collision.

### 2.2 Units, ranges, limits

- **[rec] Amount unit = fraction of knob travel, −100% … +100%.** Travel is measured in the
  param's own taper, so it is logarithmic for exponential params. One unit works for every
  knob. Surge and Vital do the same.
- **[rec]** The UI always also shows the effect **in destination units at the current base**,
  for example `Attack 8 ms · swings 0.45 – 142 ms`. Numeric entry accepts either form (`+25 %`,
  or `1.5 oct` for exponential params).
- **[rec]** Bipolar source (LFO BI, ±1): the range is base ± |amount|. Unipolar source (ADSR,
  velocity, LFO UNI, 0…1): the range is base … base + amount.
- **[rec] Polarity = the sign of the amount.** `Invert` flips the sign and keeps the magnitude.
  For a bipolar source, inverting does not change the ring's extent, so the peak dot and the
  route card carry the difference.
- **[rec] Limits:** the engine clamps the result to the param's range. The ring shows the
  clamp.
- **[rec]** The default amount on drop is **+25%**, so the effect is visible and audible at
  once. Alt-release drops at 0%. Existing jack cables keep amount 1.0, so old patches sound
  the same.

### 2.3 Combining sources

- **[rec]** Sum every active route in knob-travel space, add the sum to the base, then clamp
  once. There is no per-route clamp. Multiply/ring combination modes: none until a real patch
  needs one.
- **[rec]** Revision 1's "an input holds one cable, drop offers Replace" rule no longer applies
  to knobs. Knobs take several routes. It still applies to **jack** inputs until the engine
  sums cables there. **[engine]** Today only the first cable into an input counts.

### 2.4 Discrete controls (selectors, switches)

- **[rec]** A route to a selector moves it by whole options. amount × (options − 1) is rounded
  to steps (±1 step at 34% on a 4-option selector). The result snaps to the nearest option,
  with a ±0.1-step hysteresis so it never chatters.
- **[rec]** A bracket under the selector shows the reachable options. The filled segment stays
  the base. On/off switches flip at half travel.
- **[rec]** A change applies at the next block. **[engine]** Most modules read discrete params
  once per block (`io.param(...).at(0)`: LFO waveform, VCA response).

### 2.5 Voice and global behaviour

- **[rec]** Voice source → voice destination: per voice. Global → voice: every voice gets the
  same value. Voice → global: **averaged across voices**, which is the compiler's current rule.
  The route card says so.
- **[rec]** Evaluation rate follows the destination. Params that the module reads per block
  (ADSR times, LFO rate and waveform today) update per block. Params that take a `Signal` (osc
  frequency, filter cutoff/resonance, VCA gain) can update per sample. The route card states
  `block rate` where it applies.
- **[rec]** Envelope times (Attack …) are modulated continuously. The stage in progress follows
  the new time. Sample-at-stage-start is the alternative. **[open]** It changes the sound, so
  choose it by ear in the prototype.
- **[rec]** Feedback (an LFO's Out → its own Rate) is allowed. It goes through the existing
  cycle handling and adds a 1-block delay, which the route card states.

## 3. Primary and advanced controls

- **[owner]** Each module declares default primary controls. The user chooses which ones stay
  primary. Advanced controls appear when the module expands **in place**.
- **[rec] Face rule:** the compact face shows exactly the primary controls, in the module's
  declared order. The face widens with the number of primary knobs: ≤ 2 → 7u, 3 → 8u, 4 → 10u.
- **[rec] Expansion rule:** expanding never moves a face control. The advanced area is appended
  to the right, behind an engraved divider. The name stays centred over the face. The toggle
  sits in the header's right corner: `+9` (the count of controls not on the face) when
  collapsed, `Less` when expanded. It wins hit-testing over the header drag zone.
- **[rec]** **Jacks are always on the face.** Collapsing never hides a cable end.
- **[rec]** Expansion pushes the modules to its right along the row, and cables follow their
  jacks. Other rows never move. If the expanded module would leave the view, the rack pans to
  keep it whole. Measured at 1280×800 with the drawer open: the reference LFO (x 234, 600 px wide)
  fits with no pan. Any 600 px module that starts right of x ≈ 332 needs a pan.
- **[rec]** Choose primary: context menu `Choose primary controls…` puts the module into
  expanded + choose mode. Every control shows a pin. Filled pin = on the face. The header
  shows the count and the resulting face width, then `Done`. The choice is saved per module
  instance in the patch, and it is undoable (one op). `Reset to module default` is in the
  context menu. **[core]** Needs a per-instance UI-state field.
- **[rec] A modulated control hidden by collapse:** the route docks on the `+N` button, which
  gets the route ring (and in Hidden mode a `◆ 1 hidden` badge). Its tooltip names the route.
  Clicking the button expands the module and flashes the target. A route is never silently
  hidden.
- **[open]** Expansion pushes neighbours, and long cables that pass the expanded module then
  cross its advanced area (see `lfo-expanded-rack.png`). The mitigation shown is Focus
  (`lfo-expanded-focus.png`). The alternatives (auto-Focus while expanded; a floating
  expansion that overlays neighbours) need the hands-on prototype.

## 4. Rich LFO (design)

Engine today: `rate_hz` 0.01–100 Hz (exponential) and `waveform` (SIN TRI SAW SQR S&H),
per-voice rate, both read once per block. Everything else below is **conceptual (◆)**.

| Control | Range / options | Default | Face by default |
|---|---|---|---|
| Rate | 0.01–100 Hz, exponential. Shift-drag = fine ×0.1; numeric entry | 0.80 Hz | ✓ |
| Waveform | SIN TRI SAW SQR S&H | TRI | ✓ |
| Fine ◆ | ±10 % of Rate: a detune, for two LFOs drifting against each other | 0 | |
| Phase ◆ | 0–360°. Start phase on (re)trigger | 0° | |
| Fade-in ◆ | 1 ms – 10 s, exponential. Amplitude ramps up after each trigger | 1.2 s shown | |
| Amplitude ◆ | 0–1 | 1.00 | |
| Offset ◆ | −1…+1. Added after amplitude; the output is clamped to ±1 | 0 | |
| Sync ◆ | FREE / TEMPO. In TEMPO, Rate shows divisions (1/16 … 8 bars) and modulating it steps through them. **[engine]** needs a clock | FREE | |
| Polarity ◆ | BI (±1) / UNI (0…1) | BI | |
| Trigger ◆ | FREE (runs freely) / NOTE (restarts at Phase on note-on) / ONCE (one cycle, then holds, like an envelope) | NOTE | |
| Scope ◆ | VOICE (one LFO per voice, today's behaviour) / GLOBAL (one LFO shared by all voices, for slow sweeps across a chord) | VOICE | |
| Trig ◆ (gate jack) | restarts the phase on a rising edge | — | jack, always on face |

Output = clamp(offset + amplitude × fade(t) × shape(phase), −1, 1). With UNI, the shape is
mapped to 0…1 first. The expanded panel has an **output preview** drawn from these params,
not from telemetry. Every control above is a modulation destination (§2).

## 5. Storyboard: LFO → ADSR Attack

[`img/storyboard.png`](img/storyboard.png), frames `img/story-1…8.png`, at 1440×900:

| # | Gesture | Feedback | Commit / undo |
|---|---|---|---|
| 1 | Press on LFO Out and drag | Every knob gets a dashed blue target ring. Compatible jack inputs get the revision-1 target ring. Hovering Attack: strong ring + tooltip `Release to modulate (+25 %)` · `Alt-release: +0 %` | Esc / release on empty space cancels |
| 2 | Release on Attack | Plug docks at 6 o'clock. Range ring, peak dot, value pill. Tooltip `+25 % · Base 8 ms · swings 0.45 – 142 ms` | one `Connect` op (route with amount) |
| 3 | Drag the **ring** (outer band) | Handle on the ring. Tooltip updates live (`+40 %`, now clamped at 0.1 ms). The knob body still sets the base | one route `SetParam` on release |
| 4 | Route card `Invert` | Amount `−40 %`, the peak dot moves to the low end. The drawer shows the card for the selected source | one route `SetParam` |
| 5 | `Bypass` | Cable dashed and grey. Ring becomes a hollow grey outline. Pill edge grey. The card says `Bypassed: no effect, amount kept` | one route `SetParam` **[engine]** |
| 6 | `Cables: Hidden` | Same layout. Attack keeps ring + pill + `← LFO` badge. LFO Out shows `→ 2 routes` | view only; not undoable |
| 7 | Click Attack's ring | Destination inspector: base value + Reset, range bar (▲ base, thin bracket = depth per source, thick bar = result), source cards | view only |
| 8 | `Undo` | Reverts Bypass, the last patch edit. The result bar returns. Toast `Undid: Bypass LFO → Attack · Redo` | — |

Ring vs body: the knob body (cap + skirt, radius r + 2) adjusts the base. The ring band
(r + 7 … r + 15) adjusts the depth of the **selected** source, or the most recent route when
none is selected. Hit regions: both are inside the knob's 2r + 8 square. The ring band gets
priority only while the knob has routes. **[open]** This split exists only on paper. It needs
the prototype to show that it doesn't cause mis-grabs at r = 17.

## 6. Difficult cases ([`img/cases-sheet.png`](img/cases-sheet.png))

1. **Several sources on one knob:** §2.3. The fan-out plugs, the sum ring, and the per-source
   lanes while inspected.
2. **Discrete control:** §2.4.
3. **Hidden by collapse:** §3, the `+N` button.
4. **Modulating an LFO:** an ordinary knob route on Rate. ±12 % of travel on 0.01–100 Hz =
   ×0.33…×3. Self-modulation adds a 1-block delay (§2.5).
5. **Cable crossing a control [rec]:** the control wins hit-testing. A cable is selectable only
   at its plugs and over bare rack. Hovering a control that a cable covers fades the crossing
   cables to 25 % ("x-ray") until the pointer leaves. Focus and Hidden solve it for good.
6. **Skin behind labels:** §7.

## 7. Illustrated skins ([`img/skin-sheet.png`](img/skin-sheet.png))

- **[owner]** User and non-core modules may carry illustrated skins with light and dark
  variants. The UI renders the controls, essential labels and values over the artwork.
- **[rec]** A skin is art only: `light.png`, `dark.png`, one accent colour, and optional
  "busy zones". It has no text and no controls. Every control group sits on a **plate in the
  theme's colour**, not in the art's colour (the revision-1 C clear-zone rule). Text contrast
  therefore never depends on the picture. Measured against the placeholder art's extreme
  colours, the worst case is 10.2:1 (dark) and 11.1:1 (light).
- **[rec]** If a skin has no `dark.png`, the light art is dimmed 35 % under dark plates.
- Core modules stay A / A-dark **[owner]**.

## 8. Unchanged from revision 1

Cable presentations All/Focus/Hidden **[owner]**. Positions never change between them. Badges,
drawer rows, keyboard, numeric entry, overview/popover, and the rest of revision 1's action
table.
