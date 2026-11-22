# kabl V2: video side — turning synteza into the video core

Date 2026-11-22. Status: analysis and proposal, not owner decisions. Audits synteza `master` (`fccad14`).
Paths are prefixed `synteza/` or `kabl/`. Lumen facts come from general knowledge; re-check them before quoting publicly.

## 0. Verdict

**synteza gives us shaders, ideas and prior art, not a video engine.** The Rust side is an offline proof of concept:
- One hard-wired look (`synteza/crates/hyst-render/src/renderer.rs` `FrameParams`: Julia → memory field → persistence → bloom → composite → beam).
- 8-bit textures everywhere (`gpu.rs:125`, `Rgba8Unorm`). This silently clamps the HDR feedback the shaders were written for: `memory_field` clamps to [0, 64] and composite tonemaps.
- About 10 runtime GLSL compiles and 11 queue submits per frame, with pipelines rebuilt on every draw.
- Blocking readback, no window, no surface.
- wgpu 22. kabl already locks **wgpu 29** through egui-wgpu 0.35 (`kabl/Cargo.lock`, `kabl/vendor/egui-baseview/Cargo.toml:32`).
- The `.hyst`/ISF parser works, but nothing runs it. It also rejects image inputs, persistent buffers and multipass, which are exactly what a video synth needs.

**Keep:**
- the shader bodies (memory field, persistence, bloom, tonemap, Julia with perturbation, Mandelbulb, beam);
- the "hysteresis" design rule: drive parameters through springs and integrated rates, never raw features;
- the audio analysers;
- the TS patch graph design (typed timescales, CV-overridable params, per-consumer pruning);
- the DMX, Art-Net, sACN and ILDA encoders.

**Rewrite:** the runtime, as a frame-graph of typed modules.

**Target:** a Lumen-class video synthesiser.
- Small primitives patched in kabl's rack: oscillators, shapes, feedback, colourise, key/mix, displace.
- synteza's composed scenes on top as factory "Looks".
- Multiplatform via wgpu (Vulkan, Metal, DX12).

## 1. What synteza contributes, piece by piece

| Piece | Where | State | Decision |
|---|---|---|---|
| GPU setup, offscreen targets | `hyst-render/src/gpu.rs` | default device, no features, RGBA8 only, blocking readback that needs `width*4 % 256 == 0` | **rewrite** (pool, RGBA16F, async readback, surface) |
| Fixed renderer | `renderer.rs` | one look, ~10 full-res targets | **delete**, replaced by graph scheduler |
| Memory field (curl advection, fold, decay) | `passes/memory_field.rs` | self-contained, needs HDR | **port** as `fx.feedback.flow` module |
| Persistence | `passes/persistence.rs` | trivial | **port** as `fx.trails` |
| Bloom / blur | `passes/bloom.rs` | single level, 9-tap | **port** and upgrade to a mip cascade (the TS original had one) |
| Composite / tonemap | `passes/composite.rs` | add + Reinhard + gamma | **split** into `mix.add` and `out.tonemap` |
| Beam (instanced polyline) | `passes/beam.rs` | good | **port** as `gen.path` (scope, Lissajous, laser preview) |
| Julia (perturbation, deep zoom) | `passes/julia.rs` (1675 lines) | shader is good; driver mixed in | **split**: `gen.julia` (shader) + `ctl.julia.pilot` (control module) |
| Mandelbulb | `passes/mandelbulb.rs` (2568 lines) | 86–111 ms/frame at 1024 px (own measurement) | **port late**, with a resolution/quality param |
| `.hyst`/ISF parser + translator | `hyst_format.rs`, `isf_translate.rs` | parser tested; translator emits combined samplers, which naga likely rejects (unverified); no runtime | **reuse the parser**, rebuild the runtime (§4) |
| Audio features | `hyst-audio` | FFT, bands, centroid, flatness, chroma, flux, beat PLL; allocates on the audio thread, takes a mutex | **port** as kabl `analyze.*` modules on a non-RT thread (AV-MERGE §4.4) |
| Build/drop/break detectors | TS only, `src/audio/worklet/brain/` | not ported | **port later**; only for external audio |
| SignalBus | `hyst-core/src/signal.rs` | one struct with 35 fields | **dissolve** into named module outputs; keep `TimescaleTag` as port-rate metadata |
| Spring-damper, dt smoother | TS `choreography/`, `conductor/dt-smoother.ts` | sound | **port** as `ctl.spring` and `ctl.integrate` control modules (substep when dt·ω > 1) |
| Easing curves | `hyst-choreo/src/curve.rs` | 15 tests | **port** into a `ctl.curve` shaper |
| TS patch graph | `src/render/conductor/patchgraph/` | scalar-only, validated, typed timescales | **design prior art**; kabl's graph replaces it |
| Lua scripting | `hyst-script` (mlua) | not sandboxed (`Lua::new()` loads io/os), not wired to anything | **drop for now**; kabl's WASM sandbox and shader modules cover it |
| DMX/Art-Net/sACN/WLED/Enttec, ILDA/Ether Dream, OSC | TS `src/dmx`, `src/ilda`, `src/osc` | encoders unit-tested, browser relay transport | **port encoders** as output modules (V5) |
| Pixel-map field | `hyst-output` | abstraction only | **port the idea** as `out.pixelmap` (texture → LED array) |
| Dance compiler, previz, CLI, multi-arm choreo | `hyst-compile`, `hyst-previz`, `hyst-cli`, `hyst-choreo` | afekt's | **move to afekt** before any deletion (afekt calls them) |
| `hyst-hw` | 2-line stub | — | **delete** |

