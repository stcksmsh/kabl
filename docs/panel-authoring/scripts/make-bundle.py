#!/usr/bin/env python3
"""Portable owner bundle from recorded binaries, embedded projects and original/licensed assets."""
from pathlib import Path
import sys,shutil,os,json,hashlib,subprocess
repo=Path(__file__).resolve().parents[3];root=Path(sys.argv[1]).resolve()
for d in ['bin','plugins','patches','packages','profile','projects','docs','media','scripts','art']:(root/d).mkdir(parents=True,exist_ok=True)
for src,dst in [('target/release/kabl-ui','bin/kabl-ui'),('target/release/libkabl_clap.so','plugins/kabl.clap'),('scratch/d10-host/d10-arrangement.rpp','projects/arrangement.rpp'),('scratch/d10-host/profile/reaper.ini','profile/reaper.ini')]:shutil.copy2(repo/src,root/dst)
for name in ['voice','stereo-effect','portable']:shutil.copytree(repo/f'docs/panel-authoring/examples/{name}',root/'patches'/name,dirs_exist_ok=True)
for name in ['voice.json','stereo-effect.json']:shutil.copy2(repo/f'docs/panel-authoring/examples/{name}',root/'packages'/name)
shutil.copytree(repo/'docs/panel-authoring/examples/art',root/'art',dirs_exist_ok=True)
for name in ['README.md','design.md','REPORT.md','REVIEW.md','CHECKLIST.md','BRIEF.md']:
 if (repo/'docs/panel-authoring'/name).exists():shutil.copy2(repo/'docs/panel-authoring'/name,root/'docs'/name)
for src,dst in [('crates/ui/assets/FONT-LICENSE.txt','docs/FONT-LICENSE.txt'),('vendor/nice-plug/LICENSE','docs/nice-plug-LICENSE'),('vendor/nice-plug/STRICT-PROFILE.md','docs/STRICT-PROFILE.md')]:shutil.copy2(repo/src,root/dst)
shutil.copytree(repo/'docs/panel-authoring/media',root/'media',dirs_exist_ok=True)
(root/'launch.sh').write_text('''#!/bin/sh
set -eu
bundle_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
export KABL_USER_DIR="$bundle_dir/user-library"
export KABL_FACTORY_DIR="$bundle_dir/unavailable-factory"
exec "$bundle_dir/bin/kabl-ui" --patch "$bundle_dir/patches/${1:-portable}" --size 1440x900 --no-rt
''')
(root/'launch-reaper.sh').write_text('''#!/bin/sh
set -eu
bundle_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
export CLAP_PATH="$bundle_dir/plugins"
export KABL_USER_DIR="$bundle_dir/user-library"
export KABL_FACTORY_DIR="$bundle_dir/unavailable-factory"
exec pw-jack /usr/local/bin/reaper -newinst -cfgfile "$bundle_dir/profile/reaper.ini" "$bundle_dir/projects/arrangement.rpp"
''')
for name in ['launch.sh','launch-reaper.sh']:os.chmod(root/name,0o755)
(root/'README.md').write_text('''# D10 owner test bundle

Linux x86_64. Run ./launch.sh (optional voice or stereo-effect) or ./launch-reaper.sh with installed REAPER. No original checkout, factory, library or artwork path is required. Projects embed both panel images and exact definitions. packages/ contains independent portable imports; art/ contains original SVG/PNG authoring examples.

Use Design on a face, or Composites instance tools. Apply is one project Undo step. Public controls/help always reaches every exposed parameter. Publishing a version never changes a song. See docs/CHECKLIST.md and docs/REPORT.md; engineering, merge and owner acceptance remain separate.

Embedded IBM Plex fonts are OFL-1.1 (docs/FONT-LICENSE.txt). Original artwork and code are MIT OR Apache-2.0; nice-plug remains ISC. Generated material reference was not imported as an asset. Hardware/listening/latency/feel, ordinary-render nondeterminism and dense-editor backend/performance limits remain pending.
''')
head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip()
manifest={'product_head':head,'platform':'Linux x86_64','files':{str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(root.rglob('*')) if p.is_file() and p.name!='manifest.json'}}
(root/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');print(root)
