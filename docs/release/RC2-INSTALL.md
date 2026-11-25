# v1.0.0-rc.2 clean-profile install test (R3)

Date 2026-11-24. Result: **R3 passed**, no step failed. Tarball downloaded with `gh release download v1.0.0-rc.2` (`kabl-1.0.0-rc.2-linux-x86_64.tar.gz`, built from cd9578b). Clean profile: `HOME` and all `XDG_*` pointed at an empty scratch directory. Audio forced to the silent sink with `PIPEWIRE_NODE=kabl_agents PULSE_SINK=kabl_agents`. REAPER 7 used a scratch `-cfgfile`.

| # | Step (from README.txt / install.sh) | Result |
|---|---|---|
| 1 | `sha256sum -c SHA256SUMS` | pass: `kabl-1.0.0-rc.2-linux-x86_64.tar.gz: OK` |
| 2 | Extract tarball; `ldd` on `bin/kabl-ui` and `lib/clap/kabl.clap` | pass: no missing libraries |
| 3 | `./install.sh` | pass, exit 0: `kabl installed: <HOME>/.local/bin/kabl-ui, plugin <HOME>/.clap/kabl.clap` plus the note `<HOME>/.local/bin is not on PATH; use the desktop entry or the full path` |
| 4 | Run the installed `kabl-ui --size 1280x800` | pass: UI opens on the Sounds page (129 factory sounds found), status `audio running`, `real-time priority granted`, 0 warnings or errors in `kabl.log` |
| 5 | Standalone makes sound | pass: default patch "Pluck line" runs its own sequence; recorded the sink monitor for 9 s, peak -4.73 dBFS |
| 6 | REAPER scans `~/.clap` and finds the plugin | pass: scratch `reaper-clap-linux-x86_64.ini` lists `dev.stcksmsh.kabl=1\|kabl (stcksmsh)` (REAPER also listed Surge XT from the system CLAP path) |
| 7 | REAPER loads the CLAP and renders | pass: one track with `CLAPi: kabl (stcksmsh)` (default state) and a MIDI note, `reaper -renderproject` exit 0, 2 s 48 kHz stereo WAV, peak -18.55 dBFS |
| 8 | `./install.sh --uninstall` | pass, exit 0: `kabl removed from <HOME>/.local and <HOME>/.clap (your sounds are untouched)`; only the empty user `sounds` dir and a `mimeinfo.cache` remained |

## Audio routing

`pactl list sink-inputs` while the standalone ran showed one stream: `Sink: 72` (`kabl_agents`), `application.name = "PipeWire ALSA [kabl-ui]"`. The REAPER run was an offline render, so it opened no playback stream (REAPER logged `error creating client` for JACK, which is not running here; harmless). The default sink was not touched.

## Notes for the release text

- README.txt does not say that REAPER needs a rescan or that `~/.clap` is searched by default; it worked here with no extra step. Since added to README.txt (`packaging/linux/package.sh`).
- At `--size 1280x800` the bottom status bar is clipped at the window edge and its text overlaps (see screenshot). Since fixed: the bar keeps its row inside the window and elides the left-side text; `crates/ui/tests/status_bar.rs` checks 1280x800 and 1180x680.
- The screenshot's hint card over the top-right text ("Press play on any sound to hear it.") was a step of the first-run tour, retired in `aabee983` after the build checked here (`cd9578b6`); master no longer shows it.
- The standalone only proves sound through the patch's own sequence; no MIDI device was available.

## Captures (scratch, not committed)

- Standalone screenshot: `/secondary/Programming/Github/kabl/.claude/tmp/r3/cap/standalone.png`
- Standalone sink recording: `/secondary/Programming/Github/kabl/.claude/tmp/r3/cap/standalone.wav`
- REAPER render: `/secondary/Programming/Github/kabl/.claude/tmp/r3/cap/reaper-render.wav` (peak -18.55 dBFS)
