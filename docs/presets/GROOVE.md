# Factory presets: grooves

Fourth part of the bank (`BRIEF.md`): Groove to 15 (14 new grooves plus Loading Dock, moved from the pilot set). Generator: `crates/ui/examples/presets_groove.rs` (same recipe as `presets_pad.rs`: a `Rack`, one function per preset, a `Preset` entry with metadata and a clip phrase). Each preset is a folder under `patches/presets/` and an entry in the `FACTORY` list of `crates/ui/tests/library.rs`. Patches are schema 8 (`Rack::migrated`).

A Groove is a playable, self-running patch: a clock, a `drum.grid` kit (pattern A is the groove, pattern B a fill the grid cues can switch to), a sequenced bass line and, where it suits, a sequenced chord part. **Run starts it.** MIDI plays a third voice over the top (a chord or lead voice that follows the same filter and pump). Space defaults follow the brief; no limiter, level by patch gain.

I can render and measure but not listen. Kosta's ears decide.

## Plan: 14 grooves

Signature share: 4 of 14. Each macro is named for what it does for that groove.

| Preset | Genre | BPM / swing | Character | What plays | Macros (CC 20 up) | Signature | Group |
|---|---|---|---|---|---|---|---|
| Copper Mile | French touch | 123 / 28 | filtered disco loop: saw chord stabs and a round bass, phaser, hard pump | kick, clap, hats; bass line; chord stabs; keys play stabs | Sweep, Swirl, Pump, Room | yes | 1 |
| Brick Dust | French touch | 126 / 25 | distorted, compressed bass stomp, blunt and dry | kick, snare, hats; overdriven bass; keys play bass | Grind, Squash, Push, Slap | yes | 1 |
| Ore Cart | techno | 132 / 0 | rolling kick, rumble bass, off-beat hat, one stab | kick, hats, perc; bass; stab | Filter, Rumble, Pump, Space | no | 1 |
| Stoop Sale | hip hop | 86 / 55 | boom-bap: dusty kick and snare, sub bass, a minor-seventh chord | kick, snare, hats; sub bass; chord | Swing, Dust, Filter, Space | no | 1 |
| Fault Line | drum and bass | 174 / 0 | two-step breaks, reese bass | kick, snare, hats, ghost snares; reese | Reese, Filter, Hats, Space | no | 1 |
| Corner Store | funk | 108 / 40 | slap-style bass, tight kit, clav stabs | kick, snare, hats; bass; clav | Swing, Filter, Drive, Room | no | 1 |
| Glass Coast | synthwave | 100 / 0 | gated snare, arpeggio, long echo | kick, snare, tom, hats; arp; pad | Filter, Gate, Chorus, Space | no | 1 |
| Still Water | ambient | 68 / 0 | sparse soft perc, slow bass, bell chords | rare hats and a tom; bass; chord | Bloom, Pulse, Filter, Space | no | 1 |
| Slab City | trap | 140 / 0 | half-time kick and snare, 808 and rolling hats | kick, clap, hats with chance; 808 | Tone, Glide, Hats, Room | no | 2 |
| Rope Walk | French touch | 118 / 30 | disco bass and phased chord loop | kick, clap, hats; bass; chords | Filter, Phase, Pump, Room | yes | 2 |
| Lamp Oil | dub techno | 124 / 0 | deep chords through a long tempo echo | kick, rim, hat; sub; chord | Chord, Dub, Filter, Space | no | 2 |
| Acid Line | acid house | 124 / 16 | squelchy 303-style bass over a 909-style kick, clap and offbeat open hats | kick, clap, hats, open hat; resonant saw bass with accents and slides | Cutoff, Resonance, Accent, Slide | no | 2 |
| Scrap Metal | electro, French touch | 128 / 20 | robot stomp: crushed drums and a talking filter | kick, snare, clap, hats; bass; formant stab | Vowel, Crush, Pump, Room | yes | 2 |
| Dune Buggy | UK garage | 130 / 58 | shuffled two-step with a sub and chopped chords | kick, snare, hats; sub; chords | Swing, Filter, Chop, Space | no | 2 |

Slugs checked against `origin/master`: none of the fourteen exists there.

