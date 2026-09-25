# Sound palette: patch guide

Six playable patches in `patches/palette/` and the piece patch `patches/sound-palette`, all
built by `crates/ui/tests/sound_palette.rs` (rewrite them with
`cargo test -p kabl-ui --test sound_palette write_palette_patches -- --ignored`). Every patch
has four named macros on **CC 20–23** (channel 1), a Perform panel (`--perform`) with labelled
cards, and more mapped controls from CC 24 up. Stepped controls (the keyboard mode, glide
mode, bank buttons) are on the panel but, as before, not MIDI-learnable.

Launch any of them with real audio and MIDI:

    cargo run --release -p kabl-ui -- --patch patches/palette/strings --perform --rate 48000 --frames 256
    # add --size 1280x800 for the smaller window; --midi "KL Essential" picks the controller

Levels: measured offline for one note and 4- and 6-note chords, at the saved settings and
with every macro at 1 (README, "Headroom"). Poly patches are quieter per note than the mono
lead because chords add up; the Level card (CC 26) trims each.

## strings — animated ensemble strings (POLY)

Two saws per voice: one at the note with 4-voice unison, one an octave down with 2-voice
unison, slightly flat, so the ensemble beats slowly. A ladder filter tracks the keyboard
at 70 %, opens a little with the bow envelope and velocity, and drifts on a slow triangle;
a 5.3 Hz vibrato on both oscillators; a stereo chorus; a small plate.
Play: slow chords, a top line over held chords.

| control | CC | what it does |
|---|---|---|
| Bright | 20 | ladder cutoff |
| Ensemble | 21 | chorus depth and mix, unison detune |
| Bow | 22 | slower attack and a longer release |
| Space | 23 | plate mix and length |
| Chorus rate · Release · Level | 24 · 25 · 26 | |
| Keys (mode) | — | POLY / MONO / LEGATO |

## pad — warm evolving pad (POLY)

A pulse wave (35 %) whose width sweeps on a 0.11 Hz sine (PWM), a detuned saw and a little
per-voice pink noise into a driven, resonant ladder that breathes on an 8-second triangle
(depth on Evolve). Long envelope (1.4 s attack, 2.8 s release), gentle chorus, a ping-pong
echo and a big plate.

| control | CC | what it does |
|---|---|---|
| Warmth | 20 | cutoff and filter drive |
| Evolve | 21 | depth of the filter's slow breathing |
| Air | 22 | the noise layer |
| Space | 23 | plate and echo |
| Attack · Release · Level | 24 · 25 · 26 | |

## lead — singing mono lead (LEGATO)

**LEGATO**, **LAST** priority, glide **LEGATO** (only between overlapping notes), 90 ms.
A saw plus a pulse hard-synced to it, tuned a fifth up; the envelope sweeps the synced pulse
on each attack (a vowel-like bite), resonant ladder with key tracking, per-note drive,
vibrato scaled by a macro, ping-pong echo, plate. Detached notes re-attack and don't glide;
overlapping notes glide and don't re-attack.

| control | CC | what it does |
|---|---|---|
| Tone | 20 | cutoff |
| Vibrato | 21 | vibrato depth (0 = none) |
| Bite | 22 | drive and resonance |
| Echo | 23 | echo mix and repeats |
| Glide time | 24 | 5 ms – 3 s |
| Level | 26 | |
| Keys · Glide | — | mode; OFF / ALWAYS / LEGATO |

## bass — resonant sequence bass (sequenced)

An 8-step sequence at 112 bpm (banks Line, Lift, Hold, Rest) into a saw plus a square an
octave down, a ladder at high resonance with 9 dB of input drive, the envelope and the
step velocity (accent) into the cutoff, and a per-voice drive stage. Dry on purpose.

| control | CC | what it does |
|---|---|---|
| Cutoff | 20 | |
| Resonance | 21 | |
| Accent | 22 | longer decay, more filter drive |
| Drive | 23 | the drive stage |
| Tempo · Level | 25 · 26 | |
| transport, Bass banks, Transpose | — | |

## breath — airy wind / breath texture (POLY)

Per-voice pink noise through a narrow band-pass that follows the key 1:1 (the note is the
band), plus a wider, brighter air band and a quiet sine for the pitch centre. A slow LFO
gusts both bands (depth on Gust). Wide chorus, long plate.

| control | CC | what it does |
|---|---|---|
| Breath | 20 | more of the airy band |
| Gust | 21 | wind movement |
| Color | 22 | narrower, brighter band |
| Space | 23 | plate mix and length |
| Width · Level | 24 · 26 | |

## perc — noise percussion (sequenced)

Three sequencers on one clock (112 bpm): a kick (a 46 Hz sine swept down from about 2½
octaves by a 45 ms envelope), a snare (white noise through a 1.9 kHz band plus a 185 Hz
body) and a hat (white noise above 7 kHz, 35 ms). Banks per drum (Four/Push/Pulse,
Back/Ghost, Eighths/Run/Tick, plus rests). A drum-bus drive and a short room.

| control | CC | what it does |
|---|---|---|
| Decay | 20 | all three decays |
| Tone | 21 | snare band and hat cutoff |
| Crunch | 22 | bus drive |
| Room | 23 | room mix and length |
| Hat level | 24 | |
| transport, Kick / Snare / Hat banks | — | |

## sound-palette — the piece patch

Everything above in one rack at 104 bpm: the percussion and bass sequenced and cued by
section, and the four keyboard voices (breath, strings, pad, lead) as **layers on faders**.
Every `midi.in` hears every key (as before; there are no keyboard zones), so a layer is
played by raising its fader; the lead is mono (LEGATO), the others poly. Shared chorus
(strings, pad, breath), a clock-synced echo on a send from the lead, a plate, and a small
room for the drums. A −4 dB master trim keeps the densest moment (all macros up, three
layers at 0.8, the Lift banks) under −1 dBFS. The sequences start resting (the cue **Air**)
so the piece opens on the atmosphere.

| control | CC | what it does |
|---|---|---|
| Energy | 20 | bass cutoff and drive, drum crunch, hats |
| Motion | 21 | every slow modulation: pad breathing, breath gusts, lead vibrato, chorus depth |
| Bright | 22 | the keyboard layers' filters |
| Space | 23 | plate and echo |
| Breath · Strings · Pad · Lead | 24–27 | layer faders |
| Lead echo | 28 | echo send |
| Glide time | 29 | the lead's glide |
| cue buttons Air · Pulse · Groove · Lift · Break | 40–44 | next bar |
| cancel · run/stop · restart | 45 · 46 · 47 | |
