#!/usr/bin/env python3
"""REAPER code-module benchmark: what a code module (wasm, run by wasmi) costs inside REAPER,
compared with a built-in module doing similar work.

    cargo build --release -p kabl-clap
    python3 docs/eng-v1/scripts/reaper_code_bench.py

One kabl CLAP instance on one track plays a held 8-note chord through

    midi.in -> osc.va -> X x N (in series) -> vca (env.adsr) -> out

with X = nothing (baseline), `filter.svf` (built-in) or `demo.bitfilter` (the example code
module, embedded in the plugin state the way a saved song carries it). REAPER renders the project
offline with its own renderer; offline rendering runs as fast as the CPU allows, so render time
is the plugin's CPU cost. Each variant runs KB_REPEATS times (default 3) in a fresh REAPER; the
fastest run counts. Cost per module = (variant - baseline) / N.

Before rendering, bench.lua waits for kabl's fuel calibration (KABL_CODE_CALIBRATION_REPORT) so
code modules run with their calibrated budget, not the start-up fallback. A code module that runs
out of budget outputs silent blocks (crates/modules/src/code/module.rs), so every render's
silent 256-frame blocks are counted: a non-zero count means overruns, not a valid timing.

Private Xvfb display, scratch REAPER profile in scratch/code-bench (the user's REAPER config is
never read), audio device on the silent `kabl_agents` sink.
"""
from pathlib import Path
import base64, hashlib, json, os, shutil, struct, subprocess as sp, textwrap, time
import numpy as np

repo = Path(__file__).resolve().parents[3]
S = repo / 'scratch/code-bench'
OUT = Path(os.environ.get('KB_OUT', repo / 'docs/eng-v1/evidence/reaper-code-bench'))
PLUGIN = Path(os.environ.get('KB_PLUGIN', repo / 'target/release/libkabl_clap.so'))
BLOB = repo / 'crates/modules/src/code/examples/demo_bitfilter.wasm'
LUA = repo / 'docs/eng-v1/scripts/bench.lua'
SECS = int(os.environ.get('KB_SECS', '30'))
REPEATS = int(os.environ.get('KB_REPEATS', '3'))
COUNTS = [int(n) for n in os.environ.get('KB_COUNTS', '1,2,3').split(',')]
KINDS = os.environ.get('KB_KINDS', 'filter.svf,demo.bitfilter').split(',')
CHORD = [48, 52, 55, 59, 60, 64, 67, 71]
BLOCK = 256