Loading Dock (filter house, 124 / 28 swing, Sweep, Drive, Wash, Pump) moved here from the pilot set after #62: it keeps its drums (kick, clap, open hats on sequencer lanes, the kick sidechaining the music) and five cues, so it is a Groove that plays itself on Run. Its generator stays in `presets_pilot.rs`; category Groove, clip `clips/loading-dock.*`. That makes 15 grooves.

## Group 1

Clips in `docs/presets/clips/groove/group1/` (`<slug>.flac`, `<slug>.mp3` at 320 kbps, `tour.flac` with a 20 ms fade at both ends of each clip). Tour start times (one second of silence between clips): brick-dust 0, copper-mile 17, corner-store 34, fault-line 51, glass-coast 68, ore-cart 85, still-water 102, stoop-sale 127 seconds. Each clip is the groove running with a short MIDI riff on top.

| Preset | Macros (CC 20 up) | Listen for |
|---|---|---|
| Copper Mile | Sweep, Swirl, Pump, Room | The loop ducking on every kick; Swirl up for a deeper phaser on the stabs. |
| Brick Dust | Grind, Squash, Push, Slap | Blunt distorted bass hits locked to the kick; Grind up for a crushed bite. |
| Ore Cart | Filter, Rumble, Pump, Space | The bass rolling between kicks; Rumble up for more low end. |
| Stoop Sale | Swing, Dust, Filter, Space | The late, loose off-beats; Swing up to drag them, Dust up for vinyl hiss. |
| Fault Line | Reese, Filter, Hats, Space | The detuned bass under the break; Hats up for a busier top. |
| Corner Store | Swing, Filter, Drive, Room | Bass and kick on swung 16ths; Swing up for more shuffle. |
| Glass Coast | Filter, Hats, Chorus, Space | The arpeggio over the pulsing bass; Chorus up for width. |
| Still Water | Bloom, Pulse, Weight, Space | Rare soft hits that never repeat exactly; Pulse up for more of them. |

### Checks (group 1)

- `every_preset_keeps_to_the_brief`, the library tests and `factory_levels_hold_the_target` pass (peak under -1 dBFS with every macro up, loudness in the window).
- Levels as `level_table` measures them (peak / loudness dBFS): Brick Dust -3.9 / -20.3, Copper Mile -6.9 / -21.2, Corner Store -3.2 / -18.2, Fault Line -2.2 / -18.0, Glass Coast -3.6 / -19.0, Ore Cart -5.6 / -20.5, Still Water -8.5 / -22.6, Stoop Sale -3.3 / -18.3.
- Macros (`presets_groove macros patches 1`, dB of the difference render under the render): the weakest are Stoop Sale Dust -18, Glass Coast Hats -24, Brick Dust Dirt -19, Still Water Pulse -19; the rest are -12 dB or louder. A route into a mixer level adds to the stored value and stops at 1, so Bass and Hats store a lower base level.
- Stoop Sale Dust is a hiss layer, not the bitcrusher: with the bitcrusher at a mix above 0 the REAPER render after reopen differed from the first.
- clips: 320 kbps mp3 against the float render, all eight pass.
- `audio_defects.py`: clean for all eight (no CLICK, PINNED or NONFINITE; HF_BURST advisories only). The first scan flagged CLICK in six clips: the drum voices began every hit from silence with a one-sample step (`drums.rs`, `Declick`: "a hit from silence is left alone, so its attack stays sharp"), and the scanner flags a step only when the rest of its 50 ms window is quiet, which is why the flags sat at fixed clip times (0.5 + 2.5k s in Ore Cart: 22 sixteenths at 132 bpm land on a block boundary). Muting layers pinned it to the drums (MIDI, space, pump, compressor, drive, echo and reverb off: same six flags; hats off: three left). Fix in the engine: a hit from silence now rises over a 0.4 ms raised-cosine ramp; retriggers keep the offset declick. Measured on the isolated voices (first 20 ms of a default hit): peak unchanged for all five voices; energy above 5 kHz changes by -2.8 dB (kick), -10 dB (tom), -0.5 dB (hat), -0.2 dB (clap), -0.07 dB (snare). The kick and tom figures are the step's own splatter (their energy above 5 kHz is 12 and 11 dB against 45 dB for the snare); a 0.3 ms ramp would still change them by -2.3 and -7 dB.
- REAPER (`host.py` with `PH_SELF=1`, evidence in `evidence/host-groove/`): all eight save, quit, reopen with identical state. The render after reopen is identical for six; Ore Cart and Stoop Sale differ in about half the runs, because the free-running clock starts at another phase in each host run (relative difference 0.8 to 1.4); for self-playing patches the check compares loudness (within 1.5 dB) instead of samples. `host.py` also writes plugin state version 5 now (it wrote 2, which the plugin migrates, so state compared unequal).
- Standalone (`standalone.py`, evidence in `evidence/standalone-groove/`): all eight open, take a chord from a virtual MIDI controller and stay running, audio on the silent sink.

