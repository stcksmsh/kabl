# Promo v2: storyboard

Cut 1 (`STORYBOARD-V1.md`, `CUT1-NOTES.md`) built one patch up slowly over 86 s. The owner's verdict:
"more demo than promo. A promo needs more sounds, faster cuts, more stuff in general", and it should
be "nice and atmospheric or massive and complex", with Lichen as the flagship and Balcony and
Laundromat as the sounds that feel like kabl. Version 2 is a trailer: 55 s, a cut every bar (half a
bar near the drop), 24 factory presets plus the promo's Lichen-plus patch, and one arc.

Phase 1 is audio only. `build_animatic.py` renders every segment below offline through the real
engine and assembles the audio animatic; the video is cut to that audio in phase 2.

## Grid

- 124 BPM, one bar = 1.935 s, 28 bars: 54.2 s. The file ends on the bar line after two bars of end
  card, the last 1.5 s fading to silence (the first animatic ran a further 2 s of near-silence).
- Every keyboard-played sound is in D minor over one progression, a chord a bar: Dm, Bb, F, C.
- Every clocked preset is rendered at 124 BPM (its clock's `bpm` set before the run), from enough
  bars of pre-roll that it is already in full swing on its first bar. Estuary runs at 62 BPM, half
  of 124, so its slow bars still land on the grid. Pads and choirs are pressed a second before their
  cut so they arrive already open.
- Cuts land on bar lines, placed to the sample. The incoming segment starts on the bar line with a
  3 ms raised-cosine fade-in, short enough to keep its first transient, and the outgoing one fades
  out under it over 12 ms, so no cut clicks. There is no silence before the drop: the Siren, the pulse
  and the riser all run up to its bar line, and the riser's release rings into the hit. Depth Charge hits sit on the big cuts.

## The arc

| Time | Bars | Segment | Preset (cue or macro move) | Visual idea |
|---|---|---|---|---|
| 0:00.0 | 0 | Cold open | **Temple Bowl**, Strike 0.9, D3 + A3, left to ring | Black, then the Temple Bowl card lights on the Sounds page; the Strike knob snaps up |
| 0:01.9 | 1 | Cold open | **Sock Puppet**, a talkbox line D-F-A-C with Mouth swept from a to u, Chatter on | The Mouth macro sweeping; the vocoder's bands flicker |
| 0:03.9 | 2 | Build | **Laundromat**, cue Full; Depth Charge thump | Perform view, the Full pad fires in teal |
| 0:05.8 | 3 | Build | **Glass Elevator**, Bb chord in a 3-3-2 rhythm | Rack, cables glowing from the keyboard into the tine voice |
| 0:07.7 | 4 | Build | **Acid Line**, Cutoff 0.3 to 0.9 | Macro knob sweep; the filter cable brightens |
| 0:09.7 | 5 | Build | **Horn Section**, horn line C-D-E-G | The Sounds page flips to Lead, Horn Section card |
| 0:11.6 | 6 | Build | **Balcony**, cue Chorus; Depth Charge thump | Perform scenes: the Chorus pad fires, macro rings jump |
| 0:13.5 | 7 | Build | **Vox Chop**, Bb house stabs, Vowels up | The cable palette switches: every cable re-colours at once |
| 0:15.5 | 8 | Build | **Dune Buggy**, as stored | Rack zoomed out, every cable pulsing on the beat |
| 0:17.4 | 9 | Build | **Pulpit**, C organ chord, Rotor 0.2 to 1 (variant D: **Glacier**, C pad chord already bloomed, Frost 0.2 to 0.8) | Rotor knob sweep; a code module's scope speeding up (D: the Glacier card, the Frost knob turning) |
| 0:19.4 | 10 | Build, half bars | **Oracle**, Dm choir, Vowel a to o; then **Undertow**, D wobble bass; Depth Charge thump | Sounds page, Voice row; the Wobble macro flicks |
| 0:21.3 | 11 | Build, half bars | **Pawn Shop**, cue Hook; then **Spark Plug**, Bb arpeggio in sixteenths | Perform, the Hook pad; sequencer steps lighting |
| 0:23.2 | 12 | Build, half bars | **Fault Line** (reese and break, at 124); then **Copper Mile**, Sweep up | The reese's cables glowing hot; the Sweep knob turning |
| 0:25.2 | 13 | Build, half bars | **Hot Wire**, C squelch in sixteenths with Squelch up; then **Short Circuit**, Siren on, held into the drop | Squelch sweep; the Siren knob slammed |
| 0:21.3-0:26.9 | 11-13.9 | Riser (layer) | **Updraft** on D3, Build 0.45, Lift 1, Air 0.4, swelling 16 dB | Updraft's Build knob; the filter cables brighten to white |
| 0:03.9-0:26.9 | 2-13.9 | Pulse (layer) | **Loading Dock**, cue Intro, Stabs 0: the kick and hats under every flash except the Piece ones (Laundromat, Balcony, Pawn Shop bring their own drums; variant C keeps it under them too); Sweep opens over bars 12-14 | (under everything) |
| 0:27.1 | 14-17 | DROP | **Lichen-plus**, cue Bloom+ (Focus and Chords at 1: Lichen's loops plus Balcony's chords and arpeggio); Depth Charge impact, Boom 0.9 | The full rack with every cable glowing and the Bloom+ pad lit; the camera pushes in |
| 0:34.8 | 18-19 | DROP | **Gantry**, cue Peak, at 124; Depth Charge on A1 | Perform: the Peak pad fires, every macro ring full |
| 0:38.7 | 20-21 | DROP | Back to **Lichen-plus** (the take kept running); Echo 0.35 to 1 in the last bar, its tail crossfades into the breath | Rack again, the Echo macro sweeping, cables fading into the echo's colour |
| 0:42.6 | 22-23 | Breath | **Estuary**, cue High Tide, at 62 BPM | A slow push over the Estuary rack; the bells' cables blinking in 5, 7 and 11 |
| 0:46.5 | 24-25 | Breath | **Chandelier**, bowed D5 + A5, over Estuary's falling tail | The Bow knob turning, the light dimming |
| 0:50.3 | 26 | Final hit | One beat of **Lichen-plus** Bloom+, **Depth Charge** (Boom 1) and **Temple Bowl** (D3 + A3) | A white flash on the whole rack for one beat |
| 0:50.3-0:55.4 | 26-28.6 | End card | Temple Bowl and Depth Charge ring out; the last second fades | "kabl 1.0: a whole song in one patch" |

24 factory presets: Temple Bowl, Sock Puppet, Laundromat, Glass Elevator, Acid Line, Horn Section,
Balcony, Vox Chop, Dune Buggy, Pulpit, Oracle, Undertow, Pawn Shop, Spark Plug, Fault Line, Copper
Mile, Hot Wire, Short Circuit, Loading Dock, Updraft, Depth Charge, Gantry, Estuary, Chandelier;
plus Lichen-plus (`lichen-plus.kabl`, Lichen with Balcony's chords, from cut 1). Every category of
the bank appears: Piece, Voice, FX, Keys, Bass, Lead, Pluck, Groove (Pad only through Oracle's choir
and Estuary).

## Loudness shape

Every segment is set by its loudness (BS.1770 K-weighted, over its placed bars), not its RMS, so a
bright lead and a bass-heavy groove sit at the same perceived level: the build at -16 LUFS each,
the drop 4 dB louder (-12), the final hit -11, the breath -18.5 (all before the master). The first
animatic matched RMS, which left the build's presets 4.1 dB apart in loudness (Horn Section -14.2 to
Undertow -18.3); now they are within 0.1 dB. The master is the video mix, not a preset: a limiter (ceiling -2 dBFS
sample peak) and gain to -14 LUFS integrated, true peak at or under -1 dBFS after the mp3 encode.
The limiter works about 2 to 5 dB on the drop's and the final hit's transients and under half a
dB elsewhere.

## Building the animatic

From the repository root:

```sh
cargo build --release -p kabl-tools --example promo_render
python3 docs/promo/build_animatic.py OUT_DIR
```

`tools/examples/promo_render.rs` renders a list of segments from a JSON spec: any preset folder or
`.kabl` file, a clock tempo, pre-roll, macro values, macro ramps at runtime, cue launches and
keyboard notes (`promo_render list PATCH...` prints a patch's tempo, macros and cues). The script
holds the storyboard as data (`S`), writes the spec, renders, places each render on the bar grid,
sets each to its loudness target over its placed bars, applies the crossfades, fades and the
riser's swell, and masters with ffmpeg. It writes `NAME.wav`, `NAME.mp3` (320k) and
`NAME-cuesheet.txt` (with each segment's raw loudness and the level it is set to); `--name` sets
NAME, default `promo-v2-animatic`. `PROMO_SKIP_RENDER=1` re-mixes from existing renders.

### The video

```sh
xvfb-run -a -s "-screen 0 1920x1080x24" target/release/examples/promo_render video \
  OUT_DIR/NAME-segments/spec.json OUT_DIR/NAME.wav NAME.mp4
```

`video` draws the real UI offline (look B, dark, 1920x1080, 60 frames a second of virtual time,
so a slow software renderer gives the same clip) and cuts on the same frames as the mix. Each cut
loads its preset and plays its segment through the engine exactly as the audio render did, so the
faces, cables, macro knobs and scene pads move with what is heard. The visual text picks the view:
the Sounds page (searched to the preset, its card selected), Perform (for presets with scenes), or
the Rack, fitted. "push" zooms in over the cut, "palette" steps the cable palette each beat,
"dims" darkens; the last cut flashes white for a beat, then the end card. It opens from black and
fades to black with the mix. The mix is muxed in (AAC 320k), and a still just after every cut and
in the middle of every caption goes to `NAME-stills/`.

The UI is always drawn at the zoom its fit chose. The push scales the finished picture instead,
because the rack switches its labels on and off at fixed zoom levels, and a zoom that crossed one
would make text pop in or out mid-move.

#### Captions

One line each, in the lower third on a dark plate, in the end card's typeface. Each fades in and
out over half a beat on its bar lines. They live in `board()` in `build_animatic.py` and reach the
video through `spec.json`. Every claim comes from the README, the changelog or this storyboard.
The bars are variant A's; the build captions stretch with `--fullbar`, and the later ones move
with the drop.

| Bars | Caption |
|---|---|
| 0-2 | kabl: a legible modular synthesizer |
| 2-4 | Perform: launch scenes and cues, they land on the bar |
| 4-7 | More than 100 factory sounds, each played from a few named macros |
| 7-9 | Cables glow from the live signal, in switchable palettes |
| 9-11 | 53 built-in module kinds: FM, wavetable, filters, drums, a vocoder |
| 11-14 | Functional cables: step patterns, probability, A/B morph and glide |
| 14-18 | Lichen-plus: Lichen's loops and Balcony's chords in one patch |
| 18-20 | Perform: transport, scenes, four macros and the step lanes |
| 20-22 | Write your own modules in Rust or C with the creator SDK |
| 22-24 | Estuary at 62 BPM: bell cables blinking in 5, 7 and 11 |
| 24-26 | A standalone app and a CLAP plugin, on Linux |

## Variants

Three switches of the script answer the open questions by ear; every variant has the same timing
and mix.

| File | Command | What changes |
|---|---|---|
| `promo-v2-A-polished` | `build_animatic.py OUT --name promo-v2-A-polished` | Nothing: the storyboard above |
| `promo-v2-B-fullbar` | `... --fullbar` | A full bar per flash from bar 10 (eight bars instead of four, so the drop is at bar 18 and the promo is 61.9 s); the riser still starts three bars before the drop |
| `promo-v2-C-dockpulse` | `... --dock-pulse` | The Loading Dock pulse keeps playing under Laundromat, Balcony and Pawn Shop |
| `promo-v2-D-padflash` | `... --pad-flash` | Glacier, a Pad, in place of Pulpit at bar 9 |

## Open for the owner

- Half-bar or full-bar flashes in bars 10-13 (A or B).
- The Loading Dock pulse under the Piece flashes or not (C or A).
- A Pad flash in place of Pulpit (D or A).