The crate graph is clean: render, audio and script depend on nothing in compile, choreo, previz or cli, so the extraction needs no code surgery.

## 2. Target architecture

### 2.1 Where the code lives

**Recommendation:**
- **synteza becomes the video engine library and the installation runner.**
- **kabl is the instrument and the editor.**
- One editor, kabl's rack, so there is no second patcher UI to build.

```
synteza (repo)                                   kabl (repo)
  synteza-gpu       device, texture pool,          core      patch/op log (+ domains, AV-MERGE)
                    pipeline cache, surfaces       engine    audio compiler/scheduler
  synteza-graph     VisualModule trait, frame      modules   audio modules
                    graph compiler + scheduler     visual    glue: kabl PatchState → synteza graph,
  synteza-modules   built-in visual modules                  bridge, render thread, preview
  synteza-shader    shader-module format + loader  ui        rack shows both domains
  synteza-analyze   audio analysers (non-RT)
  synteza-out       window, record, NDI/Syphon/Spout, DMX, pixel map, laser
  synteza (bin)     headless/installation runner: plays a .kabl patch fullscreen, external audio in
```

- kabl depends on synteza crates as a git dependency pinned to a revision; vendor the code if a release must be self-contained.
- synteza depends on nothing in kabl, except the runner binary, which needs kabl-core and kabl-engine to load and play a `.kabl` file.
- **Licence:** synteza has **no licence file today**. kabl is GPL-3.0-or-later.
  - Choose MIT/Apache-2.0 (most reusable, GPL-compatible) or GPL-3.0 (matches kabl). Owner decision.
  - Until then kabl cannot legally ship synteza code.

The alternative is to move everything into the kabl workspace. That is simpler to co-develop but loses synteza as a separate product and portfolio piece. Revisit if cross-repo churn hurts during V1–V2.

### 2.2 Types on the wire (visual domain)

| Type | Payload | Notes |
|---|---|---|
| `Video` | texture handle, `Rgba16Float`, graph resolution (per-module override later) | never leaves the GPU and never crosses to audio as a texture |
| `Value` | f32 per frame | the visual equivalent of CV |
| `Trigger` | list of events this frame, each with a sub-frame time and a strength | from gates or edges |
| `Path` (later) | polyline, up to N points + colour | beam, laser |
| `Array` (later) | f32[N] per frame | spectrum, scope, a row of pixels |

### 2.3 Module trait (sketch)

```rust
pub trait VisualModule: Send {
    fn info(&self) -> &'static VisualInfo;            // ports (domain = Visual), params, rate
    fn prepare(&mut self, gpu: &Gpu, cfg: &FrameConfig) -> Result<(), VisualError>; // pipelines (cached), state textures
    fn frame(&mut self, cx: &mut FrameCx, io: &mut VisualIo); // record commands into cx.encoder; no submit, no readback
    fn resize(&mut self, gpu: &Gpu, cfg: &FrameConfig) {}
    fn save_state(&self, _: &mut StateBuf) {}          // same shape as kabl Module state carry
    fn load_state(&mut self, _: &StateBuf) {}
}
// FrameCx: encoder, frame index, presentation-time sample position, dt, texture pool.
// VisualIo: input Video/Value/Trigger slices, output Video targets from the pool, params (already smoothed).
```

Rules:
- **No panics.** Every wgpu error routes to an error state on the module's face. Device loss rebuilds the graph.
- One `CommandEncoder` per frame and one submit.
- Pipelines are cached by (shader hash, format, blend).
- **Feedback is explicit**: a `prev(x)` edge reads last frame's texture, so cycles are legal and cost one frame.
- Ping-pong is owned by the scheduler, not by modules.
- Readback is always asynchronous: it is needed for pixel-map output, video→audio and recording, and has a 1–3 frame latency.

