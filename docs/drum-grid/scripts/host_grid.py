#!/usr/bin/env python3
"""Real-REAPER check of the drum grid in the CLAP plugin: the showcase patch (one clock, one
grid, five voices) plays from the plugin's own clock and the master mix is rendered offline.

    cargo build --release -p kabl-clap
    cargo test -p kabl-ui --test drum_demos write_state_json -- --ignored
    python3 docs/drum-grid/scripts/host_grid.py

Same method as docs/drums/scripts/host_drums.py (private Xvfb display and REAPER profile, project
<STATE> chunk written by this script, host_rhythm.lua saves the project and renders). The
kick is found in the low band and compared with the 124 bpm beat grid; the project is saved,
reopened and rendered again, and the two renders must be bit-identical.

No sound reaches a real device: REAPER runs on the `kabl_agents` null sink (PIPEWIRE_NODE and
PULSE_SINK), and the script prints the links of its JACK ports so that can be checked.
"""
from pathlib import Path
import base64, hashlib, json, os, re, shutil, struct, subprocess as sp, textwrap, time, uuid
import numpy as np

repo = Path(__file__).resolve().parents[3]
S = repo / 'scratch/grid-host'
OUT = repo / 'docs/drum-grid/evidence/host'
NEW = Path(os.environ.get('SE_PLUGIN', repo / 'target/release/libkabl_clap.so'))
OLD_PROJECT = repo / 'docs/factory-performance-bank/evidence/host/arrangement.rpp'
LUA = repo / 'docs/sound-engines/scripts/host_rhythm.lua'
STATE = repo / 'target/drum-grid/state.json'
SR = 48000
BEAT = 60 / 124 * SR
shutil.rmtree(S, ignore_errors=True)
S.mkdir(parents=True); OUT.mkdir(parents=True, exist_ok=True)


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


def build_project(name, patch):
    proj = re.sub(r'\n      <PARMENV[\s\S]*?\n      >', '', OLD_PROJECT.read_text())
    proj = re.sub(r'\n    <TEMPOENVEX[\s\S]*?\n    >', '', proj)
    i1 = proj.index('\n  <TRACK'); i2 = proj.index('\n  <TRACK', i1 + 1); last = proj.rindex('\n>')
    header, first, footer = proj[:i1], proj[i1:i2], proj[last:]
    guid = r'\{[0-9A-Fa-f-]{36}\}'
    t = re.sub(guid, lambda m: '{' + str(uuid.uuid4()).upper() + '}', first)
    t = re.sub(r'(NAME )"[^"\n]*"', lambda m: m.group(1) + f'"{name}"', t, count=1)
    t = re.sub(r'<STATE\s+.*?\s*>', lambda m: chunk_of(state_for(patch)), t, flags=re.S)
    t = re.sub(r'\n        POOLEDEVTS [^\n]*', '', t)
    t = re.sub(r'(\n        E [^\n]*)+', '\n        E 0 80 00 00', t, count=1)
    t = re.sub(r'LENGTH \d+', 'LENGTH 30', t, count=1)
    t = re.sub(r'LOOP 1', 'LOOP 0', t, count=1)
    return header + t + footer


(S / 'grid.rpp').write_text(build_project('Drum grid', json.loads(STATE.read_text())))

DISP = ':232'
SINK = {'PIPEWIRE_NODE': 'kabl_agents', 'PULSE_SINK': 'kabl_agents'}
env0 = dict(os.environ, DISPLAY=DISP, XDG_RUNTIME_DIR='/run/user/1000', KABL_FACTORY_DIR=str(S / 'unavailable-factory'),
            KABL_USER_DIR=str(S / 'unavailable-library'), KABL_SE_HOST=str(S), **SINK)
log = open(S / 'run.txt', 'w')
x = sp.Popen(['Xvfb', DISP, '-screen', '0', '1440x900x24', '-nolisten', 'tcp'], stdout=log, stderr=log); time.sleep(.8)
wm = sp.Popen(['metacity', '--sm-disable'], env=env0, stdout=log, stderr=log); time.sleep(.8)


