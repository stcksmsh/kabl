# Licences of non-code material

Source code: GPL-3.0-or-later, see `LICENSE` and `COPYRIGHT`. Dependency licences:
`THIRD-PARTY-NOTICES.md`, generated from `Cargo.lock` and checked in CI. This file covers the
rest. Audit and sources: `docs/licensing/AUDIT.md`.

Copyright of all kabl-made material below is held by the owner named in `COPYRIGHT`. Music made
with kabl, including with the factory patches, belongs to the person who made it. No licence
here puts a condition on that audio. The conditions below apply to the patch files, wavetables,
documents and artwork themselves, and to copies and adaptations of them.

| Material | Where | Licence |
|---|---|---|
| Name "kabl" and the logo: `kabl-logo.svg`, `kabl-mark.svg`, `kabl-logo-mask.png`, `kabl-lockup.svg` and the icon PNGs/SVG derived from them | `crates/ui/assets/logo/`, `packaging/linux/logo/` | **All rights reserved.** Not covered by GPL-3.0. Kabl's owner permits only: copying the files unchanged together with the kabl source or an unmodified kabl binary, and fair reference to kabl. A fork must replace them. See `TRADEMARKS.md`. |
| Factory patches | `patches/` | CC BY-SA 4.0 (proposal, owner to confirm). Attribution line: "kabl factory patches by Kosta Vukićević". |
| Wavetables `saw-square`, `harmonic-sweep`, `vowels`, `glass-bell`, `digital-hollow` | `crates/modules/assets/wavetables/` | computed by `build.py`; same licence as the code (GPL-3.0-or-later) |
| Wavetables `epiano`, `organ`, `choir` | same | derived from Adventure Kid Waveforms by Kristoffer Ekstrand, CC0 1.0 (`PROVENANCE.md`); the derived files are offered under the code licence, the CC0 source stays CC0 |
| IBM Plex Sans, Sans Condensed (SemiBold, G1b mock) and Mono | `crates/ui/assets/*.ttf`, `crates/ui/examples/app_target/fonts/` | SIL OFL 1.1, Copyright 2017-2018 IBM Corp., Reserved Font Name "Plex"; text in `crates/ui/assets/FONT-LICENSE.txt` |
| egui default fonts (Ubuntu-Light, Hack, Noto Emoji, emoji-icon-font) | **not shipped** | The `default_fonts` feature of egui and eframe is off (`crates/ui/Cargo.toml`, `crates/clap/Cargo.toml`), so `epaint_default_fonts` and the Ubuntu Font Licence are no longer in the build. Every glyph the interface draws comes from IBM Plex; `theme_tests.rs` fails if a character in the source has no glyph there. Text a user types in a script outside Plex's Latin, Greek and Cyrillic coverage shows as a missing-glyph box. |
| Documentation text, design mockups, screenshots, clips, demo audio and video | `docs/` | CC BY-SA 4.0 (proposal, owner to confirm), except `docs/promo-demo` audio and video, which are all rights reserved until the owner decides |
| Voice recording `voice-espeak-16k.wav` (the vocoder demo input and the level-test voice) | `docs/vocoder/audio/` | made for this project by running espeak-ng 1.51 (en-us voice) on text written for it, not taken from any recording; offered under the same licence as the other documentation audio |
| Scripts, tests, fixtures, REAPER projects | everywhere | GPL-3.0-or-later |

Nothing in the repository is taken from Surge XT, Vital or any other synthesizer's data.

Items marked "proposal" are not final.
