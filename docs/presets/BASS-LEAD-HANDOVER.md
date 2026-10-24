# Basses and leads: handover

## Task (condensed)

Build the first part of the factory preset bank: Bass to 14 and Lead to 12 (the pilot's Cinder Block and Undertow count toward Bass, Neon Highway toward Lead), spread over the genres in `docs/presets/BRIEF.md`, about a quarter signature. Read `BRIEF.md` and `PILOT.md` first. No module or engine changes (write gaps down). Every preset passes the brief's checklist and the level test, loads and plays in the standalone and REAPER on the silent sink `kabl_agents`. Deliver in two groups for listening: per preset one FLAC plus 320 kbps mp3, plus a tour FLAC (one second of silence between presets) with start times, and one line per preset (name, role, genre, signature or not, what to listen for). Agent 2 builds keys and plucks (`claude/presets-keys-pluck`) and also edits the `FACTORY` list in `crates/ui/tests/library.rs`: keep entries in one contiguous block. Do not merge. Report to "Kabl overseer". `cargo clean` at the end.

## Done

- Group 1 (8 presets) delivered on draft PR #48, branch `claude/presets-bass-lead`: Steel Toe, Tar Pit, Acid Rain, Oatmeal (bass), Spun Sugar, Pig Iron, Hot Wire, Lantern (lead). Commits `b1d21f2` (group 1), `74e1ec3` (Acid Rain and Hot Wire resonance and click fix), `14b93af` (merge of master with PR #42 and #43), then the rename commit and the group 2 code commit (see `git log`).
- Kosta's verdict: Steel Toe and Tar Pit keep; Oatmeal, Spun Sugar, Pig Iron, Lantern OK (renamed from Soft Shoe, Satin Ribbon, Rust Belt, Paper Crane at his pick); Acid Rain and Hot Wire were too resonant with high clicks, fixed (see lessons).
- Generator `crates/ui/examples/presets_bass_lead.rs`; report and checks in `docs/presets/BASS-LEAD.md`; clips in `docs/presets/clips/bass-lead/group1/`; evidence in `docs/presets/evidence/host-bass-lead/` and `standalone-bass-lead/` (these still carry the old slugs Paper Crane, Satin Ribbon, Soft Shoe, Rust Belt; rerun the scripts to refresh).
- Group 2 code is written and compiles but is NOT verified: the 15 functions and `Preset` entries (`group: 2`) are in the generator; their patches are not written to `patches/presets/`, not in `FACTORY`, not leveled, not heard.

## Remaining, in order

1. `cargo run --release -p kabl-ui --example presets_bass_lead -- write patches 2` (the last argument is the group; omit for all).
2. `target/release/examples/presets_bass_lead levels patches 2`: each preset must land at -19 +- 3 dBFS loudness, peak under -1; adjust each `trim_db` (or the compressor makeup for heavily compressed ones). `... macros patches 2` shows each macro's effect in dB (nothing dead) and the all-0 / all-1 peaks.
3. `... library patches 2` prints `FACTORY` entries: paste them in one block after the group 1 block in `crates/ui/tests/library.rs`. Run `cargo test -p kabl-ui --test library --test presets --test factory_bank` (library search tests fail if a description contains a word those tests expect only elsewhere, such as "singing" or "warm").
4. Check the clips objectively (`>3 kHz` energy in 5 ms windows, see the lessons) since nobody on this side can listen.
5. Clips and tour: `PP_TOOL=presets_bass_lead PP_OUT=docs/presets/clips/bass-lead/group2 PP_GROUP=2 python3 docs/presets/scripts/clips.py`.
6. Hosts: `target/release/examples/presets_bass_lead phrases scratch/bass-lead-phrases.json`, `cargo build --release -p kabl-clap`, then `PH_PHRASES=scratch/bass-lead-phrases.json PH_OUT=docs/presets/evidence/host-bass-lead-2 python3 docs/presets/scripts/host.py <slugs>`; standalone: `PH_OUT=... python3 docs/presets/scripts/standalone.py slug=note,note ...` (`cargo build --release -p kabl-ui --bins --examples` first). Rail Yard (and Steel Toe) play their drums with the transport: host.py allows the kick without MIDI.
7. Report to Kosta: one line per preset, tour start times; update `BASS-LEAD.md` and the PR body; `cargo clean`.

Group 2 in the generator: bass Rubber Band (funk), Slate Roof (neuro, drum and bass), Gutter (French touch, signature), Low Tide (ambient sub), Copper Wire (experimental ring mod), Tail Light (synthwave), Rail Yard (house, drum grid sidechain), Oak Stave (upright pluck); lead Glass Needle (modular sine), Loud Mouth (French touch, formant, signature), Dust Devil (trap whistle), Afterglow (synthwave, 7-voice unison), Horn Section (funk and disco brass), Fire Escape (house stabs, 8 + 4 voice unison, pitch dive), Short Circuit (French touch, ring mod and crush, signature).

## Lessons

- **Filter-attack clicks on acid sounds.** A 1 ms filter-envelope attack makes the cutoff jump and the resonance ring; with ladder drive plus the drive stage feeding a 0.40 resonance it reads as high clicks. Keep filter attack at 4 ms or more, amp attack 3 ms, resonance 0.3 or less, ladder drive 2 dB, and give Squelch a small range. Measure energy above 3 kHz in 5 ms windows against total level before and after.
- **The stored macro value already moves the knob.** A macro on `cutoff_hz` at 0.5 with amount 0.5 raises the cutoff by 0.25 of its travel (exponential 20 Hz to 20 kHz, so position = log10(f / 20) / 3). Add base position, macro, envelope and velocity amounts: keep the peak under about 0.85, or the filter opens fully on every note.
- **Compressors make trim insensitive**; level heavily compressed presets with the compressor makeup (Steel Toe).
- **Level test notes:** a mono preset whose first module (id 1) is `midi.in` is played at C3, otherwise at E4. Basses put `midi.in` first (`Recipe.bass`).
- **Depth routes** (macro scales a route's amount, no VCA): `Rack::depth_route`. Used for vibrato, Pluck, Slap, Blat, Dive, Growl, Tide, Pulse.
- `clips.py` compares peaks below 16 kHz now; a bright saw's edges above it are not the encoder's doing.
- Unison is up to 16 on master now (PR #42); `osc.va` also has `curve`, `width`, `blend`, `restart` and an `out_r` (not used yet: all voices here are mono through a mono filter). Poly glide (PR #43) is not needed for basses and leads.
- Commands are refused in the worktree when compound (`&&`, heredocs, `sed 'Nr file'`): write scripts with the Write tool and run them as one plain command.

## Open decisions for Kosta

- Evidence folders still carry old slugs.
- Whether Afterglow should use the stereo `out_r` unison width (needs a stereo filter path; not done).

## Standing rules

Report to the session named "Kabl overseer"; do not merge; stop when done; caveman and ponytail at ultra in chat, normal English in code, commits and docs.
