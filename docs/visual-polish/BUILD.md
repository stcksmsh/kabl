# Build estimate for the visual polish pass

Rough, for one engineer (or agent) who knows the UI crate; the numbers are agent-days of focused work, not
calendar time, and they exclude Kosta's review rounds. They assume the proposal in [`PROPOSAL.md`](PROPOSAL.md)
with the recommended mapping (Option 1 for theme A, Option 2 for theme B). Choosing one option for both themes
removes about 1.5 days (one recipe set instead of two); choosing Option 1 alone costs the most per-frame tuning.

## What changes, by layer

### Theme model (`crates/ui/src/style/`, `docs/design/app-target/THEMING.md`)

New recipe values and dials in slots that already exist, so a theme file keeps its shape and old user themes
keep loading (an unknown recipe name already falls back per value with a message):

| Slot | Addition |
|---|---|
| `face.material` | `satin-soft` (halved gradient, light top edge), `tinted` (4% top light, hairline); `lacquer` loses the hard gloss step and gets a continuous top light |
| `display.frame` | `seated` (bezel, lip, inner shadow, sheen) and `tinted` (deeper face hue, hairline); dials: `bezel`, `inner_shadow`, `sheen`, `tint_depth` |
| `face.ports` (new) | `recess` or `flat`: how the port tray is drawn |
| `knob.body` / `knob.scale` | `flat-arc` (flat disc inside a value arc) alongside `cap` and `glass` |
| `jack` | `nut-seated` (metal nut in the recess) alongside `nut`, `glow-ring`, `donut` |
| `selector` | one idiom, `segmented-dark`; the light variant goes |
| `well` (new) | shared frame metrics for every well: radius, margin, grid-gap |

Also: loader clamps and validation for the new dials, round-trip test for both built-ins, contrast tests for
the new wells and the segmented control, `THEMING.md` section 13 update. Estimate: **3 days**.

### Face kit (`crates/ui/src/kit/face.rs`, `kit/recipes.rs`, `kit/controls.rs`)

- `well(rect)`: the one frame function every display, grid and editor calls (replaces `display_frame` and the
  private frames in `drum_grid.rs` and `tables.rs`).
- `ports_tray(rect, inputs, outputs)`, `header(title, caption, chip)`, `knob_block(...)` with the size-by-count
  rule, `segmented(...)` unified (including the compact header variant), kit drop-down replacing the stock combo
  boxes on faces.
