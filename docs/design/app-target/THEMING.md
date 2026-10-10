# kabl theming: design note

Status: **proposal for Kosta's decision. Nothing is built.** Written after the design-target captures (README in this folder). A (Satin Studio) and B (Lacquer & Light) are treated as the first two built-in themes, and are the test of whether this system is expressive enough. A is the default unless Kosta says otherwise.

Principle: the interface is customisable and extensible by design, the way the engine is. A theme is a small package of parts that compose, not one fixed look and not a stylesheet bolted on afterwards.

## 1. Vocabulary

| Term | Meaning |
|---|---|
| **Token** | A named value: colour role, size, radius, shadow, duration. Data. |
| **Recipe** | A named, shipped way of drawing one component part (for example `knob.body = glass`). Code that lives in the app. A theme *selects* recipes and *tunes* their dials; it does not contain them. |
| **Dial** | A bounded number or choice a recipe exposes (glow strength, arc width, gradient depth). Data. |
| **Slot** | One place a recipe is chosen: knob body, jack, cable, face material, display frame, and so on. |
| **Theme** | Tokens + recipe choices + dials + section palettes + optional embedded assets, for a light and a dark mode. |
| **Panel** | The existing D10 composite face: which controls exist, where they are, optional art. Not part of a theme. |

## 2. What a theme can change

"Data" means a number, colour or choice in the theme file. "Recipe" means the theme picks among drawing code that ships with kabl. "Code" means something a theme cannot do without new drawing code.

### 2.1 Tokens (all data)

| Group | Contents | Bounds enforced on load |
|---|---|---|
| Colour roles | `bg surface raised inset line line2 text text2 text3 accent on_accent accent_soft focus audio cv gate good warn bad rack rail hole` | Text roles opaque; others alpha allowed |
| Display roles | `disp_bg disp_grid disp_trace disp_text` | Same |
| Knob roles | `knob knob_hi knob_ink` | Same |
| Section palettes | per section `base ink ink2` for oscillator, filter, envelope/LFO, amp/mix/out, timing, effects, plus optional per-kind overrides (`osc.wt`, user kinds) | `ink`/`ink2` contrast against `base` checked |
| Type scale | seven roles (display, title, h3, body, label, value, caption), each = font ref + size + case + tracking | Sizes clamp to 11 to 40 px; **11 px floor is not themable** |
| Spacing | scale of 8 steps | 0 to 64 px |
| Radii | small, medium, large | 0 to 24 px |
| Shadows | three levels (offset, blur, alpha) | blur at most 48 px |
| Motion | hover, press, drawer, view, glide, pulse durations; easing from a closed list | 0 to 1200 ms; "reduce motion" always overrides |

### 2.2 Slots (recipe choice + dials)

This is the list of things that actually differ between A and B today.

| Slot | Recipes shipped (v1) | Dials | Data / recipe / code |
|---|---|---|---|
| `face.material` | `satin`, `lacquer`, `flat` | top/bottom gradient mix, gloss band alpha, grain alpha, edge colour mix, outline width, shadow level | Recipe + dials |
| `face.hardware` | `nuts`, `none`, `screws` | size | Recipe |
| `face.title` | `centered`, `left-dot`, `caps-rule` | size role refs | Recipe |
| `face.out_plate` | `tint`, `dark`, `outline` | alpha, radius | Recipe + dials |
| `rack.surface` | `plain`, `grid`; rail style `bar`, `slotted` | grid alpha, hole size | Recipe |
| `knob.body` | `cap` (skirt with grip), `glass`, `disc` | highlight alpha, rim alpha, grip count | Recipe + dials |
| `knob.scale` | `ticks`, `track-arc`, `none` | tick count/length, track width | Recipe + dials |
| `knob.value` | `line`, `dot`, `notch`, `arc` | arc width, glow | Recipe + dials |
| `knob.mod` | `arc-dot`, `arc-glow`, `segment` | offset from rim, width, glow, dot size | Recipe + dials |
| `selector` | `inset-pill`, `accent-pill`, `outlined` | radius, padding | Recipe |
| `jack` | `nut`, `glow-ring`, `donut` | ring width, glow | Recipe + dials |
| `cable` | `rope` (shadow + highlight), `glow`, `outlined` | width, sag, glow, bead size/speed, dimmed alpha | Recipe + dials |
| `plug` | `cap`, `ring`, `dot` | radius | Recipe |
| `display.frame` | `glass` (inner shadow), `neon` (outer tint), `paper` | grid density, border width, glow | Recipe + dials |
| `display.trace` | `soft-glow`, `strong-glow`, `crisp` | width, glow, fill alpha, playhead style | Recipe + dials |
| `button`, `chip`, `field`, `slider`, `toggle`, `list-row`, `tag`, `card` | shape `rounded`, `pill`, `square`; fill `flat`, `gradient`; outline | radii/outline from tokens | Recipe |
| `chrome.view_switch` | `segmented`, `tabs` | none | Recipe (closed layout variant) |
| `chrome.navigator` | `left`, `preset-bar` | width | Recipe (closed layout variant) |
| `browser.rows` | `cards`, `table` | row height | Recipe (closed layout variant) |
| `icons` | `stroke` | stroke width | Recipe (one set in v1) |

