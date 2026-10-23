# Macros: more than four, and macros that scale a depth

Branch `claude/macros` (from `origin/master`). Two limits from the factory preset pilot are gone:

1. A patch can have up to eight macros per macros module (`m1`..`m8`), each with a name saved in the patch.
2. A macro, or any other modulation source, can move the **amount** of a route into a knob, so a "Motion" macro can turn an LFO's depth on the filter cutoff up and down.

Four stays the recommended number. `m5`..`m8` are there for a sound that needs them and are not shown anywhere until they are used.

## Decision 1: how many macros

Options considered:

| Option | Engine cost | Saved patches | UI |
|---|---|---|---|
| Raise the module output limit and give the one `macro` module 8 outputs (chosen) | `MAX_OUTPUTS` 4 to 8 and four more match arms in `process_block`; the stack scratch array grows by four slice pairs; one more 20 ms glide per extra macro, each a multiply-add per sample | Additive: `m5`..`m8` are new params and ports; a patch saved with `m1`..`m4` has no value for them and loads with them at 0 | Existing pins, names (`name.mK` labels), CC mappings, Perform tiles, cue targets and CLAP slots already address a macro by port and param name, so they work unchanged |
| A macros module with a configurable count | A count param would have to change the port list, and ports are static per kind: every consumer of `ModuleInfo::ports` (compiler, UI jacks, routing drawer) would need a dynamic path | Count must be saved and old patches migrated | New "count" control, and removing a macro would orphan its routes |
| Several macros modules | None | None | Works today, but each module has its own Perform card, name list and cue row; a sound with five macros would show two cards |

Chosen: one module, eight fixed outputs. Ponytail reasoning: ports that nothing is routed from cost a glide and a pool buffer, and nothing else. The module face grows by four knobs; the Perform page shows only pinned macros, so a patch that does not pin `m5` looks as before.

What had to be touched for the count:

- `MAX_OUTPUTS` is 8 with match arms for 5 to 8 outputs; `compile()` still rejects a module above the bound with `TooManyPorts`, so a future module needing more fails loudly.
- Cue macro targets: a launch carries up to 16 macro targets (was 8). The cue editor and "Store macros" list the first four macros of each macros module plus any later one that is routed, pinned or named (`cues::active_macros`), so an unused `m5` does not crowd the cue.
- CLAP: all eight are automation candidates (`automation::candidates`); a lane can take any. Test `all_eight_macros_are_automation_candidates`.
- MIDI CC learn: a macro is a pin like any other, so learn works for every macro; the CC is stored per pin.
- Perform: five to eight macros share one card and one row; tiles narrow before the row wraps. See captures.

## Decision 2: macro-scaled depth

Mechanism: **a route into the amount of another route.** A route is already a cable (`CableState`) with an `amount`. Routes into the parameters of a cable existed since schema v7 (`PortRef::CableParam`: chance, morph, glide). The route's own `amount` is now a fourth parameter a `CableParam` route can name, `"amount"`, valid for a route that ends on a knob.

Effective amount = stored amount + sum over depth routes of (source value / source full scale) x depth amount. It is added in the same unit as the stored amount (fraction of knob travel), so a depth of +0.6 with a macro at 1.0 makes a route with amount 0.0 behave like amount 0.6. Everything stays additive and clamped once at the knob, like every other route.

Why this rather than alternatives:

- *A "depth" param on every module:* touches every module and doubles the knob count.
- *A multiplying (VCA-style) mod stage:* an extra module in the signal path, and the routes already carry the sign and amount the user edits.
- *Making the macro multiply the route:* a multiplicative scale makes a route with amount 0 stay 0 forever; additive lets "off to full" work, and the route's own amount still reads as the depth at rest.
- Reuses the cable-route UI ("moved by", Add source), the log (`Connect` to a `CableParam`), undo, copy/paste, the routing drawer list, and the runtime-value path (a depth amount edit ramps over 15 ms and is not a rebuild).

