# Licences of bundled assets

Source code: `MIT OR Apache-2.0` (`LICENSE-MIT`, `LICENSE-APACHE`). Dependency licences:
`THIRD-PARTY-NOTICES.md` (regenerate with `python3 -I packaging/gen-notices.py > THIRD-PARTY-NOTICES.md`).
This file covers what is not Rust code.

| Asset | Where | Licence | Status |
|---|---|---|---|
| IBM Plex Sans Regular, IBM Plex Mono Regular (UI fonts, embedded in the binary) | `crates/ui/assets/*.ttf` | SIL Open Font License 1.1, Copyright 2017-2018 IBM Corp., Reserved Font Name "Plex". Full text in `crates/ui/assets/FONT-LICENSE.txt`. | established |
| egui default fonts (Ubuntu-Light, Hack, Noto Emoji, emoji-icon-font), embedded via `epaint_default_fonts` | dependency | OFL-1.1, Ubuntu Font Licence 1.0, MIT; texts in `THIRD-PARTY-NOTICES.md` | established |
| Factory wavetables `saw-square`, `harmonic-sweep`, `vowels`, `glass-bell`, `digital-hollow` | `crates/modules/assets/wavetables/` | computed by `build.py`, original work, `MIT OR Apache-2.0` | established (`PROVENANCE.md`) |
| Factory wavetables `epiano`, `organ`, `choir` | same | derived from Adventure Kid Waveforms (AKWF) by Kristoffer Ekstrand, CC0 1.0; checked 2026-10-09, see `PROVENANCE.md` | established |
| Logo and icon (`logo.svg`, `logo_small.svg`; copies in `packaging/linux/logo/`) | `packaging/linux/logo/` | none declared in the repository | **unclear: owner (Kosta) to state** |
| Factory patches | `patches/` | none declared in the repository; the patch files carry no licence or author field | **unclear: owner to state** (presumably the project licence) |
| Documentation text and screenshots | `docs/` | none declared | **unclear: owner to state** |

Until the owner states a licence for the logo, the patches and the documentation, nothing here
grants one beyond what the owner has already published. The release tarball carries the logo
as the application icon and the patches as factory sounds, so this should be settled before 1.0.
