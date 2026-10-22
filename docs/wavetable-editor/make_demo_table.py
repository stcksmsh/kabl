#!/usr/bin/env python3
"""Writes the demo wavetables used for the wavetable editor clips: computed here, no third-party
material. Standard library only.

    python3 docs/wavetable-editor/make_demo_table.py OUTDIR

- sweep-16.wav: 16 frames of 2048 samples (the Serum/Vital layout, no clm chunk). A band of
  harmonics (a formant) sweeps from the 2nd to the 26th harmonic over a 1/h spectrum, so playing
  through the frames sounds like a filter opening.
- cycle-1000.wav: one cycle of 1000 samples (not a power of two, not a multiple of 2048): a
  soft triangle with a third and a fifth harmonic, to show the single-cycle path.
"""
import math
import struct
import sys
import wave

FRAME = 2048


def write(path, samples, rate=44100):
    peak = max(abs(s) for s in samples) or 1.0
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(b"".join(struct.pack("<h", int(s / peak * 0.9 * 32767)) for s in samples))


def sweep(frames=16):
    out = []
    for k in range(frames):
        centre = 2 + 24 * k / (frames - 1)
        for i in range(FRAME):
            x = 2 * math.pi * i / FRAME
            out.append(
                sum(
                    math.sin(h * x) / h * math.exp(-(((h - centre) / 2.5) ** 2))
                    for h in range(1, 64)
                )
            )
    return out


def cycle(n=1000):
    return [
        math.sin(2 * math.pi * i / n)
        + 0.3 * math.sin(6 * math.pi * i / n)
        + 0.15 * math.sin(10 * math.pi * i / n)
        for i in range(n)
    ]


if __name__ == "__main__":
    out = sys.argv[1]
    write(f"{out}/sweep-16.wav", sweep())
    write(f"{out}/cycle-1000.wav", cycle())
