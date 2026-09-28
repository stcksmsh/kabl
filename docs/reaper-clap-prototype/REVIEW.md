# Independent review record

## Original prototype review

An independent reviewer inspected base `f1e2c9ee204f96e348bf4c874159775d6ba41321` and the first prototype snapshots. It found an unbounded nice-plug state length abort, three missing parameter-rescan state failures, rejected GUI edits promoted into serialized state on repaint, stale rack cutoff on host changes, unbounded graph swap bursts, and a collector drain gap. The initial in-scope fixes retained the valid GUI patch, synchronized the rack on editor frames, bounded graph swaps to one per 64-frame block, drained retired graphs on lifecycle/state paths, and guarded wildcard choke. The old validator failures and lack of REAPER host observations were recorded at that point. This is historical evidence, not the current result.

## State and host follow-up review

A fresh independent reviewer inspected the new CLAP state shim and the current local head `3dce7784e156091cba428fc19ee928ce5c039352` against the same base, including raw validator JSON, REAPER logs, project, WAV and strict file boundaries. The reviewer made no edits. Findings and dispositions:

| Finding | Disposition |
| --- | --- |
| P1: nice-plug applies params before `PersistentField::set`, which cannot reject; queue saturation can leave changed cutoff with rejected graph while state load returns success. | Unresolved D07 acceptance seam, explicitly stated in REPORT. The shim handles malformed/oversized/invalid compiled patch input before mutation but does not prove atomic acceptance at a full 256-slot queue. |
| P2: a rack cutoff setter was sent before the whole editor snapshot compiled/queued. | Fixed: host setter now runs only after `submit_editor_snapshot` accepts the snapshot. Five unit tests and cdylib build passed afterward. |
| P2: old report implied there was no background thread. | Fixed: REPORT distinguishes no plugin-owned audio device/watchdog from the pinned framework's task thread; teardown is not claimed from REAPER. |
| P2: old review text claimed current validator crashes and no REAPER observations. | Replaced with the current measured result and limits. |

The final recheck found no new P0/P1 implementation issue, confirmed tracked changes remain in `prototypes/reaper-clap/` and `docs/reaper-clap-prototype/`, and corroborated 36 validator successes/8 skips plus the REAPER offline stereo render and two-instance project reopen. It did **not** certify host cutoff editing, changed-instance recall, embedded rack operation, live audio, repeated lifecycle teardown, or atomic state acceptance under queue saturation. Those remain open before production D07 approval. This review supports keeping the PR a draft prototype.
