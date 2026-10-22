import base64, json, sys, textwrap
from pathlib import Path
repo = Path('/secondary/Programming/Github/kabl/.worktrees/sound-engines-2')
out = Path(sys.argv[1])
patch = json.loads((repo / 'patches/sound-engines/demos/imported-morph/checkpoint.json').read_text())
patch['tables'] = {}
for m in patch['modules'].values():
    if m['kind'] == 'osc.wt':
        m['params']['user'] = 0.0
        m['params']['table'] = 1.0
    if m['kind'] == 'env.adsr':
        m['params'].update({'attack_ms': 3500.0, 'decay_ms': 100.0, 'sustain': 1.0, 'release_ms': 400.0})
lanes = [{'target': None, 'retired': False} for _ in range(16)]
state = {'version': 4, 'patch': patch, 'output_gain': 0.6, 'lanes': lanes,
         'slot_values': [0.0] * 16, 'host_clock': False}
raw = json.dumps(state, separators=(',', ':')).encode()
enc = base64.b64encode(len(raw).to_bytes(8, 'little') + raw).decode()
chunk = ''.join('          ' + l + '\n' for l in textwrap.wrap(enc, 120))
proj = f'''<REAPER_PROJECT 0.1 "7.75/linux-x86_64" 0
  TEMPO 120 4 4
  SAMPLERATE 48000 1 0
  RENDER_FILE "render"
  RENDER_FMT 0 2 48000
  RENDER_RANGE 0 0 8 0 1000
  RENDER_STEMS 0
  RENDER_DITHER 0
  <RENDER_CFG
    ZXZhdyAAAA==
  >
  <TRACK
    NAME "wavetable editor"
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
      LENGTH 8
      <SOURCE MIDI
        HASDATA 1 960 QN
        E 960 90 30 64
        E 4800 80 30 00
        E 9600 b0 7b 00
      >
    >
  >
>
'''
out.write_text(proj)
print(out)
