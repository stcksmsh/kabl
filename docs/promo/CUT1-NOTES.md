# Promo v1, cut 1: notes

The first cut of the 1.0 promo, recorded from `STORYBOARD-V1.md` with the owner's three decisions:
the peak layer is Balcony's chords and arpeggio, the Sounds montage uses the main take's audio, and
there is no on-camera package insert. Nobody has watched or heard this cut yet; the agent that made
it cannot hear. The checks below are measurements.

The mp4 is not in git. It was written to `.claude/tmp/listen/promo/kabl-promo-v1-cut1.mp4`, with a
contact sheet (12 frames, one per shot) at `.claude/tmp/listen/promo/cut1-frames.png`.

## What was recorded

- 1920x1080, H.264 CRF 18, 30 fps (captured at 60, see below), AAC 192 kb/s 48 kHz, 86.3 s (bars 0-40 at 112 BPM).
- Audio is the real app's output (the null sink `kabl_agents`; no device sound). Integrated loudness -13.9 LUFS,
  true peak -1.4 dBFS (sample peak -1.45), LRA 9.2 LU.
- Two takes joined with a hard cut: take A (bars 0-8.25) and take B (bars 8.25-40).

| # | Bars | What is on screen | Sound |
|---|---|---|---|
| 1 | 0-4 | Empty rack, "Add module": VA Oscillator, Ladder Filter, Output added by typing in the add menu; Output level down 14 dB; two cables | silent until the second cable lands (about bar 4.2) |
| 2 | 4-8 | Oscillator to triangle; LFO added and patched to Cut CV (teal cable); cutoff opened to about 10 kHz | one voice, wobbling and opening |
| 3 | 8-12 | Lichen-plus, whole rack (fit, all cables), clock starts on bar 8 | Spore, Focus 0.1 |
| 4 | 12-15 | The 7-loop's cable (46) pattern in the side panel | Spore |
| 5 | 15.5-21 | Perform (teal pads), Colony cue | the loops mutate |
| 6 | 21.5-23 | Focus dragged 0.35 to about 0.55 (the cue then holds it) | voices come forward |
| 7a | 23.5-28 | Bloom, then Bloom+ (Chords 0 to 1 over 8 beats) | peak: Lichen plus Balcony's chords and arpeggio |
| 7b | 28-32 | Rack on 28, Perform on 29; Chords dragged down on 30 and back up on 31 (50 and 60 px) | peak 2 |
| 8 | 32-35.5 | Sounds: Piece category, Balcony, Laundromat, Gantry selected one a bar (no preview played) | the main take keeps playing |
| 9 | 35.5-40 | Perform, Die-back; end card "kabl / 1.0: a whole song in one patch" over bars 38-40, audio fade over the same 2 bars, picture fades in the last second | the melodies sink into their echo |

## The patch

`tools/examples/presets_piece/promo_v1.rs` (a submodule of `presets_piece.rs`, run as
`presets_piece promo write|render`) writes `docs/promo/lichen-plus.kabl` and `docs/promo/empty.kabl`.
It is Lichen as shipped (the factory presets are byte-identical after the `lichen_with` refactor)
plus a layer on Lichen's clock: Balcony's three unison-saw chord voices and its triangle arpeggio,
re-voiced from C major to D dorian (same pitch set, centred on D): Dm, F/C, C, G/B, one bar each,
with the arpeggio in eighths. The layer has its own room (3.5 s) and joins Lichen's bus before the
master filter, drive, compressor, echo and room. Macro 6, **Chords** (CC 25), takes its level from
the gain's minimum to -2 dB; every Lichen cue sets it to 0. Cue 4, **Bloom+** (CC 43), is Bloom with
Focus and Chords at 1, an 8-beat morph; Die-back moved to cue 5 (CC 44). No limiter.

Offline render of take B (`presets_piece promo render`): peak -2.5 dBFS; RMS per section Spore -20.0,
Colony -19.5, Bloom -18.9, Bloom+ -17.7, Die-back -19.4 dBFS. The layer adds about 1 dB of RMS at
the peak: Lichen's bus compressor holds the level, so the ramp is mostly density, not loudness.

## Deviations from the storyboard

