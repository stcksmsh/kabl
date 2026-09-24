# Sound Palette + Playable Voices

**Built, waiting for Kosta's hands-on listening and keyboard review.** Supervisor scope
(2026-09-24), authorized as one batch: distinct playable strings, pads, leads, basses,
textures and percussion. Composition + Motion is a separate batch, also still waiting for
its review; neither is approved here. Rationale: `docs/decisions.md`, "Sound palette batch".
The keyboard rules were written first: [keyboard.md](keyboard.md).

    cargo run --release -p kabl-ui -- --patch patches/sound-palette --perform --rate 48000 --frames 256
    cargo run --release -p kabl-ui -- --patch patches/palette/lead --perform --rate 48000 --frames 256
    # patches/palette/{strings,pad,lead,bass,breath,perc}; add --size 1280x800; --midi "KL Essential"

Start with the [hands-on checklist](CHECKLIST.md) and the [patch guide](PATCHES.md).

**Automated vs. owner evidence — read this first.** This batch was finished in a cloud
container, not on Kosta's laptop: a 4-vCPU Xeon VM (~2.1× slower than the i7-13700H on the
same benchmark), no sound card, no ALSA sequencer, no rtkit, software OpenGL. Everything
below was checked by tests, offline renders through the real engine, or the real release
app on a virtual display (Xvfb) with audio to a silent PipeWire sink and MIDI from the
virtual controller through a fifo stand-in (`KABL_MIDI_PIPE`, below). The takes and the
walkthrough are **scripted performances**. Sound on speakers, feel on a keyboard, hardware
MIDI unplugging and the callback figures on the real machine are Kosta's checks.

## What was built

### Stage A–B: modules (committed earlier in the batch, `ffdbd26..5c0c1c4`)

- **`osc.va`**: pulse width `pw` 5–95 % (ramped per sample, DC removed so PWM doesn't
  thump), `fine` ±100 ct, `unison` 1–4 per voice (level 1/√N, 20 ms fades when the count
  changes, a fixed spread table), `detune`, and band-limited hard sync (the reset edge is
  placed between samples and gets a PolyBLEP residual; in-block lookahead). Defaults are the
  old oscillator sample for sample; the new controls are in its advanced area.
