# Keys / Pluck presets, group 2: handover (branch `claude/presets-keys-pluck-2`)

Group 1 (8 presets, Parquet Loop glide and legato clips, `host_glide.py`) is PR #46 on `claude/presets-keys-pluck`. This branch is group 2 on top of master. Report to the session "Kabl overseer", do not merge, stop when done.

## Task (condensed)
Finish 15 keys and plucks: rename keys Copper Wire (slug clash with bass Copper Wire on master), level, register, tests, clips FLAC + 320k mp3, tour FLAC with start times, every macro moves the render, REAPER save/reopen check, standalone check. Run `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` before every push.

## Done
- Rename: keys Copper Wire is now Brass Filament (`brass-filament`); master's bass Copper Wire kept.
- Generator `crates/ui/examples/presets_keys_pluck/group2.rs`; new check `presets_keys_pluck moves [slug]` (each macro alone at 0 and at 1, difference in dB below the louder render; under -40 dB prints FLAT). No preset is FLAT.
- Level test (`levels`; band -19 +- 5.5 loud, peak <= -1.0): all 15 inside, loud -17.0 (Willow) to -20.9 (Spinet), peaks -1.5 to -5.3.
- Tests: `cargo test -p kabl-ui --test library --test factory_bank` pass; fmt and clippy clean.
- Clips and tour: `docs/presets/clips/keys-pluck/group2/` (FLAC + 320k mp3 per preset, mp3 checked against the float WAV; `tour.flac`, `tour.txt`).
- REAPER (real CLAP, save, quit, reopen): `docs/presets/evidence/host-keys-pluck-2/`; all 15 states identical, restart renders equal, silent without MIDI.
- Standalone (kabl_agents sink, real-time priority): `docs/presets/evidence/standalone-keys-pluck-2/`; 15 of 15 ok.

## Tour start times (tour.flac)
Tin Roof 0:00, Turnstile 0:14, Hand Bell 0:25, Corduroy 0:40, Matinee 0:50, Snowmelt 1:08, Scaffold 1:28, Brass Filament 1:38, Spinet 1:55, Willow 2:02, Dry Creek 2:14, Gumdrop 2:21, Cold Cut 2:28, Rivet Gun 2:38, Tuning Fork 2:44.

## Presets (4 macros each, from code)
- Tin Roof: Wurlitzer-like electric piano that barks harder when played harder. Tone, Bark, Room, Chorus.
- Turnstile: driven, phased saw chord pumping against an internal four-on-the-floor kick, 123 BPM (signature). Cutoff, Crunch, Room, Pump.
- Hand Bell: inharmonic FM bell, three operator pairs. Strike, Grit, Space, Shimmer.
- Corduroy: funk clavinet, narrow pulse with a hard filter snap. Cutoff, Drive, Room, Phase.
- Matinee: wide keyboard from seven detuned saws. Cutoff, Warmth, Space, Width.
- Snowmelt: slow soft electric piano with drifting tuning. Tine, Warmth, Space, Drift.
- Scaffold: dark wide drum and bass stab, eight saws, phaser, compressor. Cutoff, Grit, Room, Length.
- Brass Filament: metallic FM tone that rings and wavers. Strike, Rust, Space, Warp.
- Spinet: bright harpsichord-like key with a faint buzz. Tone, Buzz, Room, Chorus.
- Willow: soft harp, quick pluck, long ring. Pluck, Warmth, Space, Stretch.
- Dry Creek: nylon-string guitar pluck. Pick, Warmth, Room, Chorus.
- Gumdrop: short bouncy pop pluck, resonant filter bounce. Cutoff, Drive, Space, Decay.
- Cold Cut: dark short bell-pluck for trap. Tone, Dirt, Room, Decay.
- Rivet Gun: bright staccato phased pluck, driven and squashed. Cutoff, Crunch, Room, Phase.
- Tuning Fork: tuned metal bars, six free-bar partials. Strike, Warmth, Space, Wobble.

## Measured flags
- Level test loud/peak dBFS: Tin Roof -18.7/-4.6, Turnstile -19.1/-3.4, Hand Bell -19.0/-1.5, Corduroy -20.0/-2.8, Matinee -18.6/-5.3, Snowmelt -17.2/-1.5, Scaffold -20.5/-4.7, Brass Filament -19.1/-3.1, Spinet -20.9/-3.4, Willow -17.0/-2.2, Dry Creek -19.3/-1.8, Gumdrop -18.5/-4.3, Cold Cut -19.7/-1.5, Rivet Gun -19.8/-1.5, Tuning Fork -19.6/-1.5.
- Clip peaks are lower (-1.5 Turnstile and Snowmelt to -10.0 Cold Cut): the clip phrases are sparse and soft.
- Click scan (5 ms windows, share above 3 kHz over 0.7 at audible level): Hand Bell 36 windows (bright by design, inharmonic partials), Cold Cut 5 windows (metallic click by design); all others 0. Not judged by ear.
- Weakest macro moves: Corduroy Cutoff -24.1 dB, Spinet Tone -21.8, Matinee Cutoff -18.7; the rest are -17 dB or stronger.
- Phrase registers (MIDI): Tuning Fork 69-84 and Cold Cut 67-76 highest; Matinee, Snowmelt, Scaffold, Willow reach down to 41.
- Nothing here is a listening verdict; Kosta has not heard group 2.

## Remaining
1. Kosta listens to the group 2 clips and tour.
2. Draft PR for this branch (overseer opens it if push to a PR is denied); after #46 merges, merge master into this branch.
3. `cargo clean` when done.

## Facts
- `PH_OUT=... PH_PHRASES=... python3 docs/presets/scripts/host.py [slugs]`; phrases from `presets_keys_pluck phrases <file>`.
- Compound shell commands can trip the worktree guard. A temp `git worktree add` with `CARGO_TARGET_DIR` set to this target dir avoids a second build.
