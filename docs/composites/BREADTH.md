# Composite breadth audit: small blocks that recur across the factory patches

This is a read-only audit at `origin/master` 9855c726. It is the second pass after
`docs/pivot/composite-catalogue.md`. That pass ranked big blocks and gave the six factory
composites in `composites/` (Analog Voice, Space, Sequenced Part, Sequenced Part + Sub, Drum
Kit, Glue Bus). This pass looks for the small, general blocks (2–6 modules) that many patches
share, inside or next to those six.

**Reach** is the number of distinct Sounds-browser patches (129, per
`docs/pivot/corpus-manifest.csv`, `user_facing = yes`). **Instances** counts every match, so a
patch with three amp envelopes counts once in reach and three times in instances. Hidden demos
are not counted.

## 1. Method

The first pass's method, rebuilt as scratch scripts (deleted, as before):

1. **Replay.** Every `log.jsonl` replayed the way `format::replay` does and compared with
   `checkpoint.json`: all 158 folders match (modules, params, cables).
2. **Mining.** List every connected group of modules in every patch, then give groups with the
   same shape the same fingerprint, so identical blocks can be counted across patches. Hubs
   `macro`, `cues`, `clock` and `out` are removed first.
   - *ESU* is the standard algorithm that lists every connected group of N modules exactly once.
   - *WL hash* is the fingerprint: it combines each module's kind with its cable labels
     (`from_port>to_port` / `from_port>@param`) over 3 rounds of neighbours.
   - *Closed patterns* only: a shape is dropped when a shape one module bigger occurs in exactly
     the same patches, so only the largest form of each block is listed.
3. **Role matchers** for each candidate, run over every browser patch. They return members,
   parameter values per role, the cables that cross the boundary and the hub routes that land
   on members.

What changed from the first pass:

- **Size 2–6** instead of 2–9, and **ranked by reach** first, not support × size.
- **The six are matched first.** I wrote a strict matcher for each built definition (the exact
  wiring in its `v1.json`) and mined a second time on the *residual* graph, the modules none of
  the six claims. That is how the blocks next to the six surfaced.
- **Each candidate has two counts:** instances wholly inside one of the six, and instances
  wholly outside. None of the recommended four has an instance that straddles one of the six;
  the few that do elsewhere are noted as partial.
- **Exposures** come from the graph: hub routes (`macro`, `clock`) that land on a member are
  what a user or a performance control turns today.

Strict matches of the six (these are the "inside" counts below and the baseline in §5):

| composite | reach | instances | modules |
|---|---|---|---|
| Analog Voice (`osc.va ×1–2 → mixer → drive → ladder → vca(env) → gain`, optional filter env) | 65 | 65 | 565 |
| Space (`split.left → delay → reverb`) | 93 | 93 | 279 |
| Sequenced Part | 9 | 42 | 246 |
| Sequenced Part + Sub | 13 | 22 | 162 |
| Drum Kit (grid + voices + kit-only submix) | 23 | 23 | 200 |
| Glue Bus (`[drive] → ladder → gain → comp [→ comp(sc)]`) | 12 | 12 | 57 |

These are lower than the first pass's reach for Analog Voice (105) and Glue Bus (16). That
pass counted loose families; this one counts only wiring the built definitions can host. For
example, the Glue Bus in the pieces has unity mixers between its stages and does not match.

## 2. Ranked candidates

Ranked by total reach. "Inside" means inside one of the six.

