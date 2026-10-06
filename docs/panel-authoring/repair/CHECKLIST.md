# Kosta's repaired UI checks

## Final retest priorities

Owner disposition is **changes requested** until Kosta reviews these fixes. Use the new d10-final-owner-test bundle and compare [real scripted video](final/media/final-interactions.mp4); previous working projects and libraries are untouched.

- Open native More and Keyboard / exact value entry. Confirm the window grows and shrinks in place when content fits, with readable labels and original exact-entry values.
- Use a tall module and smaller/native-scaled viewport. Confirm only genuinely overflowing content scrolls, with Close fixed and the last setting reachable.
- Wheel Default voice #3 through its top/bottom boundaries. Confirm only inner contents move; rack, parent and camera stay still. Check authored Public controls and nested menus too.
- Wheel outside the child on uncovered rack. Confirm ordinary pan and modifier zoom still work, alongside existing control/keyboard gestures.
- Drag over occupied panels and between neighbours. Confirm the visible insertion rectangle sits above stationary panels/cables and below the lift, and exactly matches final position/width. Check Escape, edge scrolling and one Undo per drop.

The preset/sequence bank, searchable Sequences category and Perform setups remain a separate follow-up. Hardware/listening/latency/feel and inherited render/performance limits remain pending; engineering checks do not approve owner interaction quality.

Owner disposition is **changes requested**. These boxes remain unchecked until Kosta reports hands-on results.

- [ ] Run the new normal launch.sh. Confirm Sounds has factory presets; confirm your own library edits remain available. Keep the previous bundle/project intact.
- [ ] Select and drag native modules, Default voice, Subtractive Voice and Stereo Chorus Room by title and unused body. Cross occupied neighbours and rows. Check the exact grab, lifted copy, placeholder, legal preview and final drop.
- [ ] Cancel a move with Escape; complete a move and Undo once. Confirm composite internals, graph routes and automation targets stay intact. Drag near the bottom/side edge to reach another row.
- [ ] Turn knobs, click selectors and patch/unplug jacks without moving a face or panning the rack. Try zoom and your real monitor scale.
- [ ] Inspect internals, then the nested oscillator. Confirm breadcrumb, own internals and boundary bindings. Back twice should restore the outer camera and selection. Check outside cue/clock explanations in a patch that uses them.
- [ ] Open More controls; inspect cable context and all advanced settings, including keyboard/exact value entry. Close by Escape and outside click. Confirm neighbours, zoom and underlying values stay fixed.
- [ ] Open native Edit face, hide/show controls, reorder within knob/selector rows and Restore defaults. Apply, Undo/Redo, save, quit fully and restart. Confirm hidden settings remain in More.
- [ ] Load the new private REAPER project, save and recall. Check your native lanes, pins and controller mappings retain their targets after visibility/order changes.
- [ ] Compare light/dark at 1440×900 and 1280×800: warm pale light workspace/drawers/dialogs versus charcoal dark surfaces, readable sectional faces, controls and cables. Report material/interaction judgments separately from engineering results.
- [ ] Use launch-portability-test.sh for the explicit unavailable-library check; it is separate from normal factory-enabled startup.
- [ ] Play and listen using physical hardware. Record feel, latency and xruns; software captures and scripted host replay do not close these checks.

No merge, D11 launch or other-chat messages are authorized. Ordinary-render nondeterminism and dense-editor/backend limits remain open.
