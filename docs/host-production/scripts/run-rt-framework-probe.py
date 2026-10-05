#!/usr/bin/env python3
"""Build and inspect the locked framework's real AtomicCell types; exit 2 means R6 fails."""
import json
from pathlib import Path
import subprocess
import sys

repo = Path(__file__).resolve().parents[3]
out = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else repo / 'scratch/r6-audit'
out.mkdir(parents=True, exist_ok=True)
build = subprocess.run(
    ['cargo', 'build', '--locked', '-p', 'kabl-clap', '--lib', '--message-format=json'],
    cwd=repo, capture_output=True, text=True, check=True,
)
(out / 'probe-build.jsonl').write_text(build.stdout)
(out / 'probe-build.txt').write_text(build.stderr)
libraries = {}
for line in build.stdout.splitlines():
    record = json.loads(line)
    if record.get('reason') == 'compiler-artifact':
        name = record['target']['name']
        if name in ('nice_plug', 'crossbeam_utils'):
            libraries[name] = next(p for p in record['filenames'] if p.endswith('.rlib'))
assert set(libraries) == {'nice_plug', 'crossbeam_utils'}, libraries
command = ['rustc', '--edition=2021', str(Path(__file__).with_name('rt-framework-probe.rs')),
           '-L', 'dependency=' + str(Path(libraries['nice_plug']).parent)]
for name, path in libraries.items():
    command += ['--extern', name + '=' + path]
command += ['-o', str(out / 'rt-framework-probe')]
subprocess.run(command, cwd=repo, check=True)
probe = subprocess.run([str(out / 'rt-framework-probe')], capture_output=True, text=True)
(out / 'atomic-cell.txt').write_text(probe.stdout + probe.stderr)
(out / 'probe-command.json').write_text(json.dumps(command, indent=2) + '\n')
print(probe.stdout + probe.stderr, end='')
sys.exit(probe.returncode)
