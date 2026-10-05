#!/usr/bin/env python3
"""Create source-free Linux owner bundle with complete embedded project state."""
import hashlib, json, shutil, subprocess, sys
from pathlib import Path
repo=Path(__file__).resolve().parents[3];out=Path(sys.argv[1]).resolve();out.mkdir(parents=True,exist_ok=False)
for name in ('plugins','projects','media','profile','profile/empty-vst','scripts','runs'): (out/name).mkdir(exist_ok=True)
shutil.copy2(repo/'target/release/libkabl_clap.so',out/'plugins/kabl.clap')
shutil.copy2(repo/'docs/host-production/projects/arrangement.rpp',out/'projects/arrangement.rpp')
for name in ('render.lua','serial-repeat.lua','compare-renders.py','recall.lua'): shutil.copy2(repo/'docs/host-production/scripts'/name,out/'scripts'/name)
for source in (repo/'docs/host-production/media').iterdir():
    if source.is_file(): shutil.copy2(source,out/'media'/source.name)
render=Path(sys.argv[2]).resolve() if len(sys.argv)>2 else repo/'scratch/host/d08-render-38.wav'
shutil.copy2(render,out/'media/arrangement.wav')
(out/'docs').mkdir()
for name in ('README.md','design.md','REPORT.md','REVIEW.md','CHECKLIST.md','RT-OWNERSHIP.md'):
    shutil.copy2(repo/'docs/host-production'/name,out/'docs'/name)
(out/'profile/reaper.ini').write_text('[reaper]\nlinux_audio_mode=0\nlinux_audio_srate=48000\nlinux_audio_bsize=256\nvstpath64='+str(out/'profile/empty-vst')+'\n')
(out/'launch.sh').write_text('''#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
export CLAP_PATH="$root/plugins" KABL_FACTORY_DIR="$root/unavailable-factory"
export KABL_D08_HOST="$root/runs" KABL_D08_SCRIPTS="$root/scripts"
mkdir -p "$root/runs"
project="$root/projects/arrangement.rpp"
case "${1:-}" in *.rpp|*.RPP) project=$1; shift ;; esac
exec pw-jack /usr/local/bin/reaper -newinst -nosplash -cfgfile "$root/profile/reaper.ini" "$project" "$@"
''');(out/'launch.sh').chmod(0o755)
(out/'render-policy.sh').write_text('''#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
exec "$root/launch.sh" "$root/scripts/serial-repeat.lua"
''');(out/'render-policy.sh').chmod(0o755)
(out/'README.txt').write_text('''D08 Linux x86_64 owner test, REAPER 7.75 + PipeWire JACK.
Run ./launch.sh from any directory. Uses isolated profile and packaged production CLAP.
No source checkout, build toolchain, factory library or external media is required.
R6 remains unresolved: the framework callback mutex, fallback spin locks and retrying handoffs
prevent strict whole-callback real-time acceptance. No repair or acceptance waiver was shipped.
Read docs/RT-OWNERSHIP.md for source evidence, concrete alternatives and maintenance impact.
Project embeds played virtual/scripted MIDI and Host-synchronized sequence, complete states,
stable mappings, tempo changes and automation. No physical controller evidence is claimed.

Open either rack. Automation slots show mappings and native normalized values. Host clock
follows REAPER; Free clock follows document BPM. Host transport labels cannot be clicked.
Use REAPER Write/Touch to record slot/rack gestures, then Read to replay. Save a COPY, quit
REAPER completely, relaunch the copy through this isolated profile. Owner checks remain pending.

Repeatable offline policy: run ./render-policy.sh in a fresh process. Script initializes audio,
waits one second for REAPER project setup, disables track buffering/anticipation, closes audio,
and completes three 36-second float32 stereo 48-kHz renders in runs/. FIRST full render is
warm-up and is retained; compare SECOND and THIRD PCM with scripts/compare-renders.py.
This policy is explicit. Ordinary repeated renders still differ. Do not discard failures silently.
Restart/seek/loop reset Host DSP seeds; stop releases Host notes and keeps effects tails.
Mode changes reset DSP. Free mode retains legacy behavior. Initial bar-launch policy is 4/4.

media/arrangement.wav is actual completed production render under declared policy.
UI/video input is scripted; walkthrough video is silent. Controller/listening/feel/physical latency
need Kosta's checks. Extreme 94-module two-instance open-editor case shows backend errors in
Xvfb measurements; use fewer open dense editors or larger host buffer if needed.

Optional install: close host, ensure ~/.clap/kabl.clap does not exist, then copy plugins/kabl.clap
there. Never overwrite an existing plugin without preserving it. Optional uninstall: close host
and remove ONLY that installed ~/.clap/kabl.clap. For isolated usage, close scratch REAPER and
remove this bundle directory. No global installation or user profile modification was performed.
''')
shutil.copy2(Path('/home/stcksmsh/.cargo/git/checkouts/nice-plug-cc55f1f1bcef5b66/263b168/LICENSE'),out/'nice-plug-LICENSE')
verification=json.loads((repo/'docs/host-production/evidence/submission-verification.json').read_text())
assert hashlib.sha256((out/'plugins/kabl.clap').read_bytes()).hexdigest()==verification['binary_sha256'],'binary differs from tested product'
manifest={'product_head':verification['product_head'],'binary_sha256':verification['binary_sha256'],'mutable_dirs':['profile','runs'],'files':{str(p.relative_to(out)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(out.rglob('*')) if p.is_file() and p.relative_to(out).parts[0] not in ('profile','runs')}}
(out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');print(out)