- Peak layer is Balcony (owner's decision), not Laundromat's disco part; macro "Chords" instead of "Disco".
- Shot 8 shows the Sounds page while the main take keeps playing (owner's decision); no preview is played and the rack is never stopped (no `pstop` in 7b, no `prun` in 9).
- No on-camera package insert (owner's decision).
- 30 fps, not 60. The capture held 60 fps and the Perform and Sounds views drew about 50-58 distinct frames a second, but the full Lichen rack at fit zoom under llvmpipe drew about 9-12 a second (the app's own live-cable cadence is 33 ms). The output is the 60 fps capture resampled to 30.
- Take A starts from an empty rack with no clock, so shot 1 has no sequencer: oscillator, ladder filter and output, a plain held tone (C4). Its first sound is at about bar 4.2, not bar 2: each scripted UI action costs about 0.3 s and each cable drag about 1.4 s. The first 9.5 s of the video are silent.
- Shot 4 opens the 7-loop cable's pattern but does not morph it: the cable has no B pattern in Lichen.
- Shot 6 has no sequencer bank switch: Lichen's sequencers have one bank.
- The cut is at bar 8.25, not 8.0: in take B the first quarter bar after Run is silent (the clock starts with a reset pulse and the first beat does not sound), and cutting on the downbeat left a 0.5 s gap.
- The generator lives at `tools/examples/presets_piece/promo_v1.rs` rather than `tools/examples/promo_v1.rs`, so it uses `presets_piece`'s helpers without copying them; the patches are in `docs/promo/` because every folder under `patches/` becomes a factory sound.
- Loudness: the mix's peak-to-loudness ratio is about 16 dB, so -14 LUFS with peaks under -1 dB is not reachable with gain alone; the two-pass `loudnorm` (target -14 LUFS, -2 dBTP so the AAC encode stays under -1) falls back to its dynamic mode on the final mix. The patch itself has no limiter.

## Known issues

- The take-A tone is C4 and Lichen is in D: the cut changes key.
- The Sounds page in shot 8 shows the Factory list with the Piece filter; the selected sound's detail
  appears about a beat after each click.
- Cue clicks happen half a bar early and land on the clock's next bar; they are scripted against the
  drive log, so they can be up to about 1 s off when a gesture before them overruns.
- Bar 0 of each take comes from the drive log, corrected by the audio envelope against a reference
  (take B: the offline render, correlation 0.95; take A: a tone that starts at bar 4.2, 0.996).

## Reproduce

```sh
cargo run --release -p kabl-tools --example presets_piece -- promo write docs/promo/lichen-plus.kabl docs/promo/empty.kabl
cargo run --release -p kabl-tools --example presets_piece -- promo render OUT/renderB.wav OUT/timeline.json
ffmpeg -i OUT/renderB.wav -c:a pcm_s16le OUT/refB.wav   # post.py reads 16-bit wavs
python3 docs/promo-demo/scripts/make-drive-script.py - docs/promo-demo/targets.json OUT/driveA.txt docs/promo/take-a.json
python3 docs/promo-demo/scripts/make-drive-script.py - docs/promo-demo/targets.json OUT/driveB.txt docs/promo/take-b.json
# take A's reference: 9.1 s of silence, then a tone, 19.5 s in all (OUT/refA.wav)
KABL_ANCHOR=empty-add TAKES="docs/promo/empty.kabl:OUT/driveA.txt:OUT/timeline.json:OUT/refA.wav" OUT=OUT/takeA FINAL=OUT/takeA.mp4 docs/promo-demo/record.sh
KABL_ANCHOR=view:All TAKES="docs/promo/lichen-plus.kabl:OUT/driveB.txt:OUT/timeline.json:OUT/refB.wav" OUT=OUT/takeB FINAL=OUT/takeB.mp4 docs/promo-demo/record.sh
END=86.214 FADE=4.286 LUFS=-14 RATE=30 CARD="kabl|1.0: a whole song in one patch" \
  python3 docs/promo-demo/scripts/post.py --join kabl-promo-v1-cut1.mp4 18.2 OUT/takeA/take-1/take.mp4 OUT/takeB/take-1/take.mp4
```

Script changes for this cut: `record.sh` honours `CARGO_TARGET_DIR` and runs the app through
`KABL_BIN`, builds `promo_demo` only for the default take, and names the null sink it creates;
`make-drive-script.py` takes a storyboard `setup` list (an empty rack has nothing to fit);
`post.py --join` takes `END`, `FADE`, `CARD`, `LUFS` and `RATE`.
