#!/usr/bin/env python3
"""Screenshot g4-surfaces.html surfaces 1:1: shot_mockup.py OUTDIR surf[,surf...]

Writes OUTDIR/<surf>__<graphite|paper>__<1440x900|1180x680>.png using headless Chrome (the page
reads its state from localStorage, so each shot is a copy of the page with the state preset)."""
import os, subprocess, sys, tempfile, json
out, surfs = sys.argv[1], sys.argv[2].split(',')
here = os.path.dirname(os.path.abspath(__file__))
src = open(os.path.join(here, '../mockups/g4-surfaces.html')).read()
os.makedirs(out, exist_ok=True)
css = '<style>.wrap>*:not(#out){display:none!important}.wrap{padding:0!important;margin:0!important;max-width:none!important}body{margin:0!important;background:#fff!important}#out,#out.screen{margin:0!important;padding:0!important;border:0!important;border-radius:0!important;box-shadow:none!important;background:none!important}#out svg{display:block;width:100%;height:auto}</style>'
for s in surfs:
    for app in ('graphite', 'paper'):
        for size in ('1440', '1180'):
            w, h = (1440, 900) if size == '1440' else (1180, 680)
            state = json.dumps({'mode': 'one', 'surf': s, 'app': app, 'size': size})
            page = f'{css}<script>localStorage.setItem("kabl-g4",{json.dumps(state)})</script>' + src
            with tempfile.NamedTemporaryFile('w', suffix='.html', delete=False) as f:
                f.write(page)
            png = os.path.abspath(f'{out}/{s}__{app}__{w}x{h}.png')
            subprocess.run(['google-chrome', '--headless=new', '--no-sandbox', '--disable-gpu', f'--window-size={w},{h}',
                            '--virtual-time-budget=4000', f'--screenshot={png}', 'file://' + f.name],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            os.unlink(f.name)
