#!/usr/bin/env python3
"""Generate a redistributable MIDI type-0 host fixture."""
from pathlib import Path
import struct
import sys

def vlq(n):
    parts = [n & 127]
    while n > 127:
        n >>= 7
        parts.insert(0, (n & 127) | 128)
    return bytes(parts)

events = [(0, bytes.fromhex("ff510307a120")), (0, bytes.fromhex("903c64")),
          (480, bytes.fromhex("803c00")), (0, bytes.fromhex("904064")),
          (480, bytes.fromhex("804000")), (0, bytes.fromhex("904364")),
          (480, bytes.fromhex("804300")), (0, bytes.fromhex("ff2f00"))]
track = b"".join(vlq(delta) + event for delta, event in events)
data = b"MThd" + struct.pack(">IHHH", 6, 0, 1, 480)
data += b"MTrk" + struct.pack(">I", len(track)) + track
out = Path(sys.argv[1] if len(sys.argv) > 1 else "proof.mid")
out.write_bytes(data)
print(out)