- **`noise`**: white (xorshift32) or pink (Paul Kellet's refined filter), both −14 dBFS
  RMS at 0 dB, seeded by module id and voice lane, carried across live edits.
- **`filter.ladder`**: 4-pole zero-delay-feedback ladder (Zavalishin's TPT one-poles with
  a tanh input stage), k = 4.4·resonance, input compensation 1 + k/2, drive 0–24 dB (advanced),
  self-oscillates above ~91 %.
- **`chorus`** (stereo, global): one swept tap per side plus a small fast vibrato, width =
  the right side's sweep offset, equal-power mix exact at the ends, lines carried across
  edits. (A 3-tap version notched the wet by 10 dB around 100–200 Hz; module doc.)
- **`drive`**: y = tanh(g·x)/tanh(g), g = 10^(dB/20) − 1; exact bypass at 0 dB with a 10 ms
  engage fade; first-order ADAA; trim; mix (advanced).

### Keyboard expression (rules first: [keyboard.md](keyboard.md))

- The keyboard state machine lives on the audio thread in `PatchEngine`, one per `midi.in`,
  outside the graphs (`crates/engine/src/keyboard.rs`), so live edits neither lose nor
  replay a key. POLY (LRU voice, pedal-aware steal), MONO and LEGATO with LAST/LOW/HIGH
  priority, glide OFF/ALWAYS/LEGATO at constant time, a one-sample gate dip for envelope
  restarts, sustain pedal, All Notes Off.
- **Settings belong to the `midi.in`** as ordinary params (saved, undoable): `mode`,
  `priority`, `glide`, `glide_ms`. Its face (now 6 units wide) shows the keys, **a line that
  names all four settings** (`LEGATO · LAST · glide 90 ms`, including the ones off the
  face), then Mode and Glide as full-width selectors with the step labels POLY/MONO/LEGATO
  and OFF/ALWAYS/LEGATO; Priority (LAST/LOW/HIGH) and Glide time are in the advanced area.
  The outputs moved to a row on a plate like every other module. What doesn't fit above
  them goes to the advanced area (all four on the face: the three selectors, no picture).
- **MIDI wiring** (`crates/ui/src/main.rs`): every message goes through
  `KeyEvent::from_midi`; note on/off, CC 64, CC 120 and CC 123 are key events sent through an
  `rtrb` queue and played by `engine.key()` on the audio thread; they never reach the CC
  learn queue. All notes off, a disconnect and a port switch push `KeyEvent::AllOff`. The
  stats line's "MIDI notes held" (and new "voices sounding") come from `engine.keys()`
  through an atomic. `VoiceAllocator` is no longer used by the app (`kabl-standalone`
  keeps it).

### Engine: per-voice noise

Found while testing: a `noise` module has no inputs, so no `midi.in` ever reached it and it
compiled to **one shared stream** even when it fed a per-voice VCA — a chord added noise in
amplitude (+6 dB per doubling), not power. Now a noise chain whose every consumer is a
MIDI voice chain runs per voice (`compile.rs`, after the voicing fixpoint); a noise that
also feeds a path no MIDI reaches stays one lane-0 instance, so that path keeps its level
(a per-voice stream there would become a quiet voice average). Tested in
`tests/sound_palette.rs`: +3.0 dB per doubling per voice, +6.0 dB shared, lane 0 alone.

### Stage C: the patch library and the piece

Six patches in `patches/palette/` and the piece patch `patches/sound-palette`, all built by
`crates/ui/tests/sound_palette.rs` (writer + "committed = builder" check + headroom). They
differ by synthesis, not by effects: an ensemble of unison saws with vibrato and drift;
PWM + noise into a breathing driven ladder; a synced, driven mono lead with legato glide;
a resonant accented sequence bass; per-voice key-tracked band-passed noise; a synthesised
kit. Descriptions, controls and CC maps: [PATCHES.md](PATCHES.md).

The piece patch holds all of them. Since every `midi.in` hears every key (as before; no
keyboard zones in scope), the keyboard voices are **layers on faders** (CC 24–27); the
sequences are switched by five cues on the next bar (Air, Pulse, Groove, Lift, Break). It
starts on Air (sequences resting) so it opens on the atmosphere.

**Headroom** (`cargo test --release -p kabl-ui --test sound_palette headroom -- --nocapture`;
offline, 48 kHz; a note or chord held 3.9 s of a 7 s render, sequences 8 bars; the right
column has every macro at 1; for the piece: breath 0.4, strings/pad/lead 0.8, Lift banks):

| patch | take | peak dBFS | RMS dBFS | peak, macros at 1 |
|---|---|---|---|---|
| palette/strings | one note (C4) | -13.6 | -27.4 | -11.5 |
| palette/strings | 4-note chord | -6.1 | -20.9 | -6.0 |
| palette/strings | 6-note chord | -4.5 | -19.5 | -3.3 |
| palette/pad | one note (C4) | -13.5 | -27.1 | -11.0 |
| palette/pad | 4-note chord | -6.8 | -20.7 | -5.0 |
| palette/pad | 6-note chord | -4.1 | -19.6 | -2.5 |
| palette/lead | one note (C4) | -6.4 | -19.2 | -7.0 |
| palette/lead | 4-note chord | -8.0 | -20.0 | -7.5 |
| palette/lead | 6-note chord | -6.4 | -19.2 | -7.0 |
| palette/bass | 8 bars | -6.1 | -13.5 | -5.6 |
| palette/breath | one note (C4) | -12.8 | -28.1 | -9.9 |
| palette/breath | 4-note chord | -7.2 | -21.7 | -3.5 |
| palette/breath | 6-note chord | -5.8 | -20.4 | -1.2 |
| palette/perc | 8 bars | -4.4 | -17.4 | -2.6 |
| sound-palette | one note (C4) | -26.0 | -40.9 | -4.6 |
| sound-palette | 4-note chord | -19.1 | -34.4 | -3.3 |
| sound-palette | 6-note chord | -18.4 | -32.7 | -2.6 |

Tuning decisions made from these numbers (logged in decisions.md): poly gains set so a
6-note chord peaks near −4 dBFS (one note is therefore −13 dBFS peak); the lead's gain into
its drive lowered from +20 to +11 dB after the drive comparison showed it was clipping a
+3 dBFS input rather than shaping it; the piece has a −4 dB master trim. There is no
limiter; all four piece faders and all macros at the top can clip.

## Evidence (all scripted performances)

- **[The piece](audio/sound-palette-take.m4a)** (9:34, AAC of the 32-bit WAV kabl's
  recorder wrote in the real app; script `scripts/piece.txt` from `scripts/make_piece.py`).
  An original piece in E minor at 104 bpm. Peak −3.6 dBFS, RMS −20.1 dBFS, no NaN, stereo
  (L−R −12.8 dB), complete. One second under −60 dBFS inside the piece (1:36, the breath
  before the drums) and the last 3 s (tails gone before the recorder stops). Recorded at
  **48 kHz / 512 frames** in the container (below, "Callbacks"): the recorder taps what the
  engine renders in 64-sample blocks, so the audio is the same at any callback size.

  | at | section |
  |---|---|
  | 0:00 | Atmosphere: breath chords alone, Space and Motion opening |
  | 1:36 | Foundation: cue Pulse (kick pulse, hat tick); 1:59 cue Groove (kit, bass line); breath fades |
  | 3:06 | Strings in (Em–Cmaj7–D6–Bm7…), 3:46 the pad under them; Motion, Bright up |
  | 4:49 | Lead: cue Lift; strings out, a little pad as halo, the mono lead with echo; legato phrases with glide, detached call-and-answer, vibrato swells on Motion |
  | 6:34 | Breakdown: cue Break (bass held notes, hat ticks); breath and pad back, big Space |
  | 7:49 | Return: cue Groove, 8:26 Lift; strings and pad; a last lead phrase |
  | 9:11 | Close: cue Air, a 6-note chord rings, 9:27 Stop (tails), 9:34 recording stopped |

- **Isolated examples** ([audio/](audio/), each patch alone, ~30–45 s, 48 kHz / 256
  frames): `example-<patch>.m4a` as saved, `example-<patch>-reduced.m4a` with its space
  macro (Space/Echo/Room) at 0 — reduced effects, the synthesis unchanged. The bass has no
  reduced take (it is dry). Scripts: `scripts/examples/` from `scripts/make_examples.py`.
- **Comparisons at matched levels** (`audio/compare-<feature>-ab.m4a`: A = off, 2 s of
  silence, B = on; **offline renders** through the same compiler and engine, since exact
  A/B needs identical playing): PWM (pad, LFO→width route bypassed), unison (strings, 1 vs
  4+2), chorus (strings, mix 0 vs saved), drive (lead, 0 dB vs saved). Both halves are
  scaled to −20 dBFS RMS; the adjustments:

| comparison | patch | A (off) RMS | B (on) RMS | matched RMS | A gain | B gain |
|---|---|---|---|---|---|---|
| pwm | palette/pad | -19.4 | -19.5 | -20.0 | -0.6 dB | -0.5 dB |
| unison | palette/strings | -19.1 | -18.9 | -20.0 | -0.9 dB | -1.1 dB |
| chorus | palette/strings | -19.1 | -18.9 | -20.0 | -0.9 dB | -1.1 dB |
| drive | palette/lead | -15.9 | -17.7 | -20.0 | -4.1 dB | -2.3 dB |

- **[Walkthrough video](walkthrough.mp4)** (2:26, 1280×800 video of the 1440×900
  window, stereo audio from the app's output; timeline [walkthrough-marks.txt](walkthrough-marks.txt)):
  breath, strings unison, pad PWM, Groove from a button with the bass ladder opening,
  the lead in LEGATO then MONO (clicked on its face) then back, the drive and chorus faces,
  Break, Stop with tails. Script `scripts/walkthrough.txt`, recorder `record-walkthrough.sh` (run at 512 frames in the container, as the take).
- **Screenshots** ([img/](img/), each at 1440×900 and 1280×800 in A-light and A-dark):
  `01` MIDI In with its advanced area, `02` VA oscillator (PWM pad) advanced, `03` synced
  pulse, `04` ladder, `05` chorus, `06` drive, `07` noise, `08` macros, `09` the piece's
  Perform panel mid-Groove. At 1280×800 the piece's twelfth card (Glide time) is below the
  panel's fold (the panel scrolls, or Taller).
- **Keyboard in the real app** ([evidence/keys-real-app.txt](evidence/keys-real-app.txt)):
  two keys then the pedal holding them after key-up, pedal up; a held key when the
  controller is unplugged (released, "0 held") and plugged back (its late note-off
  ignored); a held key across a knob drag and an undo (stays); across Load (forgotten);
  All notes off with the pedal down; Restart and Stop/Run with a key held (untouched).
  Ends at 0 held, 0 sounding.

## Verification

`cargo test --workspace`: **407 pass**, 13 ignored (fixture/patch/clip/render writers).
`cargo clippy --workspace --all-targets`: clean. New or changed checks:

| area | where | what |
|---|---|---|
| Palette modules | `crates/modules/tests/{osc_palette,noise,ladder,chorus_drive}.rs` | tuning, PW duty and DC, unison level and fades, band-limited sync, alias tables, noise spectra/levels/seeds, ladder stability/gain/self-oscillation, chorus flatness/width/continuity, drive bypass/level/ADAA/latency |
| Keyboard | `crates/engine/tests/keyboard.rs` (19) | every rule in keyboard.md, event by event |
| Swaps and lifecycle | `crates/engine/tests/sound_palette.rs` (6, no allocation) | self-swap null with every new module in POLY/MONO/LEGATO (single and overlapping swaps, < 1e-4); two choruses and two drives keep their own histories under swaps; noise per voice (+3 dB), shared (+6 dB), lane 0 alone; build through the op log, save, reload (bit-identical), an edit and its undo, a fresh Load carries nothing; old patches unchanged by the new params |
| UI | `crates/ui/src/rack.rs` | every control inside its panel and apart (all kinds, face/advanced/all-primary); the MIDI In face: keys, Mode, Glide, room for LEGATO/ALWAYS, overflow to the advanced area |
| Patches | `crates/ui/tests/sound_palette.rs` | committed = builder, values in range, compiles, four named macros, ≥ 7 pins, ≥ 5 CC maps; headroom table |
| Recorder | `crates/ui/tests/record.rs` | the `/dev/full` case skips when run as root (/dev is writable there) |

## Callbacks

Tested setting 48 kHz / 256 frames. **All numbers are from the container, not the laptop.**
The same `bench_composition` that measured a 140 µs median on the i7-13700H measures
293 µs here, so this VM is about 2.1× slower; it also has host stalls (an offline loop with
no other work saw single 8 ms callbacks). Estimates for the laptop below are that ratio
applied and are not measurements.

**Offline** (`cargo run --release -p kabl-ui --example bench_palette 48000 256`: the whole
callback as `main.rs` runs it, recorder on; [evidence](evidence/bench-palette-48k-256.txt)):

| scene | steady median / p99 / max µs | during continuous edits (crossfading) median / p99 | swap callbacks median / max |
|---|---|---|---|
| piece dense: 6-note chord on all four layers, Lift banks, chorus + drive + echo + 2 plates | 1606 / 1843 / 3301 | 3251 / 4927 | 3514 / 6923 |
| strings: 8 voices × 2 oscillators × 4-voice unison | 414 / 568 / 2041 | 818 / 1189 | 842 / 3291 |
| lead: sync, ladder, drive, echo, plate | 387 / 731 / 8235* | 779 / 1064 | 913 / 1588 |

(* one host stall.) A key event every callback costs nothing measurable (median
unchanged). "Continuous edits" = a macro edit every 4th callback, as a turning knob sends:
each swap crossfades two graphs for 15 ms, so most callbacks then run two graphs (≈2×).
Where the piece's time goes ([parts](evidence/bench-palette-parts.txt)): the mono lead
~290 µs (its chain still runs 8 voice lanes though only voice 0 plays), strings ~280,
pad ~100, breath ~60; the rest is the global chain (two plates, chorus, echo, drums, bass).
Estimated on the laptop: piece ~0.8 ms steady and ~1.6 ms while a knob turns (30 % of
the budget) — to be measured there.

**Real app** (release, Xvfb, PipeWire null sink, fifo controller; rtkit is absent, so the
audio thread was given SCHED_FIFO 80 with `chrt`, which is what rtkit grants on a desktop;
the stats line still says "RT priority refused" because the app's own request fails;
[evidence/callbacks-real-app.md](evidence/callbacks-real-app.md)):

| run | frames | callbacks | execution worst µs | over half | late | arrival worst µs | late arrivals | xruns |
|---|---|---|---|---|---|---|---|---|
| example bass | 256 | 6867 | 2588 | 0 | 0 | 11812 | 4 | 0 |
| example breath-reduced | 256 | 7841 | 4073 | 3 | 0 | 24501 | 14 | 2 |
| example breath | 256 | 7670 | 3772 | 5 | 0 | 10651 | 8 | 0 |
| example lead-reduced | 256 | 7814 | 7537 | 5 | 2 | 19939 | 13 | 0 |
| example lead | 256 | 7428 | 5497 | 1 | 1 | 34686 | 13 | 2 |
| example pad-reduced | 256 | 8821 | 5215 | 7 | 0 | 17169 | 5 | 0 |
| example pad | 256 | 8592 | 3090 | 1 | 0 | 11164 | 6 | 0 |
| example perc-reduced | 256 | 6518 | 7015 | 4 | 1 | 10313 | 3 | 0 |
| example perc | 256 | 6142 | 2379 | 0 | 0 | 9680 | 3 | 0 |
| example strings-reduced | 256 | 9012 | 4220 | 6 | 0 | 36039 | 9 | 2 |
| example strings | 256 | 8822 | 5008 | 3 | 0 | 32087 | 14 | 2 |
| piece take | 512 | 54394 | 23781 | 1926 | 65 | 108101 | 17 | 3 |
| walkthrough | 512 | 13956 | 35839 | 580 | 8 | 59058 | 3 | 1 |

The piece at 256 frames in the container: four attempts. The first ran 9:36 with 158 late
callbacks and 20 xruns (and a composition gap, so it was redone); in two others the
PipeWire-ALSA stream **stopped calling back** after a burst of xruns (at 6:38 after 12, at
3:24 after 4: the callback count froze while the app kept running); one was a script race. At 512
frames it ran complete twice (5 and 3 xruns). So in this container the dense piece does not
run cleanly at 256 frames, and an xrun burst can stall the stream for good. Whether the
latter happens on the laptop's PipeWire is unknown — the earlier batches there had 0
xruns, so it was never exercised; it is on the checklist.

**The Composition watch item** (one unexplained 3–5 ms callback per 9-minute take on the
laptop): not reproduced or explained here. In this VM, host stalls of 8–100 ms appear even
in offline loops, so they would hide it. Still open; not attributed to any cause.

Not claimed: 96 kHz / 64-frame operation; anything on a Pi 4.

## Limits

- Every `midi.in` hears every key (no zones/splits): the piece uses layer faders.
- A mono `midi.in` (MONO/LEGATO) still compiles its chain for all voices; only voice 0
  plays. It is the piece's single largest cost (above); a follow-up could compile such a
  chain once.
- PolyBLEP limits at high notes (saw worst alias −49 dB at 440 Hz, −37 at 1760, −31 at
  3520; the naive saw −38/−26/−20; unison 4 at 1760 Hz −26); triangle and sine are not
  band-limited (their harmonics fall fast). Sync is 6–9 dB better than the old naive sync
  (−48.5 vs −40.6 at 220×2.37; −33.8 vs −25.3 at 880×3.3).
- **Drive anti-aliasing is ADAA only, no oversampling**: 6–10 dB less on the worst alias
  component (7–14 dB in total alias power) than the naive curve where it matters (24 dB drive at 2489 Hz: −35.9 vs −27.9 dB worst; 36 dB drive at
  4978 Hz: −27.2 vs −17.4). Heavy drive on high notes still aliases audibly.
- The chorus wet path rolls off −0.5 dB at 12 kHz; 50 % mix is −0.5 dB on noise, +1 dB on
  a 30 Hz sine (in-phase lows).
- Modulation stays block-rate (the kick's pitch sweep steps every 1.3 ms).
- Poly patches are quieter per note than the mono lead (chord headroom); no limiter.
- Container-only evidence (top of this file). Carried from earlier batches: 96 kHz / 64
  frames doesn't run on the laptop's PipeWire; Pi 4 unmeasured.

## Test hooks added

- `KABL_MIDI_PIPE=PATH` (`main.rs`): an input named `kabl-pipe` listed while the fifo PATH
  exists, read through the same message handler as a real port; removing the fifo is an
  unplug. `examples/midi_player` writes it when the variable is set; drive.py's `midikill`
  removes it. For machines without an ALSA sequencer only.
- drive.py: `wait T` (absolute time, so long scripts don't drift), `goto KEY X Y`
  (wheel-pan to an off-screen target), `KABL_APP_LOG`, a 10 s target wait, tolerant hits
  reads.

## Sources and licenses

All DSP written from the equations or papers; no code copied. PolyBLEP (Välimäki et al.; e.g.
Välimäki & Huovilainen, "Antialiasing oscillators in subtractive synthesis", IEEE SPM 2007); pink noise
filter by Paul Kellet (musicdsp.org, 2000, public domain); xorshift32 (Marsaglia 2003);
ladder: Zavalishin, *The Art of VA Filter Design* rev. 2.1.2 (2018), Stilson & Smith
(ICMC 1996), topology after Moog's US 3,475,623 (expired); ADAA: Parker, Zavalishin & Le
Bivic (DAFx 2016), Bilbao, Esqueda, Parker & Välimäki (IEEE SPL 2017); plate reverb as
before (Dattorro 1997). No new dependencies (tempfile, already in the workspace, became an
engine dev-dependency).

