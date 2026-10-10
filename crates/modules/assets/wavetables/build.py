#!/usr/bin/env python3
"""Regenerates the factory wavetables (*.wav) next to this file.

    python3 crates/modules/assets/wavetables/build.py CACHE_DIR

Procedural tables are computed here (original work, MIT OR Apache-2.0). Sampled tables are
assembled from Adventure Kid Waveforms (CC0 1.0) at the pinned commit below; the downloads are
cached in CACHE_DIR. Every table is 16-bit mono, 2048-sample frames, with the `clm ` chunk
that names the frame length, so the file is also a valid table for Serum-style tools.
"""
import os
import struct
import sys
import urllib.request

import numpy as np

N = 2048
HERE = os.path.dirname(os.path.abspath(__file__))
AKWF_COMMIT = "8de90bf94376670947369e69de0af6b9fbd19286"
AKWF_URL = (
    "https://raw.githubusercontent.com/KristofferKarlAxelEkstrand/AKWF-FREE/"
    + AKWF_COMMIT
    + "/AKWF/{d}/{d}_{i:04d}.wav"
)
T = np.arange(N) / N


def additive(amps):
    """One cycle from harmonic amplitudes amps[h-1], sine phases."""
    h = np.arange(1, len(amps) + 1)[:, None]
    return (np.asarray(amps)[:, None] * np.sin(2 * np.pi * h * T[None, :])).sum(0)


def saw_square():
    out = []
    for i in range(16):
        t = i / 15
        amps = [(1.0 if h % 2 else 1.0 - t) / h for h in range(1, 513)]
        out.append(additive(amps))
    return out


def harmonic_sweep():
    return [additive([1.0 / h for h in range(1, k + 1)]) for k in range(1, 33)]


def vowels():
    # Formants (Hz) of a male voice (Peterson & Barney averages), a fixed 110 Hz source.
    forms = {
        "a": (730, 1090, 2440),
        "e": (530, 1840, 2480),
        "i": (270, 2290, 3010),
        "o": (570, 840, 2410),
        "u": (300, 870, 2240),
    }
    seq = ["a", "e", "i", "o", "u"]
    f0 = 110.0
    out = []
    for i in range(16):
        x = i / 15 * (len(seq) - 1)
        a, b = seq[int(x)], seq[min(int(x) + 1, len(seq) - 1)]
        t = x - int(x)
        fa, fb = np.array(forms[a]), np.array(forms[b])
        f = fa * (1 - t) + fb * t
        amps = []
        for h in range(1, 190):
            hz = h * f0
            env = sum(g * np.exp(-0.5 * ((hz - c) / (0.09 * c + 40)) ** 2)
                      for g, c in zip((1.0, 0.7, 0.35), f))
            amps.append(env / h ** 0.8)
        out.append(additive(amps))
    return out


def glass_bell():
    # Phase modulation 1:3 with the index swept: the periodic spectrum of a bright FM bell.
    return [np.sin(2 * np.pi * T + idx * np.sin(2 * np.pi * 3 * T)) for idx in np.linspace(0.0, 7.0, 16)]


def digital_hollow():
    out = []
    for p in np.linspace(2.0, 17.0, 16):
        amps = [(1 - np.cos(2 * np.pi * h / p)) / 2 / h ** 0.9 for h in range(1, 513)]
        out.append(additive(amps))
    return out


def akwf(folder, count):
    cache = sys.argv[1]
    os.makedirs(cache, exist_ok=True)
    total = {"AKWF_epiano": 73, "AKWF_eorgan": 154, "AKWF_hvoice": 104}[folder]
    picks = sorted({int(round(v)) for v in np.linspace(1, total, count)})
    frames = []
    for i in picks:
        path = os.path.join(cache, f"{folder}_{i:04d}.wav")
        if not os.path.exists(path):
            with urllib.request.urlopen(AKWF_URL.format(d=folder, i=i)) as r:
                open(path, "wb").write(r.read())
        raw = open(path, "rb").read()
        assert raw[:4] == b"RIFF" and raw[8:12] == b"WAVE"
        data = raw.index(b"data")
        n = struct.unpack("<I", raw[data + 4:data + 8])[0]
        x = np.frombuffer(raw[data + 8:data + 8 + n], dtype="<i2").astype(float) / 32768
        assert len(x) == 600, (folder, i, len(x))
        spec = np.fft.rfft(x)
        spec[0] = 0
        spec[257:] = 0  # 256 harmonics: AKWF cycles carry nothing above
        spec *= np.exp(-1j * np.angle(spec[1]))  # fundamental to zero phase: frames line up
        frames.append(np.fft.irfft(spec, N) * (N / 600))
    return frames, picks


def finish(frames):
    out = []
    for f in frames:
        spec = np.fft.rfft(f)
        spec[0] = 0
        spec[513:] = 0
        out.append(np.fft.irfft(spec, N))
    return [f / np.abs(f).max() * 0.98 for f in out]  # every frame to the same peak


def write(name, frames):
    frames = finish(frames)
    pcm = np.round(np.concatenate(frames) * 32767).astype("<i2").tobytes()
    clm = b"<!>2048 00000000 wavetable (kabl)"
    pad = len(clm) & 1
    body = (
        b"WAVEfmt " + struct.pack("<IHHIIHH", 16, 1, 1, 48000, 96000, 2, 16)
        + b"clm " + struct.pack("<I", len(clm)) + clm + b"\0" * pad
        + b"data" + struct.pack("<I", len(pcm)) + pcm
    )
    open(os.path.join(HERE, name + ".wav"), "wb").write(b"RIFF" + struct.pack("<I", len(body)) + body)
    print(name, len(frames), "frames")


if __name__ == "__main__":
    write("saw-square", saw_square())
    write("harmonic-sweep", harmonic_sweep())
    write("vowels", vowels())
    write("glass-bell", glass_bell())
    write("digital-hollow", digital_hollow())
    for name, folder in (("epiano", "AKWF_epiano"), ("organ", "AKWF_eorgan"), ("choir", "AKWF_hvoice")):
        frames, picks = akwf(folder, 16)
        write(name, frames)
        print("  ", folder, picks)
