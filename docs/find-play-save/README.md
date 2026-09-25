# D01 — Find, play and save a sound

**Built, awaiting Kosta's review.** Owner-authorized batch (2026-09-25), brief:
[`product-research/briefs/D01.md`](../product-research/briefs/D01.md). Rationale:
`docs/decisions.md`, "D01 Find, play and save a sound". Hands-on checklist:
[`CHECKLIST.md`](CHECKLIST.md). Structured report: [`REPORT.md`](REPORT.md).

Kosta explicitly deferred the Composition + Motion and Sound Palette hands-on reviews while
his laptop is away; they are **still pending**, not approved by this batch. Laptop timing,
physical-controller feel, listening and beginner-ease checks are deferred to him.

Open kabl, find a sound by name, category or tag, audition it without a MIDI controller,
change its controls, save your own version and find it again.

    packaging/linux/package.sh                     # → target/dist/kabl-0.1.0-linux-x86_64.tar.gz
    tar -xzf target/dist/kabl-0.1.0-linux-x86_64.tar.gz -C ~/opt
    ~/opt/kabl-0.1.0-linux-x86_64/bin/kabl-ui      # add --size 1280x800 for the smaller window

From the source tree `cargo run --release -p kabl-ui` does the same (factory sounds are then
read from `patches/`). `--patch DIR` still opens a folder directly.

## What it does

**Sounds panel** (left; the toolbar's **Sounds** toggles it, open at launch without
`--patch`):

- **Factory** (shipped, read-only) and **Your Sounds** (saved by you), in one list with a
  heading each. **Search** matches every word against name, category, tags and description.
  Filters: **All · ★ Favorites · Recent · Factory · Your Sounds**, plus a **Category** menu
  (Basic, Bass, Lead, Pad, Strings, Wind, Percussion, Piece, Study).
- Each row: ☆/★ favorite, name, category, and how it plays: **Keys** (a keyboard voice),
  **Sequence** (runs itself from a clock) or **Seq + Keys** (a piece you can also play).
- **Browsing makes no sound.** A click selects and shows the description and tags; **Open**
  (or a double click) loads the sound into the rack. Loading is a fresh graph: no notes,
  tails or transport state carry over.
- **15 factory sounds**: Init Keyboard (new: saw → low-pass → VCA/ADSR, five controls on the
  Perform panel), the six palette voices (Ensemble Strings, Evolving Pad, Singing Lead,
  Breath as keyboard voices; Sequence Bass and Noise Percussion as sequences), the pieces
  (Sound Palette Piece, Composition, Performance, Interlocking Sequences, Echo Sequence, First
  Sequence) and two rack studies (Reference Voice, Crowded Rack). Metadata:
  `patches/**/sound.toml`, written by `crates/ui/tests/library.rs` (`write_factory_library`).

**In the rack** (bottom of the panel) always refers to the sound loaded in the rack:

