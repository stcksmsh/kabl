# D08 — Host production

## D08 strict CLAP repair — 2026-10-05

Engineering complete; strict-profile repair submitted for owner review. Owner review pending. PR #11 remains draft and unmerged. No D09/D10.

R6 is repaired for the declared Kabl strict CLAP profile. Checked exclusive plugin
ownership, immutable activation snapshots, scalar tail publication and bounded SPSC
handoffs replace the reachable callback locks, retrying queues and generic state
rendezvous. State load publishes complete recalled controls without writing native
caches; serialized callbacks own those writes. Pending getters/editor/state observations
retain canonical recall until cache synchronization. Native IDs, v1/v2 state, graph
retirement, automation/flush ordering and Host/Free behavior are preserved.

Starting submitted investigation: `790f0c40a479bdaa55753bc8d63b7dec3729784b`.
Tested/reviewed/packaged product: `f7157619836075d0d7704e59be5d66e6224890b1`. Binary SHA256: `3baa8ee3cc0f2f99e1823e0a741d7301713098f99e92dc84f7556b8d0483aca4`.
Final submitted documentation head is recorded in the bundle's submission.json after
normal push; it changes no product source. Prior product `133fdb89ac56a396db0c7c961c74067782b34efb`
and its original evidence/bundles remain retained with that provenance.

Final checks: workspace612 passed/0 failed/22 ignored; plugin25; framework primitives4;
strict workspace/all-target Clippy, release and validator35 success/9 skips pass.
Actual REAPER Write/replay, closed-editor virtual CC/buttons, full fresh-process recall,
transport and GUI/plugin destruction/recreation are refreshed. All18 native values and
both complete states survive recreation. Two fresh unwrapped production processes each
retain three full36-second renders: first is warm-up, second/third match across processes
with PCM SHA256 `c535792b70919e23b5f8b1f25dd92333f397b6bcbb5e9c3ccb2e5bb1c7e2cb7b`.
Three ordinary renders still differ and remain retained. Additional source-free packaged
fresh recall and policy renders pass, with verified bundle mapping/cwd and unavailable
factory path; packaged warm-up is also retained.

Evidence: [strict-profile index](evidence/strict-profile/index.json),
[callback audit](evidence/strict-profile/callback-audit.md),
[timing summary](evidence/strict-profile/timing-summary.json).
Owner bundle: `/home/stcksmsh/.codex/visualizations/2026/10/04/01a10707-3ccc-73d2-b3af-ee17bddde50b/d08-strict-owner-test`; run `./launch.sh`.
Full raw archive: sibling `d08-strict-evidence`. Previous archives and failures remain intact.

Limits: valid CLAP host lifecycle contracts; off-audio producer backlog can grow when a
host permanently rejects delivery; no exhaustive DSP/host/machine-code proof. Dense two
open editors still show backend ERR=2; worst measured individual process5491.27µs exceeds
the5333.33µs JACK period. Physical xruns and round-trip latency are unmeasured. Reported
plugin latency remains64frames (1.333ms at48kHz). Physical controller, listening, feel,
latency and production-beta approval remain Kosta's checks. Ordinary-render nondeterminism
and dense-editor errors are not resolved by this repair.

Maintenance: retained ISC snapshot at upstream263b16877d0b0ad30921868338a6df46eadc9a46,
original/final source manifests, reproducible patch and upgrade audit checklist in
[vendor/nice-plug/STRICT-PROFILE.md](../../vendor/nice-plug/STRICT-PROFILE.md).
Repeat ownership/ring/state/editor audit and full regression/host evidence on upstream
changes. No Cargo cache mutation, merge, global plugin install or other-chat messages.

## Use

Open the rack's **Automation slots** menu. Sixteen native normalized lanes have permanent host IDs; choose a continuous runtime target by module identity/kind/parameter. Pins and macros appear first. The menu shows current native base values and supports native gestures. Ordinary mapped rack knob edits also record through REAPER Write/Touch. Read automation changes the DSP overlay without adding patch undo entries. Rack knobs show saved document bases; the slot menu shows host playback values.

