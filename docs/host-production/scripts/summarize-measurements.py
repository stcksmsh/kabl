#!/usr/bin/env python3
"""Separate execution histograms, lifecycle arrival gaps and observed backend errors."""
import json
from pathlib import Path
repo = Path(__file__).resolve().parents[3]
results = []
for entry in json.loads((repo/'docs/host-production/evidence/host-measurements.json').read_text()):
    active=[]; inactive=[]
    for line in entry['callback_csv'].splitlines():
        row=list(map(int,line.split(',')))
        target=active if row[1]>1000 else inactive
        target.append({'instance':row[0],'callbacks':row[1],'mean_us':round(row[2]/row[1]/1000,2) if row[1] else 0,'max_us':round(row[3]/1000,2),'p99_upper_us':row[4]/1000,'arrival_max_ms':row[10]/1e6})
    nodes={}
    for line in (Path(entry['raw_dir'])/'backend-pw-top.txt').read_text().splitlines():
        fields=line.split()
        if len(fields)>9 and fields[-1]=='REAPER' and fields[1].isdigit() and fields[8].isdigit():
            nodes[fields[1]]=max(nodes.get(fields[1],0),int(fields[8]))
    results.append({'case':entry['case'],'product_head':entry['product_head'],'binary_sha256':entry['binary_sha256'],'active_instances':active,'startup_teardown_instances':inactive,'observed_reaper_nodes_error_max':nodes,'task_reaper_node_error_max':max(nodes.values(),default=0),'period_us':256/48000*1e6,'arrival_scope':'whole callback lifetime including startup/teardown; not callback execution or physical latency'})
(repo/'docs/host-production/evidence/timing-summary.json').write_text(json.dumps(results,indent=2)+'\n')
for result in results: print(result['case'],[x['p99_upper_us'] for x in result['active_instances']],result['task_reaper_node_error_max'])
