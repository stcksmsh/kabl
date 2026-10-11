# Cue macros

A cue can set macro targets. When the cue lands, each macro with a target moves from where it is to the target over the cue's morph time, driven by the reference clock's position. A macro with no target in a cue is not touched.

## Data and schema

Stored as params on the `cues` module, like the rest of a cue: `cue<N>.macro.<module id>.<k>` (k = 0..3, value 0..1; absent = leave the macro) and `cue<N>.morph` (length in beats, 0..16; absent or 0 = jump). **No schema change** (stays 7): a patch without these keys is the same file and plays the same. There is no curve option: the morph is linear and the macro module's own 20 ms glide rounds the corners. Add one when someone asks.

## Engine (`crates/engine/src/morph.rs`)

A launch carries up to 8 macro targets and the length in pulses. The engine starts each morph at the cue's boundary on its clock, then once per block writes `from + (to - from) * elapsed / length` into the macro's param. `elapsed` is the sum of the clock position's forward movement, so it is tempo-aware. Nothing allocates (tested under `assert_no_alloc`); a block's value step is smoothed by the macro's glide, so there is no zipper noise (test: no step above 1e-4 per sample over a 0.6 swing in a bar).

Decisions for the cases in the brief (each has a test in `crates/engine/tests/cue_macros.rs`):

| Case | Rule |
| --- | --- |
| Next cue arrives before the morph ends | The old morph runs until the new cue lands, then the new one starts from where the macro is at that moment. |
| Hand grabs a macro (knob, MIDI CC) | The hand wins: the morph for that macro ends and the macro stays where the hand put it. It is released by the next cue that has a target for that macro, which starts a new morph from the hand's value. Other macros in the cue keep moving. |
| Host automation of the same macro | Same as a hand, per value: a *new* host value ends the morph. The host re-sending the same value every block does not. So a moving envelope owns the macro; a flat lane does not fight the cue. Last writer wins. |
| Transport stops | The morph freezes with the clock position and resumes on Run. |
| Restart, or a host locate or loop (position jumps back, or more than a pulse and a half either way) | The morph finishes at its target at once. |
| Cue lands on a stopped clock, or length 0 | The macro jumps. |
| Tempo changes mid-morph | The morph is measured in clock pulses, so it stretches or shrinks with the tempo and still ends on its pulse. |
| Graph recompile (any edit) | The engine holds the macro at its driven value, so an edit does not snap it back to the stored knob value. |
| Load of another patch | All morphs and holds are dropped. |

The stored macro value in the patch is the hand's value: the engine never edits the document. The interface shows the live value from the engine's report (`UiState::macro_live`).

## Interface

- Perform, macro tile: the knob shows the live value, the morph's destination as the knob's range arc, and `→ 0.85 · 40%` under the value; turning it takes over.
- Perform, scenes card: a cue that sets macros says "sets macros"; the playing one fills a bar as its macros arrive; **Store macros** writes the current macro positions (live ones included) into the playing cue.
- Drawer, cue editor: per macro a toggle (target set or left alone) and a value, a Morph length in beats, and Store macros.
- Macro-only cues (no sequencer set to a bank) count as playing after they are launched.

## Plugin and host automation

Cue movement is not sent back to the host as parameter changes (that would write automation in a host's write mode). Host automation is applied as before; the rule above decides who owns the macro. Checked in REAPER 7.75 (`scripts/host.py`, `evidence/host/`): the project's MIDI track sends CC 40-43, which the patch maps to the four cues; the plugin editor at its default size is in `img/host-*.png`.

- States identical across save, full quit, reopen, save (float32); renders of the first and second run identical (relative difference 0.0).
- The cues do something in the host: the same project without the CC item renders differently (relative difference 1.12, envelope correlation 0.08). Per-second loudness: flat 0.018 until the Chorus lands (bar after 6 s), up to 0.046 by about 12 s, back to 0.02 after Verse 2.
- Recorded host automation on macro m1 (flat, then a ramp over the Bridge): render differs (relative difference 0.33).
- An old project opened and saved by this build comes back with an identical state. CLAP validator: not installed on this machine, so only CI runs it.
- Not verified: other hosts, listening, the rack face of `cues`, which still reads "no cue matches what plays" for a macro-only cue (the face code is in Agent 1's files, left alone).

## For Kosta: the demo

`patches/cue-macros` (Cue Macros in the Sounds list). Four cues on CC 40-43: Verse, Chorus, Bridge, Verse 2. Clip: `cue-macros.mp3`, 18 bars at 100 bpm (2.4 s a bar), each cue launched on its bar.

| Bar | Cue | Morph | Listen for |
| --- | --- | --- | --- |
| 0 | Verse | jump | dark, dry bass and nothing else |
| 4 | Chorus | 2 bars | the bass filter opens, an echo and room come in, a wide pad arrives |
| 8 | Bridge | 4 bars | the filter closes half way again while the space swells to the most wet, pad thins |
| 14 | Verse 2 | 1 bar | everything drains back to the dark, dry verse |

Captures (standalone, 1280x800, its minimum size): `img/01..06`, light and dark, targets set and a morph part-way for Chorus and Bridge. `scripts/capture.py` makes them with real input. `img/host-*.png`: the plugin editor in REAPER.
