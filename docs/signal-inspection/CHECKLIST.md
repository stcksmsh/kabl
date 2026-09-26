# D03 owner checklist (Kosta, laptop)

Everything here is **pending**. The cloud evidence (README) is scripted on a VM with a null
audio sink and a fifo MIDI stand-in; it is not listening, controller or learning evidence.

Launch (48 kHz / 256, your controller):

    cargo run --release -p kabl-ui -- --patch patches/init-keyboard --perform --rate 48000 --frames 256

Or the package: `packaging/linux/package.sh`, extract `target/dist/kabl-0.1.0-linux-x86_64.tar.gz`
anywhere, run `bin/kabl-ui --rate 48000 --frames 256`.

| # | Do | Expect | Pass/fail, notes |
|---|---|---|---|
| 1 | Routing drawer → **Inspect**, click the VCA, press **out**. Play and hold a chord. | Level in dBFS with "8 voice lanes"; one bar per voice lights while held; values stop being "updated 0.0x s ago" soon after you stop the audio device. | |
| 2 | Inspect **MIDI #1 gate**, open **Why no sound?**, don't play. Then play one key. | "no rising edge … over the last 1.0 s" and "No key started a note…" (not a claim about your controller). After the key: "1 rising edge". Holding a key: "held high", no missing-trigger text. | |
| 3 | Pull both plugs out of the Output (drag them off), inspect VCA out while playing. | Graph fact "Nothing is plugged into Output #5"; the VCA level is still measured; final meter below −90 dBFS. Ctrl+Z twice restores the sound. | |
| 4 | Selecting outputs and opening/closing the inspector while playing. | No audible change, no undo entry, the sound stays "saved" (no unsaved dot). | |
| 5 | **Learn → Pluck to pad** with unsaved edits. Try Cancel, then Don't save. | The ordinary unsaved question; Cancel keeps your sound and says the recipe did not start. | |
| 6 | Do the five pluck-to-pad steps with **Play test note**; use **Show**/**Back**. | One audible change per step; Show lands on the ADSR control. | |
| 7 | Step 5: **Restore reference…** → Restore, play, **Back to my version (Undo)**, play. | You hear the pluck, then your pad with every edit; Redo hears the pluck again. | |
| 8 | **Filter movement**: turn Cutoff, un-bypass the LFO route, change Rate, negative depth, compare. | Movement starts only when the route is on; the inspector shows the LFO between −1 and 1. | |
| 9 | **Interlocking sequences**: before Start, inspect the clock gate and open Why no sound?; then Start and change the lengths. | Stopped: "Clock #1 is stopped" only, no fault claims. Running: lengths 4/8 lock to the bar. | |
| 10 | Close a recipe; open another sound during one. | Close keeps the patch and edits; another sound makes the recipe say it no longer applies. | |
| 11 | Status bar **Log** → click (copies the path). Open it. | `~/.local/state/kabl/logs/kabl.log`: INFO lines for start, audio settings, MIDI, opens/saves; no DEBUG. `KABL_LOG=debug` adds graph/recipe lines. | |
| 12 | Any confusion while doing 5–10. | Write it in `docs/confusions.md` (only real observations). | |

Also still pending from earlier batches: D01/D01-R1, D02, Composition + Motion and Sound
Palette hands-on reviews.

## D03-R2 addition (2026-09-26, pending)

| # | Do | Expect | Pass/fail, notes |
|---|---|---|---|
| 13 | Init Keyboard: hold a key, **Inspect** VCA #4 **out**, pull the cable out of **VCA #4 in** (drag the plug away), open **Why no sound?** Then Ctrl+Z. | Sound stops. *From the patch*: "No cable into VCA #4 in: that is its audio input, and an unplugged input reads silence, so this stage passes nothing." *Could be*: audio outputs that feed nothing (Filter #3 lp …), not the ADSR on the cv. Ctrl+Z brings back the cable, the reading and the sound. | |
| 14 | Same, but pull the envelope's cable out of **VCA #4 cv** instead. | No "No cable into … cv" fact: cv is optional. | |
| 15 | (Log) If the log shows "stream errors in …", read the "last delivered message". | It says how long ago it was taken ("taken N s ago, possibly before this interval"), not that it belongs to those counts. | |
