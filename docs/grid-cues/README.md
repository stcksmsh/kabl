# Grid cues

A cue can set a drum grid's pattern (A to D), so a self-playing piece moves from a groove to a fill to a breakdown by section.

## Data and schema

A grid is a cue target exactly like a sequencer: `cue<N>.seq.<module id>` holds the pattern, 0 (A) to 3 (D). **No schema change.** A patch without such a key is the same file and plays the same, and a grid that no cue names keeps whatever its `pattern` param says. Deleting the grid removes its cue entries in the same undo step, as for a sequencer.

## Switching rule

A pattern switch lands on the **cue's own timing, the next bar of its reference clock by default** (a cue may also be set to next step or now), and the grid changes pattern on its **first clock edge at or after that line**. The tick counter does not restart, so a polymeter row keeps its phase.

Why the bar and not the pattern loop: rows of a grid have their own lengths (1 to 16), so "the end of the pattern" is the least common multiple of those, which can be many bars; a cue would then wait for no musical reason. The bar line is the one boundary every row shares when the lengths are 16, 8, 4, 2 or 1, and it is what the sequencer banks already use, so one cue switches grids and sequencers on the same line. A beat is never cut mid-bar. If you want a pattern to start from its first step whatever the phase, send the clock's `reset` on the same cue (not done here).

Interplay with the other pattern sources:

| Case | Rule |
| --- | --- |
| `pattern` input patched | Still added to the playing pattern, modulo 4: cue picks the base, cable adds on top. |
| Hand clicks a tab, or host automation sets `pattern` | A new value of the param takes the grid back from the cue (like a hand on a macro). The cue's next launch takes it again. |
| Launch while the clock is stopped, or timing Now | The grid is armed at once and switches on its next clock edge. |
| Cue dropped before it lands (Cancel) | The grid keeps its pattern. |
| Recompile (any edit) | The cued pattern, a pending arm and the last seen `pattern` value are carried in the module state. |
| Load of another patch | Pending launches are dropped, the grid starts on its stored `pattern`. |

## Engine

The grid has `arm`, `cancel`, `armed` and `pattern` like `Seq`; `CompiledPatch::arm_bank` and `cancel_bank` arm either kind, and `seqs()` reports grids with their playing pattern and armed pattern, so the cue buttons' playing/queued state and the Perform view work for a grid with no further change. `Launch` is unchanged. Nothing allocates (tests run under `assert_no_alloc`).

## Interface

Drawer, cue editor: one row per grid, `Drum grid #id`, with **Keep** and **A B C D**, next to the sequencer rows. **Capture** includes grids. The Perform scenes card shows a cue that targets a grid as playing or queued like one that targets a sequencer.

## Sequencer banks

Cues already switch `seq` banks through the same launch (same timing, same line, same cancel), so nothing was added for them; the grid now follows the same path rather than a second one.

## Tests

- `crates/engine/tests/grid_cues.rs`: next bar switches within two blocks of the bar line and no snare of B before it; next step lands on the next step; a grid the cue does not name keeps its pattern; a new `pattern` param value takes the grid back; Cancel keeps the pattern. All under `assert_no_alloc`.
- `crates/ui/tests/grid_cues.rs`: a grid is a cue target, the launch carries it, clearing it, removing the grid.

## Demo

`patches/grid-cues` (Grid Cues in the Sounds list), 124 bpm, one grid, four cues on CC 40-43:

| Bar | Cue | Pattern | Listen for |
| --- | --- | --- | --- |
| 0 | Intro | D | hats only |
| 2 | Groove | A | kick on every beat, snare and clap on 2 and 4, open hats |
| 6 | Fill | B | the groove for two beats, then a rising snare roll and toms |
| 7 | Break | C | a kick on the one, a clap, toms, one open hat; the room goes quiet |

Clip: `clips/grid-cues.flac` (24-bit) and `clips/grid-cues.mp3` (320 kbps, error -74.9 dBFS, snr 50.8 dB below 16 kHz), 10 bars. Rebuild: `cargo run --release -p kabl-ui --example grid_cues_demo -- render patches/grid-cues out.wav`, then `scripts/clips.py` (reuses `encode_and_check` from `docs/presets/scripts/clips.py`).

REAPER 7.75 check (`scripts/host_cues.py`, results in `evidence/host/`; the project's MIDI item sends CC 40-43 at 0.1, 3.2, 10.5 and 12.5 s, REAPER on the silent sink):

- State identical across save, full quit, reopen, save. The four grid targets are in the saved project.
- The first and second render are identical (relative difference 0.0).
- The same project without the MIDI item differs (relative difference 0.83) and stays on its stored pattern A.
- The loudness steps up when Groove lands (3.87 s, the bar-2 line of a clock that starts with the render) and down when Break lands, 5.06 bars later (windows of a sixteenth bar, so 5 bars within one window). Sample-exact timing is in the engine tests; this shows it through the real plugin.
- Not verified: other hosts, listening.

Screen recording with sound: `media/grid-cues-recording.mp4` (25 s, 1280x800, Perform view, real mouse clicks on the cue pads, the cue pads show Playing and the queued cue). Made by `scripts/record.py`: the standalone plays into the silent sink `kabl_agents`, the sound is captured from `kabl_agents.monitor`, and the script checks the sink input is on the silent sink. Mean level per 2 s: Intro -50 dB, Groove -22, Fill -22, Break -27.

Starting state: the grid stores pattern A, so a patch that is only run (no cue) plays the Groove. Intro's cue timing is Now, so pressing it gives the hats at once; the other three wait for the bar line.
