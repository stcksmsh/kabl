# Kabl AV: architecture plan, competition, and first-proof timeline

Part of the AV package: [audit](KABL_AV_PRODUCT_AND_ARCHITECTURE_AUDIT.md) · [decisions](KABL_AV_DECISIONS_AND_OPEN_QUESTIONS.md) ·
[roadmap](KABL_AV_ROADMAP_AND_SCOPE_GATES.md) · [mockup brief](KABL_AV_MOCKUP_BRIEF.md).

Date 2026-11-19. This is a recommendation for Kosta to accept, change or reject; it is not an owner decision.

## 1. The bet in one line

**Structure-reactive, not audio-reactive.** Almost every audiovisual tool drives visuals from an FFT or the loudness of
the finished mix. Kabl owns the patch, so its visuals can follow the music's *structure*: each envelope's attack, each
sequencer step, each oscillator's phase, each macro move, sample-accurately. No competitor in §2 offers that inside a
musician-grade synthesizer. Use it as the WOW and the differentiator.

## 2. Competition

Facts marked † come from the sources at the end. The others are from general knowledge and should be re-checked
before they are quoted publicly.

| Tool | What it is | Audio synthesis | Visuals | Extensibility | Lesson for kabl |
|---|---|---|---|---|---|
| VCV Rack 2 / Cardinal | Virtual Eurorack; Pro tier adds VST/AU/CLAP† | Excellent | Scopes only; paid visual-only modules are rare (Sosci oscilloscope, 2026†) | C++ plugins, unsandboxed, huge library | Rack UX is loved; no AV story; unsandboxed plugins crash hosts. Our sandbox is an advantage. |
| Bitwig Grid | Modular inside a DAW | Very good | None | Closed | "Modulators" UX: assign any source to any parameter without cables. Copy this for visual bindings. |
| Max/MSP + Jitter (+ Gen, RNBO) | General visual programming | Excellent | Strong but users say behind TD in ready-made shaders† | JS, Gen, C externals | Most capable incumbent, steep curve, paid, not an instrument out of the box. |
| Pure Data + GEM | Open-source Max cousin | Good | Dated | C externals | Free but UX-hostile; proves demand among open-source artists. |
| TouchDesigner | Visual-first node environment | Weak (CHOP analysis, not a synth) | Best-in-class | Python, GLSL | Free non-commercial capped at 1280x1280, paid tiers $300 / $600 / $2,200†. TD users bolt on Max or a DAW for sound†. That gap is our audience. |
| vvvv gamma | .NET visual programming | Weak | Strong | C#, shaders | Windows-centric; strong typed dataflow; power-user only. |
| cables.gl | Browser node patcher | None† | Good | JS ops | Shows that browser-grade onboarding works: fast first result. |
| Resolume (Wire), HeavyM | VJ software | None | Clip/effect based; HeavyM ships 1,000+ no-code effects† | FFL/ISF shaders | Non-power users want ready "looks", not graphs. |
| Synesthesia, vgalizer, Butterchurn | Audio-reactive shader players | None | Shaders fed RMS/FFT/beat† | One shader file per scene | **One shader file = one module** is the right dev story. Their input is a mix FFT; ours is patch structure. |
| Hydra | Browser live-coded video synth | None | Feedback/oscillator video synth | JS one-liners | A tiny set of composable primitives produces surprising results. Good model for the first visual modules. |

**Gap kabl can own:** an open-source instrument that is a real synthesizer first and in which the visuals are patched from
the synth's own signals and events. Users should not need a second app, OSC glue or an FFT guess.

## 3. Design goals, by user

| User | What they need | Mechanism |
|---|---|---|
| Musician, no visuals experience | A good look in one click that already moves with the music | **Looks**: visual presets that **auto-bind** to the patch by role (the main envelope, the clock, the loudest voice, the macros). This follows the existing principle "musical controls reveal their real targets": every auto-binding is visible and editable. |
| Musician, curious | Change what drives what without wiring | **Binding slots** on each visual parameter, Bitwig-modulator style: pick a source from a list or drag from any module output; amount and response curve per binding. Bindings show as edges in the node view and as a badge in the rack. |
| Power user | Freedom | **Node view** of the same patch: shader passes, feedback buffers, compositing, cross-domain converters, composites that cross domains. |
| Shader artist / dev | Write a visual module without Rust or a build | **One file = one visual module**: a fragment shader with a header that declares parameters and inputs, ISF-style (an existing open format with a large public shader library) or a kabl dialect of it. Hot reload on save, errors shown on the module face. |
| Audio dev | Make a module that also feeds visuals | An optional **telemetry declaration** on a module: named scalar and event outputs written into a fixed-size record each block. Built-ins first; code-module ABI later (D-21). |
| Core dev | Clear seams | Three layers that never call into each other's time domain (§4). |

