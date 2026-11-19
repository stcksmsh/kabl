# Kabl AV: mockup brief

Part of the AV package: [audit](KABL_AV_PRODUCT_AND_ARCHITECTURE_AUDIT.md) · [decisions](KABL_AV_DECISIONS_AND_OPEN_QUESTIONS.md) · [roadmap](KABL_AV_ROADMAP_AND_SCOPE_GATES.md).

The earlier concept preview is mood only. Every item marked **H** below is a hypothesis, not a decision. Build on the real kabl look
(graphite/paper themes, `docs/design/`), not on the preview's labels or numbers. No invented specs, ratings or feature claims.

## 1. Principles and the distinguishing workflow

- Serious instrument: legible, long-session, precise. Studio chrome is calm; the visual output may be expressive.
- Sound first: a useful result before a lesson (`PRODUCT-PLAN.md` §1). Explain the patch as it actually is.
- One patch, two projections. Same ids, same connections; layout differs.
- Domain honesty: audio, control and event connections look different; conversions are visible.
- **Distinguishing workflow (H):** play a note → see which module made the event → watch it drive one visual parameter, with the whole path inspectable.

## 2. Real vs hypothetical (so nothing misleads)

| Real today | Hypothesis (label on the mockup) |
|---|---|
| Rack, Perform, Sounds scenes; routing drawer; module faces and live displays; cable colours by type; composites; macros; signal probe | Node view; event and visual ports; converter chips; visual preview panel; fullscreen output; bridge health readout; domain badges on modules |

## 3. Screens (the six mockups)

Each: content hierarchy, key interactions, state to show.

1. **Studio, rack view.** Real audio patch (e.g. a factory bass + sequence). Left: module browser; centre: rack; right: inspector; bottom: transport + macros.
   Show a selected cable with its probe. Interaction: drag from browser, patch, Tab to node view. (Existing chrome; low risk.)
2. **Same patch, node view (H).** Identical modules, same selection highlight, auto-laid-out left-to-right; ports typed by colour and shape; a small "same patch"
   indicator and a toggle that preserves selection. Show that positions differ from the rack. First release likely read-only (D-02): show a lock affordance.
3. **Cross-domain module detail (H): ADSR.** One card: front panel (A D S R knobs) in the middle; below, three interface groups: Audio/CV out (level), Events out
   (attack, decay, sustain, release stage edges), Visual-readable (level, stage). One parameter set, no duplicate; a footer line "one module, shared parameters". Show an
   unavailable-domain state (visual runtime off → greyed group, module still valid).
4. **Audiovisual performance workspace (H).** Perform view plus a visual preview panel (resizable, dockable right or top), meters, bridge health (events/s, dropped, latency),
   a "Fullscreen on display N" control. Question: how much width must the preview take at 1280x800 (Q5).
5. **Fullscreen generated output (H).** An original abstract composition driven by the patch (not an FFT bars visualizer): layered fields and shapes whose envelope, phase and
   step events are legible when watched against a known patch. Provide a stills set and a short clip with the corresponding patch notes.
6. **Discovery / learning state.** Module browser searching by intent ("make it pulse", "soften the top"), preview-by-sound, "what does this do" card using `ModuleInfo.explain`, and
   "start from a working patch" templates. Real, buildable now; the "preview" button depends on the Phase 6 audible-preview work.

Extra states worth one frame each: silence diagnosis ("nothing is reaching Out: here's where the signal stops"), missing module/resource, visual runtime stalled banner.

## 4. Interaction rules

- **Rack:** keep the existing rack feel; ports are jacks; cables are physical. Do not turn it into a node editor.
- **Node:** nodes with typed ports left/right, edge routing, collapse/expand, minimap; dense audio detail collapses to a summary node.
- **Shared:** selection, undo (one project undo), search, inspector, cable inspection, rename.
- **View-specific:** layout, zoom, grouping, collapsed state.
- **Port language (H):** audio = thick solid, control = thin solid, event = dotted with pulse marker, visual resource = distinct shape (square), each with colour **and** shape/pattern
  for accessibility. Converters render as a small chip on the cable.
- **Easier in rack:** performing, fast patching of audio. **Easier in node:** following long chains and cross-domain logic. Communicate with a short hint on first switch, not a manual.

## 5. Example patch narrative

Sequencer clock → step events + notes; osc → filter → VCA ← ADSR (gate from sequencer). ADSR level (scalar) → visual "field brightness"; ADSR attack event → "pulse
ring spawn"; oscillator phase → "rotation"; macro "Bloom" → field scale and filter cutoff together. Output: audio out + visual out. Same story in rack and node.

## 6. Open design questions (flag them on the board)

Q1 single integrated screen vs docking? (H: integrated, docked preview.) Q2 switch scope global vs per selection. Q3 how multi-domain modules show in rack (badges vs extra jack
rows). Q4 clutter budget for port types. Q5 minimum space for rack + node + preview at 1280x800. Q6 fullscreen/projector control without losing the editor.
Q7 performance workflow: macros vs scenes vs snapshots vs MIDI map. Q8 what a saved project contains and how missing resources read. Q9 inspector vs on-module information.
Q10 first-use vs advanced exposure.

## 7. Sequence and review checkpoints

1. Mockup 1, 6 (low risk, mostly real). Checkpoint: owner picks the browser/inspector shape.
2. Mockup 2, 3 (data-model dependent). Checkpoint: confirm D-02, D-03, D-06 directions.
3. Mockup 4, 5. Checkpoint: after spike S4 gives frame-rate and space facts.
Static first; a click-through only for the rack↔node switch and the ADSR detail.
