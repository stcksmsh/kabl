# Echo: the first delay module

Supervisor scope (2026-09-23): a production delay/echo in the approved rack, with live editing.
Built, verified, and waiting for Kosta's sound and control review.

    cargo run --release -p kabl-ui -- --patch patches/echo                    # 1440×900
    cargo run --release -p kabl-ui -- --patch patches/echo --size 1280x800

Commits `8f8e5fa..` on `master` (local, not pushed). Rationale: `docs/decisions.md`, "Delay:
first echo module (supervisor scope)".

## What it is

`delay` (Effect, global rate): `in` (audio, mono) and `clock` (gate) in; `left` and `right`
(audio) out.

| Control | Range | Where |
|---|---|---|
| Time | 20 ms – 4 s, log taper (free time) | face |
| Feedback | 0 – 95 %, no self-oscillation | face |
| Mix | 0 % (dry) – 100 % (wet), equal power | face |
| Sync | FREE, 1/16, 1/8, 1/8D (dotted), 1/4 | face |
| Tone | 500 Hz – 16 kHz low-pass in the feedback loop | advanced (`+2`) |
| Mode | MONO, PING | advanced (`+2`) |

- Under the sync selector a readout says what the delay follows: `sync · 388 ms` (locked,
  in the gate colour), `held · 388 ms` (clock stopped, last time kept), `unlocked · 420 ms` (no
  clock timing yet, the free time plays), `free · 420 ms` (sync set to FREE).
- The Time knob always sets the free time. While locked, the readout (not the knob) shows the
  time in use.
- Echoes land at 1, 2, 3… × the time. Two lines always run as a ping-pong pair. Mono only
  changes the output mix (both lines at 1/√2 to both sides, equal power with ping-pong), so a
  mode switch keeps the tail. It fades over about 20 ms.
- The first echo is full range. Each repeat after it passes the tone low-pass once more, so
  the repeats get darker.
- Time changes glide: a 100 ms one-pole with the read speed limited to 0.5–1.5× (at most
  about ±7 semitones while it moves, never reversed). Big jumps take a while: 20 ms → 4 s
  glides for about 8 s. Feedback, mix and mode are smoothed over 20 ms.
- No tape model, saturation or reverb.

## Sync: acquisition and limits

- The clock is a visible cable. One pulse is one 16th note (the existing `clock` gate).
  Nothing looks up a master clock.
- The delay measures the time between rising edges. An interval counts only if it agrees
  within 10 % with the interval before it. So locking takes three pulses (two agreeing
  intervals). Until then the free time plays, and the readout says `unlocked`.
- Stop: no more edges. The last accepted time stays, and after two missing pulses the
  readout says `held`. Echoes keep decaying; nothing clears the lines.
- Run or Restart: the first interval includes the stopped time (or Restart's one-sample
  gap). It disagrees with the one before it, so it is never taken as tempo. The delay
  relocks on the third pulse after Run. It keeps the old time until then. No reset input was
  needed.
- Tempo changes: a small change is taken on the next pulse. A jump of more than 10 % is taken
  once two intervals agree (two pulses), then the time glides there.
- Limits: sync time is clamped to 20 ms – 4 s (only a divided clock can be that slow). A clock
  that swings or changes tempo by more than 10 % on every pulse never locks, and the free
  time plays. A `clock.div` output works as long as it is steady.
- A clock cable is ignored while sync is FREE, but edges are still tracked, so switching to a
  sync setting takes effect at once.

## Live-edit state

- `StateBuf` (12 scalars) is unchanged. `Module::carry_from(&dyn Module)` is new: after
  `load_state`, `carry_state` hands each module the same-kind module in the playing graph.
  The delay copies both lines (4 s + 4 samples each) and its scalar state. Every other module
  ignores it.
- The carry is keyed by module id, as before, so two delays keep separate histories. It runs
  on the audio thread at fade start, from the graph that is actually playing. For overlapping
  swaps that is the graph promoted when the fade in flight ends.
