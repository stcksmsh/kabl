# kabl UI redesign: design brief for the design agent

You have no repository access. This file plus `images/` is everything you get. Facts below were read from the
kabl source at commit `6e92737` (origin/master, 2026-10-31). Where something is a guess, a relayed owner
statement, or unverified, it is marked **[guess]**, **[relayed]** or **[unverified]**. File paths are given so a later
coding agent can map your designs to code; you do not need to open them.

The owner is called Kosta. He prefers terse communication, clarity, and a UI that looks clean at every scale:
nothing overlapping, nothing truncated, nothing glued on.

---

## 0. What we want from you (one paragraph)

Plan a NEW visual design for kabl's whole UI (module faces first, then the screens around them), and plan the
workflow that builds it and gets it reviewed and signed off by the owner. Target mockups come first. Code comes
after the owner has approved the mockups. Section 5 lists the deliverables exactly.

---

## 1. What kabl is

kabl is a legible modular synthesizer written in Rust (GPL-3.0-or-later source; name and logo reserved). The pitch in the
README: "Cables are instruments, patches explain themselves, and the sound you hear can be traced back through the
patch." It is a software patch instrument in the Eurorack idiom: a rack of module faces, each with knobs,
displays, selectors and jacks, wired by cables.

It ships in two forms from the same UI code:

- **Standalone app** (`kabl-ui`): sound browser, rack, Perform view, routing inspector, editors. Linux is the supported
  platform for 1.0.
- **CLAP instrument plugin** (`kabl.clap`), tested in REAPER and with the CLAP validator. The plugin editor is the same
  egui renderer; its default window is **1180 x 680** logical pixels (`crates/clap/src/lib.rs:889`).

Content and features (README, `docs/PRODUCT-PLAN.md`, `docs/presets/BRIEF.md`):

- **53 module kinds** in `KNOWN_KINDS` (`crates/modules/src/registry.rs`; older docs say 21, 24 or 51, they are stale): analog-style
  (VA), FM (2-operator and 6-operator) and wavetable oscillators; state-variable, ladder, comb and formant filters; ADSR,
  LFO, noise; clock, clock divider, step sequencer, arpeggiator, drum grid, five drum voices; VCA, gain, mixer, ring
  modulator, pan, crossfade, attenuverter, comparator, logic, quantizer, random, sample and hold, slew; drive, chorus,
  phaser, flanger, delay, reverb, compressor, EQ, limiter, bitcrusher, vocoder; macros, cues, MIDI in, MIDI note, audio in,
  output; stereo split/join. Composite modules can be built from these and saved with their own face.
- **Functional cables**: a cable can carry a step pattern, a pass probability, an A/B morph and glide, locked to the patch
  clock. Macros can drive cable parameters.
- **Presets ("Sounds")**: a factory bank in `patches/` (sounds, sequences, performances), searchable in a browser with
  categories (Bass, Lead, Keys, Pad, Pluck, Groove, Piece, Voice). User sounds are saved separately.
- **Macros**: up to 8 performance knobs per patch (4 is the recommended number), each named for what it does for that
  sound (`Bite`, `Wash`). MIDI CC 20+ maps to macros in order. A macro moves many destinations through modulation routes.
- **Cues / scenes**: a `cues` module holds up to 8 named section cues; a cue picks a bank (A to D) for several
  sequencers and launches them together on the next step or bar. Cue pads are the main Perform controls.
- **Sequencer**: 8 steps, 4 banks A to D, per step pitch, gate on/off, velocity, probability, ratchet; length, gate length,
  gate mode. (On master: 8 steps. In flight, not yet merged: the seq-32 branch raises this to 32 steps per pattern, length 1 to 32, shown on the face as pages of 8 steps. Design for 32 steps in pages of 8, plus banks.)
- **Drum grid**: 8 rows x up to 16 steps, per-row length (polymeter), 4 patterns A to D, 4 velocity levels and 4
  chance levels (100/75/50/25 %) per step, one gate output per row.
- **Audio path** is real-time safe (no allocation or locking in the callback); the UI never blocks it.
  Engine changes go through a command queue.

Target users and contexts (`docs/PRODUCT-PLAN.md`): a musical instrument Kosta uses himself, and which other musicians
can use without his help. First external audience: musicians new to modular synthesis (not people new to music).
Two use contexts, one engine:

1. **Performance / playing**: Perform view, cue pads, macro knobs, tempo and bar readout, MIDI controller, often read
   from a distance; also inside a DAW (REAPER) as an instrument where the window is small (1180 x 680) and the user
   mostly picks a sound and tweaks macros.
