#!/usr/bin/env python3
"""Compare complete embedded CLAP states after an actual REAPER save/restart."""
import base64
import hashlib
import json
from pathlib import Path
import re
import struct
import sys

def states(path):
    chunks = re.findall(r'<STATE\s+([^>]+)>', Path(path).read_text())
    assert len(chunks) == 2, 'expected two embedded instrument states'
    result = []
    for chunk in chunks:
        raw = base64.b64decode(''.join(chunk.split()), validate=True)
        size = struct.unpack_from('<Q', raw)[0]
        assert size == len(raw) - 8, 'invalid REAPER state size prefix'
        result.append(json.loads(raw[8:]))
    return result

if __name__ == '__main__':
    assert len(sys.argv) == 3, 'usage: compare-recall.py original.rpp recalled.rpp'
    original, recalled = (states(path) for path in sys.argv[1:])
    equal = original == recalled
    print(json.dumps({
        'files': sys.argv[1:], 'states_equal': equal,
        'complete_state_sha256': [hashlib.sha256(json.dumps(state, sort_keys=True).encode()).hexdigest() for state in recalled],
        'instances': [{'modules': len(state['patch']['modules']),
                       'output_gain': state['output_gain'], 'host_clock': state['host_clock'],
                       'lanes': state['lanes'], 'slot_values': state['slot_values']} for state in recalled],
    }, indent=2))
    sys.exit(0 if equal else 1)
