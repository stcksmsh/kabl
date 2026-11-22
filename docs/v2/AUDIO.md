# kabl V2 — audio side: what it has, what it needs to beat Surge XT

Date 2026-11-22. Status: analysis + proposal, not owner decisions. Source audit of `master` (`788dafc`).
Paths are relative to the repo root. Surge XT facts are from general knowledge; re-check before quoting publicly.

## 0. Verdict in five lines

- The DSP is careful and often textbook: TPT/ZDF filters, PolyBLEP, mipmapped wavetables, ADAA drive, oversampled FM, FTZ/DAZ,
  allocation-free callback enforced by tests, deterministic seeded renders, CI audio-defect scan.
- The *breadth* is narrow: 8 fixed voices, MIDI-only (no MPE / note expressions / poly-mod), block-rate knob modulation,
  4 filter families, 1 reverb, 8 wavetables, 100 presets, Linux + CLAP only.
- The *architecture* already beats Surge where Surge cannot follow: a real modular graph with audio-rate cables and feedback,
  live edits with crossfaded graph swaps, sandboxed WASM code modules, composites with custom panels, built-in sequencing
  (seq, drum grid, arp, cues, functional cables).
- "Beat Surge" therefore means: close the per-voice sound and expression gap (A1–A3), go multiplatform (A4), grow content (A5).
  Do not chase Surge's feature count; chase "a Surge user would not miss anything they actually use, and gains a modular".
- Biggest single levers: configurable polyphony, per-sample modulation, MPE / CLAP note expressions, filter + shaper families
  with shared oversampling, macOS/Windows builds.

## 1. Inventory (what exists)

### 1.1 Oscillators
| Module | What it does | Quality notes |
|---|---|---|
| `osc.va` (`builtins/osc_va.rs`) | saw, pulse (PW 5–95 %), triangle, sine; hard sync; unison 1–16 with detune curve up to JP-8000 supersaw spread, stereo width | PolyBLEP saw/pulse, band-limited sync with lookahead. **Triangle is naive** (file header: polyBLAMP not implemented). Exponential FM via `pitch` only; no linear / through-zero FM input |
| `osc.fm` | 1 sine operator, phase modulation, feedback | 2× (opt. 4×) internal oversampling; 10-sample latency per hop, not reported to host |
| `osc.fm6` | 6-op, 8 DX algorithms, per-op ADSR + velocity, op-6 feedback | whole voice 2×/4×; ~40 ns + 17 ns/op/sample |
| `osc.wt` | 1–64 frames × 2048, audio-rate `pos`, `.wav` import, table editor (`tableedit.rs`) | 10 FFT mip levels, linear interp; **mono, no unison; 8 factory tables** |
| `noise` | white, pink | seeded per voice, reproducible |
| drums | kick, tom, snare, clap, hat (808/909-ish) + `drum.grid` | declicked retrigger |

Missing vs a modern hybrid: string / physical model (only Karplus via `filter.comb`), macro oscillators (Plaits-style), window/formant osc,
additive, sampler, granular.

### 1.2 Filters
| Module | Topology | Nonlinear | Gaps |
|---|---|---|---|
| `filter.svf` | TPT SVF; LP/BP/HP + notch | no | no drive, no oversampling |
| `filter.ladder` | 4× TPT one-pole + ZDF feedback | tanh input, cheap estimate | LP24 only; not oversampled |
| `filter.comb` | Hermite-interpolated delay, damped loop | no | — |
| `filter.formant` | 5 parallel TPT BPs, vowel morph | no | — |
| `vocoder`, `eq` | 8–32 bands; 4-band Simper f64 | — | — |

No key-tracking parameter (patch it via `pitch` → attenuverter). Missing families: diode, K35/MS-20, OB/SEM nonlinear SVF,
12 dB ladder + HP/BP ladder modes, cutoff-warp/tri-pole style, filter FM input, per-voice waveshaper.

### 1.3 Modulation
- `env.adsr`: exponential only, no delay/hold, no curves, no loop. `lfo`: 5 shapes, free/clock sync, per voice.
  Plus random, S&H, slew (doubles as follower), quantizer, logic, comparator, 8 macros, cue morphs (`engine/src/morph.rs`).
- **Cables into ports are audio rate. Cables into parameters ("routes") are evaluated once per 64-sample block**
  (`engine/src/compile.rs` header). Audio-rate modulation exists only where a module exposes a CV port.
