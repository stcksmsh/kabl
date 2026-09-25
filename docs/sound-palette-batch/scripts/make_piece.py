#!/usr/bin/env python3
"""Writes scripts/piece.txt: the scripted 9½-minute take of patches/sound-palette (an original
piece in E minor at 104 bpm), played on the virtual controller by drive.py and recorded by
kabl's own recorder. A scripted performance, not a person playing.

Sections (times are the script's; the recording starts ~0.5 s after the first line):
  0:00  Atmosphere     breath layer alone, sequences resting (cue Air)
  1:35  Foundation     cue Pulse (kick pulse, hat tick), 1:58 cue Groove (full kit, bass line)
  3:05  Strings / pad  strings up, then the pad under them; Motion and Bright rise
  4:50  Lead           cue Lift; strings out, a little pad as a halo, the mono lead with echo
  6:35  Breakdown      cue Break (bass held notes, hat ticks); breath and pad, big space
  7:50  Return         cue Groove, then Lift; strings and pad; a last lead phrase
  9:10  Close          cue Air, the chords ring out, transport stopped, tails

Controls (channel 1): macros CC 20-23 (Energy, Motion, Bright, Space), layer faders
CC 24-27 (Breath, Strings, Pad, Lead), lead echo CC 28, cue buttons CC 40-44 (Air, Pulse,
Groove, Lift, Break), run/stop CC 46.
"""
import os

BEAT = 60.0 / 104.0
BAR = 4 * BEAT
ev = []  # (time s, line)


def at(t, line):
    ev.append((t, line))


def cc(t, num, v):
    at(t, f"midi cc 1 {num} {int(round(v))}")


def ramp(t, num, a, b, secs):
    at(t, f"midi ramp 1 {num} {int(round(a))} {int(round(b))} {secs:.2f}")


def note(t, n, dur, vel=90):
    at(t, f"midi on {n} {vel}")
    at(t + dur, f"midi off {n}")


def chord(t, notes, dur, vel=85):
    for n in notes:
        note(t, n, dur, vel)


def button(t, num):
    cc(t, num, 127)
    cc(t + 0.15, num, 0)


def line(t, notes, overlap=0.08, vel=100):
    """(note, beats) in legato: each next note starts `overlap` s before the last ends."""
    for n, beats in notes:
        d = beats * BEAT
        if n is not None:
            note(t, n, d + overlap, vel)
        t += d
    return t


def v(x):
    return x * 127


EM = [52, 59, 64, 67]
CMAJ7 = [48, 55, 59, 64]
D6 = [50, 57, 62, 66]
BM7 = [47, 54, 57, 62]
AM9 = [45, 52, 55, 59, 64]
G6 = [43, 50, 55, 59, 64]

# Pick up every mapping at its saved value.
T0 = 0.5
for num, val in [(20, 0.3), (21, 0.4), (22, 0.4), (23, 0.4), (24, 0.8), (25, 0), (26, 0),
                 (27, 0), (28, 0)]:
    cc(T0, num, v(val))
at(T0 + 0.2, "click rec-start")
S = T0 + 1.0

# Atmosphere (0:00-1:35): breath alone, slow dyads and fragments; space opens.
t = S
ramp(t, 23, v(0.4), v(0.65), 30)
ramp(t + 20, 21, v(0.4), v(0.7), 40)
ramp(t + 10, 24, v(0.8), v(1.0), 20)
for notes, dur in [([64, 71], 9), ([67, 74], 8), ([62, 69], 9), ([59, 66, 71], 10),
                   ([64, 71, 76], 9), ([60, 67], 8), ([62, 69, 74], 10), ([59, 64, 71], 9),
                   ([57, 64, 69], 10)]:
    chord(t, notes, dur, 80)
    t += dur + 1.0
line(t - 12, [(76, 3), (79, 3), (78, 6)], overlap=0.3, vel=75)

# Foundation (1:35-3:05).
t = S + 95
button(t, 41)  # Pulse
ramp(t + 2, 20, v(0.3), v(0.45), 10)
button(t + 23, 42)  # Groove
ramp(t + 30, 24, v(1.0), v(0.3), 20)
for k, (notes, dur) in enumerate([([64, 71], 8), ([67, 74], 8), ([62, 69], 8),
                                  ([64, 71], 8), ([59, 66], 8), ([64, 71], 10)]):
    chord(t + 4 + k * 14, notes, dur, 75)
ramp(t + 60, 20, v(0.45), v(0.55), 20)

# Strings / pad (3:05-4:50).
t = S + 185
ramp(t, 24, v(0.3), 0, 6)
ramp(t, 25, 0, v(0.75), 8)
prog = [EM, CMAJ7, D6, BM7, EM, AM9, CMAJ7, D6, G6, D6, AM9, BM7]
ramp(t + 40, 26, 0, v(0.55), 12)
ramp(t + 45, 21, v(0.7), v(0.85), 20)
ramp(t + 50, 22, v(0.4), v(0.6), 20)
for k, c in enumerate(prog):
    chord(t + 1 + k * 2 * BAR * 0.95, c, 2 * BAR * 0.95 - 0.25, 85)

