# kabl master plan: from the 1.0 tag to an instrument people recommend

Date 2026-11-22. Status: **proposal for Kosta**. Decisions already made are listed in §1.3 and are not reopened here.
This file sits above [AUDIO.md](AUDIO.md), [VIDEO-CORE.md](VIDEO-CORE.md) and [AV-MERGE.md](AV-MERGE.md). It sets priorities and order,
refers to them for technical detail, and says where it supersedes them (§1.4).

**How to read the statements in this file**

| Mark | Meaning |
|---|---|
| **Fact** | Verified in this repository (path given) or in the coordinator brief's code audit |
| *(notes file §n, tag)* | External evidence from the research notes, with its confidence tag |
| **Rec** | A recommendation of this plan. "Rec, no evidence" means nothing in the notes supports or contradicts it |

Notes files (in `research_notes/Kabl synteza afekt project plans/`): **R2** = `round2_product_claims_verified.md`,
**SAC** = `synth_adoption_community.md`, **PPT** = `plugin_platform_tech.md`, **AVM** = `av_instrument_market.md`,
**AIC** = `ai_composition_cost.md`, **DCA** = `local_deaf_composition_audit.md`. R2 takes precedence over the others.

Tags: **confirmed** = primary source or code read (in R2: a primary or major-outlet source seen in a search extract; no page was opened in full);
**snippet** = search-result extract only (all of SAC and AVM, and the items PPT marks so); **recalled** = researcher's memory, not checked;
**unverified** = the notes say it could not be established. Estimates in agent-weeks are estimates, not promises.

---

## 0. Summary

