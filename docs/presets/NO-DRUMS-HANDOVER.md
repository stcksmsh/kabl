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

## Step 2 done (claude/presets-no-drums)

Update (Kosta approved): Loading Dock is not reworked. It is back to its master version with its drums, category Groove (it plays itself on Run; `GROOVE.md` counts 15, `BRIEF.md` updated). Everything below about Loading Dock (duck depth, -4.0 dB trim, its levels, evidence) is superseded: no trim, patch and clip equal master apart from the category. The before/after tour has the other three only.

Steel Toe, Rail Yard, Turnstile, Loading Dock: `drum.*`, `seq` lanes, `drum.grid`, the sidechain compressor and the drum bus are gone. Each has `clock.div` (div 4) into `env.adsr` (3 ms attack, 200-230 ms decay) routed to the voice's trim `gain_db`, depth -0.30 (bass) or -0.25 (Turnstile, Loading Dock) scaled by the Pump macro (m3; m4 in Loading Dock). Without MIDI the three keyboard/bass presets render silence (rms 0.0); Loading Dock plays itself. Steel Toe's amp attack went 1 to 2 ms: the kick had hidden a CLICK at 6.548 s. Trims: Turnstile -4.8 dB, Loading Dock -4.0 dB.
Levels (clip peak / loudness): Steel Toe -4.3 dBFS / -14.8, Rail Yard -8.4 / -18.6, Turnstile -3.5 / -15.1, Loading Dock -2.4. `factory_levels_hold_the_target` passes (all macros up, peak <= -1 dBFS).
Evidence: `evidence/host-no-drums`, `evidence/standalone-no-drums`. Before/after tour `clips/no-drums/before-after-tour.flac` (start seconds, before / after): steel-toe 0 / 10, rail-yard 20 / 30, turnstile 40 / 51, loading-dock 62 / 93. Group tour starts: steel-toe 65 s (bass-lead group1), rail-yard 110 s (bass-lead group2), turnstile 153 s (keys-pluck group2).
`audio_defects.allow` had no entries for these four; the `host.py` no-MIDI exemption for steel-toe and rail-yard is removed.
