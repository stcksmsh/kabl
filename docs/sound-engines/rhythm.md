# Rhythm: arpeggiator, swing and ratchets

Three additions that make a sequence move: an arpeggiator (`arp`), a `swing` knob on `clock`, and a
per-step ratchet on `seq` that `arp` shares. Clocks and gates stay plain signals, so all of it
patches into `logic`, `quantizer`, `random`, `slew` and `sample.hold` with nothing hidden.
Status: engineering verified (below); **listening is Kosta's call**, nobody has heard these.

Reasons for each decision: `docs/decisions.md`, "Rhythm: arpeggiator, swing and ratchets".

## arp

Held keys in, `pitch`, `gate` and `velocity` out, one note per rising edge on `clock`.

| | |
|---|---|
| ports | `clock`, `reset` in; `gate`, `pitch` (semitones, as `midi.in`), `velocity` out |
| `mode` | UP, DOWN, UP-DN (end notes once per turn), PLAYED (press order), RANDOM |
| `octaves` | 1 to 4: the chord repeated that many octaves up |
| `gate_len` | 1 to 100 %: how much of the step the gate is high |
| `latch` | OFF, ON: the chord stays after release |
| `ratchet` | 1X to 4X: each note repeated that many times inside its step |

It hears the keyboard directly (every key event of every source and channel), so it needs no
cable from `midi.in`; patch `arp.pitch` and `arp.gate` where `midi.in`'s would go. Global rate,
like `seq`: every voice of a voice-rate chain it feeds plays the same note. `midi.in`, the keyboard
rules and voice allocation are untouched, so a chord can sound through `midi.in` and be
arpeggiated at the same time.

What it does, defined:

- **One key**: that note repeats on every clock tick (through its octaves).
- **A key added mid-pattern**: UP, DOWN and UP-DN carry on from the pitch last played, so a new
  note above it is reached in this pass and one below it next pass; PLAYED appends it to the end.
- **A key released**: it leaves the pattern at once; the rest carries on. When the last one is
  released the gate falls within the block (no note rings on) and the pattern starts over with
  the next chord. The first note after a silence plays on the next clock edge, never between
  edges.
- **Latch**: pressing a key when none is physically down starts a new chord; pressing while others
  are down adds to it. Turning latch off drops what is no longer held.
- **Sustain pedal**: keys released while it is down stay in until it comes up.
- **Panic / notes off / source lost** clears everything.
- **Reset**: a rising `reset` starts the pattern over on its first note (UP: lowest, DOWN:
  highest, PLAYED: first pressed) and reseeds RANDOM.
- **RANDOM** is seeded from the module id and reseeded on `reset`, one draw per clock edge whatever
  is held: a render repeats exactly, and two arpeggiators play different lines.
- **Gate length** follows the step: as long as the clock interval two edges back (so a swung clock
  gives each note its own length). The first step after a load follows the clock pulse. 100 % still
  drops the gate for one sample so the next note retriggers.
- Limits: 16 keys held; key events land at the start of the block they arrive in and the pattern
  takes them at the next clock edge; the held keys are the player's and are not saved in the sound.
  For slower notes put `clock.div` in front of `clock`.

## swing (on clock)

`clock.swing`, 0 to 100 %. Each pair of 16ths keeps its length; the first stretches to `1 + s`
pulses and the second shrinks to `1 - s`, `s = swing / 200`. 0 % straight, about 67 % the triplet
shuffle (a 2:1 pair), 100 % puts the second pulse three quarters of the way in.

It is on the clock, not on each consumer, so everything stepping on that clock swings by the same
amount: a `seq`, an `arp`, a `random`, a delay locked to it, a patterned cable and a hi-hat
triggered straight from `gate`. A consumer given `clock.div` skips the delayed pulses and plays
straight (swing is a property of 16ths; divide by 2 and you hear straight 8ths). The `reset`
output and the pulse count are unchanged. Sample-exact in free running (the pulse phase is an
`f64` advanced per sample); in a host the pair half and phase are found from the host's beat
position every block, so a loop, a locate and a tempo change land where running there would.
Side effect: the clock's `gate` is high for the first half of its pulse, so the long pulse's gate is
longer than the short one's; modules that time their own gate (`seq` LENGTH mode, `arp`) are not
affected.

## ratchets (seq)

`k1..k8` per bank (also `b.k1` and so on): 1 to 4, default 1. The step plays that many evenly
spaced notes inside its length, each a gate of the step's usual shape (half the sub-step in CLOCK
mode, `gate_len` of it in LENGTH mode) at the step's pitch and velocity. The step length is
predicted from the interval two clock edges back (so a swung pair is handled), the first steps after
a load play plain until two intervals are known, and a clock edge always ends the step.

**With probability**: one draw per step advance as before; the whole ratchet plays or none of
it. A rest (gate off) stays a rest. Editing a probability never shifts the random stream.

Old files: the ratchet params are new names appended after every existing param, so no stored
index moves and a patch without them reads the default 1 (no ratchet). Tested:
`an_old_sequencer_patch_has_no_ratchets` (engine) and
`no_ratchet_by_default_one_note_per_step` (modules). The seq's state keys gained two
(`iv0`, `iv1`), read as 0 when absent. The patch schema is not bumped.

## Demos

Built through the real editor (`crates/ui/tests/arp_demos.rs`; rewrite with `write_patches`,
`write_renders` there), committed under `patches/sound-engines/demos/` and played by a test.

