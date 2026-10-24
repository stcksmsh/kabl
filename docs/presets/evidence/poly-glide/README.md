# Poly glide in REAPER (CLAP plugin)

MIDI from a REAPER item reaches the keyboard the same way as standalone MIDI and the on-screen keys: the host's note events go through `Timeline` and `PatchEngine::key_at` to `send_key`, which stamps the engine time and calls `Keyboard::midi` (so `glide_from` is set). The standalone's hardware input uses the same `Timeline` (`crates/ui/src/main.rs`).

Renders by `docs/presets/scripts/host.py` (REAPER 7.75, silent sink `kabl_agents`), Parquet Loop with the Glide macro at 1 (about 830 ms), POLY, glide ALWAYS. Each project saves, quits, reopens with an identical state and renders the same after the reopen. Renders: `line.flac`, `chord.flac`.

## Legato line (`line.flac`)

C3 and C5 alternating, overlapping, note-ons at 1.6 s (C5) and 2.7 s (C3). Pitch by autocorrelation on 40 ms windows, Hz (130.8 = C3, 523.3 = C5):

| after note-on | -0.05 | +0.1 | +0.2 | +0.3 | +0.45 | +0.6 | +0.8 | +1.0 |
|---|---|---|---|---|---|---|---|---|
| 1.6 s, C3 to C5 | 130.8 | 157.4 | 190.5 | 222.2 | 287.4 | 366.4 | 516.1 | 521.7 |
| 2.7 s, C5 to C3 | 521.7 | 424.8 | 360.9 | 311.7 | 241.2 | 186.0 | 135.6 | 130.4 |

With the default Glide (0.5) the same line slides in about 50 ms. Before the fix the first notes did not slide at all (`docs/presets/PILOT.md`).

## Chord change (`chord.flac`)

C3 E3 G3 held from 0.5 s; D3 F3 A3 played at 3.0 s (tones 10 ms apart, one chord). Strongest spectral peak (150 ms window) near each tone, Hz, by time after the change:

| tone | -0.1 | +0.05 | +0.15 | +0.3 | +0.5 | +0.7 | +1.0 | +1.4 |
|---|---|---|---|---|---|---|---|---|
| root 130.8 to 146.8 | 127.4 | 132.2 | 136.2 | 137.3 | 141.0 | 143.9 | 146.1 | 146.9 |
| third 164.8 to 174.6 | 164.1 | 166.6 | 167.4 | 168.1 | 171.4 | 171.4 | 174.0 | 174.3 |
| fifth 196.0 to 220.0 | 195.6 | 200.7 | 200.7 | 203.6 | 211.3 | 216.4 | 220.1 | 219.4 |

Each tone slides from the nearest tone of the previous chord (root from C3, third from E3, fifth from G3) and lands on its target after the glide time.
