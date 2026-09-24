# Performance batch: reverb, sequence expression, Perform panel, MIDI learn, recorder

Supervisor scope (2026-09-24), owner-authorized as one batch: Kosta can build, play and
record an evolving piece inside kabl. Built on `master` (local, not pushed), commits
`b742a70..`. Rationale: `docs/decisions.md`, "Performance batch".

    cargo run --release -p kabl-ui -- --patch patches/performance --perform       # 1440×900
    cargo run --release -p kabl-ui -- --patch patches/performance --perform --size 1280x800
    # choose the MIDI input in the Perform panel, or: --midi "KL Essential"

Other flags: `--record-dir DIR` (where takes go; default `recordings/`, git-ignored),
`--rate 96000` and `--frames 256` (ask the device for a rate and a fixed buffer).

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
  *Transport*). Cards show the module (`Mixer #27`), the control (`Level 2`), and for mixer
  levels where the channel comes from (`from VA Oscillator #16`, traced upstream to the nearest
  sequencer, MIDI input or oscillator).
- ◀ ▶ reorder, ✕ unpins. Pins are saved with the patch, undo like any edit and never rebuild
  audio. Deleting a module removes its pins and mappings; undo brings them back.
- A card's slider is the real control: same stored value as the rack knob, same undo (one
  step per drag), modulation routes still add on top.
- The transport card sends Run/Stop and Restart to that clock, exactly like the clock's face
  (runtime commands, never undo entries).
- Fixed size (206 px tall, 184 px cards, egui widgets), so it reads the same at any rack zoom.
  Keyboard: Tab to a slider, arrows change it. Both themes.

### MIDI CC learn

- **Learn** on a card (or right-click a module → **MIDI learn ▸**), then move a knob: the
  control gets that CC and channel (`CC 74 · ch 1`). Learning a CC that's already in use moves
  it. **Clear** removes it. Escape or Cancel stops learning. Mappings save with the patch.
- The MIDI input is chosen in the panel (the list refreshes every 2 s) or with `--midi`.
  Switching releases every voice. Mappings are channel + CC, not tied to a device name.
- **Soft takeover** (default, always): a mapping moves its control only once the hardware
  reaches the control's position (within 1.5/127, or by passing it). Until then the card shows
  `pickup: turn up to 42 %`; after it, `● hardware in control`. Any other change to that value
  (mouse, undo, redo, load) cancels the pickup again, so the next turn never jumps.
- A turn is one undo step (messages less than 1 s apart). CC drives the base value; routes
  still modulate on top. Absolute 7-bit CC only, continuous controls only (not switches).
- Notes are untouched: they still go straight to the audio thread.

### Recorder

Right end of the Perform panel: **● Record** / **■ Stop**, elapsed time, file name (hover:
full path), and the folder field.

- Records exactly what goes to the device: final stereo output, 32-bit float WAV at the
  device rate, named `kabl-YYYYMMDD-HHMMSSZ.wav` (UTC).
- The audio callback copies frames into a 4 s lock-free ring; a writer thread writes the file
  and updates the WAV header every 0.5 s. No allocation, lock or I/O on the audio thread.
- Stopping the sequencer leaves the recorder running (releases and tails are captured).
  Recording is a runtime action, never an undo entry.
- Stop finalizes the file; closing the window does too. A killed process leaves a file that
  is readable up to the last checkpoint (≤ 0.5 s lost).
- If the writer falls 4 s behind, the frames that don't fit are counted: the panel shows
  `N frames lost: take incomplete` in red and the file is renamed `…-INCOMPLETE.wav`. Write
  errors (disk full, bad folder) stop the take and show the error.

### Engine change: one instance for chains no MIDI reaches

