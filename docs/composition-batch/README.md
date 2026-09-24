# Composition + Motion batch: banks, launches, cues, synced LFO, macros, MIDI buttons

**Built, waiting for Kosta's hands-on review.** Supervisor scope (2026-09-24), authorized as
one batch: develop a piece through sections while playback continues. Built on `master`
(local, not pushed), commits `5d54b68..` (see `git log`). Rationale: `docs/decisions.md`,
"Composition + Motion batch"; the launch rules were written down first, in
[`design.md`](design.md).

    cargo run --release -p kabl-ui -- --patch patches/composition --perform --rate 48000 --frames 256
    # add --size 1280x800 for the smaller window; --midi "KL Essential" picks the controller

Start with the [hands-on checklist](CHECKLIST.md) (controller map included).

**Automated vs. owner evidence.** Everything below was checked by tests or by driving the
real release app on a virtual X display (Xvfb, 1440×900) with MIDI from a virtual controller
(`examples/midi_player`) and audio to a silent PipeWire sink. The take and the walkthrough
are **automated performances** (a script plays the controller and clicks), not a person
playing. Feel, the sound on speakers and a hardware controller are Kosta's check.

## What was built

### 1. Pattern banks (`seq`)

- Four banks, A–D, of eight steps. A bank holds pitches, rests, velocities, probabilities,
  length, gate mode and gate length. Transpose and direction stay module-wide.
- **Old patches are bank A, unchanged**: bank A uses the names the sequencer always had
  (`p1`, `g1`, `v1`, `length`, …), B–D the same names prefixed `b.`/`c.`/`d.`. Routes, pins
  and CC mappings keep their targets; a target in a bank names its bank (`C · P3`). No
  migration step was needed and the schema is unchanged. Every existing test and render is
  identical.
- **Edit, playing and queued are separate.** The face header has **EDIT** tabs (which bank
  the face, the advanced area and the drawer show — a view choice, never saved or undone) and
  **PLAY** buttons (launch; the playing bank is lit, a queued one blinks, **Cancel** appears
  while one is queued). Editing another bank than the playing one says "editing C … (not
  playing)" and the step light turns hollow. Editing an inactive bank never touches what
  plays (tested sample for sample).
- Target identity: changing the edit bank retargets nothing. A route into a bank that is
  not on the face plugs into that bank's EDIT tab (a dot marks it). Inspecting such a target
  (a route, a pin, a mapping) switches the edit bank to reveal it; it never launches.
