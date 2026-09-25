#!/usr/bin/env python3
"""Looks for dropouts in a walkthrough recording: 5 ms windows at least 40 dB below both the
20 ms before and after them while those are above -50 dBFS (sound stopping mid-note), and
single-sample jumps larger than 0.5 (clicks). Prints video times.

    python3 docs/signal-inspection/scripts/gaps.py target/signal-inspection-walkthrough
"""
import math, os, struct, subprocess, sys, wave

out = sys.argv[1]
wav = os.path.join(out, "stereo48k.wav")
subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", os.path.join(out, "walkthrough.mp4"),
                "-ac", "1", "-ar", "48000", "-c:a", "pcm_s16le", wav], check=True)
w = wave.open(wav)
sr, n = w.getframerate(), w.getnframes()
d = [v / 32768 for v in struct.unpack("<%dh" % n, w.readframes(n))]
win = sr // 200


def rms(a, b):
    a, b = max(a, 0), min(b, n)
    if b <= a:
        return 0.0
    return math.sqrt(sum(x * x for x in d[a:b]) / (b - a))


def db(v):
    return 20 * math.log10(v) if v > 0 else -200.0


gaps = []
i = 4 * win
while i < n - 5 * win:
    mid = db(rms(i, i + win))
    before, after = db(rms(i - 4 * win, i)), db(rms(i + win, i + 5 * win))
    if before > -50 and after > -50 and mid < min(before, after) - 40:
        gaps.append((i / sr, before, mid, after))
        i += 5 * win
    else:
        i += win // 2
clicks = [k / sr for k in range(1, n) if abs(d[k] - d[k - 1]) > 0.5]
print(f"{n / sr:.1f} s analysed at {sr} Hz")
print(f"dropouts: {len(gaps)}")
for t, b, m, a in gaps:
    print(f"  {t:7.3f} s  before {b:6.1f} dB  gap {m:6.1f} dB  after {a:6.1f} dB")
merged = []
for t in clicks:
    if not merged or t - merged[-1] > 0.05:
        merged.append(t)
print(f"sample jumps > 0.5: {len(clicks)} in {len(merged)} places")
for t in merged[:50]:
    print(f"  {t:7.3f} s")
