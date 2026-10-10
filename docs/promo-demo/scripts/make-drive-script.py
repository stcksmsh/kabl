#!/usr/bin/env python3
"""Turns timeline.json (what happens when, from promo_demo.rs) and targets.json (which UI
targets to press) into a script for docs/rack-migration/drive.py.

    make-drive-script.py docs/promo-demo/timeline.json docs/promo-demo/targets.json OUT.txt

The clock is restarted by a click at a known time, so bar 0 of the piece is that time plus the
click latency; every gesture is an editor opened a few seconds ahead and a timed drag on the
cable's "Morph A to B" value.
"""
import json
import sys

timeline, targets, out = (json.load(open(sys.argv[1])), json.load(open(sys.argv[2])), sys.argv[3])
t = targets["timing"]
bar = timeline["bar_seconds"]
t0 = t["restart_at_s"] + t["restart_latency_s"]
lines = [
    f"sleep {t['settle_s']}",
    f"click {targets['fit_view']}",
    f"aim {targets['restart']}",
    f"wait {t['restart_at_s']}",
    f"click {targets['restart']}",
]
for g in timeline["gestures"]:
    cable = timeline["cables"][g["cable"]]
    start, end = t0 + g["start_bar"] * bar, t0 + g["end_bar"] * bar
    lines.append(f"# bar {g['start_bar']}-{g['end_bar']}: {g['what']}")
    lines.append(f"wait {start - t['open_lead_s']:.2f}")
    for cmd, key in targets["open"][cable["kind"]]:
        lines.append(f"{cmd} {key.format(id=cable['id'])}")
    morph = targets["morph"].format(id=cable["id"])
    lines.append(f"aim {morph}")
    lines.append(f"wait {start:.2f}")
    dx = (g["to"] - g["from"]) * targets["morph_points_per_unit"]
    lines.append(f"slide {dx:.0f} {end - start:.2f}")
lines.append(f"wait {t0 + timeline['bars'] * bar:.2f}")
open(out, "w").write("\n".join(lines) + "\n")
print(f"{out}: {len(timeline['gestures'])} gestures, piece {timeline['bars'] * bar:.0f} s from t0={t0:.2f} s")
