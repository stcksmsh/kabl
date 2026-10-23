# Pilot presets

Eight presets that test the direction in `BRIEF.md` before the 100-preset batch. They use only modules on master (no module, engine, patch-format or schema change). They are written by `crates/ui/examples/presets_pilot.rs` to `patches/presets/<slug>/` and listed in the `FACTORY` list of `crates/ui/tests/library.rs`.

Kosta listened to the first version and gave a verdict per preset; the second round (this one) answers it. The author of the pilot can render and measure but not hear, so everything below about sound is what the signal chain is built to do and what the renders measure; Kosta's ears decide.

## The eight, spread across genres and roles

Three carry the French-touch signature (tagged `signature`), five cover other genres and roles. Each preset names its own macros after what they do (see `BRIEF.md`, "Macros"); they sit on CC 20 and up in the order shown.

| Preset | Role | Genre | Plays | Macros (CC 20, 21, 22, 23) |
|---|---|---|---|---|
| Cinder Block | Bass | French touch, signature | mono, short off-beat notes, mod wheel opens the filter | **Bite** ladder cutoff; **Grit** drive stage; **Room** slapback and tight room (0.25); **Crush** bitcrusher mix |
| Parquet Loop | Keys | disco, signature, 122 BPM swing | hold a chord, press Run: the filter closes on every beat; chord tones glide | **Cutoff** ladder cutoff; **Crunch** drive stage; **Space** dotted echo and tight room (0.25); **Glide** slide time |
| Loading Dock | Piece | filter house, signature, 124 BPM swing | press Run; five cues; a hand-played voice on top | **Sweep** ladder cutoff; **Drive** drive stage and bitcrusher; **Wash** noise riser, echo and room (0); **Pump** how hard the kick ducks bass and stabs (the sidechain ratio) |
| Glass Elevator | Keys | pop | poly FM electric piano, velocity | **Tine** FM index and a lowpass; **Grit** drive stage; **Space** ping-pong echo and room (0.60); **Chorus** mix and rate |
| Undertow | Bass | hip-hop, drum and bass | mono, legato glide, long low notes | **Cutoff** ladder cutoff; **Weight** drive stage; **Room** short room (0.30); **Beating** the two saws pull apart and the cutoff sweeps |
| Salt Flat | Pad | ambient, cinematic | poly, slow, open chords | **Bloom** ladder cutoff; **Saturation** a little drive; **Space** long echo and a 9 s room (0.65); **Texture** the staticky layer (default 0 = clean) |
| Boxwood | Pluck | mallet | poly FM mallet, velocity | **Brightness** FM index and a lowpass; **Warmth** a little drive; **Space** echo and a medium room (0.45); **Chorus** mix and rate |
| Neon Highway | Lead | synthwave | mono, legato glide | **Cutoff** ladder cutoff; **Bite** drive stage; **Space** dotted echo and room (0.60); **Vibrato** vibrato depth |

Loading Dock cues (CC 40-44, or the pads), as Sweep / Drive / Wash / Pump: Intro (0.12, 0.30, 0.00, 0.50), Build (0.90, 0.75, 0.65, 0.80, over 12 beats), Gap (0.08, 0.60, 0.85, 1.00, at once), Drop (0.78, 1.00, 0.10, 1.00, at once), Break (0.20, 0.25, 0.60, 0.30, over 4 beats). Cues set macros only; both sequencers play on.

## Round two: Kosta's verdict and what changed