- Bank edits (right-click an EDIT tab, or the drawer's **Bank ▾**): copy to another bank,
  clear, rename, set as startup bank — each one undo step. The **startup bank** (`bank`) is
  what a Load starts on; setting it does not change what plays. Pending launches and
  playheads are runtime state.
- The advanced area: a velocity row (+ Gate length), a probability row (+ Transpose) and the
  selectors Length, Gate, Direction, Startup. Four banks' controls are never shown at once.

### 2. Queued launches and cues

- Timing on an explicit reference clock: **Now**, **Next step**, **Next bar** (default; bar =
  16 sixteenths, 4/4). A sequencer's own launches use its patched clock unless another is
  chosen in the drawer ("Launch on"); a cue stores its clock.
- The boundary is found on the audio thread to the sample; each sequencer switches on the
  **first edge of its own clock input at or after it**, so a divided sequencer can start a few
  ticks later than a direct one, and no edge is invented.
- One pending launch per sequencer: a new one replaces it; Cancel drops it.
- Across transport and edits (tested, `crates/engine/tests/launch.rs`): Stop turns a pending
  launch into a selection played from the first edge after Run; a launch while stopped is a
  selection and makes no sound; a Restart pulse lands pending launches; graph swaps (edits,
  undo, redo) neither lose nor replay one; Load drops them; a deleted reference clock drops
  its launches.
- **Cues** (`cues` module, up to 8): a name, a reference clock, a timing and a bank (or
  Keep) per sequencer. Edit them in the drawer (select the Cues module): Add (from what
  plays), Capture, Delete, names, clock, timing, banks. Launch from the module face, the
  Perform panel's Cues card or MIDI buttons. The card lights the cue whose banks all play,
  blinks the one queued, and has Cancel. Cue definitions are saved, undoable edits; a launch
  is a runtime command. Deleting a module removes the cue entries naming it in the same
  undo step, so undo restores them.

### 3. Pattern variation

- Direction FWD, REV, PEND; per-step probability 0–100 % (default 100 %). Exact order,
  endpoints, reset and randomness: [`design.md`](design.md) §2. Forward and 100 % are the
  old behaviour.

### 4. Clock-synced LFO (`lfo`)

- `clock` and `reset` inputs, **Sync** FREE / 1/16 / 1/8 / 1/4 / 1/2 / 1–8 bars, **Phase**
  (advanced area). The face tag shows `4bar · synced` (or `acquiring`, `held`). FREE is the
  old LFO, sample for sample.
- No clock yet: the free rate. Clock stops: the last tempo holds, the modulation goes on.
  After a gap the stopped time never counts as tempo. Reset only through the `reset` input.
  Acquisition glides (the phase error is spread over the next pulse, at most ±50 % of the
  rate), so square and S&H keep their own hard edges but nothing else jumps.

### 5. Macros (`macro`)

- Four named 0–1 knobs with CV outputs (names in the drawer), gliding 20 ms. They reach any
  number of destinations through ordinary modulation routes with signed depths, pin to the
  Perform panel, learn CC with soft takeover, and undo like any knob. A destination shows
  the macro's route on its ring, apart from its base value.

### 6. MIDI button actions

- Right-click a sequencer, the Cues module or a clock → **MIDI button**: launch bank A–D,
  launch a cue, cancel queued, run/stop, restart. Absolute CC: an action fires on a low→high
  crossing (≥ 64) only; releases and repeated highs do nothing; no pickup. After every
  (re)connect, 0.5 s of messages only set button states, so a reconnect cannot fire anything.
  One CC has one use: learning a button takes the CC from a continuous mapping and the
  reverse. Notes and continuous mappings are unchanged.

### 7. The demo: `patches/composition`

Built by `crates/ui/tests/composition.rs`
(`cargo test -p kabl-ui --test composition write_composition_patch -- --ignored`).
112 bpm, E minor. Bass banks Intro / Drive / Lift (ghost notes with chance) / Hold; arp
banks Bells (5 steps) / Line (7) / Answer (6, with chance) / Drops. Cues Intro, Main,
Variation, Breakdown, Return. A 4-bar triangle LFO on the sequenced filters and an 8-bar
sine on the pad, both scaled by **Motion**; **Energy** (brighter, punchier sequences),
**Space** (reverb and echo), **Glow** (pad). The performance demo's MIDI lead, echo, reverb
and recorder. Controller map: [CHECKLIST.md](CHECKLIST.md).

Offline render of each section (8 bars, with a live edit mid-section,
`crates/engine/tests/composition.rs`): peaks −7.7 to −5.7 dBFS, RMS −21.3 to −19.2 dBFS;
bank switches land in the block holding the bar line.

## Evidence

- **[Walkthrough video](walkthrough.mp4)** (2:00, 1280×800 video of the 1440×900 window,
  stereo audio from the app's output, automated): banks, an inactive-bank edit, queue and
  cancel, cues from controller buttons, macros, the synced LFO, pendulum, breakdown, return,
  Stop with ringing tails. Timeline: [walkthrough-marks.txt](walkthrough-marks.txt).
  Script: `scripts/walkthrough.txt`, recorder: `record-walkthrough.sh`.
- **[Recorded take](audio/composition-take.m4a)** (9:21, AAC of the 32-bit WAV kabl's own
  recorder wrote in the real app; automated by `scripts/take.txt`, generated by
  `scripts/make_take.py`). Timeline, from the driver's log:

  | at | what |
  |---|---|
  | 0:00 | Intro (startup banks: bass Intro, arp Bells); Glow brings the pad in; a lead phrase |
  | 1:20 | an inactive bank edited while Intro plays (arp bank D, step 1, on its knob) |
  | 1:31 | Main from the cue button; Energy up, Space in; Motion up; lead phrases |
  | 3:24 | Variation queued, then **replaced** by Breakdown within the bar. The scripted Cancel came 0.7 s later, after Breakdown had landed, so Breakdown played for a bar (the walkthrough shows a cancel that lands in time) |
  | 3:34 | Variation queued and played |
  | 3:45 | arp direction to PEND (Perform card); 4:00 arp bank C step 2 probability lowered on its knob; 4:32 back to FWD |
  | 5:11 | Breakdown: held bass, sparse drops, Space to a big room, Glow up; lead |
  | 6:40 | Return (Main's bass, Variation's arp); Energy and Motion up; lead |
  | 8:33 | back to Intro, energy down, then Stop (button): the echo and reverb tails ring out |
  | 9:21 | Record stopped |

  Peak −3.6 dBFS, RMS −19.1 dBFS, no NaN, no second below −60 dBFS, stereo (L−R −20 dB),
  complete (not `-INCOMPLETE`).
- **Screenshots** ([`img/`](img/), each at 1440×900 and 1280×800, A-light and A-dark):
  `01/02` Perform panel with Main queued (blinking outline) from a button, `03/04` the bass
  sequencer editing bank C while B plays, `05/06` the Cues module and the drawer's cue
  editor, `07/08` a synced LFO (`4bar · synced`) with its Sync and Phase controls, `09/10`
  the Energy macro among the routes on the bass cutoff.

## Verification

`cargo test --workspace`: **346 pass**, 10 ignored (fixture/patch/clip writers). `cargo
clippy --workspace --all-targets`: clean. New checks:

| area | where | what |
|---|---|---|
| Old patches | every pre-existing test; `crates/ui/tests/banks.rs` | renders unchanged; the performance demo is bank A with its pins, maps, routes |
| Banks | `crates/modules/tests/seq_banks.rs` (12) | names/indices, directions and endpoints, startup bank, arm offset at the sample, inactive-bank edits change nothing, probability rests keep time, per-module seed, reset replays, a probability change doesn't shift the stream, carried state |
| Launch engine | `crates/engine/tests/launch.rs` (10, no allocation) | next bar/step/now; a /3 sequencer starts on its first edge after the bar; replace; cancel; Stop → selection → Run; launch while stopped makes no gate; Restart lands it; swaps keep and never replay; Load drops; deleted clock drops; six tempos putting the bar at different block offsets |
| Editor | `crates/ui/tests/banks.rs` (10), `cues.rs` (8) | edit tab changes the face only; inactive-bank writes; copy/clear/rename undo exactly; off-bank inspection reveals, never launches; PLAY/Cancel send runtime commands only; shared face choice; save/load of names, startup, launch settings; cue edits undoable, saved, no audio rebuild; cue launch is one command; deleting a referenced module and undo; buttons fire once per press, never after a reconnect, take CCs from continuous maps and back; macros named, pinned, learnable |
| LFO | `crates/modules/tests/lfo_sync.rs` (7) | FREE sample-identical; free rate until a clock; one-bar lock on bar lines with a glide; held tempo on Stop, gap not tempo; reset only when patched, to the phase; square stays ±1; carried state |
| Macros | `crates/modules/tests/macros.rs` | start at the knob, glide, settle |
| Demo | `crates/engine/tests/composition.rs`, `crates/ui/tests/composition.rs` | every section with a live edit, no allocation, headroom, switches on bar lines; committed patch = builder |

Real app (release, Xvfb 1440×900, virtual controller, PipeWire null sink, `--rate 48000
--frames 256`, RT priority granted; the device honoured 48 kHz, 256 frames, 2 ch):

| run | callbacks | execution worst | over half | late | arrival worst | xruns |
|---|---|---|---|---|---|---|
| the take (9:21, recording, CC turns = swaps every frame, clicks, launches, notes) | 107,657 | 3418 µs | 1 | 0 | 5886 µs | 0 |
| the same take, earlier run | 107,703 | 5196 µs | 1 | 0 | 5958 µs | 0 |
| the walkthrough (2:00, screen and audio being captured) | 23,051 | 2486 µs | 0 | 0 | 5787 µs | 0 |
| recording only, 7 min | 80,794 | 974 µs | 0 | 0 | 5713 µs | 0 |
| Breakdown with lead notes after silences, 2 min | 25,097 | 1108 µs | 0 | 0 | 5713 µs | 0 |

Every run: 1 live graph allocation at the end (the playing graph), 0 MIDI notes held.
Offline (`cargo run --release -p kabl-ui --example bench_composition 48000 256`, recorder on,
6000+6000 callbacks): median 140 µs, max 689 µs (13 %); swap callbacks max 656 µs; 3-graph
bursts 640 µs; callbacks with a launch 381 µs. At 64 frames: max 266 µs of 1333 (20 %).

**The one long callback per take.** Each 9-minute take had exactly one callback over half
the budget (3.4 and 5.2 ms of 5.3; not late, no xrun). The stats line now records when the
worst callback happened (stream and wall-clock time) and `drive.py` can log each action's
time (`KABL_DRIVE_LOG`): in the last take it fell mid-note in the Return section, 1 s after
the nearest action. A 7-minute recording-only run and a 2-minute replay of the Breakdown
section stayed under 1.1 ms, and offline runs never exceed 1.3 ms. So it is not tied to a
launch, an edit, a note or the recorder; it looks like the host occasionally delaying the
real-time thread (the execution time includes any preemption). It stays a watch item for
the hands-on run and for the Pi.

## Limits

- One ~3–5 ms callback per 9-minute take on this laptop (above); 0 late, 0 xruns.
- Launch timing: the engine arms a sequencer when its reference clock starts the boundary
  pulse. Clocks are scheduled first, so this is sample-exact; only a clock that is itself
  modulated from downstream of a sequencer could run after it, and then that sequencer
  switches one of its edges late. No demo does this.
- Direction is per sequencer, not per bank (like transpose). Probability is per step and
  bank.
- A synced LFO with a large phase error takes up to one cycle to glide in (an 8-bar LFO: up
  to 8 bars); a Restart patched to its reset input aligns it at once.
- The edit bank is not saved (a view choice); after a Load each sequencer edits what it plays.
- A button press within 0.5 s of a MIDI (re)connect only sets the button's state.
- Up to 8 cues, 16 sequencers per launch, 32 pending launches.
- Stepped sequencer params (gates, length, direction, startup) are not MIDI-learnable, as
  before; launches and transport are reached through MIDI buttons.
- Carried from earlier batches: 96 kHz / 64 frames doesn't run on this PipeWire setup;
  Pi 4 unmeasured.
