#!/usr/bin/env python3
"""Real-REAPER check of osc.fm / osc.wt in the CLAP plugin (PR #17).

    cargo build --release -p kabl-clap
    cargo build --release -p kabl-engine --example wavetable_import
    SE_BASELINE_PLUGIN=/path/to/libkabl_clap.so-built-from-origin/master \
        python3 docs/sound-engines/scripts/host.py

Same method as docs/factory-performance-bank/scripts/host.py: a private Xvfb display and REAPER
profile, a project whose <STATE> chunks are written by this script, CLAP_PATH pointing at a copy
of the plugin, the factory and user library paths made unavailable. host.lua saves the project
and renders the master mix with the host's own renderer; REAPER is then quit completely and
started again on the project it saved.

Phases
  new-initial / new-restart : a two-track project (track 1 = the imported-morph demo with a table
      imported from a .wav that is deleted before REAPER starts, osc.wt + osc.fm; track 2 =
      tine-keys, osc.fm). Saved, fully quit, reopened, saved and rendered again.
  old-new / old-base        : a project saved by the previous build (version 2 states, from the
      factory-bank evidence) opened by this build and by the baseline build.
"""
from pathlib import Path
import base64, hashlib, json, os, re, shutil, struct, subprocess as sp, time, uuid, wave
import numpy as np

repo = Path(__file__).resolve().parents[3]
S = repo / 'scratch/sound-engines-host'
OUT = repo / 'docs/sound-engines/evidence/host'
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
    return '<STATE\n' + ''.join('          ' + l + '\n' for l in __import__('textwrap').wrap(enc, 120)) + '        >'


# ---- the table: a real .wav, imported into a copy of the demo, then deleted -------------------
def sweep_wav(path):
    frames, n = 24, 2048
    t = np.arange(n) / n
    pcm = []
    for f in range(frames):
        centre = 2 + 20 * f / (frames - 1)
        cyc = sum((1 + 5 * np.exp(-.5 * ((h - centre) / 1.6) ** 2)) / h * np.sin(2 * np.pi * h * t) for h in range(1, 61))
        pcm.append((cyc / np.abs(cyc).max() * 30000).astype('<i2'))
    with wave.open(str(path), 'wb') as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(48000)
        w.writeframes(np.concatenate(pcm).tobytes())


demo = S / 'imported-morph'
shutil.copytree(repo / 'patches/sound-engines/demos/imported-morph', demo)
src_wav = S / 'host-sweep.wav'
sweep_wav(src_wav)
sp.run([str(repo / 'target/release/examples/wavetable_import'), str(demo), '1', str(src_wav)], check=True)
src_wav.unlink()
assert not src_wav.exists(), 'the original .wav must be gone before the host starts'
imported = json.loads((demo / 'checkpoint.json').read_text())
assert imported['tables']['1']['name'] == 'host-sweep.wav'
tine = json.loads((repo / 'patches/sound-engines/demos/tine-keys/checkpoint.json').read_text())


def state_for(patch):
    return {'version': 4 if patch.get('tables') else 2, 'patch': patch, 'output_gain': .4, 'host_clock': False,
            'lanes': [{'retired': False, 'target': None} for _ in range(16)], 'slot_values': [0.0] * 16}


# ---- the project: the previous build's two-track project, states and items replaced -----------
tpl = OLD_PROJECT.read_text()
proj = re.sub(r'\n      <PARMENV[\s\S]*?\n      >', '', tpl)
expected = [state_for(imported), state_for(tine)]
it = iter(expected)
proj = re.sub(r'<STATE\s+.*?\s*>', lambda m: chunk_of(next(it)), proj, flags=re.S)
item = re.search(r'\n    <ITEM[\s\S]*?\n    >(?=\n  >)', proj).group(0)
fresh = lambda m: '{' + str(uuid.uuid4()).upper() + '}'
item2 = re.sub(r'\{[0-9A-Fa-f-]{36}\}', fresh, item)
first_end = proj.index('\n  <TRACK', proj.index('<TRACK') + 1)
second_end = proj.rindex('\n  >\n>')
assert '<ITEM' not in proj[first_end:second_end]
proj = proj[:second_end] + item2 + proj[second_end:]
proj = proj.replace('Glass Keys - virtual/scripted MIDI', 'Imported Morph (osc.wt user table + osc.fm)').replace('Slow Horizons - editable real sequencer', 'Tine Keys (osc.fm)')
assert proj.count('<ITEM') == 2 and len(states_of(proj)) == 2
(S / 'new.rpp').write_text(proj)
# The same project with the imported table removed from the first state: the sound must change.
no_table = json.loads(json.dumps(expected[0])); no_table['patch']['tables'] = {}; no_table['version'] = 2
it2 = iter([no_table, expected[1]])
(S / 'no-table.rpp').write_text(re.sub(r'<STATE\s+.*?\s*>', lambda m: chunk_of(next(it2)), proj, flags=re.S))
(S / 'expected.json').write_text(json.dumps(expected, indent=1))

