# D12 decision sheet (settled by Kosta, 2026-11-08)

Companion to `D12.md`. Planning only; nothing here is built. Kosta's answers are in `D12-OWNER-INPUT-2026-11-08.md` (kept by the overseer; its content is recorded in `D12-DECISIONS.md`). Every item below shows his decision as the Recommendation, with `Reversible?` and `Depends on`. Facts were checked against origin/master b772a76e; guesses are marked `[unverified]`.

**Words.** *Module* = a small program (`.wasm`) a creator writes; kabl runs it in a sandbox. *ABI* = the list of functions and memory layout a module and kabl agree on. *Saved settings* = a module's inner memory stored in a song (separate from knob positions). *Package* = one `.kablmod` file. *Fuel* = wasmi's instruction counter, kabl's time limit per block. *Running example:* Ana ships `ana.chorus` v1, v2 adds a knob, v3 renames one; Bob has songs saved with each.

**Governing principle (Kosta, 2026-11-08): nothing is set in stone until we ship.** Until kabl 1.0 ships, the ABI, layout, meta format, package format and every other contract may change, including breaking changes; a breaking change before ship rebuilds the examples and saved test files and adds a changelog line. The "additive only" rule and "escalate breaking changes to Kosta" apply from the 1.0 ship onward. This overrides any "stays loadable" or "promise" wording below: before ship it describes the design, not a guarantee.

## Contradictions found, all answered by Kosta (2026-11-08)

1. **Unknown meta keys.** `parse_meta` errors on any unknown key (`wasmi_engine.rs:406`), so "additions only" would break older hosts. **Decided:** the host ignores unknown keys; a module declares `abi_minor N` and the host rejects it if N is greater than the host's minor (covers new required keys); the `cargo kabl` validator warns on unknown keys to catch typos. PR 0 (host), PR 2 (validator).
2. **The layout cannot grow.** `init` returns exactly nine words (`abi.rs:70`) and v0 exports no ABI number (`wasmi_engine.rs:30`). **Decided:** optional export `layout_ext()` returns the address of a length-prefixed table (first word = count, then the extension words); it can grow without another export and an unaware host ignores it. PR 0.
3. **0BSD "no notice" is incomplete** (Rust `core` and `compiler_builtins` are MIT/Apache-2.0 inside every wasm). **Decided:** keep 0BSD. `cargo kabl build` writes the Rust toolchain's MIT notice (complying via the MIT option) into a `kabl.notices` custom section and kabl shows it in module info. The sheet and SDK.md say "notices handled automatically", not "no notice burden". PR 2 (build, ~+60 lines) and a host item to show it in module info (~+80, inside PR 4). `libm` stays dropped.
4. **Authorship.** 133 of 1,071 commits on master are authored as `Claude`; `CONTRIBUTING.md` line 3 is marked draft. **Decided:** finalise `CONTRIBUTING.md` (remove the draft status) and add to `COPYRIGHT`: "Commits authored as 'Claude' are output of the owner's own AI-assisted sessions; any rights in them are held by the owner." Basis: Anthropic's terms assign all rights "if any" in Outputs to the user; purely AI-generated code may be uncopyrightable; either way no third party holds rights, so the module exception needs no one else's consent. No further check of plan terms. **Placement:** these are `COPYRIGHT` and `CONTRIBUTING.md` edits, so they go into PR 0 with the exception paragraph, where Kosta approves the wording once; this planning PR does not touch legal files.

Facts that **confirm** the input: `LICENSE` is GPLv3 (line 2); `deny.toml:1,25` allows `GPL-3.0-or-later` and `0BSD` (line 17); `Cargo.toml:9` licence GPL-3.0-or-later; `COPYRIGHT` already carries "Additional terms under section 7" (attribution, naming, trademark; lines 12-17, items b-e), so the module exception is a permission added beside them; `FtzGuard::new()` is created in `crates/standalone/src/main.rs:143`, `crates/ui/src/main.rs:629`, `crates/clap/src/lib.rs:1157` and wasmi runs on that thread; crate names `kabl`, `kabl-sdk`, `kabl-module-sdk`, `kabl-sdk-cli`, `kabl-module-test`, `cargo-kabl` showed no match in `cargo search` on 2026-11-08 (exact-name availability is confirmed only at publish).

