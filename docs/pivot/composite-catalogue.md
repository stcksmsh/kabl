# Composite candidate catalogue (pivot section C)

This is a read-only audit at `origin/master` 89d9c1be. It uses the corpus in `corpus-manifest.md`/`.csv`: 160 patches, of which 129 are in the Sounds browser and 19 are hidden engine demos.

In this document, **reach** means the number of patches that appear in the Sounds browser. The hidden demos and the `docs/` assets are listed separately. The patch names below drop the `patches/` and `presets/` prefixes; `demo:` marks a hidden engine demo.

## Method (derived from the graphs, not from guesses)

1. **Replay.** I replayed every `log.jsonl` the way `format::replay` does, and checked the result against `checkpoint.json` (all 158 folders match). I also decoded the two `.kabl` files.
2. **Mining.** I mined frequent connected induced subgraphs.
   - **Nodes:** modules, labelled by kind. The control hubs `macro`, `cues`, `clock` and `out` were removed from the graph; they always stay at the top level.
   - **Edges:** jack cables and module-param routes, labelled `from_port>to_port` or `from_port>@param`.
   - **Enumeration and identity:** ESU enumeration of every connected subset of 2–9 modules per patch, which gives 179,907 distinct shapes. Each shape is identified by a 3-round Weisfeiler–Lehman hash over kinds and edge labels.
   - **Ranking:** I kept the *closed* patterns, meaning those with no one-node extension that has the same support. I ranked them by support × size.
3. **Matching.** The miner surfaced a few recurring structures. For each one I wrote a role matcher that runs over every patch. The matcher returns exact members, a variant signature, all parameter values per role, and boundary statistics. The boundary statistics are:
   - jack and route crossings;
   - internal cables;
   - **cable-param routes that `encapsulate` would refuse.** I used its exact rule: the source module and `end_module` sit on different sides of the boundary (`crates/ui/src/composites.rs:193-205`, `crates/core/src/state.rs:192-201`).
4. **Cross-check.** The generator helpers that build the corpus encode the same blocks by hand:
   - `space_tail` (`tools/examples/presets_piece.rs:361`; copied into every `presets_*.rs`);
   - `Recipe`/`Voice` (`presets_bass_lead.rs:622-660`);
   - `add_line` (`presets_piece.rs:491`), `add_kit` (`:656`), `sum` (`:569`) and `divided` (`:465`);
   - `clock_duck`/`beat_duck` (`presets_bass_lead.rs:991`, `presets_keys_pluck/group2.rs:79`).

   The graph analysis and the code agree.

The scripts were scratch files and have been deleted, as the brief asked. The method above is enough to rebuild them. Most code is ESU plus a WL hash plus role matchers of about 300 lines.

### What the miner found (top closed patterns)

| support | size | shape |
|---|---|---|
| 93 | 3 | split.left → delay → reverb |
| 59 | 4 | compressor → split.left → delay → reverb |
| 27 | 5 | gain → chorus → split.left → delay → reverb |
| 20–22 | 7–9 | … → reverb → split → two mixers (wet L/R + drums) → join |
| 102 | 3 | midi.in.gate → env.adsr → vca.cv |
| 85 | 4 | midi.in → {osc.va.pitch, env→vca.cv} |
| 62 | 7 | osc.va → mixer.in1 → drive → filter.ladder → vca(env) → gain |
| 54 | 8 | the same, with a second env routed to `@cutoff_hz` |
| 22 | 9 | drum.grid → kick/snare/clap/hat(closed+open)/tom → mixers |
| 11–13 | 9 | seq.gate/pitch → osc → vca(env) → gain → filter.svf |
| 14 | 9 | drive → filter.ladder → gain → compressor ⇄ compressor (glue bus) |

## Ranked candidates

The value score is reach × legibility × standalone usefulness. Legibility is the number of modules folded into cards plus the internal cables hidden; standalone usefulness is rated 1–3. The "blocked" column counts browser patches where `encapsulate` would refuse today because of gap G1.

