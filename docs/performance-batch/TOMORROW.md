# Tomorrow: playing the performance batch

The batch is **closed: Kosta approved it hands-on (2026-09-24)**. This page is for your first hands-on session. The full
record is [`README.md`](README.md).

## Launch

From the repo root:

    cargo run --release -p kabl-ui -- --patch patches/performance --perform --rate 48000 --frames 256

- `--rate 48000 --frames 256` is the measured setting (5.3 ms buffers). On this laptop, with
  real-time priority granted, it ran with 0 late callbacks and 0 xruns in every measured run,
  including the 30-minute soak. The status bar says what you actually got: sample rate,
  channels, the buffer size requested, and a callback line (`callback worst …/5333 µs · 0 late
  · 0 xruns · RT on`). Hover it for the full report.
- **`RT on` matters.** kabl asks for real-time priority for its audio thread through rtkit.
  If the status bar says `RT refused`, expect glitches at small buffers; tell me what it says.
- 64 frames also ran clean with RT on. 96 kHz / 256 ran clean with RT on. 96 kHz / 64 doesn't
  run on this PipeWire setup: the status bar then says `audio stalled`.
- Window: 1440×900 by default, `--size 1280x800` for the smaller one.

## Your controller

1. In the **Perform** panel (bottom), open **MIDI in** and pick your keyboard (the name
   contains "KL" / "Essential"). Or add `--midi Essential` to the launch command.
2. The lead plays from the keys straight away.
3. The demo's controls are already mapped to **CC 20–29 on channel 1**. Your knobs probably
   send other numbers, so learn them: on a card, click **Learn** (or the `CC …` badge), turn a
   knob. The badge then shows your knob's CC and channel. Learning a knob that's already used
   moves it to the new control. **✕** removes a mapping. Escape or **Cancel learn** stops.
4. A mapped control doesn't move until your knob reaches its value (soft takeover). The badge
   shows `↑ 42%` / `↓ 42%` (which way to turn) until then, and `● live` once the knob has it.
   A mouse edit or an undo on that control hands it back to the mouse: the knob picks it up
   again when it gets there.
5. **Save** (toolbar) keeps your mappings and labels with the patch. Save to a new folder,
   e.g. `patches/my-performance`, so the demo stays as it is.

If the keyboard is unplugged, kabl releases its notes and reconnects on its own when it comes
back (within about 2 s). **All notes off** in the panel releases every MIDI voice at any time;
it doesn't stop the sequencers.

## Control map

| Card | What it is | CC (ch 1) |
|---|---|---|
| Transport | Clock #1: Stop/Run, Restart (step 1 on the next tick) | – |
| Bass | Mixer #26 level 1: the bass sequence | 20 |
| Arp | Mixer #27 level 1: the arp sequence (7 steps against the bass's 8) | 21 |
| Pad | Mixer #27 level 2: the drone pad | 22 |
| Lead | Mixer #28 level 1: your keyboard, into the echo | 23 |
| Bass cutoff | SVF Filter #6 cutoff | 24 |
| Arp cutoff | SVF Filter #11 cutoff | 25 |
| Pad cutoff | SVF Filter #19 cutoff | 26 |
| Bass transpose | Sequencer #3 transpose (±24 st) | – |
| Echo feedback | Delay #29 feedback | 27 |
| Reverb | Reverb #32 mix | 28 |
| Reverb decay | Reverb #32 decay (0.3–30 s) | 29 |

Double-click a card's title (or use its **⋯** menu) to rename it; the grey line under it
always says what it really is. **⋯** also moves and unpins cards. To pin anything else:
right-click a module → **Pin to Perform**. **Taller** gives the panel more rows.

The **Out** meter (status bar, bottom left) is the measured final output: green, yellow above
−6 dBFS, and a **CLIP** light that stays on until you click the meter. There is no limiter.
The demo peaks around −4 to −6 dBFS; a four-note chord on the lead at full velocity peaks at
−2.6 dBFS.

## Five-minute first play

1. Launch; check the status bar says `48000 Hz … RT on`, and the Out meter moves.
2. Pick your keyboard; play a few notes over the sequence: the lead echoes left and right.
3. Learn two knobs: **Bass cutoff** and **Reverb**. Turn them; watch pickup, then `● live`.
4. Turn both at once, then Ctrl+Z: both go back in one step.
5. Stop and Run the transport card, then Restart: tails ring through Stop.
6. Open the bass sequencer's advanced area (`+12` on Sequencer #3): the velocity row V1–V8
   and Gate (CLOCK/LENGTH), Gate length. Try an accent, and CLOCK against LENGTH.
7. **● Record**, play for a minute, stop the sequencer, let the tails ring, **■ Stop**.
8. **Open folder** (or **Copy path** next to *Last take*) and play the file.

## Recordings

- Takes go to `recordings/` in the repo root by default (the **to** field changes it; the
  folder is created if missing; `recordings/` is git-ignored). Names are
  `kabl-YYYYMMDD-HHMMSSZ.wav` (UTC), with `-2`, `-3` … rather than ever overwriting a file.
- 32-bit float stereo WAV at the device rate: exactly what went to the speakers.
- After Stop the panel shows **Last take** with its length; **Open folder** opens the folder,
  **Copy path** copies the file's path.
- A take that lost audio or hit a write error is renamed `…-INCOMPLETE.wav` and shown in red.
  Quitting (window close or Ctrl+Q) finishes the take properly.

## Limits

Verified automatically (tests and the real app driven on a virtual display, with a virtual MIDI
controller and a silent audio sink):

- everything above, including unplug/replug, simultaneous knobs, undo grouping, pickup, record
  → stop → record, Load while recording, bad folders, forced write errors and overflow, quitting
  while recording, and a 30-minute soak (see README).

Not verified yet (needs you):

- a hardware controller: its knob feel, its CC numbers, whether 1.5/127 pickup feels right;
- listening on speakers or headphones;
- the audio thread's priority on your own session (look for `RT on`).

Known limits: one MIDI input at a time; mappings are channel + CC (not tied to a device name);
absolute 7-bit CC only; switches (stepped controls) can't be learned; the unplug check runs every
2 s; a killed process loses up to 0.5 s of its take; the Out meter is peak only (no loudness);
Pi 4 unmeasured.

## Evidence

- Record and verification: [`README.md`](README.md) ("Follow-up" sections).
- Walkthrough video: [`walkthrough.mp4`](walkthrough.mp4); screenshots: [`img/`](img/).
- Musical take (recorded by kabl): [`audio/performance-take.m4a`](audio/performance-take.m4a).
- Code and evidence: `master` (local), review package at `416cd24` plus this page's commit;
  `git log --oneline 4a632ea..` lists the follow-up. Callback measurements and the soak:
  README "Callback misses: measured" and "30-minute soak".