| # | candidate | modules · internal cables | reach / instances: total · inside the six · outside | relation to the six | standalone (1–3) | exposures it would need |
|---|---|---|---|---|---|---|
| 1 | **Amp Env** | `env.adsr → vca.cv` · 2 · 1 | 106 / 188 · 74 / 129 · 45 / 59 | subset of Analog Voice, Sequenced Part, Sequenced Part + Sub | 2 | Attack, Decay, Sustain, Release (macro routes: release 26, decay 16, attack 12, sustain 7). Ports: In, Gate, Out, Env Out (the envelope also drives something outside in 19 instances) |
| 2 | **Dual Osc** | `osc.va → mixer.in1`, `osc.va → mixer.in2` · 3 · 2 | 81 / 94 · 59 / 68 · 23 / 26 | subset of Analog Voice and Sequenced Part + Sub | 2 | Wave 1/2, Unison, Detune (macro 27), Osc 2 tune (`base_hz`, `fine`), Level 1/2 (macro 27). Ports: Pitch (one source feeds both oscillators in 88 of 94), Fine mod 1/2 (vibrato, 26 + 22 crossings; 40 macro routes on those cables' amounts become ports, as `ENCAPSULATE-WITH.md` § Cable parameters describes), Extra In (`mixer.in3`, 26), Out |
| 3 | **Tone Stage** | `[drive] → filter.ladder → gain` · 2–3 · 1–2 | 48 / 48 · 12 / 12 · 36 / 36 | subset of Glue Bus (its first three stages); independent elsewhere | 3 | Drive, Drive trim, Tone (Hz), Resonance, Trim (macro: trim 30, tone 23, drive 13, resonance 9). Ports: In, Out, Cutoff mod (17), Trim mod (3) |
| 4 | **Stereo Sum** | `split.left/right → mixer.in1 ×2 → join.left/right` · 4 · 4 | 26 / 26 · 0 / 0 · 26 / 26 | independent; sits right after Space's reverb in 22 patches (it is the extra half of the first pass's C6, Space Return: Space plus a stereo return mixed with dry drums) | 2 | Return level, Dry level (L and R are equal in all 26, so one knob each needs a two-target control, CI5: the planned presentation mapping that lets one control drive several targets with a display range and unit). Ports: In (stereo return), Dry In (`mixer.in2` both sides, one source in 23 of 25), Out |
| 5 | Keys Voice | Dual Osc + Amp Env · 5 · 4 | 23 / 33 · 13 / 22 · 11 / 11 | the Sequenced Part + Sub core without its sequencer | — | assembly of #1 and #2, see §4 |
| 6 | Post-VCA Voice | Amp Env + Tone Stage (`vca(env) → [drive] → ladder → gain`) · 4–5 | 18 / 18 · 0 · 16 / 16 (+2 partial) | independent (the filter sits after the amp, so Analog Voice cannot host it) | — | assembly of #1 and #3, see §4 |
| 7 | FM Voice | `osc.fm6` + Tone Stage · 4 · 3 | 16 / 16 · 0 · 15 / 15 (+1 partial) | independent | — | one module plus #3, see §4 |

Where the outside instances are, by what feeds them:

- **Amp Env** (59 outside): piece keys voices `osc → vca → ladder` (9), keys/pluck `mixer → vca →
  drive` (9), learning voices `svf → vca` (10), and the rest one or two each.
- **Dual Osc** (26 outside): plain pair 23, pair plus noise or ringmod on `in3` 3.
- **Tone Stage** (36 outside): after `osc.fm6` 15, after a VCA 16 (7 with drive, 9 without),
  on a Groove bus after a mixer 5.
- **Stereo Sum** (26): split fed by the reverb 22, by a chorus 2, by a compressor 2.

**Parameter variants per role** (min–max over all instances; candidates that differ only in
these values are one candidate):

| candidate | role | ranges |
|---|---|---|
| Amp Env | env | attack 0.5–3200 ms, decay 20–2600 ms, sustain 0–1, release 6–5500 ms |
| | vca | gain 0 in all 188 |
| Dual Osc | osc 1 | waveform 0–3, unison 1–8, detune 0–37, pw 20–50, `base_hz` 65.4–523.3 |
| | osc 2 | waveform 0–3, unison 1–4, detune 1.4–20, fine −30–10, `base_hz` 32.7–523.3 |
| | mixer | level1 0.07–1, level2 0–0.6, level3 0–1 (26 instances use it) |
| Tone Stage | drive | `drive_db` 0–29.7, `trim_db` −10–5 (present in 39 of 48) |
| | ladder | cutoff 180–20000 Hz, resonance 0–0.6 |
| | trim | `gain_db` −12–14.5 |
| Stereo Sum | mixers | level1 (return) 0.78–1, level2 (dry) 0.3–1, level3 1; L = R in all 26 |

