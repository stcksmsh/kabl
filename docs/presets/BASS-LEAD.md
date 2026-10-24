# Factory presets: basses and leads

First part of the bank (`BRIEF.md`): Bass to 14 and Lead to 12, with the pilot's Cinder Block, Undertow and Neon Highway counting. Generator: `crates/ui/examples/presets_bass_lead.rs` (same recipe as the pilot, one `Recipe` for the mono voice and one function per preset). Each preset is a folder under `patches/presets/` and an entry in the `FACTORY` list of `crates/ui/tests/library.rs` (one contiguous block after Neon Highway).

I can render and measure but not listen. Everything below about sound is what the chain is built to do plus what the renders show; Kosta's ears decide.

## Plan: 14 basses, 12 leads

| Preset | Role | Genre | Signature | Group |
|---|---|---|---|---|
| Cinder Block | Bass | French touch | yes | pilot |
| Undertow | Bass | drum and bass, hip-hop | no | pilot |
| Neon Highway | Lead | synthwave | no | pilot |
| Steel Toe | Bass | French touch, 124 BPM kick sidechain | yes | 1 |
| Tar Pit | Bass | trap 808 | no | 1 |
| Acid Rain | Bass | house, techno | no | 1 |
| Oatmeal | Bass | pop | no | 1 |
| Spun Sugar | Lead | pop | no | 1 |
| Pig Iron | Lead | French touch | yes | 1 |
| Hot Wire | Lead | house, techno | no | 1 |
| Lantern | Lead | ambient | no | 1 |

The rest (group 2) is planned as: bass Rubber Band (funk), Slate Roof (neuro, drum and bass), Gutter (French touch, signature), Low Tide (ambient sub), Copper Wire (experimental FM), Tail Light (synthwave), Rail Yard (house), Oak Stave (plucked, other); lead Glass Needle (modular), Loud Mouth (French touch talking lead, signature), Dust Devil (trap), Afterglow (synthwave), Horn Section (funk and disco), Fire Escape (house), Short Circuit (French touch, signature). Signature share: 3 of 14 basses and 3 of 12 leads.

## Group 1

Clips in `docs/presets/clips/bass-lead/group1/` (`<slug>.flac` and `<slug>.mp3` at 320 kbps, `tour.flac`). Tour start times (one second of silence between clips): acid-rain 0, hot-wire 10, lantern 20, oatmeal 35, pig-iron 45, spun-sugar 55, steel-toe 65, tar-pit 75 seconds.

| Preset | Macros (CC 20 up) | Listen for |
|---|---|---|
| Steel Toe | Bite, Grit, Pump, Room | The bass snapping shut on every kick and swelling back; Pump at 0 plays flat, at 1 pumps hard. Press Run for the kick. |
| Tar Pit | Tone, Dirt, Slide, Room | The thump on each attack, the long sine body and the slide between notes; Dirt up for grit on small speakers. |
| Acid Rain | Cutoff, Squelch, Space, Drive | The filter snapping on each note and the slides between overlapped ones; Squelch up for the bite. |
| Oatmeal | Tone, Pluck, Grit, Room | The clean round low end; Pluck up adds a snap at the front of each note. |
| Spun Sugar | Tone, Edge, Space, Vibrato | The even vocal tone and the glide between overlapped notes; Vibrato up for the long notes. |
| Pig Iron | Cutoff, Grit, Crush, Room | The blunt attack and crushed edge; Crush up for the digital grind, down for a plain hard lead. |
| Hot Wire | Cutoff, Squelch, Space, Pulse | The filter snap, the slides and the dotted echo; Pulse up for a slowly moving thickness. |
| Lantern | Tone, Swell, Space, Vibrato | Each note blooming out of silence and the long tail carrying into the next. |

Depth routes: Oatmeal's Pluck scales the filter envelope's depth; the vibratos (Spun Sugar, Lantern) and Hot Wire's Pulse scale an LFO's depth, with no VCA in between.

### Checks (group 1)

