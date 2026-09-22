# Handoff prompt: arbiter agent

Paste everything below the line into the arbiter. It is self-contained; the repo holds the detail.

---

Project: **kabl**, repo `/secondary/Programming/Github/kabl`, branch `master`.

## What kabl is

kabl is a modular synthesizer in Rust. Its pitch: cables are instruments, patches explain
themselves, and modules can be built like guitar pedals. The goal is **not a toy**. kabl should
eventually make anything Surge XT or Vital can, while being simpler to learn, start with and
create in, rather than a player of presets. The long-term musical target is Tangerine Dream in
spirit (the Encore / Ricochet / Force Majeure era): interlocking sequences, evolving timbre,
atmosphere, and performed leads.

## Where things are (have the worker read these first)

- `docs/STATUS.md`: the top "Current handover" block is authoritative. Older sections below it
  are partly stale, and the block names which parts.
- `docs/decisions.md`: append-only rationale. The G1 entries and the three latest entries
  (revision round 2; revision 2 review: owner feedback) hold Kosta's design decisions.
- `docs/PLAN.md`: the design plan that was executed (P1–P5, then gate G1).
- `docs/design/revision-2/REVIEW.md`: the **current** design package. Its companions:
  - `INTERACTIONS.md`: the current interaction spec. It overrides `docs/design/INTERACTIONS.md`.
    Every rule is tagged [owner], [rec] or [open].
  - `render2.py`: the mockups. Run `python3 docs/design/revision-2/render2.py`, or add `--qa`.
- `docs/design/REVIEW.md`, `BRIEF.md`, `QA.md`, `render.py`: revision 1. Kept as history and
  as the base layout model.

## State of the build (trust code and git over docs)

- Engine: flat-schedule compiler with cycles, buffer reuse and carry-over on recompile.
  `process_block` does not allocate. Swaps crossfade and can overlap.
- Nine built-in modules.
- `kabl-standalone` has been played live with a real MIDI keyboard.
- `kabl-ui` (egui) has a Patchbay view and a rough Eurorack view v1. It has never been run with
  real audio.
- The new rack design exists only as mockups.
- Kosta's machine has real audio, a display and a MIDI controller. Pi-4 performance is
  unmeasured.

## Design decisions (Kosta, at G1 and the revision-2 review)

- **Look:** direction A (warm studio hardware) for core modules, plus A-dark: A's layout with
  B's material (texture, knurled metal knobs, restrained glow).
- **Skins:** illustrated light/dark art for user-made and non-core modules. The UI draws every
  control, label and value. The skin flag `labels_on_art` is **default false** (labels on theme
  plates); a skin maker may set it to true, putting labels straight on the art for beauty.
- **Modulation:** any knob accepts a modulation cable; there is no CV jack per param. Amount,
  polarity and bypass belong to the route. The cable, drawer row, badge and knob ring are views
  of the same route.
- **Density:** modules declare primary/advanced params. The user chooses the primary ones.
  Advanced params appear by expanding the module in place.
- **Cables:** one rack layout, shown All / Focus / Hidden.
- **Revision 2 accepted as the direction** ("all the other things feel right"): the knob
  anatomy (pointer = base, ring = result, peak dot, depth lanes when inspected); amount as a %
  of knob travel, also shown in units, summed then clamped; the plug at 6 o'clock as a thinner
  mod lead; the face = primary controls, with expansion appended right and jacks always on the
  face; the rich LFO (features marked ◆ are conceptual, not in the engine); a 1440×900
  default viewport with 1280×800 as the minimum.

## Open questions (settle in the prototype)

- Hidden-mode layout: keep the same rack, or switch to a compact synth layout? Build both.
- Expansion: push the neighbours (shown) or float over them? Auto-Focus while expanded?
- Ring vs knob-body drag at a small knob radius.
- Envelope-time modulation: continuous vs sampled at stage start (choose by ear).
- **Prototype go-ahead has not been stated explicitly. Confirm it with Kosta before starting.**

## Known code gaps behind the design

- The compiler passes params as constants. `ProcessIo::param` already returns `Signal`
  (Scalar/Buffer), so param modulation is mostly compiler plumbing. Voice-rate vs global-rate and
  control-rate need care.
- `CableState.params` exists but the compiler ignores it, so there is no per-cable amount yet.
- Fan-in: only the first cable into an input is used.
- The op log has no undo grouping, so replace or repatch takes two undo steps.
- Risks found by reading the code but never reproduced:
  - MIDI reaches only the active graph during swaps.
  - The UI never calls its deferred-drop collector.
  - MIDI routing assumes module ID 1.
  - Layout edits rebuild the audio.

## Candidate next jobs (recommended order; confirm with Kosta before implementation)

1. ~~Design revision round~~: done and reviewed (revision 2).
2. **Interactive prototype** of the revision-2 design, isolated from the engine: rack with
   A/A-dark; knob drop, ring and body drag; route card and inspector; All/Focus/Hidden; both
   hidden-mode layouts; expand/collapse and choosing primary controls. It has to answer the
   open questions above. Start only after Kosta's go-ahead.
3. **Engine groundwork for param modulation:** knob routes with per-route amount/sign/bypass,
   summed then clamped, and tests. Independent of job 2.
4. **Correctness prerequisites** for any playable integration: MIDI during swaps, the
   deferred-drop collector, MIDI routing that assumes module ID 1, undo op grouping.
5. Deferred until Kosta prioritizes them: the LFO features marked ◆, clock and sequencers,
   more modulation sources, real skin art (image generation; prompts are in
   `docs/design/IMAGEGEN_PROMPTS.md`).

## Rules for delegated jobs

- Commit small, meaningful changes directly to `master`. No branches, no PRs. Pushing needs
  Kosta's request.
- AIW and Recall are not in use. Track progress in `docs/STATUS.md` (overwritten) and
  `docs/decisions.md` (append-only).
- Preserve unrelated files, including the untracked `.ai/`.
- Be honest about status: a concept is never presented as built, measurements are reported
  as they are, and anything unverified is labelled.
- Kosta prefers terse, token-lean replies and minimal, non-speculative code.
- A worker that ends a job updates the STATUS handover block, so the next session can start
  cold.
