#!/usr/bin/env python3
"""Macro audit: measures what each macro (CC 20-23) of a category does, from offline renders.

    cargo build --release -p kabl-ui --example macro_audit --example presets_groove
    target/release/examples/presets_groove phrases scratch/groove-phrases.json
    target/release/examples/macro_audit patches scratch/groove-phrases.json scratch/macro-audit SLUG ...
    python3 docs/presets/scripts/macro_audit.py --category Groove --renders scratch/macro-audit \
        --patches patches --clips docs/presets/clips/macro-audit/groove --out scratch/macro-audit-report SLUG ...

Reads <renders>/<slug>.json (routes) and the step renders (macro alone at 0, 1/7 ... 1) and the
sweep renders (macro 0 -> 1 -> 0 over 8 s) that the `macro_audit` example wrote. Writes to --out:
audit.json (every number) and tables.md (one row per macro, the flags, the clip start times).
With --clips it also encodes one mp3 per macro sweep, one tour per preset (its macros back to back)
and one tour of everything, and lists the start times. Needs numpy, scipy, soundfile, ffmpeg.

Flags (a draft verdict; a person reads the numbers and decides)
  dead       two or more neighbouring step pairs (of 7) that differ by less than DEAD_DB while 0
             against 1 differs by more than WEAK_DB, or 0 against 1 differs by less than DEAD_DB.
  jump       one neighbouring pair differs by more than JUMP_FACTOR times the median pair and more
             than JUMP_DB.
  weak       0 against 1 differs by less than WEAK_DB.
Difference of two renders (`change`, dB): the larger of the spectral distance (mean absolute
difference of the third-octave levels of the long-term spectrum, over bands within 50 dB of the
loudest) and the envelope distance (mean absolute difference of the 10 ms level, over frames
within 40 dB of the peak). A phase change alone is 0.
  bad end    at 0 or 1: peak above PEAK_DB, loudness under SILENT_DB, or loudness 12 dB under the
             middle or 9 dB over it; a CLICK in the end render.
  zipper     a CLICK in the sweep that the end renders do not have (audio_defects.clicks).
  duplicate  two macros of one preset move the same measures the same way (cosine of the change
             vectors over DUP_COS, both moving more than 1.5 dB), or drive the same module.param.
"""
import argparse, json, subprocess as sp, sys, tempfile
from pathlib import Path
import numpy as np
import soundfile as sf
from scipy.signal import butter, find_peaks, sosfilt, welch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import audio_defects as ad

SR = 48000
STEPS = 8
DEAD_DB = 0.15
WEAK_DB = 1.0
WAVE_DB = -15.0  # 0 against 1 differs in the waveform by more than this: it moves
WAVE_DEAD_DB = -40.0  # a step pair that differs in the waveform by less than this does not move
JUMP_FACTOR = 3.0
JUMP_DB = 1.5
PEAK_DB = -1.0
SILENT_DB = -45.0
DUP_COS = 0.9
BANDS = [(None, 250), (250, 2000), (2000, 6000), (6000, None)]
BAND_NAMES = ['low (<250 Hz)', 'mid (250 Hz-2 kHz)', 'upper (2-6 kHz)', 'air (>6 kHz)']
LEG_FADE = 0.02  # s, fade at the join of tour clips
GAP = 1.0  # s of silence between clips of a tour


def db(v):
    return 20 * np.log10(max(float(v), 3e-5))  # floor -90 dBFS


def band_filters():
    out = []
    for lo, hi in BANDS:
        if lo is None:
            out.append(butter(4, hi, 'lp', fs=SR, output='sos'))
        elif hi is None:
            out.append(butter(4, lo, 'hp', fs=SR, output='sos'))
        else:
            out.append(butter(4, [lo, hi], 'bp', fs=SR, output='sos'))
    return out


FILTERS = band_filters()