- `every_preset_keeps_to_the_brief`, the library tests and `factory_levels_hold_the_target` pass.
- Levels (`presets_bass_lead levels patches`), peak / loudness dBFS: Steel Toe -10.0 / -21.9, Tar Pit -13.7 / -20.5, Acid Rain -10.9 / -20.5, Oatmeal -11.4 / -20.1, Spun Sugar -11.0 / -21.3, Pig Iron -10.9 / -20.0, Hot Wire -8.4 / -21.3, Lantern -9.0 / -22.1.
- Macros (`presets_bass_lead macros patches`): every macro moves the render (at least 23 dB above the null; most 4 to 15 dB), all renders finite, peaks under -1.5 dBFS with every macro at 0 or 1 except Steel Toe at all 0 (-2.7 dBFS, limiter ceiling is -1.5).
- Cutoff headroom: a first version opened the ladder to the top of its range on every note (the stored macro value already adds travel); fixed, above 8 kHz is under 1 % of the energy in all eight except Hot Wire (0.65 %).
- clips: 320 kbps mp3 decoded against the float render, 40 to 82 dB under the signal below 16 kHz, peaks within 0.1 dB (peaks are compared below 16 kHz now; a bright saw's edges above it are not the encoder's doing).
- REAPER 7.75 (`scripts/host.py`, evidence in `evidence/host-bass-lead/`): all eight saved, fully quit, reopened, identical state, identical render; the MIDI is what plays them (Steel Toe keeps its kick). Standalone (`scripts/standalone.py`, evidence in `evidence/standalone-bass-lead/`): all eight open and stay running with real-time priority. Both ran on the silent sink `kabl_agents`.

## Round two (Kosta's first listen)

Acid Rain was too resonant with high clicks; Hot Wire the same, less. Measured as energy above 3 kHz in 5 ms windows: Acid Rain peaked 16 dB under the total level at default, now 22 dB lower than before. Causes: 1 ms attacks on the filter envelope (the cutoff jumped and the resonance rang), ladder drive plus the drive stage feeding the resonance, and a resonance of 0.40. Now: filter attack 6 ms (Hot Wire 4), amp attack 3 ms, resonance 0.28 (Hot Wire 0.32), ladder drive 2 dB, drive stage 7 dB (Acid Rain), a smaller envelope and velocity sweep, and a smaller Squelch range (at Squelch 1 Acid Rain is still 10 dB quieter above 3 kHz than the old default). Slides are untouched.

## Group 2

Eight basses (Rubber Band, Slate Roof, Gutter, Low Tide, Copper Wire, Tail Light, Rail Yard, Oak Stave) and seven leads (Glass Needle, Loud Mouth, Dust Devil, Afterglow, Horn Section, Fire Escape, Short Circuit); clips and tour in `clips/bass-lead/group2/`, tour start times printed by `clips.py` (alphabetical order, one second of silence between).

### Checks (group 2)

- `every_preset_keeps_to_the_brief`, the library, presets and factory_bank tests pass.
- Levels, peak / loudness dBFS: Rubber Band -9.3 / -21.8, Slate Roof -11.9 / -19.3, Gutter -7.3 / -16.5, Low Tide -12.2 / -21.7, Copper Wire -13.9 / -21.0, Tail Light -10.2 / -21.4, Rail Yard -8.9 / -21.4, Oak Stave -5.6 / -21.9, Glass Needle -9.9 / -20.9, Loud Mouth -13.9 / -22.1, Dust Devil -14.1 / -21.6, Afterglow -7.9 / -21.7, Horn Section -12.1 / -21.1, Fire Escape -9.9 / -21.6, Short Circuit -11.6 / -21.4. Loud Mouth sits at the edge of the -19 +- 3 band (its compressor makes trim insensitive).
- Macros: every macro moves the render. Weakest after a fix: Oak Stave Pluck (-24 dB under the render; the first version was -31) and Slate Roof Crush (-20 dB; first version -37, bitcrusher now 6 bits, downsample 4). A Pluck that is only a 240 ms filter snap on a triangle stays subtle by nature.
- Clicks: nobody on this side can listen. Energy above 3 kHz in 5 ms windows, flagged where it jumps 10 dB while total level rises under 6 dB: none in 10 of 15. Gutter (bitcrusher plus 24 dB drive), Rubber Band (the slap), Slate Roof (14 dB drive), Rail Yard (hats and clap) and Tail Light show flagged windows; the same measure flags Steel Toe and Acid Rain in group 1, so it separates grit from clicks poorly. These five need Kosta's ears.
- clips: 320 kbps mp3 against the float render, all 15 pass (SNR 41 to 82 dB below 16 kHz, peaks within 0.1 dB).
- REAPER (`scripts/host.py`, evidence in `evidence/host-bass-lead-2/`): all 15 saved, quit, reopened with identical state and identical render; nothing plays without MIDI except Rail Yard's own drum grid (exempted in `host.py` like Steel Toe). The standalone check (`scripts/standalone.py`) was not run for group 2.

## Gaps

- Unison is capped at four voices on master (PR #42 lifts it); poly glide is not needed here (all voices are mono).
