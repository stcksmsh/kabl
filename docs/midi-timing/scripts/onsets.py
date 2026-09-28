#!/usr/bin/env python3
"""Onset timing of a live recording of fixtures/gate-probe against the scripted note-ons.

    python3 docs/midi-timing/scripts/onsets.py TAKE.wav docs/midi-timing/scripts/pulse.events

Each note-on in EVENTS (channel 1) should appear as one rising gate edge in the take. The
recording starts at an arbitrary moment, so the constant offset is removed (the median);
what remains is how much each note moved against the script: sender and transport jitter,
plus the app's mapping (pre-D06: quantized to callback starts; D06: each message keeps its
place inside the callback period). Prints count, spread and inter-onset error in ms.
"""
import statistics
import struct
import sys


def read_wav(path):
    with open(path, "rb") as f:
        data = f.read()
    pos, fmt, rate, ch, bits, frames = 12, None, None, None, None, None
    while pos < len(data):
        cid, size = data[pos:pos + 4], struct.unpack("<I", data[pos + 4:pos + 8])[0]
        body = data[pos + 8:pos + 8 + size]
        if cid == b"fmt ":
            fmt, ch, rate = struct.unpack("<HHI", body[:8])
            bits = struct.unpack("<H", body[14:16])[0]
        elif cid == b"data":
            frames = body
        pos += 8 + size + (size & 1)
    assert fmt in (1, 3, 0xFFFE) and frames is not None, "unsupported WAV"
    n = len(frames) // (bits // 8)
    if bits == 32 and fmt in (3, 0xFFFE):
        s = struct.unpack(f"<{n}f", frames[:n * 4])
    elif bits == 16:
        s = [v / 32768 for v in struct.unpack(f"<{n}h", frames[:n * 2])]
    else:
        raise SystemExit(f"unsupported {bits}-bit format {fmt}")
    return rate, s[0::ch]


def main():
    rate, left = read_wav(sys.argv[1])
    ons = []
    for line in open(sys.argv[2]):
        w = line.split()
        if not w or w[0].startswith("#"):
            continue
        if w[1] == "90" and int(w[3], 16) > 0:
            ons.append(float(w[0]))
    edges, high = [], False
    for i, v in enumerate(left):
        if v > 0.06 and not high:
            edges.append(i)
        high = v > 0.06
    n = min(len(edges), len(ons))
    print(f"{sys.argv[1]}: {rate} Hz, {len(edges)} onsets for {len(ons)} scripted note-ons")
    if n < 10:
        return
    dev = [edges[i] / rate - ons[i] for i in range(n)]
    med = statistics.median(dev)
    dev = [(d - med) * 1e3 for d in dev]
    ioi = [((edges[i + 1] - edges[i]) / rate - (ons[i + 1] - ons[i])) * 1e3 for i in range(n - 1)]
    a = sorted(abs(d) for d in dev)
    print(f"  offset vs script: std {statistics.pstdev(dev):.3f} ms, peak-to-peak "
          f"{max(dev) - min(dev):.3f} ms, p99 |dev| {a[int(0.99 * (len(a) - 1))]:.3f} ms")
    print(f"  inter-onset error: std {statistics.pstdev(ioi):.3f} ms, max |err| "
          f"{max(abs(x) for x in ioi):.3f} ms")
    grid = sum(1 for e in edges[:n] if e % 64 == edges[0] % 64)
    print(f"  onsets on one 64-frame phase: {grid}/{n}")


main()
