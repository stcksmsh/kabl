# Space (`kabl.space` v1)

Catalogue section C2 in `docs/pivot/composite-catalogue.md`, roadmap row CB1. An echo into a
room: the ambience tail that 97 of the 129 browser patches end in.

**Status: prototype.** The interface below is the catalogue's proposal, built so it can be
judged on captures and renders. A definition is immutable once it ships (D09), so names,
defaults and the face may still change before this version is released.

## Source

The inner graph and every default come from the Space tail of the factory sound **Soft Sine**
(`patches/presets/soft-sine`, modules 14 and 15). Soft Sine is in the catalogue's most common
variant (compressor before the split, mono into the echo, no macro). With no macro, its echo
and room mixes are stored settings rather than zero-until-a-macro-opens-them, and its room
constants are the corpus majority (damp 3500 Hz, width 60 %, predelay 0 ms).

Inside: `split.left → delay.in`, `delay.out → reverb.in`. The output is `reverb.out`.

**Mono in, stereo out**, as in every corpus Space: `In` takes a stereo cable, only its left
side feeds the echo, and the room spreads the result to stereo at Width.

## Interface

Ports (interface keys 1–7):

| port | kind | inside |
|---|---|---|
| In | audio in, stereo cable | `split.in` (left side used) |
| Clock | gate in, optional | `delay.clock` (used when Sync is not Free) |
| Out | audio out, stereo | `reverb.out` |
| Echo mod | modulation in | route into `delay.mix` |
| Room mod | modulation in | route into `reverb.mix` |
| Decay mod | modulation in | route into `reverb.decay_s` |
| Feedback mod | modulation in | route into `delay.feedback` |

Controls (interface keys 8–17):

| control | default | range | unit | inside, what it changes |
|---|---|---|---|---|
| Echo | 12 | 0–100 | % | `delay.mix`: how much echo is heard |
| Time | 310 | 20–4000 (exponential) | ms | `delay.time_ms`: echo time when Sync is Free (60 is a slapback) |
| Sync | Free | Free, 1/16, 1/8, 1/8d, 1/4 | selector | `delay.sync`: echo time from the Clock port |
| Feedback | 40 | 0–95 | % | `delay.feedback`: how many repeats |
| Tone | 3500 | 500–16000 (exponential) | Hz | `delay.tone_hz`: darkens the repeats |
| Room | 10 | 0–100 | % | `reverb.mix`: how much room is heard |
| Decay | 1.4 | 0.3–30 (exponential) | s | `reverb.decay_s`: room length |
| Damp | 3500 | 500–16000 (exponential) | Hz | `reverb.damp_hz`: darkens the room tail |
| Width | 60 | 0–100 | % | `reverb.width`: room stereo width |
| Predelay | 0 | 0–250 | ms | `reverb.predelay_ms`: gap before the room |

Every control uses its parameter's own range and curve, so no control needs a mapping (G2).

Face: 720 × 340 points. The echo controls in the top row, the room controls below, the seven
jacks in two columns on the right. No artwork.

## Differences from the catalogue proposal

- **Defaults are Soft Sine's**, not the catalogue's: Echo 12 (catalogue 0), Room 10 (0),
  Time 310 ms (375), Feedback 40 % (30), Decay 1.4 s (0.8). The catalogue's 0 mixes come from
  patches where a macro opens the mixes; Space at 0/0 would pass the input through dry.
- **Sync is stored as Free.** Soft Sine stores no `sync`, so its delay has the module default
  1/8; without a clock cable that plays the free time, as Free does. The render through Space
  is bit-identical to Soft Sine's (test below).
- **Modulation ports are labelled "Echo mod", "Room mod", "Decay mod", "Feedback mod".** A
  port and a control cannot share a label, and the catalogue gives both the same names.

## Left out

- **Space Return (C6).** The catalogue says it should nest Space and add a "Dry In" port.
  `encapsulate_with` wraps leaf modules into a top-level composite only, so a definition that
  contains Space cannot be generated with it yet.
- **One "Space" knob on the face** that opens Echo and Room together: a control with two
  targets is gap G2 (CI5). Patches keep their macro routes outside, landing on Echo mod and
  Room mod.
- Nothing in the C2 interface needs CI3 (cable amounts) or CI4 (fan-in).
- The delay's ping-pong `mode` is not exposed (the catalogue does not list it). It stays at
  mono, Soft Sine's setting. Glass Needle (a C2 patch) and Glass Elevator use ping-pong, so a
  migration of those two would need it as an extra control.

## Regenerating

`cargo run -p kabl-tools --example space_composite -- write composites` writes
`composites/space/v1.json`. Two runs write the same bytes: the package holds the patch state
only, with fixed module positions, not the editor log's wall-clock timestamps. Once released,
the file is not regenerated: a change is a new version.

The same example renders `wav DIR` (a phrase on the default voice, dry and through Space) and
`capture DIR` (the real UI, under `xvfb-run`).

## Tests

`crates/ui/tests/composites_space.rs`:

- `space_loads_from_the_bank_with_its_interface`: the bank lists `kabl.space` v1 with the
  ports and controls above, around `split`, `delay`, `reverb`.
- `space_alone_renders_a_wet_tail`: `audio.in → Space → out`, a 50 ms tone, then a tail after
  the tone stops.
- `a_space_control_changes_the_render_inside_a_voice`: the default voice through Space; Room
  from 10 to 60 % changes the render.
- `space_reproduces_soft_sine_bit_for_bit`: Soft Sine flat, and Soft Sine with its split,
  delay and reverb replaced by an inserted Space, render the same samples.

`composites_factory.rs` `every_shipped_factory_definition_inserts` inserts it from the bank.