def profile():
    d = S / 'profile'; d.mkdir(exist_ok=True); (S / 'empty-vst').mkdir(exist_ok=True)
    (d / 'reaper.ini').write_text(f'[reaper]\nvstpath={S}/empty-vst\nvstpath64={S}/empty-vst\nlinux_audio_bsize=256\nlinux_audio_mode=0\nlinux_audio_srate=48000\nwnd_w=1280\nwnd_h=800\nwnd_x=0\nwnd_y=0\n')
    return d / 'reaper.ini'


def run(label, project, secs=16, start=0.0):
    pdir = S / 'plugins'; pdir.mkdir(exist_ok=True)
    shutil.copy2(NEW, pdir / 'kabl.clap')
    env = dict(env0, CLAP_PATH=str(pdir), KABL_SE_TAG=label, KABL_SE_SECS=str(secs), KABL_SE_START=str(start), KABL_SE_TEMPO='0')
    app = sp.Popen(['pw-jack', '/usr/local/bin/reaper', '-newinst', '-nosplash', '-cfgfile', str(profile()),
                    str(project), str(LUA)], env=env, cwd=S, stdout=log, stderr=log)
    time.sleep(4)
    links = sp.run(['pw-link', '-l'], capture_output=True, text=True).stdout
    (OUT / f'links-{label}.txt').write_text(links)
    end = time.monotonic() + 150
    while time.monotonic() < end and not (S / f'done-{label}').exists() and app.poll() is None: time.sleep(.3)
    assert (S / f'done-{label}').exists(), f'{label}: host did not finish; see run.txt'
    assert [l for l in Path(f'/proc/{app.pid}/maps').read_text().splitlines() if 'kabl.clap' in l], 'plugin not mapped'
    app.terminate(); app.wait(timeout=15)
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


def lowpass(m, hz):
    k = 1 - np.exp(-2 * np.pi * hz / SR)
    for _ in range(2):
        y = np.empty_like(m); acc = 0.0
        for i, v in enumerate(m):
            acc += k * (v - acc); y[i] = acc
        m = y
    return m


result = {'plugin_sha256': hashlib.sha256(NEW.read_bytes()).hexdigest()}
try:
    run('grid', S / 'grid.rpp', secs=16)
    run('grid-restart', S / 'saved-grid.rpp', secs=16)
    s0, s1, s2 = (states_of(p.read_text()) for p in (S / 'grid.rpp', S / 'saved-grid.rpp', S / 'saved-grid-restart.rpp'))
    assert s0 == s1 == s2, 'patch state changed through save and reopen'
    g, g2 = pcm(S / 'render-grid.wav'), pcm(S / 'render-grid-restart.wav')
    m = g.mean(1)
    assert np.isfinite(g).all()
    low = np.abs(lowpass(m, 120))
    # The kick sits on every beat; the first beat is at the start of the render.
    kicks = [int(np.argmax(low[int(k * BEAT):int(k * BEAT) + 2400])) for k in range(1, 20)]
    med = int(np.median(kicks))
    result['grid'] = dict(peak=float(np.abs(g).max()), rms=float(np.sqrt((g ** 2).mean())),
                          kick_offsets_samples=kicks, kick_median_offset_samples=med,
                          restart_render_bit_identical=bool(g.shape == g2.shape and (g == g2).all()))
    assert max(abs(k - med) for k in kicks) <= 24, kicks
    assert result['grid']['restart_render_bit_identical']
    assert result['grid']['rms'] > 0.02
finally:
    for pr in (wm, x):
        pr.terminate(); pr.wait(timeout=8)
    log.close(); shutil.copy2(S / 'run.txt', OUT / 'run.txt')
    (OUT / 'result.json').write_text(json.dumps(result, indent=1) + '\n')
print(json.dumps(result, indent=1))