Reordering panels does not change identities. Deleting a target leaves **Unassigned (deleted)**, including after Undo; explicitly assign a lane to reuse it. Loading another browser patch starts a new document session with unassigned lanes. Host project recall restores saved mappings instead. Removing a mapping restores the latest document base, including queue saturation/retry.

**Host clock (off = Free)** follows REAPER play, stop, tempo, seek and loop boundaries. Host/Synced labels replace rack Run/Restart actions while Host owns transport. Standalone and old D07 projects remain Free. Supported initial bar policy is 4/4; non-4/4 bar launches are unsupported. MIDI devices remain owned by REAPER.

## Owner bundle

Durable source-free Linux x86_64 bundle: `/home/stcksmsh/.codex/visualizations/2026/10/04/01a10707-3ccc-73d2-b3af-ee17bddde50b/d08-strict-owner-test`. Run `./launch.sh` there from any directory. It uses its own profile and packaged production CLAP; the factory path deliberately does not exist. The project embeds two complete states, played scripted MIDI, a synchronized probabilistic sequence, tempo changes and native envelopes. Run `./render-policy.sh` for the declared three-render policy; its first full render is retained warm-up.

Optional install/uninstall instructions are in the bundle's README.txt. No global plugin install or user REAPER profile changes were performed. REAPER 7.75 and PipeWire JACK are prerequisites; the bundle does not distribute REAPER. Owner listening, physical controller, latency and feel checks are unchecked.

## Reproduce

From the dedicated D08 checkout:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p kabl-clap --release
# Place release binary at scratch/host/plugins/kabl.clap; keep isolated profile.
python3 docs/host-production/scripts/run-host.py docs/host-production/scripts/serial-repeat.lua scratch/host/d08-arrangement.rpp serial.done
python3 docs/host-production/scripts/compare-renders.py <second.wav> <third.wav>
python3 docs/host-production/scripts/run-host.py docs/host-production/scripts/record-rack.lua scratch/host/d08-arrangement.rpp rack-record.done
python3 docs/host-production/scripts/run-host.py docs/host-production/scripts/replay-automation.lua scratch/host/rack-recorded.rpp replay.done
python3 docs/host-production/scripts/make-cc-probe.py scratch/host/d08-arrangement.rpp scratch/host/cc-probe.rpp
python3 docs/host-production/scripts/run-host.py docs/host-production/scripts/cc-closed.lua scratch/host/cc-probe.rpp cc.done
D08_PROXY=1 D08_TRACE=transport-final.csv python3 docs/host-production/scripts/run-host.py docs/host-production/scripts/transport.lua scratch/host/d08-arrangement.rpp transport.done
D08_MEASURE_EVIDENCE="$PWD/docs/host-production/evidence/strict-profile/host-measurements.json" D08_MEASURE_ROOT="$PWD/scratch/strict-host/measure" python3 docs/host-production/scripts/run-measurements.py
```

The runner owns Xvfb :110, metacity, an isolated REAPER process and optional silent X11 video. It closes only its own host/display. REAPER Lua and scripted virtual MIDI are actual host evidence, not physical controller evidence. Measurements use an external diagnostic timing proxy; the owner plugin is the unwrapped production binary. Scripts preserve unique completed render files and failure artifacts.

## Limits and provenance

Ordinary repeated renders still differ. Only the documented initialized, device-closed, no-anticipation, full-warm-up policy is verified bit-exact. No claim that every REAPER render configuration is deterministic.

The declared strict CLAP profile has checked exclusive ownership and bounded callback
handoffs. See RT-OWNERSHIP.md for exact coverage and valid-host assumptions. Accepted GUI
output is retained off audio; its backlog may grow under permanent host rejection.

Extreme two-instance 94-module open-editor timing produced backend errors and callback execution near the period. Callback execution, arrival gaps, PipeWire node errors and physical xruns are separate measurements; Dummy-Driver evidence is not physical latency proof.

Sources: existing kabl built-ins/patches (workspace MIT OR Apache-2.0 declaration); nice-plug ISC license copied into the bundle. No new sampled assets, external DSP code, device backend or package dependency. The project carries embedded patch/MIDI state with no external media paths.