Knob parts compose: A is `cap + ticks + line + arc-dot`, B is `glass + track-arc + dot + arc-glow`. A knob that mixes them, such as `cap + track-arc + line + arc-glow`, needs no new code. That is the main source of the "deep" expressiveness without a general scripting layer.

### 2.3 What is deliberately not themable (in any version)

Layout geometry and hit areas (face sizes, jack and knob positions, control spacing), which panels exist and where they dock, the 11 px text floor, accessibility metadata (labels, value text, tooltips), the number of colour channels for state (state is never colour alone), and anything on the audio path. A shared theme therefore cannot hide or move a control.

## 3. How themes relate to module panels (one skinning system, two layers)

The D10 work (`docs/panel-authoring/design.md`, `crates/modules/src/skin.rs`) already lets a composite or a skinned module have its own face: embedded light/dark PNGs, placed controls keyed by immutable interface ID, labels on theme plates unless `labels_on_art`.

Rule: **the panel says what and where; the theme says how.**

| Concern | Owner |
|---|---|
| Which controls exist, their IDs, placement, face size, optional background art | Panel (D10), unchanged |
| How a placed knob, selector, jack, label plate, value text and cable are drawn | Theme recipes |
| Procedural face when a panel has no art (the D10 "missing artwork" fallback) | Theme `face.material` + section palette |
| Face colour of built-in modules | Theme section palette |
| Art on a panel | Panel. The theme draws controls and plates over it, as today |

So there is not a second skinning system. A panel with art overrides only the *face background* of that one module. Everything on top follows the theme. The panel format does not change.

Future, optional, not v1: a panel may carry extra art keyed by theme id (`art_for: { "<theme id>": png }`) with the existing light/dark pair as the fallback. That is an optional field and adds no rework, as D10 already deserialises older composites without it.

A panel's `labels_on_art` contrast stays the panel author's responsibility, as now.

## 4. Package format and installation

A theme is a folder or a single zip with this layout:

```
my-theme/
  theme.toml          required
  assets/             optional PNGs (grain, texture, section art)
  LICENSE             required if assets are present
```

`theme.toml` sketch (abridged):

```toml
id = "studio.satin"          # lowercase dotted, unique; the package identity
version = "1.0.0"            # semver; published versions are immutable
name = "Satin Studio"
author = "kabl"
license = "MIT OR Apache-2.0"
ui_api = 1                   # recipe/slot vocabulary this theme was written against
extends = "kabl.base@1"      # optional; chain depth at most 4, cycles rejected
modes = ["light", "dark"]

[type]                       # font refs name built-in families only in v1
display = { font = "plex-sans-semibold", size = 28 }
label   = { font = "plex-sans-medium",   size = 12 }

[light.colors]   # roles from 2.1
[dark.colors]

[light.faces.osc]  base = "#d79a82"  ink = "#2a1c14"  ink2 = "#3e291d"

[slots]
face.material = { recipe = "satin", grain = 0.02, gloss = 0.0 }
knob = { body = "cap", scale = "ticks", value = "line", mod = "arc-dot" }
cable = { recipe = "rope", width = 4.6 }
display.frame = { recipe = "glass" }
chrome = { view_switch = "segmented", navigator = "left" }
```

Rules:

