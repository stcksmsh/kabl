# D04 (+ D03-R2) owner checklist (Kosta, laptop)

Everything here is **pending**. The cloud evidence (README) is scripted on a VM: Xvfb, a
PipeWire null sink, the fifo MIDI stand-in, no RT priority. It is not listening, controller
feel, beginner or laptop evidence.

Launch (48 kHz / 256, your controller):

    cargo run --release -p kabl-ui -- --patch patches/composition --perform --rate 48000 --frames 256

Or the package: `packaging/linux/package.sh`, extract `target/dist/kabl-0.1.0-linux-x86_64.tar.gz`
anywhere, run `bin/kabl-ui --patch share/kabl/patches/composition --perform --rate 48000 --frames 256`.
For the counters below add `KABL_STATS_FILE=/tmp/kabl-stats.txt` in front.

| # | Do | Expect | Pass/fail, notes |
|---|---|---|---|
| 1 | Let the piece play. Turn **Macros m1** on the rack, drag the **m3** card slider, and turn the controller knobs mapped to CC 20–24 (macros, Lead level) for a while. | Each move is heard at once and smoothly: no clicks, gaps, restarts of the sequence, envelopes or echoes/reverb tails. `/tmp/kabl-stats.txt` line "control:" shows 0 graphs compiled and many runtime values. | |
| 2 | Turn a mapped knob far from where the on-screen value is. | Nothing moves until the knob reaches (or crosses) the value, then it follows (unchanged pickup). | |
| 3 | Minimise the window (or leave it covered) and turn the mapped knobs; press the mapped Run/Stop (CC 46) and Restart (CC 47) and a cue button (CC 40–45). | Sound, transport and cues respond with no window drawn. Bring the window back: values show where the knobs left them; nothing jumps or replays. | |
| 4 | While a mapped knob is moving, plug a cable (e.g. Osc #10 out into Mixer #22 in4). Then Ctrl+Z twice, Ctrl+Shift+Z twice. | The knob keeps working through the edit; the new cable is heard; undo/redo step through the CC movement and the cable. | |
| 5 | Turn mapped knobs, then **Save** (or Save As), quit, reopen. | The saved sound has the values the knobs left; Ctrl+Z after a knob turn undoes the whole turn as one step. | |
| 6 | Routing drawer → **Compare**: Capture, change controls, Restore reference, Back to my version. | Each switch is heard at once; values match the reference / your version. | |
| 7 | **Inspect** VCA #14 out and keep turning a macro. | Readings stay current ("updated 0.0x s ago") while you turn; plugging or pulling a cable restarts them. | |
| 8 | **D03-R2**: Init Keyboard, hold a key, Inspect **VCA #4 out**, pull the cable out of **VCA #4 in** (drag the plug away), open **Why no sound?** | Sound stops. Facts: "No cable into VCA #4 in: that is its audio input…"; the envelope on its cv is not offered as the source; "Audio outputs that feed nothing: Filter #3 lp…" as a possibility. Ctrl+Z restores cable, reading and sound. | |
| 9 | Same patch, Undo first: remove the envelope's cable into **VCA #4 cv** instead. | No "No cable into … cv" fact (cv is optional); the existing note that its gain is 0 and nothing drives it explains the silence. | |
| 10 | Status bar while you do all this. | No "change(s) waiting for audio" except, briefly, if audio stalls; a broken edit shows "recompile failed" and the sound keeps playing. | |
| 11 | Any confusion. | Write it in `docs/confusions.md` (only real observations). | |

Also still pending: D03, D02, D01/D01-R1, Composition + Motion and Sound Palette hands-on
reviews. D00 is not complete.