| rank | candidate | reach (browser patches / instances) | hidden demos | modules folded | internal cables hidden | standalone | blocked today (G1) |
|---|---|---|---|---|---|---|---|
| 1 | **Analog Voice** | 105 / 185 | 13 | 1,085 | 1,144 | 3 | 40 patches (48 instances) |
| 2 | **Space** (echo + room tail) | 97 / 98 | 1 | 309 | 331 | 3 | 2 (estuary, lichen) |
| 3 | **Sequenced Part** (seq + voice) | 32 / 80 | 2 | 418 | 520 | 2 | 3 (estuary, pawn-shop, stoop-sale) |
| 4 | **Drum Kit** (grid + 5 drums + submix) | 23 / 23 | 1 | 177 | 292 | 3 | 0 |
| 5 | **Glue Bus** (drive → tone → trim → comp, kick-ducked) | 16 / 16 | 0 | 66 | 66 | 2 | 2 (checkerboard, spark-plug) |
| 6 | Space Return (C2 stereo-return variant, nests C2) | 22 / 22 | 0 | about 5 per patch on top of C2 | 7 | 1 | 1 |
| 7 | Generative Line (random → quantizer → voice) | 1 (lichen) / 2 | 3 | 13 | 12 | 2 | 1 (lichen) |
| 8 | Tempo Pump (clock.div → env → gain-dB route) | 5 / 5 | 1 | 5 | 5 | 2 | 0 |

**Not composites.** These recurring structures were deliberately left out:

- **The `clock.div` ×2 cascade** (21 instances in 7 pieces). It exists only because `div` caps at 8 (`crates/modules/src/builtins/clock_div.rs:35-38`). That is a module limitation, not a musical block.
- **A single `lfo`** (vibrato `@fine` in 19 patches, cutoff in 16). It is one module.
- **The formant "mouth"** (`presets_voice.rs:918`, 3 patches). It is bespoke.
- **The acid glide** (seq → slew/crossfade, acid-line only). It is bespoke.
- **The macro and cues hubs.** They must stay top-level, because performance controls, CC and cues address them.

---

### C1. Analog Voice: rank 1

- **Purpose.** The subtractive keyboard voice behind most Bass, Lead, Pad, Voice and FX presets. One to four `osc.va` (sometimes plus noise) → mixer → drive → ladder filter swept by its own envelope → amp envelope → trim.
- **Occurrences.** 185 instances in 105 browser patches, plus 13 demos and 9 `docs/` assets.

**Truly equivalent core: 52 patches.** These share the same wiring: `osc.va×N → mixer → drive → filter.ladder(@cutoff ← fenv) → vca(cv ← aenv) → gain`, with `midi.in` pitch and gate fanned in. They differ only in parameter values, oscillator count, and an optional vibrato `lfo → osc@fine`.

| oscillators | vibrato lfo | patches |
|---|---|---|
| 1 osc | none | acid-line brick-dust copper-mile corner-store dune-buggy fault-line glass-coast lamp-oil ore-cart rope-walk scrap-metal slab-city still-water stoop-sale (the keyboard part of the Groove presets) |
| 2 osc | none | acid-rain cinder-block fire-escape loud-mouth oak-stave oatmeal picture-window pig-iron pump-house rail-yard rubber-band steel-toe tar-pit tin-voice vox-chop |
| 2 osc | yes | glass-needle horn-section hot-wire lantern low-tide parquet-loop salt-flat soft-sine spun-sugar tidepool |
| 3 osc | none | concrete-cloud dry-cleaner gutter rooftop tail-light undertow |
| 3 osc | yes | afterglow cotton-mill fog-bank glacier slate-roof waiting-room |
| 4 osc | none | deep-field |

**Superficially similar: 13 patches.** These have extra stages that would need a variant or should stay bespoke:
- ringmod in the chain: copper-wire, short-circuit, static-garden, temple-bowl;
- comb filter: whiteout;
- several trims: chandelier, oracle, sock-puppet, updraft, automaton, maw;
- no filter envelope: neon-highway;
- no drive (mixer → ladder): expressive/lead, palette/lead, sound-palette.

