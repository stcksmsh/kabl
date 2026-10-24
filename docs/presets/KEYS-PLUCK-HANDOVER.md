# Keys / Pluck presets: handover

Branch `claude/presets-keys-pluck` (PR #46, draft; never merge). Report to the session "Kabl overseer". Stop when done.

## Task as received
1. Commit and push the previous agent's uncommitted work as WIP.
2. Diagnose "no glide in Parquet Loop" (Glide 0.8, ALWAYS): pitch-track, trace Glide to ms, check poly and mono, confirm in REAPER on the `kabl_agents` sink.
3. Fix an engine or renderer defect only if one exists. Do not continue to group 2 presets.

## Done
- `a1a8d98` WIP commit: group 2 start (15 preset definitions in `crates/ui/examples/presets_keys_pluck/group2.rs`, 14 new patch dirs, library entries), Parquet Loop legato and glide-demo clips, `clips.py` additions.
- Glide diagnosis: no engine or renderer defect (below). Added `docs/presets/scripts/host_glide.py` and evidence in `docs/presets/evidence/host-glide/`.
- `cargo test -p kabl-ui --test library`: 24 passed with the WIP.

## Glide finding
- Cause: none found. The stored patch glides in the offline renderer and in real REAPER (CLAP plugin on `kabl_agents`).
- Measured: constant-time linear slide in semitones, about 0.49 s for a 16-semitone leap (A3 from E2: 40.7 to 56.9 in 0.50 s; D4 from G2: 43.7 to 61.9 in 0.50 s). Offline FLAC and the REAPER render match within 0.5 semitone.
- Chain: macro m4 stored 0.9, route amount 0.8 on `midi.in.glide_ms` (5..3000 ms, exponential): 0.72 of range gives 5 * 600^0.72 = about 500 ms. Matches the measurement. `midi.in.glide` stored 1 (ALWAYS).
- Poly (`poly_on`, glide from nearest tone of the previous chord) and mono (legato clips) both slide. Same-key retrigger in POLY never glides, by design.
- Off control (`macro.m4=0`): glide_ms is 5 ms, so notes step. That is the "Glide down" behaviour.

## Remaining (not started, in order)
1. Ask Kosta how he tested: which build, which preset copy, whether he moved the Glide knob down, and whether he played overlapping chords. If still silent for him, repeat with his exact MIDI.
2. Group 2: render clips (`docs/presets/scripts/clips_keys_pluck.py`), listen, host check with `host.py`, tune. None done. Patches exist, sound unreviewed.
3. Next command: `target/release/examples/presets_keys_pluck write <dir>` after `cargo build --release --examples -p kabl-ui`.

## Facts that cost effort
- `host_glide.py` needs `scratch/presets-phrases.json` with a `parquet-loop` entry (`secs`, `hits` as `[t, note, vel]`, `cues`). `GLIDE_M4=<0..1>` overrides macro m4. Needs a release `kabl-clap` build.
- `PP_SET="macro.m4=0"` and `PP_HITS="t:note:vel;..."` override params and phrase in `presets_pilot`.
- Autocorrelation pitch tracking reads the first note as an octave error; ignore note 0.

## Working assumptions / open decisions
- Assumption: Kosta heard a 0.5 s slide on a decaying pluck as no glide, or tested a different build. Unconfirmed.
- Decision for Kosta: shorter default glide, or exponential (not linear) slide, if 0.5 s reads too weak.
