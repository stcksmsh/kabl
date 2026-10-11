#!/usr/bin/env python3
"""Write one single-module patch per new kind, for capturing each face in the real editor.

    python3 docs/effects-filters/scripts/face_patches.py OUTDIR
"""
import json
import os
import sys

KINDS = ["phaser", "flanger", "compressor", "eq", "limiter", "bitcrusher", "filter.comb", "filter.formant",
         "filter.svf"]

out = sys.argv[1]
for kind in KINDS:
    d = os.path.join(out, kind)
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, "log.jsonl"), "w") as f:
        json.dump({"seq": 0, "t_ms": 0, "op": {"AddModule": {"id": 1, "kind": kind, "pos": {"x": 24.0, "y": 20.0}}},
                   "inverse": {"RemoveModule": {"id": 1}}, "source": "User"}, f)
        f.write("\n")
    with open(os.path.join(d, "meta.toml"), "w") as f:
        f.write("schema_version = 5\n")