### 2.4 Scheduler

1. Compile on the control thread: resolve kinds, topo-sort with feedback edges cut, allocate pool slots by liveness (reuse textures), build the step list.
2. Deliver to the render thread through an SPSC queue with a revision number. Swap at a frame boundary. Carry state by `ModuleId` (mirrors kabl `carry_state`). Optionally crossfade the outputs over N frames, like audio swaps.
3. Each frame:
   - choose the target presentation time;
   - read bridged values for that time (AV-MERGE §4);
   - run the steps;
   - present to every output surface;
   - kick off any async readbacks.

### 2.5 Frame clock

- The render loop follows vsync of the primary output window. Secondary outputs present the same frame.
- The offline render mode runs at a fixed fps with no vsync. It drives the same graph for golden clips and "render to file".

## 3. Module catalogue

Priority: **P1** is needed to claim "video synth". **P2** is needed to beat Lumen. **P3** is the long tail.

### 3.1 Generators
| Module | Priority | Notes |
|---|---|---|
| `gen.osc` video oscillator | P1 | sine/tri/saw/square ramp; horizontal, vertical, radial, angular; frequency, phase, rotation, sync-to-clock; **pitch input in V/oct, so a note can set the stripe frequency (in tune with the music)** |
| `gen.shape` | P1 | SDF circle, rect, polygon (n sides), star, line; size, softness, position, rotation |
| `gen.noise` | P1 | animated value/simplex/fbm/curl; scale, speed, octaves |
| `gen.gradient` / `gen.color` | P1 | — |
| `gen.path` | P1 | from beam.rs: polyline from an `Array` (scope, Lissajous X/Y from two audio signals, laser preview) |
| `gen.julia` | P2 | from synteza; HDR palette |
| `gen.text` | P3 | glyph atlas |
| `gen.mandelbulb` | P3 | expensive; quality parameter |

### 3.2 Processors
| Module | Priority | Notes |
|---|---|---|
| `fx.feedback` | P1 | prev-frame transform: zoom, rotate, translate, decay, hue shift. The classic video-synth core |
| `fx.feedback.flow` | P1 | synteza memory field: curl advection, fold, mirror |
| `fx.trails` | P1 | persistence |
| `fx.colorize` | P1 | palette/cosine palette, hue rotate, posterize, invert, levels |
| `mix.blend` | P1 | crossfade plus add, multiply, screen, difference, overlay, lighten/darken |
| `mix.key` | P1 | luma and chroma key with softness |
| `fx.displace` | P1 | warp A by B (luma or RG offset), amount |
| `fx.transform` | P1 | scale, rotate, translate, tile, mirror, kaleidoscope (fold count) |
| `fx.blur`, `fx.bloom` | P1 | mip cascade |
| `fx.edge`, `fx.sharpen`, `fx.threshold`, `fx.pixelate` | P2 | — |
| `fx.polar` | P2 | rect ↔ polar |
| `fx.timedelay` | P2 | frame buffer, read N frames back, slit-scan |
| `fx.shader` | P1 | user/ISF shader module (§4) |

