# Image-generation prompts (bespoke art for direction C)

This session could not generate images. Direction C's module art is currently a **procedural
vector placeholder** drawn by `render.py`. These prompts are meant for a separate image-generation
agent.

Directions A and B need no raster art. Their panel textures are procedural (SVG noise) and their
printed graphics are vector. Generate A/B textures only if G1 picks A or B and asks for richer
material.

## How the results are used

- Save each result as `docs/design/art/c-illustrated/<kind>.png`, using the file names below.
- Then run `python3 docs/design/render.py`. Each image fills its whole panel, scaled to cover it
  and centre-cropped.
- Every control group sits on a plain cream plate (a "clear zone") drawn over the art. The art
  needs no empty areas, and it must never contain its own text or controls.
- Keep the prompt and the tool/model name next to each file. Append them to the provenance table
  below.

## Shared style block (prepend to every module prompt)

> Flat illustrated artwork for the front panel of a boutique guitar-pedal-style synthesizer
> module. Portrait format, aspect ratio 21:34, 1260×2040 px. Screen-print look: 3–5 flat colours,
> subtle paper grain, soft risograph texture, gentle shapes, no hard photographic lighting. Fills
> the whole frame edge to edge. No text, no letters, no numbers, no logos, no knobs, no buttons,
> no jacks, no screws, no frames, no borders, no UI elements, no people. Mid-tone overall, so
> cream label plates placed on top stay the main contrast. Consistent style across a set of
> seven panels.

## Module prompts

| File | Width | Palette anchor | Subject (append after the style block) |
|---|---|---|---|
| `osc.va.png` | 7u (21:34) | coral `#e7775b`, cream | Rolling horizontal sine waves like ocean swell seen from above, layered bands, calm rhythm. |
| `lfo.png` | 7u (21:34) | deep indigo `#40356b`, pale gold | A night sky with a crescent moon high on the right and sparse small stars; slow, cyclical mood. |
| `filter.svf.png` | 7u (21:34) | steel blue `#3f80a8`, navy | Three layered mountain ridges receding into haze; the lower ridges darker, like a low-pass horizon. |
| `env.adsr.png` | 8u (24:34) | sand `#dcb98a`, ochre | Desert dunes whose silhouette rises steeply, falls, holds a long plateau, then slopes away (an ADSR shape, not literal). |
| `vca.png` | 7u (21:34) | sage `#7fa37a`, pale green | Soft sun rays fanning out from a point in the upper middle; quiet, warm. |
| `midi.in.png` | 5u (15:34) | mustard `#e5b547`, amber | Abstract diagonal piano-key stripes, slightly tilted, like light through blinds. |
| `out.png` | 5u (15:34) | slate `#4d5d6d`, blue-grey | Concentric ripples on dark water spreading from the centre, like sound leaving a speaker. |

For 5u and 8u panels, change the aspect ratio in the style block to the one in the table
(5u: 900×2040; 8u: 1440×2040).

## Acceptance check before use

- No text or glyph-like marks. Generated images often hide pseudo-letters in texture, so zoom in
  and look.
- No fake controls: nothing that reads as a knob, jack or switch.
- Re-render, then check that labels on the clear zones still read at 100%. Contrast is
  guaranteed by the plates, but a busy image can still distract.

## Provenance

| File | Tool / model | Date | Prompt (exact) | Notes |
|---|---|---|---|---|
| — | — | — | — | none generated yet |
