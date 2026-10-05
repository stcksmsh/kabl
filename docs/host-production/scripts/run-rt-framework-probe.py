#!/usr/bin/env python3
"""Build and inspect the locked framework's real AtomicCell types; including adversarial tests of the actual vendored source."""
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
        if name in ('nice_plug', 'nice_plug_core', 'crossbeam_utils', 'atomic_refcell', 'parking_lot', 'rtrb'):
            libraries[name] = next(p for p in record['filenames'] if p.endswith('.rlib'))
assert set(libraries) == {'nice_plug', 'nice_plug_core', 'crossbeam_utils', 'atomic_refcell', 'parking_lot', 'rtrb'}, libraries
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
if probe.returncode: sys.exit(probe.returncode)
# Compile the same committed module with its private adversarial tests enabled.
# No standalone vendor dependency resolution or test-only copy of implementation.
command = ['rustc', '--test', '--edition=2024', str(repo / 'vendor/nice-plug/crates/nice-plug/src/wrapper/clap/strict.rs'),
           '-L', 'dependency=' + str(Path(libraries['nice_plug']).parent)]
for name in ('nice_plug_core', 'atomic_refcell', 'parking_lot', 'rtrb'):
    command += ['--extern', ('strict_rtrb' if name == 'rtrb' else name) + '=' + libraries[name]]
command += ['-o', str(out / 'strict-framework-tests')]
subprocess.run(command, cwd=repo, check=True)
tests = subprocess.run([str(out / 'strict-framework-tests')], capture_output=True, text=True)
(out / 'strict-framework-tests.txt').write_text(tests.stdout + tests.stderr)
print(tests.stdout + tests.stderr, end='')
sys.exit(tests.returncode)
