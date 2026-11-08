# D12 creator SDK: owner decisions (2026-11-08)

Scope unchanged: D12 gives creators a supported SDK, package format and workflow on top of D11's wasmi modules. Detail, options and checks are in `D12-DECISION-SHEET.md`; the plan is `D12.md`. Each decision is recorded in `docs/design/verdicts.json`. Four points where a verified fact contradicts the input are listed at the top of the sheet and are still open.

---

## 1. No ABI freeze before 1.0
- The ABI stays preview, may change, until kabl 1.0. No freeze in D12.
- Rule: each revision only adds (optional exports, optional meta keys). Nothing existing is changed or removed. A change that breaks this is escalated to Kosta, not merged.
- Consequence: `demo.logic`, `demo.bitfilter` and `.kabl` files saved today stay loadable. Rebuild examples when the SDK changes; old blobs are not rejected.
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
- **D v0 blobs:** stay loadable (see 1).
- **E Beta scope:** PRs 0-4 and 6 (about 2,800 lines); panels wait for 1.0; the compatibility corpus starts in PR 0.
- **F Fresh-agent test:** different model, fresh context, docs and crate only, fixed task list, two runs with two modules; pass = both succeed, ≤ 3 doc defects each, all fixed.
- **G Reproducible builds:** two clean Linux CI builds on the pinned toolchain byte-identical; no cross-OS promise.

## Acceptance for D12
- A fresh agent working only from the SDK docs builds two distinct useful modules that survive save/reload, automation, polyphony where advertised and host render.
- Old projects keep their embedded module versions or show an actionable missing-version report.
- No "infinite performance or freedom" claim; local package workflow only; no store, accounts or payments.
