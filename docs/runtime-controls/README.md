# D04 — Runtime controls without recompilation (+ D03-R2 closeout)

Owner-authorized assignment (brief: [`../product-research/briefs/D04.md`](../product-research/briefs/D04.md)).
Engineering: submitted, see [REPORT.md](REPORT.md) and [REVIEW.md](REVIEW.md). **Owner review:
pending** ([CHECKLIST.md](CHECKLIST.md)). Design: [design.md](design.md); parameter
inventory: [classification.md](classification.md) (generated).

Built in a cloud session on branch `claude/d02-musical-controls-knjmgu` (the environment
requires this branch name), restarted from master `4f7e729` (product baseline `6404b21`,
Kosta's PR #5 merge). Exact commits: REPORT.md.

## What it does

- **Knobs, macros, pins and mapped CCs change the sound in place.** Turning a rack knob,
  a Perform slider or macro, a pinned control, or a controller knob mapped to one, no longer
  builds a new graph. The value reaches the playing graph through a small ordered queue and
  moves there over 15 ms (or through the module's own smoothing, or at once for choices and
  sequencer steps). Voices, notes, envelopes, sequencer positions, echoes and reverb tails
  continue untouched, and an open inspector keeps measuring.
- **Structural edits still compile**, exactly as before: adding, removing or rewiring
  modules and cables, bypassing a route, a MIDI In's mode/priority/glide, opening a sound.
- **One document.** Mouse, CC, Undo/Redo, recipe steps and comparison restore all edit the
  same patch; the audio side follows it in order. Saving includes every CC edit.
- **MIDI control runs without the editor.** Mapped CCs (with pickup) and MIDI buttons
  (Run/Stop, Restart, bank launches, cues) are applied by a control thread as they arrive,
  whether or not a window frame is drawn. CC learning still starts in the UI.
- **Pending and failed states.** The status bar shows "N change(s) waiting for audio" while
  the audio thread has not taken them, and "recompile failed: …" (the last good graph keeps
  playing) until a compile succeeds.
- **D03-R2.** *Why no sound?* names a stage on the path whose audio input has no cable
  ("No cable into VCA #4 in: that is its audio input, and an unplugged input reads silence,
  so this stage passes nothing"), never suggests an envelope on a cv as the missing audio,
  offers audio outputs that feed nothing as a possibility, and leaves optional inputs alone
  (cv, a mixer's other channels, one side of a stereo input). The stream-error log line
  gives the age of the last delivered message instead of pairing it with the interval.

## How it works (code)

- `crates/engine/src/runtime.rs`: targets, `ParamSet`, `ToAudio`, `Feedback`, the
  classification (`param_class`, `ramped`, `smoothed_by_module`) and `runtime_changes`.
- `crates/engine/src/compile.rs`: `CompiledPatch::rev`, sorted param/route slots,
  `set_runtime`, ramps (`RAMP_MS`, `MAX_RAMPS`), `param_value`/`route_amount` for tests.
- `crates/engine/src/patch_engine.rs`: `set`, bounded `drain`, stale-graph refusal in
  `receive_swap`, `Transport::Toggle` in `transport`.
- `crates/ui/src/control.rs`: `Delivery` (diff, revisions, held graph/values, flush,
  counts), `deliver`, `midi`.
- `crates/ui/src/main.rs`: `Core` behind one mutex; the control thread `pump`; the MIDI input
  queues CCs and wakes it; the callback calls `PatchEngine::drain`; `KABL_STATS_FILE` gains a
  "control:" line; `KABL_LATENCY_FILE` (measurement only).
- `crates/ui/src/perform.rs`: `apply_cc` takes the events (called by the control layer),
  `rearm`, the Run/Stop button sends `Toggle`. `lib.rs`: `show` no longer applies CCs.
- D03-R2: `crates/ui/src/inspect.rs` (`needed_inputs`, `idle_audio_outputs`, audio-only
  `upstream`), `crates/standalone/src/lib.rs` (`stream_error_line`).

## Tests

- `crates/engine/tests/runtime_controls.rs`: classification of document changes; which values
  ramp; a set value renders bit-identically to a compile of it (modulated too); ramps land
  exactly; revision order across active/fading/pending graphs and a stale graph; unknown and
  reused targets; bounded, ordered, allocation-free draining of a full queue; every voice lane.
- `crates/ui/tests/runtime_controls.rs` (production path, allocation forbidden in every audio
  callback): no graph builds for knobs, macros, pins and mapped CCs; a structural edit during
  CC movement; undo/redo; comparison restore; deletion/recreation and a new document; a paused
  audio thread (bounded, then the last values); a failed compile; MIDI buttons and pickup with
  no frame, then a frame, save and undo; inspector lineage.
- `crates/ui/tests/signal_inspection.rs`: the D03-R2 cable removal, optional inputs, the
  `needed_inputs` table against the registry. `crates/standalone/tests/stream_error_line.rs`:
  the message age. It has its own file so that the heap count in `stream_error_overflow.rs`
  runs alone in its process.

## Compatibility

- **Fixed settings, bit for bit.** `crates/engine/examples/render_hash.rs` renders all 17
  factory sounds (six palette voices, Init Keyboard, the pieces, the recipes, the rack studies)
  for 20 s each with a held chord, single notes and release tails, seeds and sequencers
  running. Baseline `6404b21` and this branch give identical hashes for every one:
  [evidence/render-hash-baseline-6404b21.txt](evidence/render-hash-baseline-6404b21.txt),
  [evidence/render-hash-final.txt](evidence/render-hash-final.txt).
- **A value set before a graph plays** equals a compile of that value, modulated or not
  (`a_set_value_renders_like_a_compile_of_it`).
- **The transition itself differs, by design.** `examples/transition_compare.rs` changes one
  knob mid-note both ways (the old rebuild with its 15 ms crossfade, and the runtime value):
  identical before the change; afterwards the difference is confined to the first 50 ms for
  a filter cutoff, a macro and the echo's feedback (identical after), and stays small where a
  level feeds an envelope or effect tail (sustain, the lead mixer level): the two transitions
  leave slightly different envelope/tail states. [evidence/transition-compare.txt](evidence/transition-compare.txt).
- Saved patches, mappings and the patch format are unchanged (no migration).

## Performance

All numbers come from the cloud VM (4 vCPU, no RT priority). Composition, the dense piece, ran
at 48 kHz / 256 frames through a PipeWire null sink, with xdotool on Xvfb and the fifo MIDI
stand-in. That is not laptop evidence, and no overhead target was set.

The two builds were run interleaved, 3 runs per workload, about 60 s each:

- **base:** `6404b21` plus the measurement-only patch
  [evidence/baseline-measurement.patch](evidence/baseline-measurement.patch);
- **final:** release build of **`e7dca9a`**.

The workloads:

- **steady:** the piece playing on its own;
- **knob:** a rack knob kept turning;
- **cc:** two mapped CCs swept continuously from the fifo controller;
- **swaps:** a cable's undo and redo, a topology change, about 3 per second.

Full table: [evidence/perf-table.md](evidence/perf-table.md); raw data:
[evidence/perf-summary.txt](evidence/perf-summary.txt). Callback execution, arrival lateness
and xruns are kept apart.

| Workload | Graph builds per run (base → final) | Runtime values (final) | Execution p99 bin (base → final) | CC arrival → audio callback (base → final) |
|---|---|---|---|---|
| steady | 0 → 0 | 0 | ≤550–600 → ≤550 µs | — |
| knob | 1075–1148 → **0** | 1136–1170 | ≤1000–1100 → **≤550** µs | — |
| cc | 1230–1266 → **0** | 5929–5943 | ≤1000 → **≤550** µs | p50 49–50 ms, p99 66–68 ms, max 76–167 ms → **p50 2.9–3.1 ms, p99 8.5–8.9 ms, max 16–24 ms** |
| swaps | 201 → 201 | 0 | ≤850–900 → ≤800–850 µs | — |

**Reading.**

- Turning knobs and CCs no longer builds graphs. The callback p99 bin falls to the
  steady-play level while controls move.
- CC-to-audio latency drops by about 16× at the median. In the baseline a CC waited for an
  editor frame plus a compile and crossfade. It now takes the control thread and the next
  callback.
- Topology swaps still compile, and cost the same in both builds.
- Worst-case execution, late callbacks and xruns vary more between runs than between builds.
  VM noise dominates the tails.

**Stalls (censored runs).**

- The known cloud stream stall, with no callbacks for more than 1.5 s, hit **one baseline
  run** (swaps-base-2, after 2161 callbacks) and **one final run** (knob-final-1, after 2260
  callbacks).
- Both are kept in the table with their shortened counts. Neither recovered; recovery belongs
  to D05.
- This D04 data can't assign a cause. The stall happens with and without the new code.

## Real-app evidence

All of this is scripted: xdotool on Xvfb (1600×1000). The app is the release build of
**`e7dca9a`**. Its runtime behaviour is the same as the submitted product head: later commits
change only a test file and docs. Audio goes through ALSA → PipeWire into a silent null sink
and is recorded from the sink's monitor. The "controller" is `examples/midi_player`, writing to
the `KABL_MIDI_PIPE` fifo. None of it is hardware, listening or laptop evidence.

- **Walkthrough:** [walkthrough.mp4](walkthrough.mp4), 63 s, Composition at 1440×900 and
  48 kHz / 256, with audio (mean −19.6 dB, peak −5.7 dB). Script:
  [scripts/walkthrough.txt](scripts/walkthrough.txt). Log, drive timeline and stats:
  [evidence/walkthrough/](evidence/walkthrough/). Timeline, in video seconds:
  - **7–15:** a rack knob (Macros m1), a Perform slider (m3), and mapped CCs 21 and 24 swept by
    the controller. These are runtime values: no graph build.
  - **16.5:** plugging Osc #10 into Mixer #22 in4 while CC 21 is still moving. That compiles
    once (generation 2) and the CC keeps working.
  - **22.5–27.6:** Undo, Undo, Redo, Redo. Only the steps involving the cable compile
    (generations 3 and 4); the undone and redone CC movement doesn't.
  - **29–44:** Compare. Capture a reference, change m4 and CC 24, then restore the reference.
    Restoring applies runtime values and doesn't compile. "Back to my version" also doesn't.
  - **46–53:** Inspect VCA #14 out while CC 20 and the knob keep moving. The reading keeps
    updating ("updated 0.0x s ago").
  - **55.6:** D03-R2. Pulling the cable into VCA #14 in compiles (generation 5), the sound
    goes, and **Why no sound?** names "No cable into VCA #14 in". Undo at 64 compiles
    (generation 6) and brings back the cable, the reading and the sound.

  CC to audio callback during the take: 242 batches, p50 2.97 ms, max 10.15 ms. There was
  1 xrun and no stall.
- **Screenshots**, 1440×900 and 1280×800, in A-light and A-dark, in [img/](img/). Script:
  [scripts/shots.sh](scripts/shots.sh).
  - Composition: `light-runtime-controls`, and `{light,dark}-inspect-during-cc`.
  - Init Keyboard (D03-R2): `{light,dark}-why-audio-input`, `dark-why-undone`.
  - Walkthrough frames: `1440x900-wt-*`.

  The dark 1280×800 inspect shot catches the status bar's "1 change(s) waiting for audio"
  mid-sweep: a value queued but not yet taken by the callback.
- **Package**, [evidence/package.txt](evidence/package.txt), run by
  [scripts/package.sh](scripts/package.sh):
  - The package was built from the checkout and installed in a temp directory with a fresh
    HOME, with the checkout's `patches/` hidden.
  - With the installed Evolving Pad, a mapped CC sweep plus a rack knob drag gave "0 graphs
    compiled · 137 runtime values · 0 actions not delivered".
  - Quitting showed D01's unsaved question. After "Don't save", the log reads
    `shutdown result=ok`.
  - Screenshots: `img/1440x900-package-{runtime-controls,quit-question}.png`.

## Reproduce

Container audio/display as in earlier batches: a session D-Bus, `pipewire`, `wireplumber`,
`pipewire-pulse`, `pactl load-module module-null-sink sink_name=kabl_rec`,
`Xvfb :97 -screen 0 1600x1000x24 &`. Then:

    cargo build --release -p kabl-ui --bin kabl-ui --example midi_player
    cargo test --workspace && cargo clippy --workspace --all-targets
    cargo run --release -p kabl-engine --example render_hash -- 20 patches/palette/pad ...
    cargo run --release -p kabl-engine --example transition_compare
    cargo test -p kabl-engine --test runtime_controls write_classification -- --ignored
    DISPLAY=:97 RUNS=3 docs/runtime-controls/scripts/perf.sh      # needs the baseline binary
    DISPLAY=:97 docs/runtime-controls/record-walkthrough.sh

The baseline binary for `perf.sh`: `git worktree add ../base 6404b21`, apply
`evidence/baseline-measurement.patch` there (measurement only: CC arrival → graph installed),
`cargo build --release -p kabl-ui --bin kabl-ui`, copy it to `target/base-bin/kabl-ui-measure`.

## Known limits

See design.md §12 and REPORT.md "Limits". Everything here is cloud evidence: Xvfb, a null
sink, a fifo MIDI stand-in and no RT priority; not listening, controller-feel or laptop
evidence.
