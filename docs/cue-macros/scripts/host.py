#!/usr/bin/env python3
"""Real-REAPER check of cue-driven macro morphs in the CLAP plugin.

    cargo build --release -p kabl-clap
    python3 docs/cue-macros/scripts/host.py

Same method as docs/functional-cables/scripts/host_routes.py: a private Xvfb display and REAPER
profile, a project whose <STATE> chunk is written by this script from patches/cue-macros, a
CLAP_PATH holding a copy of the plugin, and docs/functional-cables/scripts/host.lua, which saves
the project and renders the master mix with the host's own renderer. REAPER is quit completely
and started again on the project it saved.

The project has one track with the plugin and a MIDI item that sends the cue buttons (CC 40-43,
the patch maps them to the four cues): Verse at 1.5 s, Chorus at 6 s, Bridge at 16 s, Verse 2 at
33 s. The plugin's clock runs free at 100 bpm (2.4 s a bar); a cue lands on the next bar.

Phases
  cues-initial / cues-restart : saved, fully quit, reopened, saved and rendered again.
  nocues                      : the same project without the MIDI item; its render must differ.
  automated                   : the cues project plus recorded host automation on lane 1 (macro
                                m1, Brightness): flat 0.1 to 14 s, ramp to 0.95 at 24 s, held.
                                The host value changes in every block of the ramp, so it takes
                                the macro from the Bridge's morph; the render must differ.
  old-new                     : a project saved by an earlier build opened and saved by this
                                one: its state must come back unchanged.
"""
from pathlib import Path
import base64, hashlib, json, os, re, shutil, struct, subprocess as sp, textwrap, time, uuid
import numpy as np

repo = Path(__file__).resolve().parents[3]
S = repo / 'scratch/cue-macros-host'
OUT = repo / 'docs/cue-macros/evidence/host'
PLUGIN = Path(os.environ.get('FC_PLUGIN', repo / 'target/release/libkabl_clap.so'))
OLD_PROJECT = repo / 'docs/factory-performance-bank/evidence/host/arrangement.rpp'
HOST_LUA = repo / 'docs/functional-cables/scripts/host.lua'
SECS = 40
CUES_AT = [(40, 1.5), (41, 6.0), (42, 16.0), (43, 33.0)]  # (cc, seconds)
ENV = [(0, 0.1), (14, 0.1), (24, 0.95), (40, 0.95)]
shutil.rmtree(S, ignore_errors=True)
S.mkdir(parents=True)
OUT.mkdir(parents=True, exist_ok=True)


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


patch = json.loads((repo / 'patches/cue-macros/checkpoint.json').read_text())
macro_id = int(next(i for i, m in patch['modules'].items() if m['kind'] == 'macro'))
lanes = [{'retired': False, 'target': None} for _ in range(16)]
lanes[0]['target'] = {'module': macro_id, 'kind': 'macro', 'param': 'm1'}
state = {'version': 2, 'patch': patch, 'output_gain': .4, 'host_clock': False, 'lanes': lanes, 'slot_values': [0.0] * 16}

# ---- the project: the template's first track, its notes replaced by cue buttons ------------------
tpl = OLD_PROJECT.read_text()
proj = re.sub(r'\n      <PARMENV[\s\S]*?\n      >', '', tpl)
t2 = proj.index('\n  <TRACK', proj.index('<TRACK') + 1)
proj = proj[:t2] + '\n>\n'
proj = re.sub(r'<STATE\s+.*?\s*>', lambda m: chunk_of(state), proj, count=1, flags=re.S)
proj = proj.replace('Glass Keys - virtual/scripted MIDI', 'Cue Macros (cue buttons on CC 40-43)')
assert proj.count('<ITEM') == 1 and len(states_of(proj)) == 1, (proj.count('<ITEM'), len(states_of(proj)))
TICKS = 960 * 2  # 120 bpm project tempo: 2 quarter notes a second
events, now = [], 0
for cc, t in CUES_AT:
    for val, at in ((127, t), (0, t + 0.2)):
        tick = round(at * TICKS)
        events.append(f'        E {tick - now} b0 {cc:02x} {val:02x}')
        now = tick
midi = re.search(r'(<SOURCE MIDI\n(?:        (?:HASDATA|CCINTERP|POOLEDEVTS).*\n)+)((?:        E .*\n)+)', proj)
assert midi
with_cues = proj[:midi.start(2)] + '\n'.join(events) + '\n' + proj[midi.end(2):]
no_cues = proj[:midi.start(2)] + proj[midi.end(2):]
parmenv = ('\n      <PARMENV 1:1248030256 0 1 0.5 "Slot 1 / kabl / Automation 1"\n'
           '        EGUID {5A1C2E70-7B3D-4C59-9E21-0D4F6A8B1C33}\n        ACT 1 -1\n        VIS 1 1 1\n'
           '        LANEHEIGHT 0 0\n        ARM 1\n        DEFSHAPE 0 -1 -1\n'
           + ''.join(f'        PT {t} {v} 0\n' for t, v in ENV) + '      >')
