#!/usr/bin/env python3
"""Real-REAPER check of the drum voices in the CLAP plugin, triggered from a MIDI item.

    cargo build --release -p kabl-clap
    python3 docs/drums/scripts/host_drums.py

Same method as docs/sound-engines/scripts/host_rhythm.py (private Xvfb display and REAPER profile,
project <STATE> chunks written by this script, factory and user library made unavailable;
host_rhythm.lua saves the project and renders the master mix offline). One track holds a MIDI
drum kit: six `midi.note` modules (General MIDI 36, 38, 39, 42, 46, 45) into the five voices.
Two projects:

- `single`: one drum at a time on the beats of a 120 bpm bar pair (kick, snare, clap, closed hat,
  open hat, tom, then the kick at velocity 127 and 30). Each hit's first sample is found in the
  render and compared with the sample the MIDI note is on.
- `groove`: four bars of kick, snare, clap, 16th hats and an open hat from the MIDI item. The kick
  is found in the low band and compared with the beat grid; the render is saved and quit and
  reopened (state equal, second render bit-identical).
"""
from pathlib import Path
import base64, hashlib, json, os, re, shutil, struct, subprocess as sp, textwrap, time, uuid
import numpy as np

repo = Path(__file__).resolve().parents[3]
S = repo / 'scratch/drums-host'
OUT = repo / 'docs/drums/evidence/host'
NEW = Path(os.environ.get('SE_PLUGIN', repo / 'target/release/libkabl_clap.so'))
OLD_PROJECT = repo / 'docs/factory-performance-bank/evidence/host/arrangement.rpp'
LUA = repo / 'docs/sound-engines/scripts/host_rhythm.lua'
SR = 48000
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


