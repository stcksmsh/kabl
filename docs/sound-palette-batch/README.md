# Sound Palette + Playable Voices

**Built, waiting for Kosta's hands-on listening and keyboard review.** Supervisor scope
(2026-09-24), authorized as one batch: distinct playable strings, pads, leads, basses,
textures and percussion. Composition + Motion is a separate batch, also still waiting for
its review; neither is approved here. Rationale: `docs/decisions.md`, "Sound palette batch".
The keyboard rules were written first: [keyboard.md](keyboard.md).

    cargo run --release -p kabl-ui -- --patch patches/sound-palette --perform --rate 48000 --frames 256
    cargo run --release -p kabl-ui -- --patch patches/palette/lead --perform --rate 48000 --frames 256
    # patches/palette/{strings,pad,lead,bass,breath,perc}; add --size 1280x800; --midi "KL Essential"

Start with the [hands-on checklist](CHECKLIST.md) and the [patch guide](PATCHES.md).

**Automated vs. owner evidence — read this first.** This batch was finished in a cloud
container, not on Kosta's laptop: a 4-vCPU Xeon VM (~2.1× slower than the i7-13700H on the
same benchmark), no sound card, no ALSA sequencer, no rtkit, software OpenGL. Everything
below was checked by tests, offline renders through the real engine, or the real release
app on a virtual display (Xvfb) with audio to a silent PipeWire sink and MIDI from the
virtual controller through a fifo stand-in (`KABL_MIDI_PIPE`, below). The takes and the
walkthrough are **scripted performances**. Sound on speakers, feel on a keyboard, hardware
MIDI unplugging and the callback figures on the real machine are Kosta's checks.

@@BODY@@