### Harshness (group 1)

Corner Store had a narrow tone at 3111 Hz, 23 dB above the spectrum around it: the 5th harmonic of the D#5 clav stab (a 30 % pulse an octave too high; layer muting pinned it to the stab part, the bus filter macro did not move it). The clav stab now sits an octave lower with a 1.9 kHz low-pass, and the MIDI voice an octave lower. Glass Coast's arp had the same build-up (2625 Hz, 5th harmonic of the arp): it is now an octave lower, four-voice, with a sub, a 2.4 kHz low-pass and longer notes, plus a shorter gated snare and a longer echo and room.

| Preset | Strongest peak above 2 kHz before / after (level dB, prominence dB) | 2-5 kHz energy / total before / after (dB) |
|---|---|---|
| Corner Store | 3111 Hz -52.8, 23.2 / 2487 Hz -61.8, 13.6 | -16.8 / -20.4 |
| Glass Coast | 2625 Hz -56.4, 16.5 / 2101 Hz -61.5, 9.2 | -14.1 / -19.6 |
| Brick Dust, Copper Mile, Fault Line, Ore Cart, Stoop Sale | unchanged: no peak above 8.5 dB prominence, level under -65 dB | -22.9, -24.6, -19.5, -28.1, -26.8 |
| Still Water | 2353 Hz -62.5, 29.0 (unchanged; a quiet bell partial) | -25.8 |

With every macro at 1 Corner Store's strongest peak is 2180 Hz at -57 dB (it was 3111 Hz at -49.5 dB with Drive up).

## Group 2

Clips in `docs/presets/clips/groove/group2/` (`<slug>.flac`, `<slug>.mp3` at 320 kbps, `tour.flac` with a 20 ms fade at both ends of each clip). Tour start times (one second of silence between clips): acid-line 0, dune-buggy 17, lamp-oil 34, rope-walk 51, scrap-metal 70, slab-city 87 seconds. Each clip is the groove running with a short MIDI riff on top.

| Preset | Macros (CC 20 up) | Listen for |
|---|---|---|
| Slab City | Tone, Glide, Hats, Room | The sine 808 sliding between its notes under a dark, soft pad; Glide up for longer slides, Hats up for busier rolls. |
| Rope Walk | Filter, Phase, Pump, Room | The octave bass walking under phased stabs; Phase up for swirl, Pump up for a harder duck. |
| Lamp Oil | Chord, Dub, Filter, Space | The muted chord stab repeating into a dotted echo; Dub up for a longer tail. |
| Acid Line | Cutoff, Resonance, Accent, Slide | The bass squelching on the accented notes and sliding into the marked ones; Resonance up for the squeal, Accent up for wider opening on the accents, Slide up for longer glides. |
| Scrap Metal | Vowel, Crush, Pump, Room | The stab changing vowel; Crush up for grit. |
| Dune Buggy | Swing, Filter, Chop, Space | Shuffled hats and chopped chords; Chop down for shorter stabs. |

New generator pieces (no engine or module change): a `slew` on the 808's pitch for Glide, a `filter.formant` on Scrap Metal's stab (Vowel sweeps it), per-row lengths on the drum grid and a per-part `length` on the sequencers for Pinball's polymeter, a tempo-synced dotted-eighth echo on the chord part alone for Lamp Oil's Dub, and macro routes into the clock's `bpm`, the oscillators' `fine` and the chord sequencer's `gate_len` (Tempo, Pitch, Chop). A macro that moves a base value stores the base that far below the design (for example Tempo stores 120.3 minus the route, so the stored macro lands on 120.3 bpm). Chop needs the sequencer's gate mode set to LENGTH: in CLOCK mode `gate_len` does nothing (this also means the gate lengths of the group 1 parts are not in effect).

