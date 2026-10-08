#!/usr/bin/env python3
"""Create a fresh portable owner bundle without replacing any owner resources."""
from pathlib import Path
import sys, shutil, subprocess as sp, hashlib, json

repo=Path(__file__).resolve().parents[3]
root=Path(sys.argv[1]).resolve()
if root.exists(): raise SystemExit('Refuse to overwrite an existing owner bundle; choose a new path.')
for name in ['bin','plugins','factory','patches','projects','profile','docs','audio']:
    (root/name).mkdir(parents=True,exist_ok=True)
shutil.copy2(repo/'target/release/kabl-ui',root/'bin/kabl-ui')
shutil.copy2(repo/'target/release/libkabl_clap.so',root/'plugins/kabl.clap')
shutil.copytree(repo/'patches',root/'factory',dirs_exist_ok=True)
shutil.copytree(repo/'docs/factory-performance-bank/performance-project',root/'patches/performance',dirs_exist_ok=True)
shutil.copy2(repo/'docs/factory-performance-bank/evidence/host/arrangement.rpp',root/'projects/arrangement.rpp')
shutil.copytree(repo/'docs/factory-performance-bank',root/'docs/factory-performance-bank',dirs_exist_ok=True)
shutil.copytree(repo/'docs/factory-performance-bank/audio',root/'audio',dirs_exist_ok=True)
shutil.copy2(repo/'crates/ui/assets/FONT-LICENSE.txt',root/'docs/FONT-LICENSE.txt')
shutil.copy2(repo/'vendor/nice-plug/LICENSE',root/'docs/nice-plug-LICENSE')
shutil.copy2(repo/'vendor/nice-plug/STRICT-PROFILE.md',root/'docs/STRICT-PROFILE.md')
shutil.copy2(repo/'Cargo.lock',root/'docs/Cargo.lock')
(root/'profile/empty-vst').mkdir()
(root/'profile/reaper.ini').write_text(f'[reaper]\nvstpath={root}/profile/empty-vst\nvstpath64={root}/profile/empty-vst\nlinux_audio_bsize=256\nlinux_audio_mode=0\nlinux_audio_srate=48000\nwnd_w=1280\nwnd_h=800\n')
common='''#!/bin/sh
set -eu
bundle_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
export KABL_FACTORY_DIR="$bundle_dir/factory"
'''
(root/'launch.sh').write_text(common+'''# Keep explicit KABL_USER_DIR or normal native/XDG user-library discovery.
exec "$bundle_dir/bin/kabl-ui" --size "${KABL_WINDOW_SIZE:-1440x900}" --no-rt
''')
(root/'launch-reaper.sh').write_text(common+'''export CLAP_PATH="$bundle_dir/plugins"
exec pw-jack /usr/local/bin/reaper -newinst -cfgfile "$bundle_dir/profile/reaper.ini" "$bundle_dir/projects/arrangement.rpp"
''')
(root/'launch-portability-test.sh').write_text('''#!/bin/sh
set -eu
bundle_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
export KABL_FACTORY_DIR="$bundle_dir/unavailable-factory"
export KABL_USER_DIR="$bundle_dir/portability-test-profile"
exec "$bundle_dir/bin/kabl-ui" --patch "$bundle_dir/patches/performance" --size "${KABL_WINDOW_SIZE:-1440x900}" --no-rt
''')
for name in ['launch.sh','launch-reaper.sh','launch-portability-test.sh']: (root/name).chmod(0o755)
(root/'README.md').write_text('''# Kabl Factory Performance Bank

Linux x86_64. Run `./launch.sh`, select Sounds/Sequences/Performances + Factory, search `curated`, and Open. Prepared Perform controls appear; sequenced content opens stopped. All existing factory entries are included. An explicit KABL_USER_DIR is preserved; otherwise the application uses its normal XDG/native user library. Existing projects, libraries and earlier bundles are untouched.

`./launch-reaper.sh` starts a separate installed REAPER host with bundled plugin and editable two-track project. `./launch-portability-test.sh` separately verifies an embedded complete performance without available factory/user libraries. Click Perform after direct folder startup. Short previews and the 72-second editable performance are in audio/; the complete performance patch is patches/performance. They use original synthesis with scripted/virtual notes and controls.

Read docs/factory-performance-bank/CHECKLIST.md, REPORT.md and REVIEW.md. Engineering, merge and owner approval remain separate. Listening, physical controller feel, physical latency/xruns and production-beta approval remain pending. Ordinary host-render nondeterminism, warm-up policy, dense-editor/backend limits and strict-profile assumptions are inherited.

System requirements: existing Linux audio/GUI runtime libraries and an installed REAPER for host use. No global plugin install is performed. Remove this bundle to uninstall; user libraries remain separate. Factory synthesis is original Kabl work under workspace MIT OR Apache-2.0 licensing. Font notices: docs/FONT-LICENSE.txt; nice-plug ISC notice: docs/nice-plug-LICENSE. No reference recording or external artwork is included.
''')
head=sp.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip()
product=sp.check_output(['git','log','-1','--format=%H','--','crates','patches'],cwd=repo,text=True).strip()
manifest={'starting_head':'23075cd713884903a8985c88ab43bd1817477308','product_head':product,'runtime_build_head':'160b3c2b20b8eb1437756f9dd7f090b8acacc0f2','packaged_head':head,'factory_entries':len(list((root/'factory').rglob('sound.toml'))),'platform':'Linux x86_64','normal_factory':'factory','normal_user_library':'explicit KABL_USER_DIR or normal native/XDG discovery','portability_test':'separate launch-portability-test.sh','files':{str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(root.rglob('*')) if p.is_file()}}
manifest['mutable_profile_directories']=['profile','portability-test-profile']
manifest['files']={name:digest for name,digest in manifest['files'].items() if Path(name).parts[0] not in manifest['mutable_profile_directories']}
(root/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(root)
