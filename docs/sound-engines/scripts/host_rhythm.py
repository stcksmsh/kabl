#!/usr/bin/env python3
"""Real-REAPER check of swing, the arpeggiator and ratchets in the CLAP plugin.

    cargo build --release -p kabl-clap
    SE_BASELINE_PLUGIN=/path/to/libkabl_clap.so-built-from-origin/master \
        python3 docs/sound-engines/scripts/host_rhythm.py

Same method as host_util.py (private Xvfb display and REAPER profile, project <STATE> chunks
written by this script, factory and user library made unavailable; host_rhythm.lua saves the
project and renders the master mix offline, then REAPER is quit completely and started again on
what it saved). Plugin states have `host_clock` on, so the clock follows the host's position and
tempo. Four projects:

- `all3`: swing-seq, arp-chord and ratchet-roll on three tracks, 20 s, the template's tempo changes (120, 140 from 8 s, 100 from 16.6 s). Saved, quit, reopened, saved again: states equal, second render bit-identical.
- `swing`: the swing-seq demo alone. Hi-hat onsets are found in the render and compared with where
  the swung clock puts them, from the project's tempo map: 0-20 s, and again from a render that
  starts at 8.2 s (a jump of the transport, as a loop makes).
- `arp`: the arp-chord demo with its sound made plain (sine, short envelope, no delay), a chord
  held 0-4.5 s and another 8.5-18.4 s from the project's MIDI item, swing on: note onsets against
  the clock, the same two renders.
- the factory-bank project from before, through this build and the baseline build.
"""
from pathlib import Path
import base64, hashlib, json, os, re, shutil, struct, subprocess as sp, textwrap, time, uuid
import numpy as np

repo = Path(__file__).resolve().parents[3]
S = repo / 'scratch/sound-engines-host-rhythm'
OUT = repo / 'docs/sound-engines/evidence/host-rhythm'
NEW = Path(os.environ.get('SE_PLUGIN', repo / 'target/release/libkabl_clap.so'))
BASE = Path(os.environ['SE_BASELINE_PLUGIN'])
OLD_PROJECT = repo / 'docs/factory-performance-bank/evidence/host/arrangement.rpp'
SWING = 58.0
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
    return {'version': 2, 'patch': patch, 'output_gain': .4, 'host_clock': True,
            'lanes': [{'retired': False, 'target': None} for _ in range(16)], 'slot_values': [0.0] * 16}


def mod(patch, kind):
    return next(m for m in patch['modules'].values() if m['kind'] == kind)


def probe_arp():
    """The arp-chord demo with a plain sound: onsets are easy to find."""
    p = demo('arp-chord')
    mod(p, 'clock')['params']['swing'] = SWING
    mod(p, 'arp')['params'].update(mode=0.0, octaves=2.0, gate_len=40.0)
    mod(p, 'osc.va')['params'].update(waveform=0.0, unison=1.0)
    mod(p, 'env.adsr')['params'].update(attack_ms=1.0, decay_ms=50.0, sustain=0.0, release_ms=10.0)
    mod(p, 'delay')['params']['mix'] = 0.0
    for c in p['cables'].values():  # the envelope no longer needs to move the filter
        if 'Param' in c['to']: c['params']['amount'] = 0.0
    return p


def midi_item(notes):
    """`notes`: (note, start beat, end beat). Source lines in 960-tick quarter notes."""
    ev = sorted([(int(a * 960), 0x90, n, 100) for n, a, b in notes] + [(int(b * 960), 0x80, n, 0) for n, a, b in notes],
                key=lambda e: (e[0], e[1] == 0x90))
    lines, at = [], 0
    for t, st, n, v in ev:
        lines.append(f'        E {t - at} {st:x} {n:02x} {v:02x}'); at = t
    return '\n'.join(lines)


CHORD = [(50, 0, 9), (53, 0, 9), (57, 0, 9), (60, 0, 9), (46, 17, 40), (50, 17, 40), (53, 17, 40), (57, 17, 40)]
# The template project carries a tempo envelope (square points); the beat-to-seconds map is built
# from it, so the checks use the host's own tempo changes (120, 140 from 8 s, 100 from 16.57 s).
TEMPO = [(float(t), float(v)) for t, v in re.findall(r'\n\s+PT ([0-9.]+) ([0-9.]+) 1 ', re.search(r'<TEMPOENVEX.*?\n  >', OLD_PROJECT.read_text(), re.S).group(0))]
assert len(TEMPO) >= 2 and TEMPO[0][0] == 0.0, TEMPO


def secs_at(beat):
    t, left = 0.0, beat
    for i, (t0, bpm) in enumerate(TEMPO):
        end = TEMPO[i + 1][0] if i + 1 < len(TEMPO) else float('inf')
        span = (end - t0) * bpm / 60.0
        if left <= span: return t0 + left * 60.0 / bpm
        left -= span
    raise ValueError(beat)


