# Factory Performance Bank Report

## Status and Heads

Engineering checks pass for the authorized content scope. Independent review is recorded in REVIEW.md. Submission is a draft PR, not a merge or owner approval. Listening, controller feel, physical latency/xruns and production-beta acceptance remain pending.

Verified starting origin/master: `23075cd713884903a8985c88ab43bd1817477308`, merged D10 PR #13. No newer master changes were present at pickup. Isolated branch: `codex/factory-performance-bank`; checkout: `/home/stcksmsh/Programming/Github/kabl/.worktrees/factory-performance-bank`. Root master, `.ai`, prior worktrees, owner libraries and existing REAPER sessions were preserved. Private displays, task-owned profiles and separate host processes were used.

Runtime build/source: `160b3c2b20b8eb1437756f9dd7f090b8acacc0f2`. Final authored content/test source: `79d71beaeaebe3054adcdfb4cd75e7da7fe61342`. Later source changes are test-only Clippy fixes, authored factory gain and review repairs to the performance gesture score/guide, not runtime changes. Reviewed head is in REVIEW.md. Exact submitted/packaged heads live in external `manifest.json` and `submission.json`; a commit cannot contain its own final hash. `evidence/verification.json` records binary hashes and checks.

## Inventory and Acceptance

README.md lists stable IDs and controls. Six sounds: Round Keyboard Bass, Singing Lead, Glass Keys, Evolving Pad, Ensemble Strings, Breath. Four sequences: Sequence Bass, Interlocking Sequences, Echo Sequence, Slow Horizons. Two performances: Composition, Sound Palette Piece. Nine IDs reused, three additions. All 22 factory entries remain accessible. No new DSP runtime, dependencies or copied recordings.

| Requirement | Disposition | Evidence |
|---|---|---|
| Purpose/search/Factory/User and old metadata | Pass; graph-derived types retain old role categories | Factory-bank/browser tests and purpose-filter screenshots |
| Every curated browser load and prepared Perform | Pass at both sizes | `evidence/gui.json`, actual frames and silent video |
| Real macros, pins, expression, sequencers | Pass for scripted inputs; musical usefulness awaits listening | Exact-state tests and per-entry audio |
| Visible Run/Stop, banks and sections | Pass for exercised widgets; lower controls use existing Taller/scroll layout | GUI script and Perform frames |
| Stopped loads and no old held notes | Pass; formerly constant Composition pad is now gated | Replacement tests; silent first second in all renders |
| Edit/save/full restart | Pass with changed macro and exact complete state | GUI receipt and restart screenshot |
| REAPER recall and closed-editor native replay | Pass, fresh process/full restart with factory unavailable | Host state, native replay and mapped-plugin receipts |
| Existing projects/identities | Pass; appended controls, old projects keep embedded state | Workspace compatibility checks; unchanged native/plugin source |
| Light/dark, 1440×900 and 1280×800 | Pass for refreshed browser/Perform | Real application PNGs |
| Normal populated bundle/source-free recall | Final disposition in `evidence/portable.json` | Hash manifest, relocated launcher frames, bundled host mapping |
| Listening/controller/latency/xruns/beta | Pending | CHECKLIST.md |

## Checks and Audio

Workspace: 653 passed, 0 failed, 24 ignored writer/render utilities; final-content log `evidence/workspace-final.txt`. Earlier complete run is retained separately. Strict workspace/all-target Clippy passes. Release standalone/CLAP build passes. CLAP validator 0.4.1, upstream revision `152b982`, reports 35 successes and 9 capability skips, no failures. Logs/JSON are retained in `evidence/`. Existing nice-plug release warnings are inherited.

The actual release is built at the runtime head above; later tests/docs/factory files do not change runtime code. Factory data is loaded from packaged definitions and saved-state fixtures. Receipts identify exact exercised binaries.

Audio uses eight voices, 48 kHz and real 64-frame engine blocks. Twelve normal and twelve all-macros-one renders each last 24 seconds. First seconds are silent, all samples finite, peaks below clipping, activity nonzero; no normalization or limiter. Metrics include peak, RMS, activity and final-second tail levels. Long pad/strings/progression releases are measured intentional tails, not proof of physical stuck-note freedom. Thirteen AAC previews retain measured levels and WAV/preview hash/duration receipts.

The 72-second Sound Palette take uses the same bass, lead, pad, strings and breath builders as several curated entries, plus percussion. Editable sequencers, real cues, phrases and macros produce transitions; no baked audio. `performance-project/` embeds the starting graph and the render test preserves the gesture schedule. Distinct structures/roles establish candidate purpose, not owner listening approval.

Initial probes had fixture mistakes: a slider click did not change a value and a host envelope initial value differed from its recalled base. Final GUI uses numeric editing and asserts an actual change; host initial values now agree. Gain analysis found new keyboard voices too quiet; authored bass/keys gain was raised and all audio evidence refreshed.

Independent review found two P2 content issues: the initial long take left Strings/Pad/Lead muted, and Sound Palette's guide/tempo contradicted its Rest/104 BPM startup. The repaired take includes 25 explicit runtime layer/send changes; the guide now requires a cue after Run and uses the true tempo. Audio and affected GUI/portable evidence are refreshed. Workspace-wide formatting check also reports pre-existing style differences in untouched files; no blanket formatting changes were made. Strict Clippy and scoped tests remain the engineering gates.

## Delivery and Limits

Owner bundle: `/home/stcksmsh/.codex/visualizations/2026/10/08/01a11be1-c6f3-7fe2-b4e6-f33210e9ea62/factory-bank-owner-test`. `./launch.sh` provides bundled factory discovery and preserves explicit KABL_USER_DIR or normal native/XDG user-library behavior. `./launch-reaper.sh` uses installed REAPER, a private profile and local plugin. `./launch-portability-test.sh` is the separate unavailable-library check. Existing Linux GUI/audio libraries and installed REAPER are external requirements. No global plugin install or previous bundle replacement.

Original synthesis follows workspace MIT OR Apache-2.0; font and nice-plug notices are bundled. Remove the bundle to remove its artifacts, not normal user libraries.

Ordinary host-render nondeterminism, declared full warm-up render policy, dense-editor/backend limits and strict CLAP lifecycle/profile assumptions remain inherited. New host evidence proves recall/replay, not deterministic rendering or physical latency. Prior strict-profile evidence remains at `docs/host-production/evidence/strict-profile/`, product `f7157619836075d0d7704e59be5d66e6224890b1`, not relabeled as fresh measurements. D10 evidence is inherited only for untouched behavior; affected browser/Perform frames are refreshed.

Findings/final recheck: REVIEW.md. Stop after draft submission; no merge, D11/custom-code work or messages to other chats.