## Summary

| # | Question | Decision | Reversible? | Blocks |
|---|---|---|---|---|
| 1 | ABI freeze | No freeze before 1.0; additions only; host ignores unknown keys, `abi_minor` gate, `layout_ext()` | Yes (freeze later) | PR 0 |
| 2 | Metadata format | Text in wasm, JSON in package | Partly | PR 0 |
| 3 | Licence | GPL + module exception; 0BSD SDK/template/spec; GPL CLI | Mostly no | PR 0/1 |
| 4 | Package | `.kablmod` zip, refuse conflicts | Partly | PR 4 |
| 5 | Trust | Hash + notice; reserve `signature` | Yes | PR 4 |
| 6 | Languages | Rust + 1-page C example | Yes | PR 6 |
| 7 | Migrate | Module `migrate`, safe fallback | Partly | PR 0 |
| 8 | Capabilities | None; reserve field | Yes | PR 4 |
| 9 | Denormals | Docs; CLI harness under FtzGuard | Yes | PR 3 |
| 10 | Polyphony | Validator renders 16 voices | Yes | PR 3 |
| 11 | SDK location | `sdk/` in this repo | Yes | PR 1 |
| 12 | Cost figure | % of block, labelled estimate | Yes | PR 3 |
| 13 A | Publishing | 0.x at beta, Kosta's account | No once published | beta |
| 14 B | Ids | `githubhandle.module`; reserve `kabl.` `demo.` `test.` | Mostly no | PR 2 |
| 15 C | Memory | Opt-in bump arena in SDK | Yes | PR 1 |
| 16 D | v0 blobs | Stay loadable | Yes | PR 0 |
| 17 E | Beta scope | PRs 0-4 and 6 | Yes | plan |
| 18 F | Fresh-agent test | 2 runs, different model, ≤3 defects | Yes | PR 6 |
| 19 G | Reproducible builds | 2 Linux CI builds byte-identical | Yes | PR 2 |

## 1. ABI freeze
**Question.** Freeze the module interface now? **What it means.** A freeze promises a module built today loads in every 1.x. Kosta decided against freezing until 1.0 (and, per the governing principle, nothing is promised before ship). The design kabl ships with is additive: new optional exports, new optional meta keys, never a changed or removed one, so 1.0 can grow without breaking. From the 1.0 ship on, a change that would break this goes to Kosta, not to master; before ship a breaking change just rebuilds the examples and test files with a changelog line.
**Decision.** No freeze; the additive design (ignore unknown keys, `abi_minor` gate, `layout_ext()`) is built now so 1.0 starts from it; the freeze belongs to the 1.0 release work; SDK and docs say "preview". Contradictions 1 and 2 above give what PR 0 adds so "additive" is true. Likely reasons for a later major (all additive under these rules if delivered as optional inputs): tempo/transport, events/MIDI, multiple outputs per port; raising `max_block` needs a new init variant.
**Reversible?** Yes: freezing later is a documentation and CI step. **Depends on.** 2, 7, 16.
Kosta: decided (2026-11-08)

## 2. Metadata format
**What it means.** Text inside the wasm today (`demo.logic`: `kind demo.logic`, `rate global`, `port in cv a 1`, `param threshold -1 1 0.5 linear 5 cv`); the package also holds `manifest.json` (`{"id":"demo.logic","version":1,"licence":"…","sha256":"…","ports":[…]}`). With the Rust builder a creator edits neither by hand (the builder writes the text, `cargo kabl pack` the JSON; only licence, source and changelog are typed).
**Decision.** Text in the wasm, JSON only in the manifest. **Reversible?** Partly: text keys are ABI; the manifest may gain fields. **Depends on.** 1.
Kosta: decided (2026-11-08)

