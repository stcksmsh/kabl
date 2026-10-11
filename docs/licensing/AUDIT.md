# Licensing audit (2026-10-11)

Drafting for a decision, not legal advice. Facts were checked in the repository at `origin/master` 1d3eaf4.

## 1. Dependencies

Method: `cargo deny list` and `cargo deny check licenses` against `Cargo.lock`, target `x86_64-unknown-linux-gnu`, dev dependencies excluded (config: `deny.toml`). Per-crate matrix: `docs/licensing/deps-by-crate.tsv` (244 crates, one column per licence).

Result: every crate carries at least one licence on the allow-list. Nothing is GPL-incompatible on its own terms.

| Licence found | Compatible with GPL-3.0 | Note |
|---|---|---|
| MIT, BSD-2/3, ISC, Zlib, 0BSD, Unlicense, CC0-1.0, BlueOak-1.0.0, Unicode-3.0 | yes | permissive |
| Apache-2.0, Apache-2.0 WITH LLVM-exception | yes with GPL-3.0 (not with GPL-2.0-only) | this is why the code licence must be GPL-3.0, not GPL-2.0 |
| MPL-2.0 (`audio_thread_priority`) | yes (MPL-2.0 section 3.3 allows GPL-3.0 as Secondary License) | its files stay MPL-2.0 |
| `self_cell`: `Apache-2.0 OR GPL-2.0-only` | yes, take Apache-2.0 | |
| OFL-1.1, Ubuntu-font-1.0 (`epaint_default_fonts`) | **open**, see problem P1 | |

Vendored `vendor/nice-plug` (fork of NIH-plug): ISC, copyright Robbert van der Helm (2022-2024, 2023 for nice-log), licence texts kept in `vendor/nice-plug/LICENSE` and `crates/nice-log/LICENSE`. kabl builds it with `default-features = false` and `strict-clap`, so the VST3 bindings (`vst3` crate) are not compiled. No file in `vendor/` or `crates/` has a copyright or SPDX header. `egui-baseview` (git dependency, codeberg) is in the lockfile; its licence passes the check.

## 2. Assets

| Asset | Path | Origin | Licence | Provable |
|---|---|---|---|---|
| IBM Plex Sans/Mono (5 TTF) | `crates/ui/assets/` | IBM Plex, github.com/IBM/plex | OFL-1.1, Reserved Font Name "Plex" | yes: `FONT-LICENSE.txt` (Debian copyright-format file) |
| IBM Plex incl. Condensed (7 TTF) | `crates/ui/examples/app_target/fonts/` | same | OFL-1.1 | text only in the other directory; example only, not shipped |
| egui default fonts: Ubuntu-Light, Hack, NotoEmoji, emoji-icon-font | inside `epaint_default_fonts` | upstream crate | UFL-1.0, Hack/Bitstream Vera terms, OFL-1.1, MIT | yes: texts ship in the crate; feature `default_fonts` is on (`crates/ui/Cargo.toml:15`), so they are embedded |
| Wavetables `saw-square`, `harmonic-sweep`, `vowels`, `glass-bell`, `digital-hollow` | `crates/modules/assets/wavetables/` | computed by `build.py` | project licence | yes: script in tree |
| Wavetables `epiano`, `organ`, `choir` | same | Adventure Kid Waveforms, Kristoffer Ekstrand, mirror commit 8de90bf | CC0 1.0 | yes: `PROVENANCE.md` records the checks of 2026-10-09 |
| Logo, mark, mask (`kabl-logo.svg`, `kabl-mark.svg`, `kabl-logo-mask.png`), icon PNGs, `kabl-lockup.svg` | `crates/ui/assets/logo/`, `packaging/linux/logo/` | README says "The artwork is Kosta's" | none declared | by his statement only; to reserve (see P3) |
| Factory patches (14 directories) | `patches/` | built with kabl by agents under Kosta's direction | none declared | the files hold no author field |
| Demo and promo media: `docs/promo-demo/*.mp4`, `*.mp3`; `docs/*/audio/*` (wav, mp3, flac, m4a); screenshots (png, webp), clips (mp4) | `docs/` | rendered or recorded from kabl by scripts in the tree | none declared | scripts in tree; the music is machine-composed |
| Design mockups | `docs/design/` | procedural SVG/PNG from `render.py`; `IMAGEGEN_PROMPTS.md` says no image model was used | none declared | no raster file from an image model is in the tree (`docs/design/art/` does not exist) |
| Test fixtures, benches, REAPER projects, Lua scripts | `crates/*/tests`, `docs/**` | written in-project | project licence | yes |

No third-party sample, photo, stock image or sound was found. Search covered docs for "downloaded", "CC-BY", "freesound", "stock", "royalty": the only hits are discussions, not material.

## 3. Plugin format, packaging, release

- CLAP: the headers and spec are MIT (`clap-sys` crate, MIT). No SDK licence terms apply to a CLAP plugin.
- VST3 / Steinberg SDK: not built, not linked. If VST3 is added later, the `vst3` crate is GPL-3.0 and Steinberg's own terms need a fresh check.
- Package (`packaging/linux/package.sh`) ships: binary, CLAP library, desktop file, icons (`packaging/linux/logo/hicolor`), `patches/`, `LICENSE-MIT`, `LICENSE-APACHE`, `THIRD-PARTY-NOTICES.md`, `ASSET-LICENSES.md`. It does not ship the `OFL.txt` or `UFL.txt` text files on their own; `THIRD-PARTY-NOTICES.md` carries them.
- `release.yml` uploads only the tarball and `SHA256SUMS`. No third-party download at release time. CI installs `clap-validator` (test tool, not shipped).
- Binary is Rust, statically linked (`lto`, `panic = "abort"`). GPL-3.0 therefore covers the whole executable; the CLAP library and the standalone are both distributed binaries, so source must be offered with each.

## 4. Derived code

No source file carries a copied-code notice. Attributions found in comments:

| File | Source | Obligation |
|---|---|---|
| `crates/modules/src/builtins/reverb.rs` | Jon Dattorro, "Effect Design" (JAES 1997), table 2; says "written from the paper; no code was copied" | none (an algorithm, not code). Delay-length constants are the paper's published numbers. |
| `crates/modules/src/builtins/noise.rs` | Paul Kellet "refined" pink filter, musicdsp.org, 2000, public domain | none; coefficients are published numbers |
| `crates/modules/src/dsp.rs`, `osc_va.rs` | PolyBLEP (Valimaki et al.), TPT state-variable filter (Zavalishin, *The Art of VA Filter Design*) | none: published methods |
| `crates/modules/src/builtins/filter_ladder.rs` | Moog ladder topology (US patent 3,475,623, 1969, expired), Stilson & Smith, Zavalishin | none; the patent is expired |
| `crates/modules/src/builtins/drive.rs` | ADAA (Parker, Zavalishin, Le Bivic; Bilbao et al.) | none |
| `crates/ui/examples/rev2_proto/draw.rs:50` | "Palettes copied from revision-2 `render2.py`" | in-project |

I did not run a similarity search against other code bases. The attributions rest on what the code comments say; a lawyer or Kosta should treat "written from the paper" as unverified.

## Problems

**P1. Ubuntu Font Licence 1.0 (Ubuntu-Light, embedded as an egui fallback font).** UFL has a rename-on-modify rule and its own copyleft-like terms. I could not establish that it is GPL-3.0 compatible. It is a font data file aggregated in the binary, not linked code, and the same combination is common, but this is a point for a lawyer. Options: (a) keep and list it, accept the residual risk; (b) turn off egui `default_fonts` and ship only Plex (product code change in `crates/ui`, not mine to make; Ubuntu-Light, Hack, Noto Emoji lose their fallback role, emoji glyphs may disappear); (c) replace the fallback with an OFL-only font. I changed nothing.

**P2. The AI-authored code.** `git log` shows 133 commits by "Claude" and 342 by Kosta's accounts. Whether code written by agents is copyright-protected, and who holds it, is unsettled. This affects "Kosta is sole owner" and any later sale. Lawyer.

**P3. Logo and name.** Ownership rests on one README sentence. A signed statement or dated original file would help for a trademark filing. Nothing in the tree proves the logo is free of third-party elements (for example, a font outline inside the wordmark).

**P4. Existing release contents.** Any tarball or tag already published under `MIT OR Apache-2.0` stays under it for those who received it. The repository has no git tags; check on GitHub whether a release was published from #22, and whether the repository is public. Lawyer.

**P5. Audio/video of the demo.** Machine-composed music: same authorship question as P2. Kosta chose it; no third-party content found.
