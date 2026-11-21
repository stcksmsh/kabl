# Promo video for 1.0: storyboard

The owner asked for this: "make it start slow and then ramp, with multiple presets playing at once, and the piece presets etc."
The tool is the scripted real-app recorder in `docs/promo-demo/` (`record.sh`, `targets.json`,
`scripts/make-drive-script.py`, `scripts/post.py`, driven by `docs/rack-migration/drive.py`). The
Morph Suite promo (2:48, 30 fps) was made with it. This is a plan only: nothing here has been built,
recorded or heard.

Depends on PR #171 (Piece presets: Lichen, Laundromat, Balcony, Gantry) and PR #172 (Voice and FX
presets) being merged, or the recording runs on a branch that contains both.

## How the tool works today

- `record.sh` builds the app and the `promo_demo` example, writes the patch and `timeline.json`, renders a
  reference wav, starts Xvfb at 1920x1080 and a null sink, and runs `ffmpeg -f x11grab -framerate 30` plus
  `-f pulse -i <sink>.monitor` into `recording.mp4`. It then runs `drive.py` on one patch.
- `drive.py` starts `kabl-ui --patch P`, reads the hit rectangles the app writes (`KABL_HITS_FILE`, every
  `ui_state.record(key, rect)`), and runs a text script: `click`, `rclick`, `drag out:A.out in:B.in`,
  `dragby KEY DX DY`, `aim KEY`, `slide DX SECONDS` (a timed horizontal drag only), `wait T` (absolute
  seconds from the start), `type`, `key`, `shot`, `wheel`.
- `make-drive-script.py` handles only morph gestures: open a cable's pattern editor, then drag
  `fn-morph:{id}` between two bars. Bar 0 is a click on `restart:1`. Nothing else can be scheduled.
- `post.py` finds bar 0 by matching the recording's loudness envelope against the render, burns
  captions (ASS), trims the file, and writes the mp4.
- Hit keys that already exist and that this storyboard needs: `empty-add`, `add:{kind}`,
  `out:{id}.{port}` / `in:{id}.{port}`, `cable:{id}`, `menu:cable-pattern`, `fn-morph:{id}`, `zoom:fit`,
  `view:All|Focus|Hidden`, the scene switch `rack`, `perform`, `browser`, and in Perform `prun:{id}`,
  `prestart:{id}`, `pcue:{id}.{n}`, `pslider:{key}`, `pbank:{id}.{b}`. In Sounds there are `sound:{id}`
  (select the sound, which starts its clip render) and `play:{id}`.

## Can two presets play at once?

Not as two patches: the app runs one rack. There are three ways to get several presets playing:
1. **A combined patch (the main method).** A new generator `tools/examples/promo_v1.rs` writes
   `patches/promo/lichen-plus`: Lichen unchanged, plus a "Disco" composite taken from Laundromat (the octave bass and the
   off-beat stabs with their bus crush and drive) retimed to Lichen's 112 BPM clock, transposed to D dorian,
   and brought in by a sixth macro, **Disco** (Lichen uses 5 of the 8 slots), whose gain is at -inf at 0, plus a
   fifth cue, **Bloom+** (Bloom with Disco at 1). It is real playback with one clock.
2. **Insert on camera (optional, a riskier shot).** `composites::insert` already adds a saved package to the
   live patch (`panel:import-package`). The same Disco part exported as a package can be dropped into the rack
   on camera. Needs a test run first: the patch recompiles while it plays (listen for a click), and the new part's macro has to
   be wired.
3. **A Sounds preview over the rack.** `sound_preview` mixes a 4 s clip on top of the rack's output (same
   device). It is not tempo-synced, so use it only while the rack is stopped (shot 8).

## Shot list (Lichen grid: 112 BPM, 1 bar = 2.143 s; 40 bars = 85.7 s)

Two takes, joined with a hard cut on the downbeat of bar 8. Take A uses a small seed patch at 112 BPM; take B
is `lichen-plus`, played without a stop from bar 8 to the end. Look B (dark) for the whole video; Perform is
teal; the cables use the factory palette with living cables on.

