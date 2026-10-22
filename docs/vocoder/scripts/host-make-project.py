#!/usr/bin/env python3
"""Writes a REAPER project that holds kabl with a patch in its state, played by a MIDI chord.

    python3 docs/vocoder/scripts/host-make-project.py OUT.rpp PATCH_DIR [--voice VOICE.wav [--dest N]]

PATCH_DIR is a patch folder with a checkpoint.json. With --voice a second track plays VOICE.wav
and sends it to kabl on input channels N+1/N+2 (N = 2 is REAPER's sidechain pair, the default).
"""
import base64, json, sys, textwrap
from pathlib import Path

out = Path(sys.argv[1]).resolve()
patch = json.loads((Path(sys.argv[2]) / 'checkpoint.json').read_text())
args = sys.argv[3:]
voice = Path(args[args.index('--voice') + 1]).resolve() if '--voice' in args else None
dest = int(args[args.index('--dest') + 1]) if '--dest' in args else 2

lanes = [{'target': None, 'retired': False} for _ in range(16)]
state = {'version': 2, 'patch': patch, 'output_gain': 0.6, 'lanes': lanes,
         'slot_values': [0.0] * 16, 'host_clock': False}
raw = json.dumps(state, separators=(',', ':')).encode()
enc = base64.b64encode(len(raw).to_bytes(8, 'little') + raw).decode()
chunk = ''.join('          ' + l + '\n' for l in textwrap.wrap(enc, 120))

# E3 B3 E4 G4 from 0.5 s to 9.5 s (960 ticks per quarter note, 120 bpm: 2 ticks per ms).
notes = [0x34, 0x3B, 0x40, 0x43]
events = ''.join(f'        E {960 if i == 0 else 0} 90 {n:02x} 64\n' for i, n in enumerate(notes))
events += ''.join(f'        E {17280 if i == 0 else 0} 80 {n:02x} 00\n' for i, n in enumerate(notes))

kabl_track = f'''  <TRACK
    NAME "kabl"
    VOLPAN 0.65 0 1 -1
    NCHAN 4
{'    AUXRECV 0 0 1 0 0 0 0 0 %d -1:U 0 -1' % dest if voice else ''}
    <FXCHAIN
      SHOW 0
      <CLAP "CLAPi: kabl (stcksmsh)" dev.stcksmsh.kabl ""
        CFG 4 1180 680 ""
        <STATE
{chunk}        >
      >
      WAK 0 0
    >
    <ITEM
      POSITION 0
      LENGTH 10
      <SOURCE MIDI
        HASDATA 1 960 QN
{events}        E 960 b0 7b 00
      >
    >
  >
'''
voice_track = f'''  <TRACK
    NAME "voice"
    VOLPAN 1 0 1 -1
    MAINSEND 0 0
    <ITEM
      POSITION 0
      LENGTH 10
      <SOURCE WAVE
        FILE "{voice}"
      >
    >
  >
''' if voice else ''

out.write_text(f'''<REAPER_PROJECT 0.1 "7.75/linux-x86_64" 0
  TEMPO 120 4 4
  SAMPLERATE 48000 1 0
  RENDER_FILE "render"
  RENDER_FMT 0 2 48000
  RENDER_RANGE 0 0 10 0 1000
  RENDER_STEMS 0
  RENDER_DITHER 0
  <RENDER_CFG
    ZXZhdyAAAA==
  >
{voice_track}{kabl_track}>
''')
print(out)