| Preset | Verdict | What changed |
|---|---|---|
| Gravel Pit, now **Cinder Block** | Harsh, high, "wobbly" resonance, almost goofy; the sound underneath is decent. | The wobble was the Motion route: a 6.5 Hz LFO into the cutoff with the resonance at 0.5, on top of a filter envelope that swept through the resonance peak on every note. The LFO and its VCA are gone; resonance 0.5 to 0.3; the filter envelope amount 0.3 to 0.2. The dirt is untouched (18 dB of drive, the bitcrusher, the 8:1 compressor). Motion became **Crush**, so the bitcrusher has its own knob instead of riding on Drive. Renamed: a gravel pit is a place with movement; a cinder block is a blunt, grey, hard object. |
| Parquet Loop | Interesting; expected a fluid transition between notes. | `midi.in` glide is ALWAYS, so every voice slides from the last pitch it played; change chord and the chord tones slide into the next ones. The time sits on a macro, **Glide** (5 ms at 0, about 65 ms stored, about 800 ms at 1). The phaser is now fixed at a working mix (it was the Motion macro). |
| Basement Drop, now **Loading Dock** | Interesting but too tame; the name does not fit. | Harder everything: real drums from PR #38 (`drum.kick` four on the floor, `drum.clap` on the 2 and 4, `drum.hat` open on the off-beats), and the kick now feeds the `sc` input of a second compressor that ducks the whole music bus (threshold -34 dB, 1 ms attack, 220 ms release), so the pump is a real sidechain, not an envelope; the **Pump** macro is that compressor's ratio (1:1 at 0, about 19:1 at 1). Also 20 dB of drive (was 14), the first compressor at 8:1 and a lower threshold, bitcrusher up to 0.6 of its mix, and a pink-noise riser let in by the **Wash** macro together with a longer echo and a 1.2 s room (these were 0.2 and 0.5 s). A fifth cue, **Gap**, sucks everything shut for a bar before the Drop. The Build is 12 beats (was 16) and lands a bar before the Drop. Renamed: it is a warehouse-floor piece, not a basement. |
| Salt Flat | A static or hiss layer; without it, "truly angelic"; Kosta likes both. | The source is the shimmer voice: a second triangle oscillator two octaves up, mixed in at 0.2. Removing it alone cuts the energy above 2 kHz by 7 dB and turns a flat, noise-like high band into sparse partials (spectral flatness 8-16 kHz is similar, but the crest factor of the high band goes from 5 to 15); the wide unison spread (12 cents) and the 45 % chorus smear add to it. They are now one control: **Texture** brings in the shimmer voice (0 to 0.3 of the mixer), the chorus mix (18 to 48 %) and the unison spread (4 to 14 cents). **Default 0 = clean**, the angelic version: the pad is a landscape and the static is an effect on top of it. Texture 0.7 is about the old sound. The slow cutoff drift stays, at a fixed depth. |
| Glass Elevator, Undertow, Boxwood, Neon Highway | Good. | Sound unchanged; only the macros were renamed and relabelled. |

### Glide: default or macro?

On a macro, with a moderate stored value. The slide is a matter of taste (some players want chords that step cleanly), and in poly mode the allocator gives a new note the voice that was released longest ago, so a single note slides in from whatever that voice played last; that is a swoop, not a legato line. A macro lets the player take it to a clean step (Glide at 0 is a 5 ms slide) or push it to a long portamento. The stored 65 ms is short enough to read as a connected feel rather than an effect. If the bank needs a true legato keys sound, that is a mono or legato preset (Neon Highway, Undertow), not a poly one.

### Salt Flat: which default?

Clean. The preset is the bank's ambient pad: slow, wide and long; the static fights the idea of it, and the verdict calls the clean version "truly angelic". The grain is one move away (Texture up), and `salt-flat-textured.mp3` plays the same phrase at Texture 0.7.

## Round three: the static, the glide and the first half second

Kosta heard a static or fuzz in Cinder Block and in the clean Salt Flat, no glide in Parquet Loop and a strange noise in the first half second of Loading Dock. Each was tested with A/B renders (float WAV, `PP_SET` and `PP_SR` in `presets_pilot`, `scripts/clips.py` for the encoding).

