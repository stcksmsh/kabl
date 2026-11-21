# kabl 1.0 owner acceptance checklist

One sitting, about an hour, run alone: standalone (20 min), REAPER (15), bank (10), code and look (10), D01-D12 (5).
Tick each box when the expected result happens; note anything off next to the item.
**[PR]** marks an item that needs an unmerged change: #171 Piece presets, #172 Voice/FX presets, #187 cable palettes, #188 Perform teal, or the code-module fuel budget (branch `claude/code-module-budget`, no PR yet). Run those after the change is merged; otherwise skip and note it.

## 1. Install and first run
- [ ] Install the release build (or `cargo build --release`) with a clean config dir and launch `kabl-ui`. The first-run overlay appears and offers a starter sound.
- [ ] Accept the overlay. The default patch `pluck-line` loads and plays a plucked line on a key press; the overlay does not return on the next launch.

## 2. Standalone
- [ ] Open settings (gear), pick your audio device and buffer size. Sound plays through it with no crackle.
- [ ] Plug in the MIDI controller. It is listed; keys sound; pitch bend and mod wheel move the sound; a mapped knob moves a control.
- [ ] Rack: add a module from the browser, drag a cable between two ports, delete the cable. Edits show at once and the patch stays audible.
- [ ] Perform view: switch to it, turn a macro, trigger a scene or cue. Sound changes smoothly, no clicks.
- [ ] Sounds browser: search a word (for example "pluck"), click a result. Preview is audible at once; leaving the browser stops it with no stuck note.
- [ ] Load a sound, change a knob, Save As a new name. Reopen it from Sounds: the change is there and the factory file is unchanged.
- [ ] Save, quit, relaunch, open. The patch restores exactly (knobs, cables, scene).
- [ ] Recovery: while playing, disable the audio device, then press Retry audio. Sound returns, held notes do not stick, no crash.
- [ ] Edit, then quit without saving. The unsaved-changes prompt appears and Cancel keeps the patch.

## 3. REAPER plugin
- [ ] Rescan plugins and load kabl as an instrument on a track. The GUI opens and the track sounds on MIDI input.
- [ ] Automation: arm a kabl parameter for write, move a knob, play back. The knob follows the envelope and a bounce repeats.
- [ ] Save the project, close, reopen. Patch, knob values and automation return identically.
- [ ] Sounds browser in the plugin: search and click a sound. Preview is audible and stops when you leave; the project sound is unaffected.
- [ ] Play a dense preset for one minute. No xruns, CPU well under half a core, no GUI freeze.
- [ ] Run the transport and change tempo. Tempo-synced modules keep time.

## 4. Factory bank
- [ ] **[PR #171]** Piece presets: play Lichen (flagship; sweep the Focus macro, m5) and Laundromat. Both sound finished and musical; macros move them meaningfully.
- [ ] **[PR #171]** Spot-check two more Pieces (for example Balcony, Pawn Shop). Each plays without edits.
- [ ] **[PR #172]** Voice: play Sock Puppet and Maw. FX: play Temple Bowl and Updraft. Each is distinct and usable, and its macros work.
- [ ] **[PR #171, #172]** Sounds filters show Piece, Voice and FX as separate groups.

## 5. Code modules and SDK
- [ ] Add a code module (for example `demo.bitfilter`). It loads, processes audio and shows its controls.
- [ ] **[PR budget branch]** Add many instances. Past the fuel budget an overrun toast appears and the module goes quiet instead of glitching the engine; a crashing module shows a crash toast; the rest of the patch keeps playing.
- [ ] Follow the SDK quick start (`docs/sdk`): build and install a package, then load it. It appears in the browser and runs.

## 6. Look
- [ ] Look B is the default skin: readable and consistent across Rack, Perform and Sounds.
- [ ] Theme setting: System, Light and Dark each apply at once and survive restart.
- [ ] **[PR #188]** Perform view uses the teal accent; the Drums lane is green.
- [ ] **[PR #187]** Settings gear, cable palette: skin default plus 4 palettes each recolor cables at once. Note your answer: should sequencer gate faders follow the palette?

## 7. Accessibility
- [ ] With Orca or an AccessKit inspector, focus tabs, knobs, buttons and the Sounds list. Each has a spoken name and role, not "unnamed"; tab order is sensible.

## 8. D01-D12 (definitions: docs/PRODUCT-PLAN.md)
- [ ] D01 Find, play, modify and save a sound: section 2 Sounds and save items.
- [ ] D02 Understand what a musical control changes: open a macro; its explanation matches what it does.
- [ ] D03 Diagnose by listening inside a patch: use signal inspection on a silent patch; it points at the break.
- [ ] D04 Turn controls without rebuilding the graph: sweep knobs while playing; no dropouts or zipper noise.
- [ ] D05 Recover audio: section 2 recovery item.
- [ ] D06 Correct expressive MIDI and timing: bend, wheel and channels behave; timing steady at two buffer sizes.
- [ ] D07 Play and reopen as a REAPER instrument: section 3 load, save and restore.
- [ ] D08 Automate and sync in REAPER: section 3 automation and tempo items.
- [ ] D09 Package a patch as a reusable module: group modules into a composite, duplicate, save, reload; sound is identical.
- [ ] D10 Custom light/dark module panels: open a module with a custom panel in both themes; every control is reachable.
- [ ] D11 Bounded custom-code modules: section 5 overrun item.
- [ ] D12 Creator SDK and portable packages: section 5 SDK item.

## Verdict
- [ ] Accept 1.0, or list blockers: ____________________
