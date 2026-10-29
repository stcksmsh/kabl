# No built-in drums: handover

Report to the session "Kabl overseer". Never merge. Stop when done. Caveman and ponytail ultra in chat; normal English in code, commits and docs.

## Task (condensed)

Rule (Kosta): no built-in drums in presets outside Groove and Piece (`docs/presets/BRIEF.md`, section "No built-in drums outside Groove and Piece"). A rhythmic feel is built from the host clock: `clock` into `clock.div` into an `env.adsr` that moves a gain or filter stage; depth on a macro; inert until Run.

1. Pump House on #60 (`claude/presets-pad-2`): done, commit 151021e, pushed.
2. New branch `claude/presets-no-drums` from origin/master AFTER "Merge pull request #59" is in `git log origin/master` (agent 2 changes `drums.rs` and drum hash entries). Same rework for Rail Yard, Steel Toe, Turnstile, Loading Dock. Remove their entries from `docs/presets/scripts/audio_defects.allow` and the `('steel-toe', 'rail-yard')` no-MIDI exemption in `docs/presets/scripts/host.py`. Re-render clips, plus a before/after tour (old vs new per preset, 20 ms fades). Scan with `audio_defects.py`, keep level rules (peak <= -1 dBFS with all macros up, loudness band), REAPER and standalone checks, draft PR.

## Done

- Pump House: `crates/ui/examples/presets_pad.rs` `pump_house()` (clock, `clock.div` 4, `env.adsr` 1/280/0/50 ms, `depth_route` from the envelope to the voice's trim `gain_db`, amount -0.30 on macro m3; gain_db spans 84 dB). Dips about 10 dB per beat at Pump 0.7.

## Remaining

Step 2 above. The four presets live in `presets_bass_lead.rs` (steel_toe, rail_yard, turnstile, loading_dock; drum helpers `drum_lane`, `finish_drums`). Start: `git fetch; git log origin/master --oneline | grep "pull request #59"`.

## Facts

- `Rack::trim` adds the per-slug trim to EVERY `gain` module: route the duck into the existing trim stage, do not add a second `gain`.
- `presets_pad macros` now applies the level trim, so its all-up peak is the real one.
- A route from a source to a param that the recipe already routes shares one cable; `depth_route` then scales the whole route. Use a different source or set the recipe amount to 0.
- REAPER `host.py` check ignores state `version` (script writes 2, plugin saves 5).
- Binaries vanish after `cargo clean`: rebuild `-p kabl-clap`, and `-p kabl-ui --bins --examples` (8 min) before `standalone.py`.
- `cargo test --release` fails to compile (AllocDisabler); use the debug profile.

## Decisions for Kosta

None open.
