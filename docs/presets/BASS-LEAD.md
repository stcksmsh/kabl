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
| Soft Shoe | Bass | pop | no | 1 |
| Satin Ribbon | Lead | pop | no | 1 |
| Rust Belt | Lead | French touch | yes | 1 |
| Hot Wire | Lead | house, techno | no | 1 |
| Paper Crane | Lead | ambient | no | 1 |

The rest (group 2) is planned as: bass Rubber Band (funk), Slate Roof (neuro, drum and bass), Gutter (French touch, signature), Low Tide (ambient sub), Copper Wire (experimental FM), Tail Light (synthwave), Rail Yard (house), Oak Stave (plucked, other); lead Glass Needle (modular), Loud Mouth (French touch talking lead, signature), Dust Devil (trap), Afterglow (synthwave), Horn Section (funk and disco), Fire Escape (house), Short Circuit (French touch, signature). Signature share: 3 of 14 basses and 3 of 12 leads.

## Group 1

Clips in `docs/presets/clips/bass-lead/group1/` (`<slug>.flac` and `<slug>.mp3` at 320 kbps, `tour.flac`). Tour start times (one second of silence between clips): acid-rain 0, hot-wire 10, paper-crane 20, rust-belt 35, satin-ribbon 45, soft-shoe 55, steel-toe 65, tar-pit 75 seconds.

| Preset | Macros (CC 20 up) | Listen for |
|---|---|---|
| Steel Toe | Bite, Grit, Pump, Room | The bass snapping shut on every kick and swelling back; Pump at 0 plays flat, at 1 pumps hard. Press Run for the kick. |
| Tar Pit | Tone, Dirt, Slide, Room | The thump on each attack, the long sine body and the slide between notes; Dirt up for grit on small speakers. |
| Acid Rain | Cutoff, Squelch, Space, Drive | The filter snapping on each note and the slides between overlapped ones; Squelch up for the bite. |
| Soft Shoe | Tone, Pluck, Grit, Room | The clean round low end; Pluck up adds a snap at the front of each note. |
| Satin Ribbon | Tone, Edge, Space, Vibrato | The even vocal tone and the glide between overlapped notes; Vibrato up for the long notes. |
| Rust Belt | Cutoff, Grit, Crush, Room | The blunt attack and crushed edge; Crush up for the digital grind, down for a plain hard lead. |
| Hot Wire | Cutoff, Squelch, Space, Pulse | The filter snap, the slides and the dotted echo; Pulse up for a slowly moving thickness. |
| Paper Crane | Tone, Swell, Space, Vibrato | Each note blooming out of silence and the long tail carrying into the next. |

Depth routes: Soft Shoe's Pluck scales the filter envelope's depth; the vibratos (Satin Ribbon, Paper Crane) and Hot Wire's Pulse scale an LFO's depth, with no VCA in between.

### Checks (group 1)

- `every_preset_keeps_to_the_brief`, the library tests and `factory_levels_hold_the_target` pass.
- Levels (`presets_bass_lead levels patches`), peak / loudness dBFS: Steel Toe -10.0 / -21.9, Tar Pit -13.7 / -20.5, Acid Rain -10.9 / -20.5, Soft Shoe -11.4 / -20.1, Satin Ribbon -11.0 / -21.3, Rust Belt -10.9 / -20.0, Hot Wire -8.4 / -21.3, Paper Crane -9.0 / -22.1.
- Macros (`presets_bass_lead macros patches`): every macro moves the render (at least 23 dB above the null; most 4 to 15 dB), all renders finite, peaks under -1.5 dBFS with every macro at 0 or 1 except Steel Toe at all 0 (-2.7 dBFS, limiter ceiling is -1.5).
- Cutoff headroom: a first version opened the ladder to the top of its range on every note (the stored macro value already adds travel); fixed, above 8 kHz is under 1 % of the energy in all eight except Hot Wire (0.65 %).
- clips: 320 kbps mp3 decoded against the float render, 40 to 82 dB under the signal below 16 kHz, peaks within 0.1 dB (peaks are compared below 16 kHz now; a bright saw's edges above it are not the encoder's doing).
- REAPER 7.75 (`scripts/host.py`, evidence in `evidence/host-bass-lead/`): all eight saved, fully quit, reopened, identical state, identical render; the MIDI is what plays them (Steel Toe keeps its kick). Standalone (`scripts/standalone.py`, evidence in `evidence/standalone-bass-lead/`): all eight open and stay running with real-time priority. Both ran on the silent sink `kabl_agents`.

## Gaps

- Unison is capped at four voices on master (PR #42 lifts it); poly glide is not needed here (all voices are mono).
