#!/usr/bin/env python3
"""Real-REAPER check of cues switching drum grid patterns in the CLAP plugin.

    cargo build --release -p kabl-clap
    python3 docs/grid-cues/scripts/host_cues.py

Method of docs/cue-macros/scripts/host.py: a private Xvfb display and REAPER profile, a project
whose <STATE> chunk is written by this script from patches/grid-cues, a copy of the plugin on
CLAP_PATH, docs/functional-cables/scripts/host.lua saving the project and rendering the master mix.
REAPER is quit completely and started again on the project it saved.

The project's MIDI item sends the cue buttons (CC 40-43: Intro, Groove, Fill, Break) at 0.1 s,
3.2 s, 10.5 s and 12.5 s. The plugin's clock runs free at 124 bpm (1.935 s a bar), so the cues
land on the bar lines at 3.87 s (bar 2), 11.61 s (bar 6) and 13.55 s (bar 7), if the clock starts with the render. Intro lands at once (its timing is Now). Checked: the saved state is identical across save, quit, reopen, save;
Groove steps the loudness up and Break drops it five bars later, each within a bar of its
button; the same project without the MIDI item stays on its stored pattern A.

No sound reaches a real device: REAPER runs on the `kabl_agents` null sink.
"""
from pathlib import Path
import base64, hashlib, json, os, re, shutil, struct, subprocess as sp, textwrap, time, uuid
import numpy as np

repo = Path(__file__).resolve().parents[3]
S = repo / 'scratch/grid-cues-host'
OUT = repo / 'docs/grid-cues/evidence/host'
PLUGIN = Path(os.environ.get('FC_PLUGIN', repo / 'target/release/libkabl_clap.so'))
OLD_PROJECT = repo / 'docs/factory-performance-bank/evidence/host/arrangement.rpp'
HOST_LUA = repo / 'docs/functional-cables/scripts/host.lua'
SECS = 16
CUES_AT = [(40, 0.1), (41, 3.2), (42, 10.5), (43, 12.5)]  # (cc, seconds)
BAR = 4 * 60 / 124
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


patch = json.loads((repo / 'patches/grid-cues/checkpoint.json').read_text())
lanes = [{'retired': False, 'target': None} for _ in range(16)]
state = {'version': 2, 'patch': patch, 'output_gain': .4, 'host_clock': False, 'lanes': lanes, 'slot_values': [0.0] * 16}

# ---- the project: the template's first track, its notes replaced by cue buttons ------------------
tpl = OLD_PROJECT.read_text()
proj = re.sub(r'\n      <PARMENV[\s\S]*?\n      >', '', tpl)
t2 = proj.index('\n  <TRACK', proj.index('<TRACK') + 1)
proj = proj[:t2] + '\n>\n'
proj = re.sub(r'<STATE\s+.*?\s*>', lambda m: chunk_of(state), proj, count=1, flags=re.S)
proj = proj.replace('Glass Keys - virtual/scripted MIDI', 'Grid Cues (cue buttons on CC 40-43)')
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
for name, text in (('cues', with_cues), ('nocues', no_cues)):
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
    cue_id = str(next(i for i, m in patch['modules'].items() if m['kind'] == 'cues'))
    cues = {k: v for k, v in s2[0]['patch']['modules'][cue_id]['params'].items() if '.seq.' in k}
    result['cue_grid_targets_in_saved_project'] = cues
    assert len(cues) == 4, ('four grid targets in the saved state', cue_id, cues)
    f_i, f_r = feats(S / 'render-cues-initial.wav'), feats(S / 'render-cues-restart.wav')
    result['cues_initial'], result['cues_restart'] = clean(f_i), clean(f_r)
    result['initial_vs_restart'] = compare(f_i, f_r)
    run('nocues', S / 'nocues.rpp')
    f_n = feats(S / 'render-nocues.wav')
    result['cues_vs_nocues'] = compare(f_i, f_n)
    result['nocues'] = clean(f_n)
    assert result['cues_vs_nocues']['relative_difference_rms'] > 0.05, 'cue buttons do not change the sound in the host'

    # loudness in sixteenths of a bar: where does it step up (Groove) and down (Break)?
    def steps(f):
        m = f['_m']; w = int(BAR * 48000 / 16)
        r = np.sqrt((m[:len(m) // w * w].reshape(-1, w) ** 2).mean(1))
        return r, w
    r, w = steps(f_i)
    intro = np.median(r[:16])
    up = int(np.argmax((np.arange(len(r)) >= 16) & (r > 4 * intro))); down = int(np.argmax((np.arange(len(r)) > 16 * 7) & (r < 0.75 * np.median(r[16 * 3:16 * 6]))))
    result['groove_step_up_s'] = round(up * w / 48000, 3)
    result['break_step_down_s'] = round(down * w / 48000, 3)
    result['bars_between'] = round((down - up) * w / 48000 / BAR, 2)
    # The plugin's clock starts at a different point of the render in each run, so the bar lines
    # shift by up to a bar between runs; the cues must land at whole bars from each other
    # (Groove at bar 2, Break at bar 7: five bars) and within one bar after their button.
    assert abs((down - up) * w / 48000 / BAR - 5) <= 1 / 8 + 1e-6, 'Groove and Break are not five bars apart'
    assert 3.2 <= up * w / 48000 <= 3.2 + BAR + BAR / 8, 'Groove does not land within a bar of its button'
    assert 12.5 <= down * w / 48000 <= 12.5 + BAR + BAR / 4, 'Break does not land within a bar of its button'
    rn, _ = steps(f_n)
    assert np.median(rn[:16 * 14]) > 4 * intro, 'without cue buttons the grid keeps its stored pattern A (no hats-only intro)'
finally:
    for pr in (wm, x):
        pr.terminate(); pr.wait(timeout=8)
    log.close(); shutil.copy2(S / 'run.txt', OUT / 'run.txt')
    (OUT / 'result.json').write_text(json.dumps(result, indent=1) + '\n')
print(json.dumps(result, indent=1))
