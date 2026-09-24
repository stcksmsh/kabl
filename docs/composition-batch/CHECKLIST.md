# Hands-on checklist: Composition + Motion

    cargo run --release -p kabl-ui -- --patch patches/composition --perform --rate 48000 --frames 256
    # add --size 1280x800 for the smaller window; --midi "KL Essential" to pick the controller

Controller map (channel 1): CC 20–23 Energy / Motion / Space / Glow, CC 24 Lead level. Buttons
(absolute CC, press = 127): CC 40–44 cues Intro / Main / Variation / Breakdown / Return, 45
cancel, 46 run/stop, 47 restart. Re-learn any of them from a module's right-click menu
(MIDI learn for knobs, **MIDI button** for launches and transport).

## Banks

- [ ] The patch starts on Intro: both sequencers' PLAY **A** is lit, the Cues card shows
      Intro lit.
- [ ] On a sequencer, click EDIT **C**: the face shows bank C, the tag says "editing C …
      (not playing)", the step light turns hollow. Turn a C knob: the sound does not change.
- [ ] Right-click an EDIT tab: Copy to, Clear, Set as startup bank, Name. Each is one
      Ctrl+Z.
- [ ] Click PLAY **B**: it blinks until the next bar, then lights; the first B note is on the
      bar. Click PLAY **C** and then **Cancel** before the bar: B keeps playing.
- [ ] Queue one bank, then another before the bar: only the second lands.

## Cues

- [ ] Perform panel Cues card: click **Main**: it blinks, lands on the next bar (both
      sequencers together), lights. Try the controller buttons too.
- [ ] Press **Variation**, then **Breakdown** quickly: Breakdown replaces it. **Cancel**
      drops it.
- [ ] Stop (CC 46 or the Transport card), launch a cue: nothing sounds, the cue shows queued;
      Run starts it from its first step.
- [ ] Restart with a cue queued: it lands on the Restart.
- [ ] Select the Cues module: the drawer edits names, clock, timing (Now / Next step / Next
      bar) and a bank or Keep per sequencer; **Capture** takes what plays now; Ctrl+Z undoes
      each edit. Undo never re-launches anything.
- [ ] Delete a sequencer: its cue entries go; Ctrl+Z brings both back.

## Motion

- [ ] The two LFOs' tags say `4bar · synced` / `8bar · synced` a few seconds after start.
      Stop the clock: `held`, the filters keep moving at the same speed. Run: synced again,
      no jump.
- [ ] Energy, Motion, Space, Glow feel useful over their whole travel (Motion 0 = still
      filters, Space 1 = a big room without washing the bass out).
- [ ] A macro's routes show as rings/lanes on their destinations, separate from the base
      value; a knob drag and a CC turn are one undo step each; CC pickup works.

## Variation

- [ ] Arp direction card: FWD / REV / PEND (pendulum repeats no endpoint).
- [ ] Expand a sequencer (+N): velocity row with Gate length, probability row with
      Transpose, selectors Length / Gate / Direction / Startup. Lower a step's Prob: that
      step sometimes rests, in time.

## Recording, sound, stability

- [ ] Record a few minutes across cue changes: effect tails carry through every launch; the
      take is complete (no -INCOMPLETE).
- [ ] Status bar: 0 late, 0 xruns during all of the above.