## 3. Licence
**What it means.** The SDK macro ends up in every module binary and modules travel inside every shared song, so the SDK licence decides what creators and companies may do.
| Part | Licence |
|---|---|
| kabl host | GPL-3.0-or-later plus a module exception (GPLv3 section 7 permission) |
| SDK crate, `kabl_module!` macro, meta builder, template, package/manifest spec | 0BSD |
| `cargo kabl` CLI, including the offline test harness | GPL-3.0-or-later |
**Reasoning.** (1) FSF guidance cuts both ways: programs run by a GPL interpreter are "just data", but plug-ins sharing memory with complex data structures are close to dynamic linking. A kabl module runs in wasmi with no imports and shares only float buffers and a parameter array; built from a permissive SDK it contains no kabl code. Likely not covered (the CLAP/LV2/VST position), but doubt keeps companies away. (2) So add a section 7 permission to `COPYRIGHT` (precedents: GCC Runtime Library Exception, Linux syscall note, GNU Classpath exception): *"kabl module exception. As an additional permission under section 7 of the GNU GPL version 3, a module that interacts with kabl only through the documented kabl module ABI (exports, linear memory layout and metadata described in `docs/sdk/ABI.md`) is not considered a work based on kabl, and may be distributed under terms of your choice. This permission does not apply to any portion of kabl itself, including code copied from it into a module."* A fork may remove it (section 7 allows removing additional permissions); that only affects the fork. (3) MIT/Apache need their notice to travel with copies, which is unworkable for code inside every song; 0BSD has no conditions, is GPL-compatible, needs no patent grant because Kosta is the sole human author (the Rust `core` caveat is answered in contradiction 3: notice written to `kabl.notices` automatically). Apache-2.0 WITH LLVM-exception was rejected as heavier text for no gain. (4) No `kabl-module-test` crate: the offline render harness is inside the GPL `cargo kabl` binary, so the creator's crate has **zero GPL dependencies**; using a GPL tool does not license its output. (5) Songs redistribute module code: publishing a `.kablmod` grants permission to redistribute the unmodified wasm inside songs (SDK.md and the first-install message say so); manifest `licence` (SPDX) is required; for copyleft licences (GPL/LGPL/AGPL/MPL) a `source` URL is required and shown in the module's info (GPLv3 6(d) lets the offer be a network location). (6) Closed paid modules are allowed, but the code can be extracted from any song; a "referenced, not embedded" mode is 1.0+ and out of scope. (7) Comparables: VCV Rack is GPL with plugins building against GPL code, so closed freeware needs VCV's non-commercial exception and paid closed plugins a commercial licence (vcvrack.com/manual/PluginLicensing); CLAP (MIT), LV2 (ISC) and Faust generated code terms `[unverified]`.
| Question | Answer |
|---|---|
| Sell a closed-source module? | Yes; the code still travels in every song |
| Keep DSP private? | Source yes; the wasm can be extracted and decompiled |
| Fork the SDK? | Yes (0BSD) |
| Company ships modules? | Yes, with no GPL in its build |
| Notice burden on creators? | Handled automatically (`kabl.notices` section written by `cargo kabl build`) |
**Work added.** `COPYRIGHT` paragraph (Kosta approves final wording in PR 0); `sdk/LICENSE` and template LICENSE (0BSD); `cargo deny` config for `sdk/`; `packaging/gen-notices.py` reads `cargo metadata` of the root workspace only (`gen-notices.py:7-12`), so a separate `sdk/` workspace is not listed in `THIRD-PARTY-NOTICES.md` (CI diffs it, `ci.yml:57-58`); confirm in PR 1 by running it. Manifest `licence`/`source` rules in PR 4.
**Reversible?** Mostly no: published versions keep their licence. **Depends on.** 11, 13. 
Kosta: decided (2026-11-08)

