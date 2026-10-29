# Grooves: handover

## Task (condensed)

From "Kabl overseer": Groove 14 total. Write `docs/presets/GROOVE.md` (plan of 14, about a quarter French touch signature), then build group 1 (~8) like the pad/bass presets: generator, level, register in `FACTORY`, tests, clips FLAC + 320k mp3 + tour FLAC with start times (20 ms join fades), every macro moves the render, no limiter (peak <= -1 dBFS incl. macro sweeps, loudness band), `audio_defects.py` clean, REAPER save/reopen, standalone check on `kabl_agents`. v8 patches. No engine/module changes (agent 1 is on stereo step 4 in `.worktrees/widget-set`). Lint + tests before push, draft PR (overseer opens if denied), `cargo clean` at the end. Branch `claude/presets-groove` from origin/master. Never merge.

## Done

- `docs/presets/GROOVE.md`: plan of 14 (4 signature: Copper Mile, Brick Dust, Rope Walk, Scrap Metal).
- `crates/ui/examples/presets_groove.rs`: one `groove(Spec)` builder (clock, `drum.grid` kit with patterns A groove and B fill, two `seq` parts, a MIDI voice, bus drive + ladder, optional phaser/chorus/bitcrusher, compressor, kick duck, space tail) and 8 group 1 specs. Patches written to `patches/presets/`, entries in `crates/ui/tests/library.rs`.
- `library` test passes (24 ok). Commits: see `git log claude/presets-groove`.

## Remaining, in order

1. Level: `cargo test --release -p kabl-ui --test factory_bank -- --nocapture` and `level_table -- --ignored --nocapture`. Fix by `trim_db` in the Spec or compressor makeup (a heavy compressor makes the trim insensitive: Brick Dust).
2. Macro sweep: `target/release/examples/presets_groove macros patches 1` (about 3 min, run in background). Last result: all alone/all-1 peaks under -1 dBFS; weakest macros were Still Water Pulse (-40 dB) and Glass Coast Hats (-36 dB) before the hat level was raised to -6 dB: rerun and confirm.
3. Clips: `PP_TOOL=presets_groove PP_OUT=docs/presets/clips/groove/group1 PP_GROUP=1` with `docs/presets/scripts/clips.py` (FLAC + 320k mp3 + tour with 20 ms fades); then `audio_defects.py`; `phrases` -> `host.py`; `standalone.py` on `kabl_agents`.
4. Fill the "Group 1" section of `GROOVE.md` (start times, levels, macro evidence, checks).
5. `cargo fmt`, `cargo clippy -D warnings`, tests, push, draft PR, `cargo clean`.
6. Group 2 (6 presets in the plan): Slab City, Rope Walk, Lamp Oil, Pinball, Scrap Metal, Dune Buggy. Needs a formant stage for Scrap Metal (see Loud Mouth) and uneven row lengths (`len1..len8` of the grid) for Pinball.

## Facts that cost effort

- A route into a mixer level adds to the stored value and clamps at 1: base 1.0 + route = dead macro. Store a lower base (Bass 0.4 + 0.6, Hats 0 + 2x kit level).
- `Rack::trim` adds the level trim to every `gain` module; the groove has several (seq parts, master), so `level_trim(slug)` in `preset_trims.inc` shifts all of them.
- Grid step text: `. , o x X ? !` (off, four velocities, 50 % chance, 75 % chance loud); 16 characters; `grid_row` asserts it.
- `seq` parts: `p1..p16` are semitones from the oscillator's C (`osc[4]` = octave), `length` 16 = one bar of 16ths at the clock.
- In this worktree the shell resets cwd; chain with `cd X && cmd` or use absolute paths. Level test numbers beat the generator's `levels`.
- `include!("preset_trims.inc")` is shared: add slugs there only if a trim is needed.

## Open decisions for Kosta

- None blocking. Grooves are heard only by measurement so far; no listening.

## Standing rules

Report to "Kabl overseer"; do not merge; stop when done; caveman and ponytail at ultra in chat, normal English in code, commits and docs.
