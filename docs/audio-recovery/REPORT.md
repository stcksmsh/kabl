# D05 engineering report (2026-09-27)

Base: fetched `origin/master` at `e951b2535d817bc7c21872cd0e46e815d35747ac`, then incorporated its documentation-only successor `f1e2c9e`. Branch: `codex/d05-audio-recovery`; [draft PR #8](https://github.com/stcksmsh/kabl/pull/8), not merged. Tested and independently rechecked **product tree `47abdc4d34dc03d694d663863ebdf0f0443511ed`**, remote commit `7fd3a966eca193ef1babf83b5947333bb1e440b4`. Product changes: stopped Load carry in the engine, pending document command targeting, explicit audio state/retry/output selection, session gates, recording interruption, and offline mono profiling. The branch documentation head follows this product commit. Engineering is a draft with R-03 unresolved; owner review pending.

## Acceptance matrix

| D05 criterion | Status and evidence |
|---|---|
| Initial unavailable output, edit/save, select another, play in same app | Source paths implemented; no GUI walkthrough in this container. Unverified end to end. |
| Explicit lifecycle, bounded retries and stale session fencing | Focused `recovery_tests` and code in `ui/src/main.rs`; actual ALSA null backend tests pass. Physical loss and blocked close unverified. |
| Fault hooks | Opt-in stall/error, reopen failure and delay in `main.rs`. Callback error and repeated close/open tested on ALSA null; synthetic hooks cannot establish physical ALSA device loss. |
| Document latest revision / compile error / control reset | `poll_retry` synchronizes the current editor state before it opens the gate; pending Load/Toggle tests in `runtime_controls.rs`. GUI edit/undo during a retry unverified. |
| Pending Load across fade, second Load and explicit Start | `runtime_controls.rs` deterministic tests, including stopped incoming graph, timed Launch and deferred Preview. |
| Recorder prefix and new take | `record.rs` and `tests/record.rs` interruption test; reviewer fixes preserve the chosen folder and avoid cross-rate duration errors. No audible GUI take in this container. |
| Core/CC latency during Save/Open/reconnect | **Open gap.** Retry backend I/O and recording file finalization are outside `Core`, but browser Save/Open and retry compile still take it. No controlled CC latency result yet. |
| Production callback bounds | Atomic gate and counters in actual callback; D04 helper test covers drain/render only. No complete new production callback allocation check. |
| Mono efficiency | Profile below; no optimization applied without a measured eligible win. |
| Workspace regression, clippy, package | Workspace 557/0/18, strict Clippy clean, and final-tree ALSA null 2/0. Package/runtime status below. |
| Visual/audio/owner evidence | No graphical session or hardware; 1440×900/1280×800 light/dark screenshots, audio and Kosta's checks pending. |

## Verification environment and commands

Ubuntu 24.04 cloud container, x86_64, Rust 1.98.1, 4 virtual CPUs. Native ALSA development files were extracted into scratch for compilation; `ALSA_CONFIG_PATH` points to a `pcm.!default { type null }` software PCM. There is no `/dev/snd`, Xvfb or usable graphical display, and no real controller. The installed toolchain is in a scratch path; the reproducible repository commands are:

```
cargo test -p kabl-ui --bin kabl-ui --test runtime_controls --test record
ALSA_CONFIG_PATH=/path/to/alsa-null.conf cargo test -p kabl-ui --bin kabl-ui recovery_tests::actual_ -- --ignored --nocapture
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run --release -p kabl-engine --example mono_profile
```

At product head `cada932`, focused main/record/runtime/inspection tests gave 57 passed, 0 failed, 2 ignored. At final product tree `47abdc4`, `cargo test --workspace` gave **557 passed, 0 failed, 18 ignored**, `cargo clippy --workspace --all-targets -- -D warnings` passed, and the opt-in ALSA null recovery tests gave **2 passed, 0 failed**. The tests use a real `cpal` stream on ALSA null PCM but do not drive the graphical app. Strict Clippy needed four small lint fixes after the first recheck; the independent reviewer rechecked the exact resulting tree.

## Profile (no DSP optimization)

On this shared VM, `cargo run --release -p kabl-engine --example mono_profile`, 48 kHz, 256 frames as four blocks, eight voices, 1200 callbacks per run, one warmup + four measured runs (4800 callbacks per patch), notes [48, 55, 60, 64] periodically. Times are process wall time around `PatchEngine::process_block` four times, without a device callback or GUI. These are not laptop results or direct evidence of audible dropouts.

| Patch | Modules | Instances (voice instances) | Buffers | p50 / p99 / max / mean (µs) |
|---|---:|---:|---:|---:|
| Init Keyboard | 6 | 41 (40) | 21 | 85.1 / 163.9 / 1030.4 / 90.2 |
| Sound Palette | 94 | 360 (304) | 169 | 1197.2 / 1770.4 / 7282.5 / 1252.5 |
| Composition | 39 | 88 (56) | 37 | 214.5 / 304.0 / 1090.4 / 221.3 |

The dense graph is costly, but the harness does not isolate a safe mono-only subset or prove a repeatable reduction. Reducing lanes also changes per-voice gain/averaging unless carefully compensated. Therefore no engine optimization was made. The old cloud stalls and Composition spike have no inferred cause.

## Review and limits

The independent subagent's first review and final tree recheck are in REVIEW.md; the recheck found its code findings resolved but kept R-03 major and open. The acceptance gaps above prevent a claim that all D05 criteria are complete. In particular, physical device recovery, real app/package walkthrough, Core/CC latency under Save/Open, production callback allocation coverage, and the owner checklist remain pending. Backend close can block without a deadline; one retry worker stays owned and the UI names the blocked state after ten seconds.
