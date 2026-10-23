#!/usr/bin/env python3
"""Real-REAPER check of eight macros and of a macro scaling a route's depth, in the CLAP plugin.

    cargo build --release -p kabl-clap
    python3 docs/macros/scripts/host.py

Same method as docs/functional-cables/scripts/host_routes.py: a private Xvfb display and REAPER
profile, a two-track project whose <STATE> chunks this script writes, CLAP_PATH pointing at a copy
of the plugin, the factory and user library made unavailable; host.lua saves the project and
renders the master mix with the host's own renderer; REAPER is then quit and started again on the
project it saved.

Track 1: patches/macros-eight. Host automation lane 1 is the FIFTH macro (m5, Delay), and the
project holds a recorded envelope on that lane (0 until 4 s, up to 1 at 12 s, hold to 18 s, down to
0 at 23 s). The patch also holds a route into the amount of an LFO's route into the cutoff, which
must survive the save.
Track 2: patches/cue-macros (an older patch with three named macros).

Phases: new-initial / new-restart (saved, quit, reopened, saved, rendered), still (envelope removed:
its render must differ), old-new (a project saved by the previous build opens and saves unchanged).
"""
from pathlib import Path
import base64, hashlib, json, os, re, shutil, struct, subprocess as sp, textwrap, time, uuid
import numpy as np

repo = Path(__file__).resolve().parents[3]
S = repo / 'scratch/macros-host'
OUT = repo / 'docs/macros/evidence/host'
PLUGIN = Path(os.environ.get('FC_PLUGIN', repo / 'target/release/libkabl_clap.so'))
OLD_PROJECT = repo / 'docs/factory-performance-bank/evidence/host/arrangement.rpp'
SECS = 24
shutil.rmtree(S, ignore_errors=True)
S.mkdir(parents=True); OUT.mkdir(parents=True, exist_ok=True)


def f32(v):
    if isinstance(v, float): return struct.unpack('<f', struct.pack('<f', v))[0]
    if isinstance(v, list): return [f32(x) for x in v]
    if isinstance(v, dict): return {k: f32(x) for k, x in v.items()}
    return v


def states_of(text):
    out = []
    for chunk in re.findall(r'<STATE\s+(.*?)\s*>', text, re.S):
        raw = base64.b64decode(''.join(chunk.split()))
        assert struct.unpack_from('<Q', raw)[0] == len(raw) - 8
        out.append(json.loads(raw[8:]))
    return out


def chunk_of(state):
    raw = json.dumps(state, separators=(',', ':')).encode()
    enc = base64.b64encode(len(raw).to_bytes(8, 'little') + raw).decode()
    return '<STATE\n' + ''.join('          ' + l + '\n' for l in textwrap.wrap(enc, 120)) + '        >'


def state_for(patch, macro=None, param=None):
    lanes = [{'retired': False, 'target': None} for _ in range(16)]
    if macro is not None:
        lanes[0]['target'] = {'module': macro, 'kind': 'macro', 'param': param}
    return {'version': 2, 'patch': patch, 'output_gain': .4, 'host_clock': False,
            'lanes': lanes, 'slot_values': [0.0] * 16}


def patch_of(name):
    return json.loads((repo / f'patches/{name}/checkpoint.json').read_text())


