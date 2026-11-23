# Changelog

## 1.0.0

First stable release of kabl: a modular synthesizer that runs as a CLAP plugin and as the
standalone `kabl-ui` app.

### Engine and modules
- Real-time-safe engine with engine-wide flush-to-zero, a compile pipeline split into stages, and
  no audio-path panics.
- Oscillators, FM voice, filters, envelopes, sequencers (32 steps per pattern, with pages), drums,
  vocoder, delay, reverb, compressor, limiter, mixer, macros, cues, MIDI and audio in.
- Composite modules, functional cables (step patterns, pass probability, A/B morph, glide, all
  locked to the patch clock) and macros that drive cable parameters.
- The vocoder is intelligible on a real voice.
- The VA oscillator's triangle is band-limited (polyBLAMP), so it aliases much less at high notes, also
  with hard sync. Patches that use the triangle sound slightly cleaner.

### Code modules and the creator SDK
- Write your own modules in Rust (or C) with `kabl-module-sdk` and `cargo kabl` (new, build,
  validate, test, pack).
- Modules ship as `.kablmod` files; drop one on the window to install it.
- Code runs as sandboxed WebAssembly. A fuel budget is measured against the real sample rate, shared
  per module and in total; a module that fails or overruns fades out alone instead of crashing the
  synth. Songs embed the code they use, so they open anywhere.
- A code module that declares more ports than the engine supports (4 inputs, 8 outputs, a stereo port
  counting as 2) is refused when it is registered, with a clear error instead of a confusing failure
  at compile time.

### Living-rack UI (look B)
- Rack faces with live displays (filter curves, mixer peak meters, delay flash, travelling gate
  light) and cables that glow from the live signal.
- Knob feel: hover value, focus ring, double-click reset, arrow keys. Transitions for the drawer,
  views and toasts; reduced-motion and system-theme support.
- Cue, pattern and wavetable editors, cable route drawer, file browser, compare panel, and a
  first-run start patch with a four-step tour.
- Switchable cable palettes.

### Perform and Sounds
- Perform view in look B with a teal accent.
- In Perform, only the scene you launched shows Playing (or Queued while it waits for the bar).
- Sounds browser with search and an audible preview at matched loudness, in the app and in the
  plugin.
- Patches are single `.kabl` files; your own sounds live in `~/.local/share/kabl/sounds` and are
  never touched by an upgrade.

### Factory bank
- More than 100 factory sounds: bass, lead, keys, pad, pluck and groove sounds; vocoder and voice
  sounds; FX (bowls, drones, risers, impacts); and Piece presets, self-playing pieces with scenes
  and cues. Each sound is played from a few named macros, checked for clicks, limiter pinning and
  NaN.

### Accessibility
- Screen-reader support through AccessKit in the standalone, deliberate text options, focus rings,
  and keyboard control of knobs.

### Packaging
- Linux tarball with installer script (`install.sh`), desktop entry and icons, third-party notices.

### Notes
- The saved-sound format is schema 8 only; older schemas are no longer migrated.
- The code-module ABI is versioned on its own (ABI v0, preview rules) and the SDK crate keeps its own
  version (0.1.0).
