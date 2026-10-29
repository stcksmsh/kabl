#!/usr/bin/env python3
"""Real-REAPER check of the pilot presets in the CLAP plugin.

    cargo build --release -p kabl-clap -p kabl-ui --examples
    target/release/examples/presets_pilot phrases scratch/presets-phrases.json
    python3 docs/presets/scripts/host.py [slug ...]

Same method as docs/cue-macros/scripts/host.py: a private Xvfb display and REAPER profile, one
project per preset whose <STATE> chunk is written from patches/presets/<slug>, a MIDI item that
plays the preset's clip phrase (and, for the piece, its cue buttons on CC 40-44), and
docs/functional-cables/scripts/host.lua, which saves the project and renders the master mix
with the host's own renderer. REAPER is quit completely and started again on the project it
saved.

Per preset
  init / restart : the project is rendered, saved, fully quit, reopened, saved and rendered
                   again. The saved states must be identical; both renders finite, under 0 dBFS
                   and not silent; the two renders equal.
  nomidi         : the same project without the MIDI item. A keyboard preset must render far
                   quieter (the MIDI is what plays it); the self-playing piece is exempt.
"""
from pathlib import Path
import base64, hashlib, json, os, re, shutil, struct, subprocess as sp, sys, textwrap, time
import numpy as np

repo = Path(__file__).resolve().parents[3]
S = repo / 'scratch/presets-host'
OUT = Path(os.environ.get('PH_OUT', repo / 'docs/presets/evidence/host'))
PLUGIN = Path(os.environ.get('FC_PLUGIN', repo / 'target/release/libkabl_clap.so'))
TEMPLATE = repo / 'docs/factory-performance-bank/evidence/host/arrangement.rpp'
HOST_LUA = repo / 'docs/functional-cables/scripts/host.lua'
phrases = json.loads(Path(os.environ.get('PH_PHRASES', repo / 'scratch/presets-phrases.json')).read_text())
slugs = sys.argv[1:] or list(phrases)
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


def project(slug, with_midi):
    patch = json.loads((repo / 'patches/presets' / slug / 'checkpoint.json').read_text())
    state = {'version': 2, 'patch': patch, 'output_gain': .4, 'host_clock': False,
             'lanes': [{'retired': False, 'target': None} for _ in range(16)], 'slot_values': [0.0] * 16}
    proj = re.sub(r'\n      <PARMENV[\s\S]*?\n      >', '', TEMPLATE.read_text())
    t2 = proj.index('\n  <TRACK', proj.index('<TRACK') + 1)
    proj = proj[:t2] + '\n>\n'
    proj = re.sub(r'<STATE\s+.*?\s*>', lambda m: chunk_of(state), proj, count=1, flags=re.S)
    proj = proj.replace('Glass Keys - virtual/scripted MIDI', f'{slug} (pilot preset)')
    assert proj.count('<ITEM') == 1 and len(states_of(proj)) == 1
    midi = re.search(r'(<SOURCE MIDI\n(?:        (?:HASDATA|CCINTERP|POOLEDEVTS).*\n)+)((?:        E .*\n)+)', proj)
    assert midi
    if not with_midi:
        return proj[:midi.start(2)] + proj[midi.end(2):]
    ph = phrases[slug]
    events = [(t, 0x90, n, v) if v else (t, 0x80, n, 0) for t, n, v in ph['hits']]
    events += [(t, 0xb0, cc, val) for cc, t in ph['cues'] for val, t in ((127, t), (0, t + .2))]
    events.sort(key=lambda e: (e[0], e[1] == 0x90))  # releases before presses at the same time
    TICKS, now, lines = 960 * 2, 0, []  # 120 bpm project tempo: 2 quarter notes a second
    for t, status, a, b in events:
        tick = round(t * TICKS)
        lines.append(f'        E {tick - now} {status:02x} {a:02x} {b:02x}')
        now = tick
    return proj[:midi.start(2)] + '\n'.join(lines) + '\n' + proj[midi.end(2):]


DISP = ':149'
env0 = dict(os.environ, PIPEWIRE_NODE='kabl_agents', PULSE_SINK='kabl_agents', DISPLAY=DISP, XDG_RUNTIME_DIR='/run/user/1000', KABL_FACTORY_DIR=str(S / 'unavailable-factory'),
            KABL_USER_DIR=str(S / 'unavailable-library'), KABL_SE_HOST=str(S))
log = open(S / 'run.txt', 'w')
x = sp.Popen(['Xvfb', DISP, '-screen', '0', '1440x900x24', '-nolisten', 'tcp'], stdout=log, stderr=log); time.sleep(.8)
wm = sp.Popen(['metacity', '--sm-disable'], env=env0, stdout=log, stderr=log); time.sleep(.8)


