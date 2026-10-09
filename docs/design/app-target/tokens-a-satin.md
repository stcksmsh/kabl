# Token sheet — A · Satin Studio

## Light theme

### Colour

| token | value | use |
|---|---|---|
| `bg` | `#e7e2d6` | window background |
| `surface` | `#f3efe5` | toolbar, panels, status bar |
| `raised` | `#fbf9f3` | cards, popovers, active segment |
| `inset` | `#dcd5c5` | fields, wells, tracks |
| `line` | `#cbc3b0` | hairlines |
| `line2` | `#aaa28f` | strong rules, outlines |
| `text` | `#2a2621` | primary text |
| `text2` | `#5a5347` | secondary text |
| `text3` | `#8a8272` | hints, disabled (non-essential) |
| `accent` | `#b8501f` | primary action, selection |
| `on_accent` | `#fff8ef` | text on accent |
| `accent_soft` | `#f0d9c9` | selected row / chip fill |
| `focus` | `#2a6ae0` | keyboard focus ring |
| `audio` | `#e0962b` | audio jack and cable |
| `cv` | `#1f9d90` | control / modulation jack, cable, ring |
| `gate` | `#8250d0` | gate and trigger jack, cable |
| `good` | `#2f8a4a` | ok state |
| `warn` | `#b0661a` | warning |
| `bad` | `#b8322a` | error, clip |
| `rack` | `#d6cfbf` | rack background |
| `rail` | `#c3bcab` | rack rail |
| `hole` | `#2a2826` | jack hole, rail slot |
| `disp_bg` | `#27241f` | live display background |
| `disp_grid` | `#3d382f` | display graticule |
| `disp_trace` | `#f1b25a` | display trace |
| `disp_text` | `#efe6d2` | display text |
| `knob` | `#2c2a27` | knob body |
| `knob_hi` | `#5d5851` | knob rim |
| `knob_ink` | `#f4efe6` | knob pointer |

### Module face families

| section | base | ink | ink2 | ink/base contrast |
|---|---|---|---|---|
| oscillator / noise | `#d79a82` | `#2a1c14` | `#3e291d` | 6.9 |
| filter | `#aebf9f` | `#1f2a1b` | `#3d4c37` | 7.7 |
| envelope / LFO | `#bdb3d0` | `#231d30` | `#463d58` | 8.2 |
| VCA / mix / output | `#cabc92` | `#2c2410` | `#3f3620` | 8.1 |
| clock / sequencer / MIDI | `#a9bec1` | `#16262a` | `#35494d` | 8.0 |
| effects / other | `#d9cfbf` | `#2a2318` | `#4d4535` | 10.1 |

## Dark theme

### Colour

| token | value | use |
|---|---|---|
| `bg` | `#161412` | window background |
| `surface` | `#1f1c19` | toolbar, panels, status bar |
| `raised` | `#292520` | cards, popovers, active segment |
| `inset` | `#110f0d` | fields, wells, tracks |
| `line` | `#3a352e` | hairlines |
| `line2` | `#5b5448` | strong rules, outlines |
| `text` | `#efe8da` | primary text |
| `text2` | `#b9af9c` | secondary text |
| `text3` | `#7f7769` | hints, disabled (non-essential) |
| `accent` | `#f0a04b` | primary action, selection |
| `on_accent` | `#26180a` | text on accent |
| `accent_soft` | `#4a3620` | selected row / chip fill |
| `focus` | `#6aa4ff` | keyboard focus ring |
| `audio` | `#f0a640` | audio jack and cable |
| `cv` | `#35c2b1` | control / modulation jack, cable, ring |
| `gate` | `#a687ee` | gate and trigger jack, cable |
| `good` | `#6cc788` | ok state |
| `warn` | `#f1b75a` | warning |
| `bad` | `#ef7a6e` | error, clip |
| `rack` | `#0f0e0d` | rack background |
| `rail` | `#3f3b35` | rack rail |
| `hole` | `#050504` | jack hole, rail slot |
| `disp_bg` | `#0f0d0b` | live display background |
| `disp_grid` | `#2a2620` | display graticule |
| `disp_trace` | `#f4b862` | display trace |
| `disp_text` | `#f1e3c6` | display text |
| `knob` | `#201e1b` | knob body |
| `knob_hi` | `#6b665d` | knob rim |
| `knob_ink` | `#f4efe6` | knob pointer |

### Module face families

| section | base | ink | ink2 | ink/base contrast |
|---|---|---|---|---|
| oscillator / noise | `#85594a` | `#fff4e6` | `#f8ece0` | 5.5 |
| filter | `#53664d` | `#f4fbee` | `#dbe7d3` | 5.9 |
| envelope / LFO | `#62537a` | `#f6f1ff` | `#e0d8ee` | 6.2 |
| VCA / mix / output | `#6e5f40` | `#fff8e4` | `#ebe0c4` | 5.9 |
| clock / sequencer / MIDI | `#3f5a60` | `#effbfd` | `#d1e4e8` | 7.0 |
| effects / other | `#5d564c` | `#f8f1e6` | `#e0d8ca` | 6.5 |

## Type scale (IBM Plex, OFL-1.1)

| step | size | face | role |
|---|---|---|---|
| display | 28 px | Plex Sans SemiBold | Perform: tempo, bar counter |
| title | 20 px | Plex Sans SemiBold | Panel heading, patch name |
| h3 | 15 px | Plex Sans SemiBold | Card heading, module name |
| body | 13 px | Plex Sans Regular | Lists, inputs, help |
| label | 12 px | Plex Sans Medium | Control labels, tabs |
| value | 12 px | Plex Mono Medium | Numeric readouts |
| caption | 11 px | Plex Sans Regular | Secondary text; floor for any text |

No text below 11 px at any zoom; faces keep an 11 px floor when the rack is zoomed out.

## Spacing, radii, shadows, motion

Spacing scale (px): 2 · 4 · 8 · 12 · 16 · 24 · 32 · 48

Radii: small 4 · medium 8 · large 12 px

| shadow | offset y | blur | alpha (light) | alpha (dark) |
|---|---|---|---|---|
| sm (controls) | 1 | 3 | 28 | 90 |
| md (cards, faces) | 4 | 12 | 38 | 110 |
| lg (popovers) | 12 | 32 | 56 | 150 |

| motion | duration | easing |
|---|---|---|
| hover | 90 ms | linear |
| press | 60 ms | linear |
| drawer / popover | 220 ms | ease-out cubic |
| view switch | 180 ms | ease-out cubic |
| value glide (knob, display) | 120 ms | ease-out cubic |
| cue / beat pulse | 1200 ms | triangle |
| displays (scope, curves, rings) | every frame, display refresh | n/a |
| cable signal beads | 90 px/s | linear |