Voice-rate modules that no `midi.in` feeds (the demo's sequencer voices and pad) used to run
in all 8 voices with identical results. They now run once. Renders are unchanged (every
existing test passes unchanged); the demo's median callback fell from 84 to 35 µs at 64
frames. The status bar now shows live callback timing.

## Demo: `patches/performance`

112 bpm, E minor. Built by `crates/ui/tests/performance.rs`
(`cargo test -p kabl-ui --test performance write_performance_patch -- --ignored`).

| Layer | Patch | Level (card) | CC (ch 1) |
|---|---|---|---|
| Bass | `seq #3`, 8 steps on 16ths, LENGTH 35 %, accented velocities → saw → filter → VCA | Mixer #26 Level 1 | 20 |
| Arp | `seq #4`, 7 steps on 16ths (against the bass's 8), a rest, LENGTH 60 % → square | Mixer #27 Level 1 | 21 |
| Pad | two detuned saws (E3, B3), slow LFO on the filter | Mixer #27 Level 2 | 22 |
| Lead | MIDI keyboard → saw → filter → VCA → `Mixer #25` (×4, see below) → echo | Mixer #28 Level 1 | 23 |

Other pins: clock transport, bass cutoff (CC 24), arp cutoff (CC 25), pad cutoff (CC 26),
bass transpose, echo feedback (CC 27), reverb mix (CC 28), reverb decay (CC 29).

Routing: the bass is dry and centred. Arp, pad and the delay (1/8D, PING, 45 %) share a
stereo bus into the reverb (6 s, 4.5 kHz damping, 30 ms pre-delay, mix 35 %), which returns to
both mains. The arp also has a small echo send. Headroom (offline, 10 s with a lead phrase):
peak −5.8 dBFS, RMS −19.3 dBFS; each layer alone −23.7 to −25.8 dBFS RMS.

`Mixer #25` takes the lead VCA on all four inputs: the voice average makes one MIDI note
1/8 as loud as a sequencer voice, and this is the only gain above 1 a patch can build today
(+12 dB).

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
- `walkthrough.mp4` (1:42, 1440×900, screen and stereo audio of the real release app;
  MIDI from the virtual controller). Script `scripts/walkthrough.txt`, recorder
  `record-walkthrough.sh`, scene times (±0.5 s) in `walkthrough-marks.txt`:

  | ~time | scene |
  |---|---|
  | 0:05 | the patch playing, Perform panel open |
  | 0:13 | the bass sequencer's advanced area: velocity row, gate mode, gate length |
  | 0:19 | step 2's velocity to 100 % (an accent), then undone |
  | 0:26 | gate CLOCK (the old half-step gates), then back to LENGTH 35 % |
  | 0:41 | pin the lead filter's cutoff from the module menu; move it left twice |
  | 0:49 | Learn, CC 74 sent far below the value: `pickup: turn up to …`, a held lead note |
  | 0:57 | the controller turns up: nothing moves until it reaches the value, then it follows |
  | 1:05 | reverb mix to 100 % (fully wet), then undone |
  | 1:23 | Record; 1:32 the sequencer stops, the take keeps the tails; 1:35 Stop, saved |

- `img/` (both window sizes, both themes; script `scripts/shots.txt`): `01`/`02` the demo
  with the Perform panel, `03`/`04` the reverb close-up with its advanced controls,
  `05`/`06` the sequencer's velocity row, `07` pickup pending after Learn, `08` hardware in
  control, `09` recording, `10` the take saved. File names end in `-1280` / `-1440`.

## Verification

`cargo test --workspace`: 279 pass, 9 ignored (patch, fixture and clip writers). `cargo
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

Reading: the compute is well inside every budget (medians 3–13 %), but single callbacks on
this laptop occasionally take several milliseconds. Those are scheduling stalls, not work: the
same callbacks take 35–150 µs offline. At 96 kHz / 256 frames that produced 2 late callbacks
in 9104. The fix is real-time priority for the audio thread (cpal's `audio_thread_priority`
feature, or rtkit); not done in this batch. **Pi 4 unmeasured.**

## Limits and not built

- Pi 4 unmeasured.
- Relative encoders, NRPN, MPE, MIDI clock/transport mapping, 14-bit CC (out of scope). CC
  on switches (stepped controls) isn't offered.
- One MIDI input at a time; mappings aren't tied to a device name.
- Pins can't be renamed; mixer levels are identified by what feeds them.
- The recorder has no input monitoring, no pause and no multitrack; takes aren't normalized.
  Names use UTC.
- A killed process loses up to 0.5 s of its take (the last checkpoint).
- Reverb inputs aren't normalled: patch both `in_l` and `in_r` (a mono source into both).
- The lead needs the ×4 mixer trick (voice average); a gain stage or a mono voice mode would
  remove it.
- The callback runs at normal (not real-time) thread priority; see Callback timing.

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
