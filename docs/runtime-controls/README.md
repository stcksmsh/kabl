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
  `needed_inputs` table against the registry. `crates/standalone/tests/stream_error_overflow.rs`:
  the message age.

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

PERF

## Real-app evidence

WALK

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