**Different families of voice:**
- **Piece keys voice**, `osc → vca → ladder → gain` (the filter sits after the VCA, with no filter envelope). In 9 pieces: balcony dead-calm estuary gantry laundromat lichen pawn-shop roller-rink spillway.
- **Keys/pluck dual-osc voice**, `2 osc → mixer → vca` (no filter). In 9 patches: checkerboard corduroy gumdrop matinee rivet-gun scaffold spark-plug spinet turnstile. This is `presets_keys_pluck/bank.rs:49 va()`.
- **Learning voices**, `osc → svf → vca`, in init-keyboard, reference, recipes/*, expressive/split, composition and performance. **Keep these flat:** they are the lessons.

**Parameter spread** (modal values in brackets) shows the instances are true parameterisations of one block:
- `env.adsr` attack 0.5–3200 ms (2), decay 15–5200 (300), sustain 0–1, release 6–5500 (60);
- ladder cutoff 130–20000 Hz (3500), resonance 0.02–0.6 (0.1);
- drive 0–31 dB (0), trim −12–3.7 dB (0);
- `osc.va` waveform 0–3 (2 = saw), unison 1–8, detune 0–45 (15);
- vca gain 0 in 200 of 201 instances;
- trim gain_db −60–28 (−6.02).

**Proposed public interface.** The generator's `Recipe` struct (`presets_bass_lead.rs:622-639`) already is this interface.

| control | underlying | default | range / unit |
|---|---|---|---|
| Wave 1 / Wave 2 | `osc.va.waveform` | saw | Sine, Tri, Saw, Pulse (selector) |
| Octave 1 / Octave 2 | `osc.va.base_hz` | 261.63 | as base_hz. G2 wants a −3…+2 octave view |
| Mix 2 | `mixer.level2` | 0.3 | 0–1 |
| Unison, Detune | osc 1 | 1, 15 | 1–8, 0–45 ct |
| Drive | `drive.drive_db` | 0 | 0–30 dB |
| Cutoff, Resonance | `filter.ladder` | 3500 Hz, 0.1 | 20–20 k Hz, 0–1 |
| Env Amount | **cable param** fenv → cutoff `amount` | per patch | −1…1. **Needs G1** |
| Filter A/D/S/R | fenv | 2 / 300 / 0.3 / 100 | ms, 0–1 |
| Amp A/D/S/R | aenv | 2 / 300 / 0.6 / 90 | ms, 0–1 |
| Level | `gain.gain_db` | −6 | dB |

**Ports:**
- *Inputs:* Pitch (pitch type); Gate (gate type); Velocity (unipolar, used by FM voices); Wheel → Cutoff (cv).
- *Modulation inputs:* Cutoff, Level, Drive, Detune.
- *Output:* Audio, mono.

**Effect per patch.** 5–9 modules and 6–17 cables leave the top level. That is 1,085 modules across the corpus, the largest legibility gain.

**Standalone.** High. It is the "classic synth voice" a new user expects.

**Render risk.** None when the voice is encapsulated over existing leaves (metadata only). There are two exceptions:
- G3 fan-in, if it is fixed by inserting pass-through modules instead of with metadata;
- an "Env Amount" control that writes to anything other than the existing cable param.

**Blocked today: 40 patches.** The macro → (fenv cable)`@amount` depth route crosses the boundary in: afterglow chandelier cinder-block corduroy cotton-mill deep-field estuary fire-escape fog-bank functional-cables/macro-morph glass-needle gumdrop gutter horn-section hot-wire lantern lichen low-tide oak-stave oatmeal parquet-loop pawn-shop pig-iron pump-house rail-yard rooftop short-circuit slate-roof soft-sine spark-plug spun-sugar static-garden steel-toe stoop-sale tail-light tar-pit tidepool updraft waiting-room whiteout.

### C2. Space (echo + room): rank 2

- **Purpose.** The ambience tail every preset ends in: mono slap or tempo echo into a stereo room. This is `space_tail`, copied into every generator.
- **Core.** `split.left → delay → reverb`, with the reverb output going to `out`. There is an optional clock → `delay.clock` (sync), and an optional pre-stage.
- **Occurrences.** 98 instances in 97 browser patches, plus demo:scale-walk and lichen-plus. The 30 browser patches with a reverb but no delay → reverb path are *not* matched.

| variant | patches | list |
|---|---|---|
| Compressor before split, mono out, no macro | 17 | checkerboard cinder-block cold-cut concrete-cloud dry-cleaner gutter horn-section linoleum oatmeal pig-iron rivet-gun scaffold short-circuit soft-sine spark-plug tar-pit tin-voice |
| The same, macro on both mixes | 15 | copper-wire depth-charge fire-escape glass-needle hot-wire lantern low-tide oak-stave oracle rubber-band slate-roof spun-sugar tail-light temple-bowl updraft |
| The same, synced echo | 4 + 3 | loud-mouth parquet-loop steel-toe turnstile; afterglow rail-yard vox-chop (with macro) |
| Chorus → split | 11 + 9 + 4 + 2 + 1 | cellar-door cotton-mill dry-creek gossamer gumdrop hand-bell pulpit rooftop rosewood spinet tin-roof; boxwood chandelier deep-field fog-bank glacier glass-elevator salt-flat static-garden tidepool (macro); matinee rain-gutter snowmelt willow (macro on decay and feedback); pump-house waiting-room (synced); brass-filament |
| Synced, **stereo return** (wet L/R summed with dry drums → join) | 17 + 2 + 1 + 1 + 1 | brick-dust copper-mile corner-store dune-buggy fault-line glass-coast lamp-oil lichen loading-dock ore-cart pawn-shop rope-walk scrap-metal slab-city spillway still-water stoop-sale; balcony roller-rink; acid-line; dead-calm; estuary. This is C6 |
| Other pre-stage | 6 | phaser (corduroy velvet-tine), eq (picture-window), bitcrusher (whiteout), none (cue-macros macros-eight expressive/lead palette/lead) |

**Equivalence.** The wiring is identical across all variants: split.left → delay.in → reverb.in. The pre-stage is *outside* the block. Internal constants are near-universal:
- reverb predelay 0 ms in 90/94 instances;
- width 60 % (67/92) or 70 % (23/92);
- damp 3500 Hz (62/95) or 5000 Hz (24/95).

Mix defaults to 0 % in 53/99 (a macro opens it).

**Proposed public interface:**

| control | underlying | default | range / unit |
|---|---|---|---|
| Echo | `delay.mix` | 0 | 0–100 % |
| Time | `delay.time_ms` | 375 | ms (60 is the slapback mode) |
| Sync | `delay.sync` | Free | Free, 1/16, 1/8, 1/8d, 1/4 (selector, 0–4) |
| Feedback | `delay.feedback` | 30 | 0–95 % |
| Tone | `delay.tone_hz` | 3500 | Hz |
| Room | `reverb.mix` | 0 | 0–100 % |
| Decay | `reverb.decay_s` | 0.8 | s |
| Damp | `reverb.damp_hz` | 3500 | Hz |
| Width (advanced) | `reverb.width` | 60 | % |
| Predelay (advanced) | `reverb.predelay_ms` | 0 | ms |

**Ports:**
- *Inputs:* In (audio, stereo cable into `split`); Clock (gate, into `delay.clock`; optional).
- *Output:* Out (audio, stereo).
- *Modulation inputs:* Echo, Room, Decay, Feedback (where macros land today).

**Effect per patch.** 3 modules (4 with chorus) and 1–3 cables leave the top level. The macro "Space" routes stay outside and land on the Echo and Room ports. Wanting one "Space" knob inside the card is gap G2.

**Standalone.** High (an FX block).

**Render risk.** None. Two exceptions:
- **estuary and lichen are blocked by G1** (macro morph routes into the delay/reverb mix routes' `amount`);
- **acid-line stores mix values of 0.16 and 0.08**, a probable unit slip (see the manifest).

### C3. Sequenced Part: rank 3

- **Purpose.** A self-playing line: step sequencer → voice → level → tone filter. This is `add_line` in pieces and `seq_part` in Groove presets.
- **Occurrences.** 80 instances in 32 browser patches.

| family | wiring | instances | patches |
|---|---|---|---|
| A. Piece line | seq → osc → vca(env) → gain → filter.svf | 34 | balcony dead-calm estuary gantry laundromat lichen pawn-shop roller-rink spillway |
| B. Groove part | seq → osc + sub-osc → mixer → vca(env) → gain (± svf) | 23 | brick-dust copper-mile corner-store dune-buggy fault-line glass-coast lamp-oil ore-cart rope-walk scrap-metal slab-city still-water stoop-sale |
| C. Learning pieces | seq → osc → filter → vca | 9 | sequence echo interlocking cue-macros functional-cables/{echo-throws,morph-arc,five-against-eight} |

Family C should stay flat: it is the teaching material.

**Equivalence.**
- A and B are each wiring-equivalent within the family.
- The seq `length` (4/5/7/8/15/16/32) and step data differ. That is the pattern, which is content, not structure.
- In A, a clock-divided line (the pad) adds a `clock.div` pair at the input.

**Proposed public interface.**
- **Controls:**
  - Steps (`seq.length`, 1–32);
  - Gate (`seq.gate_len`, %);
  - Octave and Wave (osc);
  - Unison and Detune;
  - Attack, Decay, Sustain, Release (env);
  - Level (`gain.gain_db`, modal −8 dB);
  - Tone (`filter.svf.cutoff_hz`, modal 1400 Hz).
- **Ports:**
  - *Inputs:* Clock (gate), Reset (gate). *Output:* Out (audio).
  - Pitch and Gate outputs are optional, for chaining.
  - *Modulation inputs:* Gate length, Tone, Level.

**Effect per patch.** 6 modules and 6 cables per instance. This is the biggest gain in pieces: Lichen 3 parts, Balcony and Roller-rink 5 each.

**Standalone.** Medium. It is a useful building block for a user's own pieces.

**Render risk.** None (metadata). **Cue references stay valid**, because cues address seq IDs (`cueN.seq.<id>`) and grouping keeps leaf IDs. Insert and duplicate remap them (`crates/ui/src/composites.rs:375,415`). Pattern editing stays in Inspect internals (G6).

### C4. Drum Kit: rank 4

- **Purpose.** `drum.grid` → kick, snare, clap, hat (closed + open) and tom → a two- or four-stage submix, with an optional ladder on the clap/hat/snare bus. This is `add_kit`.
- **Occurrences.** 23 browser patches.

| variant | patches |
|---|---|
| 2 mix stages, kick feeding a sidechain | acid-line brick-dust copper-mile corner-store dune-buggy fault-line glass-coast lamp-oil ore-cart rope-walk scrap-metal slab-city still-water stoop-sale (14) |
| 4 mix stages | dead-calm lichen pawn-shop roller-rink spillway (5); balcony gantry laundromat, with sidechain (3) |
| 2 stages, no sidechain | grid-cues, plus demo:four-floor |

**Equivalence.** The wiring is identical within each variant; every kit uses all 5 voices. The parameters are true per-patch variation:
- kick decay 160–1800 ms, kick level −12–0 dB;
- hat level −22 to −2 dB;
- mixer levels.

**Proposed public interface.**
- **Controls:**
  - Kick Tune (30–120 Hz), Kick Decay (60–2000 ms), Kick Drive (0–30 dB);
  - Snare Decay, Snare Level;
  - Clap Level, Hat Level;
  - Tom Tune, Tom Level;
  - Bus Tone (ladder cutoff, where present).
- **Ports:**
  - *Inputs:* Clock (gate), Reset (gate).
  - *Outputs:* Out (audio, mix), Kick (audio, for the sidechain), Hats/Bus (audio, for the space send).
  - *Modulation inputs:* the level ports that macros drive (0.9 on average).

**Effect per patch.** 8–10 modules and 12–14 cables become one card. This is the cleanest win in the corpus: 0 blocked, 12.7 internal cables per instance on average.

**Standalone.** High (a drum machine).

**Render risk.** None. Drum-grid banks and cues keep their IDs.

### C5. Glue Bus: rank 5

- **Purpose.** The master colour and dynamics stage of Groove presets and pieces: [drive] → ladder tone → trim → compressor → second compressor ducked by the kick on its `sc` input.
- **Occurrences.** 16 browser patches.
  - With the ducked second compressor: acid-line balcony corner-store dune-buggy fault-line lamp-oil ore-cart slab-city still-water stoop-sale.
  - Single compressor: checkerboard cold-cut lichen roller-rink spark-plug spillway.
- **Equivalence.** The wiring is equivalent within each of the two variants. Pieces carry 1–2 vestigial unity mixers in the chain (Lichen #54 and #56). Parameters vary: comp threshold −34 to 0 dB, ratio 1–10.9, ladder cutoff 180–20 k Hz.

**Proposed public interface.**
- **Controls:** Drive (dB), Tone (Hz), Resonance (advanced), Trim (dB), Threshold, Ratio, Attack, Release, Duck (sidechain threshold).
- **Ports:**
  - *Inputs:* In (audio), Duck In (audio; the kick).
  - *Output:* Out (audio).

**Effect per patch.** 4–7 modules.

**Standalone.** Medium.

**Render risk.** None (metadata). acid-line stores `makeup_db = −2` although the declared range is 0–24. Any public range must therefore not clamp stored values (G2).

### C6–C8: lower value

- **C6. Space Return.** Reverb → split → mixer L/R (with dry drums) → join, in 22 patches (the stereo-return list under C2). It should **nest C2** rather than be a sibling definition. It adds a "Dry In" audio port for the drums.
- **C7. Generative Line.** `clock.div → random → quantizer → (pitch, trig) → C1-style voice`. It appears in lichen ×2, lichen-plus, and the demos scale-walk, clock-logic and robot-voice. It is useful as a standalone "self-playing melody" for the learning path. In the corpus it is Lichen-specific, and **Lichen's two instances are blocked by G1** (macro morph routes).
- **C8. Tempo Pump.** `clock.div(÷4) → env → depth route to gain_db`. It appears in pump-house rail-yard steel-toe turnstile vox-chop, plus demo:robot-voice (a vca variant). It is a 2-module card; it is worth having only if a "sidechain" block is wanted for teaching.

---

## Lichen and Lichen Plus

**Today.** Lichen (`presets/lichen`) has 76 modules and 135 cables:
- 32 macro routes;
- 14 clock fan-out cables;
- 10 cable-param morph routes;
- 79 others.

**With C1–C6 applied** (my grouping, computed on the real graph):

| card | leaves |
|---|---|
| Bass line (C3) | #4–9 |
| Mid line (C3) | #10–15 |
| Pad line (C3, with its ÷16 clock.div pair inside) | #16–23 |
| Keys voice (C1, piece variant) | #25–29; midi.in #24 stays outside |
| Gen line A (C7) | #32–38 |
| Gen line B (C7) | #39–45 |
| Echo space (C2, mono-summed) | #46–50 |
| Glue bus (C5) | #51–58 |
| Drum kit (C4) | #59–68 |
| Space return (C6) | #69–75 |

- **Still top level:** Macro #1, Cues #2, Clock #3, midi.in #24, the shared `clock.div` #30 (÷2 for both generative lines), mixer #31 (its two inputs carry functional-cable step patterns, so it stays visible as the "probability mixer"), and Out #76.
- **Result:** 17 top-level objects (10 cards + 7 modules) instead of 76 modules, and 62 top-level cables instead of 135 (73 become internal).
- **What remains visible:** 32 macro routes, 14 clock/reset cables, 4 mixer and 3 svf audio sends into the bus, and the hub cables. Macro routes then dominate the top level. The "Mutate/Reach/Gate/Echo/Focus" performance layer is legible as such, but G3-style fan-in (one macro → many card ports) and G2 (a card-level "Focus" knob) are what would shrink it further.
- **Today's infrastructure gives less:**
  - Gen lines A and B and the Echo space are refused, because routes #66–67, #69–70 and #72–79 (`macro.m1/m4 → cable@amount` crossfades) cross the boundary.
  - Without G1 the result is 21 objects and 67 cables, with the two generative lines left flat.
- **What stays bespoke in Lichen:**
  - the macro/cue performance layer;
  - the probability mixer #31 with step-pattern cables;
  - the morph crossfades (macro m1 ↔ m4 on route amounts);
  - the per-piece mixer levels.

  These are the composition, not reusable blocks.

**Lichen Plus** (`docs/promo/lichen-plus.kabl`, 112 modules) is Lichen plus four more Piece lines, each with its own `clock.div` pair. That is exactly C3 ×4 (#51–58, #59–66, #67–74, #75–81, with dividers #52–53, #60–61 and #68–69). The storyboard's "Disco composite" (`docs/promo/STORYBOARD-V1.md:35`) is therefore a 4-part C3 group. It is a natural first nested composite: a parent "Disco" holding four C3 children. The file contains **no composite records today**; the "composite" is only a module group.

## Existing D09/D10 infrastructure

| aspect | today | code |
|---|---|---|
| Model | `Composite {name, help, definition{id,version}, parent, members, pos, ports, controls, next_interface, panel}`; `Exposure {label, target: PortRef}` | `crates/core/src/composite.rs:9-37` |
| Ownership | Flat leaves stay authoritative. The DSP compiles the flat graph, with no wrapper node | `composite.rs:1`; `docs/composites/design.md` §Representation |
| Validation | ≤128 composites; nesting ≤8; no cycles or repeated definitions on an ancestor path; ≤32 ports and ≤32 controls; a control must bind a module `Param` | `composite.rs:87-187` (control rule `:180-186`) |
| Engine check | Bindings must name real ports and params of a built-in | `crates/engine/src/compile.rs:994,3334-3366` |
| Operation and file | `Op::SetComposite` (schema ≥4; current schema 8). Load replays the log; the checkpoint is not consulted | `crates/core/src/op.rs:56-60`, `format.rs:22,64` |
| Host state | Composites ride inside SoundState v5/v6 | `crates/clap/src/sound_state.rs:75-98` |
| Encapsulate | Auto-exposes every boundary target with label `port · #id`. Pins become controls. **Cable-param routes across the boundary are refused** | `crates/ui/src/composites.rs:155-239` |
| Expose / unexpose | Real leaf ports and params only. Removal is refused while a boundary cable is connected | `composites.rs:245-297` |
| Package, insert, duplicate | Package schema 1. A closed dependency set is required for cue/clock references; references are remapped on copy. Document caps: 128 modules, 512 cables, 8 delay/chorus/reverb | `composites.rs:20-90,298-482` |
| Library | Immutable `<user root>/composites/<definition-id>/vN.json`. Publishing never retargets. **The definition id is random** (`kabl-<pid>-<nanos>-<n>`) | `composites.rs:125-137,483-545` |
| Panels (D10) | Optional `Panel` per composite. Width 180–840 pt, height 340. Placements are keyed by interface id; kinds are knob, selector and jack. Optional light/dark PNG art (≤128 KiB) | `crates/core/src/panel.rs:6-130`; `crates/ui/src/panels.rs:19` |
| Panel status | The owner disposition is "changes requested" | `docs/panel-authoring/design.md:3` |
| Tests | Exact PCM grouped vs flat (voice, stereo FX, feedback boundary); cue/clock dependencies; chronological undo | `crates/ui/tests/composites.rs:193,475,554`; `crates/ui/tests/panels.rs` |

**What a composite can expose today:**
- **Ports:** any member port, or a member param as a modulation input. Port type comes from the leaf: Audio, Cv, UnipolarCv, Gate or Pitch (`crates/modules/src/info.rs:29-41`). Clock, reset and trigger are all `Gate`.
- **Controls:** 1:1 aliases of one member param, with the leaf's range, unit, taper and default. Only the label can be customised.
- **Panel and help:** a custom face (knob/selector/jack) and help text.
- **Nesting:** nested child composites.

**What it cannot expose:**
- a cable param;
- one port for several targets;
- a remapped range or default;
- a step or grid widget.

## Gaps the catalogue needs (minimum extensions)

| # | gap | evidence | proposed fix (keeps D09 metadata-only semantics) |
|---|---|---|---|
| **G1** | Cable-param routes cannot cross a boundary, and a cable param cannot be exposed | Blocks C1 in 40 patches, plus parts of C2, C3 and C5 and Lichen's generative lines. 54 browser patches use 121 cable-param routes. `composites.rs:193-205`, `composite.rs:159-166` (`module_id()` is `None` for `CableParam`) | Allow `Exposure.target = CableParam` when the cable is internal: as a control ("Env Amount") and as a modulation-input port. Encapsulate then exposes it instead of refusing. Owner is "the module at `end_module`" |
| **G2** | A control is a raw alias: no musical range, unit or default, no one-to-many mapping, no curve | Space scales echo/room by per-patch `echo_max`/`room_max` (`presets_piece.rs:389-393`). Voice "Octave" is stored as `base_hz` | Add optional presentation mapping on `Exposure` (display min/max/unit/curve, or a scaled multi-target), written through the existing `param_editor`. Must never clamp stored out-of-range values (acid-line `makeup_db −2` below its 0 minimum; reverb `decay_s` 0.25 below its 0.3 minimum in some space tails) |
| **G3** | No fan-in: one public input → N internal targets becomes N ports | Voice: 947 inbound ports vs 674 distinct outside sources; duplication in 115 instances (pitch → 2–4 oscs, gate → 2 envs). Space: 346 vs 234, 70 instances (macro → both mixes) | `Exposure` holds a target list for inputs. Encapsulate groups crossings by the external source. No thru modules (they would risk render identity) |
| **G4** | No stable definition identity and no factory composite bank | Random ids (`composites.rs:125-137`). The library is user-root only (`:483-545`). Packaging packs only `patches/` | Deterministic ids for factory definitions (e.g. `kabl.space` v1), and a read-only factory `composites/` packed like `patches/` for Insert. Instances still embed and never auto-update |
| **G5** | Generators cannot emit curated composites reproducibly | Tools build with `PatchEditor` (`presets_piece.rs:52`). `encapsulate` takes only a name; labels come from `binding_label` (`composites.rs:233`) | An `encapsulate_with(definition, exposures{target, label, mapping}, panel)` API, so the generators in `tools/examples/presets_*.rs` emit stable logs |
| **G6** | Panels have no pattern widgets | `panel::Kind` is Knob, Selector or Jack only (`panel.rs:10-15`) | Accept pattern editing via Inspect internals for now; a step or grid placement kind is later work. D10 UX itself is "changes requested" |
| **G7** | Render-identity coverage | `golden_renders.txt` covers 26 patches and **none of the 100 presets**. The harness plays one chord and no cues or macros (`crates/engine/tests/golden_renders.rs:31-58`) | Record hashes for all patches before migrating (below) |
| G8 | Clock and reset are two cables per part | 14 clock cables remain at Lichen's top level | Minor. Leave it, or later add a "transport" fan-in under G3 |
| G9 | Composite commits validate the whole document against 128 modules and 8 delay/chorus/reverb | estuary has 103 modules, sound-palette 100 | Fine today. Keep an eye on headroom |

## Recommended first batch

**Composites.** Pick these because they are high value and need no gap fix, or only G4/G5:
1. **Space** (C2, with C6 as a nested variant);
2. **Drum Kit** (C4);
3. **Sequenced Part** (C3, families A and B);
4. **Glue Bus** (C5).

Add **Analog Voice** (C1) in the same batch only once G1 (cable-param exposure) and G3 (fan-in) land. Before that, it applies cleanly to 65 of its 105 patches, but with ugly duplicated ports.

**Infrastructure to build first:**
- G4/G5 (stable ids plus curated encapsulate for generators) are prerequisites for migrating generated patches.
- G1 and G3 unlock C1 and Lichen.
- G2 is polish and can follow.

**Representative patches to migrate first:**
- **lichen.** It exercises C2, C3, C4, C5 and C6, cues, macros and morph routes: the validation case.
- **laundromat and balcony.** These are Kosta's "kabl sound" pieces; they cover C3 A, C4 with 4 stages plus sidechain, and C6.
- **corner-store.** A Groove preset: C1, C3 B, C4 2-stage, C5 with sidechain, C2 stereo return.
- **willow.** Keys: C2 with chorus and the macro on decay/feedback, FM6 source. A small, simple check of standalone Space.
- **cinder-block.** C1 with the G1-blocked env-amount route: the regression case for G1.

Leave the learning patches flat: init-keyboard, reference, recipes/*, sequence, echo, interlocking.

**How to prove behaviour is preserved:**
1. **Structural identity (exact, no audio).** For every migrated patch, assert that the replayed `modules`, `cables`, `labels` and `tables` equal the pre-migration state, with only `composites` added. Compile ignores grouping (`docs/composites/design.md`; exact-PCM test at `crates/ui/tests/composites.rs:193`), so this alone implies bit-identical DSP. Write it as a test that loads each patch at the base commit (or from a copy) and at the new commit.
2. **Golden hashes for the whole corpus, recorded before any migration.**
   - On the base commit, run `KABL_WRITE_GOLDEN=1 systemd-run --user --scope -p MemoryMax=10G -p MemorySwapMax=2G cargo test -p kabl-engine --test golden_renders`. The test walks `patches/` to depth 3, so presets and demos get rows (`golden_renders.rs:12-28,62-78`).
   - Land that list as its own text-only PR.
   - Each migration PR must then pass `cargo test -p kabl-engine --test golden_renders` unchanged.
   - `crates/engine/tests/legacy_patches.rs` keeps guarding the 22 legacy rows.
3. **Longer transport renders.** Run `cargo run --release -p kabl-engine --example render_hash -- 20 <patch dirs…>` (`crates/engine/examples/render_hash.rs:1-9`) before and after the migration. It plays a chord then single notes, runs clocks and sequencers from the start, and prints one FNV hash per patch. The hashes must match.
4. **Cues, macros and automation.** Render the generators' scripted phrases (the `clips` subcommands, for example `presets_piece.rs:3218`) before and after, and compare the PCM hashes. Add a cue-firing render for Lichen modelled on `crates/engine/tests/cue_macros.rs`. Also load the migrated preset through SoundState save/recall (`crates/clap/src/sound_state.rs`) and compare state with only `composites` added.
5. **Leaf identity.** Any generator change must keep leaf and cable IDs and insertion order. Cable-ID accumulation order is part of the PCM contract (`docs/composites/design.md` §Representation). The safest migration appends `SetComposite` groups after the existing ops, rather than rebuilding.
