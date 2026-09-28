# D06 design: expressive MIDI and partition-independent event timing

Written before the code. Base: `origin/master` `bd64165840bda6bf543191a01302a4af5165dd34`.
Branch `codex/d06-expressive-midi`. The code is `crates/engine/src/timeline.rs` (the adapter),
`keyboard.rs` (identity, routing, expression), `patch_engine.rs` (delivery),
`crates/modules/src/builtins/midi_in.rs` (sample-offset changes) and `crates/ui/src/main.rs`
(the standalone callback). Tests: `crates/engine/tests/{timeline,expression}.rs`.

## 1. Sample timeline and timestamp origin

The engine renders fixed `BLOCK` = 64-frame blocks. D06 anchors those blocks to one absolute
sample timeline per audio session:

- **Origin.** Sample 0 is the first frame the adapter (`Timeline`) is asked to produce after it
  is created or `reset`. In standalone this is the first callback of a stream session (a D05
  Retry or a sample-rate change starts a new session, a new engine and a new timeline). A
  future host wrapper resets it on activate / `reset()`.
- **Grid.** Engine block *k* covers timeline samples `[64k, 64k+64)`. The grid never moves
  with host buffer boundaries. A host buffer of any length (1 frame and up, varying from call
  to call) only decides *when* the adapter renders a block, never *which* samples a block
  covers.
- **Event time.** An event handed to the adapter at frame offset *o* of a host call starting at
  timeline sample *T* has time *t = T + o*. It is applied inside engine block `t / 64` at
  offset `t % 64`.

## 2. The adapter and its latency

`Timeline::process` walks the host buffer frame by frame. At every timeline sample *h* with
`h % 64 == 0` and `h ≥ 64` it renders engine block `h/64 − 1`: first the optional
`before_block(engine, block_start)` hook, then every queued event whose time falls in that
block (in queue order, each with its offset), then `PatchEngine::process_block`. Output
sample *h* is engine sample *h − 64*. The first 64 output frames of a session are silence.

Why this is correct: block *k* is rendered at host sample `64k + 64`. Every event with
`t < 64k + 64` belongs to a host call that started at or before that sample, so it is
already queued. No event for a rendered block can arrive later. Therefore the engine receives
exactly the same sequence of (events with offsets, `process_block`) calls for every host
partition, and the output is **bit-identical** across partitions after the fixed shift.

**Latency: exactly 64 frames** (`Timeline::LATENCY`), at every sample rate and buffer size.
63 frames is the theoretical minimum for a 64-frame grid with arbitrary partitions; 64 keeps
output blocks grid-aligned and costs one extra frame (0.02 ms at 48 kHz). A host wrapper
(D07) reports `Timeline::LATENCY` through its latency extension (CLAP `latency.get`) and
never changes it at runtime. Offline renders flush by processing at least
`LATENCY` further frames plus the patch's own tail (section 8).

## 3. Event queue: ordering, late, out of range, overflow

- Bounded FIFO of `QUEUE` = 1024 timed events, fixed storage, no allocation.
- **Equal times** keep arrival order (stable). A host must hand each call's events sorted by
  offset (CLAP guarantees this); the adapter does not sort.
- **Out of range** (offset ≥ call length) is clamped to the call's last frame and counted.
- **Late / out of order**: an event earlier than the previous queued event is moved to that
  event's time (never earlier than something already queued) and counted. By construction an
  in-contract event can never be earlier than a rendered block.
- **Overflow.** When the queue is full, a *release-type* event (note off, sustain up, notes
  off, reset controllers, source lost, panic) is never dropped: it sets a bounded fallback
  flag that forces the equivalent source-wide release at the start of the next rendered block
  (a panic when a panic overflowed). Other events (note on, bend, wheel, sustain down) are
  dropped and counted. No stuck note can result from overflow; notes may be lost.
- `midi.in` keeps at most 64 sample-offset changes per voice per block. More merge into the
  last slot (the change lands a few samples early/late within the block; a release never
  disappears). The adapter never delivers that many to one voice in practice; the bound exists
  for the realtime guarantee and is covered by a test.

Counters (`TimelineStats`: queued, clamped, reordered, dropped, forced releases, late live
arrivals) are atomics read by the UI's stats line; nothing is logged from the callback.

## 4. Live MIDI arrival to the audio timeline (standalone)

The MIDI thread stamps each message with `Instant::now()` on arrival. The audio callback
records its own start instant. Messages popped in callback *n* arrived during callback
period *n−1*; each is placed at offset `(arrival − start[n−1]) × rate`, clamped to
`[0, frames[n] − 1]`. This is the standard jitter-removing mapping: the spacing between
messages is kept, at a constant extra delay of one callback period. Messages that arrived
before `start[n−1]` (after a stall or on the first callback) land at offset 0 and count as
late.

