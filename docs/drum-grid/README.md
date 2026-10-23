# Drum grid

`drum.grid` programs a whole beat in one module, like a drum machine: eight rows of up to 16 steps, a gate output per row, four patterns, per-step velocity and chance, per-row length.

Before the grid, one beat needed one 8-step `seq` per drum, a clock and a cable pair per voice: the four-on-the-floor demo used five sequencers. It now uses one grid.

## Ports

| Port | Type | |
|---|---|---|
| `clock` | in | Each rising edge plays the next step of every row. Swing is the clock's. |
| `reset` | in | The next clock edge plays step 1 again. A reset on the same sample as a clock edge restarts at once. |
| `pattern` | in | Added to the face's pattern: `round(3 * cv)` patterns up, modulo 4. Read on every clock edge, so a change lands on the next step. |
| `kick` `snare` `clap` `chat` `ohat` `tom` `perc1` `perc2` | out | One gate per row. |

## Design decisions

- **Eight rows, eight gate outputs.** The engine allows 8 outputs per module (`MAX_OUTPUTS`, raised from 4 by PR #40) and 175 params. Eight rows is one output each. A velocity output per row would need 16.
- **Velocity rides the gate level.** A step's gate is `0.5 + 0.5 * velocity`. The drum voices already fire on a rising edge through 0.5; when their `velocity` input is unpatched they now read `2 * gate - 1` as the velocity (`hit_gain` in `drums.rs`). A gate of 1, which every existing source sends, is still full velocity, so existing patches are unchanged. A patched `velocity` input still wins.
- **The hat choke works.** Row 4 (`chat`) goes to the hat's `closed`, row 5 (`ohat`) to `open`: one cable each.
- **Steps are packed into params.** A step is a 5-bit code (0 off, else `1 + velocity + 4 * chance`). Four steps fit one param (20 bits, exact in an f32), so a pattern is 32 params, four patterns 128, plus 8 lengths and `pattern`: 137 of 175. One param per step would be 512. The saved patch holds ordinary params (`a11` is pattern A, row 1, steps 1 to 4), so there is no schema change and old patches are untouched.
- **Four velocity levels (30, 55, 78, 100 %) and four chances (100, 75, 50, 25 %).** A grid is for quick programming; four levels is what a step button row on a drum machine gives.
- **Polymeter.** Row `r` plays step `tick % len(r)`. Each row has a length of 1 to 16.
- **Chance.** Every row draws once per clock edge, whatever its steps hold, so editing a chance never shifts the random stream. Seeded from the module id and reseeded on reset, so a loop with chance repeats exactly after a reset.
- **Patterns A to D** are stored in the module. The face tabs set the `pattern` param; the `pattern` input adds to it. A cue can switch the pattern on the next bar (see `docs/grid-cues`). Not done: a chain arranger (out of scope).
- **Editor on the face.** The face is 22 units wide (660 px at 100 %), so 16 steps are about 36 px wide, enough for click and drag. No separate expanded editor.

## Editing

- Click a step to write the brush (the velocity and chance picked top right). Click a step that already holds the brush to clear it.
- Drag to paint: starting on an empty step writes, starting on a step that holds the brush erases.
- Scroll over a step to change its velocity; right-click to cycle its chance.
- Shift-click a step to set the row's length (steps after it are dimmed).
- A–D tabs pick the pattern. The playhead of each row is outlined.

## Notes

- Playhead: the face reads the reference clock's position the way the cable pattern editor does, so a grid clocked by another clock shows the reference clock's position.
- Limits: a gate follows the clock pulse, so a step is as long as the clock's pulse; chance and velocity cannot be set by a cable.

## Evidence

- Module tests: `crates/modules/tests/drum_grid.rs` (default beat, velocity in the gate level, chance rate and determinism, polymeter, reset, pattern param and input, voices reading the gate level, no allocation).
- Demo: `patches/sound-engines/demos/four-floor` is now one clock, one grid, five voices (it was a clock and five sequencers). Pattern A is the groove, pattern B adds a snare ghost note and a tom fill. `crates/ui/tests/drum_demos.rs` builds it through `PatchEditor`. The pumping-kick demo is unchanged. The old lanes sent velocities of 1 to a percent param (about 1 %); with real velocities the kit mixer is at about 0.4 of the old level to keep the factory loudness (peak 0.61, rms 0.116).
- Clip: `media/beat-velocity-chance.mp3`, 16 s: hats at four velocities, ghost snares that play half the time or less, an open hat at 75 % (`write_clip` in `drum_demos.rs`).
- Captures: `media/grid-<A|B>-<light|dark>-1440x900.webp` and `-plugin-1180x680.webp` (the editor at the plugin's size), `grid-editing-while-playing.mp4`, `grid-edited-brush-and-chance.png`. Script: `scripts/capture.py`.
- Frame time, `bench_chrome 300 live` on the four-floor patch (rack only, displays live): dense 1440x900 light median 1.7 ms, p95 2.5, max 3.5; dark median 2.7, p95 5.2, max 7.3. 1280x800: median 2.1 and 1.4, max 3.6 and 2.8.
- REAPER 7.75: `scripts/host_grid.py` renders the patch from the CLAP plugin offline. Kick onsets sit on the 124 bpm beat grid within 10 samples of their median, the project saves and reopens with an equal state, the second render is bit-identical. Results in `evidence/host/`.

## For agents

Run every standalone, `kabl-ui` or REAPER with `PIPEWIRE_NODE=kabl_agents PULSE_SINK=kabl_agents`: that sink is silent. Check `pactl list short sink-inputs` after the start.
