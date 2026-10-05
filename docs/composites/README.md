# Reusable, inspectable composites — D09

Product checkpoint: `a337f2c1eae867671f9deabadfa10f681de2bb26`. Kosta selected chronological project Undo; independent instance state is required, separate per-instance histories are not. Final history validation is at `2c3d8319911a90c5960f9e02baa8dfd27c01de8f`. Runtime binaries remain exactly those already verified at the product head. Final independent disposition and draft submission are recorded in REPORT/REVIEW. This branch is not merged or owner-approved. See [REPORT](REPORT.md), [REVIEW](REVIEW.md), [acceptance matrix](ACCEPTANCE.md) and [owner checklist](CHECKLIST.md).

## Workflow

Open Composites in the toolbar. Select ungrouped leaves or top-level composites, enter a name and encapsulate. Boundary routes and existing pinned controls become stable public aliases. Open internals reveals the original modules and their routes; Close internals returns to compact cards. Scroll within a card to reach its remaining controls. Use instance tools to expose a real port/parameter, rename the label or remove an unconnected exposure. Label changes never retarget its ID.

Duplicate creates fresh group, leaf and internal cable IDs below the existing rack. It copies parameter state, embedded definitions and pins with new order positions. External cables stay unpatched. CC/button mappings and native automation assignments are omitted so the copy cannot accidentally share controller/host destinations. Existing assignments remain attached to original leaves. External cue/clock references remain original during same-project duplication; publishing/importing requires including dependencies, with an error naming any unresolved target.

Instance edits affect only its embedded graph. Publish a new immutable version writes a standalone library package beneath the existing user library's `composites/<definition-id>/vN.json`. Insert explicitly chooses a version and creates independent embedded leaves. Publishing, updating or removing library files never changes an existing song. A song does not resolve a library entry on load.

## Chronological project Undo

Kosta explicitly selected the existing chronological project Undo on 5 October 2026. Undo reverses the latest project transaction regardless of which instance is open; Redo reapplies it. Independent instance state is required; separate per-instance Undo histories are not. A focused A/B/A edit sequence proves that values remain isolated and Undo/Redo follows chronology after standalone save/reload. Host recall embeds sound state, not a persistent undo stack; standalone save retains its applied project log.

## Evidence and reproduction

Linux x86_64, Rust/Cargo 1.93.0; REAPER 7.75; PipeWire/JACK through pw-jack. UI captures use Xvfb, metacity and scripted X11 input, with exact 1440×900 and 1280×800 client viewports in both themes. No capture is a design render or physical-input claim. FLAC examples are scripted 48 kHz stereo engine renders. The walkthrough video is silent.

```
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --release -p kabl-ui -p kabl-clap
cargo run -p kabl-ui --example composite_demo -- docs/composites/examples
cargo build -p kabl-ui --example composite_host_fixture
python3 docs/composites/scripts/make-host.py
python3 docs/composites/scripts/workflow.py
python3 docs/composites/scripts/capture-app.py
```

Host reproduction uses the unchanged isolated runner in `docs/host-production/scripts/run-host.py`, D09 `scripts/replay.lua`, and the D08 `recall.lua`/`serial-repeat.lua` scripts. Stage the release plugin at `scratch/host/plugins/kabl.clap` and use a private REAPER profile. Accepted serial rendering initializes audio, disables anticipation, closes the device and retains a full warm-up before comparing renders. First-process and source-free bundle PCM hashes match repaired D08's accepted PCM exactly. Ordinary-render nondeterminism is not fixed or relabeled.

[Voice](media/voice.flac), [stereo effect](media/stereo-effect.flac), [actual REAPER arrangement](media/reaper-arrangement.flac), [real widget walkthrough](media/workflow.mp4), [small light viewport](media/app-1280x800-light.png), [large dark viewport](media/app-1440x900-dark.png). Examples contain portable packages, standalone logs and a two-track REAPER project. Final commands, heads and artifact hashes are indexed in `evidence/verification.json`. Intermediate failures and complete raw WAVs remain in the task-owned raw archive; they are not additional passes.

Source-free owner bundle: `/home/stcksmsh/.codex/visualizations/2026/10/05/01a10979-ac89-7a01-bd38-576bb907cce1/d09-owner-test`. Its README/launchers explain operation and limitations. Vendor nice-plug retains its ISC license and [strict profile](../../vendor/nice-plug/STRICT-PROFILE.md); no new runtime or third-party dependency was added.

## Limits

Depth 8, 128 groups/leaves, 512 cables, 32 exposed ports and 32 controls per group, 8 delay/chorus/reverb leaves and the existing host state byte cap. No custom DSP runtime, panel authoring, marketplace or automatic library updates. Import/export supports closed dependency sets; arbitrary external rebinding is deferred. UI reuses existing cards, sliders, routing inspector and library directory; broad visual polish remains outside D09.

D08 hardware/controller, listening/feel, physical latency/xrun and production-beta owner checks remain pending. D08's 64-frame plugin latency, ordinary-render nondeterminism, dense two-open backend ERR2 and process maximum 5491.27 µs exceeding its 5333.33 µs period remain historical limits. No D09 grouping latency is introduced: compiled leaf schedules and PCM are unchanged. That does not prove physical-device latency.
