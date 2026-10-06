#!/usr/bin/env python3
"""Make a separate repair bundle; never replace an earlier owner project/library."""
from pathlib import Path
import sys,shutil,os,json,hashlib,subprocess
repo=Path(__file__).resolve().parents[3];root=Path(sys.argv[1]).resolve()
if root.exists():raise SystemExit('Refuse to overwrite an existing owner bundle. Choose a fresh directory.')
for name in ['bin','plugins','patches','packages','profile','projects','docs','media','art','factory']:(root/name).mkdir(parents=True,exist_ok=True)
for src,dst in [('target/release/kabl-ui','bin/kabl-ui'),('target/release/libkabl_clap.so','plugins/kabl.clap'),('scratch/d10-repair/host/d10-arrangement.rpp','projects/arrangement.rpp'),('scratch/d10-repair/host/profile/reaper.ini','profile/reaper.ini')]:shutil.copy2(repo/src,root/dst)
shutil.copytree(repo/'patches',root/'factory',dirs_exist_ok=True)
for name in ['voice','stereo-effect','portable']:shutil.copytree(repo/f'docs/panel-authoring/examples/{name}',root/'patches'/name)
shutil.copytree(repo/'scratch/d10-repair/after/patch',root/'patches/repaired-crowded')
for name in ['voice.json','stereo-effect.json']:shutil.copy2(repo/f'docs/panel-authoring/examples/{name}',root/'packages'/name)
shutil.copytree(repo/'docs/panel-authoring/examples/art',root/'art',dirs_exist_ok=True)
shutil.copytree(repo/'docs/panel-authoring/repair',root/'docs/repair',dirs_exist_ok=True)
for name in ['README.md','design.md','REPORT.md','REVIEW.md','CHECKLIST.md','BRIEF.md']:shutil.copy2(repo/'docs/panel-authoring'/name,root/'docs'/name)
for src,dst in [('crates/ui/assets/FONT-LICENSE.txt','docs/FONT-LICENSE.txt'),('vendor/nice-plug/LICENSE','docs/nice-plug-LICENSE'),('vendor/nice-plug/STRICT-PROFILE.md','docs/STRICT-PROFILE.md')]:shutil.copy2(repo/src,root/dst)
legacy=root.parent/'d10-owner-test/user-library'
common='''#!/bin/sh
set -eu
bundle_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
export KABL_FACTORY_DIR="$bundle_dir/factory"
'''+f'''# Reuse an earlier bundle library when it contains owner resources; otherwise use normal app discovery.
if [ -z "${{KABL_USER_DIR+x}}" ] && {{ [ -d "{legacy}/sounds" ] || [ -d "{legacy}/composites" ]; }}; then
    export KABL_USER_DIR="{legacy}"
fi
'''
(root/'launch.sh').write_text(common+'''exec "$bundle_dir/bin/kabl-ui" --patch "$bundle_dir/patches/${1:-repaired-crowded}" --size "${KABL_WINDOW_SIZE:-1440x900}" --no-rt
''')
(root/'launch-reaper.sh').write_text(common+'''export CLAP_PATH="$bundle_dir/plugins"
exec pw-jack /usr/local/bin/reaper -newinst -cfgfile "$bundle_dir/profile/reaper.ini" "$bundle_dir/projects/arrangement.rpp"
''')
(root/'launch-portability-test.sh').write_text('''#!/bin/sh
set -eu
bundle_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
export KABL_FACTORY_DIR="$bundle_dir/unavailable-factory"
export KABL_USER_DIR="$bundle_dir/portability-test-profile"
exec "$bundle_dir/bin/kabl-ui" --patch "$bundle_dir/patches/${1:-repaired-crowded}" --size "${KABL_WINDOW_SIZE:-1440x900}" --no-rt
''')
for name in ['launch.sh','launch-reaper.sh','launch-portability-test.sh']:(root/name).chmod(0o755)
(root/'README.md').write_text('''# D10 repaired owner bundle

Linux x86_64. Run `./launch.sh` for the repaired crowded example, or `./launch.sh portable`, `voice`, or `stereo-effect`. Sounds uses bundled factory presets and needs no checkout. The launcher preserves an explicit KABL_USER_DIR; otherwise it reuses the previous bundle library when owner sounds/composite resources exist, or uses normal app discovery (XDG_DATA_HOME/kabl, then ~/.local/share/kabl). Previous bundle files and projects are untouched. No library contents are copied or replaced.

`./launch-portability-test.sh` is the explicit isolated, unavailable-factory/library test. It is separate from normal startup. `./launch-reaper.sh` opens an independent installed REAPER instance with this bundle's plugin and own profile.

Click unused body/title to select or drag. Lift follows the exact grab; preview shows the legal drop. Escape cancels; a completed move is one Undo. Drag near rack edges to reach more rows. Inspect internals opens a scoped rack; breadcrumb/Back restore the outer view. More controls dims the rack and keeps its layout fixed. Native Edit face chooses visibility/order; hidden settings remain accessible in More, including keyboard/exact value entry.

Read docs/repair/REPORT.md and CHECKLIST.md. Owner disposition remains changes requested until Kosta reviews the repair. Engineering verification, merge and owner acceptance are separate. Hardware/listening/latency/feel, ordinary-render nondeterminism and dense-editor performance/backend limits remain open. No merge or D11 is authorized.

Fonts: OFL-1.1 (docs/FONT-LICENSE.txt). Original art/code: MIT OR Apache-2.0; nice-plug: ISC. No generated or reference image was imported as a product asset. Existing project art remains embedded and unchanged.
''')
head=(repo/'docs/panel-authoring/repair/evidence/product-head.txt').read_text().strip()
artifact_head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip()
manifest={'product_head':head,'artifact_head':artifact_head,'platform':'Linux x86_64','factory_sounds':len(list((root/'factory').rglob('sound.toml'))),'normal_factory':'factory','normal_user_library':'explicit environment, prior owner resources, or normal app discovery','explicit_unavailable_test':'launch-portability-test.sh','files':{str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(root.rglob('*')) if p.is_file() and p.name!='manifest.json'}}
(root/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');print(root)