- **Validate-then-install, all or nothing.** Same discipline as D10: parse, bounds-check every number, check ids, sizes and contrast, then install. A rejected package changes nothing and reports each problem.
- **Embedded assets only.** No filesystem paths or URLs in a theme. PNG caps follow D10: 128 KiB each, 2048 px per edge, one megapixel, 8 megapixels in total.
- **Fonts.** v1: built-in families only (IBM Plex Sans, Condensed, Mono; OFL-1.1). Embedded fonts are a later step: they add a parser to the trust surface and a licence question.
- **Where installed.** User themes live in the per-user config directory under `themes/<id>@<version>/`. They are installed by Settings → Appearance → Install theme…, or by dropping a package into that folder. Built-ins ship in the binary as the same files, parsed by the same loader, so the loader is exercised on every launch.
- **Immutable versions.** Editing a theme creates a new version or a copy with a new id. Selecting `id` uses the newest installed version unless a version is chosen.
- **Not part of a song.** Earlier in this conversation I suggested projects could pin a theme. On reflection the default should be **no**: look is a per-user preference, songs and host project files would otherwise depend on art that a collaborator may not have, and D10 already embeds what a song actually needs (panel art). Optional "preferred theme" hint in a project is possible later, advisory only.
- **Per surface.** The standalone app and the plugin editor use the same setting unless the user sets them apart.

## 5. Layering: inherit, override, fall back

Resolution order for any value, last wins:

1. `kabl.base` built-in (always complete; the floor for every key)
2. each `extends` ancestor, farthest first
3. the theme itself
4. user tweaks (a small separate `tweaks.toml`: accent colour, "reduce effects", density if allowed). Never edits the theme file
5. a panel's own art, for that one face, as in section 3

Missing, wrong or partial:

| Situation | Behaviour |
|---|---|
| Key missing | Value from the next layer down. No warning for optional keys |
| Unknown key | Ignored. Reported once in Appearance as "unused" (forward compatibility) |
| Unknown recipe id for a slot | The base recipe for that slot; Appearance shows a warning on the theme |
| Recipe dial out of range | Clamped; warning |
| Only one of light/dark provided | The other mode uses base for that mode. No automatic inversion, which would hide a design error |
| Higher `ui_api` than the app knows | Theme loads with unknown parts ignored and a "written for a newer kabl" notice |
| Cycle or `extends` chain over 4 | Rejected |
| Theme fails validation at start-up | Previous theme, then A. The app always starts |
| Contrast below 4.5:1 for text roles | Warning badge plus "Fix" action. Below 3.0:1: rejected |
| Asset missing or unreadable | That asset is dropped; the procedural fallback is used |

## 6. Trust boundary

Themes will be shared between users, so a theme must be safe to open.

**v1 (declarative) contains no code.** Everything is a number, colour, id from a closed list, or a bounded PNG. Consequences:

- It cannot run anything, read a file, touch the network or the audio thread.
- It cannot hide, move or enlarge a control, or cover content (no layout slots beyond the closed variants).
- Cost is bounded by the recipes' own limits: dials are clamped, each recipe has a cost class, and the global **Reduce effects** switch forces glow, gradients and shadows to their cheap forms. A theme cannot exceed this.
- Text roles must be opaque and legible, so a theme cannot make text invisible.

**Later, custom recipe code, tied to D11.** A theme may one day carry a painter recipe written by its author. That is the same question D11 answers for audio modules (supported ABI, capability and resource limits, state and failure handling), so it should reuse that work rather than make a second sandbox:

- a recipe gets a bounded paint-list API (shapes, text in allowed fonts, declared assets), no I/O, no clocks other than the injected animation time;
- a per-frame time budget per recipe; on overrun, panic or invalid output the recipe is disabled and the base recipe takes over;
- a theme that contains code asks the user for explicit confirmation on install and is marked in Appearance;
- never on the audio thread, and nothing in it can affect sound.

This is explicitly **not part of the first theme build**.

## 7. A and B expressed in the slots

