# kabl whole-application visual target

Design exploration for Kosta. Nothing under `crates/ui/src` or any other crate changed. The only code is one example, `crates/ui/examples/app_target/`, which draws every frame with egui's own painter (`egui::Painter`, meshes, text, shapes), on top of the real `kabl_ui::rack::layout` geometry and the real patches in `patches/reference` and `patches/composition`. Every capture is a framebuffer screenshot from egui (`ViewportCommand::Screenshot`), not a mockup.

Decided by Kosta and kept in all three directions: the Eurorack rack stays; the current module faces are the base; everything else must reach Vital / Pigments polish. The owner's recorded preference (continuous coloured satin faces, recessed hardware) is honoured in A and B and reinterpreted in C.

## The three directions

| | **A · Satin Studio** | **B · Lacquer & Light** | **C · Silkscreen** |
|---|---|---|---|
| One line | The current faces, finished: satin, engraved ink, recessed jacks, warm paper and graphite chrome | Dark-first, display-led: glossy lacquer faces, luminous rings and traces, pill controls | Flat matte colour with bold print: ink outlines, condensed caps, hairline chrome |
| Faces | Satin gradient, grain, nuts, soft shadow | Lacquer gradient plus a gloss band, no nuts, glow on selection | Flat fill, 2 px ink outline, printed rule under the title |
| Knob | Charcoal cap with skirt grip, tick scale, thin modulation arc | Glass cap, glowing value arc, glowing modulation arc and dot | Black disc, bold pointer, printed ticks, thick modulation segment |
| Displays | Warm dark glass, amber trace, light glow | Near-black glass, cyan trace, strong glow | Paper (light) or ink (dark) with black trace and accent playhead |
| Type | Plex Sans SemiBold headings, Medium labels | Plex Sans SemiBold caps labels, Plex Mono numbers | Plex Sans Condensed SemiBold caps, Plex Mono numbers |
| Chrome | Rounded cards, soft shadows, one toolbar row | Pill controls and tabs, preset bar, glow focus | Square controls, 1.5 px ink rules, densest layout |
| Light / dark | Warm paper / warm graphite | Cool grey day / blue-black night | White-and-ink / ink-and-bone |
| Closest to the current app | Closest | Medium | Furthest |

## Where to look

Each direction folder under [`media/`](media/) holds the same set at `1440x900` and `1280x800`, light and dark (seven scenes x 2 sizes x 2 themes = 28 WebP images per direction), plus `motion.mp4` (about 19 s at 30 fps, 1280x800).

| Scene file | What it shows |
|---|---|
| `rack-sounds` | Main rack with the Sounds browser open, cable mode **Focus**, audition card, collapsed inspector rail, new status bar |
| `perform` | Transport, scenes, macros, sequence lanes, small controls |
| `composition` | The 41-module, 76-cable Composition patch at fit zoom: outline drawer, overview cards, focus cables, numbered routing inspector |
| `faces` | Close-up of five faces with live displays (scope, filter response, LFO, envelope curve, VCA transfer) and the knob modulation anatomy |
| `wavetable` | Concept: wavetable oscillator face (stacked frames, table chip, single-cycle inset, Position ring moved by an LFO) in a rack row, with a frame strip |
| `step-cable` | Concept: a cable that carries a 16-step pattern and a probability, with pulses thinned live, plus the cable editor |
| `components` | Replacements for stock widgets: buttons, chips, fields, lists, tooltip, menu, dialog, dropdown, toast, empty state, status bar, diagnostics popover |

Recordings (`media/<direction>/motion.mp4`): rack with moving scope, filter, envelope, LFO and cable beads; Perform with a cue queued then launched and step lanes running; Composition with focus moving between modules; face close-up; wavetable; step cable. A uses the light theme, B and C the dark theme.

Theming design note (proposal, decisions for Kosta): [`THEMING.md`](THEMING.md).

Supporting documents: [`tokens-a-satin.md`](tokens-a-satin.md), [`tokens-b-lacquer.md`](tokens-b-lacquer.md), [`tokens-c-silkscreen.md`](tokens-c-silkscreen.md) (colours, face families, type scale, spacing, radii, shadows, motion; generated from the same tables the renderer uses), [`stock-widget-inventory.md`](stock-widget-inventory.md), [`contrast-check.txt`](contrast-check.txt).

## How the six known defects are answered (all directions)

1. **Stock widgets**: a custom kit replaces them; see the inventory.
2. **Nothing moves**: displays are pure functions of time. Scope with persistence, envelope curve with a playhead that follows the gate, filter response with the input spectrum and swept range, LFO phase dot, VCA transfer dot, modulation ring with a live dot on the knob, beads travelling on cables, running step lanes, cue pads that pulse while queued.
3. **Perform**: cards with hierarchy. Scenes are large pads (playing, queued, ready), macros are large knobs with the destinations they move and a small CC badge, banks are pill rows, sequence lanes show level and probability.
4. **Dense patches**: semantic zoom. Below about 0.55 zoom, faces become overview cards (short name, one mini display, only connected jacks). Cables not touching the selected module are dimmed to 1.3 px lines; its own cables are lit, animated and numbered, with the same numbers in the inspector list. An outline drawer lists the rows. Cable positions follow jack IDs, module positions never move.
5. **Typography**: seven-step scale with weights, 11 px floor at every zoom (faces clamp to 11 px when the rack is zoomed out).
6. **Telemetry**: the status bar keeps meter, audio and MIDI state and voices. Callback timing, xruns, RT status and session move to a Diagnostics popover.

