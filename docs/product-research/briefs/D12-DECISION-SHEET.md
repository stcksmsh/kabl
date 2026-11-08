# D12 decision sheet for Kosta

Date: 2026-11-08. Companion to `D12.md` (the plan). Planning only; nothing here is built. Answer by number on the `Kosta:` lines. Claims were checked against master b772a76e or merged PRs; guesses are marked `[unverified]`.

**Words used.** *Module* = a small program (a `.wasm` file) a creator writes; kabl runs it in a sandbox. *ABI* = the exact list of functions and memory layout a module and kabl agree on; *ABI major* changes break old modules, *minor* only adds optional things. *Saved settings* = what a module remembers in a song (knob positions are separate; this is its inner memory such as filter history or a sequence). *Package* = one `.kablmod` file for sharing a module. *Fuel* = wasmi's instruction counter, how kabl limits a module's time per block.

**Running example.** Ana ships `ana.chorus` v1. v2 adds one knob. v3 renames a knob. Bob has songs saved with each.

## Summary

| # | Question | Recommend | Reversible? | Blocks |
|---|---|---|---|---|
| 1 | When to freeze the ABI | Freeze v1 after D11 PR 6 + one trial run | No | PR 0 |
| 2 | Metadata format | Text in the wasm, JSON only in the package | Partly | PR 0 |
| 3 | SDK licence | MIT/Apache SDK, 0BSD template, lawyer check | Mostly no | PR 1 |
| 4 | Package format and location | `.kablmod` zip, user data dir | Partly | PR 4 |
| 5 | Trust and signing | Local only, hash, notice | Yes | PR 4 |
| 6 | Languages | Rust only, plus a 1-page C note | Yes | PR 1 |
| 7 | Settings migration | Module-exported `migrate`, safe fallback | No (the export), yes (policy) | PR 0 |
| 8 | Capabilities | Reserve an empty list | Yes | PR 0 |
| 9 | Denormals | Docs only | Yes | none |
| 10 | Polyphony claim | Validator tests it | Yes | PR 3 |
| 11 | Where the SDK lives | `sdk/` in this repo | Yes | PR 1 |
| 12 | Creator cost figure | Estimate, labelled | Yes | PR 3 |
| 13 (A) | Publish to crates.io? | Git-only until 1.0 | No once published | PR 1 |
| 14 (B) | Module id namespace | Convention + reserve `kabl.`/`demo.` | Mostly no | PR 0 |
| 15 (C) | Allocator in modules | Optional bump arena | Yes | PR 1 |
| 16 (D) | Old v0 blobs after freeze | Refuse with a rebuild message | Yes | PR 0 |
| 17 (E) | Beta scope | PRs 0-4 and 6 | Yes | plan |
| 18 (F) | Trial-run thresholds | 45 min, 5 defects, fixed prompt | Yes | PR 6 |
| 19 (G) | Reproducible builds | Same-machine required, cross-machine best effort | Yes | PR 2 |

## 1. When to freeze the ABI

**Question.** Do we freeze the module interface as v1 now, ship a beta that may still change, or wait?
**What it means.** A frozen ABI is a promise: a module Ana builds today loads in every kabl 1.x. Her saved songs and Bob's embedded copies keep working. Breaking it means every creator rebuilds.
**What v1 adds over v0 (verified against `abi.rs`, `wasmi_engine.rs`):** (exports) an ABI marker, `state_save`, `state_load`, optional `migrate(old_version, bytes)`; (meta keys) `abi`, `help`, `group`, `skin`, `panel`, `x-` ignorable keys, `max_instances`, `polyphony`. v0 has none of these; it has `init/process/reset/memory` and no ABI number.
**Options.**
- (a) Freeze v1 after D11 PR 6 and one trial run. Cost: PR 0 (~250 lines). Creator: stable target. User: songs keep working. Rules out: casual changes.
- (b) Beta, "may change", one deprecation window. Cost: the same plus compat shims later. Creator: rebuild risk. User: possible refusals. Rules out nothing.
- (c) Wait for REAPER and real modules. Cost: delays the SDK; nobody can build. Rules out the beta stopping point.
**Likely reasons for a v2 within a year,** and whether additive: tempo/transport input (additive: new optional import or meta key, a minor); events/MIDI (additive if delivered through a new optional input region; major only if the per-block layout must change); variable block size (**major risk**: `Layout.max_block` is fixed at `init`; allowing change of `n_frames` within the max already works, so only raising the max would break); multiple outputs per port (additive: new port types in meta). So the likely majors are few; freeze the layout words generously (reserve spare words in the `init` record, `[unverified]` that nine is the right count to grow).
**Deprecation window.** I propose "one prior ABI major stays loadable for twelve months or two kabl minor releases, whichever is longer". Calendar vs releases is yours to set.
**Freeze too early vs too late.** Too early: a wrong layout is locked in and v2 costs every creator a rebuild. Too late: no creator can start, and there is no real feedback.
**Reversible?** No. After beta, shipped modules and songs depend on it; only adding a major later is possible.
**Depends on.** D11 PR 6 (so the fault behaviour is final), decisions 2, 7, 8.
**Recommendation.** (a): the trial run (decision 18) on v0 first finds layout mistakes cheaply. Would change if: the trial run needs a layout change after PR 1 (then re-run once, still freeze before beta).
Kosta: ____

