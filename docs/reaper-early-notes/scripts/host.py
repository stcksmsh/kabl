#!/usr/bin/env python3
"""Smallest REAPER repro for early notes lost with a large plugin state.

    cargo build --release -p kabl-clap
    python3 docs/reaper-early-notes/scripts/host.py PAD_BYTES [NOTE_SECONDS] [offline|realtime]

One track, one kabl instance (init-keyboard patch, one table padded so the saved state is about
PAD_BYTES larger), one note at NOTE_SECONDS. `offline` renders with the host's own renderer;
`realtime` plays the project from the start and records the master with the host (render at
real-time speed through the audio device). Prints the first sound and the RMS per 0.25 s.
Method as docs/cue-macros/scripts/host.py.
"""
from pathlib import Path
import base64, json, os, shutil, struct, subprocess as sp, sys, textwrap, time
import numpy as np

repo = Path(__file__).resolve().parents[3]
PAD = int(sys.argv[1])
AT = float(sys.argv[2]) if len(sys.argv) > 2 else 0.5
MODE = sys.argv[3] if len(sys.argv) > 3 else 'offline'
S = repo / f'scratch/early-notes-{PAD}-{MODE}'
PLUGIN = Path(os.environ.get('FC_PLUGIN', repo / 'target/release/libkabl_clap.so'))
HOST_LUA = Path(__file__).with_name('host.lua')
SECS = 6
shutil.rmtree(S, ignore_errors=True)
S.mkdir(parents=True)

patch = json.loads((repo / 'patches/init-keyboard/checkpoint.json').read_text())
if PAD or os.environ.get('SLOW'):
    # imported-morph carries a real table; cut its sample data to the frames that give PAD base64 characters
    patch = json.loads((repo / 'patches/sound-engines/demos/imported-morph/checkpoint.json').read_text())
    wav = base64.b64decode(patch['tables']['1']['wav'])
    keep = max(1, (PAD or 1) * 3 // 4 // 4096) * 4096
    patch['tables']['1']['wav'] = base64.b64encode(wav[:44 + keep]).decode()
if os.environ.get('SLOW'):  # the wavetable-editor host project: slow attack, factory table selected
    for m in patch['modules'].values():
        if m['kind'] == 'osc.wt':
            m['params'].update({'user': 0.0, 'table': 1.0})
        if m['kind'] == 'env.adsr':
            m['params'].update({'attack_ms': 3500.0, 'decay_ms': 100.0, 'sustain': 1.0, 'release_ms': 400.0})
lanes = [{'retired': False, 'target': None} for _ in range(16)]
state = {'version': 4, 'patch': patch, 'output_gain': .4, 'host_clock': False, 'lanes': lanes, 'slot_values': [0.0] * 16}
raw = json.dumps(state, separators=(',', ':')).encode()
enc = base64.b64encode(len(raw).to_bytes(8, 'little') + raw).decode()
chunk = ''.join('          ' + l + '\n' for l in textwrap.wrap(enc, 120))
tick = round(AT * 1920)
proj = f'''<REAPER_PROJECT 0.1 "7.75/linux-x86_64" 0
  TEMPO 120 4 4
  SAMPLERATE 48000 1 0
  <TRACK
    NAME "early"
    VOLPAN 0.65 0 1 -1
    <FXCHAIN
      SHOW -1
      <CLAP "CLAPi: kabl (stcksmsh)" dev.stcksmsh.kabl ""
        CFG 4 1180 680 ""
        <STATE
{chunk}        >
      >
      WAK 0 0
    >
    <ITEM
      POSITION 0
      LENGTH {SECS}
      <SOURCE MIDI
        HASDATA 1 960 QN
        E {tick} 90 3c 64
        E 960 80 3c 00
      >
    >
  >
>
'''
(S / 'p.rpp').write_text(proj)
DISP = ':%d' % (150 + PAD % 40)
env = dict(os.environ, DISPLAY=DISP, XDG_RUNTIME_DIR='/run/user/1000', KABL_FACTORY_DIR=str(S / 'nf'),
           KABL_USER_DIR=str(S / 'nl'), KABL_SE_HOST=str(S), KABL_SE_SECS=str(SECS), KABL_SE_TAG='r',
           KABL_SE_MODE=MODE, KABL_SE_DELAY=os.environ.get('DELAY', '2'), CLAP_PATH=str(S / 'plugin'), KABL_EARLY_LOG=str(S / 'plugin.log'))
log = open(S / 'run.txt', 'w')
x = sp.Popen(['Xvfb', DISP, '-screen', '0', '1440x900x24', '-nolisten', 'tcp'], stdout=log, stderr=log)
time.sleep(.8)
wm = sp.Popen(['metacity', '--sm-disable'], env=env, stdout=log, stderr=log)
time.sleep(.8)
ok = False
try:
    (S / 'plugin').mkdir()
    (S / 'empty').mkdir()
    shutil.copy2(PLUGIN, S / 'plugin/kabl.clap')
    (S / 'profile').mkdir()
    (S / 'profile/reaper.ini').write_text(f'[reaper]\nvstpath={S}/empty\nvstpath64={S}/empty\nlinux_audio_bsize=256\nlinux_audio_mode=0\nlinux_audio_srate=48000\nwnd_w=1280\nwnd_h=800\nwnd_x=0\nwnd_y=0\n')
    app = sp.Popen(['pw-jack', '/usr/local/bin/reaper', '-newinst', '-nosplash', '-cfgfile', str(S / 'profile/reaper.ini'),
                    str(S / 'p.rpp'), str(HOST_LUA)], env=env, cwd=S, stdout=log, stderr=log)
    end = time.monotonic() + 120
    while time.monotonic() < end and not (S / 'done-r').exists() and app.poll() is None:
        time.sleep(.3)
    ok = (S / 'done-r').exists()
    app.terminate()
    app.wait(timeout=15)
finally:
    for pr in (wm, x):
        pr.terminate()
        pr.wait(timeout=8)
    log.close()
assert ok, 'host did not finish; see ' + str(S / 'run.txt')
wavfile = (S / 'done-r').read_text().strip() if MODE == 'realtime' else str(S / 'render-r.wav')
b = Path(wavfile).read_bytes()
at = 12
fmt = None
while at < len(b):
    cid, size = b[at:at + 4], struct.unpack_from('<I', b, at + 4)[0]
    if cid == b'fmt ':
        fmt = struct.unpack_from('<HHIIHH', b, at + 8)
    if cid == b'data':
        raw_pcm = b[at + 8:at + 8 + size]
        if fmt[5] == 24:  # a recording is 24-bit PCM
            q = np.frombuffer(raw_pcm[:len(raw_pcm) // 3 * 3], np.uint8).reshape(-1, 3).astype(np.int32)
            v = (q[:, 0] | q[:, 1] << 8 | q[:, 2] << 16) << 8 >> 8
            p = (v / 8388608.0).astype('f4').reshape(-1, 2).mean(1)
        else:
            p = np.frombuffer(raw_pcm, '<f4').reshape(-1, 2).mean(1)
        break
    at += 8 + size + (size & 1)
w = 12000
rms = [round(float(np.sqrt((p[i:i + w] ** 2).mean())), 4) for i in range(0, len(p) - w + 1, w)]
idx = np.nonzero(np.abs(p) > 1e-4)[0]
first = float(idx[0]) / 48000 if len(idx) else None
print(f'pad={PAD} state={len(raw)}B note_at={AT} mode={MODE} first_sound={first} rms/0.25s={rms[:16]}')
