#!/usr/bin/env python3
"""Real-REAPER check of the 16-voice unison, in the CLAP plugin, on the silent sink.

    cargo build --release -p kabl-clap && cp target/release/libkabl_clap.so scratch/unison-host-new.so
    (build the same with origin/master's osc_va.rs) && cp ... scratch/unison-host-master.so
    python3 docs/unison/scripts/host.py scratch/unison-host-new.so scratch/unison-host-master.so

Same method as docs/macros/scripts/host.py (private Xvfb display and REAPER profile, the project's
<STATE> chunks written by this script, host.lua saves the project and renders the master mix),
with audio on the silent `kabl_agents` sink (CLAUDE.md, Audio).

Project `super`: track 1 is patches/palette/pad with every osc.va at unison 16 (detune 30, curve
60), track 2 patches/palette/strings as it is. Saved, quit, reopened, saved, rendered: the state
must be identical and the two renders bit-identical and finite.
Project `old`: tracks patches/palette/pad and patches/palette/strings as they are (unison up to
4). Rendered with the new plugin and with the master plugin: the renders must be bit-identical.
"""
from pathlib import Path
import base64, hashlib, json, os, re, shutil, struct, subprocess as sp, sys, textwrap, time, uuid
import numpy as np

repo = Path(__file__).resolve().parents[3]
S = repo / 'scratch/unison-host'
OUT = repo / 'docs/unison/evidence/host'
NEW, MASTER = (Path(a).resolve() for a in sys.argv[1:3])
TEMPLATE = repo / 'docs/factory-performance-bank/evidence/host/arrangement.rpp'
HOST_LUA = repo / 'docs/macros/scripts/host.lua'
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


def state_for(patch):
    return {'version': 2, 'patch': patch, 'output_gain': .4, 'host_clock': False,
            'lanes': [{'retired': False, 'target': None} for _ in range(16)], 'slot_values': [0.0] * 16}


def patch_of(name):
    return json.loads((repo / f'patches/{name}/checkpoint.json').read_text())


def supersaw(patch):
    n = 0
    for m in patch['modules'].values():
        if m['kind'] == 'osc.va':
            m['params'].update(unison=16.0, detune=30.0, curve=60.0)
            n += 1
    assert n
    return patch


tpl = TEMPLATE.read_text()
base = re.sub(r'\n      <PARMENV[\s\S]*?\n      >', '', tpl)


def project(patches):
    proj = base
    expected = [state_for(p) for p in patches]
    it = iter(expected)
    proj = re.sub(r'<STATE\s+.*?\s*>', lambda m: chunk_of(next(it)), proj, flags=re.S)
    item = re.search(r'\n    <ITEM[\s\S]*?\n    >(?=\n  >)', proj).group(0)
    item2 = re.sub(r'\{[0-9A-Fa-f-]{36}\}', lambda m: '{' + str(uuid.uuid4()).upper() + '}', item)
    first_end = proj.index('\n  <TRACK', proj.index('<TRACK') + 1)
    second_end = proj.rindex('\n  >\n>')
    assert '<ITEM' not in proj[first_end:second_end]
    proj = proj[:second_end] + item2 + proj[second_end:]
    assert proj.count('<ITEM') == 2 and len(states_of(proj)) == 2
    return proj, expected


DISP = ':177'
env0 = dict(os.environ, DISPLAY=DISP, XDG_RUNTIME_DIR='/run/user/1000', KABL_FACTORY_DIR=str(S / 'unavailable-factory'),
            KABL_USER_DIR=str(S / 'unavailable-library'), KABL_SE_HOST=str(S), KABL_SE_SECS=str(SECS),
            PIPEWIRE_NODE='kabl_agents', PULSE_SINK='kabl_agents')
log = open(S / 'run.txt', 'w')
x = sp.Popen(['Xvfb', DISP, '-screen', '0', '1600x1000x24', '-nolisten', 'tcp'], stdout=log, stderr=log); time.sleep(.8)
wm = sp.Popen(['metacity', '--sm-disable'], env=env0, stdout=log, stderr=log); time.sleep(.8)


def profile():
    d = S / 'profile'; d.mkdir(exist_ok=True); (S / 'empty-vst').mkdir(exist_ok=True)
    (d / 'reaper.ini').write_text(f'[reaper]\nvstpath={S}/empty-vst\nvstpath64={S}/empty-vst\nlinux_audio_bsize=256\nlinux_audio_mode=0\nlinux_audio_srate=48000\nwnd_w=1280\nwnd_h=800\nwnd_x=0\nwnd_y=0\n')
    return d / 'reaper.ini'


