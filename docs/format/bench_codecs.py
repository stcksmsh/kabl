#!/usr/bin/env python3
"""Research benchmark for the single-file .kabl format. Not part of the build.

Usage: bench_codecs.py <repo-root>
Needs: pip install zstandard brotli cbor2 msgpack
"""
import sys, os, json, time, lzma, zlib, glob
import zstandard as zstd, brotli, cbor2, msgpack

root = sys.argv[1]
pats = sorted({os.path.dirname(p) for p in glob.glob(root + '/**/log.jsonl', recursive=True)
               if '/target/' not in p and os.path.exists(os.path.join(os.path.dirname(p), 'meta.toml'))})
rd = lambda p, f: open(os.path.join(p, f), 'rb').read() if os.path.exists(os.path.join(p, f)) else b''

def tablebytes(log):
    return sum(len(l) + 1 for l in log.splitlines() if b'"SetTable"' in l)

logs = [rd(p, 'log.jsonl') for p in pats]
cps = [rd(p, 'checkpoint.json') for p in pats]
tabs = [tablebytes(l) for l in logs]
print(f'patches {len(pats)}  log {sum(map(len,logs))}  checkpoint {sum(map(len,cps))}  SetTable-in-log {sum(tabs)}')
print(f'patches with tables: {sum(1 for t in tabs if t)}  median log {sorted(map(len,logs))[len(logs)//2]}  max {max(map(len,logs))}')
print('ops per log: median', sorted(l.count(b"\n") for l in logs)[len(logs)//2], 'max', max(l.count(b"\n") for l in logs))

def parse(l): return [json.loads(x) for x in l.splitlines()]
parsed = [parse(l) for l in logs]
def lean(entries):  # drop inverse (derivable by replay)
    out = []
    for e in entries:
        e = dict(e); e.pop('inverse', None); out.append(e)
    return out
variants = {
    'json': lambda ents, raw: raw,
    'json-noinverse': lambda ents, raw: b'\n'.join(json.dumps(e, separators=(',', ':')).encode() for e in lean(ents)),
    'cbor': lambda ents, raw: cbor2.dumps(ents),
    'msgpack': lambda ents, raw: msgpack.packb(ents),
    'cbor-noinverse': lambda ents, raw: cbor2.dumps(lean(ents)),
}
Z = zstd.ZstdCompressor
D = zstd.ZstdDecompressor
codecs = {
    'deflate9': (lambda b: zlib.compress(b, 9), zlib.decompress),
    'zstd3': (lambda b: Z(level=3).compress(b), lambda b: D().decompress(b)),
    'zstd19': (lambda b: Z(level=19).compress(b), lambda b: D().decompress(b)),
    'zstd22long': (lambda b: Z(compression_params=zstd.ZstdCompressionParameters.from_level(22, enable_ldm=True, window_log=27)).compress(b),
                   lambda b: D(max_window_size=1 << 27).decompress(b)),
    'xz9e': (lambda b: lzma.compress(b, preset=9 | lzma.PRESET_EXTREME), lzma.decompress),
    'brotli11': (lambda b: brotli.compress(b, quality=11, lgwin=24), brotli.decompress),
}
# dictionary trained on even patches, evaluated on odd (held-out) patches
samples = []
for i, l in enumerate(logs):
    if i % 2 == 0:
        ls = l.splitlines()
        samples += [b'\n'.join(ls[j:j + 20]) for j in range(0, len(ls), 20)]
dic = zstd.train_dictionary(112 * 1024, samples)
print('dict size', len(dic.as_bytes()))
codecs['zstd19+dict(112k)'] = (lambda b: Z(level=19, dict_data=dic).compress(b),
                               lambda b: D(dict_data=dic).decompress(b))
heldout = [i for i in range(len(pats)) if i % 2 == 1]

def run(vn, cn, idx):
    enc, dec = codecs[cn]; raw = comp = 0; te = td = 0.0
    for i in idx:
        b = variants[vn](parsed[i], logs[i]); raw += len(b)
        t = time.perf_counter(); c = enc(b); te += time.perf_counter() - t
        t = time.perf_counter(); dec(c); td += time.perf_counter() - t
        comp += len(c)
    return raw, comp, te, td

hdr = f'{"variant":16}{"codec":20}{"raw":>9}{"comp":>9}{"ratio":>8}{"enc ms/patch":>14}{"dec ms/patch":>14}'
print('\n== whole corpus, per-patch compression ==\n' + hdr)
for vn in variants:
    for cn in codecs:
        if '+dict' in cn: continue
        r, c, te, td = run(vn, cn, range(len(pats)))
        print(f'{vn:16}{cn:20}{r:9}{c:9}{r/c:8.1f}{1000*te/len(pats):14.2f}{1000*td/len(pats):14.2f}')
print('\n== held-out (odd) patches only, dictionary trained on even ==\n' + hdr)
for vn in ('json', 'json-noinverse', 'cbor-noinverse'):
    for cn in ('zstd19', 'zstd19+dict(112k)', 'brotli11'):
        r, c, te, td = run(vn, cn, heldout)
        print(f'{vn:16}{cn:20}{r:9}{c:9}{r/c:8.1f}{1000*te/len(heldout):14.2f}{1000*td/len(heldout):14.2f}')

print('\n== multi-frame (one zstd-19 frame per save) vs single frame, json-noinverse ==')
for K in (1, 10, 50):
    single = multi = 0
    for l in logs:
        lines = [json.dumps(e, separators=(',', ':')).encode() for e in lean(parse(l))]
        single += len(Z(level=19).compress(b'\n'.join(lines)))
        multi += sum(len(Z(level=19).compress(b'\n'.join(lines[i:i + K]))) for i in range(0, len(lines), K))
    print(f'ops/save {K:3}: single {single}  multi {multi}  overhead x{multi / single:.2f}')

print('\n== checkpoint.json on top of log (zstd19) ==')
cl = sum(len(Z(level=19).compress(l)) for l in logs)
cc = sum(len(Z(level=19).compress(c)) for c in cps)
cb = sum(len(Z(level=19).compress(l + c)) for l, c in zip(logs, cps))
print(f'log {cl}  checkpoint {cc}  together {cb}')