**Goal (owner's words).** "An app people actually use, want to use and like using enough that they recommend it to others", on the audio
side, the visual side and their intersection; multiplatform eventually.

**The bet.** kabl will not win on feature count. It can win on three things that users of the leading free synths say they miss:
patches that explain themselves, a project that visibly stays alive and treats every user the same, and factory sounds that are cheap on
CPU and trustworthy (§3.2). Those are cheap for kabl because most of the machinery exists. Reach then decides how many people can find
out: about 96% of plugin use is on Windows and macOS, and the largest hosts do not load CLAP (§3.3). So after 1.1 the next big step is
macOS/Windows with VST3 and AU, ahead of the wider sound palette. The audiovisual side is a second, later reason to recommend kabl; it is
gated on synteza's engine core and must never delay audio releases.

**Three biggest risks.**
1. **Nobody hears about it.** Linux + CLAP reaches a low-single-digit share of producers, and there is no existing audience to launch
   into (§3.1, §3.3). Mitigation: a deliberate Linux launch now, and a second launch with the multiplatform release.
2. **One maintainer, growing surface.** Windows/macOS windowing bugs, signing, three plugin formats, a GPU engine and a preset library all
   land on one person plus agents (§9). Mitigation: fixed order, small releases, wrappers instead of second implementations, no AV code in
   audio releases until its gates pass.
3. **The first impression is "small".** 100 presets, 8 voices and 4 filter families will be compared with Surge XT's 2,800+ patches
   (§3.4). Mitigation: lead with what is different, keep quality gates on presets, grow the count through a community route, and do not
   claim parity.

**Next 30 days.** Tag and publish 1.0; run the Linux launch (§5.2); ship a 1.0.1 inside the window to show cadence; add the two smallest
"instrument for programs" commands (`kabl render`, `kabl modules --json`, §7); write the migration chain design for 1.1; apply to SignPath
Foundation; test the competitor "Infinite" on Linux. Work orders are in §13.

---

## 1. Where kabl stands

### 1.1 Verified facts (brief §3, re-checked where marked)

| Area | Fact |
|---|---|
| Code | ~140k LOC Rust, ~1,380 tests, CI (fmt, clippy `-D warnings`, tests, licences, audio-defect scan, perf, package). `release.yml` runs on `v*` tags. Re-checked: 0 tags, workspace version 0.1.0 (`Cargo.toml:9`) |
| Repo weight | Checkout 352 MB, of which `docs/` is 331 MB; git pack ~934 MB |
| Content | 53 built-in modules; 100 presets in `patches/presets` (re-checked) plus ~47 demos; 8 factory wavetables |
| Audio limits | 8 voices fixed (`crates/clap/src/lib.rs:30`, `crates/standalone/src/lib.rs:125`); parameter routes evaluated once per 64-sample block; no MPE, aftertouch, CLAP note expressions, poly-mod or microtuning; `osc.va` triangle not band-limited; 4 filter families; one reverb; all effects global; no shared oversampling; no SIMD; code modules on wasmi |
| Known defect | Compiler `MAX_INPUTS = 4` (`crates/engine/src/compile.rs:91`) against SDK `MAX_PORTS = 16` (`sdk/kabl-module-sdk/src/meta.rs:27`) |
| Plugin | Vendored nice-plug, strict CLAP profile; 16 automation slots + gain + host clock; MIDI-dialect note port; VST3 wrapper present but disabled. eframe/egui 0.35 on `glow`; lockfile already carries wgpu 29 |
| Platform | Linux only. No hard lock-in in code; blockers for macOS/Windows are HOME/XDG path handling, CI/packaging and signing. Packaging today is a tarball with `install.sh` (`packaging/linux/`); no Flatpak, `.deb` or AUR package in the repo |
| File format | Loader accepts exactly schema 8 (`crates/core/src/format.rs:99-108`) |
| CLI | `kabl cat / pack / pack-tree / unpack` (`crates/core/src/cli.rs:9`) and a fixed-chord self-test render. Offline rendering exists only as cargo examples (`crates/engine/examples/render_midi.rs`, `tools/examples/av_proof.rs`). No module/param JSON dump, no MIDI-file import, no timeline or automation lanes |
| Already built for "self-explaining" | First-run overlay with a starter sound; Sounds browser with search and loudness-matched preview; macro explanations (D02); signal inspection (D03); screen-reader names (all in `docs/ACCEPTANCE-V1.md`) |
| Project rules | GPL-3.0-or-later with section 7 attribution and naming terms (`COPYRIGHT`); name and logo reserved; contributions under a licence grant with sign-off; AI-tool use must be disclosed in pull requests (`CONTRIBUTING.md:24`) |
| Panic policy | `panic = "abort"` (`Cargo.toml:14`): any in-process panic kills audio |

### 1.2 Product intent already written down

`docs/PRODUCT-PLAN.md` §1 states the promise: "start with a useful sound, change it intentionally, understand what made the difference,
and reuse what you learned", for musicians new to modular synthesis, Linux and REAPER first, and "prefer a small, coherent instrument over
feature-count parity". Its §7 defers Windows/macOS, MPE and AI patch generation until there is a tested Linux release and identifiable
users. This plan keeps that intent and says when those deferrals end.

### 1.3 Decisions already made (brief §2)

1. kabl 1.0 ships as it is. No Surge-parity work before the tag. Remaining: owner's acceptance sitting, SDK port-limit fix, version bump,
   an rc tag to prove `release.yml`, README/STATUS tidy. Recommended yes: Linux-only CLAP for 1.0; SDK stays a 0.1 preview; release notes
   from `CHANGELOG.md`.
2. First 1.1 item: a file-format migration path. Then the A1 voices/expression batch.
3. Dependency direction: kabl → synteza engine crates, later, one way. synteza never depends on kabl.
4. synteza is a standalone product and kabl's video core. kabl AV work starts only after synteza's engine core ("S1") exists.
5. AV model: one graph, two domains, visible converters on cross-domain cables, joint modules, cross-domain containers; bindings are
   ordinary modulation cables.
6. Release order V1.0 → 1.0.x → 1.1 → … → 2.0. Audio never waits for video.

### 1.4 Where this plan supersedes the other docs in `docs/v2/`

| Doc and place | It says | This plan says |
|---|---|---|
| README title | "audio that beats Surge, video that beats Lumen" | Internal ambition only. Not a public claim (§4.4) |
| AUDIO §3 A0 | polyBLAMP triangle, render nondeterminism, preset defects and the migration chain are prerequisites of 1.0 | 1.0 needs only the items in §1.3 point 1. polyBLAMP, nondeterminism and preset defects go to 1.0.x; migration is the first 1.1 item |
| AUDIO §3 A1.3 | MPE, CLAP note expressions and poly-mod as one item | Order inside it: MPE and aftertouch first, CLAP note expressions second, poly-mod last (§3.3) |
| AUDIO §3 A1.4, A4.3 | MTS-ESP and VST3 licences "need checking" | Both checked in the notes and usable in GPL-3 (§3.3, §5.4) |
| AUDIO order | A4 multiplatform after A2 and A3 | A4 moves ahead of A2 and A3, split in two (§5.5) |
| VIDEO-CORE §2.1 | "kabl is the only editor"; a synteza runner that loads `.kabl` files and links kabl-core/engine | synteza is standalone and never depends on kabl. For installations kabl streams bridge data to synteza. Its phases are owned by synteza's own plan |
| AV-MERGE §11 | M0 includes the migration path; M2 and M3 ship in 1.1; "run M1 now" | Migration is 1.1 audio work. M1 waits for S1. No AV milestone is tied to an audio release number (§5.7) |

---

## 2. What kabl has that the field does not

**Fact** (AUDIO.md §0, §2): a real modular graph with audio-rate cables and feedback; live edits with crossfaded graph swaps; sandboxed
WebAssembly code modules with a creator SDK; composites with custom panels; built-in sequencing (seq, drum grid, arp, cues, functional
cables); deterministic seeded renders; an allocation-free callback enforced by tests. Sandboxed user-code modules have no precedent among
the Rust synths found *(SAC §9, snippet, inference)*.

---

## 3. What the evidence says

### 3.1 Why free synths get recommended

| Reason given by reviewers and users | Example | Source and tag |
|---|---|---|
| Free with no functional crippling | Vital's free tier has the full engine | SAC §1, snippet |
| Looks and feels like a commercial product | "a level of complexity and polish that you don't normally see in free software" | SAC §1, snippet |
| Modulation is visible while it happens | BPB (June 2026) recommends Vital to beginners for this | SAC §1 and §3, snippet |
| A large or instantly available preset library | Surge XT: 2,800+ factory patches | SAC §1 and §4, snippet |
| Low CPU | A reviewer rates Surge above Vital because it is "much gentler on my CPU" | SAC §1, snippet (review ID not confirmed) |
| "Still being developed" | Same review | SAC §1, snippet |
| Fast editing, predictable CPU, simple trustworthy presets | 2026 KVR thread "The synths I keep are the ones that make editing feel quick" | R2 §8, confirmed as extract; check wording before quoting |

Two structural points. Every breakout in the set had an existing audience or ecosystem to launch into (Helm users for Vital, the Vember
user base for Surge, Eurorack culture for VCV, DX7 cartridges for Dexed); a new modular with its own module format has none and must
substitute strong factory content, creator seeding and import paths *(SAC §1, inference)*. And the same two names lead every 2026 "best
free synth" list found; a newcomer realistically targets a niche entry first, such as "best free modular", "best Linux synth" or "best
CLAP synth" *(SAC §1, inference)*.

### 3.2 Complaints about competitors that kabl can answer

| Complaint | Evidence | kabl's answer |
|---|---|---|
| Nobody explains the modules | VCV: "there is no handholding"; option overload for beginners. 2026 KVR thread on Bitwig's Grid asks for "tutorials which explain the modules" *(SAC §2 and §1, snippet)* | The self-explaining patch: macros that show their real targets, signal inspection, a working sound before any lesson (§6 items 1–4). Arturia made in-app tutorials a headline of Pigments 7, so the direction is validated at the commercial top *(SAC §3, snippet)* |
| Is it still maintained? Will my patches survive? | Vital: 1.6.x is early access for paying tiers; a free user was still on 1.5.5 in May 2026; "no changelog", "looks like it's abandoned"; status updates only in a paying-members Discord *(R2 §2, confirmed as user complaints; SAC §2, snippet)* | Same build for everyone on the same day, with a changelog *(R2 §2, inference)*; a public roadmap; old songs always open (the 1.1 migration chain) |
| Factory presets are CPU hogs | Phase Plant, Pigments and Serum 2 presets drive their CPU reputation; GUI animation is counted as synth CPU *(SAC §2, snippet)* | CPU-budgeted factory presets under the existing perf gate; cheap drawing when the editor is closed (§6 item 5) |
| Account and login walls | Vital needs a login and one online fetch of factory content; lasting resentment *(SAC §2, snippet)* | No account, no network use, content in the package. **Fact**: kabl has none of these today; keep it so |
| Dense or dated interface | The most common Surge XT criticism; "steeper for beginners" *(SAC §2, snippet)* | Existing rack, Perform view and Sounds browser; do not add a second modulation UI |
| Using a modular inside a DAW costs money | VCV's plugin is the paid Pro edition; Cardinal fills the gap *(SAC §2, snippet)* | kabl's plugin is free and the same program |

Surge XT is "between generations": stable is still 1.3.4, nightlies continue, and an incompatible "XT 2.0" is under discussion
*(R2 §2, partly confirmed)*. That is an opening, not a weakness to attack.

### 3.3 Platform and format reach

| Measure | Numbers | Source and tag |
|---|---|---|
| Plugin usage by OS, one developer's update-check data, June 2026, n = 57,789 | Windows 72.09%, macOS 23.88%, Linux 4.00% | SAC §7, snippet (forum post; post ID not confirmed) |
| KVR poll Dec 2024–Jan 2025, 418 votes, self-selected | Windows 46%, macOS 27%, Linux 10% | SAC §7, snippet |
| KVR poll to April 2026, 233 votes, self-selected | Linux 18% | SAC §7, snippet |
| General desktop share | StatCounter: Linux 4.39%, Windows 56.55% (June 2026, relayed on KVR) *(R2 §5)*; Linux 3.1% in March 2026 *(SAC §7; source article not confirmed)*. The two notes disagree; both put Linux at 3–5% | R2 §5 and SAC §7, unverified as producer share |
| KVR format poll, multiple answers | VST3 45%, CLAP 19%, VST2 16%, AU 11% | R2 §5, unverified as market share (enthusiast audience) |
| Mac share of revenue, one developer | "around 15%-20%" of users but 45% of sales; free plugins skew to Windows and REAPER | R2 §5 and SAC §5, snippet (anecdote) |

R2 states that no representative producer survey for 2024–2026 was found. Treat the table as order-of-magnitude.

| Host | Loads CLAP? | Source and tag |
|---|---|---|
| Bitwig, REAPER (since 6.71), FL Studio (since 2024.1) | Yes | R2 §3, confirmed |
| Studio One 7 | Yes, implementation reported as rough | R2 §3, partly confirmed |
| Ableton Live (12.4) | No | R2 §3, confirmed |
| Logic Pro | No (Audio Units only) | PPT §1, snippet |
| Pro Tools | No | PPT §1, snippet |
| Cubase / Nuendo | No official statement; a blog claim of "announced" is likely wrong | R2 §3, unverified |

Only Bitwig is confirmed for CLAP note expressions and per-voice modulation; for every other host this is unverified, so a plugin must
also work from plain MPE/MIDI *(R2 §3)*. MPE over MIDI 1.0 is the common expression transport across hosts *(PPT §7, snippet)*.

**What this implies.**
- Linux + CLAP + standalone reaches a low-single-digit percentage of producers. It is a good beta population (technical, forgiving,
  vocal) and a poor proxy for mass adoption; forum buzz will overstate reach *(SAC §7, inference)*.
- Windows and macOS builds with VST3 and AU gate access to roughly 96% of users and to the mainstream "best free synth" lists, which are
  written for Windows/macOS DAW users *(SAC §5 and §7, inference)*.
- "Native Linux support" is itself a cited reason to recommend Vital *(SAC §1, snippet)*. Keep Linux first-class after going multiplatform.
- REAPER and Bitwig users are the most reachable DAW communities for a CLAP-only release *(SAC §5, inference)*.

### 3.4 Preset-count expectations

| Product | Factory count | Source and tag |
|---|---|---|
| Vital free tier | 75 presets, 25 wavetables | SAC §1, snippet |
| Vital Pro ($80) | 400+ presets, 150 wavetables | SAC §1, snippet |
| Surge XT | 2,800+ patches | SAC §1, snippet (third-party comparison) |
| Serum on Splice | about 50,966 presets | SAC §4, snippet |
| kabl 1.0 | 100 presets, 8 wavetables | **Fact** |

100 is in the class of Vital's free tier and far below Surge XT; the number is used as a headline comparison and will be read as "small"
*(SAC §4, inference)*. Quality counts as much as number: Surge's bundled library "doesn't really show Surge at its absolute best", and a
KVR poster trusts "simple presets more than spectacular ones" *(SAC §4, snippet; R2 §8)*.

### 3.5 Windows and macOS signing from Serbia

| Option | Status for an individual in Serbia | Cost in the notes | Source and tag |
|---|---|---|---|
| Azure Artifact Signing | Not available: individuals limited to USA and Canada; individual onboarding reported paused | not readable | R2 §10, confirmed (Microsoft Q&A level) |
| SignPath Foundation | Realistic route. Free; the Foundation is the named publisher; needs an OSI licence without dual-licensing, a public repo, CI-built binaries, MFA, a published signing policy and at least one prior release. No country restriction appeared | free | R2 §10, partly confirmed; acceptance of Serbian applicants unverified (PPT §5) |
| Certum Open Source certificate | Paid fallback, issued to a natural person as "Open Source Developer, <name>"; valid at most 459 days. The notes disagree on which variant costs 69 EUR | "from EUR 69.00 gross" | R2 §10, partly confirmed; sale to Serbian residents unverified |
| Reseller OV certificate | Possible; key must be on a hardware token or cloud HSM | about 219–400 USD/year | PPT §5, snippet |
| Apple Developer Program (Developer ID + notarisation) | Required for macOS. Availability to individuals in Serbia was not checked | 99 USD/year | PPT §5, confirmed (price); unverified (Serbia) |

No certificate gives instant SmartScreen trust any more; reputation accrues per file, so expect warnings on each new release at first
*(PPT §5, snippet)*. **Open question:** kabl's licence carries GPL section 7 terms and contributions carry a licence grant. Whether
SignPath reads either as "dual-licensing" is not in the notes; ask in the application.

### 3.6 Monetisation precedents that work with GPL

| Model | Precedent | Evidence | Source and tag |
|---|---|---|---|
| Free, optional payment for the same thing | Bespoke Synth: free, or $5 / $15 | Sources disagree on where the money goes | SAC §6, snippet |
| Source free, official binaries behind pay-what-you-want | Ardour: about US$130k in 2017 from 3,000+ small subscribers, after years of missed goals. Zyn-Fusion: "$45+" | The only GPL audio model here with published, livable numbers; it took Ardour over a decade | SAC §6, snippet |
| Content and services on an open engine | Vital: preset/wavetable tiers at $25 / $80 or $5 per month | No revenue figures public | SAC §6, snippet |
| Open core with paid modules | VCV Rack Pro ($99 at launch), paid modules, VCV+ | Pushback on the subscription | SAC §6 and §2, snippet |
| Patronage | Sonic Pi: full-time on Patreon/GitHub Sponsors, yet "only a couple of months left of funding" in Sept 2024. Base rate: 25% of active Patreon campaigns earn over $120 per month, 5% over $750 | Fragile | SAC §6, snippet |
| Grants | NLnet funded cables.gl | One case | AVM §1 and §7, snippet |

GPL lets others remove a paywall on convenience: Cardinal ships a free plugin build of the Rack engine *(SAC §6, snippet)*.

### 3.7 The audiovisual field: Lumen frozen, Infinite arrived

- **Lumen** (the owner's video benchmark) is in maintenance: "there are currently no new features for Lumen under development"; macOS
  only; $129 *(R2 §6, confirmed)*.
- **Infinite** is new: a free, MIT-licensed "modular audiovisual workstation" (Sept–Oct 2026) with 300+ node types, a live GLSL editor,
  VST3 hosting and Syphon/Spout/NDI. BPB lists macOS and Windows; its README adds Linux *(R2 §6, confirmed as new entrant; maturity, real
  Linux status and reception unverified)*.
- "Integrated modular audio + video" is therefore no longer an empty category. Differentiation must rest on synth quality, plugin form
  factor, Linux polish and structure-aware visuals, not on the bare claim of integration; Infinite should be tested before the AV plan is
  final *(R2 §6, inference)*.
- Demand is indirect: a VCV user "would certainly be willing to pay for video modules"; several VCV threads ask for video; the common
  workaround is sending data to TouchDesigner *(R2 §8; AVM §2, snippet)*. This supports "people keep asking", not a measured market.
- "Follows the verse, chorus and drop" is now a marketing line of 2026 AI-video tools; a local, real-time, non-generative structure-exact
  visual is still different, but the phrase alone is no longer novel *(R2 §8, inference)*.
- No commercial leader (Resolume, TouchDesigner, Synesthesia, Lumen, Videosync, VS 2) ships a native Linux build *(R2 §6, confirmed
  absence at secondary level; AVM §5, snippet)*. Resolume's own view: a port "will probably not lead to more sales" *(AVM §5, snippet)*.
- Do not bundle Shadertoy-derived shaders or MilkDrop preset packs in a GPL release without per-item clearance; support the formats and
  let users load content. The ISF stock-shader licence name is unverified in R2 (an earlier note called it MIT from a mirror) *(R2 §7)*.

---

## 4. Positioning

### 4.1 kabl 1.0

kabl 1.0 is a free, open-source modular synthesizer for Linux, standalone and as a CLAP instrument, whose patches explain themselves: you
start from a sound that works, see what each control really changes, and can trace what you hear back through the cables. It has built-in
sequencing and performance (sequencer, drum grid, cues, macros, functional cables) and lets you write your own sandboxed modules. It is
not a Surge or Vital replacement, not a wavetable flagship, not an MPE instrument, and not available for Windows or macOS yet. One
sentence for news sites: "a free modular synth for Linux that explains its own patches". (**Rec**; the need for a reusable one-line pitch
is from SAC §8, snippet.)

### 4.2 kabl 1.x

1.1 makes kabl play like a modern synth (more voices, smooth modulation, MPE, microtuning) and guarantees that old songs open. 1.2 is the
release for everyone else: Windows and macOS, VST3 and AU, signed installers. 1.3 and 1.4 widen the sound (filters, shapers, oversampling,
MSEG, then effects). Through all of it the promise stays the same: same build for everyone on release day, a changelog, no account.

### 4.3 kabl 2.0

kabl 2.0 is an audiovisual instrument: one modular graph in which sound and image are patched together, so an envelope can open a filter
and a feedback zoom at once, and the picture follows the patch's own structure (steps, cues, notes) exactly instead of guessing from the
mix. The video engine is synteza, which is also a standalone visualizer. The claim to earn is "structure-exact visuals from a real synth,
in a plugin-capable, Linux-first instrument", shown in clips, not asserted.

### 4.4 What kabl should NOT claim

| Do not claim | Why |
|---|---|
| "Beats" or "rivals" Surge XT, in public | No blind A/B exists (AUDIO A5.3 is the gate). Small synths that invite the comparison lose it *(SAC §9, snippet, attribution uncertain)* |
| "The first" or "the only" integrated audio + video modular | Infinite claims it, free and MIT *(R2 §6, confirmed)* |
| "Video that beats Lumen" | Lumen facts in VIDEO-CORE are from general knowledge; kabl has no video yet |
| "AI synth", "AI-powered" | The disliked pattern is opaque generation; see §7.4 |
| MPE, polyphonic modulation, microtuning | Not in 1.0. After 1.1, describe poly-mod as a Bitwig feature only *(R2 §3)* |
| "Runs on Windows/macOS" or "cross-platform" | Not until signed builds exist |
| "VST" as part of the product name or logo misuse | Trademark rules remain under the MIT SDK *(PPT §2, confirmed)* |
| "Visuals that follow the drop" as the headline | Already a vendor slogan; one buyer of a lighting box said "claims went further than the results" *(AVM §4, snippet)* |
| Any user or download number that was not counted | No baselines exist for any free synth *(R2 §4, unverified)* |

---

## 5. Roadmap

### 5.1 Phases at a glance

| Phase | Release | Content | Done when | Estimate |
|---|---|---|---|---|
| **P0** | 1.0 | Acceptance sitting; SDK port-limit fix; version bump; rc tag; README/STATUS tidy | `v1.0.0` tag exists; `release.yml` artifacts are on the Releases page; a clean machine installs from the tarball and plays the first-run sound | in progress (separate session) |
| **P1** | 1.0.x | Launch (§5.2); bug-fix releases; polyBLAMP triangle; preset defects from the CI scan; render nondeterminism root cause; `kabl render` and `kabl modules --json` (§7); Linux packaging | Launch checklist ticked; at least one 1.0.x release with a changelog within 30 days of the tag; `kabl render` used by CI for golden clips | 3–5 agent-weeks |
| **P2** | 1.1 | Migration chain first; then A1: polyphony, per-sample route smoothing, expression, microtuning | Every released schema opens (test); a factory patch plays 16+ voices inside the perf gate; an MPE controller bends single notes in standalone and in one CLAP host; a `.scl` file retunes a patch | 1 + 6–8 agent-weeks (A1 figure from AUDIO §5) |
| **P2b** | preview builds | A4 part 1, in a parallel worktree: paths, CI matrix on macOS and Windows, unsigned CLAP + standalone preview builds | CI green on all three systems; the owner or a tester loads the preview in one host on each | 2–3 agent-weeks |
| **P3** | 1.2 | A4 part 2: VST3 and AU, signing, installers, more than 16 host parameters; preset growth; second launch | Signed installers for Windows and macOS; kabl loads in Ableton Live (VST3) and Logic (AU) and passes the §3 REAPER checks of `ACCEPTANCE-V1.md` adapted per host | 3–5 agent-weeks plus signing lead time |
| **P4** | 1.3 | A2: shared oversampling, filters and shapers, MSEG and envelopes, oscillators, expression module | Each new family has a clip, a CPU row in `docs/benchmarks.md` and at least five factory presets that use it | 10–14 agent-weeks (AUDIO §5) |
| **P5** | 1.4 | A3: reverbs, shifter, tape, per-voice effects, lookahead limiter | Same rule as P4 | 4–6 agent-weeks (AUDIO §5) |
| **A5** | continuous | Presets, wavetables, blind A/B | See §6 item 7 and §5.6 | content-bound |
| **AV** | flagged branch, then its own releases | §5.7 | per milestone | AV-MERGE §11 |

### 5.2 Launch plan for 1.0

Everything here follows from SAC §5 and §8 unless marked. SAC found no measured effect for any channel ("inferred from sequence, not
measured"), so the list is ordered by cost, not by proven return.

| Step | Action | Evidence |
|---|---|---|
| Pitch | One sentence (§4.1) and the story "1.0 by one person" | Bespoke's "1.0 after 10 years" launch was picked up within days *(SAC §8, snippet)* |
| Demo material | A 60–90 second sound-first video; three screenshots (exist in `docs/readme-v1/`); five short audio clips from factory pieces; one 3-minute "open a preset and see why it sounds like that" walkthrough | 60–90 s video is the notes' inference *(SAC §8)*; the walkthrough is **Rec** from §3.2 |
| Install path | Tarball (exists); AUR package; then Flathub in two parts: app and `org.freedesktop.LinuxAudio.Plugins` extension | Flatpak hosts only see plugins shipped as extensions *(PPT §6, confirmed from Bitwig's manifest)*; discovery of plugin add-ons is weak *(PPT §6, snippet)* |
| Same-day posts | KVR product page and thread; LinuxMusicians; r/linuxaudio; r/synthesizers | SAC §5 and §8, inference |
| Press note | Short email with video and screenshots to BPB, Synthtopia, CDM, Gearnews and Linux press | These sites covered Vital, Cardinal and Bespoke within days *(SAC §5, snippet)*; "lowest-cost, highest-reach action" is inference |
| Educators | Early builds to two or three YouTube educators, one a modular teacher | One creator is repeatedly credited with onboarding VCV users *(SAC §1, snippet)*; effect on a new synth unverified |
| Follow-up | Nominate kabl for a KVR One Synth Challenge month, or run a patch contest four to eight weeks after launch | OSC has featured small free synths *(SAC §4, snippet)*; no source measured a download effect |
| Developer audience | A talk proposal (FOSDEM, Linux Audio Conference, ADC) and a "Rust modular synth" post | Rust matters to developers, not musicians *(SAC §9, inference)* |
| Cadence | Ship 1.0.1 with a changelog inside the launch window | "still being developed" is a recommendation reason *(SAC §1)*; session-breaking bugs at launch "impede customers from spreading the word" *(SAC §3, snippet)* |

**What to measure** (methods in §10): release downloads per asset on day 1, 7 and 30; stars; issues and discussions opened by people
other than the owner; which channel each newcomer names (ask in the issue template, **Rec**); crash or audio-failure reports per download.

**Before launching, fix:** anything in the acceptance sitting that interrupts a session. Do not delay the launch for features.

### 5.3 1.0.x

Audio-only, no format change. In order: launch fixes; preset defects named in AUDIO A0.5; polyBLAMP triangle; render nondeterminism root
cause; `kabl render` and `kabl modules --json`; packaging. Also reduce repository weight for contributors (331 MB of `docs/`), for example
by moving clips and mockups out of the default checkout (**Rec, no evidence**).

### 5.4 1.1

1. **Migration chain** (decided). A chain of `vN → vN+1` functions, a test that opens a saved file of every released schema, and a rule
   that no schema bump merges without its migration. Done before any v9 change.
2. **A1 in this order** (AUDIO §3 A1, re-ordered inside item 3 by the host evidence):
   polyphony → per-sample route smoothing → per-voice expression model with MPE zones and aftertouch → CLAP note expressions on the same
   model → microtuning (`.scl`/`.kbm`) → CLAP poly-mod last, as a Bitwig showcase.
   - Declare both `CLAP` and `MIDI_MPE` note dialects with `CLAP` preferred *(PPT §7, inference from the confirmed header text)*.
   - Microtuning: the Rust `tune` crate (MIT) reads `.scl`/`.kbm`; the MTS-ESP client is under a zero-clause licence and is vendored by
     GPL-3 Surge XT, so the licence question in AUDIO A1.4 is closed *(PPT §8, confirmed)*.
   - A "CLAP Expression Tests" host-side suite exists for checking behaviour *(R2 §3)*.

### 5.5 When multiplatform and VST3/AU should land: recommendation

**Rec: move A4 ahead of A2 and A3, and split it.** Part 1 (paths, CI matrix, unsigned preview builds) runs beside A1; AUDIO §5 already
says A1 and A4 are independent. Part 2 (VST3, AU, signing, installers) is the 1.2 release, directly after 1.1.

| | A4 right after 1.1 (recommended) | A4 after A2 and A3 (AUDIO.md order) |
|---|---|---|
| Reach | Opens roughly 96% of users and the mainstream lists about 14–20 agent-weeks sooner (A2 + A3 estimates) | Stays at a low-single-digit share through 1.3 and 1.4 |
| Goal fit | Recommendation needs people who can install it | Better sound for an audience that cannot run it |
| First impression | Mainstream users meet kabl with 4 filter families and a small library; risk of "small" (§3.4) | Wider palette at the mainstream launch |
| Cost | Windowing work (below), signing lead time, support load on three systems while A2 is being built | Cross-platform bugs found late, across more code |
| News | A second launch moment, free *(SAC §8, inference)* | One launch, later |
| Feedback | Palette priorities for A2 come from many more users | Priorities guessed from Linux users only |

Why not before 1.1: the mainstream launch should not meet 8 fixed voices and stepped modulation, and "old songs always open" must be true
before a large audience saves songs. Why not before 1.0: decided (§1.3).

Technical notes for A4, all from PPT:
- **Wrapper route.** clap-wrapper (MIT, 0.18.0) projects one CLAP into VST3, AUv2, AUv3, AAX and standalone; its VST3 and AUv2 wrappers
  have long been "feature complete", AUv3 and AAX are only months old *(PPT §2, confirmed)*. A Rust packaging exists as the
  `clap-wrapper` crate. **Rec:** keep CLAP as the single native format and wrap for VST3 and AUv2; do not enable the framework's VST3 export
  as a second code path. How the wrapper maps note expressions to VST3/AU is not documented in the notes (gap).
- **VST3 licence.** MIT since SDK 3.8 (29 Oct 2025); no agreement to sign *(PPT §2, snippet for the announcement, confirmed in the SDK
  README)*. The licence check in AUDIO A4.3 is closed.
- **Framework drift.** Upstream NIH-plug is in maintenance mode; nice-plug is at 0.4.2 and egui-baseview at 0.9.0 *(PPT §3, confirmed)*.
  kabl's vendored copy must either be rebased or kabl owns the windowing fixes *(PPT §3, inference)*. Decide at the start of part 1.
- **Known windowing hazards** in baseview editors: Windows frames driven by a 15 ms low-priority timer; re-entrancy that can abort the
  host; macOS teardown crashes and missing resize events; X11 embed resize in Bitwig *(PPT §3, confirmed as a fork's documented patches)*.
  Budget for these; they are the main schedule risk of 1.2.
- **Standalone on macOS.** cpal's README lists macOS 14.2 as the CoreAudio minimum *(PPT §3, confirmed)*; check against the intended floor.
- **Wayland.** Plugin editors stay X11/XWayland; expect that through 2027 *(PPT §3, inference)*.

### 5.6 Order of A2 to A5

1. **A5 is continuous from 1.0.x**, not last. Presets grow under gates (§6 item 7).
2. **A4** as in §5.5.
3. **A2**, in this internal order (**Rec**): shared oversampling and real `QualityConfig` (A2.3) → filters and shapers (A2.2; the largest
   gap in AUDIO §2) → MSEG and envelopes (A2.4; the MSEG also serves as an automation lane for programs, §7) → oscillators (A2.1) →
   expression module (A2.5).
4. **A3** after A2, because the lookahead limiter and per-voice effects depend on per-patch latency reporting from A2.3.
5. **Blind A/B** (A5.3) once A1 and A2 are in. It is the gate for any public comparison with Surge.

Let user requests reorder items inside A2 and A3 once 1.2 is out. `PRODUCT-PLAN.md` §7 already says: add the smallest feature that solves
a demonstrated task.

### 5.7 The AV track

Rules: gated on synteza S1; developed in a feature-flagged branch; ships no code in an audio release until M1's gate passes; never blocks
an audio release. Milestone content stays as in AV-MERGE §11 except where this table changes it.

| Milestone | Gate to start | Change from AV-MERGE §11 | Done when |
|---|---|---|---|
| **M0** | — | Now only: 1.0 tag and SDK fix on the kabl side. Migration moves to 1.1. synteza cleanup, licence and CI ("S0") belong to synteza's plan. kabl cannot ship synteza code until synteza has a licence | `v1.0.0` released |
| **Pre-M1** (new) | 1.0.x `kabl render` exists; synteza has frozen its sidecar format | `kabl render --events-out`: write a sidecar of the patch's own events (steps, gates, notes, cue landings, with sample positions) next to the WAV. synteza standalone then draws structure-exact visuals for a kabl piece with no live bridge and no code dependency; afekt can read the same file | A factory piece rendered by kabl plays in synteza with visuals that follow its cues; owner judges the clip |
| **M1 bridge proof** | synteza S1 (engine core) exists | Not "now". Otherwise unchanged: `BridgeOut` steps and snapshot/event ring behind a flag; two visual modules in a window; kill test | 10-minute soak with 0 xrun delta and 0 audio-thread allocations; event-to-photon p50/p99 measured; killing the render thread leaves audio playing; owner plays it live and approves continuing |
| **M2 domains** | M1 approved; 1.1 migration chain merged | No longer tied to 1.1. Ships as an opt-in AV preview alongside whichever audio release is current. eframe moves from `glow` to wgpu here | A musician patches ADSR → feedback zoom and gate → flash in the rack, saves, reloads, plays fullscreen |
| **M3 containers + Looks** | M2 | Factory Looks are built from synteza's default hysts; a kabl Look wraps a hyst | The "Bloom Bass" container: drop it in, play, sound and image together |
| **M4 joint modules** | M3 | unchanged | `osc.scan` and `voice.particles` run at 60 fps with 16 voices |
| **M5 shaders + offline** | M4 | Ship no third-party shader packs without licence clearance (§3.7) | A user drops a shader file and gets a module; golden AV clips pass in CI |
| **M6 media + outputs** | M5 | unchanged | Each output verified on its system |
| **M7 2.0** | M6 and audio 1.2 (multiplatform) shipped | "Runner/sidecar mode" becomes: kabl streams bridge data to synteza for installations, and exports events for afekt. The synteza runner does not load `.kabl` files | Windows/macOS/Linux builds of the AV instrument; recorded comparison clips |

Technical constraints to carry into M1 (all PPT §4): one wgpu device per process, surfaces per window, treat surface loss as routine,
never block the UI thread on the GPU queue; a triple buffer for latest-value state and an SPSC ring for events (AV-MERGE §4.1 already
does this). wgpu ships a breaking major release every three to four months *(confirmed)*; synteza is on wgpu 22 and kabl's lockfile on 29
(**Fact**), so the two repos need one agreed pin. One team shipping wgpu editors rejects `panic = "abort"` *(PPT §3, confirmed as a PR
text, not verified in a real DAW)*; kabl sets it (**Fact**), which makes the M1 kill test decisive.

Before M1 starts: test Infinite (§13, W10) and write down, in AV-MERGE, what kabl's AV side will do that Infinite does not.

---

## 6. What makes people recommend it

Concrete product work, each tied to evidence or labelled.

| # | Work | State today | Evidence |
|---|---|---|---|
| 1 | **First run: sound within seconds.** Keep the overlay and starter sound; measure time from launch to first sound on a clean machine; one obvious next action that changes the sound | Overlay and `pluck-line` exist (`ACCEPTANCE-V1.md` §1) | First-run priority order *(SAC §3, inference)*; "a working sound precedes a lesson" (`PRODUCT-PLAN.md` §1) |
| 2 | **Preset browser: fast and filterable.** Keep preview instant; add CPU cost and "uses code module" badges; favourites; a "simple" tag | Search, groups and matched-loudness preview exist | Slow browsing is why Dexed "feels heavy" *(SAC §3, snippet)*; trust in simple presets *(R2 §8)* |
| 3 | **Visible modulation.** Every modulated knob shows its moving value; a "what modulates this" list on each knob (AUDIO §4) | Routes, macros and face displays exist | The single most-cited reason a synth is called beginner-friendly *(SAC §3, snippet)* |
| 4 | **In-app explanation.** Every factory preset carries a short "how this works" note tied to its macros; signal inspection reachable in one click from a silent patch | Macro explanations (D02) and inspection (D03) exist | "No handholding" in VCV; Pigments 7 in-app tutorials *(SAC §2, §3, snippet)* |
| 5 | **CPU.** A CPU budget per factory preset in the perf gate; cheap or no drawing when the editor is closed; surface the reduce-motion setting (code for it appears in `crates/ui/src/cable_life.rs` and `style/load.rs`; verify) | Perf gate exists | Users count GUI load as synth CPU *(SAC §2, inference)*; "one patch eats the session" *(R2 §8)* |
| 6 | **Stability.** A crash or audio failure in a host is a release blocker; keep the audio-recovery path; test each release in REAPER and Bitwig | Recovery (D05) and CLAP validator runs exist | Launch bugs suppress word of mouth *(SAC §3, snippet)* |
| 7 | **Preset count with quality gates.** Grow in batches; each preset passes the loudness gate, the defect scan, a CPU budget and a recorded listening pass, and has named macros and a note. Publish the count in each changelog | Loudness gate and defect scan exist (AUDIO §1.8) | Count is a headline comparison; quality carries the first impression *(SAC §4, inference)* |
| 8 | **Creator and preset ecosystem.** A documented, stable patch format (after 1.1); a `community-patches` repository or Discussions category with a submission template that runs the same gates in CI; credit authors in the browser. Patches and courses are what third parties can sell around a GPL synth | `.kabl` files carry their code modules | Preconditions seen in every ecosystem: stable format, a place to share, a reason to make content *(SAC §4, inference)* |
| 9 | **Docs and tutorials.** Five short task guides (first sound, change a sound, build a voice, sequence it, use it in REAPER), each under five minutes, each ending in a saved patch; one walkthrough video per release | Feature guides and SDK docs exist | Bitwig publishes its own Grid tutorial series, and users still ask for module explanations *(SAC §1, snippet)* |
| 10 | **Community channels, open to everyone.** GitHub Discussions as the home; announcements with the changelog there first. A chat room only if the owner will be present in it | none | Vital's updates inside a paying-members Discord damaged its reputation among free users *(SAC §2, §6)*. Which platform is **Rec, no evidence** |
| 11 | **Release cadence and roadmap.** A public roadmap page (this file's §5.1 table); a changelog on every release; no tiers | `CHANGELOG.md` planned for release notes | "Same build, same day, with a changelog" is a cheap differentiator *(R2 §2, inference)* |
| 12 | **No account, no network, no telemetry.** State it in the README | True today | Account walls cause lasting resentment *(SAC §2, inference)* |

Items 1–6 and 11–12 are mostly protection of what exists. Items 7–10 are new work and belong in 1.0.x and 1.1.

---

## 7. kabl as an instrument for programs (deaf-composition tie-in)

### 7.1 Why

deaf-composition drove REAPER and Surge XT through about 150 MCP tools. The audit documents the cost: each preset apply was about 500
parameter round-trips and over 2 minutes; 7.6–7.9 s fixed overhead per render; RPC timeouts that did not mean failure; silent presets;
hidden parameter units *(DCA §4, confirmed in that repo)*. The scripted LLM spend was small (about 50 calls, about $1.2 per clean song);
the budget most likely went to interactive agent sessions doing execution and debugging as tool calls *(DCA §2, inference)*. Note that
AIC §8 models the same loop at $9.25–22 per song from assumed call counts; the local audit read the code and should be preferred.

kabl removes most of that by construction: fixed 64-sample blocks, seeded and hashable renders, parameters in real units, and a patch
that is a JSON op log (**Fact**; DCA §5). Derived speed: the densest shipped patch runs about 20–30× realtime, so a 40 s part renders in
roughly 1.5–2 s with no host start-up *(DCA §5, derived from `docs/benchmarks.md`, not measured as wall time)*.

### 7.2 What to add, sized and ordered

| # | Addition | Detail | Size (estimate) | Release |
|---|---|---|---|---|
| 1 | `kabl render PATCH [EVENTS] OUT.wav` | Promote `crates/engine/examples/render_midi.rs`: `--secs`, `--tail`, `--rate`, stereo float WAV; print hash, peak and active duration as JSON | 0.5–1 agent-week | 1.0.x |
| 2 | `kabl modules --json` | Dump every kind's ports and `ParamInfo` (name, min, max, default, unit, taper). The data exists (`registry.rs:233 all_infos`) but is not serialisable | about 0.5 | 1.0.x |
| 3 | Patch editing from the command line | `kabl new --from PRESET OUT` and `kabl set OUT module.param=value`, writing valid ops with their mandatory inverses | 0.5–1 | 1.0.x |
| 4 | Note input from a MIDI file | `.mid` (type 0/1, tempo map) → the existing events format; also accept sequencer data by setting `seq` / `drum.grid` step params with item 3 | 0.5–1 | 1.0.x or 1.1 |
| 5 | Per-part stems and a song manifest | One patch per part gives a stem by construction. Add `kabl render-song MANIFEST`: parts, start times, gains → one WAV per part plus the sum | 1–2 | 1.1 |
| 6 | Automation, option A: parameter keyframes in the events file | Timed runtime parameter sets (the engine path `av_proof.rs` already uses from Rust), ramped per block | 1–2 | 1.1 |
| 7 | Automation, option B: lanes as a module | The A2.4 MSEG, tempo-synced, as an in-patch automation lane | part of A2 | 1.3 |
| 8 | Metre | Cue launches assume 16 sixteenths per bar (`patch_engine.rs:124`, per DCA); the last deaf-composition song was in 5/4 | 0.5–1 | 1.1 |
| 9 | Event sidecar out | `--events-out` (§5.7 Pre-M1) | 1–2 | after synteza freezes the format |
| 10 | Optional MCP server | A separate binary that wraps items 1–6 (§7.4) | 1–2 | after items 1–5 are stable |

Not planned: a full song timeline or piano roll inside kabl (`PRODUCT-PLAN.md` §7 defers it; a manifest plus stems covers the program
case). Sample playback stays out until the asset store (AV-MERGE §6).

### 7.3 Why this also serves ordinary users

- **Batch render**: bounce a patch or a folder of patches to WAV without opening a host.
- **Preset previews**: generate the browser's preview clips and loudness trims from the command line instead of ad-hoc tools.
- **CI golden clips**: the release pipeline renders every factory preset and compares hashes; today this lives in cargo examples.
- **Community submissions**: the gates in §6 item 7 run on a pull request without a person listening first.
- **Preset feature table**: attack time, loudness, centroid per preset, computed once; useful for search in the browser and for programs
  *(DCA §3)*.

### 7.4 An optional AI-patching interface without backlash

**Shape (Rec, following AIC §7's inference):** the synth binary contains no model, makes no network calls and collects no telemetry.
The interface is the CLI above plus an optional MCP server as a separate binary. Any model the user chooses can drive it: their own key
or a local model. Ops written through the interface are tagged with their source in the op log (the format already has a `source` field,
DCA §5), so every such edit is visible, reversible and attributable. Factory content is declared human-made.

| Evidence | Source and tag |
|---|---|
| No large backlash against a specific DAW AI feature was found; reactions are mixed and mild | R2 §9 |
| FL Studio's Gopher "alarmed some FL Studio users" over data security at launch | R2 §9, confirmed as extract |
| Users wish AI would teach rather than generate ("…Help Me Get Better at Making Music, Not Just Make It for Me") | R2 §9 |
| "Here's what we feared: the entirely AI synth"; a UI that "screams AI slop" | R2 §9 |
| Tolerated pattern: optional, transparent, output is an ordinary editable patch, nothing leaves the machine by default | R2 §9, inference |
| Ardour bans LLM-generated code contributions but accepts LLM-related functionality and plans an MCP server; the thread is sharply divided | R2 §9 |
| The leading Ableton MCP bridge has about 3.4k stars, and ships telemetry on by default | AIC §7, confirmed |
| Intel's Audacity AI plugins run "100% locally" as a separate optional plugin | AIC §7, confirmed |
| Ableton reportedly announced Live 13 with no AI features | R2 §9, single source |
| Bandcamp: "Music and audio that is generated wholly or in substantial part by AI is not permitted"; "substantial part" undefined; removal on suspicion | R2 §1, confirmed |
| Spotify and DistroKid do not ban AI music and use AI credits; Deezer tags fully-AI tracks | R2 §1, confirmed / partly confirmed |
| One text-to-patch request costs on the order of $0.001 on a small hosted model, so bring-your-own-key is viable | AIC §7–8, parametric estimate from confirmed prices |

**Risks to state plainly.**
- Any mention of "AI" can attach the "slop" label to the synth itself. Keep it out of the pitch and the UI; document it as "scripting and
  automation interface", with MCP as one client.
- Users who publish music made this way on Bandcamp carry a removal risk under the policy wording *(R2 §1, inference)*. Say so in the docs.
- Community objections to expect, from general open-source patterns rather than synth-specific evidence: bundled or default-on AI,
  network access from an audio plugin, AI-made presets presented as hand-made, AI-written core code *(AIC §7, inference)*. kabl's
  `CONTRIBUTING.md` already requires disclosure of AI tool use; add a short written policy covering the four points.
- No case of an open-source audio project forked over an AI feature was found; that is not established as absent *(R2 §9, gap)*.
- `PRODUCT-PLAN.md` §7 defers "AI text/audio-to-patch generation". This plan keeps that: kabl ships an interface, not a generator.

---

## 8. Sustainability

**Realistic options, in the order to try them (Rec):**
1. **Free download with an optional payment for the same thing** (Bespoke pattern). Zero adoption friction; compatible with Flathub and
   AUR, where builds are free anyway *(SAC §6, inference)*.
2. **Recurring donations** through one platform, linked from the app's About screen and the release notes. Donations scale with audience
   and with visible, regular communication *(SAC §6, inference)*.
3. **One grant application** for a bounded piece of work (for example the multiplatform release or accessibility). Precedent: NLnet
   funded cables.gl *(AVM, snippet)*. Whether kabl qualifies is unverified.
4. **Later, paid content**: expansion preset or wavetable packs licensed separately from the code (Vital pattern). A repackager cannot
   trivially remove that reason to pay *(SAC §6, inference)*. Only after the free factory set is no longer "small".

**What not to expect.**
- A salary. Ardour needed a DAW-sized user base and over a decade; the typical Patreon campaign earns very little *(SAC §6)*.
- Income from Linux users specifically: one commercial developer reports "not a single Linux license has been claimed" *(SAC §7, snippet)*.
- Volunteers building core components: "you can't rely on volunteers to build key components of a large open source project"
  (Meadowlark post-mortem) *(SAC §6, snippet)*.
- Revenue from gating formats or the plugin build: Cardinal shows it will be repackaged for free *(SAC §6)*.

**Do not** put official binaries behind a paywall, sell tiers of the program, or move updates to paying members. All three conflict with
the adoption goal and with the Vital lesson. Note also that SignPath's free signing requires no dual-licensing *(R2 §10)*, which rules out
a commercial second licence while kabl relies on it.

---

## 9. Risks

| Risk | Evidence | Mitigation |
|---|---|---|
| Solo frontend overload | Meadowlark's author "severely underestimated" the frontend and paused *(SAC §3, snippet)* | No second editor UI; rack stays the only editor; node view stays late |
| Windows/macOS editor bugs sink 1.2 | Documented baseview hazards *(PPT §3)* | Part 1 preview builds early; one host per system as the gate; decide rebase vs own fixes first |
| Signing blocked from Serbia | Azure unavailable; SignPath and Certum acceptance unverified *(R2 §10)* | Apply right after 1.0; keep Certum as fallback; ship unsigned previews meanwhile with clear instructions |
| Launch lands with a session-breaking bug | *(SAC §3)* | Acceptance sitting first; 1.0.1 ready inside the window |
| "Small" first impression | §3.4 | Do not compare; lead with explanation and sequencing; publish preset growth |
| Infinite occupies the AV position | *(R2 §6)* | Test it; position on synth quality and structure-exact visuals; no "first" claims |
| AV work starves audio | Combined estimates in `docs/v2/README.md` | AV gated on S1 and M1; flagged branch; separate worktree |
| wgpu and framework churn | Major wgpu release every 3–4 months; NIH-plug frozen *(PPT §3, §4, confirmed)* | Pin versions; one agreed pin across kabl and synteza; migrate on a schedule, not ad hoc |
| CLAP expression features work in one host only | *(R2 §3)* | MPE first; poly-mod labelled as Bitwig-only |
| Evidence is thin | Every external number here comes from search extracts; no page was read in full (R2 method note) | Treat §3 as direction; re-check any figure before printing it publicly |
| Format break after 1.0 | Loader is strict equality on schema 8 (**Fact**) | Migration chain before any schema change |
| AI label damages reputation | §7.4 | Interface only; separate binary; written policy |

---

## 10. Metrics without telemetry, and targets as hypotheses

| Metric | How to collect | Caveat |
|---|---|---|
| Release downloads per asset | GitHub releases API `download_count` (the notes name this field for Surge) *(R2 §4)* | Counts downloads, not users; distro packages bypass it |
| Flathub installs | Shown on each app's Flathub page *(R2 §4)* | Counts artifact downloads, not machines *(SAC §5)* |
| Stars, forks, watchers | Repository page | Developer interest, not musician use |
| Issues and discussions opened by people other than the owner; time to first reply | GitHub | The best free signal of real use (**Rec**) |
| Preset and module submissions | Count of merged community patches and `.kablmod` packages | Needs §6 item 8 first |
| Third-party mentions | Manual monthly search: KVR thread replies, KVR "favourite"/"want" counters (Vital shows 411 and 258 *(R2 §4)*), videos, list entries | Manual, about an hour a month |
| "How did you hear about kabl?" | One optional line in issue and discussion templates | Self-selected |
| Crash and audio-failure reports per release | Issues with a label | Under-reports |

No baseline exists: no published user or download totals were found for Vital, Surge XT, Dexed, Cardinal or VCV Rack *(R2 §4,
unverified)*. The targets below are therefore **hypotheses set by this plan**, phrased as events that either happen or do not. Replace
them with numbers after 30 days of real data.

| Hypothesis | Checked at | If false |
|---|---|---|
| H1. At least one of the five news sites in §5.2 posts about 1.0 | 30 days after the tag | The pitch or the demo is wrong; rewrite before the 1.2 launch |
| H2. People other than the owner open issues or discussions in the first month, and at least one is about making music, not installing | 30 days | Installation is the barrier; fix packaging before features |
| H3. At least one patch made by someone else is shared publicly | 90 days | The submission route or the docs are missing; do §6 items 8–9 |
| H4. At least one video about kabl is made by someone the owner did not ask | 90 days | Seed educators again with the 1.1 build |
| H5. Downloads of the 1.2 Windows and macOS installers exceed Linux downloads of the same release | 30 days after 1.2 | Reach evidence did not transfer; investigate before investing further in formats |
| H6. A stranger recommends kabl in a thread where the owner is not present | by 1.2 | The differentiators are not felt; run five observed first-use sessions |

---

## 11. What NOT to do

- Do not add Surge-parity features before the 1.0 tag, and do not delay the launch for features.
- Do not ship a schema change before the migration chain.
- Do not make poly-mod or CLAP note expressions a baseline; do not skip MPE.
- Do not write a second native plugin implementation; wrap the CLAP.
- Do not start kabl AV code before synteza S1, and do not merge AV code into an audio release before M1's gate.
- Do not make synteza depend on kabl, and do not have a synteza runner load `.kabl` files.
- Do not bundle Shadertoy or MilkDrop community content, or any shader pack, without per-item licence clearance.
- Do not put a model, a network call or telemetry in the synth binary.
- Do not add accounts, tiers, paywalled binaries or members-only updates.
- Do not grow presets without the gates; do not chase 2,000.
- Do not build a song timeline, piano roll, node view or marketplace now.
- Do not print any external number from §3 publicly without re-checking its source.
- Do not make the public claims listed in §4.4.

---

## 12. Decisions needed from the owner

| # | Decision | Recommendation |
|---|---|---|
| 1 | Move A4 ahead of A2/A3, split in two, with 1.2 as the multiplatform release (§5.5) | Yes |
| 2 | Run A4 part 1 (preview builds) in parallel with A1 | Yes, if a second agent stream is acceptable |
| 3 | Which machines exist for testing macOS and Windows, and whether to pay the Apple Developer Program fee | Needed before P2b; availability in Serbia must be checked |
| 4 | Apply to SignPath Foundation after the 1.0 release; accept the Foundation as named publisher | Yes; Certum as fallback |
| 5 | Wrapper route (clap-wrapper) for VST3 and AU instead of the framework's VST3 export | Yes |
| 6 | Rebase onto current nice-plug, or keep the vendored copy and own the fixes | Decide from a one-week spike at the start of P2b |
| 7 | `kabl render` and `kabl modules --json` in 1.0.x | Yes |
| 8 | An optional MCP server as a separate binary, later, with a written AI policy | Yes to the policy now; server after §7.2 items 1–5 |
| 9 | Community home (GitHub Discussions) and whether to open a chat room | Discussions yes; chat only if the owner will be present |
| 10 | Funding: optional payment and one donation platform at launch | Yes; no paid tiers |
| 11 | Community patch repository with CI gates, and the licence for submitted patches | Yes; licence to be chosen with `ASSET-LICENSES.md` |
| 12 | The pre-M1 event sidecar export as the first AV-facing step | Yes, once synteza freezes the format |
| 13 | Accept the "do not claim" list (§4.4), including retitling `docs/v2/README.md` | Yes |

---

## 13. First 30 days: work orders

Each row is sized for one agent work order unless marked **owner**. The separate 1.0 session (promo video v2, small fixes) keeps its work;
nothing here duplicates it.

| ID | Work order | Output | Done when |
|---|---|---|---|
| W1 | **owner** Acceptance sitting (`docs/ACCEPTANCE-V1.md`) | Ticked checklist, list of blockers | Verdict line filled in |
| W2 | SDK port-limit fix: make `MAX_INPUTS` and SDK `MAX_PORTS` agree; test both sides | PR | An SDK-valid 16-port module compiles or is rejected by the SDK with a clear error |
| W3 | Version bump, `CHANGELOG.md`, rc tag to prove `release.yml`, README/STATUS tidy, then `v1.0.0` | Release page with artifacts | A clean machine installs from the tarball and plays the first-run sound |
| W4 | Launch kit: one-sentence pitch, press note, five audio clips, walkthrough script; KVR product page text | `docs/launch/` folder | Owner approves the text; clips pass the loudness gate |
| W5 | AUR package and a Flathub submission plan (app + plugin extension, runtime branch, required permissions) | PKGBUILD; a written plan | AUR package installs standalone and `kabl.clap`; Flathub plan lists every blocker |
| W6 | **owner** Launch day: posts and emails per §5.2 | Links recorded in `docs/launch/LOG.md` | All same-day channels posted |
| W7 | `kabl render` subcommand from `render_midi.rs`, with JSON report | PR + docs | CI renders every factory preset through it; hashes match the golden tests |
| W8 | `kabl modules --json` | PR + schema doc | Output lists all 53 kinds with ports and params; a test fails if a kind is missing |
| W9 | Migration chain design note and skeleton: `vN → vN+1` functions, fixture files per released schema, test | Design doc + PR with v8 fixture | Test opens the v8 fixture; adding v9 without a migration fails CI |
| W10 | Test Infinite on Linux: install, build one audio + video patch, record CPU, stability, what it does and does not do | Half-page report in `docs/av-audit/` | Report states whether its Linux build works and names three things kabl's AV side must do differently |
| W11 | SignPath Foundation application draft: signing policy page, CI evidence, the section 7 / licence-grant question | Draft + `docs/CODE-SIGNING.md` | Owner submits it after the 1.0 release exists |
| W12 | Metrics script: release download counts, stars, external issues/discussions, written to a dated CSV | `tools/` script + first snapshot | Running it on day 7 and day 30 fills the H1–H2 table |
| W13 | Community setup: Discussions categories, issue templates with "how did you hear", AI policy paragraph, public roadmap page | PR | Templates live; roadmap page matches §5.1 |
| W14 | 1.0.1: fixes from launch feedback, preset defects from the CI scan, polyBLAMP triangle if ready | Release with changelog | Published within the 30 days |
| W15 | Preset "how this works" notes for the 100 factory presets, checked against their macros | PR | Every preset shows a note in the browser; a test fails on a missing note |
| W16 | Update AV-MERGE §11 and VIDEO-CORE §2.1 to match §1.4 and §5.7 of this plan | PR (docs only) | The three docs no longer contradict the brief |

Order: W1–W3 first; W4–W6 with the tag; W7–W9 and W12–W13 in the first two weeks; W10–W11 and W14–W16 in the second half.