Compile rules (errors name the fix): the target must be a route that ends on a knob; a route cannot move its own amount; loops of routes are rejected as before; a bypassed depth route does nothing; a bypassed target route takes its depth routes with it.

No zipper noise: the macro itself glides over 20 ms and every route is evaluated once per 64-sample block, as before. A depth change moves the modulation by one small step per block, the same step size a macro moving a knob directly makes.

In the UI: open a knob's drawer, select a route's row, and under the rows a "Depth moved by" list with "Add source" appears, with the same row widgets as a route into a cable parameter (amount, invert, bypass, pattern, remove).

## Schema

No bump. A depth route is an ordinary cable whose destination is `CableParam { cable, param: "amount" }`, which already serializes (schema v7 allows any param name; the compiler used to refuse "amount"). New macro params are absent in old files and default to 0. An old build that opens a new file with a depth route reports the compile error that names the route, and does not crash. Tests: `crates/engine/tests/route_depth.rs`.

## Proof that old patches are unchanged

`cargo test --release -p kabl-ui --test factory_bank render_digest -- --ignored` renders every patch under `KABL_DIGEST_ROOT` for 24 s the way the bank render does and writes an FNV digest of the sample bits to `KABL_DIGEST`. Run on `origin/master` and on this branch, over `patches/` and over the `patches/` tree of `claude/presets-pilot`: all 42 patches in `patches/` and all 50 in the pilot branch give bit-identical digests (`evidence/digest-*.txt`). Only `macros-eight` differs: it does not compile on master (unknown param `m5`).

## Host check (REAPER 7.75, CLAP plugin)

`python3 docs/macros/scripts/host.py` (method of `docs/functional-cables/scripts/host_routes.py`; evidence in `evidence/host/`). Track 1 is `patches/macros-eight` with host automation lane 1 bound to the **fifth** macro (`m5`, Delay) and a recorded envelope on it; track 2 is `cue-macros`.

- Saved, REAPER quit completely, reopened, saved: both states identical as float32, including the lane-1 target (`m5`) and the route into a route's amount. The renders before and after the reopen are bit-identical (relative difference 0.0, spectrum cosine 1.0).
- The automation does something: the same project without the envelope renders differently (relative difference 0.26, spectrum cosine 0.97).
- A project saved by the previous build (state version 2) opens and saves unchanged.

`media/plugin-perform-1180x680.png` is the plugin editor in REAPER at its default 1180 by 680 with the eight named macros in one row (`scripts/editor.py`). The standalone minimum window is 1280 by 800 (`media/standalone-perform-*.png`, light and dark).

## Tests

Workspace: 980 passed, 0 failed, 34 ignored. New: `route_depth.rs` (7), `macros_perform.rs` (1), plus additions to `macros.rs`, `cues.rs`, `functional_cables.rs`, `library.rs` and the clap `host_tests.rs`.

## Demo

`patches/macros-eight` (Eight Macros): a self-playing bass with eight named macros pinned on Perform, CC 20 to 27. Motion (`m7`) is routed into the amount of an LFO's route into the filter cutoff.

```sh
cargo run -p kabl-ui --example macros_demo -- write patches/macros-eight
cargo run --release -p kabl-ui --example macros_demo -- render patches/macros-eight clip.wav   # prints the wobble per window
```

`media/motion-scales-lfo-depth.mp3` is 12 s: Motion 0 for 4 s, 0.5 for 4 s, 1.0 for 4 s. Listen for the filter wobble coming in. Measured: the lowest 50 ms RMS in each window falls 0.065, 0.015, 0.009 while the loudest stays at 0.21.

## Known gaps

- The routing drawer's "All sources" span ignores depth routes (it shows the stored amount).
- A long macro name truncates in its tile ("Resonan..." at eight macros in 1180 px); the hover and the drawer show it in full.
- Eight is a hard ceiling; a ninth needs a second macros module.
- A route into a route that ends on a cable parameter (a cable's morph or chance) has no depth yet.