## 4. Package format and location
**What it means.** Ana sends Bob one `.kablmod` (zip: wasm, manifest, optional art, README, LICENSE). Songs always use their embedded copy, so uninstalling never breaks a song.
**Decision.** Zip. Same id and version, different hash: the second install is refused and both hashes shown. Same id from another author: first install wins, the second is refused with a clear message. Linux install dir `$XDG_DATA_HOME/kabl/modules` (the user root today is `library.rs:281-291`, override `KABL_USER_DIR`); Windows/macOS paths `[unverified]`, decided in PR 4. In REAPER (CLAP) the plugin reads the same per-user dir, read-only, scanned on the control thread; sandboxed hosts `[unverified]`. Uninstalled module: songs open (blob embedded by hash, `embedded.rs`), the browser stops offering it. Cap 4 MiB per package is ample (wasm ≤ 2 MiB, art ≤ 128 KiB each).
**Reversible?** Partly (new fields yes; another container means two readers). **Depends on.** 3, 14. 
Kosta: decided (2026-11-08)

## 5. Trust and signing
**What a user sees today** (`crates/ui/src/code_notice.rs`): opening a `.kabl` with code shows "This patch contains custom code modules", the module ids, versions and short hashes, and a line that they run in a sandbox without file or network access; Open runs them, Cancel opens the patch with the code off; the answer is remembered per exact hash. An installed package would show the same notice the first time its module is used in a patch (to build).
**Decision.** Keep this; reserve an optional `signature` field in the manifest now, so signing can be added later without breaking unsigned packages. **Reversible?** Yes. **Depends on.** 4.
Kosta: decided (2026-11-08)

## 6. Languages
**Decision.** Rust supported; plus a one-page C example (~60 lines, clang to wasm32) to prove the ABI doc stands alone. A C creator must export `init/process/reset/memory`, place buffers at the layout offsets, add the `kabl.meta` section and avoid imports; the example shows each. **Reversible?** Yes. **Depends on.** 1.
Kosta: decided (2026-11-08)

## 7. Migrate and downgrade
**Walk-through.** v1→v2 (knob added): a song embedding v1 keeps running v1. If Bob chooses to switch it to v2, v2's `migrate(1, bytes)` maps the settings and the new knob takes its default; with no `migrate` the settings reset and the old bytes stay in the song. v2→v3 (rename): `migrate` carries the old value across, otherwise lost but kept in the song. v3→v1 (downgrade): the song carries its own v3 copy and plays it; only a manual reinstall of v1 creates a downgrade, and then v1's `load()` receives the newer version, rejects it, the module starts at defaults and the v3 settings stay in the song.
**Decision.** Module-exported `migrate`; a trap or bad output counts as no migrate (defaults + kept settings), never a crash. **Reversible?** Partly: the export is additive; the host call policy can change. **Depends on.** 1. Wiring `state_save`/`state_load`/`migrate` is D12 PR 0 (`state_save`/`state_load` are only names in `abi.rs:18-19`; D11 PR 6 does not call them).
Kosta: decided (2026-11-08)

## 8. Capabilities
**Decision.** None in D12; reserve `capabilities` (empty) in the manifest. First likely: tempo/transport, seeded random, sample-accurate events; each can be added as optional inputs or imports under the additive rule, so "none now" costs nothing later. **Reversible?** Yes. **Depends on.** 1.
Kosta: decided (2026-11-08)

## 9. Denormals
**Decision.** #128 is merged (`FtzGuard` in standalone, kabl-ui and CLAP process; wasm runs inside it): docs only (D11 REPORT: 12.2 vs 0.93 us/block/voice with the flag off/on). The CLI's offline harness must also run under `FtzGuard`, or creators see renders up to 13x slower on denormals and cost figures that do not match kabl. **Reversible?** Yes. **Depends on.** none.
Kosta: decided (2026-11-08)

## 10. Polyphony
**Decision.** The validator renders 16 voices for a `rate voice` module (1,000 blocks of 64) and one instance for `global`; at about 1 us per block per voice (D11 REPORT) the DSP is tens of milliseconds; I will state the measured total run time in PR 3, `[unverified]` until then. **Reversible?** Yes. **Depends on.** 9.
Kosta: decided (2026-11-08)