| # | Time | Bars | View | Patch / state | Actions (drive keys) | Audio, sync |
|---|---|---|---|---|---|---|
| 1 | 0:00-0:08.6 | 0-4 | Rack, empty | take A: empty | bar 0 `click empty-add`, `add:seq`, `add:osc`; bar 1 `add:out`; bar 2 `drag out:seq.gate in:osc...` then `drag out:osc.out in:out.in` | silence until the first note on bar 2 beat 1; the cable starts pulsing on that note |
| 2 | 0:08.6-0:17.1 | 4-8 | Rack | take A: + LFO, filter | bar 4 `add:lfo`; bar 5 `drag` LFO to the filter cutoff (cable sways); bar 6 `dragby knob:{f}.cutoff_hz` | one voice that opens up; a 1-beat riser into bar 8 |
| 3 | 0:17.1-0:25.7 | 8-12 | Rack, `zoom:fit`, `view:All` | take B: Lichen-plus, Spore, Focus 0.1 | bar 8 Run (anchor); the camera holds, the living cables carry the 7- and 5-loops | Spore: bass, 7-loop, kick; **cut on the bar 8 downbeat** |
| 4 | 0:25.7-0:34.3 | 12-16 | Rack + pattern drawer | Spore | bar 12 `rclick cable:{7-loop}`, `menu:cable-pattern` (its step pattern is shown); bar 13-15 `slide` on `fn-morph` A to B | the 7-loop's rhythm changes over 2 bars |
| 5 | 0:34.3-0:42.9 | 16-20 | **Perform** (teal) | Colony | bar 16 `click perform`, `click pcue:{m}.2` (Colony, 16-beat morph: the macros move on their own over bars 16-20) | the loops mutate; Focus 0.35 |
| 6 | 0:42.9-0:51.4 | 20-24 | Perform | Colony | bar 20 `pbank` switches to sequencer bank B (or `bank-edit` adds one); bar 22-24 timed drag of `pslider:Focus` 0.35 to 0.8 | the part changes on bar 20; voices come forward |
| 7a | 0:51.4-1:00.0 | 24-28 | Perform, then Rack | **Bloom** | bar 24 `pcue Bloom` (8-beat morph, full at 26); bar 26 `pcue Bloom+`, Disco fades in over bars 26-28 | **peak 1**: all 8 Lichen voices plus the disco bass and stabs |
| 7b | 1:00.0-1:08.6 | 28-32 | alternate `rack`/`perform` each bar | Bloom+ | 1-bar view cuts on the downbeats of 28-31; bar 30-31 timed drag of `pslider:Disco` from 1 to 0.6 and back | **peak 2**, the densest bars; bar 31 beat 4 `pstop` |
| 8 | 1:08.6-1:17.1 | 32-36 | **Sounds** | rack stopped | `click browser`; a bar ahead of each: `sound:{id}` (render); on the downbeats of 32, 33, 34 and 35: `play:laundromat`, `play:balcony`, `play:gantry`, `play:sock-puppet` | a 4-preset montage, one per bar (each play stops the previous clip); the cuts fall on Lichen's grid, the clips keep their own tempos |
| 9 | 1:17.1-1:25.7 | 36-40 | Perform, then an end card | Die-back | bar 36 `prun` and `pcue Die-back` (16 beats); bar 38 end card: the kabl lockup, "1.0: a whole song in one patch" | the melodies sink into their echo; fade out over bars 38-40 |

The song structure the owner wants, a whole song in one patch, is shots 3 to 7 and 9: one patch, with cues,
sequencer banks and macros. Shots 1 and 2 show the empty start. Shot 8 shows the rest of the bank.

## Gaps in the tool, and the smallest change for each

1. **No scheduling except morphs.** Extend `make-drive-script.py` with a storyboard input: `{"bpm": 112,
   "anchor": "prun:{clock}", "shots": [{"bar": 16, "do": ["click perform", "click pcue:12.2"]}, ...]}`, which
   writes `wait t0+bar*spb` followed by the raw commands. About 20 lines; the morph path stays as it is.
