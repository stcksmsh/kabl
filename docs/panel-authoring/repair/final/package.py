#!/usr/bin/env python3
"""Create a new final bundle. Old bundles, owner patches and libraries remain untouched."""
from pathlib import Path
import sys,runpy,json,hashlib
repo=Path(__file__).resolve().parents[4];root=Path(sys.argv[1]).resolve()
runpy.run_path(str(repo/'docs/panel-authoring/repair/make-bundle.py'),run_name='__main__')
common='#!/bin/sh\nset -eu\nbundle_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)\nexport KABL_FACTORY_DIR="$bundle_dir/factory"\nif [ -z "${KABL_USER_DIR+x}" ]; then\n'
for prior in ['d10-repair-owner-test','d10-owner-test']:
    library=root.parent/prior/'user-library'
    common+=f'    if [ -z "${{KABL_USER_DIR+x}}" ] && {{ [ -d "{library}/sounds" ] || [ -d "{library}/composites" ]; }}; then export KABL_USER_DIR="{library}"; fi\n'
common+='fi\n'
for name in ['launch.sh','launch-reaper.sh']:
    p=root/name;s=p.read_text();p.write_text(common+s[s.index('export CLAP_PATH=') if name=='launch-reaper.sh' else s.index('exec "$bundle_dir/bin/'):])
p=root/'README.md';p.write_text(p.read_text()+"\nThis final repair grows More with exact-entry sections, gives wheel gestures one owner, and paints insertion above stationary panels/cables and below the lift. See docs/repair/final/README.md and submission.json for current evidence and exact heads. Normal library discovery also preserves resources from the prior repaired bundle when present.\n")
(root/'profile/empty-vst').mkdir(exist_ok=True)
p=root/'profile/reaper.ini';s='\n'.join(line for line in p.read_text().splitlines() if not line.startswith(('vstpath=', 'vstpath64=')))+'\n';s=s.replace('[reaper]\n',f'[reaper]\nvstpath={root}/profile/empty-vst\nvstpath64={root}/profile/empty-vst\n',1);p.write_text(s)
p=root/'manifest.json';m=json.loads(p.read_text());m['product_head']=(repo/'docs/panel-authoring/repair/final/evidence/product-head.txt').read_text().strip();m['files']={str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(root.rglob('*')) if p.is_file() and p.name!='manifest.json'};p.write_text(json.dumps(m,indent=2)+'\n')
