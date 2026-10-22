# Early notes lost in a REAPER offline render

## Cause

A REAPER offline render lost the first notes of a project because the plugin asked the host to restart it while the render was already running.

REAPER switches an active plugin to offline mode when a render starts (`clap_plugin_render.set`). The nice-plug wrapper answers a mode change on an active plugin with `request_restart`. REAPER then deactivates and activates the plugin in the middle of the render, and the stretch of the timeline that is processed during the restart is never delivered to the plugin: no audio block and no MIDI event for it. A note on in that stretch is gone, and so is the sound it would have made.

The loss depends on state size because the restart lands a short wall-clock time after the render starts (about 0.1 to 0.3 s), and how that falls against the render's first stretch depends on how long activation and state load keep REAPER busy. The load itself was measured at about 145 ms for a 20 KB state; its growth with size was not isolated. The size is the trigger, the cause is the restart request.

Evidence in `evidence/`:

- `baseline-lost-run-blocks.txt` and `baseline-ok-run-blocks.txt`: the plugin's view of one lost and one sounding run (blocks with their song position). The lost run processes 0.000 to 0.445 s, is activated again, and resumes at 0.576 s. The note on at 0.5 s sits in the gap. In the sounding run the gap (0.616 to 0.811 s) holds no event.
- `offline-lost-notes-by-state-size.txt`: runs lost out of 5 for a note at 0.5 s, with the render started as soon as the project is loaded (`DELAY=0`) and after 2 s (`DELAY=2`).

| state | before, render at once | before, render after 2 s | after (both) |
|---|---|---|---|
| 8.8 KB | 0 of 5 | 0 of 5 | 0 of 5 |
| 41.6 KB | 2 of 5 | 0 of 5 | 0 of 5 |
| 129 KB | 4 of 5 | 0 of 5 | 0 of 5 |

With the render at once and a 20 KB state, 6 of 8 runs lost the note before; with only the render extension changed, 0 of 8 did; with only the latency change, 1 of 8 did.

Real-time playback is not affected (no mode switch happens): three recordings of a 129 KB state with the old build all sound from 0.5 s. `evidence/fixed-realtime-130k.txt` and `fixed-offline-130k.txt` are the new build.

## Fix

`crates/clap/src/state_bridge.rs`:

- The plugin answers `clap.render` itself: `set` accepts either mode and does nothing (kabl renders the same offline and in real time; nothing reads the mode), `has_hard_realtime_requirement` is false. No restart request.
- The plugin answers `clap.latency` itself with the engine's constant latency, from the first query. Before, `activate` reported the latency, and the wrapper starts at 0, so the first activation told the host "latency changed" from inside `activate`, which a host answers with a restart. The restart was not the cause of the lost notes in the runs above (see the table), but it is the same kind of fault and costs a restart per instance.

No state or schema change. Old projects and the factory bank load and render as before.

## Test

`state_bridge::tests::render_mode_switch_and_activation_never_request_a_host_restart` activates the plugin through the exported CLAP entry, switches the render mode offline and back while it is active, activates it again, and counts the host's `request_restart` calls (must be 0). It also checks the latency answer before the first activation. With the render extension disabled it fails with 2 restarts.

## Reproduce

```
cargo build --release -p kabl-clap
DELAY=0 python3 docs/reaper-early-notes/scripts/host.py STATE_BYTES 0.5 offline   # or realtime
```

The project is one track, one instance of `imported-morph` with its table cut to the size asked for, one note at 0.5 s. `DELAY` is the seconds REAPER waits after loading before it starts the render. Needs `Xvfb`, `metacity`, `pw-jack` and REAPER as in `docs/cue-macros/scripts/host.py`. The offline result is a race: repeat a few times. `realtime` records a second track fed by a send from the first (an armed track does not play its own items).

## Not found out

- A second CLAP host was not available (no Carla, Ardour, Qtractor or clap-host installed). The cause is a host-specific reaction (REAPER restarts on request mid-render) to a request the plugin no longer makes, so a second host would only show that the request is harmless there. The `clap-validator` run is unchanged against its baseline.
- Why REAPER drops the stretch during its restart rather than delivering it is REAPER's behaviour; it is not fixed by anything in the plugin other than not asking.
- Patches up to 2 MiB were not each rendered; the table limit gives about 1 MB of state, and 129 KB (the size that failed most) was.