## 11. Where the SDK lives
**Decision.** `sdk/` in this repo (own workspace like `crates/modules/code-examples/`, which today has its own lockfile and `build.sh`); PR 1 ports the two examples to it; split to a separate repo only after the first fresh-agent run passes. Root `cargo deny` and `gen-notices.py` do not see `sdk/` (see 3). **Reversible?** Yes. **Depends on.** 3.
Kosta: decided (2026-11-08)

## 12. Cost figure
**Worked example (estimate, `[unverified]` until PR 3 measures).** D11's kernel (one biquad plus feedback) used 2,933 fuel per 64-sample block, about 1.03 us, 0.08 % of a 1.333 ms block at 48 kHz. `demo.bitfilter` has four biquads and a soft clip, about 4x: about 12,000 fuel. At the default 1,000 fuel/us that reports 12 us, **0.9 %** of the block; wall clock would be about 4 us, **0.3 %**, so the estimate is conservative by up to 3x. Near the 25 % cap: reduce work per sample, tables or voices; over it the module is cut off (silent blocks, D11 decision 5).
**Decision.** % of block at 48 kHz/64, labelled "estimate". **Reversible?** Yes. **Depends on.** 9.
Kosta: decided (2026-11-08)

## 13 (A). Publishing
**Decision.** Publish 0.x at the beta under Kosta's own account. The crate is `kabl-sdk-cli` with a `[[bin]]` named `cargo-kabl` (required for `cargo kabl`); `kabl-module-sdk` is the SDK crate; no `kabl-module-test`. **Reversible?** No once published (names and versions stay; only yank). **Depends on.** 3.
Kosta: decided (2026-11-08)

## 14 (B). Ids
**Decision.** `githubhandle.module`. Reserve `kabl.`, `demo.`, `test.`; the validator rejects them for third-party modules (the host's own examples keep `demo.`). **Reversible?** Mostly no (ids are in songs); more reservations can be added. **Depends on.** 4.
Kosta: decided (2026-11-08)

## 15 (C). Memory
**Decision.** Opt-in bump arena in the SDK (memory handed out once at init, never freed; sized from sample rate, never touched in `process`). SDK-only, no ABI change. Without it creators use fixed `static` arrays as the examples do. **Reversible?** Yes. **Depends on.** none.
Kosta: decided (2026-11-08)

## 16 (D). v0 blobs
**Decision.** Nothing has shipped, so there is no compatibility promise: before ship an old blob may be rejected with a "rebuild" message, and the examples and saved test files are rebuilt whenever the SDK changes. Where cheap, v0 blobs still load (no marker = revision 0); from the 1.0 ship they must. **Reversible?** Yes. **Depends on.** 1.
Kosta: decided (2026-11-08)

## 17 (E). Beta scope
**Decision.** Minimum for "code creator beta": PRs 0-4 and 6, about 3,030 lines (new sizes in `D12.md` section 7). Panels (PR 5) wait for 1.0; the compatibility corpus starts as a regression check in PR 0 with the two examples (it may be regenerated before ship) and becomes a binding compatibility test at the 1.0 freeze. Host-side example panels in `crates/ui/src/code_examples.rs` stay as they are. **Reversible?** Yes. **Depends on.** 18.
Kosta: decided (2026-11-08)

## 18 (F). Fresh-agent test
**Decision.** A different model from the docs author, fresh context, only the SDK docs and crate, a fixed task list in the repo, two runs with two different modules. Pass: both succeed, ≤ 3 doc defects per run, all fixed. A defect is any point where the agent had to read kabl's source or guess. Transcripts stored. **Reversible?** Yes. **Depends on.** 17.
Kosta: decided (2026-11-08)

## 19 (G). Reproducible builds
**Decision.** Two clean Linux CI builds on the pinned toolchain must be byte-identical (`--remap-path-prefix`, locked deps). No promise across operating systems. **Reversible?** Yes. **Depends on.** none.
Kosta: decided (2026-11-08)

## Still to answer
Nothing. All 19 decisions and the four contradictions are decided.
