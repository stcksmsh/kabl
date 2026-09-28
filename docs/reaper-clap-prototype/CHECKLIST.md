# Owner host acceptance checklist

The cloud proof now builds, passes the pinned validator and renders MIDI from two CLAP instances in REAPER 7.80. The following are still owner host experiments; use the [fresh-profile commands](evidence/host-run.txt) and do not overwrite existing projects or preferences.

- [x] Build/test and rerun pinned validator: 5 unit tests, 36 validator success/8 skipped; exact build and raw logs in evidence.
- [x] REAPER scan and insertion, two MIDI tracks, nonzero offline stereo render; [project](evidence/proof.rpp) and [WAV](evidence/proof-render.wav) attached.
- [ ] On a working audio backend, open the actual kabl rack and capture it; edit host and rack cutoff, close/reopen repeatedly during playback, and offline render with editor closed.
- [ ] Edit topology, save/quit, hide any source patch directory, reopen, and compare sound, cutoff and cable layout.
- [ ] Change only one of two instances and verify the other remains independent through save/reload; create/destroy and deactivate/reactivate repeatedly while checking logs and collector growth.
- [ ] Measure 1/63/64/65/127/256/512-frame boundaries, rates, note release and state reload. Record onset/timing; do not infer D06 accuracy from validator success.
- [ ] Resolve queue-saturation state acceptance and host parameter readback before approving a production D07 integration.
