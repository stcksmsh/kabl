# D12 PR 3: `cargo kabl test` handover

Task: offline harness inside the GPL `cargo kabl` binary (spec: docs/product-research/briefs/D12.md lines 20, 37, 60, 94). Real WasmiEngine, own `FtzGuard`, no separate crate. Checks: golden hash, sweep, 16 voices, reset, state round trip, determinism. Fault matrix (trap, spin, NaN) from template features. Cost report in % of block at 48 kHz with the D11 2x caveat. Reuse validate.rs smoke render.

## Done
- Branch `claude/d12-test`, commit fb596e23, draft PR #141 (open, CI was still pending when stopped).
- `crates/cargo-kabl/src/test.rs` (harness + 3 unit tests); `validate::render` returns an output hash, `write_inputs` split out; `build::blob` split from `build::run`, `--features` support.
- Template: `fault-trap`/`fault-spin`/`fault-nan` features. Trap and spin fire only on non-zero input: the host's load-time priming block is silent, and an always-trapping module is refused at load.
- CI step runs `cargo kabl test` on the template. Local: fmt, clippy -D warnings, cargo-kabl tests, template tests green; end-to-end run on the template all ok.

- Newer stable clippy broke `lint`/`sdk` on unrelated code (`needless_range_loop`); fixed in audio_in, bitfilter example, drum_grid, macros (a2659f43, 13822083). Local `cargo +1.99.0 clippy --workspace --all-targets -- -D warnings` is clean (default stable 1.93 is too old to show them). Run `+1.99.0` before pushing.
- On a2659f43 all checks passed except `lint`; CI for 13822083 was pending when stopped.

## Remaining
1. `gh pr checks 141` (right after a push it says `no checks`; wait a minute); need ALL green.
2. Send `DONE 141 <commit>` to "Kabl overseer"; Kosta marks ready.
3. Codex round: comment `@codex review` after each fix push; fix valid findings, reply on each thread, until thumbs-up. Then `DONE 141 <commit> codex clean`, cargo clean, do not merge.

## Facts
- `cargo test --locked` fails until Cargo.lock has kabl-engine; it is committed now.
- Golden file is `kabl-golden.txt` next to Cargo.toml; do not commit one in sdk/template.
- Cost numbers from a debug-profile cargo-kabl are inflated (noted in output).

Standing rules: report to "Kabl overseer", do not merge, stop when done.
