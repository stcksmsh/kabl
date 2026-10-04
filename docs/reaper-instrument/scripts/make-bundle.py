#!/usr/bin/env python3
"""Build the Linux owner-test bundle without installing anything globally."""
import hashlib
import json
import shutil
import subprocess
import sys
from pathlib import Path

repo = Path(__file__).resolve().parents[3]
out = Path(sys.argv[1]).resolve()
out.mkdir(parents=True, exist_ok=False)
for folder in ('plugins', 'projects', 'media', 'profile', 'profile/empty-vst'):
    (out / folder).mkdir(exist_ok=True)
shutil.copy2(repo / 'target/release/libkabl_clap.so', out / 'plugins/kabl.clap')
project = (repo / 'scratch/host/after-cycles.rpp').read_text()
project = project.replace('/secondary/Programming/Github/kabl/.worktrees/d07/scratch/host/d07-musical.wav', str(out / 'media/render.wav'))
(out / 'projects/two-instances.rpp').write_text(project)
for name in ('d07-musical-demo.wav', 'live-reaper-demo.wav', 'operation-1.mp4', 'editor-cycles.mp4', 'edited-actual.png', 'browser-loaded.png'):
    shutil.copy2(repo / 'scratch/host' / name, out / 'media' / name)
(out / 'profile/reaper.ini').write_text('[reaper]\nlinux_audio_mode=0\nlinux_audio_srate=48000\nlinux_audio_bsize=256\nvstpath64=' + str(out / 'profile/empty-vst') + '\n')
(out / 'launch.sh').write_text('''#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
export CLAP_PATH="$root/plugins"
export KABL_FACTORY_DIR="$root/unavailable-factory"
exec pw-jack /usr/local/bin/reaper -newinst -nosplash -cfgfile "$root/profile/reaper.ini" "$root/projects/two-instances.rpp" "$@"
''')
(out / 'launch.sh').chmod(0o755)
(out / 'README.txt').write_text('''D07 Linux x86_64 owner test (REAPER 7.75).
Run ./launch.sh from any directory. Uses a separate profile and packaged CLAP.
Project embeds two complete sounds and scripted MIDI. Factory path is unavailable deliberately.
Play, edit rack, close/reopen editors, save a COPY, quit/reopen, render.
media/d07-musical-demo.wav: 36 s trimmed offline render, uniform +16.15 dB gain.
media/live-reaper-demo.wav: 34.6 s trimmed live REAPER capture, same uniform gain.
Videos show actual scratch host operation; they have NO audio.
Physical controller, listening and feel checks remain owner pending.
Repeated renders currently differ; do not use this as a deterministic render guarantee.
Optional install: copy plugins/kabl.clap into ~/.clap only after checking no existing kabl.clap.
Uninstall: remove that exact installed file; close scratch REAPER then remove this bundle directory.
No global installation or configuration change was performed by bundle creation.
''')
license_root = Path('/home/stcksmsh/.cargo/git/checkouts/nice-plug-cc55f1f1bcef5b66/263b168')
for p in license_root.glob('LICENSE*'):
    shutil.copy2(p, out / ('nice-plug-' + p.name))
manifest = {'product_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip(), 'files': {str(p.relative_to(out)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(out.rglob('*')) if p.is_file() and 'profile' not in p.relative_to(out).parts}}
(out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(out)