Freedom and ease do not conflict here because both rest on the same model: Looks and bindings are saved patch data
built from the same parts a power user edits by hand. Nothing is a hidden second system.

## 4. Architecture

```
                          ONE PATCH (core: op log, ids, schema)
   modules · cables · bindings(new) · visual nodes(new) · per-view layout(new side-table)
          │ compile (control thread)                         │ compile (control thread)
          ▼                                                  ▼
 AUDIO DOMAIN (existing, untouched)          VISUAL DOMAIN (new, own thread → later own process)
 64-sample blocks, cpal / CLAP callback      frame loop (vsync), GPU, shader modules, feedback buffers
   modules write audio + telemetry record ─┐      ▲ reads latest snapshot + drains events,
                                           │      │ converts sample time → frame time
                                           ▼      │
                             BRIDGE (fixed-size, never blocks audio)
                 per-block scalar snapshot (latest wins) · event ring (drop-newest + counter)
                 audio ring for scopes/FFT (copied, analysed on the visual side)
                 clock pair (sample_pos, monotonic_ns) per block
```

Key choices (detail in the decision register):

1. **Audio engine untouched.** Telemetry is an extension of what face taps (`engine/src/facetap.rs`) already do: fixed-size,
   `Copy`, no allocation. FFT and analysis run on the visual side from a copied ring, never in the callback.
2. **Bindings are first-class patch data**, not cables. They map a telemetry source to a visual parameter with an amount, a curve and a
   smoothing/hold. In the node view they render as edges; in the rack as badges. This one decision is what makes the UX good for
   non-power users and keeps power users free.
3. **Three bridge kinds only** to start: *scalar* (level, phase, frequency, macro value, with last/min/max per block),
   *event* (gate rise/fall, envelope stage change, step, bar, note with sample position), *audio ring* (for scope/spectrum visuals).
   Structured values (colour, vector) are built from scalars inside the visual domain.
4. **Visual runtime renderer: wgpu**, recommended. It is cross-platform (Vulkan/Metal/DX12/GL) and is the egui-supported backend for later
   embedding. The proof may use glow/GLSL, already in the tree, if that is faster. Decide after the proof: S4 in the roadmap.
5. **Isolation.** Release builds use `panic = "abort"`. A sidecar process (`kabl-visual`) behind the same bridge protocol isolates GPU and
   driver faults and is also the clean seam for Sinteza later. Start in-process for the proof, move out before V1.1 if S4 confirms the risk.
6. **Shader modules run in the GPU sandbox only.** They cannot touch the audio path. A hung shader can at worst stall the visual process.

## 5. First real audible and visible output: the proof

Goal: a clip that makes someone say "wow". The visuals visibly follow the patch's envelopes, steps and phases, not just
loudness. No UI polish needed.

**Content.** Use an existing factory piece with interlocking sequences, the Tangerine-Dream-style north star from
`PRODUCT-PLAN.md`, so the polyrhythm itself becomes visible. Three looks, each a single shader:
- *Bloom rings*: every envelope attack spawns a ring per voice, decay shapes its fade, release dissolves it.
- *Phase geometry*: oscillator phase and wavetable position draw a morphing Lissajous/curve form.
- *Step lattice*: each sequencer lane is a row; steps light on the exact sample, with probability-skipped steps shown as ghosts. A macro moves filter cutoff and glow together.

### Track A: offline clip (fastest, deterministic, fits house rules)

The audio-sink rule and the "judge taste on real-app clips" rule both favour offline. Pieces already exist:
`crates/engine/examples/render_midi.rs` renders a patch plus MIDI to WAV through the production timeline adapter, and
`tools/examples/live_displays_clip.rs` renders frames on a fixed 60 fps clock to PNG plus ffmpeg.

