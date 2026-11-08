# D12 creator SDK: owner decisions (2026-11-08)

Scope unchanged: D12 gives creators a supported SDK, package format and workflow on top of D11's wasmi modules. Detail, options and checks are in `D12-DECISION-SHEET.md`; the plan is `D12.md`. Each decision is recorded in `docs/design/verdicts.json`. Four points where a verified fact contradicted the first input were answered the same day (section 4); nothing is open.

---

## 0. Governing principle: nothing is set in stone until we ship
- Until kabl 1.0 ships, the ABI, layout, meta format, package format and every other contract may change, including breaking changes. A breaking change before ship rebuilds the examples and saved test files and adds a changelog line.
- The additive-only rule and the escalate-to-Kosta rule below apply from the 1.0 ship onward, not now. The designs in 1 and in section 4 are what kabl ships with so 1.0 can grow additively.

## 1. No ABI freeze before 1.0
- The ABI stays preview, may change, until kabl 1.0. No freeze in D12.
- Rule from the 1.0 ship: each revision only adds (optional exports, optional meta keys). Nothing existing is changed or removed. A change that breaks this is escalated to Kosta, not merged. Before ship, see section 0.
- Consequence before ship: no compatibility promise. Rebuild examples and saved test files when the SDK changes; old blobs may be rejected with a "rebuild" message. From 1.0 they must load.
- SDK and docs are labelled "preview" until 1.0; the freeze is part of the 1.0 release work.

## 2. Licensing
- kabl host: GPL-3.0-or-later plus a module exception (GPLv3 section 7 additional permission) in `COPYRIGHT`.
- SDK crate, `kabl_module!` macro, meta builder, template, package/manifest spec: 0BSD. `cargo kabl` CLI including the test harness: GPL-3.0-or-later. No `kabl-module-test` crate.
- Why: the GPL probably does not reach a wasm module that talks to the host only through the ABI, but doubt keeps companies away, so a written exception removes it. MIT/Apache notices cannot travel with code inside every song, so 0BSD. The harness lives in the GPL CLI, so the creator's crate has no GPL dependency.
- Songs redistribute module code: publishing a package permits redistributing the unmodified wasm inside songs; manifest `licence` (SPDX) required; `source` URL required for copyleft licences. Closed paid modules are allowed but extractable; a "referenced, not embedded" mode is 1.0+.
- Facts checked: `LICENSE`, `deny.toml:1,17,25`, `Cargo.toml:9`, `COPYRIGHT` (existing section 7 terms), `CONTRIBUTING.md:3-7,16-18`, git authorship (133 of 1,071 commits authored as Claude, the rest Kosta). See sheet contradictions 3 and 4.

## 3. Other picks
- **2 Metadata:** `kabl.meta` text in the wasm; JSON only in the package manifest.
- **4 Package:** `.kablmod` zip; same id and version with a different hash: refuse the second and show both hashes; same id from another author: first install wins; songs always use their embedded copy.
- **5 Trust:** today's hash + one-time Open/Cancel; reserve an optional `signature` field.
- **6 Languages:** Rust supported, plus a one-page C example (~60 lines).
- **7 Migrate:** module-exported `migrate`; a trap falls back to defaults plus kept settings; a downgrade starts at defaults and the song keeps the newer settings.
- **8 Capabilities:** none; reserve the manifest field.
- **9 FTZ:** #128 merged (`FtzGuard` in standalone, kabl-ui, CLAP). Docs only, and the CLI harness runs under `FtzGuard`.
- **10 Polyphony:** the validator renders 16 voices and states the run time.
- **11 Location:** `sdk/` in this repo until after the first fresh-agent run passes.
- **12 Cost figure:** % of block at 48 kHz/64, labelled "estimate".
- **A Publishing:** publish 0.x at the beta under Kosta's account; the binary is `cargo-kabl`.
- **B Ids:** `githubhandle.module`; reserve `kabl.`, `demo.`, `test.`.
- **C Memory:** opt-in bump arena in the SDK.
- **D v0 blobs:** before ship may be rejected with a "rebuild" message; nothing has shipped, so no promise (see 0).
- **E Beta scope:** PRs 0-4 and 6 (about 3,030 lines); panels wait for 1.0; the compatibility corpus starts in PR 0 as a regression check that may be regenerated, and binds at the 1.0 freeze.
- **F Fresh-agent test:** different model, fresh context, docs and crate only, fixed task list, two runs with two modules; pass = both succeed, ≤ 3 doc defects each, all fixed.
- **G Reproducible builds:** two clean Linux CI builds on the pinned toolchain byte-identical; no cross-OS promise.

## 4. Contradictions answered (2026-11-08)
- **Unknown meta keys:** the host ignores them. A module declares `abi_minor N`; the host rejects it if N is greater than its own minor (covers required new keys). The `cargo kabl` validator warns on unknown keys. PR 0 (host), PR 2 (validator).
- **Layout growth:** optional export `layout_ext()` returns the address of a length-prefixed table (first word = count, then extension words). PR 0.
- **Notices:** keep 0BSD. `cargo kabl build` writes the Rust toolchain's MIT notice (core and compiler_builtins are dual MIT/Apache; comply via MIT) into a `kabl.notices` custom section, shown in module info. Docs say "notices handled automatically", not "no notice burden". PR 2 (about +60) and module-info display (about +80, in PR 4).
- **Authorship:** finalise `CONTRIBUTING.md` (remove draft status); add to `COPYRIGHT`: "Commits authored as 'Claude' are output of the owner's own AI-assisted sessions; any rights in them are held by the owner." Basis: Anthropic's terms assign all rights "if any" in Outputs to the user; purely AI-generated code may be uncopyrightable; either way no third party holds rights, so the module exception needs no one else's consent. No further check of plan terms. These legal-file edits go in PR 0 with the exception paragraph (planner's call; this PR stays planning only).

## Acceptance for D12
- A fresh agent working only from the SDK docs builds two distinct useful modules that survive save/reload, automation, polyphony where advertised and host render.
- Old projects keep their embedded module versions or show an actionable missing-version report.
- No "infinite performance or freedom" claim; local package workflow only; no store, accounts or payments.