## 3. Recommended set for 1.0: four composites

**Tone Stage, Stereo Sum, Dual Osc, Amp Env.** Why these four:

- They have the widest reach outside the six (36, 26, 23 and 45 patches), so they are the
  blocks still flat after the six are in.
- They do not overlap each other, and together they build every voice family the six cannot
  host: Keys Voice = Dual Osc + Amp Env, Post-VCA Voice = Amp Env + Tone Stage, the Corduroy
  family = all three, FM Voice = `osc.fm6` + Tone Stage. One small set covers these families,
  so they need no voice definitions of their own (rows 5–7 stay assemblies).
- Tone Stage and Stereo Sum also have a clear use on their own: a colour insert for any
  source, and a way to sum a stereo return with a dry part.
- Space Return (C6) becomes Space + Stereo Sum side by side, with no nested definition. SPACE.md
  says nesting cannot be generated yet.

Use in the existing patches (outside the six, factory generators only; the lessons in
`crates/ui/tests/*`, `cable_capture.rs` and `chrome_capture.rs` stay flat, as the first pass
said):

| composite | patches / instances | patches per generator file |
|---|---|---|
| Tone Stage | 36 / 36 | `presets_keys_pluck/group2.rs` 14, `presets_piece.rs` 9, `presets_keys_pluck/bank.rs` 6, `presets_groove.rs` 5, `presets_pilot.rs` 2 |
| Stereo Sum | 24 / 24 | `presets_groove.rs` 14, `presets_piece.rs` 9, `presets_pilot.rs` 1 |
| Dual Osc | 12 / 12 | `group2.rs` 7, `bank.rs` 2, `presets_groove.rs` 2, `presets_pad.rs` 1 |
| Amp Env | 26 / 32 | `presets_piece.rs` 9, `group2.rs` 7, `presets_pad.rs` 3, `presets_groove.rs` 3, `bank.rs` 2, `presets_voice.rs` 1, `presets_pilot.rs` 1 |

Patches, for spot checks (`×n` = instances in that patch):

- **Tone Stage:** balcony boxwood brass-filament brick-dust cellar-door copper-mile corduroy
  dead-calm dry-creek estuary gantry glass-coast glass-elevator gossamer gumdrop hand-bell
  laundromat lichen linoleum matinee pawn-shop pulpit rain-gutter rivet-gun roller-rink
  rope-walk rosewood scaffold scrap-metal snowmelt spillway spinet tin-roof turnstile
  velvet-tine willow. Inside Glue Bus: the 12 Glue Bus patches.
- **Stereo Sum:** acid-line balcony brick-dust copper-mile corner-store dead-calm dune-buggy
  estuary fault-line gantry glass-coast lamp-oil laundromat lichen loading-dock ore-cart
  pawn-shop roller-rink rope-walk scrap-metal slab-city spillway still-water stoop-sale.
  Lessons: `explain.rs` 1, `sound_palette.rs` 1.
- **Dual Osc:** checkerboard corduroy gumdrop matinee progression rivet-gun rope-walk scaffold
  slab-city spark-plug spinet turnstile. The other 14 outside instances (11 patches) are in lessons; 68 are
  inside Analog Voice or Sequenced Part + Sub.
- **Amp Env:** acid-line balcony checkerboard corduroy dead-calm depth-charge echo×2 estuary
  gantry gumdrop laundromat lichen×3 loading-dock×3 matinee pawn-shop progression rivet-gun
  roller-rink rope-walk scaffold slab-city spark-plug spillway spinet split×2 turnstile. The
  other 27 outside instances are in lessons; 129 are inside the six.

