# Design brief — physical rack proposal (P1)

Concept work for [`../PLAN.md`](../PLAN.md). Nothing here is shipped UI. Code was checked at
`9015609`.

## Goal

Show Kosta one instrument in three visual directions. Each direction appears with cables visible
and with cables hidden. The instrument should look like a physical Eurorack case: panels with
image skins, cables with weight, and a Surge XT-style modulation view that stays usable when the
cables are hidden. The priorities are, in order: **visibility, usability, beauty**. Artwork never
takes precedence over readable controls.

## Constraints from reality

- The concepts stay inside what `ModuleInfo` already describes: kind, name, category, rate, ports
  (Audio / Cv / Gate / Pitch), params (min/max/default/unit/taper), `width_units` and an optional
  `ModuleSkin` (background PNG, with controls at normalized positions). The concepts invent no
  controls that a module does not have.
- The engine has no param modulation. A modulation route is a **cable into a CV input**. For
  example, `filter.svf`'s `cutoff_cv` input computes `cutoff = cutoff_hz × 2^cv`, so an LFO
  running at ±1 moves the cutoff ±1 octave, with a fixed depth.
- `CableState.params` and `ParamTarget::Cable` exist in core, but the compiler ignores them. So a
  per-cable amount, polarity or bypass setting **requires engine work** ("cable depth, v2" in
  compile.rs).
- **Fan-in:** when several cables enter one input, only the first one is used and the rest are
  silently ignored (compile.rs, `incoming.iter().find`). The concepts must not suggest that
  sources sum today.
- Fan-out from one output works (voice-rate outputs are averaged into global inputs).
- The routing drawer and the cables are one model: `Op::Connect` / `Op::Disconnect` / `SetParam`
  on cables. Both are undoable through the op log.
- The existing `kabl-ui` has a Patchbay view and a grid-snapped Eurorack view. Only `osc.va` is
  skinned. There is no zoom, no cable-visibility setting, no drawer and no numeric entry.
- Tooling: this session has no image generation. Layout is drawn with vector graphics from one
  script (`render.py`). Bespoke raster art is requested through prompts
  ([`IMAGEGEN_PROMPTS.md`](IMAGEGEN_PROMPTS.md)), and the mockups say where that art is missing.

## Reference patch (identical in every mockup)

Four voices. Two rows in the rack. Width unit = 30 logical px, panel height = 340 px.

| Module | Width | Controls: value shown | Ports: in / out |
|---|---|---|---|
| MIDI In `midi.in` | 5u | none | — / gate, pitch, velocity |
| VA Oscillator `osc.va` | 7u | Frequency 261.6 Hz, Waveform Saw (sin/tri/saw/sqr) | pitch, sync / out |
| SVF Filter `filter.svf` | 7u | Cutoff 1.20 kHz, Resonance 0.35 | in, cutoff_cv, resonance_cv / lp, bp, hp |
| LFO `lfo` | 7u | Rate 0.80 Hz, Waveform Tri (sin/tri/saw/sqr/S&H) | — / out |
| ADSR Envelope `env.adsr` | 8u | Attack 8 ms, Decay 240 ms, Sustain 0.60, Release 420 ms | gate / out |
| VCA `vca` | 7u (6u today, see INTERACTIONS.md) | Gain 0.85, Response Lin (lin/exp switch) | in, cv / out |
| Output `out` | 5u | none | left, right / — |

Row 1: Oscillator, LFO, Filter. Row 2: MIDI In, ADSR, VCA, Output. This order was chosen during P3
so that cables crossing between the rows miss the knobs.

| # | Connection | Type |
|---|---|---|
| C1 | MIDI In.gate → ADSR.gate | gate |
| C2 | MIDI In.pitch → Oscillator.pitch | pitch |
| C3 | Oscillator.out → Filter.in | audio |
| C4 | Filter.lp → VCA.in | audio |
| C5 | ADSR.out → VCA.cv | CV |
| C6 | LFO.out → Filter.cutoff_cv, **amount +0.75 oct (proposed)** | CV (modulation) |
| C7 | VCA.out → Output.left | audio |
| C8 | VCA.out → Output.right | audio |

C6 is the example modulation route. Shipped behaviour: a fixed ±1.00 oct swing. The proposed
per-cable amount would give ±0.75 oct, which sweeps the cutoff from 714 Hz to 2.02 kHz around the
1.20 kHz base. The engine's 20 Hz–20 kHz clamp still applies. The LFO is voice-rate, so each voice
has its own LFO. The concepts label this "per voice".

## Capability map

| Feature shown | Status |
|---|---|
| Modules, ports, params, width_units, skins, grid snap, pan | existing |
| Connect / disconnect / move / param change, undo/redo | existing (ops + log) |
| Port-type colours | existing metadata; the palette is proposed |
| Sagging physical cables, cable presentation All / Focus / Hidden | proposed, UI only |
| Zoom, Fit Patch, Focus Selection | proposed, UI only |
| Routing drawer rows, connection badges on jacks | proposed, UI only (reads the existing cable list) |
| Numeric entry, fine drag, reset, discrete selectors | proposed, UI only |
| Modulation range arc on a destination knob | proposed; needs only the params + cable amount |
| Per-cable amount / polarity / bypass | **requires engine work** (compiler must read `CableState.params`) |
| Several sources summed into one input | **requires engine work** (fan-in rule) |
| Live modulated-value dot, level meter | **requires telemetry**; not drawn as real data |
| Master output level | **requires engine work** (no master gain exists) |

## References (principles only; no art is copied)

- VCV panel guide: generous jack spacing, labels always readable, outputs marked with a
  contrasting plate, artwork kept separate from interactive components.
- VCV view menu: cable opacity/tension and zoom are user settings, not per-patch data.
- Surge XT modulation: select a source, and its destinations show depth arcs; the destination
  lists every contributing source; routes can be filtered, bypassed and removed.

## Assumptions (reversible)

- 1280×800 logical px is the minimum design viewport. It is checked again at 200% scale.
- Font: IBM Plex Sans / IBM Plex Mono (installed locally; OFL). Essential labels are 13–14 px.
- One rack layout for all three cable presentations; module positions never move.
- No owner-held brief section is needed for this exercise.
