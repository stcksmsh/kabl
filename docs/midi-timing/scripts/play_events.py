#!/usr/bin/env python3
"""Plays an `.events` file live: prints `raw HEX...` lines for midi_player at their times.

    python3 docs/midi-timing/scripts/play_events.py EVENTS DELAY | target/release/examples/midi_player

DELAY seconds pass before time 0. Sleeps to 2 ms before each message, then spins, so the
sender adds well under a millisecond of jitter (scripted virtual MIDI, not a controller).
"""
import sys
import time

events = [l.split(None, 1) for l in open(sys.argv[1]) if l.strip() and not l.startswith("#")]
start = time.monotonic() + float(sys.argv[2])
for t, msg in events:
    due = start + float(t)
    while (left := due - time.monotonic()) > 0.002:
        time.sleep(left - 0.002)
    while time.monotonic() < due:
        pass
    sys.stdout.write(f"raw {msg.strip()}\n")
    sys.stdout.flush()
time.sleep(1.0)
sys.stdout.write("quit\n")
