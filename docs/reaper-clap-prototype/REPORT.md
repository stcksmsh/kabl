# D07-P1 bounded integration report

Base `f1e2c9ee204f96e348bf4c874159775d6ba41321` fetched from origin/master; intervening change after issue-time base `e951b25` was this brief. Branch `codex/d07-p1-reaper-clap`. Only `prototypes/reaper-clap/` and `docs/reaper-clap-prototype/` are changed. D05 remains separate.

## Design

Circuit-Stitch [nice-plug](https://github.com/Circuit-Stitch/nice-plug/tree/263b16877d0b0ad30921868338a6df46eadc9a46) at `263b16877d0b0ad30921868338a6df46eadc9a46` supplies the CLAP wrapper and egui 0.35/baseview embedding; its manifest is MIT OR Apache-2.0. [Clack](https://github.com/prokopyl/clack) offers finer direct CLAP lifecycle control but would require more GUI/parameter integration glue. A prototype-only CLAP entry shim bounds and prevalidates state, delegates all other extensions to this wrapper, and issues a host parameter value rescan after successful load. Remaining state-acceptance gaps still prevent D07 approval. The actual kabl rack edits `PatchEditor`; control thread compiles the same `PatchState` into real `PatchEngine` and sends `Owned<CompiledPatch>` through a bounded SPSC queue. REAPER owns devices and transport; plugin creates no CPAL/midir backend or watchdog. The pinned framework does create a background task thread; REAPER lifecycle teardown has not been independently measured.

Audio `process()` reads a host cutoff parameter, pitch notes and at most one graph swap per fixed 64-frame block, then writes stereo. It takes no app mutex and performs no app-side file I/O, heap allocation, logging or graph compilation. The UI and lifecycle paths own the control mutex. basedrop collector drains during editor frames, state loads and lifecycle transitions. The last retired graph can remain until a subsequent drain/deactivation; D07 needs a guaranteed control-thread drain.

The host `cutoff_hz` parameter targets filter module 3 (`filter.svf`, index 0) independently of GUI painting. It is the canonical runtime overlay. On editor frame/reopen, the rack's visible cutoff synchronizes off callback. A save while closed may have an older cutoff in the `patch_v1` field, but host cutoff is separately persisted and reapplied to DSP. Full patch topology is stored in the host field rather than a source path. Rejected GUI snapshots retain the working serialized patch; a rack cutoff edit is sent to the host only after the snapshot compiles and enters the queue. The entry shim rejects malformed/oversized state and a patch that fails a 48 kHz preflight before the wrapper applies parameters. **State loading remains non-atomic under queue saturation:** the 256-slot graph queue can reject a valid patch after nice-plug applies the cutoff, while the wrapper reports success. Other sample rates are compiled again at activation. This is a D07 integration seam, not a production state contract.

CLAP notes are preferred; nice-plug Basic also advertises MIDI. Both map pitch 0–127 to pitch-only on/off/choke, with duplicate pitch-ons counted. Wildcard choke key -1 maps to all-off. Wildcard note-off, per-channel/voice IDs, expression, CC and transport are unsupported. Events after an already rendered block wait until the next 64-frame boundary (0–63 frame variable delay), with no declared fixed latency. Cutoff automation is read per process call, not at each sample offset. This does not solve D06 timing, feedback grid or expressive MIDI.

## Evidence and acceptance

Initial prototype runs failed three state-reproducibility tests and crashed on random invalid state. The bounded state shim and host rescan were built and retested on Ubuntu 24.04.3 with Rust 1.98.1. `cargo test --locked --lib` passed 5/5 and `cargo build --locked --lib` succeeded. The pinned clap-validator 0.4.1 at `152b9823e992d782c5c1fd33bca0295478b919aa` exited 0: **36 success, 8 skipped, 0 failed/crashed**. The previously failing four state cases passed. See [build](evidence/build-test.txt), [summary](evidence/validator-summary.txt), [raw JSON](evidence/validator-full.json) and stderr.

Official REAPER Linux 7.80 ran on an Xvfb 21.1.12 loopback TCP display with a fresh scratch profile and `CLAP_PATH`. Its ReaScript inserted two CLAP instances, saved two MIDI tracks, exited after a bounded GUI session, and then `-renderproject` exited 0. The [render](evidence/proof-render.wav) is 2 seconds, 48 kHz, PCM 24-bit stereo, with nonzero left/right output; each channel peaks at -12.777534 dBFS. Reopening the [project](evidence/proof.rpp) found two tracks and the CLAP FX on each. The host's JACK device failed to open; no live audio is claimed. A scripted normalized cutoff set returned true but read back the default after one second, so host control editing and independent changed-instance recall remain **unverified**. The embedded rack editor did not yield a usable screenshot; repeated editor open/close and source-directory removal remain unverified. See [host run](evidence/host-run.txt) and logs.

| Requirement | Status | Evidence/limit |
| --- | --- | --- |
| Real engine/editor CLAP build | Pass | Five unit tests and cdylib build; editor source path exists, embedding not observed |
| CLAP validation/state fuzz | Pass for pinned suite | 36 success/8 skipped; queue saturation is outside the validator's coverage |
| REAPER scan, MIDI, stereo render | Pass | Two inserted FX and nonzero offline WAV under Xvfb TCP |
| REAPER saved project recall | Partial | Two FX restored; edited control/topology not established |
| Host control edit and GUI rack reopen | Unverified | Scripted set did not read back; no usable editor screenshot |
| Live audio and lifecycle repetition | Unverified | JACK device error; GUI sessions timed out after logs |
| D06 timing and expression | Outside proof | Fixed-block delay and pitch-only map |

## Minimal D06/D07 seams

1. Host engine adapter for arbitrary buffers and timestamped note identities, tested across 1/63/64/65/127/256/512 partitions, preserving modulation/feedback scheduling and declaring latency. D06 owns the timing policy.
2. Bounded versioned state decode, off-callback validation/compile, atomic accept/reject, host rescan and deterministic state bytes. Fix the pinned wrapper or choose a framework exposing these hooks before D07.
3. Editor-independent bounded control submission and guaranteed retired-graph collection, coordinated with D05's lifecycle ownership.
4. Stable host parameter mapping and bidirectional control sync before D08 expands automation; one hardcoded cutoff proves only the narrow seam.

Layout-only rack edits can still cause unnecessary graph compilation/crossfades; this is a performance follow-up for D07. No production API diff is applied. The owner host walkthrough remains open in [CHECKLIST](CHECKLIST.md); this draft does not settle framework approval or D06/D07 acceptance.
