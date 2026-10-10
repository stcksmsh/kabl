# Factory wavetable provenance

All files are 16-bit mono, 2048-sample frames, every frame peak-normalised, produced by `build.py` in this
directory (`python3 -I build.py CACHE_DIR`). Nothing here comes from Surge XT or Vital.

| File | Name in kabl | Frames | Source | Licence |
|---|---|---|---|---|
| `saw-square.wav` | Saw to Square | 16 | computed by `build.py`: even harmonics faded out of a saw | MIT OR Apache-2.0 (original) |
| `harmonic-sweep.wav` | Harmonic Sweep | 32 | computed: sine growing to a 32-harmonic saw, one harmonic per frame | MIT OR Apache-2.0 (original) |
| `vowels.wav` | Vowels | 16 | computed: a/e/i/o/u by formant envelopes (Peterson & Barney 1952 average male formant frequencies, facts not expression) on a 110 Hz source | MIT OR Apache-2.0 (original) |
| `glass-bell.wav` | Glass Bell | 16 | computed: 1:3 phase modulation, index 0 to 7 | MIT OR Apache-2.0 (original) |
| `digital-hollow.wav` | Digital Hollow | 16 | computed: saw with a moving comb on its harmonics | MIT OR Apache-2.0 (original) |
| `epiano.wav` | Electric Piano | 16 | Adventure Kid Waveforms, folder `AKWF/AKWF_epiano`, cycles 1, 6, 11, 15, 20, 25, 30, 35, 39, 44, 49, 54, 59, 63, 68, 73 | CC0 1.0 |
| `organ.wav` | Organ | 16 | Adventure Kid Waveforms, folder `AKWF/AKWF_eorgan`, cycles 1, 11, 21, 32, 42, 52, 62, 72, 83, 93, 103, 113, 123, 134, 144, 154 | CC0 1.0 |
| `choir.wav` | Choir | 16 | Adventure Kid Waveforms, folder `AKWF/AKWF_hvoice`, cycles 1, 8, 15, 22, 28, 35, 42, 49, 56, 63, 70, 77, 83, 90, 97, 104 | CC0 1.0 |

## Adventure Kid Waveforms

Author: Kristoffer Ekstrand (Adventure Kid). Mirror used: `KristofferKarlAxelEkstrand/AKWF-FREE`
at commit `8de90bf94376670947369e69de0af6b9fbd19286` (2025-12-04).

Licence checked on 2026-10-09 in three places: the repository's `LICENSE.md` is the CC0 1.0
Universal text; GitHub reports the repository licence as `CC0-1.0`; and the author's page
(adventurekid.se, "AKWF FREE") states that "Kristoffer Ekstrand has waived all copyright and
related or neighboring rights to AKWF Waveforms." Only the original `AKWF/` folder (his own 600
sample cycles) is used. The mirror's other collections (Surge, Zebra, Alchemy, ...) are derived
from other products and were not checked, so none is used.

Processing: each cycle is band-limited to 256 harmonics, DC removed, rotated so its fundamental
has zero phase (frames line up when morphing), resampled to 2048 samples, and every frame
is scaled to a peak of 0.98 so sweeping a table does not change the level. Cycles were picked at even spacing through each folder, not by
ear; replace the picks in `build.py` if a better morph is wanted.