| Suspect | Test | Result |
|---|---|---|
| Limiter (instant attack, hard clamp) | Ceiling raised to 0 dB, null against the normal render | Idle in six presets (null -240 dB). Active in Salt Flat and Glass Elevator on 8 samples at the ceiling: null -62 dB and -54 dB. Not audible; not the cause. |
| Aliasing | 48 kHz against 96 kHz render | Salt Flat: no change (above 8 kHz -53.9 against -53.1 dB). Cinder Block: 5 dB more content above 8 kHz at 48 kHz, all of it from the bitcrusher (without it, above 8 kHz is 18 dB lower). |
| Reverb, chorus | Space macro at 0 | Reverb and chorus change the sound but add no hiss; the high band is -54 dB in Salt Flat either way. |
| **The mp3 encoder** | Encode the WAV, decode, subtract | **At 192 kbps the encoding error is -46 dBFS RMS: 30 dB under the signal overall, 14 dB under it from 6 to 12 kHz and 7 dB from 12 kHz up.** That is broadband noise at the level of the pad's own high band. At 320 kbps the error is -72.5 dBFS (26 dB lower); VBR V0 is -62 dBFS. |
| 16-bit step, denormals | 16-bit truncation is about -98 dBFS; tails decay to digital silence | Too quiet to matter. |

**Cause: the clips were encoded at 192 kbps CBR**, and the error noise is what both presets share (Salt Flat is a dense wide pad, Cinder Block is bright and distorted). It is not a module or a preset problem. Fix in the render script: `scripts/clips.py` renders 32-bit float WAVs, encodes 320 kbps CBR, decodes each mp3 again and fails if the error is not at least 35 dB under the signal below 16 kHz or the peak moves by half a dB (at 320 kbps: Salt Flat 59 dB, Cinder Block 59 dB, Neon Highway 35.5 dB, the rest 42 to 57 dB). The limiter stays as it is.

Round four (Kosta, at normal volume: Salt Flat is good, Cinder Block still has some grit): Crush is now stored at 0.05 (was 0.3), which takes the content above 8 kHz from -45 to -59 dB (-63 with no crush at all) and leaves the dirt to the 18 dB of drive and the compressor; the macro keeps its full range. Earlier text: Cinder Block's bitcrusher (Crush stored at 0.3, so 18 % of the mix) added 18 dB of inharmonic content above 8 kHz, 5 dB of it aliasing. That is the dirt Kosta liked and also "static" by definition. The Crush macro takes it out; if the default should be lower, 0.15 halves it.

**Parquet Loop glide.** In poly mode a voice's first note never glides (`docs/sound-palette-batch/keyboard.md`: "each voice glides from the last pitch *it* played (a voice's first note does not glide)"), and the allocator gives a new note the free voice released longest ago. After load all eight voices are fresh, so the first eight notes (and the first chords) do not glide at all; after that each note slides in from whatever that voice played eight notes earlier. Measured on this patch with Glide at 1 (about 830 ms), an alternating C3 and C5 legato line: 200 ms after each note-on, notes 1 to 9 are already exactly on pitch (130.4 and 521.5 Hz); the first mid-slide reading is note 11 (265 Hz on its way from the C5 that voice played before). The preset's glide time is fine and the filter loop does not mask it. **This is a keyboard/allocator limitation, reported rather than worked around:** a poly glide that does what a player expects needs the allocator to take the voice whose last pitch is nearest the new note (or glide a fresh voice from the most recent pitch of any voice). A legato line is a mono preset today.

**Loading Dock first half second.** The pink-noise riser entered before the drive, so even Wash 0.15 became a loud, steady hiss (about -22 dBFS) that the drive's saturation held up; before Run there is nothing else to hear, which is why it stood out in the first half second. Now the riser joins after the drive, and Wash is 0 at the Intro (and stored at 0), so the preset is silent until Run and the riser grows with the Build from nothing.

### Defect note: poly glide (for the module fix)

