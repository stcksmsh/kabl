#!/usr/bin/env python3
"""Verify completion and retain reset telemetry for a controlled scratch render."""
import json
import os
import subprocess
import sys
import time
from pathlib import Path
repo = Path(__file__).resolve().parents[3]
name = sys.argv[1]
policy = sys.argv[2] if len(sys.argv) > 2 else 'fresh-process'
out = repo / 'scratch/closeout' / name
out.mkdir(exist_ok=False)
env = dict(os.environ, DISPLAY=':100', XDG_RUNTIME_DIR='/run/user/1000', PIPEWIRE_QUANTUM='256/48000', CLAP_PATH=str(repo / 'scratch/closeout/proxy'), KABL_FACTORY_DIR=str(out / 'unavailable-factory'), KABL_TIMING_PLUGIN=str(repo / 'target/release/libkabl_clap.so'), KABL_TIMING_OUTPUT=str(out / 'callbacks.csv'), KABL_RESET_TRACE='1', KABL_D07_HOST=str(out), KABL_RENDER_NAME=name, KABL_RENDER_POLICY=policy)
if policy == 'no-render-restart':
    env['KABL_NO_RENDER_RESTART'] = '1'
cmd = ['pw-jack', '/usr/local/bin/reaper', '-newinst', '-nosplash', '-cfgfile', str(repo / 'scratch/closeout/profile/reaper.ini'), str(repo / 'scratch/host/after-cycles.rpp'), str(repo / 'docs/reaper-instrument/scripts/render-diagnostic.lua')]
with (out / 'reaper.log').open('w') as log:
    process = subprocess.Popen(cmd, env=env, stdout=log, stderr=subprocess.STDOUT)
    deadline = time.monotonic() + 45
    while not (out / f'{name}.done').exists() and process.poll() is None and time.monotonic() < deadline:
        time.sleep(0.25)
    completed = (out / f'{name}.done').exists()
    subprocess.run(['/usr/local/bin/reaper', '-nonewinst', '-cfgfile', str(repo / 'scratch/closeout/profile/reaper.ini'), '-closeall:nosave:exit'], env=env, stdout=log, stderr=subprocess.STDOUT, timeout=5)
    try:
        code = process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        process.terminate(); code = process.wait(timeout=5)
result = {'case': name, 'policy': policy, 'completed': completed, 'exit': code, 'marker': (out / f'{name}.done').read_text() if completed else None, 'trace': (out / 'callbacks.csv.trace').read_text() if (out / 'callbacks.csv.trace').exists() else None}
(out / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
assert completed and code == 0, 'render completion not established'
