#!/usr/bin/env python3
"""Turns timeline.json (what happens when, from promo_demo.rs) and targets.json (which UI
targets to press) into a script for docs/rack-migration/drive.py.

    make-drive-script.py docs/promo-demo/timeline.json docs/promo-demo/targets.json OUT.txt

The clock is restarted by a click at a known time, so bar 0 of the piece is that time plus the
click latency; every gesture is an editor opened a few seconds ahead and a timed drag on the
cable's "Morph A to B" value.

Optional 4th argument: a storyboard JSON, a bar-timed list of any drive command:

    {"bpm": 112, "bar_beats": 4, "anchor": "prun:{clock}", "clock": "7",
     "shots": [{"bar": 16, "do": ["click perform", "click pcue:12.2", "slidey -60 2"]}]}

Each shot writes `wait t0+bar*bar_seconds` and then its raw commands. They are merged with the
morph gestures by time. `anchor` (else targets.json "anchor", else "restart") is the bar-0 click
key; `bpm` (else timeline bar_seconds) sets the bar length. The timeline may be "-" to script
only a storyboard. `setup` (a list of drive commands) replaces the click on targets.json
`fit_view` that runs before the anchor (an empty rack has nothing to fit).
"""
import json
import sys

timeline = json.load(open(sys.argv[1])) if sys.argv[1] != "-" else {"gestures": [], "bars": 0}
targets, out = json.load(open(sys.argv[2])), sys.argv[3]
sb = json.load(open(sys.argv[4])) if len(sys.argv) > 4 else {"shots": []}
t = targets["timing"]
bar = 240 / sb["bpm"] * sb.get("bar_beats", 4) / 4 if "bpm" in sb else timeline["bar_seconds"]
anchor = sb.get("anchor", targets.get("anchor", targets["restart"])).format(**sb)
t0 = t["restart_at_s"] + t["restart_latency_s"]
lines = [
    f"sleep {t['settle_s']}",
    *sb.get("setup", [f"click {targets['fit_view']}"]),
    f"aim {anchor}",
    f"wait {t['restart_at_s']}",
    f"click {anchor}",
]
events = []  # (time, lines): merged by time, since `wait` only moves forward
for g in timeline["gestures"]:
    cable = timeline["cables"][g["cable"]]
    start, end = t0 + g["start_bar"] * bar, t0 + g["end_bar"] * bar
    dx = (g["to"] - g["from"]) * targets["morph_points_per_unit"]
    events.append((start - t["open_lead_s"], [f"# bar {g['start_bar']}-{g['end_bar']}: {g['what']}",
        f"wait {start - t['open_lead_s']:.2f}",
        *(f"{cmd} {key.format(id=cable['id'])}" for cmd, key in targets["open"][cable["kind"]]),
        f"aim {targets['morph'].format(id=cable['id'])}", f"wait {start:.2f}",
        f"slide {dx:.0f} {end - start:.2f}"]))
for sh in sb["shots"]:
    at = t0 + sh["bar"] * bar
    events.append((at, [f"# bar {sh['bar']}", f"wait {at:.2f}", *sh["do"]]))
for _, ev in sorted(events, key=lambda e: e[0]):
    lines += ev
end_bar = max([timeline["bars"]] + [sh["bar"] for sh in sb["shots"]])
lines.append(f"wait {t0 + end_bar * bar:.2f}")
open(out, "w").write("\n".join(lines) + "\n")
print(f"{out}: {len(timeline['gestures'])} gestures, {len(sb['shots'])} shots, piece {end_bar * bar:.0f} s from t0={t0:.2f} s")
