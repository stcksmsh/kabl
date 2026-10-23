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
- **Patterns A to D** are stored in the module. The face tabs set the `pattern` param; the `pattern` input adds to it. Not done: switching by cue (a cue launches `seq` banks only) and a chain arranger (out of scope).
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
