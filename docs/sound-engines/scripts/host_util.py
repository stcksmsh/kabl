#!/usr/bin/env python3
"""Real-REAPER check of the utility modules in the CLAP plugin.

    cargo build --release -p kabl-clap
    SE_BASELINE_PLUGIN=/path/to/libkabl_clap.so-built-from-origin/master \
        python3 docs/sound-engines/scripts/host_util.py

Same method as host.py (private Xvfb display and REAPER profile, project <STATE> chunks written by
this script, factory and user library made unavailable; host.lua saves the project and renders the
master mix, then REAPER is quit completely and started again on what it saved), with three
self-playing tracks: scale-walk (random, quantizer), clock-logic (clock.div, logic, comparator,
random, quantizer, attenuverter) and stepped-sweep (sample.hold, slew, attenuverter, crossfade,
pan). Also an old project (saved by the factory-bank build) through this build and the baseline
build, to show the older sounds are unchanged by the new modules.
"""
from pathlib import Path
import base64, hashlib, json, os, re, shutil, struct, subprocess as sp, textwrap, time, uuid
import numpy as np

repo = Path(__file__).resolve().parents[3]
S = repo / 'scratch/sound-engines-host-3'
OUT = repo / 'docs/sound-engines/evidence/host-util'
NEW = Path(os.environ.get('SE_PLUGIN', repo / 'target/release/libkabl_clap.so'))
BASE = Path(os.environ['SE_BASELINE_PLUGIN'])
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


def demo(name):
    return json.loads((repo / f'patches/sound-engines/demos/{name}/checkpoint.json').read_text())


def state_for(patch):
    return {'version': 2, 'patch': patch, 'output_gain': .4, 'host_clock': False,
            'lanes': [{'retired': False, 'target': None} for _ in range(16)], 'slot_values': [0.0] * 16}


names = ['scale-walk', 'clock-logic', 'stepped-sweep']
patches = [demo(n) for n in names]
expected = [state_for(p) for p in patches]
want_kinds = [{'random', 'quantizer'}, {'clock.div', 'logic', 'comparator', 'random', 'quantizer', 'attenuverter'},
              {'sample.hold', 'slew', 'attenuverter', 'crossfade', 'pan'}]
silent = demo('iron-bell')  # no utility modules

# ---- the project: the old two-track project, a third track copied from the second -------------
proj = re.sub(r'\n      <PARMENV[\s\S]*?\n      >', '', OLD_PROJECT.read_text())
item = re.search(r'\n    <ITEM[\s\S]*?\n    >(?=\n  >)', proj).group(0)
fresh = lambda m: '{' + str(uuid.uuid4()).upper() + '}'
guid = r'\{[0-9A-Fa-f-]{36}\}'
first_end = proj.index('\n  <TRACK', proj.index('<TRACK') + 1)
last = proj.rindex('\n  >\n>')
assert '<ITEM' not in proj[first_end:last]
track2 = proj[first_end:last]
proj = proj[:last] + re.sub(guid, fresh, item) + '\n  >' + re.sub(guid, fresh, track2) + re.sub(guid, fresh, item) + proj[last:]
track_names = iter(['Scale Walk', 'Clock Logic', 'Stepped Sweep'])
proj = re.sub(r'(<TRACK \{[^\n]*\n\s+NAME )"[^"\n]*"', lambda m: m.group(1) + '"' + next(track_names) + '"', proj)
assert proj.count('<ITEM') == 3 and proj.count('<TRACK') == 3 and len(states_of(proj)) == 3
it = iter(expected)
base_proj = proj
proj = re.sub(r'<STATE\s+.*?\s*>', lambda m: chunk_of(next(it)), base_proj, flags=re.S)
(S / 'new.rpp').write_text(proj)
# The same project with the three states replaced by one without utilities: the sound must change.
it2 = iter([state_for(silent)] * 3)
(S / 'no-util.rpp').write_text(re.sub(r'<STATE\s+.*?\s*>', lambda m: chunk_of(next(it2)), base_proj, flags=re.S))
(S / 'expected.json').write_text(json.dumps(expected, indent=1))

# ---- host ---------------------------------------------------------------------------------------
DISP = ':149'
env0 = dict(os.environ, DISPLAY=DISP, XDG_RUNTIME_DIR='/run/user/1000', KABL_FACTORY_DIR=str(S / 'unavailable-factory'),
            KABL_USER_DIR=str(S / 'unavailable-library'), KABL_SE_HOST=str(S), KABL_SE_SECS=str(SECS))
log = open(S / 'run.txt', 'w')
x = sp.Popen(['Xvfb', DISP, '-screen', '0', '1440x900x24', '-nolisten', 'tcp'], stdout=log, stderr=log); time.sleep(.8)
wm = sp.Popen(['metacity', '--sm-disable'], env=env0, stdout=log, stderr=log); time.sleep(.8)


