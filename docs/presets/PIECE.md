# Piece presets (9 self-playing pieces with cues)

Generator: `tools/examples/presets_piece.rs` (`write`, `clips`, `phrases`, `levels`, `library`). Patches:
`patches/presets/<slug>/`. Clips: `docs/presets/clips/piece/` (per piece `<slug>.mp3`, one run of the
cues as meant, and `<slug>-tour.mp3`, the four macros 20 s each; index in `tours.txt`).

Every piece: press Run, launch the cues with the pads or CC 40+. Cues move the macros only; the sequencers
keep playing. The first cue is the stored macro values. Cue morphs are in beats; the "bar" column is where
the clip run launches each cue (a player launches them whenever). All pieces have **4 macros** (the norm), no
limiter (gain staging only), no hiss by default (texture is on a macro), peaks <= -1 dBFS at every macro
position and in the all-at-1 check. No piece is in `LOUD_EXCEPTIONS`.

| Piece | Genre | BPM | Cues (bar, morph beats) | Drums | Signature |
|---|---|---|---|---|---|
| Gantry | techno build and drop | 132 | Intro 0, Build 4 (16), Drop 8 (0), Break 16 (4), Peak 20 (8) | yes | no |
| Dead Calm | cinematic swell | 66 | Hush 0, Swell 4 (16), Impact 8 (0), Aftermath 12 (16) | toms, impact | no |
| Pawn Shop | boom bap | 88 | Verse 0, Hook 6 (4), Bridge 12 (4), Outro 18 (8) | yes | no |
| Laundromat | filtered disco | 122 | Loop 0, Cut-up 4 (0), Break 12 (4), Full 16 (8) | yes | **yes** (dry: no echo, no room) |
| Spillway | drum and bass | 174 | Intro 0, Build 4 (16), Drop 8 (0), Rolldown 12 (8) | yes | no |
| Balcony | pop loop with sections | 104 | Verse 0, Pre 4 (8), Chorus 8 (0), Bridge 12 (4), Last Chorus 16 (0) | yes | no |
| Estuary | ambient, polymeter and probability | 60 | Low Tide 0, Rising 4 (16), High Tide 10 (16), Ebb 16 (16) | none | no |
| Lichen | experimental, random loops in polymeter | 112 | Spore 0, Colony 4 (16), Bloom 12 (8), Die-back 20 (16) | yes | no |
| Roller Rink | synthwave | 100 | Intro 0, Verse 4 (0), Chorus 8 (0), Break 14 (4) | yes | no |

Genre spread follows the BRIEF table (techno, cinematic, hip-hop, disco, drum and bass, pop, ambient,
experimental, synthwave); two signature pieces at most (Laundromat; Gantry is dry as well but is not tagged).
Names are distinct from every other preset in the bank.

## What kabl shows off

Gantry: ducking compressor driven by the kick, riser and filter on one macro. Dead Calm: probability
steps (the bell plays half its notes), a slow 8-step drone against a heartbeat of toms. Pawn Shop: swing
as a macro, a worn-tape pitch wobble whose depth is a macro. Laundromat: bitcrush and drive on the bus,
stabs from ticks to chords by gate length. Spillway: beating reese against a sub, ghost snares. Balcony:
chord progression, hidden arpeggio, chorus. Estuary: three bells with cycles of 5, 7 and 11 against a
4-beat bar, chance on single steps. Lichen: random loops of 7 and 5 values held to D dorian through cables
with their own step patterns, drums looping in 5, 7 and 9 against 16 (polymeter). Roller Rink: running
eighths, gated snare, arpeggio, chorus.

## Per piece

Macro tables: "stored" is the value in the first cue. What you hear is from the descriptions in the
patches and the tours.

### 1. Gantry (132 BPM, techno)

Idea: a four-on-the-floor build and drop with the music ducking to the kick. Sections: Intro (filter closed,
bass out), Build (filter and riser open over 16 beats), Drop (rumble and acid in at once, no morph), Break
(everything pulled back in 4 beats), Peak.