- The lines are allocated in `prepare` on the UI thread when a graph compiles. Every page is
  written there, so the audio thread never takes first-touch page faults. Retired graphs
  (with their lines) are freed by the `basedrop` collector on the UI thread. The callback does
  no locking, allocation or freeing (the engine tests run under `assert_no_alloc`).
- Cost: the copy is a plain `memcpy`, about 1.5 MB per delay at 48 kHz (3 MB at 96 kHz).
  Measured below.
- **Load is now fresh.** A graph built from Load is marked `fresh`; `carry_state` copies
  nothing into it. Delay lines start empty, and every module starts as after a startup load
  (clocks running, envelopes idle), even where module ids match. Before this change a Load
  carried every module's state by id. A live edit that replaces a queued fresh graph inherits
  the flag, so an edit made right after a Load can't carry the old patch in either.
- Saved patches store configuration only (op log; `patches/echo` is a few kB). A startup
  load and a Load both start with empty delay memory. Undo, redo and every other edit are live
  edits and keep the tail.

## Demo: `patches/echo`

`patches/interlocking` (116 bpm, bass on 16ths, lead on 8ths) with the lead through a delay:
1/8D (dotted eighths against the lead's straight eighths), 55 % feedback, 3.2 kHz tone, 32 %
mix, ping-pong. Each delay output goes to its own mixer (left, right), with the dry bass on
channel 1 of both, so the bass stays dry and centred. Levels: bass 0.22, lead 0.14. The
clock's gate is patched to the delay's clock input. The slow LFO has a small route (+0.02) on
the Time knob, which only moves the time when sync is FREE. Built by `crates/ui/tests/echo.rs`
(`cargo test -p kabl-ui --test echo write_echo_patch -- --ignored`).

Headroom, rendered offline for 20 s (`crates/engine/tests/echo.rs`): peak −6.4 dBFS, RMS −18.3
dBFS; with mix at 0 %: peak −6.9, RMS −18.1. The A/B clips below are within 0.3 dB RMS of each
other. In the recorded walkthrough, with feedback raised to 70 % and 83 %, the peak is −5.9
dBFS.

## Evidence

- `walkthrough.mp4` (2 min, real release app, stereo audio and screen, 1440×900). The app's
  output went to a PipeWire null sink and was recorded from its monitor, so nothing played on
  speakers. Script: `scripts/walkthrough.txt`, recorder: `record-walkthrough.sh`. Approximate
  video times (±0.5 s) are in `walkthrough-marks.txt`:

  | ~time | scene |
  |---|---|
  | 0:00 | echo as saved |
  | 0:08 | mix 0 %: dry; 0:14 back to 32 % |
  | 0:21 | straight 1/8; 0:28 back to 1/8D |
  | 0:35 | mono; 0:42 back to ping-pong |
  | 0:48 | feedback 70 %; 0:56 tone 1 kHz; 1:03 both undone |
  | 1:08 | FREE: the LFO moves the time slowly; 1:19 back to 1/8D |
  | 1:24 | tempo 116 → 142 bpm (relock and glide); 1:31 back |
  | 1:37 | feedback 83 %, then 1:42 Stop: notes release, echoes decay (`held`) |
  | 1:44 | bass cutoff edit (a live graph swap) during the tail |
  | 1:51 | Run; 1:57 both edits undone |

- `audio/*.m4a`: 10 s offline renders of the same passage, for A/B listening: `01-dry`,
  `02-echo`, `03-straight-eighths`, `04-mono`, `05-free-time-lfo`.
- `img/`: the real app at 1440×900 and 1280×800, A-light and A-dark: the full rack (`01`,
  `05`), the delay expanded (`02`), close-ups locked (`03`, `04`) and stopped/held (`06`).
  Script: `scripts/shots.txt`.

## Verification

- `cargo test --workspace`: 240 pass, 7 ignored (fixture, patch and clip writers), clippy
  clean.
- `crates/modules/tests/delay.rs` (13): impulse lands on the time (48, 44.1, 96 kHz); repeats
  decay by the feedback and darken; ping-pong alternates L/R; mix endpoints exactly dry and
  exactly wet; block sizes 1/17/64 give identical output; sync locks after two agreeing
  intervals and follows 1/16–1/4; a synced echo lands on the clock; stop holds and a restart
  gap is not read as tempo; time changes and mode switches stay smooth (sample steps near the
  input's own); 30 s of noise at 95 % feedback with swept time stays finite and bounded;
  `carry_from` copies history and ignores other kinds.
- `crates/engine/tests/echo.rs` (6, all audio-thread calls under `assert_no_alloc`):
  headroom; the tail through Stop with identical, unrelated and overlapping swaps equals the
  no-swap render (max difference < 1e-4); two delays keep separate histories through six
  swaps; a feedback edit mid-tail has no dip; a fresh swap equals a fresh engine exactly after
  the crossfade, while the same swap as a live edit keeps the tail; the lock report; the saved
  patch is small.
- `crates/ui/tests/interaction.rs::delay_controls_and_a_fresh_load`: face and advanced
  controls through real egui input; Load sets the fresh flag.

## Callback cost

`cargo run --release -p kabl-ui --example bench_echo [frames]`, pinned with `taskset -c 2`.
Host: i7-13700H laptop, Linux 7.0, rustc 1.93, 48 kHz, 8 voices, full 4 s lines filled with
signal. Each callback runs `receive_swap` plus `frames / 64` engine blocks. A new graph (the
same patch) arrives every 4th callback, and three at once every 40th. Swap graphs are compiled
outside the timing, as the UI thread does. Worst observed callback, µs:

| frames (budget) | patch | steady | with swaps (any callback) |
|---|---|---|---|
| 64 (1333 µs) | no delay (interlocking) | 137 | 171 |
| 64 | one delay | 64 | 294 |
| 64 | two delays | 46 | 546 |
| 256 (5333 µs) | no delay | 251 | 619 |
| 256 | one delay | 297 | 738 |
| 256 | two delays | 339 | 955 |
| 1024 (21333 µs) | no delay | 906 | 1422 |
| 1024 | one delay | 1019 | 1954 |
| 1024 | two delays | 1133 | 2203 |

Medians: one delay adds about 11 µs per 256 frames of steady processing. Each swap that
carries one delay costs about 80 µs more than one without (the 1.5 MB copy). The worst
callback stays under a fifth of its budget at 256 frames and under half at 64 frames.

Limits of this measurement: one desktop CPU, a busy laptop (single-run maxima are noisy: see
the no-delay steady max at 64 frames), no real audio device in the loop, 48 kHz only (at
96 kHz the copy doubles). **Pi 4 unmeasured**: its memory bandwidth is several times lower,
so the per-swap copy is the first thing to measure there. Copying only the span in use would
be the upgrade path (`ponytail:` note in `delay.rs`).

## Not built (out of scope or deferred)

Reverb, tape model, saturation, self-oscillation, toolbar transport, compact layout, MIDI sync,
a reset input (the agreement rule made it unnecessary), a Time knob that shows the synced
time, modulating the synced time.

## Listening checklist for Kosta

1. Echo against dry: do the dotted-eighth repeats sit behind the lead without blurring the
   bass? Is 32 % mix / 55 % feedback a useful starting point?
2. Straight 1/8 versus 1/8D: does the dotted setting interlock the way you expect?
3. Ping-pong versus mono: is the width right, and is mono equally loud?
4. Feedback and tone: do the repeats darken enough? Is 95 % a sensible top?
5. Tempo change and FREE time with the LFO: are the glides acceptable (pitch bends, no clicks)?
6. Stop: do the echoes decay naturally and does the readout (`held`) make sense? After Run,
   is the three-pulse relock quick enough?
7. Edit anything during a tail: does the tail always carry on?
8. Controls: is the face choice right (time, feedback, mix, sync; tone and mode advanced)?
   Are the labels clear (`1/8D`, `PING`, the readout words)?
