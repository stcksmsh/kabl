# D07-P1 bounded integration report

Base `f1e2c9ee204f96e348bf4c874159775d6ba41321` fetched from origin/master; intervening change after issue-time base `e951b25` was this brief. Branch `codex/d07-p1-reaper-clap`. Only `prototypes/reaper-clap/` and `docs/reaper-clap-prototype/` are changed. D05 remains separate.

## Design

Circuit-Stitch [nice-plug](https://github.com/Circuit-Stitch/nice-plug/tree/263b16877d0b0ad30921868338a6df46eadc9a46) at `263b16877d0b0ad30921868338a6df46eadc9a46` supplies CLAP export and egui 0.35/baseview embedding; its manifest is MIT OR Apache-2.0. [Clack](https://github.com/prokopyl/clack) offers finer direct CLAP lifecycle control but would require more GUI/parameter integration glue. The state failures below mean this pinned framework is not approved for D07. The actual kabl rack edits `PatchEditor`; control thread compiles the same `PatchState` into real `PatchEngine` and sends `Owned<CompiledPatch>` through a bounded SPSC queue. REAPER owns devices and transport; plugin creates no CPAL/midir backend or watchdog.

Audio `process()` reads a host cutoff parameter, pitch notes and at most one graph swap per fixed 64-frame block, then writes stereo. It takes no app mutex and performs no app-side file I/O, heap allocation, logging or graph compilation. The UI and lifecycle paths own the control mutex. basedrop collector drains during editor frames, state loads and lifecycle transitions. The last retired graph can remain until a subsequent drain/deactivation; D07 needs a guaranteed control-thread drain.

The host `cutoff_hz` parameter targets filter module 3 (`filter.svf`, index 0) independently of GUI painting. It is the canonical runtime overlay. On editor frame/reopen, the rack's visible cutoff synchronizes off callback. A save while closed may have an older cutoff in the `patch_v1` field, but host cutoff is separately persisted and reapplied to DSP. Full patch topology is stored in the host field rather than a source path. Rejected GUI edits retain the working serialized patch. A rejected decoded patch retains the patch/editor snapshot, **but the pinned wrapper applies host parameters first and reports success despite field rejection**, allowing partial sound mutation. State loading is not atomic.

CLAP notes are preferred; nice-plug Basic also advertises MIDI. Both map pitch 0–127 to pitch-only on/off/choke, with duplicate pitch-ons counted. Wildcard choke key -1 maps to all-off. Wildcard note-off, per-channel/voice IDs, expression, CC and transport are unsupported. Events after an already rendered block wait until the next 64-frame boundary (0–63 frame variable delay), with no declared fixed latency. Cutoff automation is read per process call, not at each sample offset. This does not solve D06 timing, feedback grid or expressive MIDI.

## Evidence and acceptance

Initial workspace run on Ubuntu 24.04.3, Rust 1.98.1, official REAPER Linux 7.80 tarball, Xvfb 21.1.12: `cargo test --lib` passed 2 tests at initial review head `f896c4e1dff647fd7cda2e3f0f3a31d1e01988d1`; `cargo build --lib` passed. CLAP validator 0.4.1 at `152b9823e992d782c5c1fd33bca0295478b919aa` returned 32 success, 8 skipped, 3 failed and 1 crashed, exit 1. Note/processing tests passed after guarding invalid note 255. The three state reproducibility tests report Filter cutoff changes from 3000 Hz to 1158.7692 Hz without parameter rescan. `state-invalid-random` SIGABRTed from `Vec::with_capacity` on a declared length of 1025222176999353387 bytes in the pinned framework's `ext_state_load`. This is a blocker, not an accepted validator pass. Review fixes subsequently added rejected GUI snapshot and host cutoff synchronization tests and bounded swaps; reconstructed head retest passed all 4 unit tests and the CLAP library built successfully; see evidence/build-test.txt.

REAPER could be downloaded/extracted, but **not launched as a GUI**. Python `socket.socket(AF_UNIX)` returned `PermissionError: [Errno 1] Operation not permitted`; Xvfb failed to establish local/unix listening sockets. Thus no REAPER scan, stereo sound, editor screenshot, project recall, offline render or repeated instance lifecycle is claimed. The original MIDI fixture is provided for the future host run. On the reconstructed head, Rust 1.98.1 `cargo test --locked --lib` passed 4/4 and `cargo build --locked --lib` succeeded. The pinned validator was rebuilt and reproduced the same 32 success, 8 skipped, 3 failed, 1 crashed result; current raw JSON/stderr are in evidence.

| Requirement | Status | Evidence/limit |
| --- | --- | --- |
| Real engine/editor CLAP build | Prior build pass | `evidence/build-test.txt`; 4 tests pass on reconstructed head |
| CLAP scan, note/audio processing | Validator pass | Processing tests only, not REAPER |
| Host state validation | Fail | 3 reproducibility failures, malformed-state abort, partial load |
| REAPER render/editor/reopen | Unverified | GUI socket blocked |
| Project recall, two host instances | Unverified | Unit field-isolation test only |
| D06 timing and expression | Outside proof | Fixed-block delay and pitch-only map |

## Minimal D06/D07 seams

1. Host engine adapter for arbitrary buffers and timestamped note identities, tested across 1/63/64/65/127/256/512 partitions, preserving modulation/feedback scheduling and declaring latency. D06 owns the timing policy.
2. Bounded versioned state decode, off-callback validation/compile, atomic accept/reject, host rescan and deterministic state bytes. Fix the pinned wrapper or choose a framework exposing these hooks before D07.
3. Editor-independent bounded control submission and guaranteed retired-graph collection, coordinated with D05's lifecycle ownership.
4. Stable host parameter mapping and bidirectional control sync before D08 expands automation; one hardcoded cutoff proves only the narrow seam.

Layout-only rack edits can still cause unnecessary graph compilation/crossfades; this is a performance follow-up for D07. No production API diff is applied. The owner host walkthrough remains open in [CHECKLIST](CHECKLIST.md); this draft does not settle framework approval or D06/D07 acceptance.
