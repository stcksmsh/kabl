#!/usr/bin/env python3
"""Polls KABL_STATS_FILE every 0.1 s and prints each change of the stats line's MIDI part
with wall-clock time: `watch_stats.py STATS OUT` (stops when OUT's parent drive ends: kill it)."""
import re, sys, time
stats, out = sys.argv[1], sys.argv[2]
last = None
with open(out, "w") as f:
    while True:
        try:
            line = open(stats).read().strip()
        except FileNotFoundError:
            line = ""
        m = re.search(r"(\d+) MIDI notes held · (\d+) voices sounding", line)
        cur = m.groups() if m else None
        if cur != last:
            f.write(f"{time.time():.2f} held {cur[0] if cur else '?'} sounding {cur[1] if cur else '?'}\n")
            f.flush()
            last = cur
        time.sleep(0.1)
