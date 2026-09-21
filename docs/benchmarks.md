# Benchmarks

History of measured numbers. Every PR touching the engine reports ns/block for the benchmark
patches before and after (brief section 3). Filled in as benchmark patches exist (v1 milestone);
until then, spike-specific measurements are logged here under their own heading.

## Spike S1 — graph swap / crossfade

See `docs/decisions.md` 2026-09-21 "Spike S1" entry for method and the click-metric dead end.
Numbers from `crates/engine/tests/spike_s1_swap.rs` (`cargo test -p kabl-engine -- --nocapture`),
run on this session's container (not the Pi-4-class reference machine — no Pi available here;
potato-gate emulation is for the v1 milestone's S2 spike, not this one).

Scenario: 4-voice saw chord (A2/C3/E3/A3) through one shared TPT SVF lowpass (2 kHz, Q≈0.3
equiv.), cable depth toggled 1.0 ↔ 0.7, 100 times, 15 ms equal-power crossfade per swap, 48 kHz /
64-sample blocks, ~2.67 s total render (128,000 samples).

| Metric | Value | Gate | Result |
|---|---|---|---|
| Steady-state residual (engine vs. hard-switch reference, outside crossfade windows) | 0.0 (bit-exact) | < -80 dBFS | pass, by a wide margin |
| Boundary curvature (max \|2nd difference\| at a swap boundary) | 0.0318 | ≤ 1.5× typical in-chord curvature | pass — came in *below* typical (0.58×) |
| Typical in-chord curvature (max \|2nd difference\| elsewhere in the render) | 0.0546 | — | reference value |
| Diagnostic: excess jump a naive hard switch injects at the transition, isolated from ordinary waveform movement | avg 0.00107, max 0.00312 | — (diagnostic only, see decisions.md) | small at this filter cutoff; not used as a gate |
| Allocation during audio-thread path (100 swaps × 20 blocks each, `assert_no_alloc`-wrapped) | 0 | 0 | pass |
| `basedrop::Owned<T>` drop timing (separate isolated test, 8 drops) | 0 destructors run before `Collector::collect()`, 8 after | inline drop count == 0 | pass |

Not yet measured (v1 milestone, needs the real compiler/module registry, not this spike's
hand-rolled 2-node graph): ns/block for `tiny`/`classic`/`potato` benchmark patches, potato-gate
CPU percentage, jitter-gate soak.