| Slot | A · Satin Studio | B · Lacquer & Light |
|---|---|---|
| Tokens | warm paper / graphite, accent terracotta (light) or amber (dark); radii 4/8/12 | cool grey / blue-black, accent blue (light) or violet-blue (dark); radii 5/10/16 |
| Section palettes | satin tones (light) and deep earth tones (dark) | pastel lacquer (light) and saturated deep lacquer (dark) |
| `face.material` | `satin` (gradient 10 %, grain 0.02, edge) | `lacquer` (gradient 28 %, gloss band 0.24) |
| `face.hardware` | `nuts` | `none` |
| `face.title` | `centered` | `left-dot` |
| `face.out_plate` | `tint` | `dark` |
| `knob` | `cap` + `ticks` + `line` + `arc-dot` | `glass` + `track-arc` + `dot` + `arc-glow` |
| `selector` | `inset-pill` (dark thumb) | `accent-pill` |
| `jack` | `nut` | `glow-ring` |
| `cable` | `rope` | `glow` (emphasised only), light-theme under-stroke |
| `plug` | `cap` | `ring` |
| `display.frame` | `glass` | `neon` |
| `display.trace` | `soft-glow` | `strong-glow` |
| Buttons, chips, cards | rounded, soft shadow | pill, gradient card, glow on primary |
| `chrome.view_switch` | `segmented` | `tabs` |
| `chrome.navigator` | `left` | `left` (a centred `preset-bar` variant is a possible later addition) |
| `browser.rows` | `cards` | `cards` with a glow ring on the selected row |
| Type | Plex Sans SemiBold/Medium | Plex Sans SemiBold caps labels, Plex Mono numbers |

C (Silkscreen) also fits: `flat`, `disc`, `ticks`, `segment`, `donut`, `outlined`, `paper`, `crisp`, `square`, `table`, condensed type.

### 7.1 What the slots cannot express yet

Honest list of things in A or B (or the concepts) that no slot above covers:

1. **Glow is stroke-stacking, not a real blur.** egui has no backdrop blur or shader glow. B's look is approximated; a theme cannot ask for more than the recipe's cap.
2. **Display content.** The envelope's log-time axis, the filter's spectrum bars, the wavetable's perspective stack and the step-lane layout are display behaviour, not style. Only frame, grid, trace and colours are themable.
3. **Per-instance overrides.** A single module face in a different colour from its section (only via panel art today).
4. **Non-rectangular or differently shaped controls** (square knobs, slot sliders), because knob and jack shapes come from the recipe list.
5. **Icon sets.** One stroke set in v1; custom icons would need SVG or glyph support that the current stack does not have.
6. **Layout alternatives beyond the closed variants**: moving the toolbar, a different drawer arrangement, B's centred preset bar (excluded in v1).
7. **Texture or animated backgrounds** beyond a static embedded PNG: no shaders, no per-pixel noise generation, no video.
8. **Per-control animation** (custom easing curves per widget, custom pulse shapes). Only duration and a closed easing list.
9. **Sound or haptic feedback.** Out of scope.
10. **Custom fonts** (v1).
11. **Different state-colour semantics.** Audio/CV/gate colours are themable, but the three-way signal classification is fixed.

## 8. What this means for the build step (tokens + custom widget set)

The next step replaces stock egui widgets with the kit in `stock-widget-inventory.md`. To avoid rework later:

**Must be themeable from the first commit**

1. Every widget and painter reads colours, sizes, radii, shadows and fonts from a `Theme` handle by role. No colour or font literals in widgets, and no direct `egui::Visuals` colours except derived from roles. A repository check (grep or lint) enforces this.
2. The slot list in section 2.2 is the widget kit's interface: each component (knob, jack, cable, face, display frame, button, chip, field, slider, toggle, row, card) is dispatched on its recipe id, even if only two ids exist at first.
3. Built-in A and B are loaded from embedded `theme.toml` through the same loader and validator users will use. A hard-coded theme struct is the thing to avoid.
4. Resolution and fallback from section 5 exist from the start (base, extends, missing keys, startup fallback).
5. Layout constants (face sizes, hit areas, spacing used for hit testing) live outside the theme.
6. Animation takes an injected clock, so captures and recordings stay deterministic (as the prototype does).
7. The contrast validator and the 11 px floor run in CI; the screenshot matrix (`app_target`) runs once per built-in theme.
8. Reduce-effects and reduce-motion switches exist and map to each recipe's cheap form.

**Can come later without rework**

user theme install UI and folder watching; `extends`; user tweaks file; per-theme panel art; extra recipes (adding an id to a slot is additive); embedded fonts; a live theme editor; sharing channel; code recipes (via D11).

## 9. Cost and risk: deep declarative version vs plain tokens