| Macro | Stored | 0 to 1 | Moves |
|---|---|---|---|
| Climb | 0.20 | muffled and dark, to open with a rising noise riser | master ladder cutoff 140 Hz to 12 kHz and its resonance, riser gain, hat/clap filter; level-compensated by 8 dB |
| Rumble | 0.35 | the off-beat bass from out to full, and its notes lengthen | bass gain, gate length, its filter |
| Acid | 0.15 | clipped stabs to squealing, ringing stabs | stab decay, resonance, gain |
| Pump | 0.55 | music barely ducks to the kick hard pumping | ducker threshold and makeup |

Flags: Pump and Rumble are level-type (pump, bass in). Louder drop: the Drop cue is about 2 dB louder than
the Intro (Rumble at 1).

### 2. Dead Calm (66 BPM, cinematic)

Idea: a minor drone and pad on slow steps (one step per bar), a dissonant cluster, a sparse bell that plays
half its notes by chance and a low impact with a heartbeat of toms. Sections: Hush, Swell (16-beat morph
into the build), Impact (no morph), Aftermath (16-beat morph back and a long room).

| Macro | Stored | 0 to 1 | Moves |
|---|---|---|---|
| Swell | 0.15 | brightness: closed and quiet to open with a riser fading in | master ladder 180 Hz to 9 kHz, riser, drone gain; level-type |
| Dread | 0.10 | harmony: nothing to a dark, beating tritone and semitone cluster | cluster gain -40 to -2 dB and its filter 300 Hz to 1.2 kHz (a triangle cluster an octave down, after the master filter so Swell does not hide it) |
| Pulse | 0.20 | rhythm: still to a heartbeat; toms and the impact come in and the stage throbs once a beat | tom and impact mixer levels, depth of a beat-locked falling saw on the compressor makeup (0 to 6 dB), makeup -3 dB over the travel |
| Hall | 0.60 | space: dry and narrow to a long echo and a wide 9 s room | echo and room mix, room width 0 to 100 %, **level-compensated by 6 dB** over its travel |

Distinctness (owner: "the macros seem similar"). Measured on the macro-alone renders at 0 and 1 (`macro_audit`
steps): change vectors over level, centroid, band balance, onsets, beat-rate level swing, stereo width,
flatness and pitch-class content. Before: Dread and Pulse barely moved the sound at the stored Swell (the
cluster sat under the closed master filter, the toms under the drone; change 3.5 and 3.0 against Swell's
14.7) and Swell and Dread pointed the same way (cosine 0.74). Now every macro moves 7.8 to 15 and the
largest cosine between two macros is 0.27 (mean 0.15): Swell brightness, Dread pitch content, Pulse a
beat-rate level swing of 8.5 dB, Hall stereo width. The cue run keeps its stored values; per cue the
loudness moves 0.1 to 0.9 dB (Impact the most, with its throb) and the stereo width a little.

Flags: Swell and Pulse are level-type (they add parts). The piece's own first bar is nearly silent (every
line steps once per bar), so the tours were rendered after a 2-bar pre-roll (see Tools).

### 3. Pawn Shop (88 BPM, boom bap)

Idea: dusty boom bap with swing. Sections: Verse (muffled loop), Hook (opens up), Bridge (muffled and dusty),
Outro (swing dragging).

| Macro | Stored | 0 to 1 | Moves |
|---|---|---|---|
| Swing | 0.50 | straight to pushed-late off-beats | clock swing 20 to 80 |
| Dust | 0.00 | clean to vinyl hiss plus a worn-tape pitch wobble | hiss gain, LFO depth on the chord pitch |
| Tape | 0.85 | muffled and saturated to clean and open | master and hat filters, drive and trim |
| Room | 0.25 | dry to slapback echo and a small room | echo and room mix |

### 4. Laundromat (122 BPM, filtered disco; signature, dry: no echo, no room)

Idea: an octave bass and two off-beat stab lines with swing, ducking to the kick. Sections: Loop, Cut-up
(filter and crush up, stabs short), Break (dark, held chords), Full (everything open).

