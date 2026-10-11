#!/usr/bin/env python3
"""Real-REAPER check of the 1.0 effects and filters in the CLAP plugin.

    python3 docs/effects-filters/scripts/make_demos.py          # writes scratch/effects-demos
    cargo build --release -p kabl-clap -p kabl-engine --example patch_json
    SE_BASELINE_PLUGIN=/path/to/libkabl_clap.so-built-from-origin/master \
        python3 docs/effects-filters/scripts/host_effects.py

Same method as docs/sound-engines/scripts/host_rhythm.py (private Xvfb display and REAPER profile,
project <STATE> chunks written by this script, factory and user library made unavailable; the
same host_rhythm.lua saves the project and renders the master mix offline, then REAPER is quit
completely and started again on what it saved). One project with nine self-playing tracks, one per
demo patch (`<name>-wet`: phaser, flanger, compressor, eq, limiter, bitcrusher, comb, formant, and
the SVF with its notch output), and the same project with the dry patches. Checks: all nine
plugins load; plugin states are equal across save, quit, reopen and save; the reopened render is
bit-identical; the wet render is finite, not silent, and differs from the dry one. Also the
factory-bank project from before, through this build and the baseline build: bit-identical.
"""
from pathlib import Path
import base64, hashlib, json, os, re, shutil, struct, subprocess as sp, textwrap, time, uuid
import numpy as np

repo = Path(__file__).resolve().parents[3]
S = repo / 'scratch/effects-host'
OUT = repo / 'docs/effects-filters/evidence/host'
NEW = Path(os.environ.get('SE_PLUGIN', repo / 'target/release/libkabl_clap.so'))
BASE = Path(os.environ['SE_BASELINE_PLUGIN'])
OLD_PROJECT = repo / 'docs/factory-performance-bank/evidence/host/arrangement.rpp'
DEMOS = repo / 'scratch/effects-demos'
NAMES = ['phaser', 'flanger', 'compressor', 'eq', 'limiter', 'bitcrusher', 'comb', 'formant', 'notch']
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


def patch_of(d):
    out = sp.run([str(repo / 'target/release/examples/patch_json'), str(d)], check=True, capture_output=True, text=True)
    return json.loads(out.stdout)


def state_for(patch):
    return {'version': 2, 'patch': patch, 'output_gain': .4, 'host_clock': False,
            'lanes': [{'retired': False, 'target': None} for _ in range(16)], 'slot_values': [0.0] * 16}


def build_project(tracks):
    """`tracks`: (name, patch). Cloned from the first track of the old project."""
    proj = re.sub(r'\n      <PARMENV[\s\S]*?\n      >', '', OLD_PROJECT.read_text())
    i1 = proj.index('\n  <TRACK'); i2 = proj.index('\n  <TRACK', i1 + 1); last = proj.rindex('\n>')
    header, first, footer = proj[:i1], proj[i1:i2], proj[last:]
    guid = r'\{[0-9A-Fa-f-]{36}\}'
    out = header
    for name, patch in tracks:
        t = re.sub(guid, lambda m: '{' + str(uuid.uuid4()).upper() + '}', first)
        t = re.sub(r'(NAME )"[^"\n]*"', lambda m: m.group(1) + f'"{name}"', t, count=1)
        t = re.sub(r'<STATE\s+.*?\s*>', lambda m: chunk_of(state_for(patch)), t, flags=re.S)
        t = re.sub(r'LENGTH \d+', 'LENGTH 30', t, count=1)
        out += t
    return out + footer


for tag in ('wet', 'dry'):
    (S / f'{tag}.rpp').write_text(build_project([(n, patch_of(DEMOS / f'{n}-{tag}')) for n in NAMES]))

DISP = ':152'
env0 = dict(os.environ, DISPLAY=DISP, XDG_RUNTIME_DIR='/run/user/1000', KABL_FACTORY_DIR=str(S / 'unavailable-factory'),
            KABL_USER_DIR=str(S / 'unavailable-library'), KABL_SE_HOST=str(S))
log = open(S / 'run.txt', 'w')
x = sp.Popen(['Xvfb', DISP, '-screen', '0', '1440x900x24', '-nolisten', 'tcp'], stdout=log, stderr=log); time.sleep(.8)
wm = sp.Popen(['metacity', '--sm-disable'], env=env0, stdout=log, stderr=log); time.sleep(.8)


