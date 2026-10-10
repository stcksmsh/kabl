# Token sheet — B · Lacquer & Light

## Light theme

### Colour

| token | value | use |
|---|---|---|
| `bg` | `#e6e9f0` | window background |
| `surface` | `#f4f6fa` | toolbar, panels, status bar |
| `raised` | `#ffffff` | cards, popovers, active segment |
| `inset` | `#dce1ea` | fields, wells, tracks |
| `line` | `#c8cfdb` | hairlines |
| `line2` | `#9aa3b5` | strong rules, outlines |
| `text` | `#161922` | primary text |
| `text2` | `#464e60` | secondary text |
| `text3` | `#7b8396` | hints, disabled (non-essential) |
| `accent` | `#3b4fe0` | primary action, selection |
| `on_accent` | `#ffffff` | text on accent |
| `accent_soft` | `#dbe0fb` | selected row / chip fill |
| `focus` | `#3b4fe0` | keyboard focus ring |
| `audio` | `#e08a14` | audio jack and cable |
| `cv` | `#0f9d8a` | control / modulation jack, cable, ring |
| `gate` | `#7b4fd8` | gate and trigger jack, cable |
| `good` | `#1f8a4c` | ok state |
| `warn` | `#a8620f` | warning |
| `bad` | `#c23030` | error, clip |
| `rack` | `#cfd4df` | rack background |
| `rail` | `#b4bbcb` | rack rail |
| `hole` | `#2c303a` | jack hole, rail slot |
| `disp_bg` | `#12141a` | live display background |
| `disp_grid` | `#262a35` | display graticule |
| `disp_trace` | `#5ff0d6` | display trace |
| `disp_text` | `#d6f3f3` | display text |
| `knob` | `#1c1f27` | knob body |
| `knob_hi` | `#5b6274` | knob rim |
| `knob_ink` | `#f2f4fa` | knob pointer |

### Module face families

| section | base | ink | ink2 | ink/base contrast |
|---|---|---|---|---|
| oscillator / noise | `#f0a791` | `#2a110c` | `#4d2a22` | 9.0 |
| filter | `#aad3b8` | `#0f2619` | `#2c4a3a` | 9.7 |
| envelope / LFO | `#c6baf0` | `#1a1233` | `#3d3262` | 9.9 |
| VCA / mix / output | `#ebd28e` | `#2b2108` | `#524416` | 10.7 |
| clock / sequencer / MIDI | `#a1d0e2` | `#0c2530` | `#26495a` | 9.6 |
| effects / other | `#d9d2e4` | `#1e1829` | `#413a54` | 11.7 |

## Dark theme

### Colour

| token | value | use |
|---|---|---|
| `bg` | `#0d0e12` | window background |
| `surface` | `#15171d` | toolbar, panels, status bar |
| `raised` | `#1e2129` | cards, popovers, active segment |
| `inset` | `#090a0d` | fields, wells, tracks |
| `line` | `#292d38` | hairlines |
| `line2` | `#434958` | strong rules, outlines |
| `text` | `#eceef4` | primary text |
| `text2` | `#a5abbb` | secondary text |
| `text3` | `#6c7388` | hints, disabled (non-essential) |
| `accent` | `#7f8bff` | primary action, selection |
| `on_accent` | `#0b0d1f` | text on accent |
| `accent_soft` | `#24285a` | selected row / chip fill |
| `focus` | `#9aa4ff` | keyboard focus ring |
| `audio` | `#ffb454` | audio jack and cable |
| `cv` | `#3de0c8` | control / modulation jack, cable, ring |
| `gate` | `#b690ff` | gate and trigger jack, cable |
| `good` | `#5fe39a` | ok state |
| `warn` | `#ffc666` | warning |
| `bad` | `#ff7a7a` | error, clip |
| `rack` | `#08090c` | rack background |
| `rail` | `#2a2d36` | rack rail |
| `hole` | `#030305` | jack hole, rail slot |
| `disp_bg` | `#07080b` | live display background |
| `disp_grid` | `#1d212a` | display graticule |
| `disp_trace` | `#5ff0d6` | display trace |
| `disp_text` | `#cfeff0` | display text |
| `knob` | `#12141a` | knob body |
| `knob_hi` | `#4a5060` | knob rim |
| `knob_ink` | `#f2f4fa` | knob pointer |

### Module face families

| section | base | ink | ink2 | ink/base contrast |
|---|---|---|---|---|
| oscillator / noise | `#8e4338` | `#fff6ef` | `#f1ddd2` | 6.5 |
| filter | `#35705a` | `#f2fff8` | `#d3eadf` | 5.7 |
| envelope / LFO | `#5b4ca3` | `#f6f3ff` | `#ddd6f4` | 6.4 |
| VCA / mix / output | `#82672b` | `#fff9e8` | `#fcf3dc` | 5.1 |
| clock / sequencer / MIDI | `#2b6377` | `#effcff` | `#cde7ef` | 6.4 |
| effects / other | `#5d566b` | `#f7f4fb` | `#ded8e8` | 6.4 |

## Type scale (IBM Plex, OFL-1.1)

| step | size | face | role |
|---|---|---|---|
| display | 30 px | Plex Mono Medium | Perform: tempo, bar counter |
| title | 20 px | Plex Sans SemiBold | Preset name, panel heading |
| h3 | 15 px | Plex Sans SemiBold | Card heading, module name |
| body | 13 px | Plex Sans Regular | Lists, inputs, help |
| label | 11.5 px | Plex Sans SemiBold | Control labels (caps, +0.6 px tracking) |
| value | 12 px | Plex Mono Medium | Numeric readouts |
| caption | 11 px | Plex Sans Regular | Secondary text; floor for any text |

No text below 11 px at any zoom; faces keep an 11 px floor when the rack is zoomed out.

## Spacing, radii, shadows, motion

Spacing scale (px): 2 · 4 · 8 · 12 · 16 · 24 · 32 · 48

Radii: small 5 · medium 10 · large 16 px

| shadow | offset y | blur | alpha (light) | alpha (dark) |
|---|---|---|---|---|
| sm (controls) | 1 | 3 | 26 | 120 |
| md (cards, faces) | 6 | 16 | 36 | 150 |
| lg (popovers) | 16 | 36 | 56 | 190 |

| motion | duration | easing |
|---|---|---|
| hover | 120 ms | linear |
| press | 80 ms | linear |
| drawer / popover | 260 ms | ease-out cubic |
| view switch | 220 ms | ease-out cubic |
| value glide (knob, display) | 160 ms | ease-out cubic |
| cue / beat pulse | 900 ms | triangle |
| displays (scope, curves, rings) | every frame, display refresh | n/a |
| cable signal beads | 90 px/s | linear |