def kit_patch():
    """Six midi.note -> kick, snare, clap, hat (closed, open), tom -> two mixers -> out."""
    mods, cables = {}, {}

    def add(kind, params=None):
        i = len(mods) + 1
        mods[str(i)] = {'kind': kind, 'pos': {'x': 24.0 + 190.0 * ((i - 1) % 6), 'y': 20.0 + 300.0 * ((i - 1) // 6)},
                        'params': params or {}}
        return i

    def wire(a, ap, b, bp):
        cables[str(len(cables) + 1)] = {'from': {'Module': {'id': a, 'port': ap}}, 'to': {'Module': {'id': b, 'port': bp}},
                                        'params': {}, 'steps': []}

    notes = {n: add('midi.note', {'note': float(n)}) for n in (36, 38, 39, 42, 46, 45)}
    kick, snare, clap = add('drum.kick'), add('drum.snare', {'level_db': -6.0}), add('drum.clap', {'level_db': -6.0})
    hat, tom = add('drum.hat', {'level_db': -6.0}), add('drum.tom', {'level_db': -5.0})
    mix = add('mixer', {'level1': .9, 'level2': .6, 'level3': .5, 'level4': .55})
    mix2 = add('mixer', {'level1': 1.0, 'level2': .7})
    out = add('out')
    for n, v, port in ((36, kick, 'gate'), (38, snare, 'gate'), (39, clap, 'gate'), (42, hat, 'closed'),
                       (46, hat, 'open'), (45, tom, 'gate')):
        wire(notes[n], 'gate', v, port)
        if n != 46:
            wire(notes[n], 'velocity', v, 'velocity')
    for i, v in enumerate((kick, snare, clap, hat), 1):
        wire(v, 'out', mix, f'in{i}')
    wire(mix, 'out', mix2, 'in1'); wire(tom, 'out', mix2, 'in2')
    wire(mix2, 'out', out, 'left'); wire(mix2, 'out', out, 'right')
    return {'modules': mods, 'cables': cables}


def midi_item(notes):
    """`notes`: (note, start beat, end beat, velocity). Source lines in 960-tick quarter notes."""
    ev = sorted([(int(a * 960), 0x90, n, v) for n, a, b, v in notes] + [(int(b * 960), 0x80, n, 0) for n, a, b, v in notes],
                key=lambda e: (e[0], e[1] == 0x90))
    lines, at = [], 0
    for t, st, n, v in ev:
        lines.append(f'        E {t - at} {st:x} {n:02x} {v:02x}'); at = t
    return '\n'.join(lines)


def build_project(name, patch, notes):
    proj = re.sub(r'\n      <PARMENV[\s\S]*?\n      >', '', OLD_PROJECT.read_text())
    # No tempo envelope: 120 bpm throughout, so a beat is exactly 24000 samples.
    proj = re.sub(r'\n    <TEMPOENVEX[\s\S]*?\n    >', '', proj)
    i1 = proj.index('\n  <TRACK'); i2 = proj.index('\n  <TRACK', i1 + 1); last = proj.rindex('\n>')
    header, first, footer = proj[:i1], proj[i1:i2], proj[last:]
    guid = r'\{[0-9A-Fa-f-]{36}\}'
    t = re.sub(guid, lambda m: '{' + str(uuid.uuid4()).upper() + '}', first)
    t = re.sub(r'(NAME )"[^"\n]*"', lambda m: m.group(1) + f'"{name}"', t, count=1)
    t = re.sub(r'<STATE\s+.*?\s*>', lambda m: chunk_of(state_for(patch)), t, flags=re.S)
    t = re.sub(r'\n        POOLEDEVTS [^\n]*', '', t)
    t = re.sub(r'(\n        E [^\n]*)+', '\n' + midi_item(notes), t, count=1)
    t = re.sub(r'LENGTH \d+', 'LENGTH 30', t, count=1)
    t = re.sub(r'LOOP 1', 'LOOP 0', t, count=1)
    return header + t + footer


# A beat is 0.5 s at 120 bpm; nothing starts at 0 so each hit has silence before it.
SINGLE = [(36, 2, 2.25, 100), (38, 4, 4.25, 100), (39, 6, 6.25, 100), (42, 8, 8.25, 100), (46, 10, 10.25, 100),
          (45, 12, 12.25, 100), (36, 14, 14.25, 127), (36, 16, 16.25, 30)]
GROOVE = []
for bar in range(4):
    b = bar * 4 + 1
    for k in range(4):
        GROOVE.append((36, b + k, b + k + .25, 110))
        GROOVE.append((42, b + k + .25, b + k + .4, 70)); GROOVE.append((42, b + k + .75, b + k + .9, 90))
        GROOVE.append((46, b + k + .5, b + k + .65, 100) if k != 3 else (42, b + k + .5, b + k + .65, 100))
    GROOVE += [(38, b + 1, b + 1.25, 110), (39, b + 1, b + 1.25, 100), (38, b + 3, b + 3.25, 110), (39, b + 3, b + 3.25, 100)]
(S / 'single.rpp').write_text(build_project('Drum kit', kit_patch(), SINGLE))
(S / 'groove.rpp').write_text(build_project('Drum kit', kit_patch(), GROOVE))

DISP = ':174'
env0 = dict(os.environ, DISPLAY=DISP, XDG_RUNTIME_DIR='/run/user/1000', KABL_FACTORY_DIR=str(S / 'unavailable-factory'),
            KABL_USER_DIR=str(S / 'unavailable-library'), KABL_SE_HOST=str(S))
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
    """Two causal one-pole low-passes (a zero-phase filter would ring before the hit)."""
    k = 1 - np.exp(-2 * np.pi * hz / SR)
    for _ in range(2):
        y = np.empty_like(m); acc = 0.0
        for i, v in enumerate(m):
            acc += k * (v - acc); y[i] = acc
        m = y
    return m


result = {'plugin_sha256': hashlib.sha256(NEW.read_bytes()).hexdigest()}
try:
    run('single', S / 'single.rpp', secs=10)
    a = pcm(S / 'render-single.wav').mean(1)
    assert np.isfinite(a).all()
    rows = []
    for n, beat, _, vel in SINGLE:
        want = int(beat * 24000)
        win = a[want - 480:want + 12000]
        hit = np.nonzero(np.abs(win) > 1e-5)[0]
        first = int(hit[0]) - 480 if len(hit) else None
        pk = float(np.abs(a[want:want + 4800]).max())
        spec = np.abs(np.fft.rfft(a[want:want + 4096] * np.hanning(4096)))
        cent = float((spec * np.fft.rfftfreq(4096, 1 / SR)).sum() / max(spec.sum(), 1e-9))
        rows.append(dict(note=n, velocity=vel, expected_sample=want, first_sample_offset=first, peak=round(pk, 3), centroid_hz=round(cent)))
    result['single'] = rows
    for r in rows:
        assert r['first_sample_offset'] is not None and 0 <= r['first_sample_offset'] <= 2, r
    kick_loud, kick_soft = rows[6]['peak'], rows[7]['peak']
    assert kick_soft < kick_loud * 0.6, rows
    assert rows[0]['centroid_hz'] < 400 < rows[3]['centroid_hz'], rows

    run('groove', S / 'groove.rpp', secs=34)
    run('groove-restart', S / 'saved-groove.rpp', secs=34)
    s0, s1, s2 = (states_of(p.read_text()) for p in (S / 'groove.rpp', S / 'saved-groove.rpp', S / 'saved-groove-restart.rpp'))
    assert f32(s0) == f32(s1) == f32(s2)
    g, g2 = pcm(S / 'render-groove.wav'), pcm(S / 'render-groove-restart.wav')
    m = g.mean(1)
    low = np.abs(lowpass(m, 120))
    kicks = []
    for k in range(16):
        want = (k + 1) * 24000
        kicks.append(int(np.argmax(np.abs(low[want:want + 2400]))))  # the low band's peak after the hit
    med = int(np.median(kicks))
    result['groove'] = dict(finite=bool(np.isfinite(g).all()), peak=float(np.abs(g).max()), rms=float(np.sqrt((g ** 2).mean())),
                            kick_onset_offsets_samples=kicks, kick_median_offset_samples=med, restart_render_bit_identical=bool(g.shape == g2.shape and (g == g2).all()))
    # The same filter rise on every kick: no jitter beyond a few samples from the median.
    assert max(abs(k - med) for k in kicks) <= 24, kicks
    assert result['groove']['restart_render_bit_identical']
finally:
    for pr in (wm, x):
        pr.terminate(); pr.wait(timeout=8)
    log.close(); shutil.copy2(S / 'run.txt', OUT / 'run.txt')
    (OUT / 'result.json').write_text(json.dumps(result, indent=1) + '\n')
print(json.dumps(result, indent=1))