tpl = OLD_PROJECT.read_text()
proj = re.sub(r'\n      <PARMENV[\s\S]*?\n      >', '', tpl)
eight = patch_of('macros-eight')
macro_id = int(next(i for i, m in eight['modules'].items() if m['kind'] == 'macro'))
depth = {cid: c for cid, c in eight['cables'].items() if 'CableParam' in c['to'] and c['to']['CableParam']['param'] == 'amount'}
assert len(depth) == 1, 'one route into a route amount'
expected = [state_for(eight, macro_id, 'm5'), state_for(patch_of('cue-macros'))]
it = iter(expected)
proj = re.sub(r'<STATE\s+.*?\s*>', lambda m: chunk_of(next(it)), proj, flags=re.S)
item = re.search(r'\n    <ITEM[\s\S]*?\n    >(?=\n  >)', proj).group(0)
item2 = re.sub(r'\{[0-9A-Fa-f-]{36}\}', lambda m: '{' + str(uuid.uuid4()).upper() + '}', item)
first_end = proj.index('\n  <TRACK', proj.index('<TRACK') + 1)
second_end = proj.rindex('\n  >\n>')
assert '<ITEM' not in proj[first_end:second_end]
proj = proj[:second_end] + item2 + proj[second_end:]
proj = proj.replace('Glass Keys - virtual/scripted MIDI', 'Eight Macros (fifth macro automated)').replace('Slow Horizons - editable real sequencer', 'Cue Macros')
assert proj.count('<ITEM') == 2 and len(states_of(proj)) == 2
ENV = [(0, 0.0), (4, 0.0), (12, 1.0), (18, 1.0), (23, 0.0)]
parmenv = ('\n      <PARMENV 1:1248030256 0 1 0.5 "Slot 1 / kabl / Automation 1"\n'
           '        EGUID {5A1C2E70-7B3D-4C59-9E21-0D4F6A8B1C33}\n        ACT 1 -1\n        VIS 1 1 1\n'
           '        LANEHEIGHT 0 0\n        ARM 1\n        DEFSHAPE 0 -1 -1\n'
           + ''.join(f'        PT {t} {v} 0\n' for t, v in ENV) + '      >')
fxid = re.search(r'\n      FXID \{[0-9A-Fa-f-]{36}\}', proj)
still_proj = proj
proj = proj[:fxid.end()] + parmenv + proj[fxid.end():]
assert proj.count('<PARMENV') == 1
(S / 'new.rpp').write_text(proj)
(S / 'still.rpp').write_text(still_proj)
(S / 'expected.json').write_text(json.dumps(expected, indent=1))

DISP = ':173'
env0 = dict(os.environ, DISPLAY=DISP, XDG_RUNTIME_DIR='/run/user/1000', KABL_FACTORY_DIR=str(S / 'unavailable-factory'),
            KABL_USER_DIR=str(S / 'unavailable-library'), KABL_SE_HOST=str(S), KABL_SE_SECS=str(SECS))
log = open(S / 'run.txt', 'w')
x = sp.Popen(['Xvfb', DISP, '-screen', '0', '1600x1000x24', '-nolisten', 'tcp'], stdout=log, stderr=log); time.sleep(.8)
wm = sp.Popen(['metacity', '--sm-disable'], env=env0, stdout=log, stderr=log); time.sleep(.8)


def profile():
    d = S / 'profile'; d.mkdir(exist_ok=True); (S / 'empty-vst').mkdir(exist_ok=True)
    (d / 'reaper.ini').write_text(f'[reaper]\nvstpath={S}/empty-vst\nvstpath64={S}/empty-vst\nlinux_audio_bsize=256\nlinux_audio_mode=0\nlinux_audio_srate=48000\nwnd_w=1280\nwnd_h=800\nwnd_x=0\nwnd_y=0\n')
    return d / 'reaper.ini'


def run(label, project):
    pdir = S / 'plugin'; pdir.mkdir(exist_ok=True)
    shutil.copy2(PLUGIN, pdir / 'kabl.clap')
    env = dict(env0, CLAP_PATH=str(pdir), KABL_SE_TAG=label)
    app = sp.Popen(['pw-jack', '/usr/local/bin/reaper', '-newinst', '-nosplash', '-cfgfile', str(profile()),
                    str(project), str(Path(__file__).with_name('host.lua'))], env=env, cwd=S, stdout=log, stderr=log)
    end = time.monotonic() + 120
    while time.monotonic() < end and not (S / f'done-{label}').exists() and app.poll() is None: time.sleep(.3)
    assert (S / f'done-{label}').exists(), f'{label}: host did not finish; see run.txt'
    maps = [l for l in Path(f'/proc/{app.pid}/maps').read_text().splitlines() if 'kabl.clap' in l]
    assert maps, 'plugin not mapped'
    (OUT / f'{label}-mapped.txt').write_text('\n'.join(maps) + '\n')
    app.terminate(); app.wait(timeout=15)
    shutil.copy2(S / f'saved-{label}.rpp', OUT / f'{label}.rpp')
    shutil.copy2(S / f'info-{label}.txt', OUT / f'{label}-info.txt')