| Macro | Stored | 0 to 1 | Moves |
|---|---|---|---|
| Sweep | 0.45 | DJ filter closed to open, resonant when closed | master ladder 200 Hz to 14 kHz, resonance, hat/clap filter; level-compensated by 6 dB |
| Crunch | 0.20 | clean to bitcrushed and driven | bitcrusher mix, drive, trim |
| Stutter | 0.50 | clipped ticks to held chords | stab gate length 6 to 95 % |
| Pump | 0.60 | gentle to hard kick ducking | ducker threshold, makeup; level-type |

### 5. Spillway (174 BPM, drum and bass)

Idea: two-step kick and snare, a sine sub and a beating reese in F minor over a pad bed. Sections: Intro,
Build, Drop (sub and reese in), Rolldown (ghost snares, busy hats).

| Macro | Stored | 0 to 1 | Moves |
|---|---|---|---|
| Weight | 0.30 | no sub to a deep floor-shaking sub; level-type | sub gain -36 to -4 dB |
| Reese | 0.30 | slow, subtle beating to a moderate one | reese detune 2 to 18 cents (beat on the F2 fundamental 0.10 Hz at 0, 0.34 Hz stored, 0.91 Hz at 1; the first take was 4 to 40 cents, 0.20, 0.75, 2.0 Hz; the top was 14 cents, 0.71 Hz, before the owner asked for a little more at 1), its filter 500 Hz to 1.8 kHz, bus drive 4 to 14 dB with trim -3 to -8 dB |
| Break | 0.15 | plain two-step to ghost snares and busy hats | the ghost-snare and hat mixer levels (top kit mixer) |
| Haze | 0.50 | dry to echo and room | echo and room mix |

### 6. Balcony (104 BPM, pop with sections)

Idea: a four-chord progression in C with a round bass, kick and snare, an arpeggio under the chords and a
chorus. Sections: Verse, Pre (8-beat morph), Chorus, Bridge, Last Chorus.

| Macro | Stored | 0 to 1 | Moves |
|---|---|---|---|
| Lift | 0.40 | dark and narrow to open chords widened by a chorus | master ladder 600 Hz to 14 kHz, hat/clap filter, chorus mix 0 to 55 %; level-compensated by 5 dB |
| Sparkle | 0.15 | no arpeggio to a bright triangle arpeggio | arpeggio gain -40 to -8 dB |
| Bounce | 0.30 | steady chords to hard ducking to the kick; level-type | ducker threshold 0 to -30 dB, makeup |
| Room | 0.30 | dry to echo and a small room | echo and room mix |

### 7. Estuary (60 BPM, ambient, no drums)

Idea: slow pads, three bells cycling in 5, 7 and 11 steps with some steps by chance, a sine drone and a sea
of noise. Sections: Low Tide, Rising, High Tide (both 16-beat morphs), Ebb.

| Macro | Stored | 0 to 1 | Moves |
|---|---|---|---|
| Tide | 0.20 | dark pads to open pads with the sea rising | master ladder 300 Hz to 8 kHz, riser, level-compensated by 4 dB |
| Glitter | 0.30 | bells out to bells up, ringing longer | bell gains -40 to -10 dB, gate length 15 to 70 % |
| Depth | 0.25 | nothing to a drone from far below | drone gain -40 to -4 dB |
| Wash | 0.60 | dry to long echo and 12 s room | echo and room mix, **level-compensated by 2.5 dB** |

Flags: the output trim is +2 dB. At +4 the cue run peaked at -0.9 dBFS; at 0 the factory level test (`factory_levels_hold_the_target`) read -24.6 dBFS, below its -19 +- 5.5 window. The whole run is louder than the first cue (-14.1 LUFS over the run, -20.4 dB at the stored sound) because High Tide opens everything: say so in the description.

### 8. Lichen (112 BPM, experimental)