def profile(name):
    d = S / f'profile-{name}'; d.mkdir(exist_ok=True); (S / 'empty-vst').mkdir(exist_ok=True)
    (d / 'reaper.ini').write_text(f'[reaper]\nvstpath={S}/empty-vst\nvstpath64={S}/empty-vst\nlinux_audio_bsize=256\nlinux_audio_mode=0\nlinux_audio_srate=48000\nwnd_w=1280\nwnd_h=800\nwnd_x=0\nwnd_y=0\n')
    return d / 'reaper.ini'


def run(plugin, label, project, secs=20):
    key = 'base' if plugin == BASE else 'new'
    pdir = S / f'plugins-{key}'; pdir.mkdir(exist_ok=True)
    shutil.copy2(plugin, pdir / 'kabl.clap')
    env = dict(env0, CLAP_PATH=str(pdir), KABL_SE_TAG=label, KABL_SE_SECS=str(secs), KABL_SE_START='0', KABL_SE_TEMPO='0')
    app = sp.Popen(['pw-jack', '/usr/local/bin/reaper', '-newinst', '-nosplash', '-cfgfile', str(profile(key)),
                    str(project), str(repo / 'docs/sound-engines/scripts/host_rhythm.lua')], env=env, cwd=S, stdout=log, stderr=log)
    end = time.monotonic() + 150
    while time.monotonic() < end and not (S / f'done-{label}').exists() and app.poll() is None: time.sleep(.3)
    assert (S / f'done-{label}').exists(), f'{label}: host did not finish; see run.txt'
    assert any('kabl.clap' in l for l in Path(f'/proc/{app.pid}/maps').read_text().splitlines()), 'plugin not mapped'
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


result = {'plugin_sha256': hashlib.sha256(NEW.read_bytes()).hexdigest(), 'baseline_sha256': hashlib.sha256(BASE.read_bytes()).hexdigest()}
try:
    run(NEW, 'wet-initial', S / 'wet.rpp')
    run(NEW, 'wet-restart', S / 'saved-wet-initial.rpp')
    s0, s1, s2 = (states_of(p.read_text()) for p in (S / 'wet.rpp', S / 'saved-wet-initial.rpp', S / 'saved-wet-restart.rpp'))
    assert len(s0) == len(s1) == len(s2) == len(NAMES)
    assert f32(s0) == f32(s1), 'state changed on first save'
    assert f32(s1) == f32(s2), 'state changed on reopen and save'
    result['states_identical_across_save_quit_reopen'] = True
    a, b = pcm(S / 'render-wet-initial.wav'), pcm(S / 'render-wet-restart.wav')
    result['wet'] = dict(finite=bool(np.isfinite(a).all()), peak=float(np.abs(a).max()), rms=float(np.sqrt((a ** 2).mean())),
                         reopen_render_bit_identical=bool(a.shape == b.shape and (a == b).all()))
    assert result['wet']['finite'] and result['wet']['rms'] > 1e-3 and result['wet']['reopen_render_bit_identical']
    run(NEW, 'dry', S / 'dry.rpp')
    d = pcm(S / 'render-dry.wav')
    n = min(len(a), len(d))
    result['wet_vs_dry_relative_difference_rms'] = round(float(np.sqrt(((a[:n] - d[:n]) ** 2).mean()) / np.sqrt((a ** 2).mean())), 4)
    assert result['wet_vs_dry_relative_difference_rms'] > 0.05, 'the effects do not change the sound'
    run(NEW, 'old-new', OLD_PROJECT, secs=24)
    run(BASE, 'old-base', OLD_PROJECT, secs=24)
    result['old_state_identical_new_plugin'] = f32(states_of(OLD_PROJECT.read_text())) == f32(states_of((S / 'saved-old-new.rpp').read_text()))
    assert result['old_state_identical_new_plugin']
    g_n, g_b = pcm(S / 'render-old-new.wav'), pcm(S / 'render-old-base.wav')
    result['old_render_bit_identical_to_baseline'] = bool(g_n.shape == g_b.shape and (g_n == g_b).all())
    result['old_render_sha256'] = hashlib.sha256(g_n.tobytes()).hexdigest()
    assert result['old_render_bit_identical_to_baseline']
finally:
    for pr in (wm, x):
        pr.terminate(); pr.wait(timeout=8)
    log.close(); shutil.copy2(S / 'run.txt', OUT / 'run.txt')
    (OUT / 'result.json').write_text(json.dumps(result, indent=1) + '\n')
print(json.dumps(result, indent=1))