# ---- host ---------------------------------------------------------------------------------------
DISP = ':147'
env0 = dict(os.environ, DISPLAY=DISP, XDG_RUNTIME_DIR='/run/user/1000', KABL_FACTORY_DIR=str(S / 'unavailable-factory'),
            KABL_USER_DIR=str(S / 'unavailable-library'), KABL_SE_HOST=str(S), KABL_SE_SECS=str(SECS))
log = open(S / 'run.txt', 'w')
x = sp.Popen(['Xvfb', DISP, '-screen', '0', '1440x900x24', '-nolisten', 'tcp'], stdout=log, stderr=log); time.sleep(.8)
wm = sp.Popen(['metacity', '--sm-disable'], env=env0, stdout=log, stderr=log); time.sleep(.8)


def profile(name):
    d = S / f'profile-{name}'; (d).mkdir(exist_ok=True); (S / 'empty-vst').mkdir(exist_ok=True)
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


result = {'plugin_sha256': hashlib.sha256(NEW.read_bytes()).hexdigest(), 'baseline_sha256': hashlib.sha256(BASE.read_bytes()).hexdigest()}
try:
    # 1. new project, then full quit and reopen of what REAPER saved
    run(NEW, 'new-initial', S / 'new.rpp')
    run(NEW, 'new-restart', S / 'saved-new-initial.rpp')
    s0, s1, s2 = (states_of(p.read_text()) for p in (S / 'new.rpp', S / 'saved-new-initial.rpp', S / 'saved-new-restart.rpp'))
    assert len(s0) == len(s1) == len(s2) == 2
    assert f32(s0) == f32(s1), 'state changed on first save'
    assert f32(s1) == f32(s2), 'state changed on reopen and save'
    assert s2[0]['version'] == 4 and s2[0]['patch']['tables'] and s2[1]['version'] == 2 and not s2[1]['patch'].get('tables')
    result['new_states_identical_across_save_quit_reopen'] = True
    result['user_table_in_saved_project'] = {k: v['name'] for k, v in s2[0]['patch']['tables'].items()}
    assert not src_wav.exists()
    f_i, f_r = feats(S / 'render-new-initial.wav'), feats(S / 'render-new-restart.wav')
    result['new_initial'] = {k: v for k, v in f_i.items() if not k.startswith('_')}
    result['new_restart'] = {k: v for k, v in f_r.items() if not k.startswith('_')}
    result['new_initial_vs_restart'] = compare(f_i, f_r)
    run(NEW, 'notable', S / 'no-table.rpp')
    result['without_the_table'] = compare(f_i, feats(S / 'render-notable.wav'))
    assert result['without_the_table']['relative_difference_rms'] > 0.05, 'the imported table does not drive the sound'
    # 2. a project saved by the previous build
    run(NEW, 'old-new', OLD_PROJECT)
    run(BASE, 'old-base', OLD_PROJECT)
    o_orig = states_of(OLD_PROJECT.read_text())
    o_new, o_base = (states_of((S / f'saved-{t}.rpp').read_text()) for t in ('old-new', 'old-base'))
    result['old_project_versions'] = [s['version'] for s in o_orig]
    result['old_state_identical_new_plugin'] = f32(o_orig) == f32(o_new)
    result['old_state_identical_baseline_plugin'] = f32(o_orig) == f32(o_base)
    assert result['old_state_identical_new_plugin'], 'old state changed by the new plugin'
    g_n, g_b = feats(S / 'render-old-new.wav'), feats(S / 'render-old-base.wav')
    result['old_new'] = {k: v for k, v in g_n.items() if not k.startswith('_')}
    result['old_base'] = {k: v for k, v in g_b.items() if not k.startswith('_')}
    result['old_new_vs_base'] = compare(g_n, g_b)
    # noise floor: the same old project again in a fresh process of the baseline build
    run(BASE, 'old-base2', OLD_PROJECT)
    g_b2 = feats(S / 'render-old-base2.wav')
    result['old_base_vs_base_again'] = compare(g_b, g_b2)
    for lab in ('new-initial', 'new-restart', 'old-new', 'old-base', 'old-base2'):
        shutil.copy2(S / f'render-{lab}.wav', S / f'keep-{lab}.wav')
finally:
    for pr in (wm, x):
        pr.terminate(); pr.wait(timeout=8)
    log.close(); shutil.copy2(S / 'run.txt', OUT / 'run.txt')
    (OUT / 'result.json').write_text(json.dumps(result, indent=1) + '\n')
print(json.dumps(result, indent=1))