The "outside" reach in §2 also counts lesson patches, which stay flat. By generator file
(patches): Stereo Sum `explain.rs` 1, `sound_palette.rs` 1. Dual Osc `sound_palette.rs` 3,
`library.rs` 3, `expressive.rs` 2, `explain.rs` 1, `performance.rs` 1, `composition.rs` 1. Amp
Env `library.rs` 7, `cable_capture.rs` 2, `expressive.rs` 2, `sound_palette.rs` 2, `explain.rs`
1, `interlocking.rs` 1, `chrome_capture.rs` 1, `performance.rs` 1, `composition.rs` 1,
`sound_engines.rs` 1. Tone Stage has none.

Example checks against the graphs: balcony #73 split → #74/#75 mixers → #76 join (Stereo Sum);
boxwood #4 drive → #5 ladder → #6 gain (Tone Stage); corduroy #3/#4 `osc.va` → #5 mixer (Dual
Osc) and #6 env → #8 vca (Amp Env).

## 4. Not composites

| block | reach / instances | why not |
|---|---|---|
| Filter Sweep, `env.adsr →@cutoff_hz filter.ladder` | 81 / 86 (64 inside Analog Voice, 15 / 20 outside) | The ladder already belongs to Tone Stage or Analog Voice. The envelope stays next to it and lands on the Cutoff mod port. A separate block would split one filter across two cards |
| Keys Voice, Post-VCA Voice, FM Voice | 23 / 33, 18 / 18, 16 / 16 | Assemblies of the recommended four (§2 rows 5–7). FM Voice is one module plus Tone Stage |
| Insert chorus, any `→ chorus` before Space | 45 / 45 | One module plus a cable |
| Compressor before Space, `compressor → split` | 61 patches | One module plus a cable. If wanted, it is a Space option, not a new block |
| Single LFO on `osc@fine` (vibrato) | 17 / 42 | One module (first pass, unchanged). It lands on Dual Osc's Fine mod port |
| `clock.div` ×2 cascade | 7 / 21, all `presets_piece.rs` | First pass, unchanged: it exists because `div` caps at 8 |
| Learning voice, `osc → svf → vca(env)` | 12 / 15, all lessons except 2 in `presets_pad.rs` | The lessons stay flat |
| Velocity amp, `env → ringmod.a → vca.cv` | 8 / 16, all lessons | The lessons stay flat |
| No-drive voice, `mixer → ladder → vca(env) → gain` | 4 / 5, all lessons | Lessons; elsewhere it is Dual Osc + Amp Env with the ladder flat |
| Gain stack, `gain → gain [→ gain]` | 10 / 24, outside the six only in `presets_voice.rs` (7) | Bespoke trims (first pass: "several trims") |
| Noise layer, `noise → vca` | 6 / 6 | One source plus a level; trivial |
| Hubs (`macro`, `cues`, `clock`, `out`), `midi.in` | — | They stay top level, as in the first pass. `midi.in` feeds the voice ports |

From the first pass's list, the formant mouth and the acid glide did not come up again (each is
in 3 patches or fewer). The data gives no reason to revisit any of them.

## 5. Coverage

Share of the 3,170 modules in the 129 browser patches that sit inside a composite. 432 of them
are hubs or `midi.in`, which are always top level, so the second figure uses the other 2,738.

| state | modules inside a composite | of all | of non-hub |
|---|---|---|---|
| today (no browser patch uses a composite) | 0 | 0 % | 0 % |
| migrated to the six (strict matches, §1) | 1,509 | 47.6 % | 55.1 % |
| six + the recommended four, factory generators only | 1,804 | 56.9 % | 65.9 % |
| the same, if the lessons were migrated too (not recommended) | 1,908 | 60.2 % | 69.7 % |

The four add 295 modules in the factory patches: Tone Stage 99, Stereo Sum 96, Amp Env 64,
Dual Osc 36. The six are matched first and the four only claim modules the six left. Loosening
the six's matchers (Glue Bus with unity mixers, Analog Voice without drive) would raise the
middle row, but that needs those definitions to change.