One structural change common to all three: only one side panel is open at a time at 1280 px, and the Routing inspector collapses to a 44 px rail. That is what lets the rack sit at about 100 % zoom (roughly 90 % to 100 % in the captures) instead of 79 %, so face text stays at its designed size.

Jack positions on the five modules that gain a display (osc, filter, envelope, LFO, VCA) and MIDI In move down by 10 to 50 points inside the same 340 pt face. Port identities and cables are unchanged. The wavetable face (12u) and the cable cartridge are concepts: neither module nor cable data exists in the engine yet.

## Cost to build (rough, one engineer who knows the UI crate)

Shared by all directions, build once: font weights (Medium, SemiBold, Condensed from the already-embedded Plex family, same OFL licence); token struct and theme switch; widget kit (button, chip, segmented, field, slider, toggle, list row, dropdown, dialog, tooltip, toast); toolbar, drawer, status bar, diagnostics popover; display components (scope, envelope, filter, LFO, transfer, step lane); Perform rebuild; overview cards and focus-cable mode. **About 6 to 8 weeks** for the shared part.

| | Extra over shared | Risk |
|---|---|---|
| **A** | About 1 week: satin pass on faces, knob and jack polish, shadows | Lowest. Reuses today's `theme::satin` and geometry |
| **B** | About 3 weeks: gradient and glow primitives, glass knob, glowing arcs, preset bar, per-frame glow cost | Glow means extra strokes per frame; measure on the laptop. Furthest from the recorded satin preference |
| **C** | About 2 weeks: second type family usage, flat outlines, dense layouts | Loses the satin preference; strongest legibility and cheapest to draw |

Dependencies outside the UI: scope and playhead need the bounded selected-signal telemetry already planned (D03); the envelope curve, filter response, LFO and transfer curves need only parameters; wavetable needs the new oscillator; the cable cartridge needs functional cables (`CableState.steps` and params exist in core already).

## Recommendation

**Build A, borrow B's display language.**

- A is the only direction that continues what Kosta already likes and recorded as preferred, and it is the cheapest to ship without leaving a half-migrated app.
- Its light and dark themes pass the contrast checks and it reads clearly at 1280 px.
- B reaches the Vital/Pigments feel most strongly, mostly through its displays and rings. Those do not depend on B's lacquer faces: the luminous display and ring treatment can be added to A's dark theme later as a variant, without changing layout or widgets.
- C is the best at density and legibility, but flat print gives up the satin faces Kosta chose. Keep it as the fallback if A's warm palette feels too soft at small sizes.

If polish matters more than continuity, choose B and accept the glow cost.

## Verification

Done:
- All 84 stills (stored as quality-92 WebP re-encodes of the framebuffer PNGs, to keep the repository small) and three recordings were rendered by the example under Xvfb (software GL) at both sizes and themes. I reviewed full-size captures of every scene in at least one direction and theme, and contact sheets of the rest at both sizes, and fixed the defects I saw: overlapping labels, cut-off segmented labels, cable text collisions, hidden knob pointer in C dark, colliding diagnostics popover.
- `app_target check` computes WCAG contrast for text, face ink, displays and accent pairs in every direction and theme: 0 failures (text 4.5, hints and non-text 3.0). See `contrast-check.txt`. Cables are not text; they carry a dark edge (A, C) or glow (B), so they are not contrast-checked against the rack.
- No text below 11 px in the renderer (`Xf::fs` clamps).

Not verified:
- No hands-on use: nothing is interactive. Hover, press, drag and focus states are drawn as specimens only.
- Frame cost of the live displays and glow in the real app is not measured (software GL here, no laptop). B is the likeliest to need tuning.
- The HiDPI and 200 % scaling cases were not rendered; everything is 1x.
- Contrast of text over animated displays is only checked at the token level.
- Added Plex weights come from the system fonts directory (same family, OFL-1.1); `crates/ui/examples/app_target/fonts/` carries copies for the example. Embedding them in the app needs the licence file extended.
- The wavetable and cable concepts use synthetic data; parameter names such as `pos_cv` are placeholders.

## Reproduce

```
cargo build --release -p kabl-ui --example app_target
xvfb-run -a target/release/examples/app_target stills --scene all --size 1440x900 --out OUT
xvfb-run -a target/release/examples/app_target video --dir a --theme light --size 1280x800 --out FRAMES   # then ffmpeg -framerate 30 -i FRAMES/a-satin/f%04d.png
target/release/examples/app_target tokens     # token tables
target/release/examples/app_target check      # contrast report
```

## Decisions for Kosta

1. Pick a direction (recommended: A, optionally with B's displays in dark).
2. Accept one open side panel at a time at 1280 px, with Routing collapsing to a rail.
3. Accept moving jacks down inside faces that gain a display (identities unchanged).
4. Whether to extend the font licence file and embed the extra Plex weights.
