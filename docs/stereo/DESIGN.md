# Stereo cables: design and estimate

Status: design only, nothing built. Kosta approves before any step starts.
Base: `origin/master` at f9b56d1. Written from the engine code (`crates/engine/src/compile.rs`,
`crates/modules/src/{info,io,module}.rs`, `crates/modules/src/builtins/*`, `crates/core/src/{format,op,state}.rs`,
`crates/clap/src/lib.rs`, `crates/ui/src/{rack,cable_look}.rs`) and `docs/benchmarks.md`.

## Summary

- One jack and one cable carry stereo. Mono modules keep working unchanged and are run once per channel
  by the compiler when a stereo signal reaches them. Nothing is processed twice unless a stereo cable feeds it.
- Channel count is a property of the **source port**, fixed at compile time, 1 or 2. It is not stored
  per cable, so no cable file format change is needed to carry the count.
- **No legacy half jacks (Kosta, C3).** Modules that have separate L/R ports today (chorus, reverb, delay out, compressor,
  vocoder, audio.in, out, pan out, the fx family) get one stereo port of width 2 in place of the pair. The module code
  does not change: a width-2 port occupies two consecutive io slots. The old port names (`in_l`, `in_r`, `left`, `right`)
  stop existing in the model.
- Old patches and factory presets are migrated on load (schema 7 to 8, section 4). A cable pair becomes one stereo cable;
  every other use of a half port becomes a `split` or `join` module. Migrated patches render bit-identically (target met by
  construction, enforced by `legacy_patches` and `golden_renders`, which load the old files through the migration).
- `split` and `join` are pure wiring (no DSP, no instance, no CPU). They are also what the migration uses for every case
  that a single stereo cable cannot express.
- Estimated cost: about 3,700 lines in 5 PRs (was 3,000 with aliases; the migration adds about 700). Engine/CPU cost is small,
  1.5% of the block budget for a 16-voice patch (estimate, below). Biggest risk is the hot file `compile.rs` (2,900 lines);
  second is the migration of the shipped patches (67 files name a half port, section 4).

## 1. Model

### Today

- A port is one `[f32; 64]` buffer (`BLOCK = 64`, `compile.rs`). One cable per input port (the compiler takes the first
  incoming cable that names the port; unconnected = `InputSource::Silence`).
- A module is stereo only by having two ports (`STEREO_IN`/`STEREO_OUT` in `builtins/fx.rs`). A stereo
  connection is two cables, and the user wires both.
- Voice-rate modules are instanced once per voice lane (`compile.rs` ~1186: `n_lanes = voice_count`) with their own state.
  A voice-rate output feeding a global-rate input is averaged by `Step::SumVoices`.

### Proposed

1. **Width 1 or 2, derived, never stored.** A port has width 1 unless its module says otherwise.
   - *Native stereo port*: a `PortInfo` with a new field `width: u8` (default 1; 2 for stereo). A width-2 port takes two
     consecutive io slots, so the module code does not change: it keeps reading `io.input(0)`/`io.input(1)` and writing two
     outputs. `MAX_INPUTS`/`MAX_OUTPUTS` count slots, not ports, and are checked against the sum of widths. The compiler
     resolves one port name to both buffers. Example: chorus has `in` (width 2, slots 0 and 1) and `out` (width 2).
   - *Follow modules*: every other module with at least one `PortType::Audio` input. If any audio input
     is fed by a width-2 source, the compiler creates **two instances** (channel 0 reads left, channel 1 reads
     right) and all its outputs become width 2. If no audio input is stereo it stays one instance.
   - CV, gate and pitch cables are always width 1. A mono CV into a doubled instance is broadcast (both
     instances read the same buffer). Param modulation routes work unchanged (each instance already has its own
     `ParamMod` list over shared source buffers).