Costs are rough engineering weeks for one engineer who knows the UI crate, **on top of** the shared build estimated in the README (about 6 to 8 weeks).

| Option | Extra over shared | What you get | Main risks |
|---|---|---|---|
| **Tokens only**, A hard-coded | +1 week | One polished look; dark/light | Every later direction is a rewrite of drawing code; no user themes |
| **Tokens only**, A and B hard-coded as branches | +1 and +3 weeks | Two looks | Branches tangle; user themes later mean the refactor below anyway |
| **Deep declarative (recommended)**: recipes + package + loader + validation + Appearance UI, A and B built on it | +1 (A) +3 (B art) +1.5 (recipe slots) +1.5 (package, loader, validation, UI) = about **7 weeks** | A and B as built-ins, user themes, extends, safe sharing | Slot design lock-in; QA matrix grows; effect cost; long tail of "my theme looks wrong" support |
| Deep + code recipes | + D11-dependent, not estimated | Anything paintable | Sandbox, performance and trust; only after D11 |

Risk handling:

- **Lock-in:** slot vocabulary is versioned by `ui_api`; new recipes are additive, removing one is a major version.
- **QA:** the matrix is mechanical (the example already loops direction x size x mode); each built-in theme is a gate, user themes are not.
- **Performance:** every recipe declares a cost class; B's glow is the first one to profile on the laptop; Reduce effects is the escape hatch.
- **Scope creep into layout:** closed variants only; geometry stays out of themes.
- **Discipline:** if A and B do not both load through the public format with no private hooks, the format is not expressive enough. That is the acceptance test of the first theme build.

**Recommendation:** build the deep declarative version, in this order: widget kit with recipe dispatch and A as an embedded theme; loader and validator (A now loads through them); B as the second embedded theme, which proves the slots; then install UI and user themes. Defer fonts, per-theme panel art and code recipes. The extra cost over hard-coding A and B is about 3 weeks, and it removes a rewrite.

## 10. Decisions for Kosta

1. Deep declarative version (recommended), or tokens only?
2. A as default (assumed), B as second built-in?
3. Theme is a user preference and not saved into songs (recommended), or may a project carry a preferred theme?
4. Contrast policy: warn at 3.0 to 4.5 and reject below 3.0 (recommended), or reject below 4.5?
5. Custom fonts in themes: not in v1 (recommended)?
6. Layout variants (view tabs, preset bar, table rows): allowed as closed choices (recommended), or stay out of themes entirely?
7. Sharing: file import only in v1, or a theme library like composites?
8. Which tweaks may users make on top of a theme (accent colour, reduce effects; density)?
9. Should custom recipe code, when it comes, reuse the D11 sandbox (recommended), and be gated on D11's findings?
10. Does the plugin editor follow the standalone theme setting?

## 11. Repository weight

The first delivery added 84 full-size PNG captures (about 43 MB). They have been re-encoded as lossy WebP (quality 92; I compared one full-size frame against its PNG, and the other 83 were not individually compared) and the PNGs removed from the tree: **43 MB to about 8 MB**, plus about 3 MB of recordings. The captures are reproducible losslessly with the commands in the README.

What this does not do: the original PNG commit stays in the history of the PR branch. Options:

- **Squash-merge PR #16** (recommended): master then receives only the final tree, about 11 MB. No history rewrite needed.
- Rewrite the branch to drop the early commit: needs a force-push, which I have not done and will not without explicit approval.
- Keep only a curated subset in the repository (for example the 1440x900 set) and attach the rest to a GitHub release, regenerating on demand. A reasonable rule for future design batches: media over about 5 MB goes to a release asset, not the tree.

## 12. Verification and limits

- This is a document. No theme system, loader or package format exists, and nothing was built or run against it.
- The slot list is derived from what the prototype actually branches on (`crates/ui/examples/app_target`), so A, B and C are expressible by construction. Whether *user-made* themes stay coherent with the closed recipe set is not tested.
- Cost figures are estimates, not measurements. B's glow cost on the target laptop is unmeasured.
- The interaction with D10 is read from `docs/panel-authoring/design.md` and `skin.rs`; I did not run the D10 authoring flow.
- `ui_api`, the package layout and the bounds above are proposals and need Kosta's decisions (section 10) before a build brief is written.