def profile(name):
    d = S / f'profile-{name}'; d.mkdir(exist_ok=True); (S / 'empty-vst').mkdir(exist_ok=True)
    (d / 'reaper.ini').write_text(f'[reaper]\nvstpath={S}/empty-vst\nvstpath64={S}/empty-vst\nlinux_audio_bsize=256\nlinux_audio_mode=0\nlinux_audio_srate=48000\nwnd_w=1280\nwnd_h=800\nwnd_x=0\nwnd_y=0\n')
    return d / 'reaper.ini'


def run(plugin, label, project):
    key = 'base' if plugin == BASE else 'new'
    pdir = S / f'plugins-{key}'; pdir.mkdir(exist_ok=True)
    shutil.copy2(plugin, pdir / 'kabl.clap')
    env = dict(env0, CLAP_PATH=str(pdir), KABL_SE_TAG=label)
    app = sp.Popen(['pw-jack', '/usr/local/bin/reaper', '-newinst', '-nosplash', '-cfgfile', str(profile(key)),
                    str(project), str(repo / 'docs/sound-engines/scripts/host.lua')], env=env, cwd=S, stdout=log, stderr=log)
    end = time.monotonic() + 120
    while time.monotonic() < end and not (S / f'done-{label}').exists() and app.poll() is None: time.sleep(.3)
    assert (S / f'done-{label}').exists(), f'{label}: host did not finish; see run.txt'
    maps = [l for l in Path(f'/proc/{app.pid}/maps').read_text().splitlines() if 'kabl.clap' in l]
    assert maps, 'plugin not mapped'
    (OUT / f'{label}-mapped.txt').write_text('\n'.join(maps) + '\n')
    app.terminate(); app.wait(timeout=15)  # full quit
    shutil.copy2(S / f'saved-{label}.rpp', OUT / f'{label}.rpp')
    shutil.copy2(S / f'info-{label}.txt', OUT / f'{label}-info.txt')


def pcm(path):
    b = path.read_bytes(); at = 12; fmt = None
    while at < len(b):
        cid, size = b[at:at + 4], struct.unpack_from('<I', b, at + 4)[0]
        if cid == b'fmt ': fmt = struct.unpack_from('<HHIIHH', b, at + 8)
        if cid == b'data':
            assert fmt[0] == 3 and fmt[5] == 32, fmt
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


result = {'plugin_sha256': hashlib.sha256(NEW.read_bytes()).hexdigest(), 'baseline_sha256': hashlib.sha256(BASE.read_bytes()).hexdigest()}
try:
    run(NEW, 'new-initial', S / 'new.rpp')
    run(NEW, 'new-restart', S / 'saved-new-initial.rpp')
    s0, s1, s2 = (states_of(p.read_text()) for p in (S / 'new.rpp', S / 'saved-new-initial.rpp', S / 'saved-new-restart.rpp'))
    assert len(s0) == len(s1) == len(s2) == 3
    assert f32(s0) == f32(s1), 'state changed on first save'
    assert f32(s1) == f32(s2), 'state changed on reopen and save'
    kinds = [{m['kind'] for m in st['patch']['modules'].values()} for st in s2]
    assert all(w <= k for w, k in zip(want_kinds, kinds)), kinds
    result['states_identical_across_save_quit_reopen'] = True
    f_i, f_r = feats(S / 'render-new-initial.wav'), feats(S / 'render-new-restart.wav')
    result['new_initial'], result['new_restart'] = clean(f_i), clean(f_r)
    result['new_initial_vs_restart'] = compare(f_i, f_r)
    run(NEW, 'no-util', S / 'no-util.rpp')
    result['without_utilities'] = compare(f_i, feats(S / 'render-no-util.wav'))
    assert result['without_utilities']['relative_difference_rms'] > 0.05, 'the utility tracks do not drive the sound'
    run(NEW, 'old-new', OLD_PROJECT)
    run(BASE, 'old-base', OLD_PROJECT)
    o_orig = states_of(OLD_PROJECT.read_text())
    o_new = states_of((S / 'saved-old-new.rpp').read_text())
    result['old_state_identical_new_plugin'] = f32(o_orig) == f32(o_new)
    assert result['old_state_identical_new_plugin'], 'old state changed by the new plugin'
    g_n, g_b = feats(S / 'render-old-new.wav'), feats(S / 'render-old-base.wav')
    result['old_new'], result['old_base'] = clean(g_n), clean(g_b)
    result['old_new_vs_base'] = compare(g_n, g_b)
    run(BASE, 'old-base2', OLD_PROJECT)
    result['old_base_vs_base_again'] = compare(g_b, feats(S / 'render-old-base2.wav'))
finally:
    for pr in (wm, x):
        pr.terminate(); pr.wait(timeout=8)
    log.close(); shutil.copy2(S / 'run.txt', OUT / 'run.txt')
    (OUT / 'result.json').write_text(json.dumps(result, indent=1) + '\n')
print(json.dumps(result, indent=1))
