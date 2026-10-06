# Composite panel authoring

Current owner disposition: **changes requested** after hands-on use. The [hands-on repair package](repair/README.md) and [repair report](repair/REPORT.md) supersede interaction/theme and normal-launch claims below. Earlier evidence and owner bundle remain preserved.

Give a D09 composite a portable instrument face while preserving its real controls and routing. Open Composites, expand the instance tools, and choose Edit face; default cards and authored faces also have Edit face. Import separate light/dark PNGs, set rack width, and place exposed IDs as knobs, selectors or jacks. Coordinates and sizes describe the complete printed footprint, not just the knob cap.

Edit labels, then Apply panel / Undoable. Validation reports overlap, dimensions, asset limits or a wrong binding without changing the project. Preview light/dark switches the applied face. Reload saved face discards draft changes; Use default face is Undoable. Inspect internals always reaches the real modules and routes. More controls/help reaches every exposed parameter even when artwork is missing or a control is unplaced.

Export complete package writes a new local JSON file containing the graph, interface, help, panel metadata and both images. Composites → Import package file loads it as a fresh independent instance. No original image paths are required. Publish a new immutable version uses the existing library mechanism; publishing never updates saved songs or existing instances. D09 duplicate rules still omit external cables and controller/native assignments while retaining fresh private state and pin positions.

The voice and stereo-effect examples in examples/ were authored through production GUI controls using scripted X11 input. Their original SVG/PNG artwork is in examples/art/. Reproduce artwork with `python3 docs/panel-authoring/scripts/art.py`, build the app/example with `cargo build -p kabl-ui --bin kabl-ui --example panel_seed`, then run scripts/workflow.py and scripts/capture.py in a private X11 environment. The capture scripts own only their private display/profile and never automate the owner's desktop.

The curated real release walkthrough is [media/walkthrough.mp4](media/walkthrough.mp4), recorded with scripted private X11 input; it demonstrates theme changes, Design preview, label Apply, chronological Undo and public fallback. Reproduce it with scripts/walkthrough.py and the owner bundle path.

See design.md for identity/state/asset contracts, REPORT.md for exact heads and acceptance dispositions, REVIEW.md for independent findings and CHECKLIST.md for owner checks. Evidence is real application output, not the earlier design mockups. Engineering verification, merge and owner approval remain separate.

## Licenses and provenance

Original sectional art, procedural rendering and example SVGs: MIT OR Apache-2.0, matching the repository. IBM Plex Sans/Mono: OFL-1.1, reserved name Plex; complete notice in crates/ui/assets/FONT-LICENSE.txt. Vendored nice-plug remains ISC; no vendor change is intended. Reference images and the generated material-study render were not imported into the app or bundle. The prior selected design package remains the finish reference at the task's premium-v3 directory.

Hardware/listening/physical latency/feel and production-beta checks remain pending. This batch does not resolve D08 ordinary-render nondeterminism or dense-editor backend/performance limits. Native X11 scaling is a software backend exercise, not a physical monitor or owner interaction proof.