- Voice-rate → global input is **averaged** across voices. All FX are global ⇒ no per-voice effects.
- `QualityConfig {Live, Studio, Render}` is a stub; only `Live` is used.

### 1.4 Voices and expression
- **8 voices fixed** (`standalone/src/lib.rs:125 DEFAULT_VOICE_COUNT`, `clap/src/lib.rs:30 VOICES`); keyboard caps at 16.
- Steal order: longest-released free → oldest sustain-held → oldest. Steal = one-sample gate dip, no fast-release fade.
- POLY/MONO/LEGATO, priority, glide incl. poly glide. Bend 0–24 st (per keyboard). Mod wheel, sustain. Channel splits per `midi.in`.
- **No MPE, no channel/poly aftertouch, no microtuning, CLAP note port is MIDI dialect only (no note expressions, no poly-mod).**

### 1.5 Effects
Dattorro plate reverb (one algorithm); ping-pong delay (≤4 s, clock sync 1/16–1/4); BBD chorus; phaser 2–12 stages; flanger; compressor
with sidechain; limiter (no lookahead, by design — latency is fixed); drive (tanh + 1st-order ADAA, no oversampling); bitcrusher; ringmod; EQ.
Missing: hall/room/spring reverbs, frequency shifter, tape/wow-flutter, rotary, multiband, shaper library, effect buses.

### 1.6 Engine
- Fixed 64-sample block; plugin reports fixed 64-frame latency (`engine/src/timeline.rs`).
- FTZ/DAZ on x86_64 + aarch64 (`engine/src/ftz.rs`). No SIMD (S3 spike reached 1.5–1.7×, removed). `Box<dyn Module>` ~17 % dispatch.
- Oversampling only inside `osc.fm`/`osc.fm6`; no shared infrastructure.
- Compiler hard limits `MAX_INPUTS = 4`, `MAX_OUTPUTS = 8` (`compile.rs:91-95`) — **but the SDK allows 16 ports per direction**
  (`sdk/kabl-module-sdk/src/meta.rs:27`): an SDK-valid code module can fail to compile. Fix before the SDK leaves preview.
- Code modules run on wasmi (interpreter, no JIT): a ceiling for user DSP.
- Bench (i7-13700H, 48 kHz): `osc.va` saw ~10–13 ns/sample/voice; factory palette patches at 8 voices use 8–10 % of the block budget
  (extrapolated: polyphony 32–64 is affordable for typical patches — measure).
- Known open: ordinary REAPER renders not bit-identical; dense two-editor session ERR=2 and a 5.49 ms worst call vs 5.33 ms period.

### 1.7 Sequencing and performance (kabl's home ground)
`seq` (32 steps, banks A–D, velocity, probability, ratchets), `drum.grid` (8×16, polymeter), `arp` (global), `clock` + swing, `clock.div`,
`cues` (8, with macro morphs), 8 macros, functional cables (pattern, probability, A/B morph, glide, routes into cable params), Perform view.

### 1.8 Content, host, platform
- 100 presets (`patches/presets`: Keys 16, Groove 15, Bass 14, Pad 14, Lead 12, Pluck 10, Piece 9, FX 5, Voice 5) + ~47 demos.
  Loudness gate (−19 dBFS RMS ±5.5, peak ≤ −1) enforced by test; CI defect scan found real clicks in some (`docs/presets/AUDIO-DEFECTS.md`).
