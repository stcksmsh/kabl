# Performance batch: reverb, sequence expression, Perform panel, MIDI learn, recorder

Supervisor scope (2026-09-24), owner-authorized as one batch: Kosta can build, play and
record an evolving piece inside kabl. Built on `master` (local, not pushed), commits
`b742a70..`. Rationale: `docs/decisions.md`, "Performance batch".

    cargo run --release -p kabl-ui -- --patch patches/performance --perform --rate 48000 --frames 256
    # add --size 1280x800 for the smaller window
    # choose the MIDI input in the Perform panel, or: --midi "KL Essential"

Other flags: `--record-dir DIR` (where takes go; default `recordings/`, git-ignored),
`--rate 96000` and `--frames 256` (ask the device for a rate and a fixed buffer), `--no-rt`
(don't ask for real-time priority; for comparison only). Ctrl+Q quits.

**Start with [`TOMORROW.md`](TOMORROW.md)** (launch, controller, control map, first-play
checklist). The follow-up work from the same day (labels, gain, meter, recovery, the
single-instance audit, callback investigation, a 30-minute soak) is described in "Follow-up"
below; where it changed something described earlier, the earlier text is updated.

**Automated vs. owner evidence.** Everything below was checked by tests or by driving the
real release app on a virtual X display, with MIDI from a virtual controller
(`examples/midi_player`) and audio to a silent PipeWire sink. Nobody has played it with a
hardware controller or listened on speakers yet: feel, pickup on real knobs and the sound are
Kosta's hands-on check (checklist at the end).

## What was built

### Reverb (`reverb`, Effect, global)

Stereo in (`in_l`, `in_r`), stereo out (`left`, `right`).

| Control | Range | Where |
|---|---|---|
| Decay | 0.3 – 30 s (RT60), log | face |
| Damping | 500 Hz – 16 kHz low-pass inside the tank | face |
| Mix | 0 – 100 %, equal power; 100 % = fully wet (send/return) | face |
| Pre-delay | 0 – 250 ms | advanced |
| Width | 0 – 100 % (0 = mono wet) | advanced |

- Algorithm: **Jon Dattorro's plate**, "Effect Design, Part 1: Reverberator and Other
  Filters", J. Audio Eng. Soc. 45(9), 1997 (figure 1, table 2). Written from the paper; no
  code was copied, so no third-party license applies. Delay lengths are the paper's, scaled
  from its 29761 Hz to the running rate.
- One change: the paper sums its input to mono. Here each side has its own input diffusers
  and feeds the tank half that side's output taps first, so the delay's ping-pong keeps its
  sides in the early reverb (a left-only input is 15.8 dB louder on the left in the first
  80 ms); the tail still spreads to both sides.
- The decay knob is calibrated: measured RT60 at 1 s → 1.0–1.1 s, 4 s → 3.6–3.7 s, 2 s at
  44.1 kHz → 1.9–2.0 s, at 96 kHz → 1.8 s. All controls glide (20 ms; pre-delay at most
  ±0.5 samples per sample).
- Tails continue through Stop/Restart, parameter edits, unrelated edits and overlapping swaps
  (`carry_from` copies ~0.9 s of lines at fade start, audio thread, no allocation). Each
  instance keeps its own history. A Load starts empty, as before.

### Sequence expression (`seq`)

- **Velocity:** a `V1`–`V8` knob row (advanced area, one knob under each step) and a
  `velocity` output (0..1). A rest is still the step's `G` switch; velocity never mutes.
- **Gate:** `Gate` CLOCK (default, the old behaviour: the gate is the clock pulse) or LENGTH:
  the gate opens on the step's clock edge and stays open for `Gate length` % (1–100) of the
  step. Behaviour:
  - Step period = the interval between clock edges, taken once it agrees within 10 % with
    the one before (the delay's sync rule). So a divided clock works, a small tempo change is
    followed at once and a jump after two steps.
  - First step after a load: no period yet, so it follows the clock pulse.
  - Stop: the playing gate finishes its length, then nothing until Run. Run: the interval
    across the stop is ignored, so no step is stretched.
  - Restart/Reset: the cut interval is ignored too.
  - A gate still open at the next step (100 %, or the tempo sped up) closes for one sample,
    so every step retriggers. No stuck or merged gates, no duplicate steps.
- Old patches play exactly as before (CLOCK, velocity 100 %).
- In the demo, velocity reaches amplitude through the patch: envelope × velocity
  (`ringmod`) into the VCA's CV, plus a small route to the filter cutoff.

### Perform panel

`Perform` (toolbar, or `--perform`) opens a bottom panel of **pinned controls**.

- Pin from a module's right-click menu: **Pin to Perform ▸** (any control; on a clock,
  *Transport*). Each card has a **label** (double-click it, or **⋯ → Rename**; empty = the
  control's own name), saved with the patch as text metadata (`Op::SetLabel`, schema v3), and
  under it the real identity in grey: `Mixer #27 · Level 2 · from VA Oscillator #16` (mixer
  levels are traced upstream to the nearest sequencer, MIDI input or oscillator).
- **⋯** moves a card left/right and unpins it. Pins and labels are saved with the patch, undo
  like any edit and never rebuild audio. Deleting a module removes its pins and mappings;
  undo brings them back.
- A card's slider is the real control: same stored value as the rack knob, same undo (one
  step per drag), modulation routes still add on top.
- The transport cards stay in a column at the left and send Run/Stop and Restart to that
  clock, exactly like the clock's face (runtime commands, never undo entries).
- Compact cards (138 px) wrap into rows: the demo's 12 controls fit together at 1280×800
  without scrolling. **Taller** gives the panel more rows. Fixed sizes, egui widgets, so it
  reads the same at any rack zoom; keyboard: Tab to a slider, arrows change it; both themes.
- Long MIDI device names, file names and paths are truncated (hover shows them whole), so
  the buttons never move off screen.

### MIDI CC learn

- **Learn** (the badge on a card, or right-click a module → **MIDI learn ▸**), then move a
  knob: the control gets that CC and channel (badge `CC 74 · 1`). Learning a CC that's already
  in use moves it. **✕** removes it. Escape or **Cancel learn** stops. Mappings save with the
  patch.
- The MIDI input is chosen in the panel (the list refreshes every 2 s) or with `--midi`.
  Switching releases every voice. Mappings are channel + CC, not tied to a device name.
- **Soft takeover** (default, always): a mapping moves its control only once the hardware
  reaches the control's position (within 1.5/127, or by passing it). Until then the card shows
  `↑ 42%` / `↓ 42%` (which way to turn); after it, `● live`. Picking up right at the value
  writes nothing. Any other change to that value (mouse, undo, redo, load), a reconnect or a
  controller switch cancels the pickup again, so the next turn never jumps.
- A gesture is one undo step: CC messages on any mappings less than 1 s apart form one group
  (`PatchLog::append_to_group`), so two knobs turned together undo together. CC drives the
  base value; routes still modulate on top. Absolute 7-bit CC only, continuous controls only.
- An unplugged input releases its notes and reconnects when it comes back (checked every
  2 s; ALSA's renumbering tolerated). **All notes off** releases every MIDI voice.
- Notes are untouched: they still go straight to the audio thread.

### Recorder

The recorder row of the Perform panel: **● Record** / **■ Stop**, elapsed time, the take's
file name (hover: full path), the **to** folder field, **Open folder**, and after a take
**Last take** (length) with **Copy path**.

- Records exactly what goes to the device: final stereo output, 32-bit float WAV at the
  device rate, named `kabl-YYYYMMDD-HHMMSSZ.wav` (UTC). A name that exists gets `-2`, `-3` …:
  files are created with `create_new`, so nothing is ever overwritten.
- The audio callback copies frames into a 4 s lock-free ring; a writer thread writes the file
  and updates the WAV header every 0.5 s. No allocation, lock or I/O on the audio thread.
- Stopping the sequencer leaves the recorder running (releases and tails are captured).
  Recording is a runtime action, never an undo entry. Load during a take keeps recording.
- Stop finalizes the file; closing the window or Ctrl+Q does too. A killed process leaves a
  file that is readable up to the last checkpoint (≤ 0.5 s lost).
- Frames that don't fit the ring are counted (`N frames lost: incomplete`, red); a write error
  ends the take by itself and shows. Either way the file keeps what was written and is renamed
  `…-INCOMPLETE.wav`, never shown as complete.

### Engine change: one instance for chains no MIDI reaches

Voice-rate modules that no `midi.in` feeds (the demo's sequencer voices and pad) used to run
in all 8 voices with identical results. They now run once. Renders are unchanged (every
existing test passes unchanged); the demo's median callback fell from 84 to 35 µs at 64
frames. The status bar shows live callback timing. Audited against a per-voice reference
in the follow-up (below).

## Demo: `patches/performance`

112 bpm, E minor. Built by `crates/ui/tests/performance.rs`
(`cargo test -p kabl-ui --test performance write_performance_patch -- --ignored`).

| Layer | Patch | Level (card) | CC (ch 1) |
|---|---|---|---|
| Bass | `seq #3`, 8 steps on 16ths, LENGTH 35 %, accented velocities → saw → filter → VCA | Mixer #26 Level 1 | 20 |
| Arp | `seq #4`, 7 steps on 16ths (against the bass's 8), a rest, LENGTH 60 % → square | Mixer #27 Level 1 | 21 |
| Pad | two detuned saws (E3, B3), slow LFO on the filter | Mixer #27 Level 2 | 22 |
| Lead | MIDI keyboard → saw → filter → VCA → `Gain #25` (+12 dB, see below) → echo | Mixer #28 Level 1 | 23 |

Other pins: clock transport, bass cutoff (CC 24), arp cutoff (CC 25), pad cutoff (CC 26),
bass transpose, echo feedback (CC 27), reverb mix (CC 28), reverb decay (CC 29).

Routing: the bass is dry and centred. Arp, pad and the delay (1/8D, PING, 45 %) share a
stereo bus into the reverb (6 s, 4.5 kHz damping, 30 ms pre-delay, mix 35 %), which returns to
both mains. The arp also has a small echo send. Headroom (offline, 10 s with a lead phrase):
peak −5.8 dBFS, RMS −19.3 dBFS; each layer alone −23.7 to −25.8 dBFS RMS.

`Gain #25` (+12 dB) makes up for the voice average: one MIDI note is 1/8 as loud as a
sequencer voice. Full-velocity chords on the lead: 1 note −5.7, 3 notes −3.3, 4 notes −2.6
dBFS peak. (Until the follow-up this was a mixer with the VCA on all four inputs, ×4; the level
is the same within 0.04 dB, so the recorded take stands.) Pins carry labels: Bass, Arp, Pad,
Lead, Bass cutoff, …

**Reassigning to your controller:** click **Learn** on a card, turn the knob. Save the patch
to keep it.

## Evidence

- `audio/performance-take.m4a` (6:07, AAC 192k of the recorder's WAV): an **automated
  performance** recorded by kabl's own recorder in the real release app. Everything was sent
  through the app as a player would: CC messages and lead notes from the virtual controller,
  real clicks on Record, Stop/Run and Restart (`scripts/performance.txt`, generated by
  `scripts/make_performance.py`). Arrangement: 0:00 atmosphere (pad), ~0:55 bass, ~1:50 arp,
  ~2:50 lead, ~3:40 breakdown (layers fade, clock stops, tails ring), ~4:10 return (Restart,
  Run), ~5:15 outro. The take's WAV: 17,622,528 frames, 2 ch, 48 kHz float, no lost frames,
  no NaN, no silent gaps, peak −4.1 dBFS, RMS −20.2 dBFS.
- `audio/0*.m4a`: 12 s offline renders of the same passage for A/B listening, each scaled to
  the full mix's RMS (gains in dB): `01-full`, `02-no-reverb` (+1.4), `03-no-echo` (−0.3),
  `04-dry` (+1.3), `05-long-dark-reverb` (14 s, 2 kHz, −1.0), `06-clock-gates` (both
  sequencers CLOCK, −0.1), `07-flat-velocity` (all 100 %, −2.3).
- `walkthrough.mp4` (1:53, 1440×900, screen and stereo audio of the real release app;
  MIDI from the virtual controller; refreshed after the follow-up). Script
  `scripts/walkthrough.txt`, recorder `record-walkthrough.sh`, scene times (±0.5 s) in
  `walkthrough-marks.txt`:

  | ~time | scene |
  |---|---|
  | 0:05 | the patch playing, Perform panel open (labelled cards, Out meter bottom left) |
  | 0:13 | the bass sequencer's advanced area: velocity row, gate mode, gate length |
  | 0:19 | step 2's velocity to 100 % (an accent), then undone |
  | 0:26 | gate CLOCK (the old half-step gates), then back to LENGTH 35 % |
  | 0:41 | pin the lead filter's cutoff from the module menu; 0:46 renamed "Lead cutoff", moved left |
  | 0:51 | Learn, CC 74 sent far below the value: pickup pending (`↑`), a held lead note |
  | 0:59 | the controller turns up: nothing moves until it reaches the value, then `● live` |
  | 1:08 | two knobs turned together (Bass cutoff, Echo feedback); 1:12 one Ctrl+Z takes both back |
  | 1:16 | reverb mix to 100 % (fully wet), then undone |
  | 1:35 | Record; 1:44 the sequencer stops, the take keeps the tails; 1:47 Stop, Last take shown |

- `img/` (both window sizes, both themes; script `scripts/shots.txt`): `01`/`02` the demo
  with the Perform panel, `03`/`04` the reverb close-up with its advanced controls,
  `05`/`06` the sequencer's velocity row, `07` pickup pending after Learn, `08` hardware in
  control, `09` recording, `10` the take saved. File names end in `-1280` / `-1440`.

## Verification

`cargo test --workspace`: 296 pass, 9 ignored (patch, fixture and clip writers). `cargo
clippy --workspace --all-targets`: clean.

- `crates/modules/tests/reverb.rs` (9): RT60 follows the knob at 44.1/48/96 kHz; the
  0.3 s setting is −60 dB within 0.8 s; damping darkens the tail (high-band ratio 0.97 →
  0.08); a left source stays left early, width 0 is exactly mono; mix 0 is exactly dry, mix 100
  has no dry and nothing before the pre-delay; block sizes 1/17/64 give identical output; 20 s
  of full-scale noise at 30 s decay with swept damping/pre-delay stays finite (peak 5.8);
  control changes mid-tail step no more than the level change itself; `carry_from` copies the
  history exactly and ignores other kinds.
- `crates/modules/tests/transport.rs` (6 new): CLOCK is the default and unchanged; LENGTH
  gates last their share on a direct and a /2 clock; 100 % retriggers with one low sample;
  120 → 240 bpm and 120 → 30 bpm; Stop lets the gate finish, nothing sounds while stopped,
  Run and Restart never stretch a gate; rests stay silent and velocity follows the step.
- `crates/engine/tests/performance.rs` (6, audio-thread calls under `assert_no_alloc`):
  headroom and layer balance; step velocity audible (7.0 dB step-peak spread, 0.6 dB with flat
  velocity); reverb and echo tails through Stop, identical, unrelated and overlapping swaps equal
  the no-swap render (< 1e-4); a reverb decay/damping/mix edit mid-tail has no dip; two reverbs
  keep separate histories through six swaps; a fresh Load starts empty.
- `crates/engine/tests/compile.rs`: sequencer-driven chains compile to one instance,
  MIDI-driven ones per voice. Every earlier render test (legacy sound, echo, interlocking,
  live edit) passes unchanged.
- `crates/ui/tests/perform.rs` (9, real egui input): pins are presentation edits (no audio
  rebuild), reorder, undo, save/reload; a card slider edits the real value with one undo
  step; transport card sends runtime commands only; deleting a module removes its pins,
  mappings and takeover state, undo restores them; learn (Escape cancels), channel matching,
  no jump below the value, pickup by crossing, one undo step per turn, undo drops the pickup,
  a 1 s pause starts a new step, reassignment moves a CC, Clear, mappings reload; a mouse edit
  drops the pickup; routes survive CC moves; the panel fits and doesn't change with rack zoom
  at 1280×800 and 1440×900; a new pin scrolls into view.
- `crates/ui/tests/record.rs` (6): exact frames and sample values, 2 ch, device rate, 32-bit
  float; nothing recorded before Start or after Stop; a second take starts clean; a full ring
  is counted and the take renamed `-INCOMPLETE`; an uncreatable folder and `/dev/full` fail
  visibly; Stop without audio callbacks still finalizes; dropping the recorder finalizes; the
  file on disk is valid without Stop after a checkpoint.
- `crates/ui/tests/performance.rs`: the committed demo equals its builder, all values in range,
  12 pins, it compiles.
- Real app (release, Xvfb, PipeWire null sink): the walkthrough, the screenshots, the 6-minute
  take, and a virtual MIDI controller end to end (learn, pickup, lead notes).

## Callback timing

Host: i7-13700H laptop (20 threads), Linux 7.0, PipeWire, rustc 1.93, release build, 8
voices. The callback thread runs at normal priority (cpal, no real-time scheduling).

**Offline, the whole callback** (`cargo run --release -p kabl-ui --example bench_performance
RATE FRAMES`, pinned with `taskset -c 2`): the demo with a held lead note, the recorder writing
every frame, a parameter-edit swap every 4th callback and three at once every 40th, graphs
compiled outside the timing. µs:

| rate, frames (budget) | no swap median / p99 / max | swap median / max | 3-graph burst max |
|---|---|---|---|
| 48 kHz, 64 (1333) | 35 / 153 / 291 | 68 / 131 | 213 |
| 48 kHz, 256 (5333) | 144 / 366 / 620 | 362 / 671 | 825 |
| 96 kHz, 64 (667) | 36 / 223 / 410 | 69 / 340 | 341 |
| 96 kHz, 256 (2667) | 145 / 490 / 895 | 279 / 736 | 635 |

Worst case: 61 % of the budget at 96 kHz / 64 frames, 34 % at 96/256, ≤ 22 % at 48 kHz.
Before the one-instance change, the 48 kHz / 64-frame median was 84 µs and the 96/64 worst 95 %.

**Real device** (the app's own counter, `--rate R --frames N`, the demo playing, first second
reported apart; 20–25 s runs, no edits):

| asked | callbacks | worst | over half the budget | late |
|---|---|---|---|---|
| 48 kHz, 64 | 18050 | 1232 of 1333 µs | 3 | 0 |
| 48 kHz, 256 | 4570 | 732 of 5333 µs | 0 | 0 |
| 96 kHz, 256 | 9104 | 3444 of 2667 µs | 4 | **2** |
| 96 kHz, 64 | — | the stream didn't run (this PipeWire setup) | | |

The recorded 6-minute take (48 kHz, 256 frames asked, live CC edits = graph swaps every
frame of a turn, recording on): 71270 callbacks, worst 4054 of 5333 µs, 4 over half, **0
late**, no lost frames.

Reading at the time: the compute is well inside every budget (medians 3–13 %), but single
callbacks occasionally took several milliseconds, suspected to be scheduling. The follow-up
tested that hypothesis ("Callback misses: measured", below): with real-time priority granted
there were no late callbacks and no xruns in any configuration. **Pi 4 unmeasured.**

## Follow-up (2026-09-24, after the first review package)

Owner-authorized follow-up before the hands-on review. Commits `3c41a55..` on `master`.

### Usability, gain, meter

- Pin labels, compact wrapping cards, Taller, card menus, truncation: see "Perform panel"
  above. `tests/perform.rs` (12): labels (double-click, type, Enter; Escape cancels; undo;
  save/reload; never rebuild audio; the source line stays), the demo's 12 cards all visible
  and apart at 1280×800 and 1440×900 with the recorder, MIDI, notes-off and transport
  controls on screen and the rack still over 350 px tall.
- `gain` (Utility, voice rate so it works per voice before the average): −60…+24 dB, unity
  default and exact at 0 dB, 20 ms glide, modulation/undo/save like any param
  (`tests/gain.rs`). The mixer and voice averaging are unchanged.
- **Out meter** (status bar): the callback folds every output frame's |L|, |R| into two
  atomics (`PeakTap::feed`, `fetch_max` on the float bits); the UI takes and clears them each
  frame. Peak hold falling 20 dB/s, yellow above −6 dBFS, a CLIP light latched at ≥ 0 dBFS or
  a non-finite sample, click to reset, hover shows the highest peak. No limiter.

### MIDI and recorder recovery (real app, `scripts/recovery.txt`)

Driven through the real release app with the virtual controller; evidence in
`target/recovery` at the time, summarised here:

| workflow | result |
|---|---|
| controller unplugged with a note held | "disconnected: its notes were released"; the note's band falls ~23 dB over the next seconds (release + echo) |
| plugged back in | reconnects by itself (≤ 2 s), every mapping waits for pickup again |
| mouse edit, then the knob far away | no jump (value kept, `↑ 40%` shown) |
| two knobs together, one Ctrl+Z | both restored in one step (also `tests/perform.rs`) |
| All notes off with notes held | released |
| delete the Lead's mixer (pinned + learned), undo | card and mapping gone, then back |
| Stop then Record at once; Load while recording | two takes, both valid; the take continues through Load |
| bad folder (`/proc/…`) | "Recording failed: can't record to …", nothing claimed |
| injected write error (`KABL_RECORD_FAIL_AFTER`) | the take ends by itself, red error, `-INCOMPLETE.wav` kept and readable |
| forced overflow (`KABL_RECORD_RING_FRAMES=128`) | `N frames lost: incomplete` while recording, `-INCOMPLETE.wav` after |
| Ctrl+Q while recording | the take is finalized (valid WAV, header = data) |
| 96 kHz / 64 frames (the device doesn't run it) | status bar: "audio stalled: no callbacks …" |
| three takes in one second, a file already there | `-2`, `-3` names; the existing file untouched (`tests/record.rs`) |

Found and fixed on the way: a pickup at the value nudged it by up to 1/127 and made its own
undo step (now writes nothing); a newly picked-up mapping started a separate undo group (now
joins the gesture); after a writer error the panel kept showing ● REC until Stop (now ends at
once); a failed take wasn't marked (now `-INCOMPLETE`); the transport card could scroll away
(now a fixed column); the toolbar meter overlapped Save/Load at 1280 (moved to the status bar).

### Single-instance compilation audit

`compile_per_voice` keeps every voice-rate module per voice (the compiler before 48ce23c) as
a reference. `crates/engine/tests/single_instance.rs` (6) compares on a patch with a
sequenced chain, a MIDI chain, an LFO route, a delay and a feedback cycle:

- identical output (difference 0.0) with MIDI through jacks, MIDI through a velocity
  modulation route, a bypassed route, and MIDI pitch into the sequenced chain;
- only modules a `midi.in` reaches (through cables or routes, cycles included; bypassed routes
  excluded) are per voice; voices stay independent;
- adding MIDI influence while playing: identical through the swap (every voice inherits the
  single instance's state, which every reference lane had);
- removing it while voices differ (defined behaviour): the single instance continues voice 0;
  no step beyond the reference's at the swap; within 1.6e-5 of the reference one second later;
- six overlapping swaps between the shapes: finite, within 4e-6 of the reference 1.2 s later.

Fixed: the one-block feedback buffers now carry across the transition like module state.

### Callback misses: measured

The status bar and `KABL_STATS_FILE` now keep apart execution time (against the callback's
own audio), arrival (interval between callbacks against the period), backend xruns (cpal's
`ErrorKind::Xrun`), and the frames each callback actually asked for. kabl asks for real-time
priority for its audio thread once, on the first callback (`audio_thread_priority`, MPL-2.0,
via rtkit; `--no-rt` skips it). cpal's own promotion doesn't apply here: the default device is
PipeWire's ALSA plugin, which cpal excludes. Granted on this machine (RLIMIT_RTPRIO is 0; rtkit
grants it).

Same load for every run (`scripts/load.txt`, ~70 s: recording, CC turns on four controls at
once, lead notes, rack knob drags = live swaps with effect-state copies, a Restart), same
host, 1440×900 on Xvfb, negotiated rate and sizes as asked (the device honoured them):

| setting | RT | callbacks | execution worst | late (execution) | arrival worst | arrival > 1.5× period | xruns | take |
|---|---|---|---|---|---|---|---|---|
| 48 kHz, 256 (5333 µs) | on | 11912 | 1246 µs | 0 | 5722 µs | 0 | 0 | 55.9 s, complete |
| 48 kHz, 256 | off | 11919 | 3491 µs | 0 | 9148 µs | 220 | 0 | 56.0 s, complete |
| 48 kHz, 64 (1333 µs) | on | 47658 | 478 µs | 0 | 1622 µs | 0 | 0 | 56.0 s, complete |
| 48 kHz, 64 | off | 45700 | 2888 µs | 1 | 5527 µs | 3691 | **1899** | 53.7 s, complete |
| 96 kHz, 256 (2667 µs) | on | 23846 | 1272 µs | 0 | 3045 µs | 0 | 0 | 56.0 s, complete |
| 96 kHz, 256 | off | 23825 | 3309 µs | 1 | 6449 µs | 2206 | 0 | 56.0 s, complete |
| 96 kHz, 64 | – | stalls after 1 callback (reported) | | | | | | |

The hypothesis holds: without real-time priority the callback is often woken late (arrival)
and at 64 frames the device underran 1899 times, though the work itself stays small; with it,
no late wake-ups, no late execution and no xruns in any setting. ("Take complete" means no
frames lost between the callback and the file; device xruns happen after the callback, so a
take can be complete while playback glitched: the 53.7 s take is short because the stream
lost time.) These are ~70 s runs on one laptop; they are not a real-time guarantee.

### 30-minute soak

`scripts/soak.txt` (generated by `make_soak.py`), 48 kHz / 256 frames, RT on: ten ~3-minute
cycles, each a new take (so Stop → Record ten times), CC turns on four controls at once, lead
phrases and a four-note chord, rack drags, fast back-and-forth tempo drags (bursts of
overlapping swaps) and two undos, Stop / Restart / Run, Save and Load while recording (3×),
the controller unplugged with a note held and replugged, All notes off.

- 361,479 callbacks (32.1 min): execution worst 1234 of 5333 µs, 0 over half, **0 late, 0
  late arrivals, 0 xruns**.
- Memory (RSS every 10 s, 192 samples): 136–158 MB, quartile means 150/149/147/150 MB: no
  growth. Live graph allocations 1 (2 during a swap); the undo log grew to 1056 entries.
- Ten takes, 31.7 min: every header matches its data, no NaN, no silent gaps, peaks −2.3 to
  −0.6 dBFS (the CC turns open filters and feedback well past the demo's defaults), no
  `-INCOMPLETE`.
- Hung notes: a separate run of the same notes, chords, unplug and notes-off cycles ended with
  **0 MIDI notes held** (the allocator's count, now in the stats file).

## Limits and not built

- Pi 4 unmeasured.
- Relative encoders, NRPN, MPE, MIDI clock/transport mapping, 14-bit CC (out of scope). CC
  on switches (stepped controls) isn't offered.
- One MIDI input at a time; mappings aren't tied to a device name.
- The recorder has no input monitoring, no pause and no multitrack; takes aren't normalized.
  Names use UTC.
- A killed process loses up to 0.5 s of its take (the last checkpoint).
- Reverb inputs aren't normalled: patch both `in_l` and `in_r` (a mono source into both).
- The lead needs +12 dB of gain because of the voice average; a mono voice mode would remove
  that.
- Real-time priority depends on rtkit (or an RT rlimit) on the machine; if refused, the
  status bar says so and small buffers may glitch (measured below).

## Hands-on checklist for Kosta

With your controller: `cargo run --release -p kabl-ui -- --patch patches/performance --perform`,
pick the input in the panel.

1. **Sound.** Listen to `audio/performance-take.m4a` and the A/B clips. Does the reverb
   (Dattorro plate) suit the music: decay range, damping, the default 35 %? Is it clean at long
   decays (no metallic ring, no pumping)?
2. **Velocity and gate.** Play with `V1`–`V8` and `Gate length`. Do accents feel right? Is
   LENGTH vs CLOCK clear? Try Stop/Run/Restart and tempo changes: any stuck, doubled or cut
   notes?
3. **Perform panel.** Pin, reorder, unpin a few controls. Is it readable at your window size?
   Are the names clear enough (`Mixer #27 · Level 2 · from VA Oscillator #16`)?
4. **MIDI learn with real knobs.** Learn a few, turn them. Is the pickup comfortable (1.5/127)?
   Is the `pickup: turn up to …` hint understandable? Move the same control with the mouse,
   then the knob: does it wait correctly? Undo a turn: one step?
5. **Keyboard.** Does the lead still play normally while CCs move? Switch the MIDI input in the
   panel: no hanging notes?
6. **Recording.** Record a take, stop the sequencer, let tails ring, Stop. Does the file open
   and sound like what you heard? Is the folder/name handling fine?
7. **Timing.** Watch the status bar's callback line at your usual buffer size. Any `late`?
8. **Demo defaults.** Levels, headroom (peak about −4 dBFS in the take), the ×4 lead mixer.
