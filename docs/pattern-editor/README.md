# Cable pattern editor on the chrome kit

The editor for a functional cable (pattern, chance, morph, glide, routes into those) is drawn with the chrome widget kit and the active `Style`, so it follows theme A and theme B, light and dark. Code: `crates/ui/src/pattern.rs`. What a pattern means and how it is stored did not change: every control writes the same cable params as before (undo, save and the engine's in-place path are untouched).

## What it looks like and how it plays

- Header card: cable colour (the source's signal colour), `Cable #N`, `source -> destination` (for example `Macros #13 m1 -> Ladder Filter #10 cutoff_hz`), Close.
- `Steps` and `Chance` are kit sliders. Under them a step grid (two rows of eight at 16 steps): per step the level as a bar with its number above, the chance as a lower bar with its number below. A step with level 0 reads `off`.
- Click a step to switch it off; click again and it returns to the level it had (kept for the session only). Drag a bar up or down to set the level, drag across steps to paint; the lower bar works the same for chance. Hover a step for its number and values. Each touched step is one undo entry.
- With a pattern B and a morph above 0, a line on every A step marks the level morph gives that step (A blended with B).
- Playhead: the step the reference clock (lowest-id `clock`, as the compiler picks it) is on is outlined, from the existing clock position report, only while that clock runs. The grid asks for a repaint every 33 ms only while a playhead shows; a static editor does not repaint.
- Pattern B has its own length and grid. Routes into Chance, Morph and Glide use kit sliders, buttons, toggle and dropdown (also in the Routing drawer's list of cable routes, which shares the row).
- The explanation paragraph shows when the drawer's Help is on.

No theme slot was added: the editor reads `roles`, the type scale, spacing and the existing control slots. Routes into module knobs (the `Macros #13 m1  100 %  Invert ...` rows under a selected knob) and the module panel above the editor are not part of this change.

## Frame time

`cargo run --release -p kabl-ui --example bench_chrome 300 live pattern`: dense Composition, audio on, face taps animating, Routing drawer open on a patterned cable with the playhead moving every frame. Quiet machine, 1440x900: light median 4.1 ms, p95 4.2, max 4.6; dark 4.0, 4.2, 4.7. A second run at load average 9: 5.1, 5.8, 6.5. Without the editor the same bench gives 4.2 / 5.1 / 7.3 on that machine, so the editor adds well under 1 ms. One run during a heavy build spiked (median 11 ms, max 30 ms) and is not representative.

## Captures (`media/`)

`standalone-{1180x680,1440x900}-{a,b}-{light,dark}.webp` and `plugin-1180x680-{a,b}-{light,dark}.webp`: morph-arc patch, audio running, the pattern of the route into the ladder filter's cutoff, playhead on. A is Satin Studio, B is Lacquer and Light. Before: `docs/themes/media/pattern-{light,dark}.webp` (A left, B right). The plugin shots come from `scripts/host.py` (REAPER 7.75, `HOST_THEME=kabl.lacquer:dark SHOT=out.png python3 docs/pattern-editor/scripts/host.py patches/functional-cables/morph-arc scratch-dir`).

## Known gaps

- A drag across steps makes one undo entry per step touched, not one per drag.
- The remembered level of a switched-off step is not saved with the patch.
- No before capture of the plugin (the old editor was not run in REAPER).
- Index numbers of steps appear only in the hover text.
