# kabl patch corpus manifest

Audit date 2026-11-23, at `origin/master` 89d9c1be (release 1.0.0 merge). This is a read-only audit: no patch or product file was changed. The machine-readable twin is `corpus-manifest.csv`, which has the same rows plus generator, golden-hash and keys/clock columns.

## How every asset was found

1. **Patch folders.** `git ls-files` lists 158 `meta.toml` files. A folder holding `meta.toml` + `log.jsonl` (with optional `checkpoint.json` and `sound.toml`) is one patch. That is the rule `kabl_core::format::pack_tree` uses (`crates/core/src/format.rs:444-462`). 148 of these folders are under `patches/` and 10 are under `docs/`.
2. **Packed `.kabl` files.** `git ls-files '*.kabl'` finds two: `docs/promo/lichen-plus.kabl` and `docs/promo/empty.kabl`. I decoded them as the reader does (`format.rs:285-373`): skip the 20-byte header and the skippable frames, then zstd with dictionary 1 (`crates/core/assets/kabl-dict-1.zstdict`).
3. **Code-built patches.** I grepped for `AddModule`, `default_patch`, `PatchEditor::new` and `"factory:` ids in `crates/*/src`. Four turned up:
   - `kabl_standalone::default_patch()` (`crates/standalone/src/lib.rs:136`). This is the fallback when `factory:init-keyboard` is missing (`crates/ui/src/browser.rs:690-692`).
   - `recipes::pluck_to_pad` and `recipes::filter_movement` (`crates/ui/src/recipes.rs:666-689`). Both duplicate `patches/recipes/*`, which are the copies the app opens.
   - `code_examples.rs`, which wraps one leaf in a composite at run time (`crates/ui/src/code_examples.rs:249`).

   None of these four is a separate shipped asset, so they get no CSV rows. The `default_patch` fallback is listed under "other" below.
4. **Factory bank loader reconciliation.** The build packs all of `patches/` into `.kabl` files (`crates/ui/build.rs:22-31`), and so does the package script (`packaging/linux/package.sh:37`, `kabl pack-tree`). The browser then lists the factory entries in `Library::rescan` (`crates/ui/src/library.rs:514-545`), which **only scans two directory levels**. As a result:
   - **129 entries are in the Sounds browser.** That is every folder under `patches/` except `sound-engines/demos/*`, and it matches the count of user-facing = yes rows.
   - **The 19 `patches/sound-engines/demos/*` patches are packed and shipped but never listed.** This is intentional, per `crates/ui/tests/sound_engines.rs:1016-1017`.
   - The 10 `docs/` folders and 2 `.kabl` files are not packed at all.
   - The total is 129 + 19 + 12 = 160 rows, and every row was parsed.
5. **Parsing.** I replayed every `log.jsonl` op by op, the way `format::replay` does (`format.rs:111-121`). The replay's modules and cables equal `checkpoint.json` for all 158 folders, so the checkpoints are consistent. Kind, category and user-facing status come from the path and `sound.toml` (`library.rs:63-101`).

## Counts by kind

| kind | in the Sounds browser | not listed | total |
|---|---|---|---|
| factory preset (Bass/Keys/Pad/Lead/Pluck/Groove/Wind/Strings/Percussion/Basic) | 92 | 0 | 92 |
| voice (category Voice) | 5 | 0 | 5 |
| FX (category FX) | 5 | 0 | 5 |
| piece (category Piece: 9 under `presets/`, 13 at the top level) | 22 | 0 | 22 |
| example-demo (recipes, reference, the expressive split study; engine demos) | 4 | 19 | 23 |
| fixture (`crowded` ships in the browser; 7 D09/D10 examples; the MIDI gate probe) | 1 | 8 | 9 |
| promo (`lichen-plus.kabl`, `empty.kabl`, `promo-demo/patch/morph-suite`) | 0 | 3 | 3 |
| dev-only (`factory-performance-bank/performance-project`) | 0 | 1 | 1 |
| **total** | **129** | **31** | **160** |

