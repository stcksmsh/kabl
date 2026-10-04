#!/usr/bin/env python3
"""Run eight scratch REAPER cases; never touch the user's profile."""
import itertools
import json
import os
import subprocess
import time
from pathlib import Path
repo = Path(__file__).resolve().parents[3]
results = []
for kind, count, opened in itertools.product(('light', 'dense'), (1, 2), (0, 1)):
    label = f'{kind}-{count}-' + ('open' if opened else 'closed')
    out = repo / 'scratch/closeout/measure' / label
    out.mkdir(parents=True, exist_ok=False)
    env = dict(os.environ, DISPLAY=':100', XDG_RUNTIME_DIR='/run/user/1000', PIPEWIRE_QUANTUM='256/48000', CLAP_PATH=str(repo / 'scratch/closeout/proxy'), KABL_TIMING_PLUGIN=str(repo / 'target/release/libkabl_clap.so'), KABL_TIMING_OUTPUT=str(out / 'callbacks.csv'), KABL_D07_HOST=str(out), KABL_MEASURE_KIND=kind, KABL_MEASURE_COUNT=str(count), KABL_MEASURE_OPEN=str(opened))
    cmd = ['pw-jack', '/usr/local/bin/reaper', '-newinst', '-nosplash', '-cfgfile', str(repo / 'scratch/closeout/profile/reaper.ini'), str(repo / 'scratch/host/after-cycles.rpp'), str(repo / 'docs/reaper-instrument/scripts/measure-host.lua')]
    with (out / 'reaper.log').open('w') as log:
        process = subprocess.Popen(cmd, env=env, stdout=log, stderr=subprocess.STDOUT)
        backend_log = (out / 'pipewire.txt').open('w')
        backend = subprocess.Popen(['pw-top', '-b', '-n', '18'], env=env, stdout=backend_log, stderr=subprocess.STDOUT)
        deadline = time.monotonic() + 35
        while not (out / 'measure.done').exists() and process.poll() is None and time.monotonic() < deadline:
            time.sleep(0.5)
        done = (out / 'measure.done').exists()
        subprocess.run(['/usr/local/bin/reaper', '-nonewinst', '-cfgfile', str(repo / 'scratch/closeout/profile/reaper.ini'), '-closeall:nosave:exit'], env=env, stdout=log, stderr=subprocess.STDOUT, timeout=5)
        try:
            code = process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.terminate(); code = process.wait(timeout=5)
        backend.terminate(); backend.wait(timeout=5); backend_log.close()
    result = {'case': label, 'completed': done, 'exit': code, 'device': (out / 'measure-device.txt').read_text() if (out / 'measure-device.txt').exists() else None, 'callbacks': (out / 'callbacks.csv').read_text() if (out / 'callbacks.csv').exists() else None}
    result['gui_handles'] = (out / 'gui-handles.txt').read_text() if (out / 'gui-handles.txt').exists() else None
    results.append(result)
    print(json.dumps(result), flush=True)
    (repo / 'docs/reaper-instrument/evidence/host-timing-refresh.json').write_text(json.dumps(results, indent=2) + '\n')
    assert done and code == 0 and result['gui_handles'] and result['callbacks'] and all(x in result['device'] for x in ('GUI_checks=3', 'SRATE=true 48000', 'BSIZE=true 256', 'MODE=true JACK')), 'incomplete host case; raw evidence retained'
(repo / 'docs/reaper-instrument/evidence/host-timing-refresh.json').write_text(json.dumps(results, indent=2) + '\n')
