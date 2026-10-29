# Factory presets: grooves

Fourth part of the bank (`BRIEF.md`): Groove to 14. Generator: `crates/ui/examples/presets_groove.rs` (same recipe as `presets_pad.rs`: a `Rack`, one function per preset, a `Preset` entry with metadata and a clip phrase). Each preset is a folder under `patches/presets/` and an entry in the `FACTORY` list of `crates/ui/tests/library.rs`. Patches are schema 8 (`Rack::migrated`).

A Groove is a playable, self-running patch: a clock, a `drum.grid` kit (pattern A is the groove, pattern B a fill the grid cues can switch to), a sequenced bass line and, where it suits, a sequenced chord part. **Run starts it.** MIDI plays a third voice over the top (a chord or lead voice that follows the same filter and pump). Space defaults follow the brief; no limiter, level by patch gain.

I can render and measure but not listen. Kosta's ears decide.

## Plan: 14 grooves

Signature share: 4 of 14. Each macro is named for what it does for that groove.

| Preset | Genre | BPM / swing | Character | What plays | Macros (CC 20 up) | Signature | Group |
|---|---|---|---|---|---|---|---|
| Copper Mile | French touch | 123 / 28 | filtered disco loop: saw chord stabs and a round bass, phaser, hard pump | kick, clap, hats; bass line; chord stabs; keys play stabs | Filter, Phase, Pump, Room | yes | 1 |
| Brick Dust | French touch | 126 / 25 | distorted, compressed bass stomp, blunt and dry | kick, snare, hats; overdriven bass; keys play bass | Filter, Dirt, Pump, Room | yes | 1 |
| Ore Cart | techno | 132 / 0 | rolling kick, rumble bass, off-beat hat, one stab | kick, hats, perc; bass; stab | Filter, Rumble, Pump, Space | no | 1 |
| Stoop Sale | hip hop | 86 / 55 | boom-bap: dusty kick and snare, sub bass, a minor-seventh chord | kick, snare, hats; sub bass; chord | Swing, Dust, Filter, Space | no | 1 |
| Fault Line | drum and bass | 174 / 0 | two-step breaks, reese bass | kick, snare, hats, ghost snares; reese | Reese, Filter, Hats, Space | no | 1 |
| Corner Store | funk | 108 / 40 | slap-style bass, tight kit, clav stabs | kick, snare, hats; bass; clav | Swing, Filter, Drive, Room | no | 1 |
| Glass Coast | synthwave | 100 / 0 | gated snare, arpeggio, long echo | kick, snare, tom, hats; arp; pad | Filter, Gate, Chorus, Space | no | 1 |
| Still Water | ambient | 68 / 0 | sparse soft perc, slow bass, bell chords | rare hats and a tom; bass; chord | Bloom, Pulse, Filter, Space | no | 1 |
| Slab City | trap | 140 / 0 | half-time kick and snare, 808 and rolling hats | kick, clap, hats with chance; 808 | Tone, Glide, Hats, Room | no | 2 |
| Rope Walk | French touch | 118 / 30 | disco bass and phased chord loop | kick, clap, hats; bass; chords | Filter, Phase, Pump, Room | yes | 2 |
| Lamp Oil | dub techno | 124 / 0 | deep chords through a long tempo echo | kick, rim, hat; sub; chord | Chord, Dub, Filter, Space | no | 2 |
| Pinball | modular | 120 / 0 | polymeter bleeps (rows of 5, 7, 9) | grid rows of different lengths; two seqs | Pitch, Tempo, Bleep, Space | no | 2 |
| Scrap Metal | electro, French touch | 128 / 20 | robot stomp: crushed drums and a talking filter | kick, snare, clap, hats; bass; formant stab | Vowel, Crush, Pump, Room | yes | 2 |
| Dune Buggy | UK garage | 130 / 58 | shuffled two-step with a sub and chopped chords | kick, snare, hats; sub; chords | Swing, Filter, Chop, Space | no | 2 |

Slugs checked against `origin/master`: none of the fourteen exists there.

## Group 1

Details (clips, tour start times, levels, macro evidence, checks) are added below as group 1 lands.