## 2. Metadata format

**Question.** Keep today's text metadata inside the wasm, or switch to JSON/CBOR?
**What it means.** The module carries a description of itself. Today (`demo.logic`, abbreviated):
```
kind demo.logic
version 1
rate global
port in cv a 1
port out gate trig 1
param threshold -1 1 0.5 linear 5 cv
```
The package would also hold a `manifest.json` repeating what is derived, plus package-only fields:
```
{ "id": "demo.logic", "version": 1, "abi": "1.0", "sha256": "…", "licence": "GPL-3.0-or-later",
  "ports": [{"dir":"in","type":"cv","name":"a","width":1}], "params": [{"name":"threshold","min":-1,"max":1,"default":0.5,"taper":"linear","smoothing_ms":5,"unit":"cv"}] }
```
A creator using the Rust builder edits neither by hand; the builder writes the text, `cargo kabl pack` writes the JSON. Only the manifest's licence/description/changelog are typed by the creator.
**Options.** (a) text only: no new parser (PR 0 stays small), one format; JSON-less tools must parse text. (b) JSON/CBOR in the wasm: a new host parser (~200 lines), nicer for third-party tooling, a second format to keep compatible. (c) text in wasm + JSON in the package: (a) plus ~80 lines in PR 4.
**Reversible?** Partly: the text keys are part of the ABI; the manifest can evolve. Today the parser rejects unknown keys, so PR 0 must add an `x-` rule first or even an additive change breaks old hosts.
**Depends on.** 1. **Recommendation.** (c). Would change if third-party tools (editors, stores) want JSON from the wasm itself.
Kosta: ____

## 3. SDK licence