def build_project(tracks):
    """`tracks`: (name, patch, midi notes or None). Cloned from the first track of the old project."""
    proj = re.sub(r'\n      <PARMENV[\s\S]*?\n      >', '', OLD_PROJECT.read_text())
    i1 = proj.index('\n  <TRACK'); i2 = proj.index('\n  <TRACK', i1 + 1); last = proj.rindex('\n>')
    header, first, footer = proj[:i1], proj[i1:i2], proj[last:]
    guid = r'\{[0-9A-Fa-f-]{36}\}'
    out = header
    for name, patch, notes in tracks:
        t = re.sub(guid, lambda m: '{' + str(uuid.uuid4()).upper() + '}', first)
        t = re.sub(r'(NAME )"[^"\n]*"', lambda m: m.group(1) + f'"{name}"', t, count=1)
        t = re.sub(r'<STATE\s+.*?\s*>', lambda m: chunk_of(state_for(patch)), t, flags=re.S)
        if notes is not None:
            t = re.sub(r'\n        POOLEDEVTS [^\n]*', '', t)
            t = re.sub(r'(\n        E [^\n]*)+', '\n' + midi_item(notes), t, count=1)
            t = re.sub(r'LENGTH \d+', 'LENGTH 30', t, count=1)
            t = re.sub(r'LOOP 1', 'LOOP 0', t, count=1)
        out += t
    return out + footer


all3 = [('Swing Seq', demo('swing-seq'), CHORD), ('Arp Chord', demo('arp-chord'), CHORD), ('Ratchet Roll', demo('ratchet-roll'), CHORD)]
(S / 'all3.rpp').write_text(build_project(all3))
(S / 'swing.rpp').write_text(build_project([('Swing Seq', demo('swing-seq'), CHORD)]))
(S / 'arp.rpp').write_text(build_project([('Arp probe', probe_arp(), CHORD)]))

DISP = ':151'
env0 = dict(os.environ, DISPLAY=DISP, XDG_RUNTIME_DIR='/run/user/1000', KABL_FACTORY_DIR=str(S / 'unavailable-factory'),
            KABL_USER_DIR=str(S / 'unavailable-library'), KABL_SE_HOST=str(S))
log = open(S / 'run.txt', 'w')
x = sp.Popen(['Xvfb', DISP, '-screen', '0', '1440x900x24', '-nolisten', 'tcp'], stdout=log, stderr=log); time.sleep(.8)
wm = sp.Popen(['metacity', '--sm-disable'], env=env0, stdout=log, stderr=log); time.sleep(.8)


def profile(name):
    d = S / f'profile-{name}'; d.mkdir(exist_ok=True); (S / 'empty-vst').mkdir(exist_ok=True)
    (d / 'reaper.ini').write_text(f'[reaper]\nvstpath={S}/empty-vst\nvstpath64={S}/empty-vst\nlinux_audio_bsize=256\nlinux_audio_mode=0\nlinux_audio_srate=48000\nwnd_w=1280\nwnd_h=800\nwnd_x=0\nwnd_y=0\n')
    return d / 'reaper.ini'


def run(plugin, label, project, secs=20, start=0.0, tempo=False):
    key = 'base' if plugin == BASE else 'new'
    pdir = S / f'plugins-{key}'; pdir.mkdir(exist_ok=True)
    shutil.copy2(plugin, pdir / 'kabl.clap')
    env = dict(env0, CLAP_PATH=str(pdir), KABL_SE_TAG=label, KABL_SE_SECS=str(secs), KABL_SE_START=str(start),
               KABL_SE_TEMPO='1' if tempo else '0')
    app = sp.Popen(['pw-jack', '/usr/local/bin/reaper', '-newinst', '-nosplash', '-cfgfile', str(profile(key)),
                    str(project), str(repo / 'docs/sound-engines/scripts/host_rhythm.lua')], env=env, cwd=S, stdout=log, stderr=log)
    end = time.monotonic() + 150
    while time.monotonic() < end and not (S / f'done-{label}').exists() and app.poll() is None: time.sleep(.3)
    assert (S / f'done-{label}').exists(), f'{label}: host did not finish; see run.txt'
    maps = [l for l in Path(f'/proc/{app.pid}/maps').read_text().splitlines() if 'kabl.clap' in l]
    assert maps, 'plugin not mapped'
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


def mono(label): return pcm(S / f'render-{label}.wav').mean(1)


def onsets(m, hf, secs_lo, secs_hi, gap=30, frac=0.35):
    """Times (s, from render start) where the 1 ms envelope rises by more than `frac` of its
    99th-percentile rise over its minimum of the 6 ms before, at least `gap` ms apart. `hf`: look
    at the first difference (the hat) instead of the level."""
    y = np.abs(np.diff(m, prepend=0.0) if hf else m)
    env = np.maximum.reduceat(y, np.arange(0, len(y), 48))
    base = np.array([env[max(0, i - 6):i].min() if i else 0.0 for i in range(len(env))])
    rise = env - base
    thr = frac * np.percentile(rise, 99)
    out, last = [], -99
    for i, v in enumerate(rise):
        if v > thr and i - last >= gap: out.append(i * 0.001)
        if v > thr: last = i
    return [t for t in out if secs_lo <= t <= secs_hi]