- Keyboard sounds: **note** (C2–C6 in the menu; the −8/+8 octave buttons reach C1–C7), **Note / Major / Minor**,
  **Velocity** 1–127 (default 90), **Length** 0.25–4 s (default 1.5 s), **▶ Play**, **■ Stop**.
  The help line under it says exactly what it plays ("Plays C4 E4 G4 at velocity 90 for
  1.50 s into this sound's keyboard, like keys held that long. One preview can't show a
  sound's whole range: try other notes."). The notes go through the patch's own `midi.in`
  keyboards, so POLY/MONO/LEGATO, glide and the envelope behave as with a controller.
- Sequenced sounds **open stopped**; **▶ Start / ■ Stop** send the existing transport
  Run/Stop to every clock (the clock's own buttons do the same).
- Controls: the existing rack and **Perform** panel, unchanged. There is no second parameter
  state; an edit is an ordinary op-log edit with ordinary undo.

**Saving** (toolbar: document name, **Save**, **Save As**; Ctrl+S = Save):

- The name shows **●** when there are unsaved changes: the patch differs from the state last
  opened or saved. Undo back to that state and the ● goes away. Previews, Start/Stop, edit
  banks and the audio-rebuild flag never count.
- **Save** writes a sound of yours in place. On a factory or new sound it opens **Save As**
  (name, category, tags): factory sounds are never overwritten.
- A name already in Your Sounds (any letter case): Save As says so and offers **Replace it**;
  nothing is overwritten without that click. **Rename** (selected sound of yours) refuses a
  taken name. Identity is the directory, not the name: renames keep favorites and recents.
- **Open, New and quitting** (window close, Ctrl+Q) with unsaved changes ask **Save / Don't
  save / Cancel**; Cancel (or Escape) keeps the work and its undo history exactly. Save on a
  factory sound goes through Save As and then continues. A failed save keeps the question
  open with the reason and changes nothing.
- **New** (panel header) starts an untitled copy of Init Keyboard.
- The old path workflow is in the collapsed **Patch folder (advanced)** section: Load folder,
  Save to folder (refuses a factory folder), and the two library locations.

## Where things live

| What | Where |
|---|---|
| Factory sounds | `KABL_FACTORY_DIR`, else `<bin>/../share/kabl/patches` (package), else `$XDG_DATA_DIRS/kabl/patches` (`/usr/local/share`, `/usr/share`), else the source checkout's `patches/` |
| Your sounds | `KABL_USER_DIR/sounds`, else `$XDG_DATA_HOME/kabl/sounds`, else `~/.local/share/kabl/sounds` — one directory per sound |
| Favorites, recents | `library.json` beside `sounds/` (preferences: not in any patch, never undone) |
| A sound | the existing patch directory (`log.jsonl` = source of truth, `checkpoint.json`, `meta.toml`) plus optional `sound.toml` (name, category, tags, description, keys, sequence) |

kabl prints both locations on start (`kabl-ui: factory sounds: …; your sounds: …`), and the
advanced section shows them. Old patch directories without `sound.toml` load unchanged and
are listed under their directory name.

## Guarantees and failure behaviour

- **Loading** parses and compiles the patch before replacing anything
  (`library::read_patch`). A malformed log, an unknown schema, an unknown module kind or a
  missing directory leaves the working sound and its unsaved edits in the rack and says why
  ("couldn't open: the patch doesn't build: unknown module kind … Your sound is unchanged.").
- **Saving a sound of yours** writes a complete copy to `.name.new/`, reads it back and
  requires the same patch state, then swaps it in with two renames (`name` → `.name.old`,
  `.name.new` → `name`) and removes `.old`. A failure before the swap leaves the old
  directory untouched; a failure in the swap puts it back. An interrupted swap (process
  killed) is repaired on the next scan: a lone `.old` is restored, a leftover `.new` removed.
  Metadata-only changes (rename) go through a temp file and a rename.
- **Not guaranteed:** durability across power loss (no fsync); concurrent writers (two kabl
  instances saving the same sound).
- A read-only or unusable user directory: Save/Save As report "can't create …" and keep the
  work; favorites/recents stay for the session and the failure is shown. Missing factory
  sounds: the panel says how to point kabl at them (`KABL_FACTORY_DIR`); the app runs.
- A favorite or recent whose sound is gone is listed as "not found" with **Forget**.

## Preview and controller notes

- A preview is `Command::Preview { notes (≤ 4), velocity, blocks }` on the existing
  UI → audio command queue (`rtrb`, fixed-size, no allocation). The audio thread presses the
  notes through the same `midi.in` keyboards as MIDI and releases them itself after the block
  count, capped at **10 s**. It also ends on **Stop**, a new preview (never piles up), **All
  notes off**, a MIDI disconnect or port switch, **Load** (fresh graph), and when the window
  **loses focus** (the UI sends Stop). Shutdown ends the stream.
- **Ownership:** the engine keeps which keys the controller holds and which the preview
  holds. A key-up from one source is held back while the other source still holds that key,
  so a preview never releases a controller-held note and a controller key-up never cuts a
  sounding preview. Key-downs always go through (a shared key retriggers its voice, as a
  repeated key does). MONO/LEGATO keep one held list, so a mono voice returns to the
  controller's key when the preview ends.
- **Sustain pedal:** a released preview key is held by the pedal like any released key;
  pedal up releases it.
- Audio device change: kabl has no device switching (the stream is chosen at start), so
  there is no such path to handle.
- Tests: `crates/engine/tests/preview.rs` (duration, cap, stop/replace, overlap both ways,
  pedal, All Notes Off, MONO, Load, no allocation), `crates/ui/tests/browser.rs`.

## Verification

Cloud container: Ubuntu 24.04.4, 4 vCPU VM, rustc 1.94.1. No sound card, no ALSA sequencer,
no rtkit. Product code is unchanged after **f3bebd9** (the real-app evidence build); the
tests below ran at **7030b74**, which adds the focus-loss test and these documents.

- `cargo test --workspace` at 7030b74: **444 passed, 0 failed, 14 ignored** (the 13
  fixture/render writers plus `write_factory_library`), exit 0:
  [`evidence/test-workspace.txt`](evidence/test-workspace.txt). New in D01:
  `crates/engine/tests/preview.rs` (10), `crates/ui/tests/library.rs` (14 + 1 ignored
  writer), `crates/ui/tests/browser.rs` (13, real egui input through `show()` at 1440×900 and
  1280×800). `interaction.rs`'s folder-load test now answers the unsaved question.
- `cargo clippy --workspace --all-targets` at 7030b74: no warnings, exit 0
  ([`evidence/clippy.txt`](evidence/clippy.txt)).

## Real-app evidence

All of it is **scripted**: `docs/rack-migration/drive.py` clicks and types with xdotool on
Xvfb (1600×1000, 24-bit), aiming at the targets the app reports. The app is the **installed
package** built from f3bebd9 (`KABL_BIN`, factory sounds from the package). Audio: the app's
ALSA output goes through PipeWire 1.0.5 to a silent null sink (`kabl_rec`) and is recorded
from its monitor. **The "controller" is a stand-in**: `examples/midi_player` writing the
`KABL_MIDI_PIPE` fifo through the same MIDI handler, not hardware. This verifies the software
path; it does not establish controller feel, laptop reliability, sound quality or beginner
ease.

- **Walkthrough** (2 min 18 s, audible): [`walkthrough.mp4`](walkthrough.mp4), 1440×900,
  48 kHz / 256 frames. Script: [`scripts/walkthrough.txt`](scripts/walkthrough.txt); recorder:
  [`record-walkthrough.sh`](record-walkthrough.sh). Parts: launch; selecting/searching with
  no sound; open Evolving Pad; note, major chord, octave down, a preview stopped early; the
  Warmth macro changed and heard (● unsaved); Open another → question → Cancel; Save on the
  factory pad → Save As "My Warm Pad"; favorite and play Singing Lead; Sequence Bass opens
  silent, Start, Stop; Your Sounds → reopen and play My Warm Pad; Rename; New → controller
  holds C4 while a C-major preview plays and ends; Recent, Favorites, A-dark. Frames:
  [`img/walkthrough/`](img/walkthrough/) (`w01`…`w11`), times in
  [`evidence/walkthrough-drive.log`](evidence/walkthrough-drive.log) (video t0:
  [`evidence/walkthrough-t0.txt`](evidence/walkthrough-t0.txt)).
- **Audio per step** ([`evidence/walkthrough-audio-timeline.txt`](evidence/walkthrough-audio-timeline.txt),
  `scripts/audio_timeline.py`): −∞ dBFS through all browsing (0–17 s) and after each Open;
  pad preview peaks −24…−14 dBFS; Sequence Bass silent from Open (80.7 s) to Start (84.8 s),
  then −6 dBFS until Stop. Controller/preview overlap (0.25 s RMS): controller C4 −26.5 dB;
  C major preview on top −20…−22 dB; after the preview's 1.5 s the level returns to −26.5 dB
  (the controller's C4 still sounding); the controller's key-up then releases it to silence.
- **Screenshots**, real app, both sizes and themes: [`img/1440x900-light.png`](img/1440x900-light.png),
  [`img/1440x900-dark.png`](img/1440x900-dark.png), [`img/1280x800-light.png`](img/1280x800-light.png),
  [`img/1280x800-dark.png`](img/1280x800-dark.png); dialogs: `img/*-dark-save-as.png`,
  `img/*-light-unsaved-question.png`. Script: [`scripts/shots.txt`](scripts/shots.txt).
- **Portability**: [`scripts/portability.sh`](scripts/portability.sh) extracts the tarball to
  `/tmp`, runs it from there with a fresh `HOME`, `XDG_DATA_HOME`/`KABL_*` unset and the
  checkout's `patches/` moved away. Result ([`evidence/portability-run.txt`](evidence/portability-run.txt)):
  factory sounds found in the package's `share/kabl/patches`; Init Keyboard opened, played and
  saved as "Portable Test" in `$HOME/.local/share/kabl/sounds/portable-test/`. Screens:
  [`img/p01-installed-launch.png`](img/p01-installed-launch.png),
  [`img/p02-saved-outside-checkout.png`](img/p02-saved-outside-checkout.png).

Reproduce (container: `pactl load-module module-null-sink sink_name=kabl_rec`,
`Xvfb :97 -screen 0 1600x1000x24 &`):

    packaging/linux/package.sh
    mkdir -p /tmp/kabl-pkg && tar -xzf target/dist/kabl-0.1.0-linux-x86_64.tar.gz -C /tmp/kabl-pkg
    cargo build --release -p kabl-ui --example midi_player
    DISPLAY=:97 KABL_BIN=/tmp/kabl-pkg/kabl-0.1.0-linux-x86_64/bin/kabl-ui docs/find-play-save/record-walkthrough.sh
    python3 docs/find-play-save/scripts/audio_timeline.py target/find-play-save-walkthrough
    DISPLAY=:97 docs/find-play-save/scripts/portability.sh target/dist/kabl-0.1.0-linux-x86_64.tar.gz
    DISPLAY=:97 KABL_BIN=… python3 docs/rack-migration/drive.py docs/find-play-save/scripts/shots.txt 1280x800 OUT -

## Measurements (cloud VM, not the laptop)

Walkthrough take, 48 kHz / **256 frames** asked and received (every callback 256 frames),
26 272 callbacks (≈ 140 s). The app's own rtkit request was refused (no system D-Bus); the
recorder gave the `cpal_alsa_out` thread SCHED_FIFO 80 with `chrt` instead
([`evidence/walkthrough-callback-stats.txt`](evidence/walkthrough-callback-stats.txt)):

| | result |
|---|---|
| Callback execution | worst 8 525 µs of a 5 333 µs budget (at 42.3 s); 18 over half; **2 late** |
| Callback arrival | worst 49 060 µs between callbacks; **49 late** (> 1.5 × period) |
| Backend xruns | **5** (PipeWire-ALSA underruns, [`evidence/walkthrough-app.log`](evidence/walkthrough-app.log)) |
| Stream stalls | none (no "audio stalled" status) |

The late arrivals and xruns outnumber late executions, the pattern of VM scheduling stalls
seen in earlier cloud runs; the worst execution fell around the audition part of the take. This says
nothing about the laptop at 256 frames. The Composition 3–5 ms callback spike and the Sound
Palette stream-stall question are **not** investigated or resolved by this batch.

## Limits

- Scripted cloud evidence only; no hardware controller, speakers, laptop timing, or
  beginner observation. The 60-second first-sound task is Kosta's formative observation, not
  something a script can certify.
- No fsync (power-loss durability not claimed); no protection against two kabl instances
  writing the same sound.
- No delete, duplicate, import or export for user sounds; tags are edited only in Save As.
- At 1280×800 with the browser, the Routing drawer and the Perform panel all open, the rack
  area is about 620×300 px; close the drawer or the panel for more rack.
- The sustain pedal holds preview notes like any released key.
- Factory metadata (names, categories, descriptions) is the agent's wording; Kosta may want
  different names.

## Sources and licenses

No new dependencies beyond crates already in `Cargo.lock` (`serde`, `serde_json`, `toml`,
added to `kabl-ui`'s manifest). All code and patches here are kabl's own (MIT OR
Apache-2.0). Interaction ideas (search, categories, favorites, save protection) follow the
research in `docs/product-research/RESEARCH.md`; no code, presets or text were copied.