Idea: two random melodies quantized to D dorian, looping in 7 and 5 values, each through a cable with its
own step pattern, over a bass in 7, a stab in 5 and hats, open hats and toms in 5, 7 and 9 against 16.
Sections: Spore (loops repeat), Colony (they mutate), Bloom (full), Die-back (the melodies sink into echo).

Each line has its own register and timbre, so the polymeter can be followed: bass a low saw (D1 to D2,
low-pass 600 Hz), pad a dark bed under it (low-pass 700 Hz, -18 dB), the 7-loop a saw from C4 (ladder
1.5 kHz plus its envelope), the stab a bright pulse an octave higher (D5 to A5, low-pass 2.8 kHz), the
5-loop a triangle bell two octaves above the 7-loop; toms at 180 Hz above the bass, hats 7 dB up. The
master filter sits at 8 kHz (it was 3 kHz and took the stab's and the 5-loop's top off). The bus is
mono up to the room, so the separation is by register, timbre and filter band, not by panning.
Measured on solo and muted renders of the cue run (band-weighted SNR of each voice against the rest of
the mix, in the frames where it plays; worst cue, before -> after): 5-loop -7.5 -> +3.1 dB, 7-loop -4.5
-> -0.2 dB, toms -4.3 -> +1.6 dB, stab +3.3 -> +3.1 dB, bass +4.7 -> +7.4 dB. The hats are 11 dB louder
in the mix (-37 -> -26 dB) but share their band with the now open top (closed +2.4 -> 0.0 dB, open +1.6
-> +0.6 dB); the pad is a bed (-3.9 -> -7.0 dB).

Die-back: the two melodies go through a keys stage of their own (level, dotted-eighth echo with 55 %
feedback and a 1.8 kHz tone, 4.5 s room damped at 2 kHz). A route from Echo whose depth is Echo minus
Mutate moves it by Echo x (Echo - Mutate) of its travel: none at Bloom, 16 % at Spore, 3 % at Colony, all
of it at Die-back (Echo 0.8, Mutate 0). There the keys drop 9 dB, their filters fall to a sixth (7-loop
1.5 kHz to 250 Hz, 5-loop 7 kHz to 1.2 kHz) and their echo and room mixes rise to 64 % and 60 % (on top
of the shared 32 % echo and room); the dry keys are about 19 dB down. Before, Die-back only moved the
shared echo and room from 14 % to 32 % and the keys kept their level and tone (their share of the mix
-10.6 dB at Die-back, as at Spore). Now the 7-loop's share is -10.8 dB at Spore and -17.7 dB at Die-back,
the 5-loop's -14.4 and -21.7 dB, most of it echo.

| Macro | Stored | 0 to 1 | Moves |
|---|---|---|---|
| Mutate | 0.05 | the random loops repeat exactly to changing as they come round | both random modules' change 0 to 70 % (and holds back Echo's sink of the keys) |
| Reach | 0.30 | a third to two octaves of range | both random modules' range 4 to 30 semitones |
| Gate | 0.40 | ticks to held notes on bass and stab | gate length 12 to 95 % |
| Echo | 0.35 | dry to echo and room; high with Mutate low, the melodies sink into their own echo and room | echo and room mix; keys level, filters, echo and room by Echo x (Echo - Mutate) |

### 9. Roller Rink (100 BPM, synthwave)

Idea: a running eighth-note bass, lush saw chords through a chorus, a pulse arpeggio, a kick and a big gated
snare in A minor. Sections: Intro, Verse, Chorus (all open), Break (filter closes, space up). Roller Rink's
first cue (Intro) is the stored sound.

| Macro | Stored | 0 to 1 | Moves |
|---|---|---|---|
| Glow | 0.20 | dark to open, a deep chorus spreading the chords | master ladder 400 Hz to 12 kHz, hat filter, chorus mix 0 to 60 %; level-compensated by 5 dB |
| Arp | 0.20 | arpeggio out to up, longer notes | arpeggio gain -40 to -6 dB, gate length 15 to 95 % |
| Gated | 0.20 | tight to a louder snare with a long gated tail | snare drive 0 to 18 dB, decay 150 to 900 ms |
| Space | 0.45 | dry to echo and room | echo and room mix |

