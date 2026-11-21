#!/usr/bin/env python3
"""Finishes the promo: finds where bar 0 of the piece is in the recording, checks that against
the audio itself, burns captions (section names, what each cable gesture does) and writes the
final mp4.

    post.py TIMELINE RENDER.wav RECORDING.mp4 DRIVE.log FFMPEG_START_EPOCH OUT.mp4 WORKDIR

Bar 0 is the moment of the clock-Restart click (the line `click <restart target>` in the drive
log, plus a constant, `restart_latency_s`). That estimate is then corrected by the lag at which
the recorded audio's loudness envelope best matches the offline render's (searched within
+-1.5 s); the correlation is printed, and below 0.5 the script stops instead of guessing.

Environment: KABL_ANCHOR is the bar-0 click key (default targets.json "anchor", else "restart");
CRF is the final quality (default 18).

Joining takes (each already finished by this script, so t=0 is 0.5 s before bar 0 in every one):

    post.py --join OUT.mp4 CUT_SECONDS[,CUT_SECONDS...] TAKE1.mp4 TAKE2.mp4 ...

Take i plays up to cut i, take i+1 from there on, with a 20 ms audio crossfade at the cut.
Cut seconds = 0.5 + bar * bar_seconds.
"""
import sys

if sys.argv[1] == "--join":
    import os
    import subprocess

    out, cuts, takes = sys.argv[2], [float(c) for c in sys.argv[3].split(",")], sys.argv[4:]
    if len(cuts) != len(takes) - 1:
        sys.exit("need one cut per join: len(cuts) == len(takes) - 1")
    X = 0.02
    cmd, v, a = ["ffmpeg", "-loglevel", "error", "-y"], [], []
    for i, tk in enumerate(takes):
        cmd += ["-i", tk]
        lo = cuts[i - 1] if i else 0
        hi = cuts[i] if i < len(cuts) else None
        vt = f"start={lo}" + (f":end={hi}" if hi else "")
        at = f"start={lo}" + (f":end={hi + X}" if hi else "")  # audio overlaps the next take by X
        v.append(f"[{i}:v]trim={vt},setpts=PTS-STARTPTS[v{i}]")
        a.append(f"[{i}:a]atrim={at},asetpts=PTS-STARTPTS[a{i}]")
    n = len(takes)
    chain, last = [], "a0"
    for i in range(1, n):
        chain.append(f"[{last}][a{i}]acrossfade=d={X}[x{i}]")
        last = f"x{i}"
    fc = ";".join(v + a + chain + ["".join(f"[v{i}]" for i in range(n)) + f"concat=n={n}:v=1:a=0[vo]"])
    cmd += ["-filter_complex", fc, "-map", "[vo]", "-map", f"[{last}]", "-c:v", "libx264", "-crf",
            os.environ.get("CRF", "18"), "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "160k",
            "-movflags", "+faststart", out]
    subprocess.run(cmd, check=True)
    print(f"wrote {out}")
    sys.exit(0)

import json
import os
import subprocess
import sys
import wave

import numpy as np

timeline_path, render_path, rec_path, log_path, ff_start, out_path, work = sys.argv[1:8]
tl = json.load(open(timeline_path))
# targets.json sits next to scripts/ (two path components up from this file).
targets = json.load(open(sys.argv[0].rsplit("/", 2)[0] + "/targets.json"))
bar = tl["bar_seconds"]

# Bar 0 in video time, from the drive log.
anchor = os.environ.get("KABL_ANCHOR") or targets.get("anchor", targets["restart"])
click = next(float(l.split()[0]) for l in open(log_path) if l.split()[1:] == ["click", anchor])
t0 = click + targets["timing"]["restart_latency_s"] - float(ff_start)
print(f"bar 0 at video time {t0:.2f} s (drive log)")


