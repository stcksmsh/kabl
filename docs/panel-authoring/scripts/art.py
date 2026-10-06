#!/usr/bin/env python3
"""Original SVG satin faces and PNG exports. MIT OR Apache-2.0, like this repository."""
from pathlib import Path
import cairosvg
root=Path(__file__).resolve().parents[1]/'examples/art'
for kind,colors in [('voice',('#cf947c','#795344')),('stereo',('#a8b99d','#52654f'))]:
 for theme,base in zip(['light','dark'],colors):
  grain=''.join(f'<path d="M1 {y}h418" stroke="#fff" stroke-opacity=".012" stroke-width=".5"/>' for y in range(2,340,4))
  svg=f'''<svg xmlns="http://www.w3.org/2000/svg" width="420" height="340" viewBox="0 0 420 340">
<defs><linearGradient id="coat" x2=".3" y2="1"><stop stop-color="{base}"/><stop offset=".48" stop-color="{base}"/><stop offset="1" stop-color="{base}"/></linearGradient>
<linearGradient id="light" x2="0" y2="1"><stop stop-color="#fff" stop-opacity=".08"/><stop offset=".35" stop-color="#fff" stop-opacity=".01"/><stop offset="1" stop-color="#000" stop-opacity=".07"/></linearGradient></defs>
<rect width="420" height="340" fill="url(#coat)"/><rect width="420" height="340" fill="url(#light)"/>{grain}
<path d="M1 1h418" stroke="#fff" stroke-opacity=".28"/><path d="M1 339h418" stroke="#000" stroke-opacity=".3"/>
<path d="M12 61h396M12 213h396" stroke="#252921" stroke-opacity=".22" stroke-width=".6"/></svg>'''
  path=root/f'{kind}-{theme}.svg';path.write_text(svg)
  cairosvg.svg2png(bytestring=svg.encode(),write_to=str(path.with_suffix('.png')))
