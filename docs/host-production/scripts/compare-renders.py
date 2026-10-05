#!/usr/bin/env python3
"""Compare decoded float32 PCM, ignoring WAV metadata."""
import array
import hashlib
import json
import struct
import sys
from pathlib import Path

def read(path):
    raw = Path(path).read_bytes()
    assert raw[:4] == b'RIFF' and raw[8:12] == b'WAVE'
    chunks = {}
    at = 12
    while at + 8 <= len(raw):
        name, size = struct.unpack_from('<4sI', raw, at)
        chunks[name] = raw[at + 8:at + 8 + size]
        at += 8 + size + size % 2
    fmt = struct.unpack_from('<HHIIHH', chunks[b'fmt '])
    assert fmt[0] == 3 and fmt[1] == 2 and fmt[2] == 48000 and fmt[5] == 32
    data = chunks[b'data']
    assert len(data) == 36 * 48000 * 2 * 4
    return data

if __name__ == '__main__':
    values = [read(p) for p in sys.argv[1:]]
    floats = [array.array('f', b) for b in values]
    result = {'files': sys.argv[1:], 'format': '36 s / 48 kHz / stereo float32', 'pcm_equal': all(b == values[0] for b in values), 'pcm_sha256': [hashlib.sha256(b).hexdigest() for b in values], 'different_samples': [sum(a != b for a, b in zip(floats[0], f)) for f in floats[1:]], 'maximum_abs_difference': [max(abs(a-b) for a, b in zip(floats[0], f)) for f in floats[1:]]}
    print(json.dumps(result, indent=2))
