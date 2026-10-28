# Pads: handover

## Task (condensed)

Pad 14 total (the pilot's Salt Flat counts); group 1 is about eight. Write `docs/presets/PAD.md` (plan, then group 1), build group 1 like the bass/lead presets: generator, level, register in `FACTORY`, tests, clips FLAC + 320k mp3 + tour FLAC with start times, every macro moves the render, no clicks, REAPER save/reopen, standalone check. About a quarter French touch signature, the rest broad genres. Stereo only where the engine has it today (`docs/stereo/DESIGN.md` step 1); no stereo engine work. No module or engine changes. No slug clash with master or `claude/presets-keys-pluck` / `-2`. Branch `claude/presets-pad` from origin/master. Report to "Kabl overseer"; never merge; `cargo clean` at the end.

## Done

- Plan of 14 in `docs/presets/PAD.md` (4 signature: Dry Cleaner, Concrete Cloud, Pump House, Tin Voice).
- Group 1 (8 pads): Fog Bank, Deep Field, Cotton Mill, Picture Window, Drive-In, Dry Cleaner, Concrete Cloud, Static Garden. Generator `crates/ui/examples/presets_pad.rs`, patches in `patches/presets/`, entries in `crates/ui/tests/library.rs`, clips and tour in `docs/presets/clips/pad/group1/`, evidence in `evidence/host-pad/` and `evidence/standalone-pad/`. Results and measurements are in `PAD.md` (Checks).
- Commits: see `git log claude/presets-pad`.

## Remaining, in order

1. Kosta listens to group 1.
2. Group 2 (5 pads, slugs in `PAD.md`): Glacier, Pump House, Tin Voice, Rooftop, Tidepool. Add the functions and `Preset` entries (`group: 2`) to `presets_pad.rs`. Pump House needs the kick lane and sidechain pieces from `presets_bass_lead.rs` (`drum_lane`, `finish_drums`, Steel Toe's chain); Tin Voice uses `filter.formant` (see Loud Mouth).
3. Then the same steps as group 1: `write patches 2`, `levels patches 2`, `macros patches 2`, `library patches 2` (paste after the group 1 block in `library.rs`), tests, `clips.py` (`PP_TOOL=presets_pad PP_OUT=docs/presets/clips/pad/group2 PP_GROUP=2`), `phrases`, `host.py`, `standalone.py`.

## Facts that cost effort

- The level that counts is `cargo test --release -p kabl-ui --test factory_bank level_table -- --ignored --nocapture`; for a poly patch it plays E3 B3 E4 G4 at velocity 90, down at 1, 5, 13 s, up at 4, 9, 17 s. The generator's `levels` command copies that but can read 1 to 4 dB differently for slow pads: trust the test.
- Stored macro values already move the knob (see `BASS-LEAD-HANDOVER.md`); a macro on a mixer level or noise level needs a high enough route amount, or it measures about -29 dB (Static Garden Texture was that at first).
- The `limiter` module has instant attack and no lookahead (its own doc says so): a pad that pins it crackles. Keep a compressor before it (`Voice::guard`) and check clips with the second-difference test (per 50 ms, peak over RMS of the second difference above 12), not a high-frequency-energy scan; a long reverb on a bass-heavy pad adds single-sample kinks too (about 1 % of the signal at 10 s decay).
- `ringmod` has no params: a "Metal" macro is a mixer level on a ring-modulated layer, not a mix control.
- `osc.va` has `out_r`, but a stereo cable needs the `left`/`right` or `in_l`/`in_r` pairs of the fx modules (step 1), so unison stays mono until step 2.
- Commands are refused in the worktree when compound (`&&` is fine in a plain `cd X && cmd`, heredocs and `sed 'Nr file'` are not): write scripts with the Write tool.

## Open decisions for Kosta

- None blocking. Dry Cleaner has no tempo-locked pump on purpose (Pump House is the ducked pad).

## Standing rules

Report to the session named "Kabl overseer"; do not merge; stop when done; caveman and ponytail at ultra in chat, normal English in code, commits and docs.