## Checks

Loudness and peak per piece (`presets_piece levels`; peak dBFS / loudness dB, at the stored values
("normal"), every macro at 1 ("all"), and each macro alone at 1). Peaks are <= -1 dBFS everywhere.

| Piece | normal | all at 1 | m1 | m2 | m3 | m4 |
|---|---|---|---|---|---|---|
| Gantry | -4.3 / -19.2 | -5.2 / -18.2 | -5.6 / -21.1 | -2.7 / -17.1 | -2.7 / -19.1 | -5.8 / -19.2 |
| Dead Calm | -7.9 / -19.3 | -2.0 / -17.5 | -8.9 / -20.0 | -6.2 / -18.0 | -3.6 / -20.1 | -2.4 / -15.3 |
| Pawn Shop | -1.8 / -18.8 | -1.8 / -19.1 | -1.8 / -18.7 | -1.8 / -18.6 | -1.8 / -19.1 | -1.8 / -19.3 |
| Laundromat | -3.0 / -17.4 | -6.8 / -18.7 | -3.8 / -18.4 | -3.5 / -17.0 | -3.2 / -17.2 | -6.8 / -19.1 |
| Spillway | -4.4 / -20.6 | -1.9 / -17.8 | -4.3 / -19.5 | -4.9 / -19.4 | -4.1 / -20.3 | -3.9 / -20.4 |
| Balcony | -3.2 / -18.2 | -4.1 / -19.2 | -4.1 / -19.4 | -3.1 / -18.0 | -3.6 / -18.1 | -3.6 / -18.2 |
| Estuary | -7.7 / -20.4 | -3.4 / -18.4 | -8.2 / -22.0 | -5.1 / -18.9 | -7.8 / -18.9 | -4.1 / -18.0 |
| Lichen | -2.0 / -20.0 | -1.9 / -19.6 | -2.0 / -19.8 | -2.2 / -20.0 | -2.3 / -19.3 | -3.2 / -20.5 |
| Roller Rink | -4.5 / -18.5 | -2.1 / -18.1 | -4.2 / -19.8 | -4.3 / -18.0 | -4.5 / -18.4 | -3.9 / -18.5 |

Combination: all macros at 1 stays within about 2.5 dB of the stored loudness for every piece (Dead Calm
+1.8, where the swell and the impact are meant to build; Estuary +2.0; Spillway +2.7, the sub and the reese
at full); no combination clips. The factory level test (`factory_levels_hold_the_target`) passes.

Clips (checked on the mp3s): cue runs 62-92 s at -14.1 to -17.2 LUFS, peaks -2.0 to -3.9 dBFS; tours 84 s
(four 20 s clips and 1 s gaps) at -14.9 to -17.8 LUFS; no silence over 1.4 s (n=-45 dB) anywhere; tour hold
levels (6 s at macro 0 against 6 s at macro 1) are within 3 dB for every macro of every piece except the
level-type ones (Dead Calm Swell about -3 dB).

Level compensation of the two space macros that added level: Dead Calm Hall (6 dB) and Estuary Wash
(2.5 dB). Measured on the tour windows (6 s at 0, 6 s at 1): Hall +0.1 dB, Wash +1.6 dB; on steady renders
Hall -1.5 dB, and Wash +2.3 dB before its compensation. Dead Calm after its macro redesign: cue run -15.3
LUFS, peak -2.1 dBFS; tour -15.4 LUFS, peak -1.1 dBFS (Hall at 1, the widest room); tour holds Swell -2.2,
Dread +0.3, Pulse -2.1, Hall +0.1 dB.

## Tools

`MACRO_TOUR_PREROLL_BARS=<n>` (new, default 0) in `tools/examples/macro_audit.rs` tour mode renders `n`
bars first and drops them, so a piece with a slow intro (Dead Calm steps once per bar) is heard in its steady
state. The seven new tours use 2 bars.
