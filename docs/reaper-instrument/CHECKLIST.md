# Owner hands-on checks

Owner review: pending. No box below is checked by the implementing agent.

Start the persistent package with `./launch.sh`; use a copy of the project for edits. The launcher uses its own REAPER profile and packaged CLAP, so it does not require global installation.

- [ ] Play both tracks. Confirm distinct sounds; compare the clearly labeled offline and live audio files.
- [ ] Move Output gain in the host and rack. Close/reopen the editor; confirm the value and audible level persist independently per instance.
- [ ] Open Sounds, choose Expressive Lead, edit a cutoff knob, replug an output cable and use Undo. Confirm the visible and audible results.
- [ ] Save a project copy, quit scratch REAPER completely, reopen the copy, and confirm both edited sounds/topologies survive.
- [ ] Play while opening/closing both editors. Note any stall, silence, stuck note or unreadable control.
- [ ] Render with editors closed; listen through note releases and effects. Repeated renders are currently not bit-exact; report audible differences without treating them as deterministic passes.
- [ ] Connect a physical MIDI controller. Check bend, wheel, channel routing, sustain, cleanup, latency and playing feel. These remain unverified.
- [ ] Record controller input and compare playback with the live performance.
- [ ] Report listening/controller observations and any changes requested.

Optional normal install: copy `plugins/kabl.clap` into `~/.clap` only after checking for an existing file, then rescan REAPER. To uninstall, remove that exact copied file. Close scratch REAPER before deleting its package directory. The agent has made no global installation or configuration change.
