#!/usr/bin/env python3
"""Compare a CLAP validator JSON against the recorded baseline: python3 validator_diff.py NEW.json BASELINE.json"""
import collections, json, sys

new, base = (json.load(open(p))['results'] for p in sys.argv[1:3])
count = lambda rs: dict(collections.Counter(r['status']['code'] for r in rs))
key = lambda r: json.dumps(r['test'], sort_keys=True).replace('target/release/libkabl_clap.so', 'P')
print('new     ', count(new))
print('baseline', count(base))
bs = {key(r): r['status']['code'] for r in base}
ns = {key(r): r['status']['code'] for r in new}
print('tests only in new     :', [k for k in ns if k not in bs])
print('tests only in baseline:', [k for k in bs if k not in ns])
print('status changed        :', [(k, bs[k], ns[k]) for k in ns if k in bs and ns[k] != bs[k]])
print('failures in new       :', [r for r in new if r['status']['code'] not in ('success', 'skipped')])
