# D08 — Host production

D08 adds stable REAPER automation and an explicit Host clock to the production CLAP instrument. Engineering is submitted for review with acceptance limitations; Kosta's owner review remains pending. D07 Output gain identity and version-1 project recall remain compatible. No merge or D09 work is authorized by this record.

Product source: `133fdb89ac56a396db0c7c961c74067782b34efb`. Base: `c6bb8ccdab50dfe91a2b1f002d8c2766351697fc`. Full acceptance, verification and head provenance: [REPORT](REPORT.md), [design](design.md), [REVIEW](REVIEW.md), [owner checklist](CHECKLIST.md).

## Use

Open the rack's **Automation slots** menu. Sixteen native normalized lanes have permanent host IDs; choose a continuous runtime target by module identity/kind/parameter. Pins and macros appear first. The menu shows current native base values and supports native gestures. Ordinary mapped rack knob edits also record through REAPER Write/Touch. Read automation changes the DSP overlay without adding patch undo entries. Rack knobs show saved document bases; the slot menu shows host playback values.

Reordering panels does not change identities. Deleting a target leaves **Unassigned (deleted)**, including after Undo; explicitly assign a lane to reuse it. Loading another browser patch starts a new document session with unassigned lanes. Host project recall restores saved mappings instead. Removing a mapping restores the latest document base, including queue saturation/retry.

**Host clock (off = Free)** follows REAPER play, stop, tempo, seek and loop boundaries. Host/Synced labels replace rack Run/Restart actions while Host owns transport. Standalone and old D07 projects remain Free. Supported initial bar policy is 4/4; non-4/4 bar launches are unsupported. MIDI devices remain owned by REAPER.

## Owner bundle

Durable source-free Linux x86_64 bundle: `/home/stcksmsh/.codex/visualizations/2026/10/04/01a10707-3ccc-73d2-b3af-ee17bddde50b/d08-owner-test`. Run `./launch.sh` there from any directory. It uses its own profile and packaged production CLAP; the factory path deliberately does not exist. The project embeds two complete states, played scripted MIDI, a synchronized probabilistic sequence, tempo changes and native envelopes. Run `./render-policy.sh` for the declared three-render policy; its first full render is retained warm-up.

Optional install/uninstall instructions are in the bundle's README.txt. No global plugin install or user REAPER profile changes were performed. REAPER 7.75 and PipeWire JACK are prerequisites; the bundle does not distribute REAPER. Owner listening, physical controller, latency and feel checks are unchecked.

## Reproduce

From the dedicated D08 checkout:

```sh
cargo test --workspace
cargo clippy -p kabl-clap --all-targets -- -D warnings
cargo build -p kabl-clap --release
# Place release binary at scratch/host/plugins/kabl.clap; keep isolated profile.
python3 docs/host-production/scripts/run-host.py docs/host-production/scripts/serial-repeat.lua scratch/host/d08-arrangement.rpp serial.done
python3 docs/host-production/scripts/compare-renders.py <second.wav> <third.wav>
python3 docs/host-production/scripts/run-host.py docs/host-production/scripts/record-rack.lua scratch/host/d08-arrangement.rpp rack-record.done
python3 docs/host-production/scripts/run-host.py docs/host-production/scripts/replay-automation.lua scratch/host/rack-recorded.rpp replay.done
python3 docs/host-production/scripts/make-cc-probe.py scratch/host/d08-arrangement.rpp scratch/host/cc-probe.rpp
python3 docs/host-production/scripts/run-host.py docs/host-production/scripts/cc-closed.lua scratch/host/cc-probe.rpp cc.done
D08_PROXY=1 D08_TRACE=transport-final.csv python3 docs/host-production/scripts/run-host.py docs/host-production/scripts/transport.lua scratch/host/d08-arrangement.rpp transport.done
python3 docs/host-production/scripts/run-measurements.py
```

The runner owns Xvfb :110, metacity, an isolated REAPER process and optional silent X11 video. It closes only its own host/display. REAPER Lua and scripted virtual MIDI are actual host evidence, not physical controller evidence. Measurements use an external diagnostic timing proxy; the owner plugin is the unwrapped production binary. Scripts preserve unique completed render files and failure artifacts.

## Limits and provenance

Ordinary repeated renders still differ. Only the documented initialized, device-closed, no-anticipation, full-warm-up policy is verified bit-exact. No claim that every REAPER render configuration is deterministic.

The pinned nice-plug framework takes a `parking_lot::Mutex` in process/start/reset. D08 adds no callback locks and allocation guards pass, but the requested whole-plugin no-lock contract is **unmet**. This prevents an engineering-complete claim. Replacing that framework ownership boundary requires a focused dependency change; unsafe lock bypass was rejected.

Extreme two-instance 94-module open-editor timing produced backend errors and callback execution near the period. Callback execution, arrival gaps, PipeWire node errors and physical xruns are separate measurements; Dummy-Driver evidence is not physical latency proof.

Sources: existing kabl built-ins/patches (workspace MIT OR Apache-2.0 declaration); nice-plug ISC license copied into the bundle. No new sampled assets, external DSP code, device backend or package dependency. The project carries embedded patch/MIDI state with no external media paths.
