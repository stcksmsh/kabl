#!/usr/bin/env python3
"""Record exact checked source and binary identities from retained evidence."""
from pathlib import Path
import hashlib, json, re, subprocess as sp
from collections import Counter

repo=Path(__file__).resolve().parents[3]
e=repo/'docs/factory-performance-bank/evidence'
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
counts=[tuple(map(int,x)) for x in re.findall(r'test result: ok. (\d+) passed; (\d+) failed; (\d+) ignored',(e/'workspace-final.txt').read_text())]
totals=list(map(sum,zip(*counts)))
assert totals==[653,0,24],totals
validator=Counter(r['status']['code'] for r in json.loads((e/'validator.json').read_text())['results'])
assert validator=={'success':35,'skipped':9},validator
metrics=json.loads((e/'audio/metrics.json').read_text())
assert len(metrics)==12
assert all(v['peak_dbfs']<0 and v['active_seconds_above_minus50_dbfs']>0 for entry in metrics.values() for v in entry.values())
assert len(json.loads((e/'gui.json').read_text())['cases'])==24
assert json.loads((e/'host/recall.json').read_text())['complete_f32_state_equal']
receipt={'starting_head':'23075cd713884903a8985c88ab43bd1817477308','runtime_build_head':'160b3c2b20b8eb1437756f9dd7f090b8acacc0f2','tested_content_head':'f5dd27af82f08474700a7c45a5a3ef033a13003c','evidence_checkpoint_head':sp.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip(),'runtime_source_unchanged_after_build':not sp.check_output(['git','diff','160b3c2b20b8eb1437756f9dd7f090b8acacc0f2','--','crates/ui/src','crates/clap/src','crates/engine/src','crates/core/src','crates/dsp/src','vendor','Cargo.toml','Cargo.lock'],cwd=repo),'binary_sha256':{str(p.relative_to(repo)):sha(p) for p in [repo/'target/release/kabl-ui',repo/'target/release/libkabl_clap.so']},'workspace':dict(zip(['passed','failed','ignored'],totals)),'validator':dict(validator),'clippy':'workspace/all-target -D warnings passed','release':'standalone and CLAP passed at runtime_build_head','curated_entries':12,'gui_browser_loads':24,'audio':'12 normal + 12 all-macros-one, 24 seconds each; one editable 72-second performance','evidence_sha256':{str(p.relative_to(e)):sha(p) for p in sorted(e.rglob('*')) if p.is_file() and p.name!='verification.json'},'owner_acceptance':'listening, physical controller, physical latency/xruns and production-beta pending','inherited_limits':'ordinary-render nondeterminism, warm-up policy, dense-editor/backend limits and strict-profile assumptions unchanged'}
receipt['tested_content_head']=sp.check_output(['git','log','-1','--format=%H','--','crates','patches'],cwd=repo,text=True).strip()
assert receipt['runtime_source_unchanged_after_build']
(e/'verification.json').write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps({'source':receipt['tested_content_head'],'workspace':totals,'validator':dict(validator),'binary_sha256':receipt['binary_sha256']},indent=2))