Other, code-built: `default_patch()` has 6 modules (midi.in → osc.va → filter.svf → vca with env.adsr → out). It only appears when the factory directory is missing.

## Corpus facts that matter for composites

- **No shipped patch uses a composite.** Composite instances appear only in the D09/D10 fixtures: `docs/composites/examples/{voice,two-instances,workflow}` and `docs/panel-authoring/examples/{voice,stereo-effect,portable}`. No patch has a `Snapshot` op either.
- **Size of the 129 browser patches.** They hold 3,170 modules in total. The median is 18 modules and the maximum is 103 (`presets/estuary`, 187 cables). The ones over 60 modules are estuary 103, sound-palette 100, balcony 77, dead-calm 77, lichen 76, roller-rink 74, gantry 62, spillway 62 and pawn-shop 61.
- **Cues, macros and cable-param routes.** 14 browser patches have cues (scenes) and 118 have named macros. 54 patches route a macro into another cable's parameter (`PortRef::CableParam`, 121 routes in total). These routes are the main blocker for encapsulation; the catalogue's G1 gap covers them.
- **Document limits.** No patch exceeds the limits that composite operations enforce: 128 modules, 512 cables, and 8 delay/chorus/reverb (`crates/ui/src/composites.rs:21-30`).
- **Golden render coverage is thin.** `crates/engine/tests/golden_renders.txt` hashes only 26 patches: the palette, init, recipes, the early pieces and 4 demos. **None of the 100 `patches/presets/*` is covered**, Lichen included. `crates/engine/tests/golden/legacy_hashes.txt` has 22 rows from before functional cables.
- **Generators.** Most patches are written by code, not by hand: `tools/examples/presets_{bass_lead,groove,pad,voice,piece,pilot,keys_pluck}.rs`, plus `#[ignore]` writer tests in `crates/ui/tests/*.rs` (`sound_palette.rs:1267`, `composition.rs:478`, `sound_engines.rs:1016`, and others). The CSV `generator` column is a heuristic match on the function name. Any composite migration has to land in these generators too, or a later regeneration will drop it.
- **Finding for Kosta: a likely unit slip in `presets/acid-line`.** Its space tail has delay `mix=0.16` and reverb `mix=0.08`. These params are percentages (`crates/modules/src/builtins/delay.rs:72`, range 0–100 %), so the effect is close to inaudible. Every other preset uses values like 5–38. The source is `tools/examples/presets_groove.rs:2812,2821`. Changing it would be a sonic change, so it is Kosta's call. I did not change it.
- **`patches/crowded` is a layout fixture that ships in the browser.** It is written by `crates/ui/tests/interaction.rs:1225` and listed as a "Study". It is user-facing by accident, not by design.
- **Vestigial modules.** Several pieces contain mixers that pass a single input at level 1.0, left by the `sum()` and "after"-join helpers (Lichen #54 and #56). These are cleanup candidates. Removing them would change summation order, so they need a render check first.

## Columns (CSV)

| column | meaning |
|---|---|
| `path` | Repo path |
| `kind` | Classification, as above |
| `category` | `sound.toml` category |
| `name` | Name shown in the browser |
| `user_facing` | Whether it is listed in the Sounds browser |
| `modules` | Module count |
| `cables` | Cable count: `jack_cables` + `mod_routes` (module params) + `cable_param_routes` |
| `composites` | Composite instances already in the patch |
| `cues`, `cues_yes` | Number of `cueN.on` flags |
| `macros` | Number of `mN` macro values |
| `keys`, `clock` | Whether the patch has a midi.in or a clock module |
| `candidate_instances` | Catalogue candidates found in the patch (see `composite-catalogue.md`) |
| `generator` | Generator file (heuristic match) |
| `golden_hash` | Whether the patch is in `golden_renders.txt` |
| `notes` | Free text |

## Every asset

### factory preset (92)

| path | cat | user-facing | modules | cables (jack/route/cable-param) | composites | cues | macros | candidates found | notes |
|---|---|---|---|---|---|---|---|---|---|
| `expressive/lead` | Lead | yes | 15 | 28 (16/12/0) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `init-keyboard` | Basic | yes | 6 | 6 (6/0/0) | 0 | 0 | 0 | voicex1 | init/starting patch (browser.rs:35) |
| `palette/bass` | Bass | yes | 13 | 22 (15/7/0) | 0 | 0 | 4 |  |  |
| `palette/breath` | Wind | yes | 15 | 24 (15/9/0) | 0 | 0 | 4 | voicex1 |  |
| `palette/keyboard-bass` | Bass | yes | 10 | 14 (10/4/0) | 0 | 0 | 4 |  |  |
| `palette/keys` | Keys | yes | 10 | 14 (10/4/0) | 0 | 0 | 4 |  |  |
| `palette/lead` | Lead | yes | 15 | 27 (16/11/0) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `palette/pad` | Pad | yes | 22 | 33 (24/9/0) | 0 | 0 | 4 | voicex1 |  |
| `palette/perc` | Percussion | yes | 27 | 42 (33/9/0) | 0 | 0 | 4 |  |  |
| `palette/progression` | Pad | yes | 14 | 21 (16/5/0) | 0 | 0 | 4 | seqvoicex1 voicex1 |  |
| `palette/strings` | Strings | yes | 15 | 28 (14/14/0) | 0 | 0 | 4 |  |  |
| `presets/acid-line` | Groove | yes | 45 | 72 (62/10/0) | 0 | 0 | 4 | busx1 drumsx1 seqvoicex1 spacex1 voicex2 |  |
| `presets/acid-rain` | Bass | yes | 16 | 26 (16/10/0) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/afterglow` | Lead | yes | 21 | 36 (21/13/2) | 0 | 0 | 5 | spacex1 voicex1 |  |
| `presets/boxwood` | Pluck | yes | 11 | 30 (11/19/0) | 0 | 0 | 4 | spacex1 |  |
| `presets/brass-filament` | Keys | yes | 11 | 28 (11/17/0) | 0 | 0 | 4 | spacex1 |  |
| `presets/brick-dust` | Groove | yes | 42 | 68 (55/13/0) | 0 | 0 | 4 | drumsx1 seqvoicex1 spacex1 voicex2 |  |
| `presets/cellar-door` | Keys | yes | 12 | 24 (11/12/1) | 0 | 0 | 4 | spacex1 |  |
| `presets/checkerboard` | Keys | yes | 16 | 31 (16/14/1) | 0 | 0 | 4 | busx1 spacex1 voicex1 |  |
| `presets/cinder-block` | Bass | yes | 17 | 32 (17/14/1) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/cold-cut` | Pluck | yes | 11 | 20 (11/9/0) | 0 | 0 | 4 | busx1 spacex1 |  |
| `presets/concrete-cloud` | Pad | yes | 18 | 33 (19/14/0) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/copper-mile` | Groove | yes | 48 | 76 (65/11/0) | 0 | 0 | 4 | drumsx1 seqvoicex2 spacex1 voicex3 |  |
| `presets/copper-wire` | Bass | yes | 17 | 28 (19/9/0) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/corduroy` | Keys | yes | 16 | 32 (16/13/3) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/corner-store` | Groove | yes | 49 | 75 (66/9/0) | 0 | 0 | 4 | busx1 drumsx1 seqvoicex2 spacex1 voicex3 |  |
| `presets/cotton-mill` | Pad | yes | 18 | 31 (18/12/1) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/deep-field` | Pad | yes | 18 | 32 (20/11/1) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/dry-cleaner` | Pad | yes | 18 | 33 (19/14/0) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/dry-creek` | Pluck | yes | 11 | 24 (11/13/0) | 0 | 0 | 4 | spacex1 |  |
| `presets/dune-buggy` | Groove | yes | 48 | 74 (65/9/0) | 0 | 0 | 4 | busx1 drumsx1 seqvoicex2 spacex1 voicex3 |  |
| `presets/fault-line` | Groove | yes | 41 | 63 (54/9/0) | 0 | 0 | 4 | busx1 drumsx1 seqvoicex1 spacex1 voicex2 |  |
| `presets/fire-escape` | Lead | yes | 17 | 29 (17/10/2) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/fog-bank` | Pad | yes | 19 | 35 (18/14/3) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/glacier` | Pad | yes | 18 | 30 (18/12/0) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/glass-coast` | Groove | yes | 49 | 76 (66/10/0) | 0 | 0 | 4 | drumsx1 seqvoicex2 spacex1 voicex3 |  |
| `presets/glass-elevator` | Keys | yes | 12 | 28 (11/16/1) | 0 | 0 | 4 | spacex1 |  |
| `presets/glass-needle` | Lead | yes | 18 | 30 (17/9/4) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/gossamer` | Pluck | yes | 11 | 31 (11/20/0) | 0 | 0 | 4 | spacex1 |  |
| `presets/gumdrop` | Pluck | yes | 16 | 30 (16/12/2) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/gutter` | Bass | yes | 18 | 30 (19/10/1) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/hand-bell` | Keys | yes | 11 | 22 (11/11/0) | 0 | 0 | 4 | spacex1 |  |
| `presets/horn-section` | Lead | yes | 17 | 28 (16/9/3) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/hot-wire` | Lead | yes | 17 | 27 (16/10/1) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/lamp-oil` | Groove | yes | 50 | 79 (68/11/0) | 0 | 0 | 4 | busx1 drumsx1 seqvoicex2 spacex1 voicex3 |  |
| `presets/lantern` | Lead | yes | 18 | 31 (17/12/2) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/linoleum` | Keys | yes | 12 | 25 (12/13/0) | 0 | 0 | 4 | spacex1 |  |
| `presets/loading-dock` | Groove | yes | 42 | 69 (58/11/0) | 0 | 5 | 4 | seqvoicex2 spacex1 voicex3 |  |
| `presets/loud-mouth` | Lead | yes | 19 | 28 (19/8/1) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/low-tide` | Bass | yes | 18 | 28 (17/10/1) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/matinee` | Keys | yes | 16 | 26 (16/10/0) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/neon-highway` | Lead | yes | 16 | 26 (15/11/0) | 0 | 0 | 4 | voicex1 |  |
| `presets/oak-stave` | Bass | yes | 16 | 25 (16/7/2) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/oatmeal` | Bass | yes | 17 | 26 (16/9/1) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/ore-cart` | Groove | yes | 47 | 73 (64/9/0) | 0 | 0 | 4 | busx1 drumsx1 seqvoicex2 spacex1 voicex3 |  |
| `presets/parquet-loop` | Keys | yes | 19 | 31 (20/10/1) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/picture-window` | Pad | yes | 17 | 27 (17/10/0) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/pig-iron` | Lead | yes | 17 | 27 (17/9/1) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/pulpit` | Keys | yes | 11 | 28 (11/17/0) | 0 | 0 | 4 | spacex1 |  |
| `presets/pump-house` | Pad | yes | 19 | 32 (20/11/1) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/rail-yard` | Bass | yes | 19 | 31 (20/9/2) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/rain-gutter` | Pluck | yes | 11 | 22 (11/11/0) | 0 | 0 | 4 | spacex1 |  |
| `presets/rivet-gun` | Pluck | yes | 17 | 30 (17/13/0) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/rooftop` | Pad | yes | 17 | 33 (18/14/1) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/rope-walk` | Groove | yes | 50 | 81 (67/14/0) | 0 | 0 | 4 | drumsx1 seqvoicex2 spacex1 voicex3 |  |
| `presets/rosewood` | Pluck | yes | 12 | 20 (11/8/1) | 0 | 0 | 4 | spacex1 |  |
| `presets/rubber-band` | Bass | yes | 16 | 28 (16/12/0) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/salt-flat` | Pad | yes | 17 | 27 (16/11/0) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/scaffold` | Keys | yes | 17 | 31 (17/14/0) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/scrap-metal` | Groove | yes | 50 | 77 (67/10/0) | 0 | 0 | 4 | drumsx1 seqvoicex2 spacex1 voicex3 |  |
| `presets/short-circuit` | Lead | yes | 20 | 32 (22/8/2) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/slab-city` | Groove | yes | 42 | 66 (56/10/0) | 0 | 0 | 4 | busx1 drumsx1 seqvoicex1 spacex1 voicex2 |  |
| `presets/slate-roof` | Bass | yes | 19 | 29 (19/9/1) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/snowmelt` | Keys | yes | 11 | 25 (11/14/0) | 0 | 0 | 4 | spacex1 |  |
| `presets/soft-sine` | Lead | yes | 17 | 27 (16/9/2) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/spark-plug` | Pluck | yes | 16 | 30 (16/11/3) | 0 | 0 | 4 | busx1 spacex1 voicex1 |  |
| `presets/spinet` | Keys | yes | 16 | 25 (16/9/0) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/spun-sugar` | Lead | yes | 18 | 30 (17/11/2) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/static-garden` | Pad | yes | 23 | 39 (21/14/4) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/steel-toe` | Bass | yes | 19 | 33 (20/11/2) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/still-water` | Groove | yes | 47 | 72 (64/8/0) | 0 | 0 | 4 | busx1 drumsx1 seqvoicex2 spacex1 voicex3 |  |
| `presets/stoop-sale` | Groove | yes | 49 | 77 (65/10/2) | 0 | 0 | 4 | busx1 drumsx1 seqvoicex2 spacex1 voicex3 |  |
| `presets/tail-light` | Bass | yes | 18 | 30 (19/10/1) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/tar-pit` | Bass | yes | 16 | 25 (16/8/1) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/tidepool` | Pad | yes | 20 | 34 (16/14/4) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/tin-roof` | Keys | yes | 11 | 22 (11/11/0) | 0 | 0 | 4 | spacex1 |  |
| `presets/tin-voice` | Pad | yes | 20 | 33 (18/14/1) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/turnstile` | Keys | yes | 20 | 35 (21/13/1) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/undertow` | Bass | yes | 17 | 26 (17/9/0) | 0 | 0 | 4 | voicex1 |  |
| `presets/velvet-tine` | Keys | yes | 11 | 22 (11/11/0) | 0 | 0 | 4 | spacex1 |  |
| `presets/waiting-room` | Pad | yes | 21 | 38 (20/13/5) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/willow` | Pluck | yes | 11 | 21 (11/10/0) | 0 | 0 | 4 | spacex1 |  |

### voice (5)

| path | cat | user-facing | modules | cables (jack/route/cable-param) | composites | cues | macros | candidates found | notes |
|---|---|---|---|---|---|---|---|---|---|
| `presets/automaton` | Voice | yes | 21 | 29 (21/7/1) | 0 | 0 | 4 | voicex1 |  |
| `presets/maw` | Voice | yes | 19 | 30 (19/10/1) | 0 | 0 | 4 | voicex1 |  |
| `presets/oracle` | Voice | yes | 23 | 36 (23/13/0) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/sock-puppet` | Voice | yes | 24 | 36 (24/11/1) | 0 | 0 | 4 | voicex1 |  |
| `presets/vox-chop` | Voice | yes | 24 | 35 (25/8/2) | 0 | 0 | 4 | spacex1 voicex1 |  |

### FX (5)

| path | cat | user-facing | modules | cables (jack/route/cable-param) | composites | cues | macros | candidates found | notes |
|---|---|---|---|---|---|---|---|---|---|
| `presets/chandelier` | FX | yes | 24 | 40 (25/13/2) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/depth-charge` | FX | yes | 24 | 39 (25/12/2) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/temple-bowl` | FX | yes | 25 | 42 (28/14/0) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/updraft` | FX | yes | 18 | 31 (18/11/2) | 0 | 0 | 4 | spacex1 voicex1 |  |
| `presets/whiteout` | FX | yes | 22 | 35 (22/12/1) | 0 | 0 | 4 | spacex1 voicex1 |  |

### piece (22)

| path | cat | user-facing | modules | cables (jack/route/cable-param) | composites | cues | macros | candidates found | notes |
|---|---|---|---|---|---|---|---|---|---|
| `composition` | Piece | yes | 45 | 80 (61/19/0) | 0 | 5 | 4 | voicex2 |  |
| `cue-macros` | Piece | yes | 13 | 18 (13/5/0) | 0 | 4 | 3 | seqvoicex1 spacex1 voicex1 |  |
| `echo` | Piece | yes | 20 | 37 (26/11/0) | 0 | 0 | 4 | seqvoicex2 voicex2 |  |
| `functional-cables/echo-throws` | Piece | yes | 10 | 14 (14/0/0) | 0 | 0 | 0 | seqvoicex1 voicex1 |  |
| `functional-cables/five-against-eight` | Piece | yes | 10 | 11 (10/1/0) | 0 | 0 | 1 | seqvoicex1 voicex1 |  |
| `functional-cables/macro-morph` | Piece | yes | 13 | 16 (13/0/3) | 0 | 0 | 0 | voicex3 |  |
| `functional-cables/morph-arc` | Piece | yes | 24 | 31 (29/2/0) | 0 | 0 | 1 | seqvoicex2 voicex2 |  |
| `grid-cues` | Piece | yes | 11 | 15 (15/0/0) | 0 | 4 | 0 | drumsx1 |  |
| `interlocking` | Piece | yes | 16 | 28 (19/9/0) | 0 | 0 | 4 | seqvoicex2 voicex2 |  |
| `macros-eight` | Piece | yes | 10 | 17 (7/9/1) | 0 | 0 | 8 | spacex1 |  |
| `performance` | Piece | yes | 39 | 57 (50/7/0) | 0 | 0 | 0 | voicex1 |  |
| `presets/balcony` | Piece | yes | 77 | 118 (109/9/0) | 0 | 5 | 4 | busx1 clockdivx3 drumsx1 seqvoicex5 spacex1 voicex6 |  |
| `presets/dead-calm` | Piece | yes | 77 | 125 (110/14/1) | 0 | 4 | 4 | clockdivx4 drumsx1 seqvoicex5 spacex1 voicex6 |  |
| `presets/estuary` | Piece | yes | 103 | 187 (147/31/9) | 0 | 4 | 4 | clockdivx6 seqvoicex10 spacex1 voicex11 |  |
| `presets/gantry` | Piece | yes | 62 | 100 (87/13/0) | 0 | 5 | 4 | clockdivx2 drumsx1 seqvoicex4 voicex5 |  |
| `presets/laundromat` | Piece | yes | 52 | 80 (69/11/0) | 0 | 4 | 4 | drumsx1 seqvoicex3 voicex4 |  |
| `presets/lichen` | Piece | yes | 76 | 135 (101/24/10) | 0 | 4 | 5 | busx1 clockdivx1 drumsx1 randmelx2 seqvoicex3 spacex2 voicex6 |  |
| `presets/pawn-shop` | Piece | yes | 61 | 93 (79/11/3) | 0 | 4 | 4 | drumsx1 seqvoicex4 spacex1 voicex5 |  |
| `presets/roller-rink` | Piece | yes | 74 | 114 (104/10/0) | 0 | 4 | 4 | busx1 clockdivx3 drumsx1 seqvoicex5 spacex1 voicex6 |  |
| `presets/spillway` | Piece | yes | 62 | 94 (85/9/0) | 0 | 4 | 4 | busx1 clockdivx2 drumsx1 seqvoicex4 spacex1 voicex5 |  |
| `sequence` | Piece | yes | 8 | 8 (8/0/0) | 0 | 0 | 0 | seqvoicex1 voicex1 |  |
| `sound-palette` | Piece | yes | 100 | 160 (125/35/0) | 0 | 5 | 4 | voicex3 |  |

### example-demo (23)

| path | cat | user-facing | modules | cables (jack/route/cable-param) | composites | cues | macros | candidates found | notes |
|---|---|---|---|---|---|---|---|---|---|
| `expressive/split` | Study | yes | 14 | 18 (17/1/0) | 0 | 0 | 0 | voicex2 |  |
| `recipes/filter-movement` | Study | yes | 7 | 7 (6/1/0) | 0 | 0 | 0 | voicex1 | Learn recipe patch (recipes.rs:666-689) |
| `recipes/pluck-to-pad` | Study | yes | 7 | 7 (7/0/0) | 0 | 0 | 0 | voicex1 | Learn recipe patch (recipes.rs:666-689) |
| `reference` | Study | yes | 8 | 12 (6/6/0) | 0 | 0 | 0 | voicex1 | Study; also UI test fixture (interaction.rs, drawer.rs) |
| `sound-engines/demos/arp-cable` | Keys | no | 10 | 15 (14/1/0) | 0 | 0 | 0 | voicex1 | packed but below library scan depth (library.rs:518-532); engine demo |
| `sound-engines/demos/arp-chord` | Keys | no | 9 | 12 (11/1/0) | 0 | 0 | 0 | voicex1 | packed but below library scan depth (library.rs:518-532); engine demo |
| `sound-engines/demos/clock-logic` | Sequence | no | 18 | 24 (24/0/0) | 0 | 0 | 0 | randmelx1 voicex1 | packed but below library scan depth (library.rs:518-532); engine demo |
| `sound-engines/demos/dx-keys` | Keys | no | 5 | 6 (6/0/0) | 0 | 0 | 0 |  | packed but below library scan depth (library.rs:518-532); engine demo |
| `sound-engines/demos/four-floor` | Sequence | no | 10 | 15 (15/0/0) | 0 | 0 | 0 | drumsx1 | packed but below library scan depth (library.rs:518-532); engine demo |
| `sound-engines/demos/glass-bells` | Keys | no | 15 | 22 (19/3/0) | 0 | 0 | 0 | voicex2 | packed but below library scan depth (library.rs:518-532); engine demo |
| `sound-engines/demos/imported-morph` | Bass | no | 10 | 13 (11/2/0) | 0 | 0 | 0 | voicex1 | packed but below library scan depth (library.rs:518-532); engine demo |
| `sound-engines/demos/iron-bell` | Keys | no | 5 | 6 (6/0/0) | 0 | 0 | 0 |  | packed but below library scan depth (library.rs:518-532); engine demo |
| `sound-engines/demos/live-voice` | Lead | no | 13 | 14 (14/0/0) | 0 | 0 | 0 | voicex1 | packed but below library scan depth (library.rs:518-532); engine demo |
| `sound-engines/demos/pump` | Sequence | no | 15 | 23 (23/0/0) | 0 | 0 | 0 |  | packed but below library scan depth (library.rs:518-532); engine demo |
| `sound-engines/demos/ratchet-roll` | Sequence | no | 9 | 12 (11/1/0) | 0 | 0 | 0 | seqvoicex1 voicex1 | packed but below library scan depth (library.rs:518-532); engine demo |
| `sound-engines/demos/rising-pad` | Pad | no | 12 | 17 (13/4/0) | 0 | 0 | 0 | voicex1 | packed but below library scan depth (library.rs:518-532); engine demo |
| `sound-engines/demos/robot-voice` | Lead | no | 26 | 30 (29/1/0) | 0 | 0 | 0 | randmelx1 voicex3 | packed but below library scan depth (library.rs:518-532); engine demo |
| `sound-engines/demos/rust-bass` | Bass | no | 5 | 6 (6/0/0) | 0 | 0 | 0 |  | packed but below library scan depth (library.rs:518-532); engine demo |
| `sound-engines/demos/scale-walk` | Sequence | no | 11 | 12 (11/1/0) | 0 | 0 | 0 | randmelx1 spacex1 voicex1 | packed but below library scan depth (library.rs:518-532); engine demo |
| `sound-engines/demos/stepped-sweep` | Pad | no | 20 | 23 (22/1/0) | 0 | 0 | 0 |  | packed but below library scan depth (library.rs:518-532); engine demo |
| `sound-engines/demos/swing-seq` | Sequence | no | 12 | 15 (14/1/0) | 0 | 0 | 0 | seqvoicex1 voicex2 | packed but below library scan depth (library.rs:518-532); engine demo |
| `sound-engines/demos/tine-keys` | Keys | no | 12 | 18 (14/4/0) | 0 | 0 | 0 | voicex1 | packed but below library scan depth (library.rs:518-532); engine demo |
| `sound-engines/demos/vowel-drift` | Pad | no | 13 | 15 (14/1/0) | 0 | 0 | 0 | voicex1 | packed but below library scan depth (library.rs:518-532); engine demo |

### fixture (9)

| path | cat | user-facing | modules | cables (jack/route/cable-param) | composites | cues | macros | candidates found | notes |
|---|---|---|---|---|---|---|---|---|---|
| `docs/composites/examples/flat-voice` |  | no | 6 | 6 (6/0/0) | 0 | 0 | 0 | voicex1 | D09/D10 composite/panel example + evidence |
| `docs/composites/examples/two-instances` |  | no | 12 | 11 (11/0/0) | 3 | 0 | 0 | voicex2 | D09/D10 composite/panel example + evidence |
| `docs/composites/examples/voice` |  | no | 6 | 6 (6/0/0) | 1 | 0 | 0 | voicex1 | D09/D10 composite/panel example + evidence |
| `docs/composites/examples/workflow` |  | no | 10 | 10 (10/0/0) | 2 | 0 | 0 | voicex2 | D09/D10 composite/panel example + evidence |
| `docs/midi-timing/fixtures/gate-probe` |  | no | 3 | 2 (2/0/0) | 0 | 0 | 0 |  | MIDI timing probe |
| `docs/panel-authoring/examples/portable` |  | no | 6 | 5 (5/0/0) | 2 | 0 | 0 | voicex1 | D09/D10 composite/panel example + evidence |
| `docs/panel-authoring/examples/stereo-effect` |  | no | 2 | 2 (2/0/0) | 1 | 0 | 0 |  | D09/D10 composite/panel example + evidence |
| `docs/panel-authoring/examples/voice` |  | no | 4 | 3 (3/0/0) | 1 | 0 | 0 | voicex1 | D09/D10 composite/panel example + evidence |
| `crowded` | Study | yes | 16 | 29 (18/11/0) | 0 | 0 | 0 |  | written by ui test interaction.rs:1225; listed in Sounds browser (Study) |

### promo (3)

| path | cat | user-facing | modules | cables (jack/route/cable-param) | composites | cues | macros | candidates found | notes |
|---|---|---|---|---|---|---|---|---|---|
| `docs/promo-demo/patch/morph-suite` |  | no | 35 | 54 (52/2/0) | 0 | 0 | 1 | clockdivx1 seqvoicex2 voicex3 | promo video source (docs/promo/CUT1-NOTES.md) |
| `docs/promo/empty.kabl` |  | no | 0 | 0 (0/0/0) | 0 | 0 | 0 |  | promo video source (docs/promo/CUT1-NOTES.md) |
| `docs/promo/lichen-plus.kabl` |  | no | 112 | 192 (157/25/10) | 0 | 5 | 6 | busx1 clockdivx4 drumsx1 randmelx2 seqvoicex7 spacex2 voicex10 | promo video source (docs/promo/CUT1-NOTES.md) |

### dev-only (1)

| path | cat | user-facing | modules | cables (jack/route/cable-param) | composites | cues | macros | candidates found | notes |
|---|---|---|---|---|---|---|---|---|---|
| `docs/factory-performance-bank/performance-project` |  | no | 94 | 156 (121/35/0) | 0 | 5 | 4 | voicex3 | evidence project |