- **What:** `midi.in` with `glide` ALWAYS in POLY mode does not give a player a legato slide. A voice's first note never glides, and the allocator hands a new note the free voice that was released longest ago, so the pitch a note slides from is the one that voice played several notes earlier, unrelated to the note just played.
- **Measured** (Parquet Loop, `glide_ms` driven to about 830 ms, alternating C3 and C5 played legato every 0.7 s, `PP_HITS` and `PP_SET=macro.m4=1`): 200 ms after each note-on, notes 1 to 9 are exactly on pitch (130.4 and 521.5 Hz); note 11 is mid-slide (265 Hz). All eight voices are fresh after load, so nothing glides until the pool has cycled.
- **Expected:** a new note slides from the pitch a player just heard: the most recent note (or the nearest held one), also on a fresh voice.
- **Fix to assign:** in the POLY allocator (`kabl_engine::keyboard`, rules in `docs/sound-palette-batch/keyboard.md`), glide a voice's first note from the last pitch any voice played, and prefer for a new note the free voice whose last pitch is nearest. Not worked around in the preset (no per-voice slew on pitch).

## Where a fifth macro would help

The macros module holds four (`MAX_OUTPUTS` is 4 per module; `claude/macros` lifts it), so the pilot stays at four. A fifth would help here, and no more than that:

- **Parquet Loop**: **Phase**, the phaser mix and rate, which lost its macro to Glide.
- **Loading Dock**: **Shake**, an LFO on the cutoff, and a separate **Riser** knob, which now shares Wash with the echo and the room.
- **Salt Flat**: **Drift**, the depth of the slow filter drift, now fixed.
- **Cinder Block**, **Glass Elevator**, **Undertow**, **Boxwood**, **Neon Highway**: no fifth needed.

## What changed from the first pilot

The first pilot (eight presets that read as synthwave) was Cherry Chainsaw, Neon Highway, Roller Disco, Violet Hour, Glass Elevator, Chrome Chase, Midnight Drop and Android Choir. The direction then changed twice: first to a harder French-touch character, then to a general-purpose bank with the French touch as one corner. The result was the round-one pilot: Gravel Pit (from Cherry Chainsaw), Parquet Loop (Roller Disco), Basement Drop (Midnight Drop), Glass Elevator, Neon Highway, and the new Undertow, Salt Flat and Boxwood. Round two is the table above.

Dropped: Violet Hour, Chrome Chase and Android Choir. Harder reworks of the arpeggio groove (a swung 124 BPM arpeggio over a pumped chord) and the voice (a talkbox) were tried during the rework and left out of the eight to make room for other genres; they are the first candidates for the bank.

## What the pilot says about the brief

