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
- The modules that are stereo today (chorus, reverb, delay out, compressor, vocoder, audio.in, out, pan out,
  the fx family) already use paired ports (`in_l`/`in_r`, `left`/`right`). They become stereo ports by
  metadata only. Their old half-cables keep working, so saved patches and presets render bit-identically.
- `split` and `join` are pure wiring (no DSP, no instance, no CPU).
- Estimated cost: about 3,000 lines in 5 PRs. Engine/CPU cost is small, 1.5% of the block budget for a
  16-voice patch (estimate, below). Biggest risk is the hot file `compile.rs` (2,900 lines).

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
   - *Native stereo port*: a declared pair of existing ports, `StereoPort { name, left, right }` in
     `ModuleInfo` (new field, default empty). The module code does not change: it keeps reading
     `io.input(0)`/`io.input(1)` and writing two outputs. The compiler resolves the pair name to both buffers.
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
3. **Stereo cable file form:** an ordinary `CableState` whose endpoint names a pair (`PortRef::Module { id, port: "out_st" }`).
   Pair names are aliases that never equal a real port name, so the old names still resolve: `in_l` and `in_r`
   remain real ports for half-cables. A legacy patch with two half-cables compiles exactly as today.
   Examples: chorus `in_st` = (`in_l`,`in_r`), reverb `in_st`, any fx `out_st`, `out` `in_st` = (`left`,`right`),
   delay `in_st` = (`in`, new `in_r`; unwired `in_r` falls back to the same buffer as `in` for a mono-wired delay to
   keep today's sound).
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
| Already stereo (Native pair, metadata only) | `chorus`, `reverb`, `compressor` (already stereo-linked: detector is `max(|l|,|r|)`), `vocoder` (stereo carrier), `eq`, `flanger`, `phaser`, `limiter`, `bitcrusher`, `out`, `pan` (out), `audio.in` (out), `delay` (out) | add `StereoPort` entries |
| Need real code | `split`, `join` (wiring only: compiler aliases buffers), a stereo `delay` input (`in_r` port), `osc.va` stereo output and `width` param, optional `widener` (mid/side, new, not required) | small |
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

- `CURRENT_SCHEMA_VERSION` 7 to 8 (`crates/core/src/format.rs`). Like v4 to v7 it is a strict superset: a v8 file with
  no stereo cable and no `split`/`join` loads as before, v1 to v7 files load unchanged, nothing is rewritten.
  The bump exists so that a v7 build refuses a v8 file instead of failing with `UnknownPort` on `in_st` or `split`.
- Migration of saved patches and factory presets: none. Old patches only name old ports. No op, no log rewrite.
- Bit-identical: yes by construction (no width-2 port, so the compiled schedule is the same), and enforced by the
  existing tests `legacy_patches.rs` (hashes in `golden/legacy_hashes.txt`) and `golden_renders.rs` (`golden_renders.txt`),
  which must pass unmodified in every PR. Step 1 adds a test that a patch rebuilt with stereo cables and L = R
  matches its mono render, and that a width-2 chain fed by a mono source equals the mono render sample for sample.
- New `osc.va` params default so the old sound is unchanged (`width` = 0). Presets opt in.
- `save` writes the current version always (existing behaviour); see choice C5.

## 5. UI needs (no visuals here; agent 3 owns them on `claude/visual-polish-proposal`)

- A stereo cable must be distinguishable from a mono one without relying on colour alone (`cable_look.rs` already enforces
  contrast per surface): needs a width/pair cue with a minimum thickness that still passes `worst_contrast`.
  `cable_look::paint` needs a `channels` argument and `points()` is reused.
- A stereo jack must be distinguishable and larger or paired. `rack.rs` finds jacks by `PortInfo` name and direction
  (`Rack::jack`): the pair needs one jack for `in_st` while the two real halves stay addressable for legacy cables.
  Proposal for the rack: show one stereo jack; a legacy patch with half-cables draws two cables into the same twin jack.
- Connect rules in the editor: mono to stereo and stereo to mono both allowed and shown with a small badge
  ("duplicated" / "summed"), so the conversion is visible.
- Inspect/scope (`Tap`/`ProbeTarget`, 1 lane): a stereo cable shows both channels (two traces) or a single mono sum plus width.
- `split` face: one stereo input jack, jacks `left` and `right`. `join` face: jacks `left` and `right` in, one stereo out.
  No knobs. Both are Utility category, `width_units` about 3. `out` shows one stereo input jack.
- Hover text on a cable states its width. `help.rs`/explain text for the new modules.

## 6. Plugin

- `crates/clap/src/lib.rs` already declares `main_output_channels: 2` and one stereo aux input (`aux_input_ports: [2]`),
  and passes `[f32; 2]` frames to `audio.in` through `input_rx`. No port change in the plugin.
- `audio.in` keeps `left` and `right` outputs and its "mono source plays on both" behaviour; it gains the pair `out_st`
  over them. Old patches keep their two cables. Nothing about untrusted-input clamping changes.
- Needs a REAPER render check for one stereo patch (spec in the standing REAPER recipe), to run silent per CLAUDE.md.

## 7. Build plan

Each step is one PR to master, tests green, golden hashes untouched unless stated.

| # | PR | Scope | Size | Notes |
|---|---|---|---|---|
| 1 | Engine: channels in the compiler | `StereoPort` in `info.rs`; channel axis in `compile_inner` (instances, buffers, per-channel `SumVoices`, delay slots, tap, state carry); conversion rules; unit tests with a test-only pair module; a `bench_patches` CPU run on a stereo `pad` | ~900 lines + ~400 tests, 2 sessions | One agent, sole owner of `compile.rs`. Critical path. Land first. |
| 2 | Native pairs and wiring modules | pair metadata on chorus/reverb/fx/out/audio.in/delay/pan/vocoder/compressor; delay `in_r`; `split`, `join` as compile-time aliases; schema 8; format tests | ~450 + ~250 tests | After 1. |
| 3 | UI: cables, jacks, faces | connect rules, twin jack, stereo cable paint, `split`/`join` faces, probe with two traces, help text, UI tests | ~700 | After 1's API (can start against the metadata of 2). Waits for agent 3's visual PR to merge, or lands with a `channels` argument only and no style decisions. |
| 4 | Stereo sources | `osc.va` stereo output + `width`, `pan` input pair, optional `widener` | ~350 | After 2. Touches `osc_va.rs`, which owns the unison spread work (PR #42). |
| 5 | Evidence and presets | stereo demo patch, REAPER check, `docs/benchmarks.md` entry, `docs/stereo/README.md`, golden for the demo only | ~250 | After 1 to 4. |

Splitting across agents and avoiding collisions:
- Presets agents (1 and 2) edit `tests/library.rs`, `crates/ui/examples`, `patches/`: steps 1 and 2 touch none of those.
  Steps never edit presets or `golden_renders.txt`/`legacy_hashes.txt` entries; step 5 only adds new demo lines at the end.
  Presets keep using half-cables until step 3 merges; none of them needs stereo cables to be mergeable.
- Agent 3 (visuals) owns `cable_look.rs`/`rack.rs` styling; step 3 only adds a `channels` parameter and the jack variants,
  and rebases on whichever lands first. Step 3 should start only after agent 3's PR merges, or take the fork risk.
- `compile.rs`: only step 1 changes it substantially. Any other agent touching it in the meantime should tell the overseer.
- One writer per worktree. Steps 2 and 3 can run in parallel after 1 merges; 4 after 2.

## 8. Choices for Kosta

| | Choice | Options | Recommendation |
|---|---|---|---|
| C1 | Channel model | (a) derived width 1 or 2 per port, static; (b) always 2 on every cable (Bitwig); (c) VCV dynamic count | (a) |
| C2 | Stereo into a mono input | average; left only; sum | average: exact for L = R and keeps level |
| C3 | How a stereo cable is named in the file | alias pair name on existing paired modules, legacy halves kept (no migration); or rename ports to single stereo ports and migrate | alias; zero migration risk and old patches stay bit-identical |
| C4 | Rack: half jacks of `in_l`/`in_r` | hidden unless a legacy cable uses them; or always shown beside the stereo jack | hidden unless used |
| C5 | Schema 8 written when | always (existing practice); only when a stereo cable or split/join exists | always: simpler, same as v4 to v7 |
| C6 | CV, gate, pitch cables | audio only; stereo CV too (stereo LFO) | audio only now |
| C7 | `split` and `join` | wiring only, no instance; or real DSP-free modules in the schedule | wiring only (zero cost; face is still a module) |
| C8 | Build order | steps 1 to 5 as above; or engine plus `osc.va` width first for a quick listen, UI after | steps as above |
| C9 | Wait for agent 3's visual PR before step 3 | yes; or build jack variants first and restyle later | yes |

## Risks and unknowns

- The 20 ns/sample figure for filters and the +40% to +80% range are estimates; step 1 measures them. If the measured cost
  exceeds 25% of budget at 16 voices on the Pi 4 target (unmeasured, as for every other benchmark), the fallback is
  skipping the second instance for a module whose two inputs are identical buffers (a broadcast mono feeding a Follow module stays one instance).
- `compile.rs` is large and shared; a mid-air collision costs more than the feature code. Land step 1 fast.
- Pair alias naming for `delay` (`in` is a real port) needs the alias `in_st` and an added `in_r`; audit the other 12 pair modules for
  port-name collisions before step 2.
- Not checked: the exact unison and width work in PR #42 (not in the base I read: `osc.va` there has unison 1 to 4 and no width).