fxid = re.search(r'\n      FXID \{[0-9A-Fa-f-]{36}\}', with_cues)
automated = with_cues[:fxid.end()] + parmenv + with_cues[fxid.end():]
for name, text in (('cues', with_cues), ('nocues', no_cues), ('automated', automated)):
    (S / f'{name}.rpp').write_text(text)

# ---- host ---------------------------------------------------------------------------------------
DISP = ':148'
env0 = dict(os.environ, DISPLAY=DISP, XDG_RUNTIME_DIR='/run/user/1000', KABL_FACTORY_DIR=str(S / 'unavailable-factory'),
            KABL_USER_DIR=str(S / 'unavailable-library'), KABL_SE_HOST=str(S), KABL_SE_SECS=str(SECS))
log = open(S / 'run.txt', 'w')
x = sp.Popen(['Xvfb', DISP, '-screen', '0', '1440x900x24', '-nolisten', 'tcp'], stdout=log, stderr=log); time.sleep(.8)
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
                    str(project), str(HOST_LUA)], env=env, cwd=S, stdout=log, stderr=log)
    end = time.monotonic() + 180
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
            assert fmt[0] in (3, 65534) and fmt[5] == 32, fmt
            return np.frombuffer(b[at + 8:at + 8 + size], '<f4').reshape(-1, fmt[1])
        at += 8 + size + (size & 1)


def feats(path):
    p = pcm(path); m = p.mean(1); n = len(m)
    sec = np.sqrt((m[:n // 48000 * 48000].reshape(-1, 48000) ** 2).mean(1))
    return dict(frames=n, finite=bool(np.isfinite(p).all()), peak=float(np.abs(p).max()), rms=float(np.sqrt((m ** 2).mean())),
                per_second_rms=[round(float(v), 4) for v in sec], _sec=sec, _m=m)


def compare(a, b):
    n = min(len(a['_m']), len(b['_m']))
    diff = float(np.sqrt(((a['_m'][:n] - b['_m'][:n]) ** 2).mean()) / max(a['rms'], 1e-9))
    k = min(len(a['_sec']), len(b['_sec']))
    env = float(np.corrcoef(a['_sec'][:k], b['_sec'][:k])[0, 1])
    return dict(relative_difference_rms=round(diff, 4), envelope_correlation=round(env, 5))


def clean(f): return {k: v for k, v in f.items() if not k.startswith('_')}


result = {'plugin_sha256': hashlib.sha256(PLUGIN.read_bytes()).hexdigest(), 'cue_buttons': CUES_AT}
try:
    run('cues-initial', S / 'cues.rpp')
    run('cues-restart', S / 'saved-cues-initial.rpp')
    s0, s1, s2 = (states_of(p.read_text()) for p in (S / 'cues.rpp', S / 'saved-cues-initial.rpp', S / 'saved-cues-restart.rpp'))
    assert f32(s0) == f32(s1), 'state changed on first save'
    assert f32(s1) == f32(s2), 'state changed on reopen and save'
    result['states_identical_across_save_quit_reopen'] = True
    cues = {k: v for k, v in s2[0]['patch']['modules'][str(next(i for i, m in patch['modules'].items() if m['kind'] == 'cues'))]['params'].items() if '.macro.' in k or k.endswith('.morph')}
    result['cue_macro_params_in_saved_project'] = cues
    assert cues, 'cue macro targets in the saved state'
    f_i, f_r = feats(S / 'render-cues-initial.wav'), feats(S / 'render-cues-restart.wav')
    result['cues_initial'], result['cues_restart'] = clean(f_i), clean(f_r)
    result['initial_vs_restart'] = compare(f_i, f_r)
    run('nocues', S / 'nocues.rpp')
    f_n = feats(S / 'render-nocues.wav')
    result['cues_vs_nocues'] = compare(f_i, f_n)
    result['nocues'] = clean(f_n)
    assert result['cues_vs_nocues']['relative_difference_rms'] > 0.05, 'cue buttons do not change the sound in the host'
    run('automated', S / 'automated.rpp')
    f_a = feats(S / 'render-automated.wav')
    result['cues_vs_automated'] = compare(f_i, f_a)
    result['automated'] = clean(f_a)
    assert result['cues_vs_automated']['relative_difference_rms'] > 0.05, 'host automation does not change the sound'
    run('old-new', OLD_PROJECT)
    o_orig, o_new = states_of(OLD_PROJECT.read_text()), states_of((S / 'saved-old-new.rpp').read_text())
    result['old_state_identical_new_plugin'] = f32(o_orig) == f32(o_new)
    assert result['old_state_identical_new_plugin'], 'old state changed by the new plugin'
finally:
    for pr in (wm, x):
        pr.terminate(); pr.wait(timeout=8)
    log.close(); shutil.copy2(S / 'run.txt', OUT / 'run.txt')
    (OUT / 'result.json').write_text(json.dumps(result, indent=1) + '\n')
print(json.dumps(result, indent=1))
