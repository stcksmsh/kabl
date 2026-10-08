#!/usr/bin/env python3
"""Actual browser/Perform input on one task-owned X11 display and private library."""
from pathlib import Path
import os, time, json, subprocess as sp, hashlib

repo = Path(__file__).resolve().parents[3]
out = repo / 'docs/factory-performance-bank'
scratch = repo / 'scratch/factory-bank/gui'
scratch.mkdir(parents=True, exist_ok=True)
(out / 'media').mkdir(exist_ok=True)
(out / 'evidence').mkdir(exist_ok=True)
binary = Path(os.environ.get('KABL_BANK_BINARY', repo / 'target/debug/kabl-ui')).resolve()
env = dict(os.environ, DISPLAY=':137', WINIT_X11_SCALE_FACTOR='1', KABL_FACTORY_DIR=str(repo / 'patches'), KABL_USER_DIR=str(scratch / 'library'), KABL_HITS_FILE=str(scratch / 'hits.txt'))
log = open(scratch / 'run.txt', 'w')
x = sp.Popen(['Xvfb', ':137', '-screen', '0', '1600x1000x24', '-nolisten', 'tcp'], stdout=log, stderr=log)
time.sleep(.6)
wm = sp.Popen(['metacity', '--sm-disable'], env=env, stdout=log, stderr=log)
time.sleep(.6)
app = video = None
cases = []
saved_state = None
saved_dir = scratch/'library/sounds/bank-recall'

def run(args):
    return sp.check_output(args, env=env, text=True)

def hits():
    try:
        return {p[0]:list(map(float,p[1:])) for p in map(str.split,(scratch/'hits.txt').read_text().splitlines()) if len(p)==5}
    except FileNotFoundError:
        return {}

def wait(key):
    end = time.monotonic() + 20
    while time.monotonic() < end:
        if key in hits(): return
        assert app.poll() is None
        time.sleep(.15)
    raise AssertionError(('missing', key, list(hits())))

def click(key):
    wait(key)
    a,b,c,d = hits()[key]
    run(['xdotool','mousemove','--window',wid,str(round((a+c)/2)),str(round((b+d)/2)),'click','1'])
    time.sleep(.45)

def query(text):
    click('search')
    run(['xdotool','key','ctrl+a'])
    if text: run(['xdotool','type','--clearmodifiers',text])
    else: run(['xdotool','key','BackSpace'])
    run(['xdotool','key','Tab'])
    time.sleep(.5)

def category(text):
    click('category'); click('category:' + text)

def capture(name):
    run(['import','-window',wid,str(out/'media'/name)])

inventory = [
    ('palette/keyboard-bass','Round Keyboard Bass'),('palette/lead','Singing Lead'),
    ('palette/keys','Glass Keys'),('palette/pad','Evolving Pad'),
    ('palette/strings','Ensemble Strings'),('palette/breath','Breath'),
    ('palette/bass','Sequence Bass'),('interlocking','Interlocking Sequences'),
    ('echo','Echo Sequence'),('palette/progression','Slow Horizons'),
    ('composition','Composition'),('sound-palette','Sound Palette Piece')]