2. **Patch editing**: rack with dozens of modules and cables, routing inspector, explain panel ("why does this sound
   like this"), sound browser, editors (wavetable, cable pattern, drum grid). Dense, precise, mouse driven.

Product principles that constrain design (PRODUCT-PLAN section 1, recommended and not all owner-approved): a working sound
before a lesson; musical controls reveal their real targets; complexity can be opened and inspected; **the existing rack
character stays** (Eurorack-style faces, cables, rails; improve around it rather than replace it with an unrelated interface);
small coherent instrument over feature-count parity.

---

## 2. UI technology and architecture

### 2.1 Stack

- Rust, **egui 0.35** and **eframe 0.35** (`crates/ui/Cargo.toml`), GL/glow-style window in standalone;
  plugin editor via a vendored `nice-plug-egui` (`vendor/nice-plug`).
- Crates: `core` (patch state, op log, file format), `engine` (graph compiler, scheduler, voices), `modules` (DSP + module
  metadata), `cables`, `ui` (everything below), `standalone` (audio/MIDI I/O, window), `clap` (plugin).
- Immediate-mode: the UI is redrawn every frame from state. There is no retained widget tree, no CSS, no layout engine
  beyond egui's `Ui` cursor and manual `Rect` arithmetic.

### 2.2 `crates/ui/src` modules (line counts approximate; ~34k lines total)

| Module | Role |
|---|---|
| `lib.rs` (4.9k), `main.rs` (3k) | Top-level frame: toolbar, side rail, status bar, view switch (Rack / Perform), drawers, zoom/pan, theme switching. `main.rs` wires audio, MIDI, window. |
| `rack.rs` (1.3k) | **Pure rack geometry**: module face sizes, control and jack positions, row layout. Headless-testable. `rack_editor.rs` (1.3k) draws and edits faces in the canvas. |
| `kit/` (`face.rs`, `controls.rs`, `containers.rs`, `recipes.rs`, `icons.rs`, `mod.rs`) | The **widget kit**: face drawing primitives (surface, title, knob, jack, readout, display frame, trace, playhead) and chrome widgets (buttons, segmented controls, dropdown, menu, modal, card, toggles, pads, step lanes, bar meter, icons). Everything reads colours, type, radii, shadows from a `Style`. |
| `style/` (`mod.rs`, `builtin.rs`, `load.rs`) + `theme.rs` | The theme model (tokens + recipe slots), the two built-in themes, loader for user theme files. `theme.rs` is the older palette struct. |
| `perform.rs`, `perform_view.rs` (1.4k) | Perform panel: pinned controls as cards (transport, scenes, macros, sequences, other controls); MIDI CC mapping with soft takeover. |
| `panels.rs`, `composites.rs` | Module face skins (embedded light/dark PNG art, chosen control and jack positions and size) and composite (user-built) modules. |
| `displays.rs`, `fx_displays.rs`, `drum_displays.rs` | Live displays on faces: oscillator waveform, filter response, envelope curve with playhead, LFO, compressor GR, EQ curve, vocoder bands, drum envelopes. |
| `drum_grid.rs`, `pattern.rs`, `cues.rs`, `banks.rs` | Drum grid editor on the face; cable step-pattern editor; cue pads; bank selectors. |
| `routing.rs` (1.8k), `inspect.rs` (1.3k), `explain.rs` (1.1k), `help.rs` | Routing drawer/inspector (cable list, modulation routes), signal inspection, "explain this patch" panel, help. |
| `browser.rs` (2k), `library.rs`, `filebrowser.rs`, `recipes.rs` | Sounds browser (search, categories, audition), library, file dialogs, "recipes". |
| `tables.rs`, `wheel.rs` | Wavetable editor; scroll-wheel ownership logic. |
| `cable_look.rs` | How cables are drawn (rope/glow/outlined; lit vs resting; beads). |
| `about.rs`, `compare.rs`, `record.rs`, `control.rs`, `editor.rs` | About box, A/B compare, recording, controls, headless patch editing. |

### 2.3 Main screens and what they contain

(See images 01 to 10, 21 to 27.)

1. **Rack view (the main screen).** Top toolbar: logo, Sounds, module-type picker + Add, Undo/Redo, cable view
   (All / Focus / Hidden), zoom (-, %, +, Fit, Focus), theme switch, View menu, file name + Save / Save As, view switch
   (Perform / Routing). Below it, the canvas: horizontal **rack rows** with rails; module faces placed on a grid
   (`UNIT = 30` px; row pitch 370 px; face height `PANEL_H = 340` px; typical widths 210, 240, 300, 660 px).
   Cables are drawn over everything. A **routing drawer** (right side) shows cables, modulation routes, the inspector
   and the explain panel; it can be collapsed to a narrow rail. A bottom **status bar** shows meter, audio/MIDI state,
   voices; callback timing and xruns live in a Diagnostics popover.
2. **Perform view.** Cards: transport (run/stop, tempo, bar/beat readout), scenes (large cue pads: playing, queued with
   progress, ready), macros (arc knobs with the destinations they move and a small CC badge), banks (pill rows plus step
   lanes per sequencer, one hue per lane), other pinned controls. Fits 1440 x 900, 1280 x 800 and plugin 1180 x 680.
   Widths come from available room (`split` in `perform_view.rs`), heights from window height. Perform is considered the
   most finished surface today.
3. **Sounds browser.** Search, category chips, list of factory/user sounds, audition card, save.
4. **Editors**: wavetable editor (stack of frames + single wave view, toolbar of Insert/Duplicate/Move...), cable
   pattern editor (step bars, probability), drum grid (on the face itself), routing inspector (numbered cable list,
   amounts, sources).
5. **Dialogs and menus**: Save As, Diagnostics, View/settings menu, About, file browser, context menus.

### 2.4 Anatomy of a module face

A face is one rectangle, 340 px tall, 210/240/300/660 px wide (others exist), drawn by `kit/face.rs` and laid out by
`rack.rs`. From top to bottom today:

- **Header**: title (module name) and a small caption `kind . #id` (e.g. `osc.va . #10`); a `+N` chip at the right
  = number of additional controls not on the face ("more controls"); rarely a mode selector.
- **Display** (optional): a framed dark glass area with a live trace (waveform, response curve, envelope, meter).
- **Knob zone**: knobs (about 34 px diameter today, one size everywhere) with a label above and a value readout below;
  selectors (segmented pills, stock combo boxes), toggles.
- **Port area** at the bottom: input jacks (row at y = 248 px from the face top) and output jacks (row at y = 296),
  each with a label; a divider rule and a darker plate behind outputs on some faces.
- Corner "nuts" (screws) on satin; `Edit face` text printed at the foot (stage 1 made it hover-only).

Jacks are typed by signal (colour roles: audio, CV, gate, pitch; unipolar CV): `audio`, `cv`, `gate` colours come from
the theme. A jack can be 1-channel or **2-channel (stereo)**; stereo ports already exist in module metadata (e.g. the VA
oscillator has `out` and a width-2 `stereo` output). Knobs show a modulation range arc and a live dot when a route
modulates them. Faces are coloured by section family (oscillator, filter, envelope/LFO, amp/mix/out, timing, effects).

**Text hiding rule** (`kit/face.rs`): the type scale has an **11 px floor**; face text whose zoomed size would be below
**9.5 px is not drawn at all** (`HIDE_BELOW`). Titles stay up to 6 px. This is why labels vanish at low zoom (see
image 12).

### 2.5 Themes, tokens and slots

Source: `docs/design/app-target/THEMING.md`, `crates/ui/src/style/`. A `Style` is plain data: **tokens** (colours, type scale,
spacing, radii, shadows, motion) plus **slots** (a choice among drawing recipes shipped in code, with dials).

- Built-in themes: **`kabl.satin` "Satin Studio"** (theme A: soft satin faces, charcoal-cap knobs with tick scale, amber display
  trace, warm paper / graphite chrome) and **`kabl.lacquer` "Lacquer and Light"** (theme B: lacquer faces, glass knobs with
  glowing value arc, cyan display trace, cool grey / blue-black chrome). Each in light and dark: four looks
  ("A-light", "A-dark", "B-light", "B-dark" in the toolbar). A third explored direction, "Silkscreen" (flat, ink outlines),
  exists only as concept renders, not in the app.
- Tokens: colour roles (`bg surface raised inset line line2 text text2 text3 accent on_accent accent_soft focus audio cv gate
  good warn bad rack rail hole`), display roles (`disp_bg disp_grid disp_trace disp_text`), knob roles, per-section face palettes
  (`base ink ink2`), a 7-role type scale (display, title, h3, body, label, value, caption; sizes 11 to 40 px, **11 px floor
  not themable**), 8-step spacing scale, 3 radii (0 to 24), 3 shadow levels, motion durations. Spacing scale used in chrome:
  `[2,4,8,12,16,24,32,48]` (card margin 12, gap 8).
- Slots: `face.material` (satin / lacquer / flat), `face.hardware` (nuts/none/screws), `face.title`, `face.out_plate`,
  `rack.surface`, `knob.body` (cap/glass/disc), `knob.scale`, `knob.value`, `knob.mod`, `selector` (inset-pill/accent-pill/outlined),
  `jack` (nut/glow-ring/donut), `cable` (rope/glow/outlined), `plug`, `display.frame` (glass/neon/paper), `display.trace`,
  button/chip/field/slider/toggle/card shapes, `chrome.view_switch`, `chrome.navigator`, `browser.rows`, `icons`.
  Several slots are declared but no drawing code reads them yet (`plug`, `face.out_plate`, `chrome.navigator`, `controls.row`,
  rack surface, icons).
- Users can load theme files from a themes dir (`style/load.rs`), with per-value fallback and clamps; contrast of text pairs is
  asserted in tests. **A new design should be expressible as tokens + slots wherever possible**, adding new recipes only where needed.

### 2.6 Fonts

IBM Plex Sans (Regular, Medium, SemiBold) and IBM Plex Mono (Regular, Medium) are embedded (`crates/ui/assets/`), SIL OFL 1.1.
**Correction to the earlier note that Ubuntu-Light replacement is pending:** in this checkout egui's `default_fonts` feature is
**off**, so Ubuntu-Light, Hack and Noto Emoji are no longer shipped; every glyph the interface draws comes from IBM Plex, and a
test fails if a character used in source has no Plex glyph (Latin, Greek, Cyrillic). Consequence: **any icon or symbol must be
drawn as a vector shape (kit icon set, `kit/icons.rs`) or exist in Plex; no emoji, no arbitrary Unicode symbols.** A different
open-licensed font is possible but must be OFL-compatible and embedded. Rule used today: mono for numbers, ids, states; sans for names.

### 2.7 Scaling and sizes

- Rack zoom range **0.5 to 2.0** (`MIN_ZOOM`/`MAX_ZOOM`), step factor 1.2 (so the third zoom-out step is **58 %**, which the
  owner calls "50 %"). Fit and Focus buttons. Faces scale as a whole; text clamps at the 11 px floor and hides below 9.5 px.
- Captured window sizes: **1440 x 900**, **1280 x 800** (standalone; `kabl-ui --size 1280x800`) and **1180 x 680** (plugin default
  and standalone minimum). Light and dark. Native HiDPI scaling (e.g. 150 %) was verified once in an earlier task (`docs/panel-authoring`).
- Earlier explored idea (in the earlier target renders, not built): semantic zoom, where below about 0.55 zoom faces become
  overview cards (short name, mini display, only connected jacks).

### 2.8 Constraints a design must respect

1. **Immediate-mode egui**: no blur, no real shadows or filters. Inner shadows, glows and gradients are drawn with
   gradient meshes and stacked strokes, so they are harder-edged than an SVG mockup. Mockups must be buildable with: rects with
   rounded corners, strokes, linear/radial gradients (via meshes), alpha, vector paths, text in the Plex fonts. Avoid
   per-pixel noise, blurs, blend modes, masks, variable fonts, emoji.
2. **Custom painting is available** everywhere (`egui::Painter`), including bitmap faces: module skins support embedded light and
   dark PNGs with chosen control/jack positions (`panels.rs`), already used for user/composite modules.
3. **Performance budget**: the audio thread is separate and untouched by UI. UI frame budget on the dense Composition patch
   (41 modules, 76 cables) at 1440 x 900: **median under 8 ms, max under 16 ms**. Stage 1 measured 4.7 ms (light) / 3.5 ms (dark) median.
   Heavy effects must be cached as meshes per (theme, size, zoom).
4. **Plugin windows in hosts**: default 1180 x 680; hosts may resize; design must hold at that size (rack at about 100 % with one side panel
   open, or at lower zoom with panels closed). Do not rely on a large window. **[unverified]** how each host handles resize and HiDPI.
5. **Interaction**: mouse drag on knobs (with modifier fine-tuning), double-click to reset, scroll-wheel ownership rules
   (`wheel.rs`: wheel scrolls a view unless it is over a control that owns it), keyboard focus ring and shortcuts, MIDI CC learn on
   macros/knobs with soft takeover, right-click context menus, drag-to-cable between jacks, hover tooltips (`kit::Tip`).
   Every interactive widget needs hover, pressed, focus, disabled states (the kit widgets have them).
6. **Hit targets** come from `Style::metrics` and are not themable; do not make controls smaller than current hit areas.
7. **Layout is geometry-driven**: positions of every control and jack are computed in `rack.rs`. Cables and saved racks refer to
   jack identities and module grid positions; **do not change face widths** (it would shift neighbours in saved racks). Moving a
   jack a few pixels is acceptable (already accepted for faces with displays); port identities and order must stay.
8. **Light and dark, both themes** must work; the 11 px text floor; contrast checks run in tests (`contrast-check.txt`).
9. Accessibility: only contrast and keyboard focus are handled; screen readers are **[unverified]**/likely out of scope.
10. Licensing: fonts and any image assets must be OFL/MIT/CC0 or authored in-house (`ASSET-LICENSES.md`).

---

## 3. Current design state and history

### 3.1 History

- Early rack UI approved by the owner (Eurorack faces, cables, rails). Recorded owner preference: "continuous coloured satin faces,
  recessed hardware".
- `docs/design/app-target/` (earlier, "whole-application visual target"): three directions drawn with egui's own painter, A Satin
  Studio, B Lacquer and Light, C Silkscreen; resulting widget kit, themes, Perform view, routing inspector rail, status bar.
  Images 25 and 26 are from this set (they are *concept renders* of the app, not the shipped faces).
- `docs/visual-polish/` (AUDIT, PROPOSAL, BUILD, README): a polish audit against the "Vital / Pigments" bar.
  - **AUDIT** ranked 11 problems. Root cause: some things are drawn as part of the panel (knobs, jacks, face colour) and others as
    objects dropped onto it (displays, drum grid, editors, stock widgets), with no shared edge, depth or colour language. Top items:
    neutral black display slabs on tinted faces; theme B's hard gloss band; the drum grid as a separate widget with its own pastel
    and blue chips; three selector styles plus stock combo boxes; empty and crowded faces; unstructured port area, `Edit face`
    printed 51 times, `+N` chip colliding with long titles ("Comparato", "Wavetable Oscillat"); flat type hierarchy; text hidden at
    low zoom leaving unlabelled shapes; stock widgets in editors; theme B jack contrast.
  - **PROPOSAL**: every element belongs to exactly one layer: **Panel** (face material, title, labels), **Well** (anything that shows or
    edits data: displays, grid, stacks, pattern bars, port tray; cut into the panel with one frame recipe), **Hardware** (knobs, jacks,
    selectors, buttons; stand proud, own shadow). Shared rules: 4 px base spacing, 12 px face margin, 40 px header, 60 px display well,
    88 px port tray (two rows), knob size by count (r 24 for 1 to 2 knobs, 20 for 3, 17 for 4+), four type steps (title 14 SemiBold
    left-aligned, caption 11 mono, label 11 Medium, value 11 mono), one segmented selector idiom, no printed `Edit face`, continuous
    top light on lacquer. Two options: **Option 1 "Seated"** (hardware depth, bezels, inner shadow, glow) and **Option 2 "Tinted"**
    (flat, hue-continuous wells, flat-arc knobs). Images 15 to 17 show the drawn boards.
  - **BUILD**: ~34 agent-days, 4 stages, agents A to F with file ownership (theme model + face kit first, then families in parallel).
- **Decision**: Kosta chose **Option 1 "Seated" for both themes.** **Stage 1 merged (PR #49)**: displays with seated frames, continuous
  lacquer light, left-aligned titles, hover-only `Edit face` (image 14 is the after-stage-1 face sheet; compare with 11 and 13).
  **Stage 2a (PR #55, port tray, header, knob size by count, one selector idiom) is parked** **[relayed; PR #55 is not in the checked-out
  master history]**.
- Stereo cables design (`docs/stereo/DESIGN.md`): one jack and one cable carry stereo, no legacy half jacks; engine step done (width-2 ports exist
  in metadata). **Step 3 (UI: jack variants for stereo, `channels` parameter on cable look) was to start only after the visual PR
  merged, and now waits on this redesign.** The design must therefore include a **stereo jack and stereo cable look** (clearly
  distinguishable from mono at every scale, same signal-type colour language).

### 3.2 Owner's verdict **[relayed by the requesting agent, not from a file]**

> The current UI looks broken: panels glued together, titles collide, text missing at 50 % scale, FM Voice face broken at 100 %.

Observed in the images (the claims are checkable): the face sheet (11) shows the FM Voice (`osc.fm6`) tile cropped and overlapped
(operator boxes cut at the left, two `Ratio` selectors clipped, operator rows with touching labels like `Att 4`, `Dec 4`);
the 58 % sheet (12) shows unlabelled knobs and jacks; titles with `+N` chip overflow on narrow faces; image 01 shows the rack
inside a REAPER FX window with large empty bands above the rack.

### 3.3 What is good and should be kept (from the audit)

Knob design (satin cap with skirt, glass in B), face colour families, display trace colours and live behaviour, Perform view
structure, toolbar, rack rails, cable look, jack colour language. The owner wants the *kind* of thing kept; he wants it executed cleanly.

### 3.4 The new workflow the owner asked for

Design workflow instead of code-first: **target mockup first**, a **fixed capture set** of real screenshots, **automatic overlap / truncation /
gap checks**, a **side-by-side review page** (target vs real), then **owner sign-off** before merge. Details are for you to plan (section 5).

---

## 4. Design examples: what each representative face and screen must show

Parameter names are the engine's own; display labels may differ. Ranges in brackets. Port directions: I = input, O = output.
Signal types: audio, cv (+-1), ucv (0..1), gate, pitch (semitones, 1V/oct). Stereo = width 2.

### 4.1 VA Oscillator (`osc.va`, 4 HP-style wide face, about 240 px) (images 11, 15)

- Ports: pitch I (pitch), sync I (gate), out O (audio), stereo O (audio x2).
- Controls: Frequency `base_hz` [20..20000 Hz, exponential], Waveform `waveform` [SIN TRI SAW SQR, stepped], Pulse width `pw` [5..95 %],
  Fine `fine` [-100..100 ct], Unison `unison` [1..16], Detune `detune` [0..100], Curve, Width (stereo spread), Blend, Restart (on/off).
  Only Frequency and Waveform are on the face today; the rest hide behind the `+4` chip.
- Display: live waveform. Needs: one large knob, 4-way waveform selector, tray with 2 inputs and 2 outputs (one stereo).

### 4.2 SVF Filter (`filter.svf`, 210 px) (image 11, 15)

- Ports: in I (audio), cutoff_cv I (cv), resonance_cv I (cv); lp, bp, hp, notch O (audio).
- Controls: Cutoff [20..20000 Hz exp], Resonance [0..0.95]. Display: live frequency response with swept range.
- Stress case: labels `Cutoff CV` and `Res CV` nearly touch at 210 px; four outputs in one row with labels.

### 4.3 ADSR Envelope (`env.adsr`, 210 px)

- Ports: gate I, out O (unipolar cv). Controls: Attack, Decay, Release [0.1..10000 ms, exp], Sustain [0..1], Timing mode CONT / KEY.
- Display: envelope curve with a playhead that follows the gate. Mode selector goes in the header in the proposal.

### 4.4 Sequencer (`seq`, wide, about 660 px) (images 01, 11, 07)

- Ports: clock I, reset I (gates); gate O, pitch O, velocity O.
- Controls: edit bank A B C D and play bank A B C D (two selectors), 8 steps each with pitch (semitones, shown `+7 st`), gate ON/OFF toggle;
  hidden behind `+22` / `+30`: velocity, probability, ratchet per step, length, gate length, gate mode. Live step indicator (dot over the playing step).
  Needs: a clear step-row structure (8 columns, aligned labels P1..P8 / G1..G8), bank selectors, `+N` access to the per-step rows.
  **Pending change (seq-32 branch, not merged): 32 steps per pattern, face shows pages of 8. Design for that.**

### 4.5 Drum Grid (`drum.grid`, wide, about 660 px) (images 11, 18)

- Ports: clock I, reset I, pattern I (ucv); 8 gate outputs: kick, snare, clap, chat, ohat, tom, perc1, perc2.
- Editor on the face: 8 rows x 16 cells (per-row length shown by dimming), pattern select A B C D, brush velocity (4 levels: 30, 55, 78, 100),
  chance brush (100, 75, 50, 25 %), per-cell velocity/chance colouring, playhead column, row names. Needs: a well, 4-step grouping, header
  segmented controls, a clear off / low / high / chance state language that is not just colour.

### 4.6 Mixer (`mixer`, about 210 to 240 px)

- Ports: in1..in4 I (audio), out O. Controls: Level 1 to 4 [0..1]. No display: the classic "empty face" case; the knob block should look
  intentional (2 x 2 block with larger knobs, or a small input meter strip, which would be new content).

### 4.7 FM Voice, 6 operators (`osc.fm6`, 660 px, the stress case) (images 11, 13)

- Ports: pitch I, gate I, velocity I (ucv); out O (audio).
- Global controls: Algorithm [8 choices, shown like `6>5>4>3>2>1`], Feedback [0..1], Index [0..2], Fine [-100..100 ct], Frequency `base_hz`,
  Oversample (2x/4x).
- **Per operator (6 operators):** Ratio (dropdown), Fine, Level, Velocity sensitivity, and a 4-stage ADSR (Att, Dec, Sus, Rel); each operator is
  a box of about 7 controls. That is about 45 controls on one face, so it needs sub-wells (operator cards), a clear algorithm picture, and sane
  label collision rules. Today this face is the worst offender.

### 4.8 Other faces worth including in a sheet

Compressor (GR meter, 5 knobs, sidechain input), EQ (8 knobs + response curve, 660 px), Vocoder (band display), Delay (time, feedback, mix, sync
FREE/1/16/1/8/1/8D/1/4), Macros (8 knobs), Cues (8 pads, empty state), MIDI In (keyboard strip, POLY/MONO/LEGATO, glide), Clock (bpm, swing, Stop,
Restart, Host/Synced), Output (concentric-ring logo, Left/Right), Random/Quantizer/Sample and Hold (selector heavy), Logic, Comparator, Ring Mod.
Image 11 and 13 show all of them in one sheet per theme.

### 4.9 Perform view with 4 macros and cues (images 09, 26, 27)

- Transport card: Run/Stop, Restart, tempo (large, readable from a distance), bar:beat readout and a beat meter.
- Scenes card: up to 8 large cue pads (idle, queued with progress, playing), named for the sound's sections.
- Macros card: 4 (up to 8) large arc knobs, each named (`Bite`, `Wash`...), value, CC badge (`CC 20`), list of destinations moved.
- Sequence/bank cards: A B C D pills per sequencer with a step lane showing level and probability (one hue per lane).
- Other pinned controls in equal columns. Must fit 1180 x 680 with no scrolling for the common case.

### 4.10 Chrome pieces to include in the component library

Toolbar (about 20 items; must not overlap at 1180 px), side rail, status bar, routing drawer + inspector rail, Sounds browser rows + audition
card, search field, chips, tabs/segmented, dropdown, modal dialog, tooltip, toast, context menu, wavetable editor toolbar, cable pattern editor,
empty states, About box, Diagnostics popover.

---

## 5. What the design agent should deliver

### 5.1 Design plan

1. **Principles** (3 to 6, each testable), including the layered Panel / Well / Hardware idea if you keep it or your replacement.
2. **Visual language** for both themes (or a justified single language); decide whether to keep the owner-chosen Option 1 "Seated" depth, evolve it, or
   propose a different direction, and say why. Include light and dark.
3. **Layout system for a face**: fixed zones (header, display well, control zone, port tray), grid, spacing scale, margins, how widths 210 / 240 / 300 / 660
   are filled, what happens with 1 control and with 45 controls, how labels wrap or abbreviate, where `+N` lives. State the rules as numbers a
   coding agent can implement and a checker can test (min gap between any two boxes, min margin to the face edge, etc.).
4. **Typography**: type steps with sizes, weights, case, which font (Plex unless you justify otherwise), 11 px floor, **what replaces text when it would be below 9.5 px**
   (value arcs, icons, hidden-label rules). No emoji or non-Plex glyphs.
5. **Colour and theme tokens**: concrete values for the existing token names (section 2.5), per-section face palettes, signal-type colours (audio, cv, gate,
   pitch, stereo), states. Contrast numbers for text pairs. Map to existing slots; list any **new slots or recipes** needed.
6. **Component library**: knob (sizes by count, value arc, modulation arc and live dot), jack (mono, **stereo**, connected, hover, disabled), cable (rest, lit, focus, stereo,
   functional/patterned), selector/segmented, toggle, dropdown, button (primary / ghost / icon), chip, field, slider, pad (cue and drum cell), step lane, meter, display
   wells (scope, response curve, envelope, grid), tooltip, menu, dialog, toast, card.
7. **States**: default, hover, pressed, focus (keyboard), disabled, selected, modulated (range arc + live dot), queued/playing (pads), error/unconnected.
8. **Scaling rules**: behaviour at 50 / 58 / 75 / 100 / 150 / 200 % rack zoom, plus app windows 1180x680, 1280x800, 1440x900 and HiDPI. A low-zoom mode (overview faces) if you
   recommend it.
9. **Screen designs** (full mockups at 1440x900 and 1180x680, light and dark at least for the first two): Rack (simple and dense patch, drawer open and closed), Perform, Sounds
   browser, one editor (drum grid or wavetable), dialogs, plugin-in-host framing.
10. **Face sheet**: mockups of all faces in section 4 at 100 % and 58 %, using real control lists, not placeholders. Mockup format: SVG or HTML/CSS that uses only the primitives in
    section 2.8, with the real pixel sizes (faces 340 px tall) so measurements transfer 1:1.
11. **Buildability notes** per element: what egui can do directly, what needs meshes, what is risky for frame time.

### 5.2 Workflow plan (implementation + review)

Plan these; propose your own where better. Facts about how work is done here: the owner runs several coding agents (Claude / Codex style) each in its own **git worktree** on its
own branch; each agent opens a **draft PR**; **only the owner merges**; agents never push to master. CI-like checks: `cargo test --workspace`, `cargo clippy -D warnings`,
`cargo fmt --check`. A capture toolchain already exists (`docs/visual-polish/scripts/`): `capture_faces.py` renders every module kind alone in a patch under each theme, light/dark
and 100 % / 58 % zoom via the release editor under Xvfb (silent audio sink); `capture_scenes.py` renders chrome scenes at 1440x900 and 1180x680; `sheets.py` builds contact sheets;
about 20 minutes for faces, 6 for scenes. UI tests already assert non-overlap for Perform and the toolbar at several widths (`crates/ui/tests/perform.rs`, `browser.rs`,
`macros_perform.rs`); `rack.rs` is pure geometry and can be unit-tested for rect overlap.

Your workflow plan should cover:

1. **Phases and gates**, in order: (a) target mockups and design tokens, owner review, sign-off; (b) kit and theme model (single owner of `style/` and `kit/`), merged first; (c) faces
   by family in parallel on separate files; (d) editors; (e) chrome; (f) low zoom + stereo jacks/cables; (g) full verification. State what the owner sees at each gate.
2. **Fixed capture set**: exact list (module kinds x theme x mode x zoom x window size, plus scenes), naming scheme, deterministic seeds/fixtures so before/after diffs are meaningful.
   Include the plugin at 1180x680 and a native HiDPI run, which previous tasks did not capture in a host.
3. **Automatic checks** (run on every capture or in geometry unit tests): overlap between any two text/control boxes; text clipped by its container or the face edge; text truncated with an
   ellipsis where a full label is expected; minimum gaps (label to knob, jack to jack, title to chip); missing-text detection at low zoom (a label that should exist but is hidden);
   alignment to the grid; contrast of text pairs; jacks/controls outside the face; empty-zone ratio. Say which can be done on geometry (no GPU) and which need pixels, and what the
   report format is (JSON + HTML).
4. **Review page**: static HTML built from the capture set: target mockup vs real capture side by side per face/scene/theme/mode/zoom, a flip/blink toggle, auto-check findings overlaid,
   filter by failing items, per-item owner verdict (approve / change) saved to a file that coding agents read.
5. **Sign-off gates** and who decides (owner only); rules for what an agent may do alone (bug fixes inside approved tokens) versus what needs a new mockup.
6. **Work split across coding agents** in worktrees: file ownership table (agent A: `style/*`, `kit/*`; B: `rack.rs`/oscillators/filters/envelopes; C: effects/utilities; D: timing and drums; E: editors; F: chrome),
   merge order, how to avoid conflicts in `rack.rs` and `lib.rs`, PR size limits, what each PR must attach (captures + check report), how rebases happen when A merges.
7. **Rollback and staging**: ability to ship stage by stage, keep a theme switch, keep saved racks and presets loading unchanged (geometry rule section 2.8.7).
8. **Performance gate**: dense-patch frame time budget (median < 8 ms, max < 16 ms), measured with the existing `bench_chrome` tool **[tool name from docs/visual-polish/BUILD.md; not verified here]**.
9. **Prompts**: for each coding agent type, the short brief it would receive (inputs: tokens, mockups, capture list, ownership, acceptance).

---

## 6. Open questions for the owner (please carry these into your plan; do not assume answers)

1. **Direction**: keep Option 1 "Seated" as chosen, or is a new direction wanted after the "looks broken" verdict? How much of the current look must survive (rack idiom, satin faces,
   recessed hardware) versus a fresh start?
2. **Themes**: one theme or two (A Satin, B Lacquer)? Light and dark both mandatory? Is a third built-in theme wanted?
3. **Priority screen**: faces, Perform, or the whole app first? Is the DAW plugin window (1180x680) the priority size or the standalone 1440x900?
4. **Face width policy**: widths 210/240/300/660 are frozen for saved racks. Is a *rack/layout migration* ever acceptable if a better face needs different widths (FM Voice, EQ, Mixer)?
5. **Low zoom**: what must a 50 % rack communicate (names, values, only connected jacks)? Is semantic zoom (overview cards below ~55 %) welcome?
6. **Labels**: may labels wrap to two lines on narrow faces, or abbreviate (`Cutoff CV` to `CV`)? Is hiding secondary controls behind `+N` still the answer for faces with 45 controls (FM Voice)?
7. **Stereo**: how should a stereo jack and cable look (double-ring jack, two-tone cable, "L/R" tag)? Must stereo be readable at 58 %?
8. **Fonts**: stay with IBM Plex, or is a different OFL font wanted for the display role?
9. **Mockup medium**: SVG/HTML mockups acceptable, or must they be rendered by egui itself before sign-off (slower, exact)?
10. **Review cadence**: how many review rounds per stage, and in what medium (review page, PNG sheets, screen share)?
11. **Parked PR #55 and merged stage 1**: keep stage-1 code as the base, or reset to a clean design?
12. **Sequencer pages**: seq-32 (in flight) brings 32 steps in pages of 8 alongside banks A to D; confirm how pages and banks should read together on the face.
13. **Accessibility and localisation**: any requirement beyond contrast and keyboard focus? Non-Latin text is out of font coverage today.
14. **Plugin host variations**: which hosts beyond REAPER matter, and do they impose resize or HiDPI behaviour we must test?

---

## 7. Image index

All in `images/`. "Current" means a real capture of the shipping app; "drawn" means a mockup not rendered by the app.

| # | File | Shows | Right / wrong |
|---|---|---|---|
| 01 | `01-current-rack-dark-1440x900.png` | Standalone-style rack (a CLAP editor window in REAPER, A-dark, 65 % zoom) with two rows: clock, macros, sequencer, VA osc, SVF, ADSR; VCA, delay, reverb, output, VCA; coloured cables. | Right: cable colour language, jack trays. Wrong: heavy empty band between toolbar and rack, host chrome crowds the top, text tiny at 65 %, jack tray plates look pasted, `+N` chips squeeze titles (`VA Oscillator +4`). |
| 02 | `02-current-rack-light-1440x900.png` | Same patch, A-light. | Compare contrast of face colours and jack holes against 01. |
| 03 | `03-current-rack-dark-1280x800.png` | Same, 1280x800 dark. | Smaller window: text and tray density. |
| 04 | `04-current-rack-light-1280x800.png` | Same, 1280x800 light. | As 03. |
| 05 | `05-plugin-in-reaper-dark-1280.png` | Plugin editor inside REAPER, 1280 dark. | Shows real host framing, tiny type. |
| 06 | `06-plugin-in-reaper-light-1440.png` | Plugin in REAPER, 1440 light. | As 05. |
| 07 | `07-standalone-preset-acid-line-rack.png` | Standalone rack with a factory preset (Acid Line): sequencer, osc, filter, envelopes, drum grid. | Real "dense" content. |
| 08 | `08-standalone-preset-slab-city-rack.png` | Another factory preset rack (Slab City). | Variety of faces together. |
| 09 | `09-standalone-preset-acid-line-perform.png` | Perform view of the same preset: transport, cue pads, macros. | Reference for the most finished surface. |
| 10 | `10-standalone-preset-rope-walk-rack.png` | Rack of preset Rope Walk. | As 08. |
| 11 | `11-faces-sheet-satin-light-100pct.webp` | Contact sheet of all module kinds, theme A light, 100 %, **before** polish (alphabetical, label above each tile). | Wrong: black slabs on tinted faces, empty faces (Comparator, Logic, Ring Mod, Random...), `Edit face` on every face, titles clipped, FM Voice (`osc.fm6`) tile cropped and clustered, Sample & Hold `SAMPLE` clipped. Right: knob design, colour families. |
| 12 | `12-faces-sheet-satin-dark-58pct-text-missing.webp` | Same sheet, theme A dark, 58 % zoom. | Wrong: labels disappear, unlabelled knobs/jacks. Right: displays stay legible. |
| 13 | `13-faces-sheet-lacquer-dark-100pct.webp` | Sheet, theme B dark, 100 %, before polish. | Wrong: hard gloss band cuts through labels; black jack holes with little contrast; neon halo touches labels. |
| 14 | `14-faces-sheet-after-stage1-satin-light-100pct.webp` | Sheet after merged stage 1 (seated displays, left titles). | Better displays; remaining: crowded/empty faces, selectors, port area. |
| 15 | `15-mockup-board-satin-light-current-vs-option1-option2.png` | Drawn board: current capture row, then Option 1 and Option 2, six faces (oscillator, filter, ADSR, drum grid, compressor, mixer), theme A light. | Shows the proposed well/tray/hardware layers. Limits: SVG filters egui cannot do. |
| 16 | `16-mockup-board-lacquer-dark-current-vs-option1-option2.png` | Same board for theme B dark. | Calmer dark; Option 2 flat wells. |
| 17 | `17-mockup-closeup-satin-light.png` | 2x close-ups of oscillator and ADSR in the drawn options. | Detail of bezel, tray, knob size by count. |
| 18 | `18-scene-drum-grid-1440x900.webp` | Drum grid face in a rack, 2x2 sheet of themes (pre stage 1). | Wrong: grid cells black on pale face, blue chips, own pastel colours; not a "well". |
| 19 | `19-scene-cable-pattern-editor-1180x680.webp` | Cable step-pattern editor window. | Wrong: stock sliders/combos, tiny captions, plain window. |
| 20 | `20-scene-wavetable-editor-1440x900.webp` | Wavetable editor. | Wrong: row of small outlined buttons that look disabled, two slabs of different heights. |
| 21 | `21-scene-dense-rack-routing-drawer-1180x680.webp` | Dense rack at plugin size with the routing drawer open. | Wrong: drawer takes a third of width, leaves two rack rows; dark drawer edge only a hairline. |
| 22 | `22-scene-sounds-browser-1440x900.webp` | Sounds browser. | Right: consistent and legible (chrome is "in good shape"). |
| 23 | `23-scene-runtime-controls-light-1440x900.png` | Rack with runtime controls / macros in use, light. | Context for modulation display. |
| 24 | `24-scene-inspector-during-cc-dark-1280x800.png` | Routing inspector while a MIDI CC moves a control, dark. | Shows numbered cable list and modulation. |
| 25 | `25-earlier-target-components-satin-light-1440x900.webp` | Earlier target concept: buttons, chips, fields, lists, tooltip, menu, dialog, toast, status bar (drawn via egui painter). | Reference for chrome component set that exists as a kit. |
| 26 | `26-earlier-target-perform-satin-dark-1440x900.webp` | Earlier target concept of Perform. | Basis of the shipped Perform. |
| 27 | `27-scene-perform-current-1440x900.webp` | Current Perform sheet (2x2 themes). | Most finished surface; keep hierarchy. |

Note: scene and sheet `.webp` images are 2x2 or contact sheets (themes A/B x light/dark) as produced by the capture scripts; open at full size to read.

---

## 8. Facts I could not verify

- The owner's verdict quote and stage 2a PR #55 status (relayed by the requester; PR #55 is not in this checkout's history).
- Which exact face widths exist beyond 210/240/300/660 (from `PROPOSAL.md`; `rack.rs` has a 210 px test fixture).
- The exact tool name and usage of `bench_chrome` (named in `BUILD.md`, not located in the tree).
- Live behaviour (hover, drag, focus, animations): nothing was run; all statements come from code, docs and stills.
- Host behaviour for resize/HiDPI beyond REAPER, and native 150 % scaling results (only cited from older docs).
- Image 01 to 06 show the plugin/editor inside REAPER; I did not open images 02 to 10 and 15 to 27 in full, so their right/wrong notes come from the docs that produced them (AUDIT, PROPOSAL, handovers) and from the sheets I did view (01 and 11).
