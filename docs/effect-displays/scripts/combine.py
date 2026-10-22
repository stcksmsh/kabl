#!/usr/bin/env python3
"""Appends the nine-effects patch (make_patch.py) to patches/composition, for the frame-time bench:
the effects go on the rows below the composition's, ids offset by 1000.

    python3 docs/effect-displays/scripts/combine.py scratch/ed-patch OUT_DIR
"""
import json
import sys
from pathlib import Path

repo = Path(__file__).resolve().parents[3]
ops = [json.loads(l) for l in (repo / 'patches/composition/log.jsonl').read_text().splitlines()]
rows = max((o['op']['AddModule']['pos']['y'] for o in ops if 'AddModule' in o['op']), default=0)
shift = 370 * (int(rows // 370) + 1)


def walk(v):
    """Offsets every module and cable id in an op (keys 'id' under the id-carrying ops)."""
    if isinstance(v, dict):
        for k, x in v.items():
            if k == 'id' and isinstance(x, int):
                v[k] = x + 1000
            else:
                walk(x)
    elif isinstance(v, list):
        for x in v:
            walk(x)


extra = [json.loads(l) for l in (Path(sys.argv[1]) / 'log.jsonl').read_text().splitlines()]
for o in extra:
    walk(o)
    if 'AddModule' in o['op']:
        o['op']['AddModule']['pos']['y'] += shift - 10
    o['seq'] += len(ops)
out = Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
(out / 'meta.toml').write_text('schema_version = 5\n')
(out / 'log.jsonl').write_text(''.join(json.dumps(o, separators=(',', ':')) + '\n' for o in ops + extra))