2. **Conversion rules, applied per cable at compile time:**
   - mono into a stereo input: broadcast, left = right = the same buffer (zero copy, zero cost).
   - stereo into a mono input: **average** `(L+R)/2`. With L = R this equals the mono signal bit for bit
     (`x+x` is exact in float, halving is exact), so a stereo chain fed by a mono source collapses
     back to the mono sound.
   - stereo into a Follow module that has mono-only audio ports (e.g. `ringmod` `a` stereo, `b` mono): the mono one is broadcast.
3. **Stereo cable file form:** an ordinary `CableState` between ordinary `PortRef::Module` endpoints. Nothing marks it as
   stereo in the file: width comes from the source port (rule 1). No alias names, no half ports.
   New port names (all collision-free; checked against the current `PortInfo` lists):

   | Module | Old ports | New port |
   |---|---|---|
   | chorus, reverb, compressor, vocoder (carrier), fx family (`eq`, `flanger`, `phaser`, `limiter`, `bitcrusher`) | in `in_l`,`in_r`; out `left`,`right` | in `in`, out `out`, both width 2 |
   | `out` | in `left`,`right` | in `in`, width 2 |
   | `audio.in` | out `left`,`right` | out `out`, width 2 |
   | `pan` | out `left`,`right` (in `in`, `pan` unchanged, mono) | out `out`, width 2 |
   | `delay` | out `left`,`right` (in `in`, `clock` unchanged) | out `out`, width 2 |

   `delay`, `pan` input stays mono (a stereo cable into it is averaged, rule 2). The earlier proposal of a new `delay` `in_r`
   is dropped (nothing needs it; add when a stereo-in delay is wanted). Unwired side of a width-2 input is silence, as today.
   The module list above is the set the engine code names today; step 2 starts with an audit of every `PortInfo` list in
   `crates/modules` for any other `left`/`right`/`in_l`/`in_r` pair (the grep found these, confirm against the registry).
4. **Functional cables** (`kabl-cables`, `Step::Cable`): a stereo cable gets two nodes with the same seed, so
   gating and probability are identical on both channels and each keeps its own `Carry::Audio` declick state.
5. **Cycles** (1-block delay buffers): `DelaySlotKey` gets a channel field (0 for all existing keys).
6. **Why not always-2 (Bitwig):** it would double every CV, gate and pitch cable and every control-rate module, and
   break bit-identity of every saved patch. Why not VCV-style dynamic count: this engine compiles a static
   schedule and the count is knowable at compile time; dynamic channel counts would force dynamic buffer
   sizing in the audio thread for no gain.

## 2. Per-channel generic execution

Yes, generically. The compiler already builds one `Box<dyn Module>` per `(module, lane)` with its own state
and its own buffers. A channel is a second axis on the same loop: `for lane in 0..n_lanes { for ch in 0..n_ch { ... } }`.
`MAX_INPUTS`/`MAX_OUTPUTS` are per instance, so they do not change. Implementation notes:

- Instance key becomes `(ModuleId, lane, ch)`: `module_origin`, `live.voice`/`live.global`, `param_slots`,
  `carry_state`/`recompile` state carry, `Tap` lanes and `midi_ins` need the channel (0 for old data).
- `Noise` and `Random` are seeded by `(id, lane)`. They have no audio input, so they are never doubled and
  seeds are unchanged.
- State carry across edits: channel 1 instances start from channel 0's saved state when a mono chain turns stereo
  (an edit while a note rings does not click) and carry normally afterwards.

| Class | Modules | Work |
|---|---|---|
| Per channel for free (Follow) | `gain`, `drive`, `filter.svf`, `filter.ladder`, `filter.comb`, `filter.formant`, `mixer`, `ringmod`, `vca` | none, compiler only |
| Already stereo (native width-2 port, metadata only) | `chorus`, `reverb`, `compressor` (already stereo-linked: detector is `max(|l|,|r|)`), `vocoder` (stereo carrier), `eq`, `flanger`, `phaser`, `limiter`, `bitcrusher`, `out`, `pan` (out), `audio.in` (out), `delay` (out) | replace the port pair by one `width: 2` port in `PortInfo` lists; processing code untouched |
| Need real code | `split`, `join` (wiring only: compiler aliases buffers), `osc.va` stereo output and `width` param, optional `widener` (mid/side, new, not required) | small |
| Stay mono | everything with no Audio input: `lfo`, `env.adsr`, `seq`, `arp`, `clock*`, `random`, `noise`, `quantizer`, `slew`, `attenuverter`, `sample.hold`, `midi.*`, `macro`, `drum.grid`, `drums` | none |