**Question.** Under which licence do we publish the SDK crate, the template and the test harness?
**What it means.** A creator's finished wasm contains compiled SDK code (the macro and helpers). The SDK's licence therefore may attach to every shared module. kabl itself is GPL-3.0-or-later.
**Options, in plain terms.**
| | MIT/Apache-2.0 SDK | GPL-3.0 SDK | LGPL-style or GPL + exception |
|---|---|---|---|
| Sell a closed-source module | Yes | No: source must be offered | Yes if linked per the exception |
| Keep the DSP private | Yes | No | Yes |
| Fork the SDK | Yes | Yes, stays GPL | Yes, stays under the same licence |
| A company ships modules | Yes | Only if it opens them | Yes |
| Cost | none | a docs line | needs drafting/review |
**Does kabl's GPL reach a module that only talks through the ABI?** For "no": the module is a separate program in a sandbox, data crosses a narrow interface, FSF's FAQ treats that kind of separation as "aggregate"/pipes when communication is not intimate `[unverified exact FAQ wording]`. For "yes": it is loaded into kabl's address space, runs inside kabl's process, shares memory copies, and kabl specifies and calls the interface; courts and FSF have argued dynamic loading can create a combined work. No certainty either way.
**Paragraph to forward to a lawyer.** "kabl is a GPL-3.0-or-later audio application. It loads third-party WebAssembly modules into a wasmi interpreter inside its process, with no imports and a published binary interface. The modules are built from an SDK crate and template we intend to publish. We want creators to be able to distribute modules under any licence, including closed source and commercial. (1) Does kabl's GPL apply to such a module? (2) May we publish the SDK crate under MIT/Apache-2.0 and the template under 0BSD/CC0 while the host stays GPL? (3) May a separately distributed test harness that statically links the GPL host code be used by creators as a dev-dependency without affecting their module's licence? (4) Do we need an explicit exception statement for modules in the host's licence?"
**Template.** It is copied into the creator's repo. 0BSD or CC0 for the template means no notice burden. Recommend 0BSD (CC0 is not OSI-approved and is awkward in some jurisdictions `[unverified]`); `deny.toml` already allows both.
**`kabl-module-test` split.** It wraps GPL code. As a dev-dependency, running tests locally is not distribution, but if a creator ships a repo or CI container with it, the harness itself is GPL and creators will notice. Options: (a) keep it GPL, documented (zero cost); (b) split the engine-independent checks (metadata, layout, sandbox rules) into a permissive crate and keep only the real-engine render in the GPL harness (~+150 lines in PR 3); (c) license the harness under the same exception as the host.
**Comparable hosts.** CLAP is MIT; LV2 is ISC `[unverified, from memory]`. VCV Rack's plugin SDK is GPL-3.0 with a statement on plugin licensing `[unverified]`; Faust: compiler GPL, with an exception allowing generated code under any licence `[unverified]`. Bitwig Grid: nothing public found `[unverified]`. Verify before relying on any of this.
**Reversible?** Mostly no: published versions stay under their licence; a later relicense only affects new versions, and every contributor must agree.
**Depends on.** Lawyer, 13. **Recommendation.** MIT OR Apache-2.0 for crate, 0BSD template, test harness (b); ask the lawyer once before the beta. Would change if the lawyer says the host needs an exception for permissive modules.
Kosta: ____

## 4. Package format and location

