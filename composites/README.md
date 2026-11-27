# Factory composites

Read-only factory composite definitions, one file per version: `<name>/v<N>.json` is
`kabl.<name>` version N. See `docs/composites/FACTORY-BANK.md`.

| definition | what it is | doc |
|---|---|---|
| `kabl.space` v1 | an echo into a room | `docs/composites/SPACE.md` |
| `kabl.drum-kit` v1 | a drum grid playing five voices into a submix | `docs/composites/DRUM-KIT.md` |
| `kabl.sequenced-part` v1 | a step sequencer playing a voice, level and tone filter | `docs/composites/SEQUENCED-PART.md` |
| `kabl.sequenced-part-sub` v1 | the same with a sub-oscillator under the voice | `docs/composites/SEQUENCED-PART.md` |
| `kabl.glue-bus` v1 | drive, tone filter and trim into a compressor, then a second compressor the kick ducks | `docs/composites/GLUE-BUS.md` |

All are prototypes until approved.
