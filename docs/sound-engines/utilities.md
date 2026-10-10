# Utility modules

Nine small modules that make kabl modular in the Eurorack sense: constrain a pitch to a scale,
freeze a value on a clock, glide, scale, combine gates, blend, pan, and draw random values on a
clock. All are `Category::Utility`, voice-rate, and follow their inputs (see "Per voice or
shared"). Decisions and reasons: `docs/decisions.md`, "Utility modules". Cost: `docs/benchmarks.md`.

Gates are high at 0.5 or more throughout (clock, `midi.in` gate, comparator, logic). Pitch is
semitones from C4, as `midi.in` and `seq` give it.

| module (`kind`) | ports | params | what it does |
|---|---|---|---|
| Quantizer (`quantizer`) | in (pitch) → out (pitch), trig (gate) | `scale`, `root`, `transpose` (advanced) | snaps a pitch to the nearest note of a scale; `trig` pulses (5 ms) when the note changes |
| Sample & Hold (`sample.hold`) | in, clock → out | `mode` SAMPLE / TRACK | SAMPLE takes `in` at each rising clock edge and holds; TRACK follows `in` while the clock is high and holds when it falls |
| Slew (`slew`) | in → out | `rise_ms`, `fall_ms`, `mode` SLEW / FOLLOW | one-pole smoother with separate rise and fall; FOLLOW rectifies first (an envelope follower) |
| Attenuverter (`attenuverter`) | in → out | `amount` (−24 to 24), `offset` (−24 to 24) | `in × amount + offset` |
| Logic (`logic`) | a, b → and, or, xor, not | none | gate logic; `not` is the opposite of `a` |
| Comparator (`comparator`) | in → out (gate) | `threshold`, `hysteresis` (advanced) | gate high while `in` is above the threshold, with a dead band |
| Crossfade (`crossfade`) | a, b, fade → out | `mix`, `curve` (advanced) | blends `a` into `b`; `fade` adds to `mix`; `curve` 0 linear to 1 equal power |
| Pan (`pan`) | in, pan → left, right | `pan` | equal-power stereo placement; the `pan` input adds to the knob |
| Random (`random`) | clock, reset → out | `length`, `change`, `bipolar`, `range` (advanced) | a new value on each clock edge, or a loop of up to 16 values that can slowly change |

## What the existing modules already covered, and what this does not duplicate

- `lfo` has a sample-and-hold *waveform*: a free-running random step at its own rate. It cannot
  be clocked, reset, looped or fed a signal. `random` is the clocked, resettable, loopable
  version; `sample.hold` holds any signal.
- `noise` is audio-rate and white or pink. `random` is stepped and reproducible. A noise into
  `sample.hold` is also a random source, and the demo `stepped-sweep` uses it.
- `seq` has probability and a random step stream inside its own pattern, not a signal other
  modules can use. `random` outputs a plain value.
- `mixer` sums, `gain` is a trim in dB for audio, `vca` multiplies by a control signal. None
  inverts, offsets or crossfades; none pans.
- `clock.div` divides; there was no way to combine two gates.

## Quantizer

`out = nearest note of scale(root) to in, + transpose`. Twelve scales: chromatic, major,
natural minor, harmonic minor, dorian, phrygian, lydian, mixolydian, major pentatonic, minor
pentatonic, blues, whole tone. `root` is 0 (C) to 11 (B). A tie between two notes goes down. A
0.05 semitone hysteresis past the halfway point keeps a slowly moving input (an LFO) from
chattering, and a change of scale or root re-quantizes at once. The first note after the module
starts does not pulse `trig`. `transpose` is applied after snapping, so the result stays in the
scale (a shift by a whole number of semitones that is not in the scale leaves it: that is a key
change). Worst-case cost is a bit scan per input change, 19 ns (`docs/benchmarks.md`).

## Sample & Hold

An unplugged clock never samples: the output stays 0 until the first rising edge. A value that
is not finite is ignored. A held pitch stays exactly as sampled (a bit-exact copy).

## Slew

`rise_ms` and `fall_ms` are the time to get within 1 % of a step; 0.1 ms is the shortest and
passes the signal untouched (tested bit-exact). One-pole, so a big step and a small one take the
same time. The state is f64 because with f32 a slow slew stalled a few ulps short of its target (7.4998
instead of 7.5 at 100 ms, found by the settle test); now it lands exactly. In FOLLOW mode the input is rectified first: `rise_ms` is the
attack and `fall_ms` the release of a loudness follower. The output of a follower is then
usable on any control input.

## Attenuverter

Works on any signal, not only control signals: audio, pitch, gates. `amount` goes to ±24 and
`offset` to ±24 so the same module scales an LFO to a pitch range (`amount` 12) or transposes a
pitch by two octaves (`offset` −24). Param changes ramp in the engine (`compile::RAMP_MS`), so
there is no zipper noise.

## Logic and comparator

`logic` outputs exactly 0 or 1. Two clocks into `and` give a rhythm that fires only where they
coincide; into `xor`, where they differ. `comparator` is a Schmitt trigger: it goes high above
`threshold + hysteresis / 2` and low below `threshold − hysteresis / 2`, starts low, and with a
0.1 dead band a ±0.04 wobble around the threshold does not flip it (tested).

## Crossfade and pan

`crossfade` ports are typed Audio so the interface's "why no sound" tracing follows audio
through it; it blends any signal. With `curve` 0 the same signal on both inputs passes
unchanged at every position (right for control signals); with `curve` 1 the gains are
cos/sin of the position, so two unrelated sounds keep an even loudness through the middle. The
ends are exactly `a` and exactly `b` at any curve. `pan` is always equal power: centre is 3 dB
down on each side, and left² + right² is 1 at every position (tested).

## Random

`out = (draw × 2 − 1 or draw) × range` where each draw is a uniform value in 0..1 from a
xorshift32 stream seeded from the module id and voice lane (the same scheme as `noise`), so a
patch plays the same values every time. The output is 0 until the first clock.

- `length` 0 never repeats. 1 to 16 plays a loop: the first pass draws the values and
  remembers them, then the loop comes round exactly. With `change` 0 the loop's first pass is the same as the
  free-running stream's first values.
- `change` is, per step per pass, the chance that a remembered value is replaced by a new draw.
  0 keeps the loop exact; around 10 % lets it drift slowly (the Tangerine Dream trick), 100 %
  replaces everything. The chance is spent from the same stream, so the evolution is reproducible.
- A rising `reset` reseeds the stream and returns the loop to its first value. If the same edge
  is a clock edge, that edge plays the first value (the clock sees the reset first). This
  discards what `change` has done to the loop: reset means "from the beginning".
- A live edit carries the stream, the loop and its position across (`carry_from`) as long as
  the module keeps its seed; the values continue instead of restarting.
- `range` is the peak size: 1 gives −1 to 1 (or 0 to 1 unipolar), 12 gives ±12 semitones for a
  pitch. For a melody put `random` into `quantizer`.

## Per voice or shared

Every utility is a voice-rate module, like `gain`, `vca` and `noise`: the compiler runs one
instance per voice when a `midi.in` reaches it and one shared instance otherwise (`compile.rs`:
a voice-rate module is voiced only if a `midi.in` feeds it). So each utility *follows its
input*, which is what a patch cable would suggest:

- Clocked from a global `clock` or `seq`: one instance, one value, heard by all voices. A
  random melody from a clock is one shared melody (`scale-walk`, `clock-logic`).
- Clocked from a key's gate: one instance per voice. Each voice gets its own random stream
  (seeded by module id and lane), its own held value and its own glide. A chord of random notes
  (tested: three keys, three different values) is what that gives.
- `quantizer`, `slew`, `attenuverter`, `crossfade`, `pan` are stateless or nearly so; per voice
  only matters for the slew's memory and the quantizer's held note.

The one thing it cannot do is give a global clock a *different* random value per voice; for
that, drive the `random` from the key's gate.

## Demos and clips

`patches/sound-engines/demos/`, built by `crates/ui/tests/sound_engines.rs`, clips in
`docs/sound-engines/audio/` (AAC). All three play by themselves: no keyboard. They need the
transport's clock to run (the internal clock does).

| patch | clip | what it shows | the old modules could not |
|---|---|---|---|
| `scale-walk` | scale-walk.m4a (24 s) | a random loop of 16 values (8 % replaced each pass) held to D dorian; the quantizer's `trig` plays the envelope, so a repeated note rings on | a random melody held to a scale that slowly changes |
| `clock-logic` | clock-logic.m4a (30 s) | one clock divided by 3 and by 4; xor steps a random bass line, and gates a bell, which a slow LFO through a comparator gates again | combine clocks into a new rhythm |
| `stepped-sweep` | stepped-sweep.m4a (30 s) | three sample-and-hold lanes of noise on divided clocks, each smoothed by slew, move the filter, the blend of two oscillators and the pan | a stepped random sweep with a glide |

Levels (peak, no clipping): scale-walk 0.75, clock-logic 0.74, stepped-sweep 0.66. Spectral checks only, no listening: scale-walk's strongest line falls in D dorian
on 139 of 142 notes analysed (the other three were not examined; the quantizer's own output is proved in-scale by
`tests/utilities.rs` and the engine test); stepped-sweep's
spectral centroid moves between 74 and 369 Hz across 3 s windows and its left/right level ratio
between 0.73 and 0.88.

## Verified

- `crates/modules/tests/utilities.rs` (28 tests): every scale against an independently written
  list of degrees at three roots and a 0.3-semitone input grid (output in the scale, nothing
  nearer), chromatic identity and transpose, trig count and length, no chatter from a 0.01 st
  ripple, rescale at once; sample, track, exact held pitch; slew settles within 1 % in the set
  time at 5, 50 and 500 ms and not much sooner, separate rise and fall, bit-exact at 0.1 ms,
  lands exactly, follow mode; attenuverter formula; logic truth table including 0.49 and 0.5,
  two clocks through `and`; comparator threshold and dead band; crossfade ends exact, linear
  middle, unchanged for equal inputs, fade adds and clamps, equal power; pan power and ends;
  random reproducible, in range, mean near 0, loop exact, change drifts, reset, carry; state
  round trip; NaN and infinity never escape any module.
- `crates/engine/tests/utilities.rs` (4): a clock → random → quantizer (D minor) → oscillator
  patch through the compiler plays the same melody every time and every note is in the scale
  (120 probe samples, more than five different notes, within ±14); three keys on a random
  clocked by `midi.in` give three different values, one per voice; all nine modules in an
  eight-voice chain run 1500 blocks inside `assert_no_alloc` with finite audio; every utility
  param change applies in place (no compile).
- Existing patches and projects are unchanged: `crates/engine/tests/golden_renders.rs` hashes
  all 26 committed patches against hashes recorded before the sound-engines work and passes.
- CLAP validator 0.4.1 on the release plugin: 35 success, 9 skipped, the same 44 tests and
  statuses as the recorded baseline (`evidence/validator-util.json`).
- REAPER 7.75 (private Xvfb display, PipeWire), `scripts/host_util.py`, evidence in
  `evidence/host-util/`: a three-track project (scale-walk, clock-logic, stepped-sweep; no MIDI
  needed, the clocks run on their own) saved, REAPER fully quit, reopened, saved again: plugin
  states equal at all three points, the 24 s master render finite (peak 0.30, RMS 0.057, active
  every second), and the render after the reopen is bit-identical to the one before. Replacing
  the three states with a patch without utilities changes the render by 243 % of RMS (spectrum
  cosine 0.31), so the utility modules are what plays. The older factory-bank project: state
  unchanged, and its render is bit-identical to that of the plugin built from origin/master
  (sha256 `be738d7e…d59b`) and to a second render of it.

## Not verified

- How anything sounds: no one has listened. The numbers are measurements.
- The editor was not opened. Help text, short names and step labels are checked by the unit
  tests that every param has help and the face-layout test, not by looking at a face.
- Strict clippy fails on origin/master in `crates/ui/examples/app_target` (unused items from
  the design batch); the rest of the workspace is clean and this change adds nothing.
- Pi 4 cost; physical latency; a host other than REAPER.