def pcm(path):
    b = path.read_bytes(); at = 12; fmt = None
    while at < len(b):
        cid, size = b[at:at + 4], struct.unpack_from('<I', b, at + 4)[0]
        if cid == b'fmt ': fmt = struct.unpack_from('<HHIIHH', b, at + 8)
        if cid == b'data':
            assert fmt[0] in (3, 65534) and fmt[5] == 32, fmt
            return np.frombuffer(b[at + 8:at + 8 + size], '<f4').reshape(-1, fmt[1])
        at += 8 + size + (size & 1)


def feats(path):
    p = pcm(path); m = p.mean(1); n = len(m)
    sec = np.sqrt((m[:n // 48000 * 48000].reshape(-1, 48000) ** 2).mean(1))
    sp_ = np.abs(np.fft.rfft(m[48000:48000 * 21] * np.hanning(48000 * 20)))
    return dict(frames=n, finite=bool(np.isfinite(p).all()), peak=float(np.abs(p).max()), rms=float(np.sqrt((m ** 2).mean())),
                dc=float(m.mean()), active_seconds=int((sec > 1e-3).sum()), per_second_rms=[round(float(v), 4) for v in sec],
                _sec=sec, _spec=sp_, _m=m)


def compare(a, b):
    n = min(len(a['_m']), len(b['_m']))
    diff = float(np.sqrt(((a['_m'][:n] - b['_m'][:n]) ** 2).mean()) / max(a['rms'], 1e-9))
    spec = float(np.dot(a['_spec'], b['_spec']) / (np.linalg.norm(a['_spec']) * np.linalg.norm(b['_spec'])))
    env = float(np.corrcoef(a['_sec'][:24], b['_sec'][:24])[0, 1])
    return dict(relative_difference_rms=round(diff, 4), spectrum_cosine=round(spec, 5), envelope_correlation=round(env, 5))


def clean(f): return {k: v for k, v in f.items() if not k.startswith('_')}


result = {'plugin_sha256': hashlib.sha256(PLUGIN.read_bytes()).hexdigest()}
try:
    run('new-initial', S / 'new.rpp')
    run('new-restart', S / 'saved-new-initial.rpp')
    s0, s1, s2 = (states_of(p.read_text()) for p in (S / 'new.rpp', S / 'saved-new-initial.rpp', S / 'saved-new-restart.rpp'))
    assert len(s0) == len(s1) == len(s2) == 2
    assert f32(s0) == f32(s1), 'state changed on first save'
    assert f32(s1) == f32(s2), 'state changed on reopen and save'
    result['states_identical_across_save_quit_reopen'] = True
    saved = s2[0]
    result['lane1_target'] = saved['lanes'][0]['target']
    assert result['lane1_target']['param'] == 'm5'
    result['depth_routes_in_saved_project'] = {cid: c['to'] for cid, c in saved['patch']['cables'].items()
                                               if 'CableParam' in c['to'] and c['to']['CableParam']['param'] == 'amount'}
    assert len(result['depth_routes_in_saved_project']) == 1
    f_i, f_r = feats(S / 'render-new-initial.wav'), feats(S / 'render-new-restart.wav')
    result['new_initial'], result['new_restart'] = clean(f_i), clean(f_r)
    result['new_initial_vs_restart'] = compare(f_i, f_r)
    run('still', S / 'still.rpp')
    result['automated_vs_still'] = compare(f_i, feats(S / 'render-still.wav'))
    assert result['automated_vs_still']['relative_difference_rms'] > 0.05, 'host automation of the fifth macro does not change the sound'
    run('old-new', OLD_PROJECT)
    o_orig, o_new = states_of(OLD_PROJECT.read_text()), states_of((S / 'saved-old-new.rpp').read_text())
    result['old_project_versions'] = [s['version'] for s in o_orig]
    result['old_state_identical_new_plugin'] = f32(o_orig) == f32(o_new)
    assert result['old_state_identical_new_plugin'], 'old state changed by the new plugin'
finally:
    for pr in (wm, x):
        pr.terminate(); pr.wait(timeout=8)
    log.close(); shutil.copy2(S / 'run.txt', OUT / 'run.txt')
    (OUT / 'result.json').write_text(json.dumps(result, indent=1) + '\n')
print(json.dumps(result, indent=1))