def profile():
    d = S / 'profile'; d.mkdir(exist_ok=True); (S / 'empty-vst').mkdir(exist_ok=True)
    (d / 'reaper.ini').write_text(f'[reaper]\nvstpath={S}/empty-vst\nvstpath64={S}/empty-vst\nlinux_audio_bsize=256\nlinux_audio_mode=0\nlinux_audio_srate=48000\nwnd_w=1280\nwnd_h=800\nwnd_x=0\nwnd_y=0\n')
    return d / 'reaper.ini'


def run(label, proj, secs):
    pdir = S / 'plugin'; pdir.mkdir(exist_ok=True)
    shutil.copy2(PLUGIN, pdir / 'kabl.clap')
    env = dict(env0, CLAP_PATH=str(pdir), KABL_SE_TAG=label, KABL_SE_SECS=str(secs))
    app = sp.Popen(['pw-jack', '/usr/local/bin/reaper', '-newinst', '-nosplash', '-cfgfile', str(profile()),
                    str(proj), str(HOST_LUA)], env=env, cwd=S, stdout=log, stderr=log)
    end = time.monotonic() + 240
    while time.monotonic() < end and not (S / f'done-{label}').exists() and app.poll() is None: time.sleep(.3)
    assert (S / f'done-{label}').exists(), f'{label}: host did not finish; see run.txt'
    maps = [l for l in Path(f'/proc/{app.pid}/maps').read_text().splitlines() if 'kabl.clap' in l]
    assert maps, 'plugin not mapped'
    app.terminate(); app.wait(timeout=15)  # full quit
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
    p = pcm(path); m = p.mean(1)
    return dict(finite=bool(np.isfinite(p).all()), peak=float(np.abs(p).max()), rms=float(np.sqrt((m ** 2).mean())), _m=m)


result = {'plugin_sha256': hashlib.sha256(PLUGIN.read_bytes()).hexdigest()}
try:
    for slug in slugs:
        secs = int(phrases[slug]['secs']) + 1
        r = result[slug] = {}
        (S / f'{slug}.rpp').write_text(project(slug, True))
        run(f'{slug}-init', S / f'{slug}.rpp', secs)
        run(f'{slug}-restart', S / f'saved-{slug}-init.rpp', secs)
        s0, s1, s2 = (states_of(p.read_text()) for p in (S / f'{slug}.rpp', S / f'saved-{slug}-init.rpp', S / f'saved-{slug}-restart.rpp'))
        # The script writes state version 2; the plugin saves its current version (5) on first save.
        nov = lambda ss: [{k: x for k, x in s.items() if k != 'version'} for s in ss]
        assert f32(nov(s0)) == f32(nov(s1)) == f32(nov(s2)) and f32(s1) == f32(s2), f'{slug}: state changed across save, quit and reopen'
        r['states_identical_across_save_quit_reopen'] = True
        a, b = feats(S / f'render-{slug}-init.wav'), feats(S / f'render-{slug}-restart.wav')
        assert a['finite'] and b['finite'] and a['peak'] < 1.0 and a['rms'] > 0.003, (slug, a)
        n = min(len(a['_m']), len(b['_m']))
        diff = float(np.sqrt(((a['_m'][:n] - b['_m'][:n]) ** 2).mean()) / a['rms'])
        r.update(peak=round(a['peak'], 4), rms=round(a['rms'], 4), restart_relative_difference=round(diff, 5))
        assert diff < 1e-3, f'{slug}: render changed after reopen ({diff})'
        if slug != 'loading-dock':  # plays by itself with the transport running
            (S / f'{slug}-nomidi.rpp').write_text(project(slug, False))
            run(f'{slug}-nomidi', S / f'{slug}-nomidi.rpp', secs)
            q = feats(S / f'render-{slug}-nomidi.wav')
            r['rms_without_midi'] = round(q['rms'], 5)
            # steel-toe keeps its kick without MIDI; the bass is what MIDI adds
            assert q['rms'] < a['rms'] * (0.9 if slug in ('steel-toe', 'rail-yard') else 0.1), f'{slug}: plays without MIDI?'
        print(slug, json.dumps(r))
finally:
    for pr in (wm, x):
        pr.terminate(); pr.wait(timeout=8)
    log.close(); shutil.copy2(S / 'run.txt', OUT / 'run.txt')
    (OUT / 'result.json').write_text(json.dumps(result, indent=1) + '\n')
print(json.dumps(result, indent=1))