| Step | Work | Agent days |
|---|---|---|
| A1 | `tools/examples/av_proof.rs`: render the patch block by block (as `render_midi`); every 800 samples (60 fps at 48 kHz) read face-tap values (level, stage, cycle) and step/gate edges | 1 |
| A2 | Headless renderer: 2–3 fragment shaders fed those values, written to PNG frames (glow under xvfb, as the existing clip tools do, or wgpu offscreen) | 1.5 |
| A3 | Mux WAV + frames to MP4/WebM with ffmpeg; 1080p, 30–60 s per look | 0.5 |
| A4 | Tune looks on 2–3 factory pieces; contact sheet; send clips | 1 |
| | **Total** | **≈ 4 agent days**, plus one Kosta review |

No production code changes. It lives in `tools/` (outside default members), throwaway or reusable.

### Track B: real-time, playable

| Step | Work | Agent days |
|---|---|---|
| B1 | Bridge prototype (spike S1): telemetry snapshot + event ring from the running engine; `assert_no_alloc` test; overflow counters | 2 |
| B2 | Visual window: winit + wgpu (or a second eframe viewport) running the Track A shaders live; fullscreen on a chosen display | 2–3 |
| B3 | Timing (spike S2): sample→frame conversion, measured p50/p99 latency and jitter; 10-minute soak with xrun count vs baseline | 1.5 |
| B4 | Isolation check (spike S4): kill or hang the visual thread or process; audio keeps playing | 1 |
| B5 | Play it live with a MIDI keyboard; record a screen capture | 0.5 |
| | **Total** | **≈ 7–8 agent days** after Track A |

### Calendar estimate

- **Track A clip:** about 1 week from start, including one review round.
- **Track B playable proof:** about 2.5–3 weeks from start, including reviews and a one-third risk buffer. The main risks are GPU/driver behaviour
  on Kosta's laptop and timing jitter.
- **Constraint:** the WOW roadmap says "one kabl agent at a time" and V1.0 comes first. These dates hold only if Kosta allows one proof agent
  in parallel. The proof touches only `tools/` and a throwaway branch, so it does not collide with V1.0 work. If it waits until V1.0 is
  tagged, add the remaining V1.0 time in front.
- **Not in the proof:** node view, bindings UI, saving AV patches, Looks browser, plugin, video files. These are V1.1 work from the roadmap,
  and they start only after the proof shows the bet lands and V1.0 ships.

## 6. What the proof decides

| Question | Answered by |
|---|---|
| Does structure-reactive read as clearly better than FFT-reactive? | Track A clips, side by side with an FFT version of the same look |
| Is the bridge free for audio? | B1/B3 numbers |
| In-process or sidecar? | B4 |
| glow or wgpu? | A2/B2 effort and frame times |
| Which telemetry sources matter most? | Which looks Kosta keeps; this sets V1.1's fixed source list |

## Sources

- [VCV Rack product page](https://vcvrack.com/Rack) · [Sosci for VCV Rack (community thread)](https://community.vcvrack.com/t/introducing-sosci-the-super-realistic-software-oscilloscope/25658)
- [TouchDesigner products and prices](https://docs.derivative.ca/TouchDesigner_Products) · [TouchDesigner Non-Commercial](https://derivative.ca/UserGuide/TouchDesigner_Non-Commercial)
- [Cycling '74 forum: Jitter vs TouchDesigner](https://cycling74.com/forums/curious-about-your-opinions-on-jitter-vs-touch-designer-vs-unreal-vs-unity)
- [CDM on cables.gl](https://cdm.link/cables-gl-free-generative-visual-tool-adds-skin-and-bones-new-ui-new-fx-and-media-recording/)
- [HeavyM: best software for live music visuals](https://www.heavym.net/best-software-for-live-music-visuals/) · [vgalizer (Rust visualizer)](https://gh.r-mj.com/Hornfisk/vgalizer)
- [2026 creative-coding tool overview](https://www.youngju.dev/blog/culture/2026-05-16-ai-creative-coding-visual-art-2026-processing-p5-openframeworks-hydra-touchdesigner-notch-cables-tres-deep-dive.en)