def pulse_time(n, swing):
    s = swing / 200.0
    return secs_at((2 * (n // 2) + (n % 2) * (1 + s)) / 4.0)


def compare_onsets(found, offset, swing, windows, label):
    """Match each found onset (+ render start `offset`) to the nearest expected pulse in `windows`
    (project seconds). Returns the spread of (found - expected) after removing its median, ms."""
    exp = np.array([pulse_time(n, swing) for n in range(0, 400)])
    dev, miss = [], 0
    for t in found:
        tt = t + offset
        if not any(a <= tt <= b for a, b in windows): continue
        j = int(np.abs(exp - tt).argmin()); dev.append(tt - exp[j])
    dev = np.array(dev)
    med = float(np.median(dev))
    straight = np.array([secs_at(n / 4.0) for n in range(0, 400)])
    d0 = np.array([t + offset - straight[int(np.abs(straight - (t + offset)).argmin())] for t in found
                   if any(a <= t + offset <= b for a, b in windows)])
    return dict(label=label, n=len(dev), median_offset_ms=round(med * 1000, 2),
                max_dev_from_median_ms=round(float(np.abs(dev - med).max()) * 1000, 2),
                swing_effect_ms=round(float(np.abs(d0).max()) * 1000, 1))


result = {'plugin_sha256': hashlib.sha256(NEW.read_bytes()).hexdigest(), 'baseline_sha256': hashlib.sha256(BASE.read_bytes()).hexdigest()}
try:
    # --- three tracks: save, quit, reopen, tempo change ---------------------------------------
    run(NEW, 'all3-initial', S / 'all3.rpp', tempo=False)
    run(NEW, 'all3-restart', S / 'saved-all3-initial.rpp')
    s0, s1, s2 = (states_of(p.read_text()) for p in (S / 'all3.rpp', S / 'saved-all3-initial.rpp', S / 'saved-all3-restart.rpp'))
    assert len(s0) == len(s1) == len(s2) == 3
    assert f32(s0) == f32(s1), 'state changed on first save'
    assert f32(s1) == f32(s2), 'state changed on reopen and save'
    result['all3_states_identical_across_save_quit_reopen'] = True
    a, b = pcm(S / 'render-all3-initial.wav'), pcm(S / 'render-all3-restart.wav')
    result['all3'] = dict(finite=bool(np.isfinite(a).all()), peak=float(np.abs(a).max()), rms=float(np.sqrt((a ** 2).mean())),
                          reopen_render_bit_identical=bool(a.shape == b.shape and (a == b).all()),
                          sha256=hashlib.sha256(a.tobytes()).hexdigest())
    assert result['all3']['reopen_render_bit_identical']
    # --- swing: hat onsets against the swung clock under the tempo map ------------------------
    run(NEW, 'swing-initial', S / 'swing.rpp', tempo=False)
    run(NEW, 'swing-jump', S / 'saved-swing-initial.rpp', secs=11.8, start=8.2)
    full = onsets(mono('swing-initial'), True, 0.2, 19.8)
    jump = onsets(mono('swing-jump'), True, 0.2, 11.5)
    result['swing_full'] = compare_onsets(full, 0.0, SWING, [(0.3, 19.7)], 'swing 0-20 s, the project tempo map')
    result['swing_jump'] = compare_onsets(jump, 8.2, SWING, [(8.5, 19.7)], 'swing, render from 8.2 s')
    for k in ('swing_full', 'swing_jump'):
        assert result[k]['n'] > 50 and result[k]['max_dev_from_median_ms'] < 2.0 and result[k]['swing_effect_ms'] > 25, result[k]
    # --- arpeggiator: note onsets against the same clock --------------------------------------
    run(NEW, 'arp-initial', S / 'arp.rpp', tempo=False)
    run(NEW, 'arp-jump', S / 'saved-arp-initial.rpp', secs=11.8, start=8.2)
    af = onsets(mono('arp-initial'), False, 0.2, 19.8)
    aj = onsets(mono('arp-jump'), False, 0.2, 11.5)
    # Chord windows in project seconds, away from the edges where the chord starts and stops.
    win = [(secs_at(0) + 0.4, secs_at(9) - 0.3), (secs_at(17) + 0.4, secs_at(40) - 0.3)]
    result['arp_full'] = compare_onsets(af, 0.0, SWING, win, 'arp 0-20 s, the project tempo map')
    result['arp_jump'] = compare_onsets(aj, 8.2, SWING, [win[1]], 'arp, render from 8.2 s')
    for k in ('arp_full', 'arp_jump'):
        assert result[k]['n'] > 40 and result[k]['max_dev_from_median_ms'] < 2.0 and result[k]['swing_effect_ms'] > 25, result[k]
    # --- the older project is unchanged by this build -----------------------------------------
    run(NEW, 'old-new', OLD_PROJECT, secs=24)
    run(BASE, 'old-base', OLD_PROJECT, secs=24)
    o_orig, o_new = states_of(OLD_PROJECT.read_text()), states_of((S / 'saved-old-new.rpp').read_text())
    result['old_state_identical_new_plugin'] = f32(o_orig) == f32(o_new)
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