def patch(kind, n):
    mods = {1: 'midi.in', 2: 'osc.va', 3: 'env.adsr', 4: 'vca', 5: 'out'}
    chain = list(range(10, 10 + n))
    mods.update({i: kind for i in chain})
    cables = []
    m = lambda i, p: {'Module': {'id': i, 'port': p}}
    cables += [(m(1, 'pitch'), m(2, 'pitch')), (m(1, 'gate'), m(3, 'gate')), (m(3, 'out'), m(4, 'cv'))]
    out_port = {'filter.svf': 'lp', 'demo.bitfilter': 'out'}.get(kind)
    src = m(2, 'out')
    for i in chain:
        cables.append((src, m(i, 'in')))
        src = m(i, out_port)
    cables += [(src, m(4, 'in')), (m(4, 'out'), m(5, 'in'))]
    return {
        'modules': {str(i): {'kind': k, 'pos': {'x': 20.0 + 160 * (j % 8), 'y': 20.0 + 200 * (j // 8)}, 'params': {}}
                    for j, (i, k) in enumerate(mods.items())},
        'cables': {str(c + 1): {'from': a, 'to': b, 'params': {}, 'steps': []} for c, (a, b) in enumerate(cables)},
        'labels': {},
    }, chain


def code_hex(ids):
    """`Embedded::to_bytes` (crates/core/src/embedded.rs): frames of kind u8, len u32 LE, payload."""
    blob = BLOB.read_bytes()
    h = hashlib.sha256(blob).digest()
    kind = b'demo.bitfilter'
    frame = lambda k, p: bytes([k]) + struct.pack('<I', len(p)) + p
    out = frame(0, h + struct.pack('<I', 1) + struct.pack('<H', len(kind)) + kind + blob)
    out += b''.join(frame(1, struct.pack('<Q', i) + h) for i in ids)
    return out.hex()


def project(kind, n):
    p, chain = patch(kind, n)
    state = {'version': 6, 'patch': p, 'output_gain': 0.3, 'host_clock': False,
             'lanes': [{'retired': False, 'target': None} for _ in range(16)], 'slot_values': [0.0] * 16}
    if kind == 'demo.bitfilter':
        state['code'] = code_hex(chain)
    raw = json.dumps(state, separators=(',', ':')).encode()
    enc = base64.b64encode(len(raw).to_bytes(8, 'little') + raw).decode()
    ticks = SECS * 2 * 960  # 120 bpm: 2 quarter notes a second, 960 ticks each
    midi = ''.join(f'        E 0 90 {k:02x} 64\n' for k in CHORD)
    midi += f'        E {ticks} 80 {CHORD[0]:02x} 00\n' + ''.join(f'        E 0 80 {k:02x} 00\n' for k in CHORD[1:])
    return f'''<REAPER_PROJECT 0.1 "7.0/linux-x86_64" 0
  TEMPO 120 4 4
  SAMPLERATE 48000 1 0
  <TRACK
    NAME "kabl code bench"
    <FXCHAIN
      SHOW -1
      <CLAP "CLAPi: kabl (stcksmsh)" dev.stcksmsh.kabl ""
        CFG 4 1180 740 ""
        <STATE
''' + ''.join('          ' + l + '\n' for l in textwrap.wrap(enc, 120)) + f'''        >
      >
      WAK 0 0
    >
    <ITEM
      POSITION 0
      LENGTH {SECS}
      <SOURCE MIDI
        HASDATA 1 960 QN
{midi}      >
    >
  >
>
'''


def pcm(path):
    b = path.read_bytes(); at = 12; fmt = None
    while at < len(b):
        cid, size = b[at:at + 4], struct.unpack_from('<I', b, at + 4)[0]
        if cid == b'fmt ': fmt = struct.unpack_from('<HHIIHH', b, at + 8)
        if cid == b'data':
            assert fmt[0] in (3, 65534) and fmt[5] == 32, fmt
            return np.frombuffer(b[at + 8:at + 8 + size], '<f4').reshape(-1, fmt[1])
        at += 8 + size + (size & 1)


def audio(path):
    m = pcm(path).mean(1)
    body = m[4800:len(m) // BLOCK * BLOCK]  # skip the first 0.1 s (attack)
    blocks = np.sqrt((body[: len(body) // BLOCK * BLOCK].reshape(-1, BLOCK) ** 2).mean(1))
    return dict(finite=bool(np.isfinite(m).all()), rms=float(np.sqrt((m ** 2).mean())),
                silent_blocks=int((blocks < 1e-6).sum()))


def main():
    shutil.rmtree(S, ignore_errors=True)
    (S / 'plugin').mkdir(parents=True)
    (S / 'empty-vst').mkdir()
    OUT.mkdir(parents=True, exist_ok=True)
    shutil.copy2(PLUGIN, S / 'plugin/kabl.clap')
    ini = S / 'reaper.ini'
    ini.write_text(f'[reaper]\nvstpath={S}/empty-vst\nvstpath64={S}/empty-vst\nlinux_audio_bsize={BLOCK}\n'
                   'linux_audio_mode=0\nlinux_audio_srate=48000\nwnd_w=1280\nwnd_h=800\nwnd_x=0\nwnd_y=0\n')
    disp = ':151'
    env0 = dict(os.environ, PIPEWIRE_NODE='kabl_agents', PULSE_SINK='kabl_agents', DISPLAY=disp,
                XDG_RUNTIME_DIR='/run/user/1000', CLAP_PATH=str(S / 'plugin'), KB_DIR=str(S), KB_SECS=str(SECS),
                KABL_FACTORY_DIR=str(S / 'no-factory'), KABL_USER_DIR=str(S / 'no-library'))
    log = open(S / 'run.txt', 'w')
    x = sp.Popen(['Xvfb', disp, '-screen', '0', '1440x900x24', '-nolisten', 'tcp'], stdout=log, stderr=log)
    time.sleep(.8)
    wm = sp.Popen(['metacity', '--sm-disable'], env=env0, stdout=log, stderr=log)
    time.sleep(.8)
    variants = [('none', 0)] + [(k, n) for n in COUNTS for k in KINDS]
    runs = {}
    try:
        for rep in range(REPEATS):
            for kind, n in variants:
                tag = f'{kind}-{n}-r{rep}'
                rpp = S / f'{kind}-{n}.rpp'
                rpp.write_text(project(kind, n))
                report = S / f'calibration-{tag}.txt'
                env = dict(env0, KB_TAG=tag, KABL_CODE_CALIBRATION_REPORT=str(report))
                app = sp.Popen(['pw-jack', '/usr/local/bin/reaper', '-newinst', '-nosplash', '-cfgfile', str(ini),
                                str(rpp), str(LUA)], env=env, cwd=S, stdout=log, stderr=log)
                done = S / f'done-{tag}'
                end = time.monotonic() + 300
                while time.monotonic() < end and not done.exists() and app.poll() is None:
                    time.sleep(.3)
                assert done.exists(), f'{tag}: REAPER did not finish; see {S}/run.txt'
                app.terminate(); app.wait(timeout=15)
                info = dict(l.split('=', 1) for l in done.read_text().split('\n') if '=' in l)
                r = dict(render_s=float(info['render_s']), calibrated=info['calibrated'] == 'true',
                         bsize=info['bsize'], reaper=info['reaper'], **audio(S / f'render-{tag}.wav'))
                if report.exists():
                    r['calibration'] = report.read_text()
                runs.setdefault(f'{kind}-{n}', []).append(r)
                print(tag, {k: v for k, v in r.items() if k != 'calibration'}, flush=True)
                (S / f'render-{tag}.wav').unlink()
    finally:
        for pr in (wm, x):
            pr.terminate(); pr.wait(timeout=8)
        log.close()
    base = min(r['render_s'] for r in runs['none-0'])
    summary = {}
    for key, rs in runs.items():
        kind, n = key.rsplit('-', 1); n = int(n)
        best = min(r['render_s'] for r in rs)
        row = dict(render_s=round(best, 3), realtime_factor=round(SECS / best, 1),
                   silent_blocks=max(r['silent_blocks'] for r in rs), calibrated=all(r['calibrated'] for r in rs))
        if row['silent_blocks']:
            row['overrun'] = True  # modules went silent: the time measures nothing, quote no cost
        elif n:
            per = (best - base) / n
            row['per_module_ms_per_audio_s'] = round(per / SECS * 1e3, 3)
            bsize = int(rs[0]['bsize'])  # REAPER's device block, the period kabl budgets code by
            row['per_module_us_per_block'] = round(per / (SECS * 48000 / bsize) * 1e6, 2)
            row['block'] = bsize
        summary[key] = row
    cal = next((r['calibration'] for rs in runs.values() for r in rs if 'calibration' in r), None)
    result = dict(plugin_sha256=hashlib.sha256(PLUGIN.read_bytes()).hexdigest(), cpu=cpu(), secs=SECS,
                  repeats=REPEATS, chord=CHORD, block=BLOCK, summary=summary, calibration=cal, runs=runs)
    (OUT / 'result.json').write_text(json.dumps(result, indent=1) + '\n')
    print(json.dumps(summary, indent=1))


def cpu():
    for l in Path('/proc/cpuinfo').read_text().splitlines():
        if l.startswith('model name'):
            return l.split(':', 1)[1].strip()


if __name__ == '__main__':
    main()