**Question.** Is a `.kablmod` zip the shareable unit, and where does an installed one live?
**What it means.** Ana sends Bob one file. He installs it; it appears in his module browser. Songs always carry their own copy of the code, so the install only matters for new songs.
**Answers to the open points.**
- Same `id` and `version` with different hashes: the install is refused with "already installed with different content (hash a1b2…); remove it first". Songs are unaffected because each song embeds its exact blob by hash. [proposal, not built]
- Same `id` from two authors: no authority exists, so the same refusal applies; decision 14 sets the naming convention.
- Install directory: Linux `$XDG_DATA_HOME/kabl/modules` (kabl's existing user dir is `$XDG_DATA_HOME/kabl`, `library.rs:281`); Windows `%APPDATA%\kabl\modules` and macOS `~/Library/Application Support/kabl/modules` `[unverified: no existing code]`. REAPER/CLAP: the plugin runs in REAPER's process and reads the same per-user dir with `KABL_USER_DIR` override already supported; read-only, scan at start, on the control thread. Sandboxed hosts (Flatpak, macOS app sandbox) may block it: `[unverified]`, test in PR 4.
- Uninstall: a song using the removed module still opens (code travels inside the `.kabl`; `embedded.rs` stores blobs by hash). The browser stops offering it for new patches; an existing instance shows normally. Confirmed in source for the open path; the browser text is new.
- Size cap: wasm ≤ 2 MiB already; two PNGs ≤ 128 KiB each (`panel.rs`); so 4 MiB is ample.
**Options.** (a) `.kablmod` zip (~500 lines in PR 4). (b) bare `.wasm` with meta inside, no wrapper (~200 lines): no README/licence/art, nothing to show before loading. (c) folders in the user dir only (~150 lines): no sharing unit.
**Reversible?** Partly: file extension and manifest can gain fields; a different container later means supporting both.
**Depends on.** 2, 5, 14. **Recommendation.** (a).
Kosta: ____

## 5. Trust and signing

**Question.** Is "hash + one-time notice" enough for D12, or do we add signatures?
**What a user sees today** (`crates/ui/src/code_notice.rs`): opening a `.kabl` that contains code shows "This patch contains custom code modules", lists each module id, version and short hash, and says they run in a sandbox without file or network access; Open runs them, Cancel opens the patch with the code off. The answer is remembered per exact hash; a changed hash asks again. For an installed package the same notice would appear the first time its module is used in a patch (proposal; not built).
**Options.** (a) no signing. (b) optional detached signature and author list. (c) mandatory.
**Could (b) be added later without breaking (a)?** Yes: an absent signature stays "unsigned, ask once"; a signature file inside the package only upgrades the message. So choosing (a) now is safe.
**Reversible?** Yes. **Depends on.** 4. **Recommendation.** (a). Would change if modules start being distributed anonymously at scale (a store).
Kosta: ____

## 6. Languages

**Question.** Support only Rust, or ship templates for other languages?
**What it means.** The ABI is just wasm exports and a memory layout, so C or Zig work in principle. A creator in C must: export four functions, place buffers at fixed offsets, write a meta custom section by linker attribute or post-link tool, avoid imports (wasi-libc pulls them in), and keep their own `migrate`. Roughly 150-250 lines of boilerplate `[unverified]`.
**Options.** (a) Rust only. (b) Rust + a 1-page "other languages" note and one working C example (~120 lines of docs + ~100 lines C, and a CI build to prove it keeps working; ~+250 total). (c) full second template (~+600, and a second fresh-agent run).
**Reversible?** Yes. **Depends on.** 1. **Recommendation.** (b): cheap, proves the ABI is language-neutral, and does not promise support. Would change if the C CI proves costly: fall back to (a).
Kosta: ____

## 7. Settings migration (`migrate`)

**Question.** When a module's version changes, can the module convert its own saved settings?
**Walk-through.** v1 → v2 (Ana adds a knob): Bob opens a v1 song, his install has v2 only if he installed it, otherwise the song's embedded v1 blob runs unchanged. If the song's blob is replaced by v2 (he chooses "update"), v2's `migrate(1, bytes)` returns new bytes with the new knob at its default; with no `migrate`, settings reset to defaults and the old bytes are kept inside the patch (D11 decision 7). v2 → v3 (rename): the knob's old value is lost unless `migrate` maps it; the saved patch keeps the v2 bytes. v3 → v1 downgrade (Bob has v1 installed, opens a v3 song): the song carries the v3 blob, so it still plays v3; if the user refuses the notice, the module is bypassed with "needs ana.chorus v3". There is no downward `migrate`; the kept v3 bytes survive so nothing is lost.
**Options.** (a) module-exported `migrate` (PR 0). (b) declarative field mapping in meta (more host code, ~+300). (c) defaults only.
**Reversible?** The export is part of ABI v1, so adding later is a minor (optional export), but dropping it is a major; the *policy* (when the host calls it) can change. **Depends on.** 1. **Recommendation.** (a) with: trap/garbage from `migrate` counts as "no migrate" (fall back to defaults + kept bytes), never a crash.
Kosta: ____

## 8. Capabilities

**Question.** Does v1 reserve a list of host abilities a module may request?
**What it means.** First likely candidates: tempo/transport (additive: new optional input region or import, minor), seeded random source (additive: optional import `kabl.random`, minor), sample-accurate events (additive if delivered through a new optional input; if it changes `process`'s signature, major). With the empty list in the manifest, adding any of these later is a minor.
**Options.** (a) empty list reserved (costs ~20 lines). (b) nothing reserved: first capability becomes a manifest change (~still minor, but old hosts cannot detect "needs capability X" and would load and misbehave). (c) design three capabilities now (~+400, speculative).
**Reversible?** Yes. **Depends on.** 1. **Recommendation.** (a). "None now" costs nothing later.
Kosta: ____

## 9. Denormals (very small floats)

**Question.** Does the SDK need to do anything about slow tiny numbers?
**What it means.** #128 is merged: `FtzGuard` sets flush-to-zero at the top of each callback (standalone, UI, CLAP) and wasmi runs on that thread. D11 measured 12.2 us per block per voice with the flag off vs 0.93 us on. So this shrinks to docs.
**Options.** (a) docs only. (b) plus a validator denormal flood check and an optional `flush()` helper (~+80 lines).
**Reversible?** Yes. **Depends on.** none (#128). **Recommendation.** (a). The offline test harness must create its own guard.
Kosta: ____

## 10. Polyphony claim

**Question.** Must a module declare how many voices it supports, and does the validator test it?
**What it means.** `rate voice` modules get one instance per voice (decision 3 of D11). The validator would run 16 voices for 1000 blocks of 64 and require finite, identical-per-voice output. For a `global` module it checks one instance instead and rejects a polyphony claim. Runtime: at ~1 us per block per voice (D11 REPORT) this is about 16 ms of DSP plus load time; a few seconds with build cache `[unverified]`.
**Options.** (a) today's rule (voice vs global) only. (b) add a tested `polyphony` claim (~+60 lines).
**Reversible?** Yes. **Depends on.** none. **Recommendation.** (b): it is part of the acceptance sentence.
Kosta: ____

## 11. Where the SDK lives

**Question.** `sdk/` in this repo, or elsewhere?
**What it means.** Today `crates/modules/code-examples/` is a separate cargo workspace (own lockfile; `build.sh` copies two blobs into `crates/modules/src/code/examples/`). PR 1 would put the SDK in a new top-level `sdk/` workspace with its own LICENSE and `deny.toml` section, and port the two examples to use it (they stay under `code-examples/` or move to `sdk/examples/`; I propose move). A separate workspace does not enter the `cargo deny` run (`exclude-dev`, root workspace) nor `packaging/gen-notices.py`, which reads the root `cargo metadata`, so notices stay correct; the SDK needs its own `deny` run in CI and the repo's root LICENSE/COPYRIGHT must say `sdk/` has a different licence (`[unverified]` whether the packaging scripts copy only root licences).
**Options.** (a) `sdk/` in-repo. (b) separate repo now. (c) in-repo, split after the trial run.
**Reversible?** Yes (a git history split). **Depends on.** 3. **Recommendation.** (c).
Kosta: ____

## 12. Creator-facing cost figure

**Question.** Does the SDK report how much of the audio time a module uses, and with what promise?
**Worked example** (estimate, `[unverified]` until PR 3 measures): D11's kernel (one biquad plus feedback loop) used 2,933 fuel per 64-sample block and about 1.03 us, 0.08 % of a 1.333 ms block at 48 kHz. `demo.bitfilter` does four biquads (two per channel) plus soft-clip, roughly 4x: about 12,000 fuel. At the default 1,000 fuel/us that reports about 12 us, **0.9 % of the block**; wall-clock would be about 4 us, **0.3 %** (real fuel rate was about 3,000/us). So the figure is conservative by up to 3x.
**Near the 25 % cap.** The report prints a warning above 15 %: reduce work per sample, use smaller tables, process fewer voices, or lower `max_instances`. Over 25 % the module is cut off with silent blocks (D11 decision 5).
**Options.** (a) fuel estimate + offline wall-clock render, labelled "estimate". (b) wall-clock only. (c) none.
**Reversible?** Yes. **Depends on.** PR 3. **Recommendation.** (a).
Kosta: ____

## 13 (A). Publishing to crates.io

**Question.** Do we publish the SDK crate, the CLI and the test crate to crates.io, or keep them in git until 1.0?
**What it means.** `cargo install kabl-sdk-cli` and `kabl-module-sdk = "1"` need crates.io. A published name and version cannot be deleted, only yanked. `cargo search` on 2026-11-08 showed no crate named `kabl-module-sdk`, `cargo-kabl`, `kabl-sdk-cli` or `kabl-module-test` (names look free; confirm at publish). It is also the account owner's responsibility: which account (yours)?
**Options.** (a) git dependencies until 1.0 (`kabl-module-sdk = { git = "…", tag = "…" }`); install by `cargo install --git`. (b) publish now under a project account. (c) reserve names now with empty 0.0.0 crates (against crates.io's name-squatting rules, `[unverified]`).
**Reversible?** No once published (names, versions stay). **Depends on.** 3. **Recommendation.** (a); publish at the beta tag after the licence is cleared.
Kosta: ____

## 14 (B). Module id namespace

**Question.** How do we stop two creators using the same module id?
**What it means.** The patch stores `(id, version, hash)`. With no registry, `ana.chorus` could be claimed by two Anas. A song keeps working (hash), but installs would collide (decision 4).
**Options.** (a) convention: reverse-domain or GitHub handle (`ana.chorus`, `com.example.chorus`) and kabl reserves `kabl.` and `demo.` (host checks). (b) a registry file in the repo (human-reviewed). (c) none.
**Reversible?** Mostly no: ids are baked into songs. Reserving more prefixes later is possible. **Depends on.** 4. **Recommendation.** (a).
Kosta: ____

## 15 (C). Memory in modules

**Question.** Does the SDK offer an allocator, or none?
**What it means.** Porting existing DSP (delay lines, wavetables) needs memory. No allocator means a fixed memory plan decided at compile time: all in `static` arrays declared in the module (as the examples do), simple but sizes are fixed. An optional bump arena allocates once at `init`, never at `process`, matching D11 decision 9 (no allocation on the audio thread); sizes can depend on sample rate (a 2-second delay at 192 kHz).
**Options.** (a) none (smallest SDK). (b) optional arena (~+100 lines). (c) full `alloc` crate + `GlobalAlloc` (a trap risk if used in `process`).
**Reversible?** Yes (additive feature). **Depends on.** none. **Recommendation.** (b).
Kosta: ____

## 16 (D). Old v0 blobs after the freeze

**Question.** What does kabl do with a v0 module in a saved `.kabl`?
**What it means.** v0 modules carry no ABI marker (`REQUIRED_EXPORTS` has no ABI export), and `demo.logic`/`demo.bitfilter` are v0 blobs; any file saved from a current build embeds them. After the freeze there are only test files and the examples, no creators.
**Options.** (a) refuse: a v0 blob is bypassed with "built for an older module format, rebuild or update" (~20 lines). (b) keep a v0 loader (~+150 lines plus a test corpus forever). (c) migrate automatically (impossible: no marker, metadata differences).
**Reversible?** Yes now; no after the beta (then (b) is the cost). **Depends on.** 1. **Recommendation.** (a): rebuild the examples as v1 in PR 1, say so in the release notes.
Kosta: ____

## 17 (E). Beta scope

**Question.** What is the minimum for the "code creator beta" stopping point?
**What it means.** Your order is D11, D12, release hygiene, factory sounds, so D12 length delays 1.0.
**Options (from the 8 PRs in D12.md, lines are estimates).** (a) minimum: PR 0 (250), 1 (600), 2 (450), 3 (400), 4 (500), 6 (250 docs) = **2,450 lines**; PR 5 (panel metadata, 300) and PR 7 (compat corpus CI, 200) wait for 1.0. (b) everything: 2,950. (c) smallest viable: PRs 0-3 + 6 = ~1,950 and share by hand, no package install.
**Reversible?** Yes. **Depends on.** 4, 18. **Recommendation.** (a). Caution: without PR 7 the compat promise has no automatic guard; start the corpus as part of PR 0's tests.
Kosta: ____

## 18 (F). Trial-run thresholds

**Question.** How do we judge "a fresh agent can build a module from the docs"?
**What it means.** D12.md's "≤ 30 min and ≤ 3 doc defects" was my guess, not derived. A better basis: the examples took one session to write against the ABI, so allow 45 minutes and count a *defect* as any place the agent had to consult kabl source or guess. Pass: ≤ 5 defects, module distinct from the examples, all `cargo kabl test` checks green, 16-voice render ok.
**Repeatability.** Same model and effort every time (I propose the current default Sonnet) in an empty directory with only `SDK.md`, the crate and a fixed prompt (kept in the repo); no kabl source tree; the transcript is stored. Run at beta and after each ABI-affecting change.
**Options.** (a) the above. (b) tighter (30 min / 3 defects). (c) one human developer instead.
**Reversible?** Yes. **Depends on.** 17. **Recommendation.** (a).
Kosta: ____

## 19 (G). Reproducible builds

**Question.** Must the same source produce the same bytes on any machine?
**What it means.** The hash in a song identifies the exact code. Byte-identical builds across machines need pinned toolchain, `--remap-path-prefix`, locked deps and the same linker. Then anyone can check a downloaded module against its source. The examples are already reproducible on one machine (`build.sh`).
**Options.** (a) same-machine required, cross-machine best effort and documented. (b) cross-machine required, tested in CI with two runners (~+100 lines in PR 2, brittle across rustc versions). (c) none.
**Reversible?** Yes. **Depends on.** none. **Recommendation.** (a).
Kosta: ____

## If you only answer three

- **PR 0** (freeze and wiring) needs **1** (when to freeze), **2** (metadata and the ignorable-key rule) and **7** (migrate in v1). Also 14 and 16 are small and best settled with them, plus 8 (empty capability list).
- **PR 1** (crate, template) needs **3** (licence), the one with legal weight, then 6, 11, 13 and 15.
- If you want only three: **1, 3, 7**. The recommendations on the rest are reversible.
