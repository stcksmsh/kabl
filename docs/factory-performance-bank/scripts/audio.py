#!/usr/bin/env python3
"""Encode original engine renders and retain unnormalized measurement receipts."""
from pathlib import Path
import subprocess as sp, shutil, hashlib, json, wave

repo=Path(__file__).resolve().parents[3]
source=repo/'scratch/factory-bank/audio'
out=repo/'docs/factory-performance-bank'
(out/'audio').mkdir(exist_ok=True)
(out/'evidence/audio').mkdir(exist_ok=True)
files=[]
for path in sorted(source.glob('*.wav')):
    dest=out/'audio'/f'{path.stem}.m4a'
    sp.run(['ffmpeg','-v','error','-y','-i',str(path),'-c:a','aac','-b:a','160k',str(dest)],check=True)
    probe=json.loads(sp.check_output(['ffprobe','-v','error','-show_entries','format=duration','-of','json',str(dest)],text=True))
    files.append({'name':path.stem,'original_float_wav_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'preview_sha256':hashlib.sha256(dest.read_bytes()).hexdigest(),'duration_seconds':float(probe['format']['duration'])})
for name in ['metrics.json','performance-metrics.json']:
    shutil.copy2(source/name,out/'evidence/audio'/name)
shutil.copytree(source.parent/'performance-project',out/'performance-project',dirs_exist_ok=True)
(out/'evidence/audio/previews.json').write_text(json.dumps({'input':'Scripted notes/runtime macros and real editable sequencer/cue launches through the production engine. No reference recordings, normalization or limiter. AAC previews are lossy; measurements describe original float WAV.','files':files},indent=2)+'\n')