try:
    for size in ['1440x900','1280x800']:
        (scratch/'hits.txt').unlink(missing_ok=True)
        app = sp.Popen([str(binary),'--size',size,'--no-rt'], env=env, stdout=log, stderr=log)
        wait('save'); time.sleep(.8)
        wid = run(['xdotool','search','--onlyvisible','--name','^kabl$']).splitlines()[-1]
        run(['xdotool','windowmove',wid,'0','0'])
        if size == '1440x900':
            video = sp.Popen(['ffmpeg','-y','-f','x11grab','-framerate','12','-video_size','1600x1000','-i',':137','-an','-c:v','libx264','-threads','2','-preset','ultrafast','-crf','30','-pix_fmt','yuv420p',str(out/'media/browser-perform.mp4')], stdout=log, stderr=log)
        click('filter:factory')
        for purpose, expected in [('Sounds',6),('Sequences',4),('Performances',2)]:
            category(purpose); query('curated')
            cards = [k for k in hits() if k.startswith('sound:factory:')]
            assert len(cards) == expected, (purpose, cards)
            capture(f'{size}-{purpose.lower()}-browser.png')
        category('Any')
        tall = False
        for directory,name in inventory:
            query(name)
            click('sound:factory:' + directory); click('open'); time.sleep(.6)
            assert any(k.startswith('pcard:') for k in hits()), (directory, 'Perform not ready')
            if not tall:
                click('perform-size'); tall=True
            state = json.loads((repo/'patches'/directory/'checkpoint.json').read_text())
            clock = next((key for key,value in state['modules'].items() if value['kind']=='clock'),None)
            if clock:
                click('prun:'+clock)
                time.sleep(.3)
                seq = next(key for key,value in state['modules'].items() if value['kind']=='seq')
                # Lower bank/cue cards can be reached through the existing Taller mode.
                bank = 'pbank:' + seq + '.1'
                if bank in hits():
                    a,b,c,d = hits()[bank]
                    if d < int(size.split('x')[1]): click(bank)
                cue = next((key for key in hits() if key.startswith('pcue:') and key.endswith('.2')),None)
                if cue: click(cue)
                click('prun:'+clock)
            else:
                click('play'); time.sleep(.2); click('stop-preview')
            if directory in ['palette/keys','echo','composition','sound-palette']:
                for theme in ['light','dark']:
                    click('theme:'+theme)
                    capture(f'{size}-{directory.replace("/","-")}-{theme}.png')
            if directory=='palette/keys' and size=='1440x900':
                macro=next(key for key,value in state['modules'].items() if value['kind']=='macro')
                click('pslider:'+macro+'.m1')
                click('save-as'); click('dlg:name')
                run(['xdotool','key','ctrl+a']);run(['xdotool','type','--clearmodifiers','Bank Recall'])
                click('dlg:save');time.sleep(.6)
                saved_state=json.loads((saved_dir/'checkpoint.json').read_text())
                assert saved_state['modules'][macro]['params']['m1'] != state['modules'][macro]['params']['m1']
            cases.append({'size':size,'factory_id':'factory:'+directory,'browser_open':True,'Perform_cards':len([k for k in hits() if k.startswith('pcard:')]),'transport_or_audition_exercised':True})
        if video:
            video.send_signal(2); video.wait(timeout=10); video=None
        app.terminate(); app.wait(timeout=8); app=None
    (scratch/'hits.txt').unlink(missing_ok=True)
    app=sp.Popen([str(binary),'--patch',str(saved_dir),'--size','1280x800','--no-rt'],env=env,stdout=log,stderr=log)
    wait('save');time.sleep(.8)
    wid=run(['xdotool','search','--onlyvisible','--name','^kabl$']).splitlines()[-1]
    click('perform');click('save');time.sleep(.6)
    assert saved_state==json.loads((saved_dir/'checkpoint.json').read_text()),'Full restart changed complete state'
    assert any(key.startswith('pcard:') for key in hits())
    capture('standalone-full-restart.png')
    app.terminate();app.wait(timeout=8);app=None
    (out/'evidence/gui.json').write_text(json.dumps({'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'standalone_edit_save_full_restart_complete_state_equal':True,'input':'Scripted private X11 input; actual application frames, not owner or physical-controller acceptance. Video is silent; audio evidence is separate.','cases':cases},indent=2)+'\n')
finally:
    if video and video.poll() is None: video.send_signal(2); video.wait(timeout=10)
    if app and app.poll() is None: app.terminate(); app.wait(timeout=8)
    wm.terminate(); wm.wait(timeout=5)
    x.terminate(); x.wait(timeout=5)
    log.close()