def measures(x):
    """x: (n, 2). Peak, loudness, four band loudnesses, centroid (Hz), side level, onsets per second."""
    mono, side = x.mean(1), (x[:, 0] - x[:, 1]) / 2
    rms = lambda v: float(np.sqrt(np.mean(v ** 2)))
    bands = [db(rms(sosfilt(f, mono))) for f in FILTERS]
    f, p = welch(mono, SR, nperseg=4096)
    cent = float((f * p).sum() / max(p.sum(), 1e-30))
    frame = SR // 100
    env = np.sqrt(np.mean(mono[:len(mono) // frame * frame].reshape(-1, frame) ** 2, 1))
    edb = 20 * np.log10(np.maximum(env, 1e-6))
    pk, _ = find_peaks(edb, prominence=8, distance=4)
    pk = [i for i in pk if edb[i] > db(rms(mono)) - 25]
    return {
        'peak': db(np.abs(x).max()), 'loud': db(rms(mono)), 'bands': bands, 'centroid': cent,
        'side': db(rms(side)), 'onsets': len(pk) / (len(mono) / SR),
    }


EDGES = 1000 * 2.0 ** (np.arange(-14, 13) / 3)  # third-octave edges, 40 Hz to 16 kHz


def tob(x):
    """Long-term third-octave levels (dB) of the mono mix."""
    f, p = welch(x.mean(1), SR, nperseg=8192)
    return np.array([10 * np.log10(max(p[(f >= lo) & (f < hi)].sum(), 1e-20)) for lo, hi in zip(EDGES[:-1], EDGES[1:])])


def env(x):
    m = x.mean(1)
    n = SR // 100
    return 20 * np.log10(np.maximum(np.sqrt(np.mean(m[:len(m) // n * n].reshape(-1, n) ** 2, 1)), 1e-6))


def rel(a, b, ta=None, tb=None):
    """Difference of two renders, dB: the larger of the spectral and the envelope distance."""
    ta, tb = (tob(a) if ta is None else ta), (tob(b) if tb is None else tb)
    top = max(ta.max(), tb.max())
    keep = np.maximum(ta, tb) > top - 50
    sd = float(np.abs(ta - tb)[keep].mean())
    ea, eb = env(a), env(b)
    k = np.maximum(ea, eb) > max(ea.max(), eb.max()) - 40
    ed = float(np.abs(ea - eb)[k].mean())
    return max(sd, ed)


def shifts(ta, tb):
    """The biggest rise and fall of the third-octave levels from ta to tb, as words."""
    top = max(ta.max(), tb.max())
    d = np.where(np.maximum(ta, tb) > top - 40, tb - ta, 0.0)
    d = np.convolve(d, np.ones(3) / 3, 'same')
    out = []
    for i in (int(d.argmax()), int(d.argmin())):
        if abs(d[i]) >= 2.0:
            out.append(f"{d[i]:+.1f} dB near {np.sqrt(EDGES[i] * EDGES[i + 1]):.0f} Hz")
    return out


def vector(m0, m1):
    """Change from m0 to m1 over the measures a listener hears: loudness, four bands, width, density."""
    return np.array([m1['loud'] - m0['loud'], *[b1 - b0 for b0, b1 in zip(m0['bands'], m1['bands'])],
                     m1['side'] - m0['side'], 3 * np.log2((m1['onsets'] + 1) / (m0['onsets'] + 1))])


def words(m0, mm, m1, name_vec, shifts_text):
    """Plain words for the change 0 -> 1 from the measures."""
    d = name_vec
    out = []
    if abs(d[0]) >= 1.5:
        out.append(f"{'louder' if d[0] > 0 else 'quieter'} by {abs(d[0]):.1f} dB")
    low, mid, up, air = d[1], d[2], d[3], d[4]
    hi = (up + air) / 2
    if abs(hi - low) >= 3 or abs(hi) >= 3:
        if hi - low >= 3:
            out.append(f"brighter (top {hi:+.1f} dB, low {low:+.1f} dB)")
        elif hi - low <= -3:
            out.append(f"darker (top {hi:+.1f} dB, low {low:+.1f} dB)")
        else:
            out.append(f"top {hi:+.1f} dB")
    elif abs(low) >= 3:
        out.append(f"low end {low:+.1f} dB")
    if abs(mid) >= 3 and abs(hi) < 3:
        out.append(f"mid {mid:+.1f} dB")
    if m0['side'] < -60 <= m1['side'] or m1['side'] < -60 <= m0['side']:
        out.append('goes from mono to stereo' if m1['side'] > m0['side'] else 'goes from stereo to mono')
    elif abs(d[5]) >= 3:
        out.append(f"{'wider' if d[5] > 0 else 'narrower'} ({d[5]:+.1f} dB side)")
    out += shifts_text
    if abs(m1['onsets'] - m0['onsets']) >= 1.5:
        out.append(f"{'busier' if m1['onsets'] > m0['onsets'] else 'sparser'} ({m0['onsets']:.1f} to {m1['onsets']:.1f} onsets/s)")
    if abs(m1['centroid'] - m0['centroid']) >= 300:
        out.append(f"centroid {m0['centroid']:.0f} to {m1['centroid']:.0f} Hz")
    return '; '.join(out)


def read(path):
    x, sr = sf.read(path, dtype='float32')
    assert sr == SR, path
    return x.astype(np.float64)


def region(r):
    """Human text for one route."""
    if r['type'] == 'param':
        f = lambda v: f"{v:.4g}"
        return (f"{r['kind']}#{r['module']}.{r['param']} {f(r['at'][0])} / {f(r['at'][1])} / {f(r['at'][2])} {r['unit']}"
                f" (range {f(r['min'])}-{f(r['max'])}, {r['taper'].lower()}; base {f(r['base'])}, macro x {r['amount']:.2f} of travel)")
    return (f"depth of {r['from']} to {r['to']}.{'' if not r['param'] else r['param']}: "
            f"{r['at'][0]:.2f} / {r['at'][1]:.2f} / {r['at'][2]:.2f} (macro adds up to {r['depth']:.2f})")


def audit_macro(slug, entry, renders):
    k = entry['macro']
    steps = [read(renders / 'steps' / f'{slug}-m{k}-s{i}.wav') for i in range(STEPS)]
    sweep = read(renders / 'sweep' / f'{slug}-m{k}.wav')
    ms = [measures(s) for s in steps]
    mid = {
        'peak': max(ms[3]['peak'], ms[4]['peak']), 'loud': (ms[3]['loud'] + ms[4]['loud']) / 2,
        'bands': [(a + b) / 2 for a, b in zip(ms[3]['bands'], ms[4]['bands'])],
        'centroid': (ms[3]['centroid'] + ms[4]['centroid']) / 2, 'side': (ms[3]['side'] + ms[4]['side']) / 2,
        'onsets': (ms[3]['onsets'] + ms[4]['onsets']) / 2,
    }
    vec = vector(ms[0], ms[-1])
    tobs = [tob(x) for x in steps]
    pair = [rel(steps[i], steps[i + 1], tobs[i], tobs[i + 1]) for i in range(STEPS - 1)]
    ends = rel(steps[0], steps[-1], tobs[0], tobs[-1])
    half = (rel(steps[0], steps[3], tobs[0], tobs[3]) + rel(steps[0], steps[4], tobs[0], tobs[4])) / 2  # about 0.5
    wave = db(np.sqrt(np.mean((steps[0] - steps[-1]) ** 2)) / max(np.sqrt(np.mean(steps[0] ** 2)), 1e-9))
    psize = list(pair)
    med = float(np.median(psize))
    flags = []
    wpair = [db(np.sqrt(np.mean((steps[i] - steps[i + 1]) ** 2)) / max(np.sqrt(np.mean(steps[i] ** 2)), 1e-9))
             for i in range(STEPS - 1)]
    # the waveform counts too: a pitch glide or a timing change moves it without moving the spectrum
    moves = ends >= WEAK_DB or wave > WAVE_DB
    zone = [i for i in range(STEPS - 1) if pair[i] < DEAD_DB and wpair[i] < WAVE_DEAD_DB]
    if (len(zone) >= 2 and moves) or (ends < DEAD_DB and wave < WAVE_DB):
        flags.append('dead')
    jumps = [i for i in range(STEPS - 1) if psize[i] > JUMP_DB and psize[i] > JUMP_FACTOR * max(med, 0.3)]
    if jumps:
        flags.append('jump')
    if not moves and 'dead' not in flags:
        flags.append('weak')
    problems = []
    for tag, m in (('0', ms[0]), ('1', ms[-1])):
        if m['peak'] > PEAK_DB:
            problems.append(f"peak {m['peak']:.1f} dBFS at {tag}")
        if m['loud'] < SILENT_DB:
            problems.append(f"silent at {tag} ({m['loud']:.1f} dB)")
        if m['loud'] < mid['loud'] - 12:
            problems.append(f"{mid['loud'] - m['loud']:.0f} dB under the middle at {tag}")
        if m['loud'] > mid['loud'] + 9:
            problems.append(f"{m['loud'] - mid['loud']:.0f} dB over the middle at {tag}")
    base_clicks = set(round(t, 1) for s in (steps[0], steps[-1]) for t in ad.clicks(s, SR))
    end_clicks = sorted(base_clicks)
    sweep_clicks = [t for t in ad.clicks(sweep, SR) if round(t, 1) not in base_clicks]
    if end_clicks:
        problems.append(f"CLICK in the end renders at {', '.join(f'{t:.1f}' for t in end_clicks)} s")
    if problems:
        flags.append('bad end')
    if sweep_clicks:
        flags.append('zipper')
    return {
        'macro': k, 'steps': ms, 'mid': mid, 'vec': vec.tolist(), 'pair_db': pair, 'pair_size': psize,
        'ends_db': ends, 'half_db': half, 'wave_db': wave, 'zone': zone, 'jumps': jumps, 'flags': flags, 'problems': problems,
        'sweep_clicks': sweep_clicks, 'words': words(ms[0], mid, ms[-1], vec, shifts(tobs[0], tobs[-1])) or (
            f"no change in level, spectrum or density; the waveform differs by {wave:.1f} dB (pitch path, timing or phase)"
            if wave > -15 else 'no change you can name from the measures'),
        'sweep_peak': db(np.abs(sweep).max()),
    }


def encode(x, path, kbps):
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as d:
        wav = Path(d) / 'a.wav'
        sf.write(wav, x.astype(np.float32), SR, subtype='FLOAT')
        sp.run(['ffmpeg', '-y', '-loglevel', 'error', '-i', str(wav), '-b:a', f'{kbps}k', str(path)], check=True)


def faded(x):
    n = int(LEG_FADE * SR)
    x = x.copy()
    x[:n] *= np.linspace(0, 1, n)[:, None]
    x[-n:] *= np.linspace(1, 0, n)[:, None]
    return x


def tour(clips):
    """clips: [(label, array)] back to back with GAP s of silence; returns (audio, [(label, start s)])."""
    out, starts, at = [], [], 0.0
    gap = np.zeros((int(GAP * SR), 2))
    for label, x in clips:
        starts.append((label, at))
        out += [faded(x), gap]
        at += len(x) / SR + GAP
    return np.concatenate(out), starts


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--category', required=True)
    ap.add_argument('--renders', required=True)
    ap.add_argument('--patches', default='patches')
    ap.add_argument('--clips')
    ap.add_argument('--out', required=True)
    ap.add_argument('--verdicts', help='JSON {"slug/mK": [verdict, one line why]}, written by hand after reading the numbers')
    ap.add_argument('--intro', help='markdown for the top of the report (method, findings)')
    ap.add_argument('--doc', help='write the full report here')
    ap.add_argument('slugs', nargs='+')
    a = ap.parse_args()
    renders, out = Path(a.renders), Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    result, md = {}, []
    all_clips, starts_all = [], {}
    for slug in a.slugs:
        info = json.loads((renders / f'{slug}.json').read_text())
        ck = json.loads((Path(a.patches) / 'presets' / slug / 'checkpoint.json').read_text())
        kid = next(i for i, m in ck['modules'].items() if m['kind'] == 'macro')
        names = ck['labels'].get(kid, {})
        res = []
        for entry in info['macros']:
            k = entry['macro']
            r = audit_macro(slug, entry, renders)
            r['name'] = names.get(f'name.m{k}', f'm{k}')
            r['routes'] = [region(x) for x in entry['routes']]
            r['targets'] = sorted({f"{x['kind']}#{x['module']}.{x['param']}" for x in entry['routes'] if x['type'] == 'param'})
            r['start'] = entry['start']
            res.append(r)
        for i, r in enumerate(res):
            for j, q in enumerate(res):
                if j <= i:
                    continue
                u, v = np.array(r['vec']), np.array(q['vec'])
                cos = float(u @ v / max(np.linalg.norm(u) * np.linalg.norm(v), 1e-9))
                shared = sorted(set(r['targets']) & set(q['targets']))
                if (cos > DUP_COS and np.abs(u).max() > 1.5 and np.abs(v).max() > 1.5) or shared:
                    for z, o in ((r, q), (q, r)):
                        z['flags'].append('duplicate')
                        z.setdefault('overlap', []).append(
                            f"{o['name']} (m{o['macro']}): " + (f"both drive {', '.join(shared)}" if shared else f"same change, cosine {cos:.2f}"))
        result[slug] = res
        if a.clips:
            cdir = Path(a.clips)
            clips = []
            for r in res:
                x = read(renders / 'sweep' / f"{slug}-m{r['macro']}.wav")
                label = f"{r['name']}"
                encode(faded(x), cdir / f"{slug}-m{r['macro']}-{label.lower().replace(' ', '-')}.mp3", 128)
                clips.append((f"{slug} {label}", x))
            audio, starts = tour(clips)
            encode(audio, cdir / f'{slug}-tour.mp3', 192)
            starts_all[slug] = starts
            all_clips += clips
    if a.clips:
        audio, starts = tour(all_clips)
        encode(audio, Path(a.clips) / 'tour.mp3', 192)
        result['_tour'] = [(l, round(t, 1)) for l, t in starts]
        result['_preset_tours'] = {s: [(l, round(t, 1)) for l, t in v] for s, v in starts_all.items()}
    (out / 'audit.json').write_text(json.dumps(result, indent=1, default=float))
    verdicts = json.loads(Path(a.verdicts).read_text()) if a.verdicts else {}
    for slug in a.slugs:
        md.append(f'### {slug}\n')
        md.append('| macro | routes: value at macro 0 / 0.5 / 1 | effect (change vs macro 0: at 0.5 / at 1; waveform 0 vs 1) | measured flags | verdict |\n|---|---|---|---|---|')
        for r in result[slug]:
            v = verdicts.get(f"{slug}/m{r['macro']}", ['', ''])
            flags = ', '.join(dict.fromkeys(r['flags'])) or '-'
            more = ''.join(f"; {x}" for x in r['problems'])
            more += ''.join(f"; click while moving at {t:.1f} s" for t in r['sweep_clicks'])
            more += ''.join(f"; overlaps {x}" for x in r.get('overlap', []))
            md.append(f"| m{r['macro']} {r['name']} | {'<br>'.join(r['routes'])} | {r['words']}. {r['half_db']:.2f} / {r['ends_db']:.2f} dB; waveform {r['wave_db']:.1f} dB | {flags}{more} | **{v[0]}**: {v[1]} |")
        md.append('')
    (out / 'tables.md').write_text('\n'.join(md))
    if a.doc:
        flagged = [(s, r) for s in a.slugs for r in result[s] if verdicts.get(f"{s}/m{r['macro']}", ['ok'])[0] != 'ok']
        total = sum(len(result[s]) for s in a.slugs)
        text = [f'# Macro audit: {a.category}\n']
        text.append(Path(a.intro).read_text() if a.intro else '')
        text.append(f'\n## Flagged macros ({len(flagged)} of {total})\n')
        for s, r in flagged:
            v = verdicts[f"{s}/m{r['macro']}"]
            text.append(f"- {s} m{r['macro']} {r['name']}: **{v[0]}**, {v[1]}")
        text.append('\n## Tables\n')
        text += md
        if a.clips:
            text.append('## Clips\n')
            text.append(f'Every clip: the groove runs, the macro moves 0 to 1 to 0 over 8 s (1 s to 9 s of a 10 s render), the other macros at their stored values. Start times are in seconds from the start of the file.\n')
            text.append(f"- `{a.clips}/tour.mp3`: all {total} sweeps, 1 s apart, 20 ms fades:")
            text.append('  ' + '; '.join(f"{t} {l}" for l, t in result['_tour']))
            for s in a.slugs:
                text.append(f"- `{a.clips}/{s}-tour.mp3` (this preset's macros): " + '; '.join(f"{t} {l.split(' ', 1)[1]}" for l, t in result['_preset_tours'][s]))
            text.append(f"- one file per macro: `{a.clips}/<slug>-m<k>-<name>.mp3`")
        Path(a.doc).write_text('\n'.join(text) + '\n')
    print('wrote', out)


if __name__ == '__main__':
    main()