- CLAP via vendored nice-plug strict profile: params (16 automation slots + gain + host clock), state, latency, tail, render, note-ports, track-info.
  No VST3/AU/LV2 (nice-plug's VST3 wrapper exists but is disabled).
- Portability: no hard Linux lock-in. Linux-only code is `cfg`-gated (dbus, `sched_setscheduler`, portals). Real blockers:
  - XDG paths read from `HOME` (`library.rs`, `style/load.rs`, `applog.rs`, `filebrowser.rs`); Windows has no `HOME`.
  - CI and release run only on ubuntu-24.04.
  - Plugin is referenced as `.so`.
  - No macOS bundle, signing or notarisation; no Windows installer.

## 2. Gap table vs Surge XT

| Area | Surge XT | kabl | Status | Plan |
|---|---|---|---|---|
| Polyphony | up to 64 | 8 fixed | missing | A1.1 |
| Modulation resolution | per-sample-smoothed matrix | audio-rate cables, block-rate routes | partial | A1.2 |
| MPE / note expressions / poly-mod | yes | no | missing | A1.3 |
| Aftertouch | yes | no | missing | A1.3 |
| Microtuning (.scl/.kbm, MTS-ESP) | yes | no | missing | A1.4 |
| Oscillator types | 12 incl. String, Twist, Window, Alias | VA, PM-FM, FM6, WT, noise, drums | partial (FM6 > Surge FM2/3) | A2.1 |
| Anti-aliasing | BLIT-style + oversampling | PolyBLEP (tri naive), FM oversampled | partial | A0, A2.3 |
| Wavetables | hundreds | 8 + import + editor | partial | A2.1, A5 |
| Filters | ~30 models | 4 families | missing most | A2.2 |
| Waveshaper | ~40 shapes per voice | global tanh drive | missing | A2.2 |
| Envelopes | DAHDSR, curves, analog mode | exp ADSR | partial | A2.4 |
| LFO / MSEG / formula | 12/scene, MSEG, Lua formula | 5-shape LFO, unlimited count | partial | A2.4 |
| Effects | ~30 types, buses | 12, global | partial | A3 |
| Presets | ~2000+ | 100 | missing | A5 |
| Platforms / formats | Win/mac/Linux; VST3/AU/CLAP/LV2 | Linux; CLAP | missing | A4 |
| Host parameter exposure | all | 16 slots | partial | A4.4 |
| Modular graph, feedback, live swaps | no | yes | **kabl ahead** | — |
| User DSP | Lua formula only | sandboxed WASM modules + SDK | **kabl ahead** | A2.5 |
| Sequencing / cues / functional cables | no | yes | **kabl ahead** | — |
| Composites + custom panels | no | yes | **kabl ahead** | — |

## 3. Plan

Ordering principle: everything that changes *how every patch sounds or plays* (voices, modulation resolution, expression) comes before
breadth, because breadth built on block-rate routes and 8 voices has to be revisited later.

### A0 — finish 1.0 (prerequisite)
1. Merge or close PRs #171 #172 #187 #188; tick `ACCEPTANCE-V1.md`; tag `v1.0.0`; publish the release the README promises.
2. polyBLAMP triangle in `osc.va`.
3. Fix `MAX_INPUTS` vs SDK `MAX_PORTS` mismatch (raise engine limit or clamp SDK; test both).
4. Root-cause the ordinary-render nondeterminism (`docs/host-production/design.md`); deterministic renders are a selling point.
5. Fix preset defects found by the CI scan (Deep Field, Rubber Band).
6. **Format compatibility path**: `check_schema` accepts only exactly v8 (`core/src/format.rs:97`). Before any v9, add a
   migration chain and a test that opens every earlier released schema. After 1.0, old songs must always open.

### A1 — voices and expression (the "plays like a modern synth" tier)
1. **Polyphony**: per-patch voice count 1–64 (default 16), stored in the patch; voice-count change is a structural recompile.
   Envelope-aware stealing (quietest released voice first), and a 1–3 ms fast-release fade on steal instead of a gate dip.
   Measure CPU on factory patches at 16/32/64 and add to `docs/benchmarks.md` + the perf gate.
2. **Per-sample route modulation**: routes stay computed per block, but the target parameter is linearly ramped from the previous
   block value to the new one across the block (zero-latency, cheap), with an opt-in "audio-rate" flag for routes whose target module
   has a native CV port. Removes zipper/stepping, the most audible gap vs Surge.
3. **Expression**:
   - Per-voice pitch bend, pressure and timbre (CC74) as `midi.in` outputs.
   - MPE zones (lower/upper, per-note channel) and channel + poly aftertouch.
   - CLAP note expressions (volume, pan, tuning, vibrato, expression, brightness, pressure) on the same outputs.
   - CLAP polyphonic modulation for the exposed automation slots.
   - Non-MPE hosts keep working unchanged.
4. **Microtuning**: `.scl`/`.kbm` per patch (data embedded like wavetables), applied in `midi.in` → `pitch`; quantizer gains scale-from-tuning;
   MTS-ESP client later (licence check needed).
5. Release-velocity, note-off velocity outputs (cheap, completes MIDI 1.0 coverage).

### A2 — sound palette
1. **Oscillators**:
   - unison and stereo for `osc.wt`; larger factory wavetable set (A5);
   - linear and through-zero FM input on `osc.va`;
   - a physical "string" oscillator (waveguide with exciter, better than comb);
   - a macro oscillator family (Plaits-style; Plaits code is MIT — port selected models and credit them);
   - later: sampler and granular (planned as V2 shared assets, see AV-MERGE §6).
2. **Filters + shapers**:
   - ladder modes (LP12/LP24/BP/HP) with proper nonlinear solve;
   - diode ladder, K35 (MS-20), nonlinear SVF (SEM/OB character);
   - filter FM input;
   - a `shaper` module with a curated set of ~20 curves (soft/hard clip, fold, rectify, asym, sine, Chebyshev), per-voice capable.
3. **Shared oversampling**:
   - an engine-level `Oversampled<M>` wrapper (2×/4×, polyphase halfband) usable by any nonlinear module;
   - drive, ladder, shaper and fold use it;
   - make `QualityConfig` real: Live = 1×/2×, Render = 4×, chosen per patch and switched automatically on offline render;
   - report true latency per patch.
4. **Envelopes and LFOs**:
   - DAHDSR with per-segment curve and an optional loop;
   - an **MSEG** module (breakpoints, curves, loop points, tempo sync, one-shot or LFO), editable on its face;
   - LFO shapes from the MSEG library.
5. **Formula/expression modules**: kabl already has WASM code modules. Add a tiny in-app expression module (`y = sin(x*t) …`)
   compiled to the same sandbox so users don't need a toolchain for one-liners. Evaluate a JIT (Cranelift/wasmtime "winch") for code
   modules later; keep wasmi as the RT-safe fallback.

### A3 — effects
1. Reverbs: hall/room (FDN), spring; keep the plate.
2. Frequency shifter, tape (saturation + wow/flutter), rotary, multiband compressor/saturator, graphic/tilt EQ.
3. Per-voice effects: allow `Rate::Voice` for cheap effects (drive, shaper, short delay, comb) — today all FX are global.
4. Limiter with lookahead once latency reporting is per patch (A2.3).

### A4 — multiplatform
1. Paths: replace hand-rolled XDG/HOME logic with the `directories` crate (or `dirs`), keeping Linux locations byte-identical.
2. CI matrix: ubuntu, macos (arm64), windows; build + test + clippy on all; GPU-free tests only.
3. Plugin formats:
   - CLAP bundles per OS (`.clap` bundle on macOS, `.clap` DLL on Windows);
   - VST3 through nice-plug's existing wrapper. The VST3 SDK licence changed in 2025 — verify the current terms before shipping;
   - AU via clap-wrapper;
   - LV2 only if users ask.
4. Host parameters: expose more than 16 slots (configurable automation map with stable ids, already the D08 design direction).
5. Packaging: macOS signed + notarised `.app` / `.pkg`; Windows MSI or Inno installer; keep the Linux tarball.
6. Standalone audio backends: JACK feature on Linux, ASIO optional on Windows (cpal supports both).

### A5 — content and proof
1. Presets 100 → 400+, same loudness and defect gates, plus a listening pass recorded per batch.
2. Factory wavetables 8 → 100+ (generated + licensed sets with clean licences in `ASSET-LICENSES.md`).
3. **Blind A/B vs Surge** on matched patches (bass, pad, lead, pluck, keys): same notes, loudness-matched, owner + 2–3 outside listeners,
   test-retest repeats (the afekt viewing protocol). Gate for claiming parity per category.
4. Performance gate keeps dense patches under the period at 32 voices on the reference laptop.

## 4. Things to *not* copy from Surge

- Two fixed "scenes" — kabl's patch is a graph; layering is just two voices into one mixer, splits are channel filters. Offer a "layer/split"
  template composite instead.
- A fixed mod matrix UI — cables + routes + macros are already more legible; add a "what modulates this" list on each knob instead.
- 2000 presets of variable quality — fewer, gated, each with a lesson and named macros.

## 5. Rough size (agent-weeks, one agent, excluding review latency)

| Batch | Estimate |
|---|---|
| A0 | 2–3 |
| A1 | 6–8 |
| A2 | 10–14 |
| A3 | 4–6 |
| A4 | 5–7 |
| A5 | ongoing, content-bound |

A1 and A4 are independent and can run in parallel worktrees.