### Checks (group 2)

- `every_preset_keeps_to_the_brief`, the library tests and `factory_levels_hold_the_target` pass (debug).
- Levels as `level_table` measures them (peak / loudness dBFS): Slab City -4.0 / -21.3, Rope Walk -6.2 / -21.9, Lamp Oil -4.5 / -19.9, Acid Line -3.1 / -19.8, Scrap Metal -5.2 / -21.8, Dune Buggy -2.9 / -18.0.
- Macros (`presets_groove macros patches 2`: peak with every macro at 0 and at 1, and dB of the difference render under the render for each macro alone). All peaks are under -1 dBFS: all at 0 / all at 1 are -5.8 / -4.3 (Slab City), -1.5 / -5.9 (Rope Walk), -4.4 / -3.5 (Lamp Oil), -3.1 / -3.1 (Acid Line), -1.5 / -5.2 (Scrap Metal), -2.6 / -2.6 (Dune Buggy). Weakest macros: Slab City Hats -19.4, Scrap Metal Vowel -18.2 and Crush -19.0, Rope Walk Room -17.4; every other macro is -16 dB or louder. Slab City Hats is limited by the hat voice itself (more hat level did not move it), so the hats got louder velocities and the 808 got quieter.
- `audio_defects.py` on the clips: clean for all six (HF_BURST advisories only), no new allow entry. The three tempos were first set a fraction of a bpm off the round number (140.4, 120.3, 130.3) because the scan flagged CLICKs in Dune Buggy (two), Pinball (one) and Slab City (one) at round tempos. Re-checked after the drum crossfade (#64): the engine is clean. The scan was the cause. It reads fixed 50 ms windows, and at round tempos a hit lands right at a window boundary (3.5 s and 6.5 s in Dune Buggy, 2.152 s in Pinball), so the window holds the attack and its tail and nothing before it, and a 0.4 ms attack looks like an isolated spike. Shifting the window by 600, 1200 or 1800 samples flags nothing. `audio_defects.py` now reads a spike in the first 2 ms or last 1 ms of a window with the neighbouring half window too (test: `test_attack_at_the_start_of_a_window_is_not_a_click`); the committed clips give the same flags as before except those three, the Rubber Band allow entries still match. Tempos are 140, 120 and 130 again.
- Clips: 320 kbps mp3 against the float render, all six pass (snr 39.7 to 54.4 dB).
- REAPER (`host.py` with `PH_SELF=1`, evidence in `evidence/host-groove2/`): all six save, quit, reopen with identical state. The render after reopen is identical for four; Slab City and Dune Buggy differ because the free-running clock starts at another phase each run (the script compares loudness instead).
- Standalone (`standalone.py`, evidence in `evidence/standalone-groove2/`): all six open, take a chord from a virtual MIDI controller and stay running, audio on the silent sink.

### Revision after listening (Pinball, Slab City)

Kosta's notes: Pinball "claps too frequent and the tone too high"; Slab City "too goofy and hard at the same time", bassline fine, the other sound bad.

- Pinball: the clap row is one soft hit in 13 steps (was a hit in four of five), the hat row two in seven, the tom row two in nine (fill pattern B a little denser); the bleep lines are an octave and more lower (triangle waves from C3 and C4, low-passed at 1.5 and 1.2 kHz, was square and triangle from C4 and C5), have fewer notes (three of five, three of seven) and a slower attack; the played voice and the clip riff sit an octave lower, behind a softer 1.8 kHz filter.
- Slab City: the played voice, which carried the cartoonish top line, is now a slow-attack saw pad (60 ms attack, 800 ms release) through a 750 Hz ladder with wider detune, so chords read as a dark choir bed; the clip plays slow minor triads. The 808 line is unchanged. Less hard: clap 3 dB quieter, bus drive 9 to 6 dB. Slab City's tempo moved to 140.4 for the clip-click workaround below.
- New generator field `Spec::voice` (ladder cutoff, detune, amp envelope of the played voice); `None` keeps the group 1 and other group 2 sounds as they were.

### Revision 2 after listening (Pinball, Slab City)

Kosta: Pinball "better but still somewhat goofy", Slab City "a bit too goofy again". Three variants of each (a, b, c; 8 s each) were rendered for Kosta to pick from; the variant clips are not kept. Kosta picked Slab City c and Pinball a, which are the committed sounds; `PP_VAR=a|b|c presets_groove write ...` writes another.

- Slab City: a = root and a fifth late in the bar, straight eighth hats, pad filter 650 Hz, no glide; b = root, root again and a late fifth, pad 500 Hz and quieter, glide 0.12; c = root, root and a minor third, pad 650 Hz with a slow attack, glide 0.25. All: straight eighth-note hats (no triplet stutters, no rolls except two hits at the end of the fill), dark minor notes held long, no 808 slide unless the Glide macro adds it.
- Pinball: a = few pitches (root and a minor third, fifth and a minor third on the second line), b = one pitch per line (hypnotic; the Pitch macro bends it), c = sine waves, narrow intervals. All: filtered at 650 to 1000 Hz, short decays (80 and 100 ms), the played voice darker (900 Hz) and the clip riff on narrow minor steps.
- Found while checking why Slab City's Glide did nothing: the `seq` module has 8 steps (`STEPS = 8`), so `length` clamps to 8 and `p9..p16` set by the generator are ignored. Every 16-step bass and chord line in the generator (group 1 too) therefore plays only its first 8 sixteenths, looping twice per bar. Group 2's other lines are the same and were left as heard. Slab City now puts a `clock.div` (by 2) in front of its 808 sequencer so its 8 steps are a full bar of eighth notes; the Glide macro now has an effect (+2.3 dB). A fix for the other presets would change sounds Kosta has already heard.

### Harshness (group 2)

`docs/presets/scripts/harshness.py` (mean spectrum of the rendered clip: 2-5 kHz energy against the total, and the strongest tonal peak above 2 kHz with its prominence over the spectrum 150-600 Hz either side).

| Preset | Strongest peak above 2 kHz (Hz, level dBFS, prominence dB) | 2-5 kHz / total (dB) |
|---|---|---|
| Dune Buggy | 2016, -60.5, 10.3 | -25.2 |
| Lamp Oil | 2156, -66.8, 14.3 (before: 1500 Hz low-pass, -61.6, 18.5) | -35.7 |
| Acid Line | 23200, -83.5, 10.7 | -18.8 |
| Rope Walk | 7849, -67.4, 14.6 | -23.1 |
| Scrap Metal | 21454, -73.8, 3.1 | -23.6 |
| Slab City | 16799, -73.2, 13.5 | -26.8 |

Lamp Oil's first build had a tone at 2156 Hz, the 11th harmonic of the square fifth (G3) in the chord stab; the stab's low-pass went from 1.5 kHz to 1.0 kHz. Rope Walk's, Slab City's and Scrap Metal's strongest peaks are far above 5 kHz at -67 dBFS or lower. 

### Pinball replaced by Acid Line

Pinball ("concept is bad") is gone: patch, clips, generator entry and evidence deleted. Acid Line takes its slot in group 2; Groove stays at 15. `docs/presets/clips/groove/pinball-vs-acid-line.mp3`: the old Pinball clip, one second of silence, the Acid Line clip (Acid Line starts at 17.0 s). The four macro sweeps of Acid Line (0 to 1 to 0 over 8 s, groove running) are in `docs/presets/clips/groove/acid-line-macros/` (`acid-line-tour.mp3`: Cutoff 0, Resonance 11, Accent 22, Slide 33 seconds).

- Voice: a saw through its own resonant ladder (`filter.ladder`, drive 6 dB), a filter envelope (220 ms decay) scaled by each step's velocity and routed into the cutoff, a gate-length amp envelope also scaled by the velocity (accent steps are louder and open the filter further). Slides: a `slew` on the pitch and a `crossfade` between the raw and the slewed pitch, moved by a second `seq` whose pitch lane is the slide flag of each step, so only the flagged steps glide (steps 4 and 7). The kit is `drum.grid` and the 909-style voices as in the other grooves; the kick ducks the music (ratio 4, not a macro).
- Pattern length: `seq` has 8 steps on this branch, so the line is 8 sixteenths (A minor pentatonic, one rest, accents on steps 1 and 6) that repeats twice per bar. `clock.div` was not needed. When the longer `seq` lands the line can become 16 steps (change `length` and fill `p9..p16`, `v9..v16` in `acid_line`).
- Macros: Cutoff (bass cutoff 180 Hz to 5.7 kHz, 0.5 of travel), Resonance (0.25 to 0.87), Accent (envelope depth 0 to 0.6 of travel, a depth route like the Wash and Sweep of Loading Dock), Slide (glide 8 to 100 ms). Built with the findings of the macro audit (#66) in mind: the routes end where the sound stops changing, no ratio-style first step, a glide scale that starts audible.
- Macro audit of Acid Line (`macro_audit.py`, same method as #66; no dead steps, no jump, no end problem, no click while moving):


| macro | routes: value at macro 0 / 0.5 / 1 | effect (change vs macro 0: at 0.5 / at 1; waveform 0 vs 1) | measured flags | verdict |
|---|---|---|---|---|
| m1 Cutoff | filter.ladder#17.cutoff_hz 180 / 1012 / 5692 Hz (range 20-2e+04, exponential; base 180, macro x 0.50 of travel) | +2.7 dB near 8980 Hz. 1.32 / 1.17 dB; waveform -7.7 dB | duplicate; overlaps Accent (m3): same change, cosine 0.98 | **ok**: bass cutoff 180 Hz to 5.7 kHz in even steps (0.5 to 0.8 dB per step), no dead step, last step 0.3 dB as the saw's harmonics run out; overlaps Accent only by the measure (Cutoff brightens every note, Accent only the accented ones) |
| m2 Resonance | filter.ladder#17.resonance 0.25 / 0.56 / 0.87  (range 0-1, linear; base 0.25, macro x 0.62 of travel) | brighter (top +4.0 dB, low -0.4 dB); +6.8 dB near 2245 Hz. 0.62 / 1.40 dB; waveform -13.8 dB | - | **ok**: resonance 0.25 to 0.87: +7 dB near 2.2 kHz, steps 0.1 to 0.5 dB, no peak above -3 dBFS at 1, no click while moving |
| m3 Accent | depth of vca#19.out to filter.ladder#17.cutoff_hz.amount: 0.00 / 0.30 / 0.60 (macro adds up to 0.60) | brighter (top +3.7 dB, low +0.0 dB); +6.0 dB near 5657 Hz. 1.01 / 1.68 dB; waveform -9.0 dB | duplicate; overlaps Cutoff (m1): same change, cosine 0.98 | **ok**: filter-envelope depth 0 to 0.6 of travel: even steps (0.45 to 0.6 dB), accented notes open further than plain ones; overlaps Cutoff only by the measure |
| m4 Slide | slew#14.rise_ms 8 / 28.39 / 100.7 ms (range 0.1-1e+04, exponential; base 8, macro x 0.22 of travel)<br>slew#14.fall_ms 8 / 28.39 / 100.7 ms (range 0.1-1e+04, exponential; base 8, macro x 0.22 of travel) | no change in level, spectrum or density; the waveform differs by -4.8 dB (pitch path, timing or phase). 0.44 / 0.44 dB; waveform -4.8 dB | - | **ok**: glide 8 ms to 100 ms on the two marked steps: the spectrum stays and the pitch path moves (waveform -4.8 dB), even steps; no click while moving |

Loading Dock (pilot, 124 bpm filter house) moves into Groove on another branch. Lamp Oil is also 124 bpm but is a sparse dub techno groove (rim click, sine sub, chord stab in a dotted echo, no four-on-the-floor bass duck), Rope Walk is 118 bpm disco.

### Notes

- A groove's bass and stab parts come from `seq` modules and play only while the clock runs; the MIDI voice plays over them through the same filter, compressor and kick duck. The kit joins after the echo and room, so the drums are not ducked by their own kick.
- The tag `sequenced` is not used: the browser tests search "Sequence Bass", and the substring matched every groove, pushing the target row out of view.
- Pattern B of each kit is a fill the grid cues can switch to; no groove ships with cues yet.
