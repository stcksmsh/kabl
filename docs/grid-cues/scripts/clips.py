#!/usr/bin/env python3
"""Encodes docs/grid-cues/clips/grid-cues.wav as 320 kbps mp3 and 24-bit FLAC and checks the mp3.

    cargo build --release -p kabl-ui --example grid_cues_demo
    target/release/examples/grid_cues_demo render patches/grid-cues docs/grid-cues/clips/grid-cues.wav
    python3 docs/grid-cues/scripts/clips.py

Reuses `encode_and_check` from docs/presets/scripts/clips.py (read from origin/master: the
part above its `good = True` line, which only defines helpers). Run from the repo root.
"""
import subprocess as sp, sys
from pathlib import Path

repo = Path(__file__).resolve().parents[3]
src = sp.run(['git', 'show', 'origin/master:docs/presets/scripts/clips.py'], cwd=repo,
             check=True, capture_output=True, text=True).stdout.split('\ngood = True')[0]
ns = {'__file__': str(repo / 'docs/presets/scripts/clips.py'), '__name__': 'presets_clips'}
exec(compile(src, 'presets/clips.py', 'exec'), ns)
wav = repo / 'docs/grid-cues/clips/grid-cues.wav'
sys.exit(0 if ns['encode_and_check'](str(wav), str(wav)[:-4] + '.mp3') else 1)