Live latency from MIDI arrival to the rendered sample is therefore one callback period + 64
frames: at 48 kHz / 256, 5.33 + 1.33 = 6.67 ms (plus the device's own output latency, which the
app does not measure). The old path applied each message at the next 64-block start of a
partially pre-rendered ring: 0–5.3 ms plus block rounding, varying with arrival phase.
Uncertainty of the new mapping: callback wake-up jitter (the start instant is taken when the
callback runs, not when the device consumed the buffer), clamping when a period is irregular,
and the MIDI driver's own delivery delay before the arrival stamp. The physical
controller-to-sound latency is not measured here; that is an owner check.

Control-thread messages (graphs, runtime values, mapped CCs, transport, commands) are drained
at callback start, as before, and therefore take effect at the first block rendered in that
callback. They are not sample-timestamped in D06 (D08 automation will need timestamped
parameter events; the `before_block` hook is the seam).

## 5. What stays on the 64-sample grid (and why results do not depend on partitions)

Every rule below is evaluated per engine block; engine blocks are identical across
partitions, so each rule gives identical results:

- **Modulation**: every param is evaluated once per block from `source[0]`. A velocity or
  wheel change at offset *o > 0* reaches a modulated param at the next block boundary (≤ 63
  samples later). Audio-rate consumers (envelope gate, oscillator pitch) see the change at
  sample *o*.
- **Feedback cables**: one block (64 samples) late, unchanged.
- **Ramps** (runtime value smoothing, D04): per block, unchanged.
- **Clocks, sequencers, launches**: advance per block with sample-offset edges computed from
  the clock phase, unchanged; the preview countdown is per block.
- **Noise and seeded randomness**: advance only inside `process_block`; seeds by module id
  and lane, unchanged. Splitting host buffers never adds or removes a `process_block` call.
- **Graph swaps and crossfades** start at block boundaries (unchanged). With the adapter they
  take effect at the first block rendered after the control drain.

The rejected alternative, rendering partial blocks at host boundaries, would change the
number and length of `process_block` calls with the partition: the feedback delay, block-rate
modulation sampling, ramp steps and random-state advancement would all move.

## 6. Partial buffers, reset, restart, sample rate, tails

- Partial host buffers: handled frame by frame; no partial engine block ever exists.
- `Timeline::reset` clears the queue, the output block and the counters and sets the clock to
  0. It does not touch the engine; callers reset the engine by building a fresh one (D05
  Retry, a sample-rate change) or by sending `Panic`.
- A sample-rate change requires a new engine (compiled for that rate) and a timeline reset;
  the D05 retry path does both. Offline, `Timeline::new` per rate.
- Render tails: the last performed sample leaves the adapter 64 frames after it is performed.
  Reverb/delay tails are patch-dependent; an offline render adds a documented tail
  (`docs/midi-timing/scripts` uses 4 s). A host wrapper reports tail via its own policy (D07).

## 7. Expressive MIDI contract

### Supported input dialect

Channel voice messages on channels 1–16 from one controller input (standalone) plus the
app's own audition (Preview) source; a future host source uses the same identities.

| Message | Meaning in kabl |
|---|---|
| Note on (vel ≥ 1) / note off / note on vel 0 | Key down / up for identity *(source, channel, key)* |
| Pitch bend (14-bit) | Keyboard bend (below) |
| CC 1 (MSB) | Modulation wheel, 0–127 → 0–1 (7-bit; CC 33 LSB ignored) |
| CC 64 | Sustain pedal, down at ≥ 64, per (source, channel) |
| CC 120, CC 123 | Notes off for that source and channel (releases, pedal up) |
| CC 121 | Reset controllers for that source and channel: bend center, wheel 0, pedal up |
| Other CCs | Unchanged: learn, mappings, MIDI buttons (control thread) |

Not supported and ignored: MPE, channel and polyphonic aftertouch, program change, NRPN/RPN
(bend range is a patch setting, not RPN 0), CC 33 LSB, per-note expression, host note ids. A
host wrapper must not advertise note-id addressing until it exists; CLAP events with a key
and channel map to the same identities, and wildcard key/channel releases map to notes off.

### Identity and ownership

- **Note identity** = *(source, channel, key)*. Sources: Controller, Preview, Host.
- A POLY voice is keyed by *(channel, key)* and records which sources hold it (a bit per
  source). The same key from the controller and the preview on the same channel shares one
  voice (the D01 contract); it releases only when every holder has let go. Different channels
  never share a voice. A repeated note-on from the same holder restarts the envelope on the
  same voice (unchanged).