### 3.3 Visual-side control (frame rate)
`ctl.spring` (overshoot, impulse in), `ctl.integrate` (rate → wrapped/unwrapped phase), `ctl.curve` (easings), `ctl.julia.pilot` (synteza's navigator as CV outputs).

Most other control can use kabl's audio-domain CV modules (LFO, slew, logic, macros) through the bridge. Do not duplicate them.

### 3.4 Inputs and outputs
| Module | Priority | Notes |
|---|---|---|
| `out.window` | P1 | window or fullscreen on a chosen display; multiple instances = multiple outputs |
| `out.preview` | P1 | egui texture on the module face and the Perform view; same `Device` as egui-wgpu, zero copy |
| `out.record` | P2 | ffmpeg subprocess pipe, or a native encoder later; muxes kabl's audio |
| `out.share` | P2 | Syphon (macOS), Spout (Windows), NDI (all); each platform-gated |
| `in.camera`, `in.video` | P2 | needs the asset model (AV-MERGE §6); camera is opt-in |
| `out.pixelmap` | P3 | texture → LED array → Art-Net/sACN/WLED |
| `out.dmx` | P3 | Value → DMX channels |
| `out.laser` | P3 | Path → ILDA / Ether Dream |

## 4. Shader modules (one file = one module)

- **Native format: WGSL plus a JSON or comment header** declaring inputs (`video`, `value`, `trigger`, `color`, `point`), params with ranges, and passes.
- Supported features:
  - persistent buffers (feedback);
  - multipass;
  - image inputs;
  - `TIME`, `FRAMEINDEX`, `RENDERSIZE`, and a `BEAT`/`BAR` phase from kabl's clock.
- **ISF import:**
  - Reuse synteza's parser.
  - Translate GLSL → naga → WGSL at load time, so ISF's large public library works.
  - Add the things the parser currently rejects: `IMPORTED`, persistent buffers, multipass, image inputs.
  - Fix the combined-sampler emission.
- Hot reload on save. Compile errors show on the module face; the last good pipeline keeps running.
- The same file format serves factory modules, so most P1 processors can ship as shader files rather than Rust.
- Sandboxing: shaders cannot touch the host. Cap the cost through resolution and a GPU timestamp query budget (if the adapter supports it), and show a "slow" badge.

## 5. Phases

| Phase | Content | Done when |
|---|---|---|
| **V0 split cleanup** (synteza) | Move the afekt crates out with afekt pointed at them; delete `hyst-hw`; add a licence; add Rust CI (clippy + tests; GPU tests on a software adapter, lavapipe); fix the clippy errors (`julia.rs` `manual_is_multiple_of`); rewrite the README | synteza workspace = {gpu, render, audio}; CI green; README describes the video core |
| **V1 runtime** | wgpu 29; `synteza-gpu` (RGBA16F pool, pipeline cache, async readback, surfaces); `synteza-graph` (trait, compiler, scheduler, explicit feedback, state carry); `out.window` + offline file render | 1080p60 on the reference laptop with a 10-module test graph; one submit per frame; offline and live outputs match the golden image within tolerance |
| **V2 primitives (P1)** | §3.1–3.2 P1 set, mostly as shader modules; `fx.shader` runtime | A Lumen-style patch (osc → feedback → colourise → key/mix) built from primitives; looks right to the owner |
| **V3 synteza content** | port memory field, trails, bloom, tonemap, beam, Julia + pilot; spring/integrate/curve; 4–6 factory Looks as composites | The Looks run live at 60 fps; HDR bug gone (compare against TS reference renders) |
| **V4 shader ecosystem** | ISF import with multipass/persistent/image; hot reload; error UI; docs | Top-50 ISF shaders from a public set load or give a clear reason |
| **V5 I/O** | record, Syphon/Spout/NDI, camera, video files, pixel map, DMX, laser | Each output tested on its platform; laser/DMX marked "unverified on hardware" until tested |
| **V6 analysers** | `analyze.*` modules: bands, centroid, flatness, onset, beat/tempo, then build/drop/break | Runs off the audio thread; no allocation in the callback; same results as hyst-audio tests |

V1–V3 can run while kabl's audio work (AUDIO.md A1/A4) continues, because they touch only synteza until AV-MERGE M2.

## 6. Testing

- **Golden images with tolerance** (PSNR or per-pixel ΔE threshold), rendered offline at fixed resolution and fps. Do not compare hashes: GPUs differ.
- **CI on a software adapter** (lavapipe on Linux, WARP on Windows). Today GPU tests return silently when no adapter exists, so CI passes vacuously. Make "no adapter" a failure in CI.
- Performance: GPU timestamp queries per module where supported, reported in a frame-time overlay. Perf gate = p99 frame time at 1080p for factory Looks on the reference laptop.
- Per-module property tests from synteza (non-degenerate output, symmetry, monotonic response) stay as unit tests.

## 7. Multiplatform

- wgpu covers Vulkan/GL on Linux, Metal on macOS and DX12/Vulkan on Windows. Shaders stay WGSL, and naga handles GLSL import.
- Avoid optional features unless gated: float32 filtering, push constants, timestamp queries. `Rgba16Float` render targets are core.
- Platform-gated outputs: Syphon (macOS), Spout (Windows), V4L2 loopback (Linux, optional). NDI SDK licence: verify.
- Integrated GPUs are the baseline: P1 modules must run at 1080p60 on an Intel Iris Xe / Apple M1 class GPU. Mandelbulb and deep Julia scale down with resolution.

## 8. Lumen comparison (target)

| Lumen strength | Plan |
|---|---|
| Analog-video-synth primitives patched like a modular | §3 P1 set, in kabl's rack |
| Immediate, knob-driven feel | Same rack UX and macros as kabl audio; preview on every module face |
| Feedback as the core | `fx.feedback` + synteza flow field, both HDR |
| Audio/MIDI/OSC reactive | **Beyond Lumen:** structure-exact from kabl itself (AV-MERGE) |
| macOS only | wgpu on all three platforms |
| ISF/shader import (verify) | §4 |

## 9. Size (agent-weeks, one agent)

| Phase | Estimate |
|---|---|
| V0 | 1–2 |
| V1 | 4–5 |
| V2 | 4–6 |
| V3 | 3–4 |
| V4 | 3 |
| V5 | 4–6 |
| V6 | 2–3 |
