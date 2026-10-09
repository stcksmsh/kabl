#!/usr/bin/env python3
"""Refresh this batch's own new bundle, preserving its initial packaging identity."""
from pathlib import Path
import sys, json, hashlib, shutil, subprocess as sp

repo=Path(__file__).resolve().parents[3]
bundle=Path(sys.argv[1]).resolve()
manifest=json.loads((bundle/'manifest.json').read_text())
assert bundle.name=='factory-bank-owner-test'
assert manifest['starting_head']=='23075cd713884903a8985c88ab43bd1817477308'
for name,digest in manifest['files'].items():
    if Path(name).parts[0] in ['profile','portability-test-profile']: continue
    assert hashlib.sha256((bundle/name).read_bytes()).hexdigest()==digest,name
head=sp.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip()
product=sp.check_output(['git','log','-1','--format=%H','--','crates','patches'],cwd=repo,text=True).strip()
shutil.copytree(repo/'patches',bundle/'factory',dirs_exist_ok=True)
shutil.copytree(repo/'docs/factory-performance-bank',bundle/'docs/factory-performance-bank',dirs_exist_ok=True)
shutil.copytree(repo/'docs/factory-performance-bank/audio',bundle/'audio',dirs_exist_ok=True)
shutil.copytree(repo/'docs/factory-performance-bank/performance-project',bundle/'patches/performance',dirs_exist_ok=True)
manifest.setdefault('initial_packaged_head',manifest['packaged_head'])
manifest.update(packaged_head=head,product_head=product,refresh='This batch-owned new bundle only; binaries unchanged, final docs/metadata copied from declared head.')
if len(sys.argv)>2:
    submission={'starting_head':manifest['starting_head'],'submitted_head':head,'tested_head':product,'reviewed_content_head':product,'runtime_build_head':manifest['runtime_build_head'],'packaged_head':head,'initial_packaged_head':manifest['initial_packaged_head'],'draft_pr':sys.argv[2],'merged':False,'owner_approved':False,'normal_launcher':str(bundle/'launch.sh'),'limits':'Owner listening/controller/physical latency/xruns/beta pending; ordinary render/warm-up/dense-editor/strict-profile limits inherited.'}
    (bundle/'submission.json').write_text(json.dumps(submission,indent=2)+'\n')
manifest['mutable_profile_directories']=['profile','portability-test-profile']
manifest['files']={str(p.relative_to(bundle)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(bundle.rglob('*')) if p.is_file() and p.name!='manifest.json' and p.relative_to(bundle).parts[0] not in manifest['mutable_profile_directories']}
(bundle/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps({'packaged_head':head,'product_head':product,'files':len(manifest['files'])},indent=2))
