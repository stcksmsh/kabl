#!/usr/bin/env python3
from pathlib import Path
import shutil,os,json,hashlib,sys
repo=Path(__file__).resolve().parents[3]
root=Path(sys.argv[1]).resolve()
for sub in ['bin','plugins','projects','packages','profile','scripts','docs','media','patches']:(root/sub).mkdir(parents=True,exist_ok=True)
for src,dst in [('target/release/kabl-ui','bin/kabl-ui'),('target/release/libkabl_clap.so','plugins/kabl.clap'),('docs/composites/examples/projects/arrangement.rpp','projects/arrangement.rpp'),('scratch/host/profile/reaper.ini','profile/reaper.ini')]:shutil.copy2(repo/src,root/dst)
for name in ['flat-voice','voice','two-instances','workflow']:shutil.copytree(repo/'docs/composites/examples'/name,root/'patches'/name,dirs_exist_ok=True)
for name in ['voice.json','stereo-effect.json']:shutil.copy2(repo/'docs/composites/examples'/name,root/'packages'/name)
for name in ['recall.lua','render.lua','serial-repeat.lua']:shutil.copy2(repo/'docs/host-production/scripts'/name,root/'scripts'/name)
for name in ['voice.flac','stereo-effect.flac','reaper-arrangement.flac','workflow.mp4']:shutil.copy2(repo/'docs/composites/media'/name,root/'media'/name)
for name in ['vendor/nice-plug/LICENSE','vendor/nice-plug/STRICT-PROFILE.md']:
 shutil.copy2(repo/name,root/'docs'/('nice-plug-LICENSE' if name=='vendor/nice-plug/LICENSE' else Path(name).name))
(root/'launch-standalone.sh').write_text('''#!/bin/sh
set -eu
bundle_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
export KABL_USER_DIR="$bundle_dir/user-library"
export KABL_FACTORY_DIR="$bundle_dir/unavailable-factory"
exec "$bundle_dir/bin/kabl-ui" --patch "$bundle_dir/patches/${1:-two-instances}" --size 1440x900 --no-rt
''')
(root/'launch-reaper.sh').write_text('''#!/bin/sh
set -eu
bundle_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
export CLAP_PATH="$bundle_dir/plugins"
export KABL_FACTORY_DIR="$bundle_dir/unavailable-factory"
exec pw-jack /usr/local/bin/reaper -newinst -cfgfile "$bundle_dir/profile/reaper.ini" "$bundle_dir/projects/arrangement.rpp"
''')
for name in ['launch-standalone.sh','launch-reaper.sh']:os.chmod(root/name,0o755)
(root/'README.md').write_text('''# D09 owner test checkpoint

Product head: a337f2c1eae867671f9deabadfa10f681de2bb26. Linux x86_64; REAPER 7.75 used. Run launch-standalone.sh (optional argument: flat-voice, voice, two-instances or workflow), or launch-reaper.sh. No repository or original factory/library copy is required. Included library packages can be placed under user-library/composites/<definition-id>/v1.json; projects already embed their exact definitions.

Use Composites tools to select leaves, name and encapsulate; open a card to edit actual leaf modules; Close internals returns to compact cards. Scroll inside cards for controls. Duplicate creates independent leaves and copied pins; external cables, CC/buttons and native automation assignments are deliberately omitted. Repatch manually. Publishing adds a new immutable library version and never changes existing instances. Cues/clock dependencies outside the selected graph require including those modules before publishing/importing.

Kosta selected chronological project Undo: latest project transaction is undone regardless of the open instance. Instance state is independent; separate per-instance Undo histories are not required. Standalone save/reload preserves project history; host recall preserves sound, not the UI Undo stack. Engineering disposition and draft submission are recorded in docs/REPORT.md and submission.json. This bundle is not merged or owner-approved. Hardware/controller, listening/feel, physical latency/xruns and production-beta checks remain pending; D08's ordinary-render and dense-backend limits remain unchanged. Scripted FLAC examples are engine renders; workflow video is silent X11 input. REAPER project contains scripted MIDI and saved native automation, not physical performance.
''')
print(root)
