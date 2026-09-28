# D06 independent review

A fresh read-only reviewer subagent (Claude, `code-reviewer` type, no implementation context
beyond the assignment) reviewed base `bd64165840bda6bf543191a01302a4af5165dd34` against head
`b32049f8d538a1b82dbc196ec41f96ae8fb88bf0`. It read `timeline.rs`, `keyboard.rs`,
`patch_engine.rs`, `compile.rs` (`key_action`), `midi_in.rs`, the `main.rs` callback, the four
engine test files, REPORT and design. Not covered by it: UI layer files (`lib.rs`, `rack.rs`,
`routing.rs`, `help.rs`), `main.rs` retry/recording machinery beyond the callback, the CLAP
prototype, hardware. It did not rerun the implementer's full test suite.

| ID | Severity | Finding (file/function, scenario) | Resolution |
|---|---|---|---|
| R-01 | moderate | `timeline.rs::block`: an overflowing `SourceLost`/`ResetControllers` was forced as 16× `NotesOff`, which ends notes and pedals but leaves bend/wheel set — expression leaks. | Forced releases now apply a `SourceLost` for that source at the release's time (notes, pedals, bend and wheel end). Test: overflowing `ResetControllers` returns expression to rest (`late_out_of_range_and_overflowing_events_never_strand_a_note`). |
| R-02 | moderate | `patch_engine.rs::send_key`: `sync_keyboards` ran for every event (bounded, but k×8 redundant scans per block). | `sync_once`: at most once per rendered block (settings only change in `process` or on `receive_swap`, which syncs itself). All keyboard/preview/timeline tests and factory hashes unchanged. |
| R-03 | minor | `midi_in.rs::schedule`: on overflow, a release overwrote the last entry's type and fired at that entry's (later) offset. | A release now replaces the latest change and is inserted at its own sample; test asserts the gate falls at sample 10 exactly. |
| R-04 | minor | Dropped starting events (full queue) are only visible in the stats file. | Disclosed; follow-up (show adapter drops in the UI). A 1024-event backlog is far beyond ordinary input. |
| R-05 | minor | Stats line showed "arrival to frame 0.00–0.00 ms" before any MIDI. | Shows NaN (no data) until the first event. |
| R-06 | moderate (doc) | design.md said the pre-render ring was gone, but the callback still stages through it. | design.md §8 corrected: the ring is a per-callback staging buffer that the timeline fills exactly. |
| R-07 | nit | `KeyEvent::from_midi` still maps CC 120/123 to a panic while `MidiEvent::parse` maps them to channel notes off. | Intentional legacy entry point (documented on the function); production uses `parse`. No change. |
| R-08 | nit | Stats atomics `store` cumulative timeline totals; a timeline reset within one stream would show stale totals. | The timeline is only reset with a new stream, which gets new counters. No change. |

Reviewer verdict at `b32049f`: needs changes on R-01 and R-02; the rest quality items. It found
the partition-invariance, sample-accuracy and allocation claims supported directly by the
tests.

## Recheck

Pending (fix head below).
