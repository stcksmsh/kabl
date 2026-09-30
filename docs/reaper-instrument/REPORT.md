# D07 implementation checkpoint

Engineering: implementing. Owner review: pending. D08 is not authorized.

Base: `91777165cadba5ce81a1d7fdbc4de78563f95ca4`, fetched origin/master on 2026-09-30.
Branch: `codex/d07-reaper-instrument`. Worktree: `/tmp/kabl-d07`.
User checkout remains on `c41420c`; its untracked `.ai/` is untouched.
Authorized brief: [BRIEF.md](BRIEF.md). User requested regular committed checkpoints.

## Completed

Read workflow, current handoff/status/decisions, D07/D08 plan, D06 design/report/review,
prototype report/review/checklist and source, D04/D05 contracts, production timeline/control/
patch engine/editor/serialization interfaces and pinned framework state/lifecycle hooks.
No implementation or acceptance claims yet.

## Environment

Actual laptop: ThinkBook16, Linux `7.0.0-34-generic`, x86_64. Default Rust 1.93.0;
1.98.1 also installed. X11 `DISPLAY=:1`, runtime `/run/user/1000`.
REAPER `/usr/local/bin/reaper`; exact version/audio/hardware pending measurement.

## Integration decision in progress

Keep pinned nice-plug for CLAP lifecycle, MIDI conversion and egui embedding if real-host
vertical slice succeeds. Its upstream loader applies params before persistent fields and
reactivates on state loads. Production needs a separate bounded transactional state extension;
the prototype shim is insufficient. Advertise MIDI only (D06 has no CLAP note-id contract).
Use production Timeline (64-frame latency, no standalone callback-period delay).

## Next action

Implement production `crates/clap` vertical slice: embedded rack/browser, coherent persistent
host control, transactional complete sound state and editor-independent collector. Test in
scratch REAPER profile on existing PipeWire setup. Then finish acceptance, fresh independent
review/fixes/recheck and draft/ready PR according to actual evidence. Commit/push increments.
