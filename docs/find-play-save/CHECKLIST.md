# D01 — hands-on checklist (Kosta)

Leave boxes unchecked until you have done them; note pass/fail and anything odd. Evidence and
rules: [`README.md`](README.md). Composition + Motion and Sound Palette reviews are still
separately pending.

## Install and launch (laptop, outside the source tree)

    cd ~/kabl && git fetch && git checkout <submitted commit>   # see REPORT.md
    packaging/linux/package.sh
    mkdir -p ~/opt && tar -xzf target/dist/kabl-0.1.0-linux-x86_64.tar.gz -C ~/opt
    cd ~ && ~/opt/kabl-0.1.0-linux-x86_64/bin/kabl-ui --rate 48000 --frames 256
    # smaller window: add --size 1280x800 ; controller: add --midi "KL Essential"

Your sounds go to `~/.local/share/kabl/sounds` (favorites/recents: `~/.local/share/kabl/library.json`).

## Checks

- [ ] **First sound, no help (formative, time it from the window appearing):** find a pad or
      a lead and hear it within 60 seconds, without the terminal or this README. Note the
      time and where you hesitated.
- [ ] **Find a category:** use Category or search to find the Bass sounds; say whether
      Keys / Sequence / Seq + Keys made sense.
- [ ] **Browsing is silent:** click through several sounds; nothing plays until Open + Play
      or Start.
- [ ] **Audition:** Evolving Pad → Play a note, Major, −8; Stop mid-note. Every preview ends;
      the help line describes what you heard.
- [ ] **Piece:** Sequence Bass opens silent; Start and Stop behave like the clock buttons.
- [ ] **Keep edits:** change a Perform control on a factory sound → ● appears → Save → Save
      As "…" → open another sound → find yours under Your Sounds (and in Recent) → it
      sounds edited. The factory sound is unchanged.
- [ ] **Unsaved protection:** with an edit, Open another sound / New / close the window:
      Cancel keeps the edit exactly (undo still works); Don't save discards; Save saves then
      continues. Undo back to the saved state removes the ●. The prompts make sense.
- [ ] **Names:** Save As with an existing name offers Replace it; Rename to a taken name is
      refused; favorites survive a rename.
- [ ] **Controller + preview:** hold a key on the controller, Play a chord containing it:
      when the preview ends your held key keeps sounding until you release it; the pedal
      holds preview notes until pedal up; All notes off silences everything; unplug/replug
      the controller during a preview → nothing sticks.
- [ ] **Recoverable error:** copy a sound folder in `~/.local/share/kabl/sounds`, break its
      `log.jsonl` (delete half a line), restart, Open it: the message explains, your
      current sound stays. (Optional: `chmod -w ~/.local/share/kabl/sounds` then Save As.)
- [ ] **Layout:** at 1440×900 and 1280×800, A-light and A-dark: browser, toolbar, Perform
      panel and dialogs readable and reachable.
- [ ] **Existing workflows:** `--patch patches/composition --perform` and
      `--patch patches/sound-palette --perform` still run as before (clocks start running).
- [ ] **Audio health at 48 kHz / 256** during the above: status-bar late/xruns, any "audio
      stalled".

Notes:
