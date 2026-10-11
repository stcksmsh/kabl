# For a lawyer to confirm

This is drafting for the owner's decision, not legal advice. One list.

1. **Authorship of agent-written code.** 133 of 475 commits are by "Claude"; much of the rest was
   directed by the owner. Is that code protected by copyright, who holds it, and does the
   "sole owner, can relicense or sell" claim in `COPYRIGHT` hold for it? Same for the
   machine-composed demo music and the factory patches.
2. **Owner name and jurisdiction.** The exact legal name to print, and whether the owner's
   country allows the licence grant in `CONTRIBUTING.md` (moral rights, patent grant).
3. **Contributor terms.** Wording of the licence grant, sign-off as acceptance, and whether
   assignment is needed instead for a later sale.
4. **GPL section 7 terms.** That terms (b) to (e) in `COPYRIGHT` are within section 7 as
   written, and that the attribution line "Based on kabl by ..." is a "reasonable legal notice or
   author attribution" and enforceable against forks.
5. **Trademark.** Whether an unregistered name is protectable enough; whether to register
   "kabl" (class 9 software) and in which countries; whether another "kabl" already exists.
   Registration is not done.
6. **Logo under "all rights reserved" inside a GPL project.** The build needs the logo files.
   Does that make them part of the Corresponding Source, and is the unmodified-copy permission in
   `ASSET-LICENSES.md` enough? Also: proof that the logo is entirely the owner's work.
7. **Ubuntu Font Licence 1.0** (Ubuntu-Light, embedded by egui `default_fonts`) with
   GPL-3.0. Aggregation or combined work? Options in `AUDIT.md`, P1.
8. **OFL fonts** (Plex, Noto Emoji) embedded in a GPL binary: confirm the OFL "no sale by
   itself" and Reserved Font Name terms are met.
9. **MPL-2.0** (`audio_thread_priority`) in a GPL-3.0 binary: confirm section 3.3.
10. **Relicensing already-received copies.** Anyone who received the code under MIT OR
    Apache-2.0 (PR #22) keeps those rights for that version. Was anything published?
11. **Factory patches and docs under CC BY-SA 4.0** (proposal): fit with "music is the user's, no
    conditions", and whether a rendered sound is an adaptation of a patch.
12. **Apache-2.0 dependencies** need GPL-3.0, not GPL-2.0: confirm the code must never be
    offered under GPL-2.0-only.
13. **GPL-3.0-only versus or-later.** Owner to choose; `Cargo.toml`, `deny.toml`, `COPYRIGHT`,
    `README.md` and `ASSET-LICENSES.md` each say it once.
14. **CLAP and any future VST3 build.** CLAP is MIT. VST3 would need a new check.
15. **Distribution duty.** The Corresponding Source offer in the package README and a release
    page link: enough for GPL-3.0 section 6?
