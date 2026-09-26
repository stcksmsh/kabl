#!/usr/bin/env python3
"""Summarizes perf.sh's runs as a markdown table: one row per run, execution, arrival and
xruns kept apart, graph builds and runtime values, CC-to-audio latency.

    python3 docs/runtime-controls/scripts/perf_table.py target/runtime-controls-perf
"""
import glob
import os
import re
import sys

root = sys.argv[1]
rows = []
for d in sorted(glob.glob(os.path.join(root, "*-*-*"))):
    name = os.path.basename(d)
    work, build, run = name.rsplit("-", 2)
    try:
        stats = open(os.path.join(d, "stats.txt")).read()
    except OSError:
        rows.append((work, build, run, "no stats (run failed or stalled before the first write)"))
        continue
    g = lambda pat, s=stats: (re.search(pat, s) or [None, "?"])[1]
    count = g(r"^(\d+) callbacks")
    worst = g(r"run: worst (\d+) of")
    half = g(r"(\d+) over half")
    late = g(r"over half, (\d+) late")
    arr_worst = g(r"arrival: worst (\d+) µs")
    arr_late = g(r"arrival: worst \d+ µs, (\d+) late")
    xruns = g(r"· (\d+) xruns")
    p50 = g(r"p50 (\S+)")
    p99 = g(r"p99 (\S+)")
    p999 = g(r"p99\.9 (\S+)")
    logs = glob.glob(os.path.join(d, "logs", "*.log"))
    compiles = sum(open(f).read().count("compiled generation") for f in logs)
    values = g(r"(\d+) runtime values")
    try:
        lat = open(os.path.join(d, "latency.txt")).read()
        lat = re.sub(r"CC to audio callback: ", "", lat).strip()
        lat = lat.replace(" · 0 waiting", "")
    except OSError:
        lat = "-"
    if lat.startswith("0 batches"):
        lat = "-"
    stall = "stall" if "stalled" in open(logs[0]).read() else ""
    rows.append(
        (work, build, run,
         f"{count} | {p50} / {p99} / {p999} | {worst} | {half} | {late} | {arr_worst} | "
         f"{arr_late} | {xruns} | {compiles} | {values if build == 'final' else '-'} | {lat} | {stall}")
    )
print("| Workload | Build | Run | Callbacks | Execution p50 / p99 / p99.9 (µs, 50 µs bins) | "
      "Worst (µs) | Over half | Late | Arrival worst (µs) | Arrival late | Xruns | Graph builds | "
      "Runtime values | CC → callback latency | Stall |")
print("|" + "---|" * 15)
order = {"steady": 0, "knob": 1, "cc": 2, "swaps": 3}
for r in sorted(rows, key=lambda r: (order.get(r[0], 9), r[1], r[2])):
    print(f"| {r[0]} | {r[1]} | {r[2]} | {r[3]} |")