- A MONO/LEGATO keyboard keeps a bounded list of held identities (`MAX_HELD` = 32; beyond
  that the oldest held key is forgotten, never sounded again).
- **Stealing** follows the existing order (free, then pedal-held, then oldest). A stolen
  voice forgets its old holders, so a later note-off for the stolen note releases nothing.
- **Sustain** is per (source, channel). A released key stays sounding while *any* source's
  pedal is down on that note's channel; the preview uses channel 1, so the controller's
  channel-1 pedal holds an audition like any key (D01 contract).

### Destination routing (bounded)

Each `midi.in` has a **Channel** setting: ALL (default) or 1–16. A note-on, bend, wheel and
pedal-down reach every keyboard whose Channel is ALL or matches. Preview notes reach every
keyboard (audition is a whole-sound action). Defaults keep every existing patch's layering:
every keyboard hears every note, as before.

Releases are delivered by ownership, never by current routing: a note-off, pedal-up, notes
off, reset or source loss goes to every keyboard, and each releases only what it holds. So a
note held before a Channel change is released by its own note-off after the change.

At most `MAX_KEYBOARDS` = 8 keyboards receive notes (unchanged).

### Pitch bend

- **Range**: `midi.in` param **Bend** (`bend`), 0–24 semitones, stepped, default 2, saved
  with the patch, visible on the module face and in its summary ("bend ±2 st").
- **Center/endpoints**: 14-bit value 8192 is exactly 0. 0 is −range, 16383 is +range; the two
  halves scale separately (/8192 below, /8191 above) so both endpoints reach the range.
- **Held-note response**: the bend applies to every voice of that keyboard, including
  releasing tails, from the sample of the message. Unsmoothed (a 14-bit step).
- **Channel scope**: last message wins among the channels the keyboard accepts (a
  conventional non-MPE keyboard). A Channel ALL keyboard follows bends from any channel.
- **Mono/legato/glide**: the bend is added after the glide; glide targets and times ignore it.
- **Reset**: CC 121 or source loss (when that source set it), a Channel change, Load/Retry
  (a fresh keyboard) and deleting/replacing the `midi.in` return it to center. Panic and notes
  off do not move it (the physical wheel/bender is still where it is).
- A bend range change applies to held notes from the next block.

### Modulation wheel

`midi.in` gains an output **wheel** (0–1, unipolar), the same value on every voice lane of
that keyboard, so a cable from it to any knob is an ordinary modulation route (amount,
invert, bypass, block rate) and a wheel feeding a global module is not diluted by voice
averaging. Scope and reset as bend.

This is distinct from absolute CC mapping: a Perform pin mapped to CC 1 keeps its absolute
value with soft takeover exactly as before, because CC 1 still goes to the control thread for
mappings and learn. If a patch has both, both respond; nothing is silently unmapped.

### Cleanup

- **Notes off** (CC 120/123): that source and channel's notes release; that pedal comes up.
- **Panic** (panel "All notes off", Load, Retry, engine start): every note of every source
  releases, every pedal comes up; bend and wheel stay (Load/Retry start fresh keyboards
  anyway).
- **Source lost** (controller disconnect or switch): only that source's notes, pedals and
  expression; a running preview is untouched. Previously a disconnect released everything.
- **Deleting a `midi.in`** releases its voices; **replacing** it starts a fresh keyboard at
  center; no held note is replayed into it.
- **Mode change** releases the keyboard (unchanged). A Channel change resets bend and wheel
  only; held notes keep sounding until their own release.

MIDI never waits for an editor frame: it goes from the MIDI thread through a bounded queue
straight into the audio callback, as before.

## 8. Standalone integration

The callback drains control messages (D04 order, D05 gate/fences unchanged), maps queued
MIDI to offsets (section 4), then calls `Timeline::process` for the callback's frames. The
old pre-render ring is gone. Recording, metering and muting read the adapter's output as they
read the ring before. The legacy `kabl` binary (crates/standalone) is not the product app and
keeps its own block loop; `kabl-ui` is the standalone app.

## 9. Isolated REAPER/CLAP prototype

`prototypes/reaper-clap` keeps building against the new API (legacy `PatchEngine::key`).
It still renders fixed 64-frame blocks with pitch-only notes. D07 replaces its block loop with
`Timeline::process`, maps CLAP note/expression events with their `time` offsets to
`MidiEvent { source: Host, .. }`, and reports `Timeline::LATENCY`.