| patch | clip | what it shows |
|---|---|---|
| `arp-chord` | `audio/arp-chord.m4a` (16 s) | hold a Dm7 then a Bbmaj7: up and down over three octaves with a ping-pong delay |
| `swing-seq` | `audio/swing-seq.m4a` (16 s) | one bass line and hat on one clock: 8 s straight, then 8 s with swing 58 |
| `ratchet-roll` | `audio/ratchet-roll.m4a` (20 s) | steps 3, 6, 8 ratcheted 2, 3, 4 times, steps 2, 5, 7 left to chance, a little swing |
| `arp-cable` | `audio/arp-cable.m4a` (20 s) | a random arpeggio gated by a six-step cable running against the 16ths and thrown to a delay by a four-step cable at 70 % |

## Faces

Checked in the real release editor at 100 %, light and dark (`scripts/capture_rhythm.sh`):
`media/rhythm-arp-*.webp`, `media/rhythm-clock-*.webp`, `media/rhythm-seq-*.webp` and the
sequencer's more-controls dialog `media/rhythm-seq-drawer-*.webp`. What the captures changed: the
arpeggiator face is 17 units wide (below 500 points the layout puts the jacks in two rows over the
selector row); the ratchet knobs read "1X" to "4X" (they read "1.00" first); the labels "Ratch 1" and
"Gate len" were shortened so the drawer's 13 columns do not run together.

## Checks

- `crates/modules/tests/arp.rs` (11): every mode and octave order, played order, one key and none,
  key added and released mid-pattern, release at once and restart on the next chord, latch and
  its replacement rule, pedal, seeded random (same seed same render, other seed other line, covers
  the span), gate length and ratchet counts, reset, carry.
- `crates/modules/tests/swing.rs` (8): the straight clock unchanged, swing delay to the sample at
  0/50/100 %, pulse count unchanged, host position mapping, ratchet edges to the sample, rest and
  probability composition over 40 passes, ratchets inside a swung clock's long and short steps.
- `crates/engine/tests/arp.rs` (6): keys reach the arp through the engine, release is immediate,
  panic clears, no allocation across keys and ticks with swing and ratchets, a random render is
  bit-identical twice, swing moves every arpeggio note (3000/9000 samples), an old sequencer patch
  has no ratchets.
- `crates/ui/tests/arp_demos.rs` (3 + 2 writers): demos match their builders, parameters in range,
  compile, finite and in level, the swing clip changes with swing and keeps its energy.
- Golden renders of all committed patches unchanged (`crates/engine/tests/golden_renders.rs`).
- CLAP validator 0.4.1: 35 success, 9 skipped, the same 44 tests and statuses as the baseline
  (`evidence/validator-rhythm.json` against `evidence/validator-util.json`).
- REAPER 7.75, `scripts/host_rhythm.py`, evidence in `evidence/host-rhythm/` (below).

### REAPER

The plugin states have `host_clock` on. The template project already carries a tempo envelope
(120 bpm, 140 from 8 s, 100 from 16.57 s); the script takes the beat-to-seconds map from it.

- Three tracks (swing-seq, arp-chord with a held chord from a MIDI item, ratchet-roll), saved, REAPER
  fully quit and reopened, saved again: plugin states equal at all three points, the 20 s render
  finite (peak 0.29, RMS 0.067) and bit-identical before and after the reopen.
- **Swing follows tempo and transport.** The swing-seq hat alone: 160 onsets found in the render
  agree with where the swung clock puts them, from the project's tempo map across both tempo
  changes, to within 0.82 ms (the detector's resolution is 1 ms); a straight clock would be
  off by up to 43.6 ms. A render starting at 8.2 s, the way a transport jump or a loop restart
  lands, gives 90 onsets within 0.64 ms.
- **The arpeggiator follows it too.** The arp-chord demo with a plain sound (sine, short
  envelope, no delay, swing 58), a chord held 0 to 4.5 s and another from 8.4 s: 117 note onsets
  within 1.64 ms of the swung clock, 87 within 1.29 ms in the render from 8.2 s; the straight grid
  would be off by up to 44.6 ms.
- The older factory-bank project: state unchanged by this build, and its 24 s render bit-identical
  to the plugin built from origin/master (sha256 `24d8729a…e420`).

Not a real loop: REAPER's transport loop was shown by a render that starts mid-project, which is
the same jump of the host position the plugin sees at a loop point, not by a live loop.

## Cost

`docs/benchmarks.md`, "Rhythm". `seq` with the new params costs the same as before (34.7 against
35.5 ns per sample), a fully ratcheted `seq` 45 ns, `arp` 38 to 45 ns, a swung clock 5 ns.

## Files touched under crates/ui/src

`help.rs` (help for the new params and jacks, a note for `arp`), `routing.rs` (step labels, short
names, the `x` unit shown as "2X"), `rack.rs` (ratchet params drawn as knobs in the sequencer's
drawer, two rows of thirteen at 57-point spacing; nothing else in the face code changed).

## Not verified

- How anything sounds. No one has listened.
- A live REAPER loop; a host other than REAPER; Pi 4 cost; physical latency.
- A held chord from a hardware controller through the standalone app (the key path is tested through
  the engine and through REAPER's MIDI, not with a device).
- Strict clippy fails on origin/master in `crates/ui/examples/app_target` (unused items); with
  clippy 1.99.0, `crates/modules/src/wavetable.rs:490` also fails, as on origin/master. This change
  adds nothing to either.
