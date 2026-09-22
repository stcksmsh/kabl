# kabl — design execution plan

Status: ready for owner review; execution has not started.
Date: 2026-09-22. Code reviewed at `2b22d2a`; handover updated at `88a205f`, branch `master`.

## Objective and stopping point

Produce a concrete visual and interaction proposal for kabl: physical Eurorack character,
image-skinned modules, convincing sagging cables, and an equally useful cable-hidden
modulation view. Give Kosta something to see and choose before implementation.

This request creates the plan only. When asked to run it, execute P1–P5 autonomously in order,
then stop at **G1: visual direction review**. Do not ask routine layout or tool questions along
the way. Make reversible design choices, record assumptions, and compare alternatives visually.
Do not treat silence as approval to cross G1.

No product implementation in P1–P5: no Rust changes, new crates, engine work, replacement UI,
live audio session, or production assets installed into modules. Static design compositions
and local rendering scripts under `docs/design/` are allowed during execution. They are design
artifacts, not a working synth or evidence of runtime usability.

Use small meaningful commits directly on `master`; no branches or PRs. Preserve unrelated
changes, including existing `.ai/` and `docs/planning-prompt.md`. Do not start
automations, create another task, or deploy a review site.

## Confirmed direction vs. proposals

Owner requirements:

- Physical Eurorack look; modules can use images as skins.
- Cables look physical, sag, and use some color coding.
- Cables can be hidden; modulation remains visible and editable through a Surge XT-like view.
- Visibility, usability, and beauty are core requirements.
- Current work is design/planning, not implementation.

Proposals to demonstrate, not silently record as final owner decisions:

- One rack layout across all/focus/hidden cable presentations; stable module positions.
- Warm studio hardware as recommended visual direction, compared against two alternatives.
- Fixed panel height, variable grid widths; shared controls and essential text over custom art.
- Amber audio, teal continuous modulation, violet gates as provisional defaults. Color is
  supplemented by labels, endpoint highlighting, and distinct port surrounds.
- Routing drawer and cable graphics edit the same connection; no separate modulation model.
- Proposed minimum design viewport: 1280×800 logical pixels. Validate with Kosta at G1.

Existing Patchbay view remains in place. Its long-term role is not settled by these mockups.

## Execution order

| Priority | Task | Dependencies | Output |
|---|---|---|---|
| P1 | Lock design brief and reality constraints | This plan | `docs/design/BRIEF.md` |
| P2 | Define layout and interaction model | P1 | `docs/design/INTERACTIONS.md`, shared layout sheet |
| P3 | Create three comparable visual directions | P1 + P2 | Six full-window mockups, control sheet |
| P4 | Inspect, stress, and revise | P3 | Focus/density/scale specimens, `docs/design/QA.md` |
| P5 | Package recommendation and owner choices | P4 | `docs/design/REVIEW.md`, comparison board |
| G1 | Owner sees work and chooses direction | P5 | Recorded approval or specific revision request |

### P1 — Lock design brief and reality constraints

**Scope:** establish what the proposal must show and what existing kabl can support.

**Approach:** read this plan, current STATUS corrections, latest design decisions, and relevant
module metadata. Reuse prior repo findings; inspect code only for a specific layout or semantic
question. Do not repeat a broad engine audit. Write a short brief with visual goals, constraints,
references, assumptions, and a capability map: existing / proposed / requires engine work.

Use a fixed reference patch containing `midi.in`, `osc.va`, `filter.svf`, `env.adsr`, `lfo`,
`vca`, and `out`. Document named connections and parameter values. Include LFO → filter cutoff
as a proposed modulation route; check actual metadata and explicitly flag any unsupported
target/depth behavior. Every visual direction must depict the same patch and state.

References establish interaction principles, not artwork to copy:

