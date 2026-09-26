# Parameter classification (generated)

Written by `cargo test -p kabl-engine --test runtime_controls write_classification -- --ignored` from `kabl_engine::runtime` and the module registry. Rules and reasons: [design.md](design.md).

| Module | Param | Taper | Class | Runtime value | Reason |
|---|---|---|---|---|---|
| `osc.va` | `base_hz` | exponential | runtime | ramped 15 ms | read every block; the module does not smooth it |
| `osc.va` | `waveform` | stepped | runtime | set at once | read every block; a choice, set at once (never through invalid states) |
| `osc.va` | `pw` | linear | runtime | set at once | read every block; the module smooths it itself |
| `osc.va` | `fine` | linear | runtime | ramped 15 ms | read every block; the module does not smooth it |
| `osc.va` | `unison` | stepped | runtime | set at once | read every block; a choice, set at once (never through invalid states) |
| `osc.va` | `detune` | linear | runtime | ramped 15 ms | read every block; the module does not smooth it |
| `filter.svf` | `cutoff_hz` | exponential | runtime | ramped 15 ms | read every block; the module does not smooth it |
| `filter.svf` | `resonance` | linear | runtime | ramped 15 ms | read every block; the module does not smooth it |
| `env.adsr` | `attack_ms` | exponential | runtime | ramped 15 ms | read every block; the module does not smooth it |
| `env.adsr` | `decay_ms` | exponential | runtime | ramped 15 ms | read every block; the module does not smooth it |
| `env.adsr` | `sustain` | linear | runtime | ramped 15 ms | read every block; the module does not smooth it |
| `env.adsr` | `release_ms` | exponential | runtime | ramped 15 ms | read every block; the module does not smooth it |
| `env.adsr` | `timing` | stepped | runtime | set at once | read every block; a choice, set at once (never through invalid states) |
| `lfo` | `rate_hz` | exponential | runtime | ramped 15 ms | read every block; the module does not smooth it |
| `lfo` | `waveform` | stepped | runtime | set at once | read every block; a choice, set at once (never through invalid states) |
| `lfo` | `sync` | stepped | runtime | set at once | read every block; a choice, set at once (never through invalid states) |
| `lfo` | `phase` | linear | runtime | ramped 15 ms | read every block; the module does not smooth it |
| `vca` | `gain` | linear | runtime | ramped 15 ms | read every block; the module does not smooth it |
| `vca` | `exponential` | stepped | runtime | set at once | read every block; a choice, set at once (never through invalid states) |
| `ringmod` | (none) | | | | no params |
| `mixer` | `level1` | linear | runtime | ramped 15 ms | read every block; the module does not smooth it |
| `mixer` | `level2` | linear | runtime | ramped 15 ms | read every block; the module does not smooth it |
| `mixer` | `level3` | linear | runtime | ramped 15 ms | read every block; the module does not smooth it |
| `mixer` | `level4` | linear | runtime | ramped 15 ms | read every block; the module does not smooth it |
| `out` | (none) | | | | no params |
| `midi.in` | `mode` | stepped | structural | compile | keyboard configuration: read by the keyboard outside the graph when a graph is installed (voice assignment; a mode change releases) |
| `midi.in` | `priority` | stepped | structural | compile | keyboard configuration: read by the keyboard outside the graph when a graph is installed (voice assignment; a mode change releases) |
| `midi.in` | `glide` | stepped | structural | compile | keyboard configuration: read by the keyboard outside the graph when a graph is installed (voice assignment; a mode change releases) |
| `midi.in` | `glide_ms` | exponential | runtime | ramped 15 ms | read every block; the module does not smooth it |
| `clock` | `bpm` | linear | runtime | ramped 15 ms | read every block; the module does not smooth it |
| `seq` | `p1` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `p2` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `p3` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `p4` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `p5` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `p6` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `p7` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `p8` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `g1` | stepped | runtime | set at once | read every block; a choice, set at once (never through invalid states) |
| `seq` | `g2` | stepped | runtime | set at once | read every block; a choice, set at once (never through invalid states) |
| `seq` | `g3` | stepped | runtime | set at once | read every block; a choice, set at once (never through invalid states) |
| `seq` | `g4` | stepped | runtime | set at once | read every block; a choice, set at once (never through invalid states) |
| `seq` | `g5` | stepped | runtime | set at once | read every block; a choice, set at once (never through invalid states) |
| `seq` | `g6` | stepped | runtime | set at once | read every block; a choice, set at once (never through invalid states) |
| `seq` | `g7` | stepped | runtime | set at once | read every block; a choice, set at once (never through invalid states) |
| `seq` | `g8` | stepped | runtime | set at once | read every block; a choice, set at once (never through invalid states) |
| `seq` | `length` | stepped | runtime | set at once | read every block; a choice, set at once (never through invalid states) |
| `seq` | `transpose` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `v1` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `v2` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `v3` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `v4` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `v5` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `v6` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `v7` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `v8` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `gate_len` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `gate_mode` | stepped | runtime | set at once | read every block; a choice, set at once (never through invalid states) |
| `seq` | `r1` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `r2` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `r3` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `r4` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `r5` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `r6` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `r7` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `r8` | linear | runtime | set at once | step data, read when a step plays; set at once (a ramp could play a note between values) |
| `seq` | `direction` | stepped | runtime | set at once | read every block; a choice, set at once (never through invalid states) |
| `seq` | `bank` | stepped | runtime | set at once | read every block; a choice, set at once (never through invalid states) |
| `seq` | `b.*`, `c.*`, `d.*` (banks B–D) | as bank A | runtime | set at once | same as bank A's params |
| `clock.div` | `div` | stepped | runtime | set at once | read every block; a choice, set at once (never through invalid states) |
| `delay` | `time_ms` | exponential | runtime | set at once | read every block; the module smooths it itself |
| `delay` | `sync` | stepped | runtime | set at once | read every block; a choice, set at once (never through invalid states) |
| `delay` | `feedback` | linear | runtime | set at once | read every block; the module smooths it itself |
| `delay` | `mix` | linear | runtime | set at once | read every block; the module smooths it itself |
| `delay` | `tone_hz` | exponential | runtime | ramped 15 ms | read every block; the module does not smooth it |
| `delay` | `mode` | stepped | runtime | set at once | read every block; a choice, set at once (never through invalid states) |
| `reverb` | `decay_s` | exponential | runtime | set at once | read every block; the module smooths it itself |
| `reverb` | `damp_hz` | exponential | runtime | set at once | read every block; the module smooths it itself |
| `reverb` | `mix` | linear | runtime | set at once | read every block; the module smooths it itself |
| `reverb` | `predelay_ms` | linear | runtime | set at once | read every block; the module smooths it itself |
| `reverb` | `width` | linear | runtime | set at once | read every block; the module smooths it itself |
| `gain` | `gain_db` | linear | runtime | set at once | read every block; the module smooths it itself |
| `macro` | `m1` | linear | runtime | set at once | read every block; the module smooths it itself |
| `macro` | `m2` | linear | runtime | set at once | read every block; the module smooths it itself |
| `macro` | `m3` | linear | runtime | set at once | read every block; the module smooths it itself |
| `macro` | `m4` | linear | runtime | set at once | read every block; the module smooths it itself |
| `cues` | (none) | | | | no params |
| `noise` | `color` | stepped | runtime | set at once | read every block; a choice, set at once (never through invalid states) |
| `noise` | `level_db` | linear | runtime | set at once | read every block; the module smooths it itself |
| `filter.ladder` | `cutoff_hz` | exponential | runtime | set at once | read every block; the module smooths it itself |
| `filter.ladder` | `resonance` | linear | runtime | set at once | read every block; the module smooths it itself |
| `filter.ladder` | `drive_db` | linear | runtime | set at once | read every block; the module smooths it itself |
| `chorus` | `rate_hz` | exponential | runtime | ramped 15 ms | read every block; the module does not smooth it |
| `chorus` | `depth` | linear | runtime | set at once | read every block; the module smooths it itself |
| `chorus` | `mix` | linear | runtime | set at once | read every block; the module smooths it itself |
| `chorus` | `width` | linear | runtime | set at once | read every block; the module smooths it itself |
| `drive` | `drive_db` | linear | runtime | set at once | read every block; the module smooths it itself |
| `drive` | `trim_db` | linear | runtime | set at once | read every block; the module smooths it itself |
| `drive` | `mix` | linear | runtime | set at once | read every block; the module smooths it itself |

20 runtime params ramp, 72 are set at once, 3 are structural (bank A of `seq` counted; banks B–D behave the same).

## Cable parameters

| Cable | Param | Class | Reason |
|---|---|---|---|
| route (into a knob) | `amount` | runtime, ramped 15 ms (as a scale) | read every block from the route's scale; a bypassed route is not compiled, so its amount changes nothing |
| route | `bypass` | structural | a bypassed route leaves the compiled graph: scheduling and feedback (DFS back edges) change |
| route | creation / deletion | structural | scheduling, cycle breaking and buffers change |
| jack cable | any | structural (wiring); its params are not read | the compiler reads no jack cable params |

## Presentation (never reaches the engine)

`face.*`, `pin.*`, `cc.*`, `btn.*`, `launch.*`, `cue*`, labels and positions: `rack::is_presentation` / `Op::SetLabel` / `Op::MoveModule`. The compiler does not read them, so `runtime_changes` finds nothing to send.