def rms_track(samples, sr, win=0.1):
    n = int(sr * win)
    return np.sqrt(np.mean(samples[: len(samples) // n * n].reshape(-1, n) ** 2, axis=1))


def mono(path):
    raw = subprocess.run(["ffmpeg", "-loglevel", "error", "-i", path, "-vn", "-ac", "1", "-ar", "48000", "-f", "f32le", "-"],
                         capture_output=True, check=True).stdout
    return np.frombuffer(raw, dtype=np.float32)


rec = rms_track(mono(rec_path), 48000)
with wave.open(render_path) as w:
    ref = np.frombuffer(w.readframes(w.getnframes()), dtype=np.int16).astype(np.float32) / 32768
ref = rms_track(ref.reshape(-1, 2).mean(axis=1), 48000)
# Check against the audio: the loudness envelope (0.1 s windows) of the whole recorded piece
# against the render's. The lag with the best correlation is where bar 0 really is; the captions
# use it when it is within 1.5 s of the drive-log estimate.
n = len(ref)
lags = np.arange(-15, 16)
corr = []
for lag in lags:
    i = int(round((t0 + lag * 0.1) / 0.1))
    seg = rec[i : i + n]
    corr.append(np.corrcoef(seg, ref[: len(seg)])[0, 1] if len(seg) > 0.95 * n and i >= 0 else -1)
best = lags[int(np.argmax(corr))] * 0.1
print(f"envelope correlation {max(corr):.3f} at {best:+.1f} s from the drive-log estimate "
      f"(at the estimate: {corr[list(lags).index(0)]:.3f})")
if max(corr) < 0.5:
    sys.exit("the recorded audio does not match the render (recording too short or silent?)")
t0 += best
# The video starts half a second before bar 0 (the app start-up before it is cut).
trim = max(t0 - 0.5, 0.0)
t0 -= trim


# Captions.
def ts(s):
    s = max(s, 0)
    return f"{int(s // 3600)}:{int(s // 60) % 60:02d}:{s % 60:05.2f}"

ass = ["[Script Info]", "ScriptType: v4.00+", "PlayResX: 1920", "PlayResY: 1080", "",
       "[V4+ Styles]",
       "Format: Name,Fontname,Fontsize,PrimaryColour,SecondaryColour,OutlineColour,BackColour,Bold,Italic,Underline,StrikeOut,ScaleX,ScaleY,Spacing,Angle,BorderStyle,Outline,Shadow,Alignment,MarginL,MarginR,MarginV,Encoding",
       "Style: Title,Lato,72,&H00FFFFFF,&H00FFFFFF,&H00202020,&H80000000,1,0,0,0,100,100,0,0,1,4,2,8,40,40,110,1",
       "Style: Cap,Lato,40,&H00FFFFFF,&H00FFFFFF,&H00202020,&HB0000000,0,0,0,0,100,100,0,0,3,10,0,1,30,30,40,1",
       "", "[Events]", "Format: Layer,Start,End,Style,Name,MarginL,MarginR,MarginV,Effect,Text"]
for s in tl["sections"]:
    a = t0 + s["bar"] * bar
    ass.append(f"Dialogue: 0,{ts(a)},{ts(a + 3.5)},Title,,0,0,0,,{s['name']}")
for g in tl["gestures"]:
    a, b = t0 + (g["start_bar"] - 0.5) * bar, t0 + (g["end_bar"] + 1.5) * bar
    ass.append(f"Dialogue: 0,{ts(a)},{ts(b)},Cap,,0,0,0,,{g['what']}")
ass_path = f"{work}/captions.ass"
open(ass_path, "w").write("\n".join(ass) + "\n")
subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-ss", f"{trim:.2f}", "-i", rec_path, "-vf", f"ass={ass_path}",
                "-c:v", "libx264", "-preset", "medium", "-crf", os.environ.get("CRF", "18"), "-pix_fmt", "yuv420p",
                "-c:a", "aac", "-b:a", "160k", "-movflags", "+faststart", out_path], check=True)
print(f"wrote {out_path}")