- **Four fixed macros was wrong.** The first pilot held Filter, Drive, Space and Motion everywhere; Kosta's point is that the same four meanings are not what a sound needs. A bass wants Bite and Crush, a pad wants Texture, a piece wants Pump. The brief now says: each preset names its macros after what they do, four is typical, more is allowed but discouraged above four. Round two relabels all eight and swaps the fourth macro on five of them for something the sound actually owns (Crush, Glide, Pump, Texture and the renamed Motion ones).
- **Wet by default works, with a genre-appropriate default.** Mixes are 0 on the modules and driven by the space macro, so the stored value is the designed wet amount and the macro at 0 is a usable dry sound in all eight. Defaults run from 0 (Loading Dock, where Wash also brings in the riser) to 0.65 (Salt Flat).
- **A limiter on the output is part of the recipe.** With macros up, three presets peaked over 0 dBFS in the first pilot. Every preset ends in a `limiter` at -1.5 dB.
- **Heavy compression makes the gain stage insensitive.** Cinder Block and Loading Dock sit behind an 8:1 compressor; a trim gain changed their loudness by less than half a dB, so they are leveled with the compressor's makeup.
- **Modulation amounts add in the knob's travel and can saturate.** In the first Gravel Pit the filter envelope plus the Filter macro pushed the cutoff to the top of its range on every note. Keep the sum of a cutoff's routes under 1 at their peaks. The same amounts, with a high resonance, were also what made it "wobbly".
- **An LFO at a fixed depth is a macro that was not worth its slot.** The wobble's slot became Crush; Salt Flat's drift is now fixed and a fifth macro would hold it.
- **The signature rules are buildable.** Dirt (drive plus crusher plus compressor), pump (five modules), swing (the clock's own `swing`), a clock-locked filter loop (`lfo` with `sync`), a noise riser and a build-up on cues all work with existing modules.

## Gaps

Things kabl lacks that a preset wanted. Each is written down, not built.

1. **Sidechain solved by the drum voices.** The first pilot's pump was five modules (a divider, an envelope, a VCA, an attenuverter, a VCA); the kick into the compressor's `sc` input replaces them. The kick sits beside the music (after the echo and room), so it is not ducked by itself. Cinder Block and Parquet Loop are not drummed; they would take a kick the same way.
2. **A macro cannot scale a route or an LFO depth.** A fade-in LFO costs two modules (`lfo` into a VCA whose gain the macro routes). A macro target on a route's amount, or a depth knob on `lfo`, would make it one cable.
3. **Unison is capped at four voices.** A true supersaw would sound bigger for fewer modules.
4. **No stereo-in echo.** `delay` takes one input; the stereo chain taps the left side into it and adds its stereo output at the room.
5. **The arpeggiator has no accent or pattern.**
6. **No genre field.** Genre lives in the tags; the metadata would need a field (a schema change, so not done here).
7. **Vocoder.** The voice presets gain it when the module lands.
8. **Cue pieces are leveled at their first cue.** The level test renders a piece at its stored macro values (the Intro); the Drop is a few dB louder than that.
9. **Poly glide is voice-by-voice and not on the first notes (see round three).** A poly keys sound that glides like a legato line (the new note slides from the last note played, not from the voice that happened to be free) needs the allocator to prefer the voice that played the nearest pitch. Not changed here.
10. **Stepped parameters cannot be routed.** The glide mode (OFF, ALWAYS, LEGATO) is stepped, so a macro can set the glide time but not switch glide on; Glide at 0 is a 5 ms slide, not off.

Changes outside the presets: `CATEGORIES` in `crates/ui/src/library.rs` gains **Groove**, **Voice** and **Pluck**. It is a list of names for the browser and Save As; the patch format is unchanged.

## For Kosta: clips and capture

Clips are in `docs/presets/clips/`, one per preset, played the way it is meant to be used (a short MIDI phrase through the real engine; the self-playing preset running). The clips play the stored macro values: no macro is moved during a clip except by the cues in Loading Dock. What changed and what to listen for:

| Clip | What changed | Listen for |
|---|---|---|
| `cinder-block.mp3` | No wobble, lower resonance, the dirt unchanged. | A steady, blunt bass with the filter snap on each note and nothing moving on its own. |
| `parquet-loop.mp3` | Chord tones now slide into each other. | The filter closing on the beat; at the chord changes (about 3.9 s and 7.7 s) the tones slide instead of stepping. `parquet-loop-no-glide.mp3` is the same with Glide at 0. |
| `loading-dock.mp3` | Real kick, clap and hats; the kick sidechains the music; 20 dB of drive; the riser; the Gap cue. | The kick under everything, the music ducking on every beat (hardest in the Drop), the Build rising from about 4 s with the riser, the gap at about 11 s, the Drop at 12 s, the Break from 20 s. |
| `glass-elevator.mp3` | Unchanged sound. | The tine on the attack against the mellow tail. |
| `undertow.mp3` | Unchanged sound. | The slow beating and the weight of the sub; the glide on the overlapped notes. |
| `salt-flat.mp3` | Clean by default (Texture 0). | The slow bloom of each chord and the long tail, with no static. `salt-flat-textured.mp3` is the same phrase at Texture 0.7. |
| `boxwood.mp3` | Unchanged sound. | The click and the wooden ring of each note. |
| `neon-highway.mp3` | Unchanged sound. | The glide between overlapped notes and the echo trailing each phrase. |

`img/sounds-pilot.png` and `img/sounds-pilot-selected.png` are from round one and show the old names; regenerate them from the Sounds browser when the names are final. `evidence/standalone/` has the standalone Rack and Perform views of every preset with a chord held.

## Evidence

Regenerate with `presets_pilot` (`write`, `clips`, `macros`, `phrases`; `PP_F32`, `PP_SR`, `PP_SET` and `PP_HITS` for A/B renders), `scripts/clips.py`, `scripts/standalone.py` and `scripts/host.py`.

- **Levels** (`level_table`, 24 s render of each preset's own demonstration), peak / loudness in dBFS: Boxwood -2.2 / -20.3, Cinder Block -7.3 / -19.2, Glass Elevator -1.8 / -21.0, Loading Dock -7.3 / -18.3, Neon Highway -8.0 / -19.5, Parquet Loop -3.9 / -20.0, Salt Flat -1.5 / -19.9, Undertow -13.1 / -20.3. All within 2.0 dB of -19; `factory_levels_hold_the_target` passes.
- **Macros at the extremes** (`presets_pilot macros`, cues not launched): with every macro at 0 or at 1 no preset exceeds -1.5 dBFS and all renders are finite.
- **Salt Flat static** (measured on the clip, mixed to mono): without the shimmer voice the energy above 2 kHz falls 6.8 dB (-43.0 to -49.8 dBFS RMS) and the crest factor of that band rises from 5.3 to 14.8; chorus mix, unison spread and the reverb change it by a dB or less each. A pure sine instead of the triangle makes the high band tonal (spectral flatness 0.7 against 0.03).
- **Glide** (Parquet Loop clip with Glide at 0.5 against 0): the renders are bit-identical until the first chord change (about 3.9 s), differ by about -20 dB from there.
- **Lossless pair for listening:** `clips/salt-flat.flac` and `clips/cinder-block.flac` are 24-bit FLAC of the same float renders as the 320 kbps mp3s next to them.
- **Clips** (`scripts/clips.py`): 320 kbps mp3 decoded against the float WAV, error 54 to 78 dBFS RMS, 35 to 59 dB under the signal below 16 kHz, peaks within 0.1 dB.
- **Audio rule:** standalone and REAPER runs play into the silent PipeWire sink `kabl_agents` (`PIPEWIRE_NODE`, `PULSE_SINK` are set in both scripts); check `pactl list sink-inputs` after starting; prefer offline renders. The round-three host and standalone runs were made before this rule reached me and played on the real output.
- **Standalone** (`scripts/standalone.py`, release build, real audio device, real-time priority granted): all eight open, take MIDI from a virtual controller and stay running; Rack and Perform captures in `evidence/standalone/`.
- **REAPER 7.75** (`scripts/host.py`, CLAP plugin, `evidence/host/result.json`): per preset a project with the plugin and a MIDI item playing the clip phrase (the piece also gets its five cue buttons). Saved, fully quit, reopened and saved: the plugin state is identical in all eight; the render after reopen equals the first (relative difference 0.0); every render is finite and not silent. Without the MIDI item the seven keyboard presets render silent (RMS 0.0); Loading Dock plays by itself with the transport running in the host. (Rebuild the plugin with `cargo build --release -p kabl-clap` first: a stale `libkabl_clap.so` loads a drum patch as the default sound.)
- **Tests**: `every_preset_keeps_to_the_brief` (`crates/ui/tests/presets.rs`: one macro module, each named macro on CC 20 and up and pinned in order, play flags, category, tags, a description naming every macro), `every_factory_sound_is_discoverable_categorized_and_loads` and `factory_levels_hold_the_target` pass.
- **Not done**: nobody has listened to round two; other hosts; the CLAP validator and cargo-deny run in CI only.
