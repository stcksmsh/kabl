# Token sheet — C · Silkscreen

## Light theme

### Colour

| token | value | use |
|---|---|---|
| `bg` | `#efeee9` | window background |
| `surface` | `#ffffff` | toolbar, panels, status bar |
| `raised` | `#ffffff` | cards, popovers, active segment |
| `inset` | `#e6e4de` | fields, wells, tracks |
| `line` | `#d2cfc7` | hairlines |
| `line2` | `#14130f` | strong rules, outlines |
| `text` | `#15130f` | primary text |
| `text2` | `#55514a` | secondary text |
| `text3` | `#8a867d` | hints, disabled (non-essential) |
| `accent` | `#d23a12` | primary action, selection |
| `on_accent` | `#ffffff` | text on accent |
| `accent_soft` | `#fbdcd1` | selected row / chip fill |
| `focus` | `#1d4ed8` | keyboard focus ring |
| `audio` | `#e89a00` | audio jack and cable |
| `cv` | `#00968c` | control / modulation jack, cable, ring |
| `gate` | `#6c3fe6` | gate and trigger jack, cable |
| `good` | `#1b8a3d` | ok state |
| `warn` | `#a65f00` | warning |
| `bad` | `#c4271c` | error, clip |
| `rack` | `#d9d7d0` | rack background |
| `rail` | `#bdbab1` | rack rail |
| `hole` | `#1a1915` | jack hole, rail slot |
| `disp_bg` | `#fbfaf6` | live display background |
| `disp_grid` | `#dedbd2` | display graticule |
| `disp_trace` | `#14130f` | display trace |
| `disp_text` | `#14130f` | display text |
| `knob` | `#17150f` | knob body |
| `knob_hi` | `#17150f` | knob rim |
| `knob_ink` | `#ffffff` | knob pointer |

### Module face families

| section | base | ink | ink2 | ink/base contrast |
|---|---|---|---|---|
| oscillator / noise | `#f2a48b` | `#14110d` | `#3a352d` | 9.4 |
| filter | `#b5d3a8` | `#14110d` | `#3a352d` | 11.5 |
| envelope / LFO | `#cbbbee` | `#14110d` | `#3a352d` | 10.6 |
| VCA / mix / output | `#f1d57c` | `#14110d` | `#3a352d` | 13.0 |
| clock / sequencer / MIDI | `#a9d0e1` | `#14110d` | `#3a352d` | 11.5 |
| effects / other | `#dedacd` | `#14110d` | `#3a352d` | 13.5 |

## Dark theme

### Colour

| token | value | use |
|---|---|---|
| `bg` | `#111110` | window background |
| `surface` | `#1a1a18` | toolbar, panels, status bar |
| `raised` | `#222220` | cards, popovers, active segment |
| `inset` | `#0b0b0a` | fields, wells, tracks |
| `line` | `#34332f` | hairlines |
| `line2` | `#efece4` | strong rules, outlines |
| `text` | `#f3f1ea` | primary text |
| `text2` | `#b0aca2` | secondary text |
| `text3` | `#78756c` | hints, disabled (non-essential) |
| `accent` | `#ff6b3d` | primary action, selection |
| `on_accent` | `#160803` | text on accent |
| `accent_soft` | `#4a2214` | selected row / chip fill |
| `focus` | `#7ea2ff` | keyboard focus ring |
| `audio` | `#ffb21a` | audio jack and cable |
| `cv` | `#22d3c5` | control / modulation jack, cable, ring |
| `gate` | `#a98bff` | gate and trigger jack, cable |
| `good` | `#5fd388` | ok state |
| `warn` | `#ffc04d` | warning |
| `bad` | `#ff7468` | error, clip |
| `rack` | `#080808` | rack background |
| `rail` | `#2f2e2a` | rack rail |
| `hole` | `#000000` | jack hole, rail slot |
| `disp_bg` | `#0a0a09` | live display background |
| `disp_grid` | `#262622` | display graticule |
| `disp_trace` | `#f3f1ea` | display trace |
| `disp_text` | `#f3f1ea` | display text |
| `knob` | `#0e0e0c` | knob body |
| `knob_hi` | `#f3f1ea` | knob rim |
| `knob_ink` | `#f3f1ea` | knob pointer |

### Module face families

| section | base | ink | ink2 | ink/base contrast |
|---|---|---|---|---|
| oscillator / noise | `#e0714f` | `#0e0c09` | `#2b2218` | 6.2 |
| filter | `#78b27b` | `#0b0f0b` | `#1f2c20` | 7.8 |
| envelope / LFO | `#9a82e0` | `#0c0a14` | `#271f40` | 6.2 |
| VCA / mix / output | `#d5ac3a` | `#100c02` | `#32280b` | 9.1 |
| clock / sequencer / MIDI | `#4a9fc2` | `#06101a` | `#12303f` | 6.4 |
| effects / other | `#9a948a` | `#0e0d0a` | `#2b2822` | 6.5 |

## Type scale (IBM Plex, OFL-1.1)

| step | size | face | role |
|---|---|---|---|
| display | 32 px | Plex Sans Condensed SemiBold | Perform: tempo, bar counter |
| title | 20 px | Plex Sans Condensed SemiBold | Panel heading, patch name (caps) |
| h3 | 15 px | Plex Sans Condensed SemiBold | Module name (caps) |
| body | 13 px | Plex Sans Regular | Lists, inputs, help |
| label | 12 px | Plex Sans Condensed SemiBold | Control labels (caps, +0.5 px tracking) |
| value | 12 px | Plex Mono Medium | Numeric readouts |
| caption | 11 px | Plex Sans Condensed Medium | Secondary text; floor for any text |

No text below 11 px at any zoom; faces keep an 11 px floor when the rack is zoomed out.

## Spacing, radii, shadows, motion

Spacing scale (px): 2 · 4 · 8 · 12 · 16 · 24 · 32 · 48

Radii: small 2 · medium 4 · large 6 px

| shadow | offset y | blur | alpha (light) | alpha (dark) |
|---|---|---|---|---|
| sm (controls) | 0 | 0 | 0 | 0 |
| md (cards, faces) | 2 | 6 | 22 | 100 |
| lg (popovers) | 8 | 20 | 40 | 160 |

| motion | duration | easing |
|---|---|---|
| hover | 60 ms | linear |
| press | 40 ms | linear |
| drawer / popover | 160 ms | ease-out cubic |
| view switch | 120 ms | ease-out cubic |
| value glide (knob, display) | 90 ms | ease-out cubic |
| cue / beat pulse | 800 ms | triangle |
| displays (scope, curves, rings) | every frame, display refresh | n/a |
| cable signal beads | 90 px/s | linear |
