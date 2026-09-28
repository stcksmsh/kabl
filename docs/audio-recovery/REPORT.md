# D05 engineering report (updated 2026-09-28)

Base: fetched `origin/master` at `e951b2535d817bc7c21872cd0e46e815d35747ac`, then incorporated its documentation-only successor `f1e2c9e`. Branch: `codex/d05-audio-recovery`; [draft PR #8](https://github.com/stcksmsh/kabl/pull/8), not merged. The first review covered **product tree `47abdc4d34dc03d694d663863ebdf0f0443511ed`**, remote commit `7fd3a966eca193ef1babf83b5947333bb1e440b4`. R-03 at that head is addressed in the 2026-09-28 update below. Owner review remains pending.

## Acceptance matrix

| D05 criterion | Status and evidence |
|---|---|
| Initial unavailable output, edit/save, select another, play in same app | Source paths implemented; no GUI walkthrough in this container. Unverified end to end. |
| Explicit lifecycle, bounded retries and stale session fencing | Focused `recovery_tests` and code in `ui/src/main.rs`; actual ALSA null backend tests pass. Physical loss and blocked close unverified. |
| Fault hooks | Opt-in stall/error, reopen failure and delay in `main.rs`. Callback error and repeated close/open tested on ALSA null; synthetic hooks cannot establish physical ALSA device loss. |
| Document latest revision / compile error / control reset | `poll_retry` synchronizes the current editor state before it opens the gate; pending Load/Toggle tests in `runtime_controls.rs`. GUI edit/undo during a retry unverified. |
| Pending Load across fade, second Load and explicit Start | `runtime_controls.rs` deterministic tests, including stopped incoming graph, timed Launch and deferred Preview. |
| Recorder prefix and new take | `record.rs` and `tests/record.rs` interruption test; reviewer fixes preserve the chosen folder and avoid cross-rate duration errors. No audible GUI take in this container. |
| Core/CC latency during Save/Open/reconnect | 500 ms Save and Open workers: mapped CC reached the actual ALSA null callback in 6.2/6.3 ms during Save and 4.1/6.3 ms during Open across two controlled runs. Structural compile is on a coalescing worker; delayed compile test verifies prompt CC handling and newest graph installation. Backend retry/teardown remains off `Core`. Physical controller/device latency pending. |
| Production callback bounds | Opt-in actual `cpal` closure test counts allocation and deallocation over warmed drain, note, render, report, gate, active recording tap and timing: zero of each on ALSA null. The first two startup callbacks are excluded. |
| Mono efficiency | Profile below; no optimization applied without a measured eligible win. |
| Workspace regression, clippy, package | Earlier tree: workspace 557/0/18. New code: UI 229/0/16 at its earlier checkpoint, workspace 560/0/20, strict Clippy clean, ALSA null backend tests pass. Hosted release package built; graphical recovery evidence is described below when verified. |
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

`cargo build --release -p kabl-ui --bin kabl-ui` passed on the same tree (5m 10s cold build). `OUT=/workspace/scratch/94df265b395c/d05-package bash packaging/linux/package.sh` assembled `kabl-0.1.0-linux-x86_64.tar.gz` outside the checkout: 94 archive entries, 4.9 MiB, `ldd` with no missing libraries. No package GUI launch/playback was possible here; this is build and archive evidence only.

## Profile (no DSP optimization)

On this shared VM, `cargo run --release -p kabl-engine --example mono_profile`, 48 kHz, 256 frames as four blocks, eight voices, 1200 callbacks per run, one warmup + four measured runs (4800 callbacks per patch), notes [48, 55, 60, 64] periodically. Times are process wall time around `PatchEngine::process_block` four times, without a device callback or GUI. These are not laptop results or direct evidence of audible dropouts.

| Patch | Modules | Instances (voice instances) | Buffers | p50 / p99 / max / mean (µs) |
|---|---:|---:|---:|---:|
| Init Keyboard | 6 | 41 (40) | 21 | 85.1 / 163.9 / 1030.4 / 90.2 |
| Sound Palette | 94 | 360 (304) | 169 | 1197.2 / 1770.4 / 7282.5 / 1252.5 |
| Composition | 39 | 88 (56) | 37 | 214.5 / 304.0 / 1090.4 / 221.3 |

The dense graph is costly, but the harness does not isolate a safe mono-only subset or prove a repeatable reduction. Reducing lanes also changes per-voice gain/averaging unless carefully compensated. Therefore no engine optimization was made. The old cloud stalls and Composition spike have no inferred cause.

## Review and limits

The independent subagent's first review and tree recheck are in REVIEW.md; that recheck found its code findings resolved but kept R-03 major and open at the earlier product tree. The 2026-09-28 work below changes that evidence. Physical device disruption, controller feel, listening, and the owner checklist remain pending. Backend close can block without a deadline; one retry worker stays owned and the UI names the blocked state after ten seconds.

## 2026-09-28 R-03 repair and callback verification

Browser Save, Save As, folder Save, Open, New and folder Open now run on one owned worker using document/log/library snapshots. Completion under `Core` is short and only installs a loaded patch if its document and editor state still match; an intervening edit gets a new unsaved-change question. A Save commits its starting snapshot, leaving later edits visibly unsaved. The worker is joined at shutdown rather than detached. Delayed Open and Save UI tests pass with 500 ms I/O and an intervening edit.

`Delivery` now schedules structural compilation on one owned worker. Edits during a build replace its next job; a superseded result is discarded. Fresh/stopped Load intent, revision ordering and queued actions remain behind the newest graph. During Retry the previous delivery moves with the old stream to the retry worker, so joining an in-flight compiler is also outside `Core`. A 500 ms compile test accepts a mapped CC and a subsequent structural edit within 350 ms, then installs the newest graph. While a structural graph is pending, its audio application can still wait for that build; the test does not claim subframe callback latency for a CC that changes the pending document.

An opt-in test uses the actual control pump, `Core` lock and `cpal` ALSA null callback while a 500 ms browser Save and Open worker runs. Two runs measured mapped CC arrival to callback acknowledgment at **6.2/6.3 ms during Save and 4.1/6.3 ms during Open**, below the test's 350 ms bound. This is controlled software-backend evidence, not a distribution or a physical-controller guarantee. An opt-in counting allocator surrounds every warmed production callback (after two startup callbacks); with a note and active recording tap it counted **zero allocations and zero deallocations**. The callback test includes queue drain, MIDI, render, reports, gate, tap and timing. The first two callbacks, backend error callback and backend internals are outside that count.

On the revised code, `cargo test --workspace` gave **560 passed, 0 failed, 20 ignored**; `cargo clippy --workspace --all-targets -- -D warnings` passed. The four opt-in ALSA null tests cover repeated reopen/failure, injected callback stall, warmed production allocation and delayed Save/Open CC delivery. The local environment has no physical output/controller; hardware and hands-on acceptance stay with Kosta.
