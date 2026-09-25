#!/usr/bin/env python3
"""Audio level of the walkthrough per scripted step, from the recorded video.

    python3 docs/find-play-save/scripts/audio_timeline.py OUTDIR
OUTDIR is record-walkthrough.sh's output (walkthrough.mp4, drive.log, t0). Prints, for
each drive.log line, its video time and the peak level (dBFS) of the 1.5 s that follow;
then 0.25 s RMS levels around the controller + preview overlap (part 9).
"""
import math
import os
import struct
import subprocess
import sys
import wave

out = sys.argv[1]
wav = os.path.join(out, "mono8k.wav")
subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", os.path.join(out, "walkthrough.mp4"),
                "-ac", "1", "-ar", "8000", wav], check=True)
w = wave.open(wav)
sr, n = w.getframerate(), w.getnframes()
d = struct.unpack("<%dh" % n, w.readframes(n))
t0 = float(open(os.path.join(out, "t0")).read())


def db(v):
    return 20 * math.log10(v) if v > 0 else float("-inf")


def peak(a, b):
    seg = d[max(0, int(a * sr)):int(b * sr)]
    return db(max((abs(v) for v in seg), default=0) / 32768)


def rms(a, b):
    seg = d[int(a * sr):int(b * sr)]
    return db(math.sqrt(sum(v * v for v in seg) / max(1, len(seg))) / 32768)


lines = [(float(l.split(" ", 1)[0]) - t0, l.split(" ", 1)[1].strip())
         for l in open(os.path.join(out, "drive.log"))]
print("video_s  peak_dBFS(next 1.5 s)  step")
for t, cmd in lines:
    print(f"{t:7.1f}  {peak(t, t + 1.5):8.1f}  {cmd}")
on = next(t for t, c in lines if c.startswith("midi on"))
print("\n0.25 s RMS from 1 s before the controller's C4 (part 9):")
for i in range(40):
    a = on - 1 + i * 0.25
    mark = ""
    for t, c in lines:
        if a <= t < a + 0.25 and (c.startswith("midi") or c == "click play"):
            mark = "  <- " + c
    print(f"{a:7.2f}  {rms(a, a + 0.25):7.1f} dB{mark}")
