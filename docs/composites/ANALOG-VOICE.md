# Analog Voice (`kabl.analog-voice` v1)

Catalogue section C1 in `docs/pivot/composite-catalogue.md`, roadmap row CB5. The subtractive
keyboard voice behind most Bass, Lead, Pad and Voice presets: two oscillators into a mixer into
a drive into a ladder filter that its own envelope sweeps, into a VCA that the amp envelope
opens, into a level trim. 105 browser patches build it by hand; 52 of them with exactly this
wiring.

**Status: prototype.** The interface below is the catalogue's proposal with the deviations
listed further down, built so it can be judged on captures and renders. A definition is
immutable once it ships (D09), so names, defaults and the face may still change before this
version is released.

## One definition

The catalogue's core comes with one to four oscillators (14 patches with one, 25 with two, 12
with three, one with four), some with a vibrato LFO. This card has **two**:

- two is the largest family, and Cinder Block, the regression case, is one of them;
- a one-oscillator patch is this card with Mix 2 at 0, so the second oscillator costs nothing
  there and the same definition covers it;
- a third oscillator would be a dead control in the 39 patches with one or two; the 13 with
  three or four need a variant or stay flat, a question for CM1.

**No vibrato LFO.** 16 of the 52 add an `lfo → osc fine` route. An LFO inside the card would sit
unused in the other 36, and the catalogue gives it no port. A vibrato is a modulation source
patched from outside: today into Detune mod (the only oscillator modulation input the catalogue
lists), or, if Kosta wants it, into a Vibrato port on both oscillators' `fine` in a later
version.

## Inside

As the bass and lead builders (`Recipe` in `tools/examples/presets_bass_lead.rs`) store them,
with the catalogue's modal values:

- `osc.va` ×2: 261.63 Hz, saw; the first with unison 1 and detune 15 ct. Pitch feeds both.
- `mixer`: level 1 for oscillator 1, 0.3 for oscillator 2.
- `drive`: 0 dB.
- `filter.ladder`: cutoff 3500 Hz, resonance 0.1.
- `env.adsr` (filter): 2 ms, 300 ms, 0.3, 100 ms, routed into the cutoff at depth 0.2 (the
  corpus's most common non-zero filter envelope amount). This route is Env Amount.
- `env.adsr` (amp): 2 ms, 300 ms, 0.6, 90 ms, into the VCA's `cv`. Gate feeds both envelopes.
- `vca`: gain 0, so the amp envelope alone opens it.
- `gain` (Level): −6 dB. Its `out` is Out.

## Interface

Ports (interface keys 1–8):

| port | kind | inside |
|---|---|---|
| Pitch | pitch in | both `osc.va.pitch` (fan-in, G3) |
| Gate | gate in | both `env.adsr.gate` (fan-in, G3) |
| Out | audio out | `gain.out` |
| Env Amount | modulation in | the filter envelope's route into the cutoff: its `amount`, a cable parameter (G1) |
| Cutoff mod | modulation in | `filter.ladder.cutoff_hz` |
| Level mod | modulation in | `gain.gain_db` |
| Drive mod | modulation in | `drive.drive_db` |
| Detune mod | modulation in | oscillator 1's `detune` (off the face) |

Controls (interface keys 9–28):

| control | default | range | unit | inside |
|---|---|---|---|---|
| Wave 1 | saw (2) | sine, triangle, saw, pulse | | oscillator 1 `waveform` |
| Octave 1 | 261.63 | 20–20000 (exponential) | Hz | oscillator 1 `base_hz` |
| Unison | 1 | 1–16 | voices | oscillator 1 `unison` (off the face) |
| Detune | 15 | 0–100 | ct | oscillator 1 `detune` (off the face) |
| Wave 2 | saw (2) | sine, triangle, saw, pulse | | oscillator 2 `waveform` |
| Octave 2 | 261.63 | 20–20000 (exponential) | Hz | oscillator 2 `base_hz` |
| Mix 2 | 0.3 | 0–1 | | `mixer.level2` |
| Drive | 0 | 0–36 | dB | `drive.drive_db` |
| Cutoff | 3500 | 20–20000 (exponential) | Hz | `filter.ladder.cutoff_hz` |
| Resonance | 0.1 | 0–1 | | `filter.ladder.resonance` |
| Filter Attack, Decay, Sustain, Release | 2, 300, 0.3, 100 | 0.1–10000 ms, 0–1 | ms | filter `env.adsr` |
| Amp Attack, Decay, Sustain, Release | 2, 300, 0.6, 90 | 0.1–10000 ms, 0–1 | ms | amp `env.adsr` |
| Level | −6 | −60–24 | dB | `gain.gain_db` |
| Env Amt | 0.2 | −1–1 | | the Env Amount route's `amount`, the cable parameter Env Amount also exposes as a port |

Every control uses its parameter's own range and curve, so no control needs a mapping (G2).

## Face

840 × 340 points, the widest a face may be, with two rows in signal order:

- **Top row**: Wave 1 over Wave 2 as waveform glyphs, then Octave 1, Octave 2, Mix 2 | Drive |
  Cutoff, Resonance | Filter Attack to Filter Release.
- **Bottom row**: seven jacks in two rows at the left; Level under Cutoff; Env Amt under
  Resonance, next to the envelope it scales; Amp Attack to Amp Release right under the filter
  envelope, so the two envelopes read as a pair.
- **Jacks**: Env Amount, Cutoff mod, Drive mod over Pitch, Gate, Level mod, Out.
  The jacks a keyboard and the output patch to sit on the face's bottom edge, so in the rack
  capture (a `midi.in` left of the card, the output under the `midi.in`) their cables leave
  under the face and cross no control or label.

Knob and jack widths fit their longest labels ("Filter Release", "IN Env Amount") at the face's
12-point type, so every label prints whole. Knobs, not sliders: a face slider follows the
player's orientation setting.

Off the face, in the design page's list, More controls and Inspect internals: **Unison**, whose
sixteen steps need a 288-point selector (18 points a choice) where the face has 76 points free,
and **Detune** and **Detune mod**, which only spread unison voices and so do nothing at the
default Unison of 1. Detune mod is still patchable: like any unplaced port, it takes cables at
the face's footer. The face's fixed height leaves no room for displays. No artwork.

## Differences from the catalogue proposal

Questions for Kosta are marked **Q**.

- **Env Amount is a port and a knob.** The knob, Env Amt, sets the route's stored depth (0.2);
  a macro or any source patched to the Env Amount port moves it from there, as the 40
  G1-blocked patches do with their macros. Labels are unique across ports and controls, so the
  knob is "Env Amt". **Q:** keep the names, or rename the port (for example "Env Amount mod",
  as Cutoff mod) so the knob can be "Env Amount"?
- **No Velocity port.** The subtractive core uses no velocity (the catalogue lists it for FM
  voices); a port with no target inside would be dead. **Q:** add it if a later variant
  scales the amp envelope by velocity.
- **Wheel → Cutoff is Cutoff mod.** Both would target `filter.ladder.cutoff_hz`, and a route
  from outside carries its own depth, so the wheel patches to Cutoff mod. Modulation inputs are
  named "<control> mod", like the other factory cards, because a port and a control may not
  share a label.
- **Ranges are the parameters' own**: Unison 1–16 (catalogue 1–8), Detune 0–100 ct (0–45),
  Drive 0–36 dB (0–30), Octave in Hz (the catalogue's −3…+2 octave view needs a mapping, G2).
- **Octave 1 and 2 are Hz knobs** on `base_hz` that sweep continuously, not octave steps.
  **Q:** Octave 1/2 knobs show Hz and sweep continuously; relabel or wait for a G2 octave view?
- **Two oscillators and no vibrato LFO** (see One definition). **Q:** a Vibrato port on both
  oscillators' `fine`?
- **No amp envelope inputs.** Cinder Block's Hold macro routes into the amp envelope's decay,
  sustain and release; the catalogue gives the card no port for them, so the regression leaves
  those three routes out of both renders. **Q (CM1):** amp envelope modulation ports, or keep
  such patches' extra routes on Inspect internals?

## Regression: Cinder Block

`cinder_block` in the generator builds Cinder Block's voice two ways from
`patches/presets/cinder-block`: the preset flat, with its effects after the trim removed and
Hold's amp-envelope routes left out; and this card with every leaf set to the preset's values
and the same `midi.in`, macros and routes patched to its ports. Brightness's route into the
envelope's depth lands on Env Amount.

They render equal to within 4.2e-7 (a 1e-6 tolerance, about −116 dB under the 0.26 peak),
not sample-identical, and the reason is known: the cutoff sums three routes (Wheel,
Brightness, the envelope's) in cable order, and in the card the envelope's route is the card's
own cable, made before the outside ones, so it is added first; float addition does not
associate. With the preset's two outside routes made again after its envelope route (the
card's order), the flat voice renders sample-identical to the card. The test checks both.

## Regenerating

`cargo run -p kabl-tools --example analog_voice_composite -- write composites` writes
`composites/analog-voice/v1.json`; two runs write the same bytes. `wav DIR` renders eight
seconds of a two-bar line in eighths at 125 BPM through Cinder Block's flat voice
(`cinder-block-voice.wav`) and through the card set to its values
(`analog-voice-cinder-block.wav`), and the card's defaults at C4 gated by a clock at 120 BPM
(`analog-voice.wav`). The captures are the `chrome_capture` scenes `analog-voice-rack`,
`analog-voice-face` and `analog-voice-design`, built in `tools/examples/composite_gen/scene.rs`,
and `analog-voice-design-env`: the design page with Env Amt off the face, its palette scrolled
to the Env Amt entry it offers.

## Tests

`crates/ui/tests/composites_analog_voice.rs`:

- `the_committed_analog_voice_is_what_the_generator_writes`: the generator's bytes equal the
  file.
- `analog_voice_has_the_catalogue_interface`: ports with their keys and every target, controls
  with their parameter, range and default, as listed above; which leaf each reaches; the
  chain's wiring; Env Amount's cable and its 0.2, which Env Amt controls; Unison, Detune and
  Detune mod alone off the face.
- `analog_voice_plays_notes_in_tune_and_is_silent_ungated`: notes through a `midi.in` sound;
  with no note the RMS is below 1e-6; note 69 on a sine with the filter open measures 440 Hz
  within 1 % (440.01 measured).
- `with_cinder_blocks_values_it_renders_cinder_blocks_voice`: the regression above.
- `env_amount_moves_the_filter_envelope_route`: setting the depth to −0.5, 0 and 0.5 changes the
  cable's `amount` and the sound darkens then brightens; −1 renders finite and audible; a macro
  patched to Env Amount brightens it.
- `the_env_amt_knob_sets_the_route_one_step_per_drag`: on the closed face, two upward drags on
  Env Amt each raise the route's amount as one undo step, and the voice renders brighter.
- `analog_voice_plays_inside_a_parent_composite`: the card with its `midi.in` and output
  outside it, all in a parent composite, renders the same samples as before grouping and as
  the bare leaf graph.

`composites_factory.rs` `every_shipped_factory_definition_inserts` inserts it from the bank.