# Lead (4:50-6:35).
t = S + 290
button(t - 1.5, 43)  # Lift
ramp(t, 25, v(0.75), 0, 4)
ramp(t, 26, v(0.55), v(0.22), 4)
ramp(t + 1, 27, 0, v(0.8), 3)
ramp(t + 1, 28, 0, v(0.35), 3)
ramp(t + 2, 21, v(0.85), v(0.5), 6)  # Motion also scales the lead's vibrato
t += 5
p1 = [(76, 2), (74, 1), (71, 1), (74, 2), (76, 2), (79, 3), (78, 1), (76, 4), (None, 2)]
p2 = [(71, 1), (74, 1), (76, 2), (78, 2), (79, 1), (81, 1), (83, 4), (81, 2), (79, 2),
      (None, 2)]
p3 = [(76, 2), (79, 2), (78, 1), (76, 1), (74, 2), (71, 6), (None, 2)]
p4 = [(83, 1), (81, 1), (79, 1), (78, 1), (76, 2), (74, 2), (76, 3), (79, 1), (78, 4),
      (76, 6), (None, 2)]
t = line(t, p1)
t = line(t, p2)
ramp(t, 20, v(0.55), v(0.8), 10)
t = line(t, p3)
ramp(t - 3 * BEAT, 21, v(0.5), v(0.9), 2.5)  # vibrato swell on the long B
ramp(t + BEAT, 21, v(0.9), v(0.5), 1.0)
# Detached call and answer (MONO-like: no overlap, no glide, new attacks).
for n in [71, 74, 76, None, 74, 76, 79, None]:
    if n is not None:
        note(t, n, BEAT * 0.6, 105)
    t += BEAT
t = line(t, p4)
t = line(t, p1)
t = line(t, p3)
ramp(t - 2 * BEAT, 28, v(0.35), v(0.6), 3)

# Breakdown (6:35-7:50).
t = S + 395
button(t - 1.5, 44)  # Break
ramp(t + 1, 27, v(0.8), 0, 6)
ramp(t, 20, v(0.8), v(0.2), 10)
ramp(t, 23, v(0.65), v(0.85), 12)
ramp(t + 3, 24, 0, v(0.7), 8)
ramp(t + 3, 26, v(0.22), v(0.5), 8)
for k, (notes, dur) in enumerate([([52, 59, 64, 71], 10), ([48, 55, 64, 67], 10),
                                  ([50, 57, 66, 69], 10), ([47, 54, 62, 66], 12)]):
    chord(t + 6 + k * 12, notes, dur, 80)
line(t + 58, [(76, 4), (74, 2), (71, 6)], overlap=0.3, vel=70)

# Return (7:50-9:10).
t = S + 470
button(t - 1.5, 42)  # Groove
ramp(t, 24, v(0.7), v(0.2), 8)
ramp(t, 23, v(0.85), v(0.5), 10)
ramp(t, 25, 0, v(0.7), 6)
ramp(t, 26, v(0.5), v(0.5), 1)
ramp(t + 2, 20, v(0.2), v(0.6), 12)
button(t + 36, 43)  # Lift
ramp(t + 36, 20, v(0.6), v(0.85), 8)
for k, c in enumerate([EM, CMAJ7, D6, BM7, EM, AM9, CMAJ7, D6]):
    chord(t + 1 + k * 2 * BAR * 0.95, c, 2 * BAR * 0.95 - 0.25, 85)
# The last phrase: strings and pad step back for the lead.
tl = t + 1 + 8 * 2 * BAR * 0.95
ramp(tl - 2, 25, v(0.7), 0, 3)
ramp(tl - 2, 26, v(0.5), v(0.2), 3)
ramp(tl - 2, 27, 0, v(0.8), 2)
tl = line(tl, p2)
tl = line(tl, [(79, 2), (78, 2), (76, 8)])
ramp(tl - 8 * BEAT, 21, v(0.5), v(0.9), 3)

# Close (9:10-): sequences rest, the chords ring out, then the transport stops.
t = S + 550
button(t, 40)  # Air
ramp(t, 27, v(0.8), 0, 4)
ramp(t + 1, 25, 0, v(0.6), 4)
ramp(t + 1, 24, v(0.2), v(0.6), 4)
ramp(t, 23, v(0.5), v(0.8), 8)
chord(t + 2, [40, 52, 59, 64, 67, 71], 12, 80)
button(t + 16, 46)  # Stop: nothing new starts; the tails ring
at(t + 23, "click rec-stop")
at(t + 25, "sleep 1")

ev.sort(key=lambda e: e[0])
out = ["# Scripted take of patches/sound-palette; generated by make_piece.py (see its doc).",
       "# `wait T` = until T s after the script started (drive.py), so nothing drifts."]
now = 0.0
for time, l in ev:
    if time > now + 0.005:
        out.append(f"wait {time:.3f}")
        now = time
    out.append(l)
with open(os.path.join(os.path.dirname(__file__), "piece.txt"), "w") as f:
    f.write("\n".join(out) + "\n")
print(f"{len(ev)} events, {now / 60:.1f} min")