2. **Bar-0 anchor.** `restart:1` is tied to one module. Make the anchor key a field in `targets.json` (use
   `prun:{id}` or `prestart:{id}` in Perform, or a rack Run/Restart key that exists).
3. **Only horizontal timed drags.** Macro knobs (`pslider:`) are dragged vertically. Add `slidey DY SECONDS` to `drive.py`
   (a copy of `slide` that moves on y), about 6 lines.
4. **Layering.** Write the `promo_v1.rs` generator (combined patch, Disco macro, Bloom+ cue). It is a
   generator in the style of `presets_piece.rs` and needs no engine change. The optional on-camera insert needs nothing
   new except `type` into `panel:path` and a click on `panel:import-package`.
5. **Perform cue clicks.** The hit keys exist (`pcue:{id}.{n}`), so no app change is needed. Only `targets.json` needs
   the cue numbers for Lichen-plus, found once from the hits file.
6. **Sounds preview.** It works with `play:{id}`. A clip renders when its row is selected (up to 4 s of work,
   2 workers), so the script selects each `sound:{id}` at least 1 bar early, or all four during shot 7.
7. **Fixed values in `record.sh`.** It has 30 fps, the sink `kabl_rec` and one take. Add `FPS`, `SINK` (default
   `kabl_agents`, per the repo rule) and a take loop over `(patch, drive script)` pairs.
8. **`post.py` assembles a single take only.** Add the concatenation of the takes on the cut bar, a 20 ms audio crossfade at the
   cut, and the end card (drawtext or ASS over the last 2 bars). Keep the envelope bar-0 detection for each take
   (each take needs a reference render: `promo_v1 render`).
9. **Risk: frame rate under Xvfb.** The app draws with llvmpipe (no GPU). Living cables and a full Lichen rack at
   1080p may not reach 60 fps. Check `KABL_STATS_FILE` on a dry run. If the frame time is over 16 ms, record at 30
   and say so, rather than ship duplicated frames.

## Output spec

- Video: 1920x1080, **60 fps**, H.264 High, `-crf 18 -pix_fmt yuv420p -movflags +faststart`. Capture
  with `x11grab -framerate 60 -draw_mouse 1`, encoded as `libx264rgb -preset ultrafast -qp 0` during the capture
  (so no frames drop), and re-encoded in post.
- Audio routing: real app audio only. Run the app with `PIPEWIRE_NODE=kabl_agents PULSE_SINK=kabl_agents`, capture
  `-f pulse -i kabl_agents.monitor`, and check with `pactl list sink-inputs` that the stream is on `kabl_agents`
  (if not, `pactl move-sink-input <id> kabl_agents`). If the sink is missing, `pactl load-module
  module-null-sink sink_name=kabl_agents sink_properties=device.description=kabl-agents-silent`. Never use
  the default sink, and never let it play out loud.
- Final mux: take A trimmed to bars 0-8 and take B trimmed to bars 8-40, joined with the concat filter, with a 20 ms
  `acrossfade` at the cut and a fade-out over bars 38-40. Then `loudnorm` in two passes to -14 LUFS, true peak
  -1 dBTP, encoded as AAC 192 kb/s at 48 kHz. Output `docs/promo/kabl-1.0-promo.mp4`, about 86 s. No burned-in captions
  (the end card is the only text) unless the owner asks for them.
- Checks before the owner's review: sync within 1 frame at the bar 8 cut and within 0.1 s at each cue
  launch (the cue morphs start on the click); no clipping; the frame rate measured; then the owner watches it.

## Open decisions for the owner

- Should the peak layer be Laundromat's disco part (the plan above) or Balcony's chords and arpeggio?
- Shot 8: is a montage of previews at their own tempos acceptable, or should the montage be cut from the take-B
  audio instead?
- Is the on-camera package insert (method 2) worth the risk, or should only the cue Bloom+ be used?