def sinks():
    return sp.run(['pactl', 'list', 'sink-inputs', 'short'], text=True, capture_output=True).stdout


def run(label, project_path, plugin):
    pdir = S / f'plugin-{label}'; pdir.mkdir(exist_ok=True)
    shutil.copy2(plugin, pdir / 'kabl.clap')
    env = dict(env0, CLAP_PATH=str(pdir), KABL_SE_TAG=label)
    app = sp.Popen(['pw-jack', '/usr/local/bin/reaper', '-newinst', '-nosplash', '-cfgfile', str(profile()),
                    str(project_path), str(HOST_LUA)], env=env, cwd=S, stdout=log, stderr=log)
    end = time.monotonic() + 180
    while time.monotonic() < end and not (S / f'done-{label}').exists() and app.poll() is None:
        time.sleep(.3)
        if int(time.monotonic()) % 5 == 0:
            (S / f'sinks-{label}.txt').write_text(sinks())  # streams must sit on kabl_agents (sink 1956 here)
    assert (S / f'done-{label}').exists(), f'{label}: host did not finish; see run.txt'
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
    p = pcm(path)
    return p, dict(frames=len(p), finite=bool(np.isfinite(p).all()), peak=float(np.abs(p).max()),
                   rms=float(np.sqrt((p ** 2).mean())), sha256=hashlib.sha256(p.tobytes()).hexdigest())


result = {'new_plugin_sha256': hashlib.sha256(NEW.read_bytes()).hexdigest(),
          'master_plugin_sha256': hashlib.sha256(MASTER.read_bytes()).hexdigest()}
try:
    sup, sup_expected = project([supersaw(patch_of('palette/pad')), patch_of('palette/strings')])
    (S / 'super.rpp').write_text(sup)
    run('super-initial', S / 'super.rpp', NEW)
    run('super-restart', S / 'saved-super-initial.rpp', NEW)
    s0, s1, s2 = (states_of(p.read_text()) for p in (S / 'super.rpp', S / 'saved-super-initial.rpp', S / 'saved-super-restart.rpp'))
    assert f32(s0) == f32(s1), 'state changed on first save'
    assert f32(s1) == f32(s2), 'state changed on reopen and save'
    result['super_state_identical_across_save_quit_reopen'] = True
    result['super_unison_in_saved_project'] = sorted(m['params']['unison'] for m in s2[0]['patch']['modules'].values() if m['kind'] == 'osc.va')
    (a, fa), (b, fb) = feats(S / 'render-super-initial.wav'), feats(S / 'render-super-restart.wav')
    result['super_initial'], result['super_restart'] = fa, fb
    result['super_renders_bit_identical'] = bool(np.array_equal(a, b))
    assert fa['finite'] and fa['rms'] > 1e-3 and result['super_renders_bit_identical']
    old, old_expected = project([patch_of('palette/pad'), patch_of('palette/strings')])
    (S / 'old.rpp').write_text(old)
    run('old-new', S / 'old.rpp', NEW)
    run('old-master', S / 'old.rpp', MASTER)
    o0, o1 = states_of((S / 'saved-old-new.rpp').read_text()), states_of((S / 'saved-old-master.rpp').read_text())
    result['old_state_identical_new_and_master'] = f32(o0) == f32(o1) == f32(old_expected)
    (c, fc), (d, fd) = feats(S / 'render-old-new.wav'), feats(S / 'render-old-master.wav')
    result['old_new'], result['old_master'] = fc, fd
    result['old_renders_bit_identical_to_master'] = bool(np.array_equal(c, d))
    assert fc['finite'] and fc['rms'] > 1e-3 and result['old_renders_bit_identical_to_master']
    result['super_differs_from_old'] = not np.array_equal(a, c)
finally:
    for pr in (wm, x):
        pr.terminate(); pr.wait(timeout=8)
    log.close(); shutil.copy2(S / 'run.txt', OUT / 'run.txt')
    for f in S.glob('sinks-*.txt'): shutil.copy2(f, OUT / f.name)
    (OUT / 'result.json').write_text(json.dumps(result, indent=1) + '\n')
print(json.dumps(result, indent=1))