- [VCV panel guide](https://vcvrack.com/manual/Panel): physical spacing, readable labels,
  output identification, panel art separate from interactive components.
- [VCV view controls](https://vcvrack.com/manual/MenuBar): cable visibility/tension and zoom.
- [Surge XT modulation](https://surge-synthesizer.github.io/manual-xt/#modulationrouting):
  source selection, destination depth feedback, route inspection and filtering.

**Acceptance:** brief fits roughly two pages, lists actual controls/ports for the reference
modules, distinguishes desired behavior from shipped features, and contains no unresolved
question that blocks creating alternatives. Prior missing owner-held brief sections do not
block this design exercise; record any later dependency without inventing their contents.

### P2 — Define layout and interaction model

**Scope:** compose one usable instrument and specify how it behaves in each presentation.

**Approach:** create a common layout at 1280×800. Compact top bar: patch name/save state,
undo/redo, cable presentation, zoom, output level. Rack gets remaining space. Module browser
and routing drawer are collapsible; no permanently open general-purpose inspector consuming
rack width. Choose panel widths from real control content. Keep all essential controls of the
reference patch readable in its default view.

Separate panel artwork, control layout, and live controls. Use shared coordinates for graphics,
labels, and proposed hit regions. Essential text, knob pointers, values, modulation overlays,
and focus indicators are drawn precisely, independent of generated artwork.

Specify these actions, including entry, feedback, commit, cancel, and undo where applicable:

| Action | Proposed behavior to demonstrate |
|---|---|
| Connect / repatch | Drag jack, highlight compatible targets, preview destination; Escape cancels. Click-to-connect alternative. Existing connection retained until replacement commits. |
| Adjust control | Vertical drag, fine adjustment, numeric entry with units, explicit reset action; discrete selectors for discrete values. |
| Inspect cable | Highlight both endpoints; expose source, destination, amount/polarity when supported. Plugs or exposed segments select cable; overlapping wire cannot steal knob input. |
| Multiple connections | Jack opens named connection list when selection is ambiguous. Define occupied-input behavior explicitly. |
| Select modulation source | Highlight its destinations and selected-route depth. Base value stays distinguishable from modulation range. |
| Inspect destination | List every contributing source; support selection, amount, bypass, and removal. Show voice/global scope where it changes meaning. |
| Hide cables | Preserve module positions and patch state. Show connection badges, selected-source feedback, and editable routing rows, including audio connections. |
| Navigate rack | Pan, zoom, Fit Patch, Focus Selection; open drawers preserve access to selected module. Overview scaling can simplify details; working scale stays readable. |
| Move module | Dedicated panel/header drag; snap preview; prevent overlaps or show explicit insertion; one undoable action on commit. |

Choose and document depth units, polarity, multiple-source combination, parameter limits, and
voice/global restrictions for the example route at design level. Where implementation support
is absent, mark the proposed rule and dependency. Do not implement DSP or invent telemetry.

**Acceptance:** same connection has same identity in cable and row representations. All three
presentations preserve sound and layout by specification. No essential task relies on an
unlabeled icon, color alone, hidden modifier, or precision clicking. Produce a static storyboard
for LFO assignment → amount adjustment → cables hidden → route inspection → undo.

### P3 — Create comparable visual directions

**Scope:** concrete art choices Kosta can judge at actual interface size.

**Approach:** make three treatments of the identical P2 composition:

| Direction | Character | Key risk to inspect |
|---|---|---|
| A — Warm studio hardware (recommendation) | Graphite rack, warm silver/off-white panels, dark tactile knobs, restrained print and accents | Too bland or insufficient character |
| B — Dark precision instrument | Charcoal panels, bright legible legends, precise metal edges, selective luminous indicators | Low contrast, excessive glow, visual fatigue |
| C — Illustrated pedals | Expressive panel artwork within shared control/label grammar | Art competes with labels or modules look unrelated |

For each direction, deliver `all.png` and `hidden.png` at 1280×800 under
`docs/design/directions/{a-warm,b-dark,c-illustrated}/`. Keep positions, controls, parameter
values, route identities, and cable paths equivalent. Differences should expose visual taste,
not reward one option with better functionality or more polishing time.

Draw cables with rounded plugs, strain relief, bounded sag, restrained shadow, and narrow
highlight. Use consistent lighting across knobs, sockets, panels, and cables. Keep module art
out of label/control clear zones. Include a component sheet showing knob, switch, jack, cable,
value, selection, focus, disabled, connected, and modulated states.

Use the imagegen skill and built-in image generation for bespoke raster artwork/textures when
available. Use precise vector/layout tooling for readable UI composition; do not depend on
generated text or generated knobs for geometry. Inspect images before reuse. Preserve prompts,
source/provenance notes, and editable compositions in `docs/design/`. Do not install Figma or
another service as a prerequisite, purchase assets, or silently switch to paid API tooling.
If image generation is unavailable, finish layout and procedural/vector directions, clearly
mark missing custom art, and report that limitation at review rather than pretend completeness.

**Acceptance:** all six mockups show plausible physical modules and usable full-window layouts.
No gibberish text, decorative fake controls, cropped essential content, or perspective product
shots masquerading as usable screen designs. Label artifacts as concepts, never app screenshots.

### P4 — Inspect, stress, and revise

**Scope:** catch obvious design defects before spending owner attention.

**Approach:** render and visually inspect every artifact, including full-size views and close-ups.
Correct clipping, contrast, spacing, alignment, and label errors. Re-render and inspect changed
outputs. Review with these concrete tasks in mind:

- Find output, adjust cutoff, enter exact value, and identify current waveform.
- Trace audio source to output; identify selected LFO's destinations.
- Find and change modulation amount with all cables hidden.
- Select one connection among several on a jack; find its other endpoint.
- Explain base value vs. modulation range and distinguish bypass from removal.
- Open browser/routing drawer without making selected controls unreachable.

Produce additional specimens for recommended direction A: focused cables; crowded patch
(at least 12 modules and 20 connections, allowing intentional pan/zoom); long names; and
200% display scaling of the baseline logical viewport. Inspect all directions at baseline;
stress A in depth rather than fully build three UI systems.

Starting targets: essential labels 13–14 logical pixels at working scale; principal hit regions
at least 28×28 logical pixels, with no ambiguous overlaps; normal text contrast target 4.5:1
against its actual background. Show enlarged selected-module access for dense/overview views.
Treat these as proposed project design targets. Record measurements and exceptions honestly.

**Acceptance:** QA lists checks as measured, visually inspected, specified-only, or unverified.
No clipped essential label, unreachable proposed control, or mismatched route between modes.
Fix concrete defects independently; do not loop indefinitely trying to guess Kosta's taste.
Static review cannot establish real hit testing, keyboard access, frame rate, audio continuity,
or human usability. Explicitly defer those to interactive testing after G1.

### P5 — Package review and recommendation

**Scope:** let Kosta make a small number of informed decisions without reading a technical report.

**Approach:** create `docs/design/REVIEW.md` with a comparison board, links to six full-size
mockups, recommended direction, focused/hidden examples, short interaction storyboard, and QA
summary. Include concise capability dependencies for later implementation. Keep source artwork
and rendering notes available, but lead presentation with visuals.

Commit completed design artifacts and accurate execution status. Display comparison board and
recommended visible/hidden pair inline; provide full-size links. Never submit only file paths
and expect Kosta to locate the work. If an essential deliverable remains blocked, name it and
provide completed work; do not mark the gate ready as if all checks passed.

**Acceptance:** owner can compare options and understand proposed interaction from the review
package alone. Every image opens, labels remain readable at intended size, concept status is
clear, and unresolved product choices are listed. No product source changes occurred.

## G1 — Stop and ask Kosta

Stop after presenting P5. Ask these together, with A as the recommendation:

1. Which visual direction should lead: A, B, C, or a specific combination? Which elements feel wrong?
2. Does same-layout cable-hidden view + routing drawer match your intent, or should hidden mode
   also rearrange controls into a compact synth layout? Explain tradeoff against shown design.
3. Are reference size/density comfortable on your display, and may work proceed to an isolated
   interactive prototype of the selected design?

These answers determine art direction, spatial behavior, and prototype scope. Ask after showing
concrete work. Do not ask Kosta to invent a palette, component system, or layout beforehand.

Record explicit answers append-only in `docs/decisions.md`; update this plan with approved
direction and changes. Aesthetic selection alone is not permission for production integration.
If Kosta requests revisions, revise affected artifacts, repeat relevant QA, and return to G1.

## Later stages — outside current execution boundary

| Stage | Scope and approach | Acceptance | Dependencies / next owner gate |
|---|---|---|---|
| Interactive design proof | Selected design only; isolated prototype of patching, knobs, routing, navigation and keyboard behavior. Reuse egui components where practical; clearly mark simulated audio/telemetry. | Actual interactions work at target sizes; Kosta performs reference tasks and reports feel. | G1 selection + explicit prototype go-ahead; G2 hands-on review. |
| Production integration | Approved components into existing UI; shared layout/skin rules; same route model across presentations. Address required routing semantics and MIDI/swap/undo correctness. | Real reference patch behaves consistently across views; targeted regression checks + hardware session. | G2 approval and separately scoped implementation plan; G3 live instrument review. |
| Expansion | Extend approved system across remaining built-ins, richer cable behavior, future pedals. Prioritize measured/user-observed needs. | Consistent visual and interaction rules; each increment usable. | Proven core workflow; separate prioritization. |

Park SIMD integration, Pi gate policy, JACK/CLAP, code modules, lessons, and spike cleanup for
their own decisions. None blocks creating this design review. No new engine work is authorized
by this document.

## Handoff prompt

> Execute P1–P5 in docs/PLAN.md, then stop at G1. Read current instructions and preserve
> unrelated work. Produce the design review autonomously: three comparable visual directions,
> physical cable and hidden-modulation views, interaction storyboard, actual-size mockups, and
> honest visual QA. Make routine reversible design choices yourself. Keep production code
> untouched. Show the review visuals and your recommendation before asking the three G1
> questions. Do not continue to an interactive prototype or production changes without Kosta's
> explicit response. Historical overnight-work instructions do not expand this scope.