- Title left aligned with the one-step shrink before truncation; `Edit face` moved to hover and the context menu.
- Unit tests for the rect arithmetic (a face's header, well, knob zone and tray never overlap at any width).

Estimate: **6 days**. Blocks everything below.

### Face layout (`crates/ui/src/rack.rs` and the face drawers)

`rack.rs` places controls and jacks per module; the new grid moves them into the header, well, knob zone and
tray rects. The 51 module kinds fall into a few templates, so most of the work is the template, not the kinds:

| Family | Files | Notes |
|---|---|---|
| Oscillators, filters, envelopes, LFO | `displays.rs`, `rack.rs` | displays move into the well; ADSR selector to the header; 8 to 10 kinds |
| Effects with meters, dynamics, EQ, vocoder | `fx_displays.rs` | meter and curve wells; `eq`, `vocoder` are the wide faces |
| Utilities, logic, mixer, midi, output | generic path in `rack.rs` | no display; the knob block takes the space; empty-face fix |
| Timing and drums: drum voices, drum grid, seq, arp, cues, macros | `drum_grid.rs`, `drum_displays.rs`, seq, arp, cues code | the grid becomes a well with the header controls; the sequencer's step buttons follow the segmented recipe |
| Special: osc.fm6 | `rack.rs` | operator boxes become sub-wells; label collisions fixed |

Estimate: **10 days** across the families (three agents, see below). Jack positions move (inputs about 11 px
down, outputs about 3 px), so tests that assert pixel positions need updating; port identities and routing do
not change. Frame time must be re-measured: budget stays median under 8 ms and max under 16 ms on the dense
Composition at 1440x900; Option 1's shadows and glow are the risk, so cache the well and tray meshes per
(theme, size, zoom) and only redraw the trace.

### Editors and inspector (`tables.rs`, `routing.rs`, `pattern.rs`, `inspect.rs`)

Wavetable editor: header row with kit buttons (Insert, Duplicate, ... become a toolbar of icon plus label
buttons at one height), stack and wave in wells of equal height, slider and tabs from the kit. Cable pattern
editor and routing inspector: stock slider and combo boxes replaced with kit controls; pattern bars in a well.
`routing.rs` is 1834 lines and several agents touch it, so this is one agent's job alone. Estimate: **6 days**.

### Chrome and dialogs (`lib.rs`, `browser.rs`, `about.rs`, `panels.rs`)

Small: About box layout hierarchy, drawer edge in dark B, Sounds audition panel at 680 px, drop-down restyle in
the settings menu. Perform stays as is. Estimate: **3 days**.

### Low zoom and theme B gaps

Value arcs on knobs and a state glyph on selectors when text is hidden; B jack contrast and signal rim always
on; neon halo margin; read the unread slots (`plug`, `face.out_plate`, `controls.row`, rack surface). Estimate:
**3 days**.

### Verification and docs

Re-run `docs/visual-polish/scripts/capture_faces.py` and `capture_scenes.py` plus `sheets.py` (they already
exist), re-check in REAPER at 1180x680 (not done in this audit), bench, fix tests, update `THEMING.md` and the
face handover. Estimate: **3 days**.

**Total: about 34 agent-days.** With the split below that is roughly **4 weeks of calendar time** with three
agents, plus review time. A cheaper first release exists (see "Staged delivery").

## Order and split without collisions

Phases 1 and 2 are serial because everything depends on the kit; the families then run in parallel on separate
files.

1. **Agent A, theme model + face kit** (`style/`, `kit/`): days 1 to 9. Owns `style/*`, `kit/*`, `THEMING.md`.
   Merges first. Nobody else edits these files.
2. **Then in parallel, each on its own files:**
   - **Agent B**: oscillators, filters, envelopes (`displays.rs`) and the `rack.rs` template and generic path.
     Owns `rack.rs` (nobody else edits it; others ask B or send a patch to B).
   - **Agent C**: effects and utilities (`fx_displays.rs`, utility faces), and the 51-kind sweep: capture, fix
     collisions, recapture. Edits only `fx_displays.rs` and per-kind constants B exposes.
   - **Agent D**: timing and drums (`drum_grid.rs`, `drum_displays.rs`, seq, arp, cues, macros).
3. **Agent E, editors** (`tables.rs`, `routing.rs`, `pattern.rs`, `inspect.rs`): can start once the kit's
   `well`, `segmented`, `dropdown` exist (end of step 1); independent of B to D.
4. **Agent F, chrome and low zoom and B gaps**: after B lands the template; owns `lib.rs` chrome parts,
   `about.rs`, `browser.rs`, `cable_look.rs`.

Each agent keeps behind the existing theme switch; no new environment variables are needed, because the
built-in themes ship with the new recipes only when the whole set lands.

## Staged delivery (if Kosta wants something visible sooner)

| Stage | Contains | Days | What Kosta sees |
|---|---|---|---|
| 1 | B gloss step fix; `well()` with the chosen frame used by every display; `Edit face` removal; title left aligned | 5 | Displays stop looking glued on; B stops showing the step. Smallest change with the largest effect (audit problems 1 and 2). |
| 2 | Port tray and header; knob size by count; one selector idiom | 8 | Faces look systematic. |
| 3 | Drum grid and the other editors | 10 | The grid and editors join the panel. |
| 4 | Chrome, low zoom, B gaps, verification | 11 | The pass is complete. |

## Risks

- **Frame time** with Option 1 (shadows, glow). Mitigation: cache meshes, measure early with
  `bench_chrome 300 live` on the dense patch.
- **Jack position move** touches `rack.rs` layout and every pixel test; do it once, in agent B's template.
- **egui has no blur**: inner shadows and glows are drawn with gradient meshes and stacked strokes; the final
  look will differ slightly from the drawn mockups. Review the first well on a real face before the rest.
- **Label wrap on narrow faces** (SVF has two six-letter-plus-CV labels at 210 px): needs a decision, see
  README.
- **Theme files in the wild**: new recipe names must default sensibly for a theme file that does not name
  them (the loader already falls back per value).