Vocoder `mod` input (modulator) stays mono: a stereo cable into it is averaged. Compressor `sc` is mono, same rule.

## 3. Polyphony

- Instances: voice-rate Follow modules downstream of a stereo source double, from `V` to `2V`. Native stereo sources
  (e.g. `osc.va`) write two buffers from **one** instance, so the oscillator and its unison phases are not doubled.
- Memory: a buffer is 256 bytes. 16 voices, 10 doubled modules, 3 outputs each: 16 x 10 x 3 x 256 B = 120 KiB extra
  worst case. Module state doubles for doubled modules (an `svf` is a few floats). No concern.
- Summing: `SumVoices` is linear, so each channel gets its own sum step (`sources` = that channel's lane buffers),
  averaged over `V` as today. The two steps replace one. `out` reads the two sums. A mono chain keeps the one step.
- CPU, measured on this machine (i7-13700H, 48 kHz, 64-sample blocks, 8 voices, `bench_patches`, `taskset -c 2`):
  `palette/pad` 139 us median (10.5% of the 1333 us block budget), `palette/strings` 128 us (9.6%), `crowded` 60 us (4.5%).
  p99.9 values are noise from a loaded host and are ignored.
- CPU, estimated (not measured, no spike built): upper bound is "every voice-rate module after the oscillator doubles",
  so a patch like `pad` costs at most 2x, 139 us to about 280 us, 21% of budget. Realistic is lower because the
  oscillator, envelope, LFO and the global reverb/chorus do not double; expect +40% to +80% on these patches,
  i.e. 14% to 19% of the budget. For 16 voices with unison, the extra is about 16 voices x 3 doubled modules
  x 64 samples x about 20 ns = 60 us, 4.6% of the block budget; roughly 1.5% when the filter share is smaller.
  The 20 ns per module sample is an assumption (the benchmark table has no `svf`/`ladder` row); step 1 measures it
  with a `bench_patches` run on a stereo version of `pad`, and the frame and audio-callback budgets in
  `docs/benchmarks.md` (bench_echo) are re-checked then. A mono patch pays nothing: no stereo cable, no extra step.

## 4. Schema 8 and migration

### Version and where migration runs

- `CURRENT_SCHEMA_VERSION` 7 to 8 (`crates/core/src/format.rs`). v8 is **not** a superset of v7 any more: v7 names ports
  (`in_l`, `left`, ...) that no longer exist. A v7 build refuses a v8 file (`UnknownSchemaVersion`), as for v4 to v7.
  A v8 build loads v1 to v7 files by migrating, and `save` writes v8 always (C5). v8 files are never migrated.
- Migration is one pure function `migrate_v7_to_v8(&mut PatchLog)` in `crates/core`, called from `format::load` when
  `meta.schema_version < 8`, after replay and before `composite::validate`. It reads the replayed `PatchState` and
  appends ordinary ops (`Disconnect`, `Connect`, `AddModule`, `SetCablePattern`, one `Annotate` "migrated to schema 8").
  Appending ops (not editing old entries) keeps ids stable and the old history readable. The ops are deterministic: same
  input state always yields the same new module ids, positions and cable ids.
- Not touched by the loader: `checkpoint.json` (the loader ignores it, "deferred" in `format::load`), but 20 shipped
  `checkpoint.json` files name half ports. Step 2 either regenerates them with the migrated state or deletes the stale ones;
  decide there. Composites (`state.composites`, `docs/composites/examples/state.json`) hold inner cables and exposed
  ports that name half ports; `migrate` runs on each composite's inner graph with the same rules and rewrites exposed port
  names (an exposed `in_l`/`in_r` pair becomes one exposed `in`).
- Undo: the migration ops are an undo barrier. Undoing past the migration would restore half cables to ports that no
  longer exist. `PatchLog` has no barrier concept yet (to verify in step 2); fallback is that migration replaces the history
  with a snapshot of the migrated state and keeps the old entries only in the file.
- Factory presets and shipped patches (`patches/**`, 67 `log.jsonl`/`checkpoint.json` files name a half port): migrated
  by the same loader at runtime, **and** rewritten once by a script in step 2 so the files on disk are v8 and clean.
  The golden hashes must not change in that PR (they are the proof).

### Migration rules

Applied per module that has a width-2 port after the port change ("stereo module"), in this order. `P` is a stereo
input port (old pair `P_l`, `P_r`); `Q` is a stereo output port (old pair `left`, `right`). Cables are matched by
endpoints; `a -> b` is a cable.

**Inputs (old cables into `in_l`, `in_r` or into `left`, `right` of `out`):**

1. *Both patched from the same width-1 source port `S` (a mono source into both), equal cable settings.* One cable
   `S -> in`. The compiler broadcasts it to both channels: the same two identical buffers as today's two cables.
2. *Both patched from two different sources `A -> in_l`, `B -> in_r`.* New `join` module `J`: `A -> J.left`,
   `B -> J.right`, `J.out -> in`. Placed left of the module.
3. *Both patched from the left and right of the same stereo module (`X.left -> in_l`, `X.right -> in_r`; the common
   case, an fx chain).* One stereo cable `X.out -> in`.
4. *Crossed, `X.left -> in_r`, `X.right -> in_l`.* `split(X)`, `join` with legs swapped. Rare; handled by the general fallback.
5. *Only L patched (R silent).* Today R is silence. A bare mono cable into the stereo jack would broadcast and change the
   sound, so: `A -> J.left`, `J.right` unwired, `J.out -> in`. `join` with an unwired side writes silence on that
   channel. Bit-identical. Same for *only R patched* with `J.left` unwired.
6. *Mono source into both, with different cable settings* (see below, "functional cables"): `join` with the two legs, so each
   leg keeps its own cable.

**Outputs (old cables from `left`, `right`):**

7. *Both used, to the same module as a pair* (rule 3 target): one stereo cable.
8. *Only `left` used, or `left` and `right` fanned to different, unrelated inputs.* New `split` module `T`:
   `X.out -> T.in`, `T.left -> dest_a`, `T.right -> dest_b`. The wiring is aliasing, so each output is the very same buffer as
   the old `left` / `right`. A stereo cable straight into a mono input would be averaged, which is not the old sound, so
   the migration never connects a stereo port to a mono port directly.

**Edge cases the rules must cover (each gets a test, step 2):**

- *Only L patched:* rule 5. R stays silent. Test: render hash of the old patch equals the migrated patch.
- *L and R from different sources:* rule 2. Test: two oscillators into a chorus.
- *A mono source into both:* rule 1. With a stereo-source fallback if the source is an old stereo module.
- *L/R used as CV or control:* an old half port wired to a CV, gate or pitch port (for example `audio.in.left` into an
  envelope follower input, or `pan.left` driving a param route as the source of a `ParamMod`). Rule 8 gives
  `split.left -> cv_in`. The split output has width 1, so the existing CV rule holds. A **width-1 CV source into an old half
  input** (for example a CV sent into `in_l`) goes through rule 5 and a `join`; Audio and Cv port types stay
  compatible with each other as today.
- *Cable with functional settings (gate, probability, steps, `CablePattern`, `CableParam` routes).* Merging two cables
  into one stereo cable would change the random seed or the gate pattern on one channel (seed derivation per cable is not
  verified yet; check in step 1). Rule: **merge (rules 1, 3, 7) only when both cables have empty `params` and `steps` and
  nothing routes into either cable's `CableParam`.** Otherwise keep both cables as legs through `split`/`join` (rules 2, 6, 8),
  each leg keeping its own `CableState`, so nothing changes.
- *Cycles:* a cycle that runs through the stereo module keeps its 1-block delay slot per leg (one slot per channel, key
  channel 0 or 1 from step 1); migrating `X.left -> Y.in_l`, `X.right -> Y.in_r` over a cycle keeps both slots.
- *Modulation of a port the migration moved:* `ParamMod` routes have `CableParam` or `Module` sources. A module source
  on `left`/`right` is rule 8 (split leg). A route targeting the cable that was merged follows the kept cable's id (the
  L cable's id survives a merge; the R cable is removed).
- *Module with a half port unwired on both sides:* nothing to do.
- *Composite exposed ports:* migrated with the inner graph, as above.

### Bit-identity

Yes, target met by construction, and proven by the existing tests:

- Why: a width-2 port is two buffers filled from the same sources in the same order; `join` and `split` alias buffers, no
  arithmetic; mono-into-stereo is a broadcast of one buffer (the compiled values equal today's two cables from one source);
  nothing averages. The only conversion that changes values is stereo-into-mono (average), and the migration never emits one.
- Proof in CI: `legacy_patches.rs` (hashes `golden/legacy_hashes.txt`) and `golden_renders.rs` (`golden_renders.txt`) load the
  old files through the migration and must pass **with the hash files unmodified**, in every PR from step 2 on. All shipped
  patches become migration fixtures for free. Added in step 2: one synthetic v7 patch per edge case above (only L, different
  sources, crossed, CV use, functional cable pair, cycle), each with the hash from the pre-migration engine
  (recorded from master before the PR).
- Added in step 1: a patch rebuilt with stereo cables and L = R matches its mono render, and a width-2 chain fed by a mono
  source equals the mono render sample for sample.
- Not claimed: bit-identity of round-trips (load, save, load): the saved v8 file has different ids and modules than the v7
  source. The audio is equal; the file is not.
- New `osc.va` params default so the old sound is unchanged (`width` = 0). Presets opt in.

## 5. UI needs (no visuals here; agent 3 owns them on `claude/visual-polish-proposal`)

- A stereo cable must be distinguishable from a mono one without relying on colour alone (`cable_look.rs` already enforces
  contrast per surface): needs a width/pair cue with a minimum thickness that still passes `worst_contrast`.
  `cable_look::paint` needs a `channels` argument and `points()` is reused.
- A stereo jack must be distinguishable and larger or paired. `rack.rs` finds jacks by `PortInfo` name and direction
  (`Rack::jack`); a width-2 `PortInfo` is one jack, so nothing needs an alias. No half jacks exist (C3), so the rack has no
  rule for them (the old C4 is gone). A legacy patch never shows half cables: it was migrated on load.
- Connect rules in the editor: mono to stereo and stereo to mono both allowed and shown with a small badge
  ("duplicated" / "summed"), so the conversion is visible.
- Inspect/scope (`Tap`/`ProbeTarget`, 1 lane): a stereo cable shows both channels (two traces) or a single mono sum plus width.
- `split` face: one stereo input jack, jacks `left` and `right`. `join` face: jacks `left` and `right` in, one stereo out.
  No knobs. Both are Utility category, `width_units` about 3. `out` shows one stereo input jack.
- Hover text on a cable states its width. `help.rs`/explain text for the new modules.

## 6. Plugin

- `crates/clap/src/lib.rs` already declares `main_output_channels: 2` and one stereo aux input (`aux_input_ports: [2]`),
  and passes `[f32; 2]` frames to `audio.in` through `input_rx`. No port change in the plugin.
- `audio.in` has one width-2 `out` and keeps its "mono source plays on both" behaviour (module code unchanged).
  Old patches are migrated on load. Nothing about untrusted-input clamping changes. The plugin embeds `PatchState` in
  the host project (`SoundState`), so it needs `migrate_state` at `decode` and state version 5 (step 2b, see "Step 1 results").
- Needs a REAPER render check for one stereo patch (spec in the standing REAPER recipe), to run silent per CLAUDE.md.

## 7. Build plan

Each step is one PR to master, tests green, golden hashes untouched unless stated.

| # | PR | Scope | Size | Notes |
|---|---|---|---|---|
| 1 | Engine: channels in the compiler | `StereoPort` in `info.rs`; channel axis in `compile_inner` (instances, buffers, per-channel `SumVoices`, delay slots, tap, state carry); conversion rules; unit tests with a test-only pair module; a `bench_patches` CPU run on a stereo `pad` | ~900 lines + ~400 tests, 2 sessions | One agent, sole owner of `compile.rs`. Critical path. Land first. |
| 2a | Width-2 ports and wiring modules | `PortInfo.width`; replace the pairs on chorus/reverb/fx/out/audio.in/delay/pan/vocoder/compressor; `split`, `join` as compile-time aliases; schema 8; format tests; update all in-repo users of the old names (tests, `crates/ui/examples`, standalone/clap/engine callers of `in_l`/`left`) | ~500 + ~250 tests | After 1. Not mergeable without 2b: old patches would not load. Land 2a and 2b together or 2a behind the same PR. |
| 2b | Migration | `migrate_v7_to_v8` (rules in section 4), composite inner graphs, loader hook, one fixture per edge case, rewrite script, regenerate the 67 files in `patches/**` and `docs/**`, checkpoint.json decision | ~450 + ~350 tests + generated data (diff only, hashes unchanged) | After 2a, same PR train. Touches presets and `tests/library.rs` fixtures: the one step that edits preset files. Any preset agent must be paused or rebased. |
| 3 | UI: cables, jacks, faces | connect rules, twin jack, stereo cable paint, `split`/`join` faces, probe with two traces, help text, UI tests | ~700 | After 1's API (can start against the metadata of 2). Waits for agent 3's visual PR to merge, or lands with a `channels` argument only and no style decisions. |
| 4 | Stereo sources | `osc.va` stereo output + `width`, `pan` input pair, optional `widener` | ~350 | After 2. Touches `osc_va.rs`, which owns the unison spread work (PR #42). |
| 5 | Evidence and presets | stereo demo patch, REAPER check, `docs/benchmarks.md` entry, `docs/stereo/README.md`, golden for the demo only | ~250 | After 1 to 4. |

Splitting across agents and avoiding collisions:
- Presets agents (1 and 2) edit `tests/library.rs`, `crates/ui/examples`, `patches/`. Step 1 touches none of those.
  **Step 2 now edits them** (it renames ports and rewrites the shipped files), so a preset agent working in parallel will
  conflict; coordinate with the overseer before 2a starts. No step edits `golden_renders.txt`/`legacy_hashes.txt`
  entries; step 5 only adds new demo lines at the end. A preset file written by an agent after 2b merges is v8 already.
- Agent 3 (visuals) owns `cable_look.rs`/`rack.rs` styling; step 3 only adds a `channels` parameter and the jack variants,
  and rebases on whichever lands first. Step 3 should start only after agent 3's PR merges, or take the fork risk.
- `compile.rs`: only step 1 changes it substantially. Any other agent touching it in the meantime should tell the overseer.
- One writer per worktree. Steps 2 and 3 can run in parallel after 1 merges; 4 after 2.

## 8. Choices for Kosta

| | Choice | Options | Recommendation |
|---|---|---|---|
| C1 | Channel model | (a) derived width 1 or 2 per port, static; (b) always 2 on every cable (Bitwig); (c) VCV dynamic count | (a) |
| C2 | Stereo into a mono input | average; left only; sum | average: exact for L = R and keeps level |
| C3 | How a stereo cable is named in the file | **Decided by Kosta:** single stereo ports (width 2), no legacy half jacks; migrate old patches and presets on load | as decided |
| C4 | Rack: half jacks of `in_l`/`in_r` | **Gone** with C3 (no half jacks exist) | n/a |
| C5 | Schema 8 written when | always (existing practice); only when a stereo cable or split/join exists | always: simpler, same as v4 to v7 |
| C6 | CV, gate, pitch cables | audio only; stereo CV too (stereo LFO) | audio only now |
| C7 | `split` and `join` | wiring only, no instance; or real DSP-free modules in the schedule | wiring only (zero cost; face is still a module) |
| C8 | Build order | steps 1 to 5 as above; or engine plus `osc.va` width first for a quick listen, UI after | steps as above |
| C9 | Wait for agent 3's visual PR before step 3 | yes; or build jack variants first and restyle later | yes |
| C10 | Migrated shipped patches on disk | loader migrates at runtime only; or also rewrite the 67 shipped files once to v8 | also rewrite: clean files, one diff, hashes prove it |
| C11 | `delay` input | stays mono (stereo cable is averaged); or add a stereo input now | stays mono; add when wanted |
| C12 | Merge of two cables that carry gate/probability/steps | keep as `split`/`join` legs (always bit-identical); or merge when equal | legs; revisit after the seed check in step 1 |
| C13 | Undo across the migration | barrier (history before it is read-only); or no history kept for migrated patches | barrier if `PatchLog` allows, else snapshot |

## Step 1 results (engine channels, built)

Branch `claude/stereo-engine`. Measured, not estimated.

**What step 1 is.** `compile.rs` gains width per source port, one instance per channel for a module that follows
a stereo signal, `Step::Average`, per-channel buffers, delay slots, voice sums and state carry. No `PortInfo`, patch
format or module change. Step 1 spells a stereo cable with two aliases that the compiler derives from port names:
`out_st` over an adjacent `(left, right)` output pair, `in_st` over `(in_l, in_r)` or `(left, right)` inputs. Step 2a deletes
the aliases when `PortInfo.width` arrives. Differences from the design above, all small:

- A module with a stereo pair in either direction is native and never doubled. A stereo cable into any plain port of it
  (`sc`, `mod`, `in` of `delay` and `pan`) is averaged.
- A doubled module publishes only its audio outputs for channel 1. A CV output of a doubled module (none exists in the
  registry today with an audio input) would carry channel 0.
- A mono cable into the pair alias is a broadcast of one buffer. Averaging `a == b` is the buffer itself, no step.
- `module_origin` keeps its shape; a parallel `module_channel` carries the channel. State carry matches
  `(origin, channel)`, and a new channel 1 takes channel 0's state. Probe and face taps see channel 0 only.

**Tests.** `tests/stereo.rs`: stereo chain equals two mono chains sample for sample (with state), mono broadcast, stereo
average, centre pan collapses to mono, state carry mono to stereo, feedback through a stereo cable, voices summed per
channel. Whole workspace `cargo test`, strict clippy and `cargo fmt --check` pass; `legacy_patches`, `golden_renders` and
`factory_levels_hold_the_target` pass with the hash files untouched.

**CPU** (`bench_patches`, i7-13700H, 48 kHz, 64-sample blocks, `taskset -c 2`, all voices sounding, host load average 2.7;
a first run at load 26 gave about 2.5 times these numbers and is discarded). `+st` puts a `pan` before every filter, so
every voice module after the first filter runs twice, and each `in_l`/`in_r` pair fed by one source is one stereo cable.
That is the design's upper-bound case (everything after the oscillator mixer doubles):

| patch | voices | mono us (% of 1333 us) | +st us (%) | extra |
|---|---|---|---|---|
| palette/pad | 8 | 67.1 (5.0%) | 95.8 (7.2%) | +43% |
| palette/pad | 16 | 122.5 (9.2%) | 181.3 (13.6%) | +48% |
| palette/strings | 8 | 65.7 (4.9%) | 91.6 (6.9%) | +39% |
| palette/strings | 16 | 120.1 (9.0%) | 173.0 (13.0%) | +44% |
| crowded | 8 | 32.8 (2.5%) | 47.5 (3.6%) | +45% |
| crowded | 16 | 64.4 (4.8%) | 93.5 (7.0%) | +45% |

The +40% to +80% estimate held at its low end; worst case here is 13.6% of the audio budget at 16 voices on a desktop
core. The Pi 4 is still unmeasured. A mono patch pays nothing (no stereo cable, no extra step). The reproduction is
`cargo build --release -p kabl-engine --example bench_patches` and `taskset -c 2 target/release/examples/bench_patches PATCH...`;
`stereo_demo` renders the same path with an auto-pan to WAV.

**Items step 2b depends on.**

- *Cable seed.* A functional cable's gate and probability seed is `kabl_cables::seed_of(cable id)`. Two legacy cables have
  two seeds. One merged stereo cable gives both channels one seed, so merging two functional cables changes the sound.
  Confirms C12: legs through `split`/`join`, never a merge. A stereo functional cable gets two nodes on one cable id, so both
  channels gate alike and the runtime value writers reach both (`cable_steps` already holds several steps per cable).
- *Undo barrier.* `PatchLog` (`crates/core/src/log.rs`) has `entries`, `cursor` and per-entry inverses, no barrier. Undoing
  the migration ops would restore cables to ports that no longer exist. Needs a small `floor` index (about 15 lines):
  `can_undo` is `cursor > floor`, `undo` stops there, and migration sets `floor` after appending. Not in step 1.
- *checkpoint.json.* `format::save` writes it, `format::load` never reads it ("deferred", `format.rs:45`). A stale one is
  dead data, not a load bug. The rewrite script regenerates it through `save`. `ui/library.rs` copies the folder files
  unchanged.
- *Plugin embedded state.* **Plan change.** `crates/clap` saves `SoundState { version 1..=4, patch: PatchState, ... }` as JSON in the
  host project. A DAW project holds raw `PatchState` with half-port names and never goes through `format::load`. The
  migration must therefore be a pure function over `PatchState` (`migrate_state`), with the log migration built on it
  (diff of before and after becomes the appended ops), and `SoundState` needs `version` 5 (older builds refuse it, as for
  composites and tables) plus `SoundState::decode` calling `migrate_state` for versions below 5. Step 2b grows by about 100 lines
  and one test with a version-4 state blob. REAPER projects saved today stay loadable.
- *Composites.* Not touched by step 1; the plan in section 4 stands.

## Risks and unknowns

- The 20 ns/sample figure for filters and the +40% to +80% range are estimates; step 1 measures them. If the measured cost
  exceeds 25% of budget at 16 voices on the Pi 4 target (unmeasured, as for every other benchmark), the fallback is
  skipping the second instance for a module whose two inputs are identical buffers (a broadcast mono feeding a Follow module stays one instance).
- `compile.rs` is large and shared; a mid-air collision costs more than the feature code. Land step 1 fast.
- Migration is the new main risk. Mitigations: pure function, one fixture per edge case, the 67 shipped files as free fixtures,
  hashes unchanged. Settled by step 1 (see "Step 1 results"): cable seed (no merging of functional cables), `PatchLog` has no
  undo barrier (add one), `checkpoint.json` is never read, the plugin embeds `PatchState` (migrate at `SoundState::decode`,
  state version 5). Still open: composite exposed ports.
- Users with their own v7 patches: loading works through the migration, but they get a v8 file on first save and a v7
  build cannot open it. Say so in the release notes.
- `split`/`join` modules appear in migrated patches next to modules that did not have them (rules 2, 5, 8). They are
  visible clutter, not an error. The rules prefer a plain stereo cable (1, 3, 7) whenever it is bit-identical.
- The module audit for other `left`/`right`/`in_l`/`in_r` pairs in `crates/modules` (section 1) is not finished.
- Not checked: the exact unison and width work in PR #42 (not in the base I read: `osc.va` there has unison 1 to 4 and no width).
