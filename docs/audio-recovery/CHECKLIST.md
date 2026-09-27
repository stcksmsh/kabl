# D05 owner review (pending)

Use the branch/PR identified in REPORT; do not merge until satisfied. Record pass/fail and device details beside each item.

- [ ] On your 48 kHz / 256 output, confirm sound and actual callback frames in the status tooltip. Note model/backend: ______
- [ ] While a stream is unavailable, edit a knob and save. Select a valid output and Retry in the same app. Confirm the latest sound plays, then try an invalid patch and confirm its error stays visible. Result: ______
- [ ] Start a recording, disrupt/restart audio, inspect the interrupted WAV prefix and path; reconnect, explicitly Record again and confirm a separate file at the new device rate. Result: ______
- [ ] With held notes, sustain, a playing sequence and a queued launch, Retry. Confirm tails/notes stop, no old launch fires, clocks remain stopped until Start, and MIDI pickup needs a new crossing. Result: ______
- [ ] Repeat failed and successful retries and quit during a delayed retry. Watch for duplicate streams, hanging teardown, growing resource counts, or stale readings. Result: ______
- [ ] On a real controller, try mapped CCs while Save/Open and device reconnect run; report any control lag and operation duration. Result: ______
- [ ] Listen to Init Keyboard, dense Sound Palette and Composition; compare old patches, crossfade, mono/poly gain and note changes. Result: ______
- [ ] Check the recovery status/readability in light and dark themes at 1440×900 and 1280×800. Result: ______

These checks are Kosta's hands-on disposition; a cloud software PCM test is not a substitute.
