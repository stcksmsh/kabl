# Limiter removal (branch claude/audio-defect-scan)

Kosta's decision: no limiter in presets; headroom comes from gain staging.

- Removed the final `limiter` from the pilot, bass-lead, pad and keys-pluck generators (all 62 presets), and Deep Field / Fog Bank's `Voice::guard` compressor (it only protected the limiter).
- Levels: `crates/ui/examples/preset_trims.inc` holds a per-preset dB trim added to the preset's gain stage at `write` time. Peak rule: at most -1 dBFS on the clip phrases, the chord level test, each macro alone at 1 and all macros at 1.
- `factory_levels_hold_the_target` checks the macro sweeps for `presets/*`. `audio_defects.py` has a PEAK error flag (> -1 dBFS) and the CLICK rule ignores windows under 16 LSB.
- Deep Field and Salt Flat: macros at 1 add 6 dB, so the peak rule puts their loudness about 1.7 dB under the -24.5 dBFS window edge (-26.1, -26.2). Listed in `LOUD_EXCEPTIONS`. A fix needs a narrower macro range, not patch gain; Kosta decides.
- Deep Field with the old 10 s room (limiter gone): float clip clean in the scan, largest second difference near 7.95 s is 0.0017 (median 0.0003, no kink).

## Levels (dBFS), before and after

| preset | loud old/new | chord peak old/new | macro peak old/new | gain dB |
|---|---|---|---|---|
| acid-rain | -19.9 / -19.9 | -12.2 / -12.2 | -10.2 / -10.2 | +0.0 |
| afterglow | -21.6 / -21.6 | -9.0 / -9.0 | -7.2 / -7.2 | +0.0 |
| boxwood | -20.3 / -21.1 | -2.2 / -3.2 | -1.5 / -1.1 | -1.0 |
| brass-filament | -17.4 / -19.3 | -1.5 / -2.9 | -1.5 / -1.1 | -2.0 |
| cellar-door | -17.6 / -20.5 | -1.5 / -3.3 | -1.5 / -1.6 | -3.0 |
| checkerboard | -20.0 / -20.1 | -2.2 / -2.6 | -1.6 / -2.0 | -0.5 |
| cinder-block | -19.2 / -19.2 | -7.2 / -7.2 | -7.2 / -7.2 | +0.0 |
| cold-cut | -20.6 / -21.4 | -1.5 / -1.8 | -1.5 / -1.2 | -3.0 |
| concrete-cloud | -20.6 / -20.6 | -8.3 / -8.3 | -7.5 / -7.5 | +0.0 |
| copper-wire | -20.8 / -20.8 | -13.9 / -13.9 | -12.2 / -12.2 | +0.0 |
| corduroy | -19.6 / -19.8 | -2.8 / -3.0 | -1.5 / -1.1 | -0.2 |
| cotton-mill | -19.2 / -19.2 | -4.0 / -4.0 | -3.5 / -3.5 | +0.0 |
| deep-field | -22.1 / -26.1 | -7.3 / -8.3 | -4.2 / -1.2 | -7.2 |
| drive-in | -19.9 / -19.9 | -4.6 / -4.6 | -2.6 / -2.6 | +0.0 |
| dry-cleaner | -20.7 / -20.7 | -3.9 / -3.9 | -3.2 / -3.2 | +0.0 |
| dry-creek | -18.1 / -20.5 | -1.5 / -3.0 | -1.5 / -1.4 | -3.0 |
| dust-devil | -21.6 / -21.6 | -13.9 / -13.9 | -13.2 / -13.2 | +0.0 |
| fire-escape | -21.6 / -21.6 | -10.4 / -10.4 | -9.7 / -9.7 | +0.0 |
| fog-bank | -21.0 / -21.6 | -7.1 / -5.1 | -5.3 / -1.2 | -3.2 |
| glass-elevator | -21.0 / -22.9 | -1.8 / -3.8 | -1.5 / -1.1 | -2.0 |
| glass-needle | -20.2 / -20.2 | -9.6 / -9.6 | -7.4 / -7.4 | +0.0 |
| gumdrop | -16.1 / -18.7 | -1.5 / -3.0 | -1.5 / -1.3 | -3.0 |
| gutter | -16.6 / -16.6 | -7.5 / -7.5 | -7.5 / -7.5 | +0.0 |
| hand-bell | -18.1 / -21.8 | -1.5 / -3.5 | -1.5 / -1.2 | -4.0 |
| horn-section | -21.1 / -21.1 | -11.9 / -11.9 | -10.7 / -10.7 | +0.0 |
| hot-wire | -21.3 / -21.3 | -9.3 / -9.3 | -9.2 / -9.2 | +0.0 |
| lantern | -22.9 / -22.9 | -9.8 / -9.8 | -6.9 / -6.9 | +0.0 |
| linoleum | -22.1 / -22.4 | -1.5 / -1.1 | -1.5 / -1.1 | -1.6 |
| loading-dock | -18.1 / -18.1 | -7.2 / -7.2 | -7.0 / -7.0 | +0.0 |
| loud-mouth | -22.6 / -22.6 | -13.8 / -13.8 | -13.8 / -13.8 | +0.0 |
| low-tide | -22.1 / -22.1 | -14.0 / -14.0 | -11.0 / -11.0 | +0.0 |
| matinee | -17.1 / -20.0 | -1.5 / -3.6 | -1.5 / -1.1 | -3.0 |
| neon-highway | -19.5 / -19.5 | -8.0 / -8.0 | -5.5 / -5.5 | +0.0 |
| oak-stave | -22.3 / -22.3 | -5.3 / -5.3 | -4.7 / -4.7 | +0.0 |
| oatmeal | -19.9 / -19.9 | -11.4 / -11.4 | -11.5 / -11.5 | +0.0 |
| parquet-loop | -20.0 / -20.0 | -3.9 / -3.9 | -2.2 / -2.2 | +0.0 |
| picture-window | -19.9 / -21.7 | -3.6 / -5.6 | -1.5 / -1.7 | -2.0 |
| pig-iron | -20.5 / -20.5 | -10.3 / -10.3 | -10.4 / -10.4 | +0.0 |
| pulpit | -17.5 / -17.5 | -4.8 / -4.8 | -3.4 / -3.4 | +0.0 |
| rail-yard | -21.4 / -21.4 | -8.5 / -8.5 | -8.5 / -8.5 | +0.0 |
| rain-gutter | -18.9 / -21.5 | -1.5 / -3.6 | -1.5 / -1.2 | -3.3 |
| rivet-gun | -19.8 / -20.0 | -1.5 / -1.1 | -1.5 / -1.1 | -0.9 |
| rosewood | -18.0 / -20.0 | -1.5 / -3.4 | -1.5 / -1.2 | -2.2 |
| rubber-band | -21.3 / -21.3 | -9.6 / -9.6 | -9.7 / -9.7 | +0.0 |
| salt-flat | -19.9 / -26.2 | -1.5 / -7.4 | -1.5 / -1.2 | -6.3 |
| scaffold | -21.7 / -21.7 | -4.8 / -4.8 | -4.3 / -4.3 | +0.0 |
| short-circuit | -21.6 / -21.6 | -12.3 / -12.3 | -11.1 / -11.1 | +0.0 |
| slate-roof | -19.7 / -19.7 | -11.4 / -11.4 | -11.2 / -11.2 | +0.0 |
| snowmelt | -20.4 / -21.0 | -3.2 / -3.8 | -1.5 / -1.2 | -0.6 |
| spark-plug | -20.4 / -20.5 | -1.5 / -1.2 | -1.5 / -1.2 | -0.6 |
| spinet | -19.7 / -22.4 | -3.1 / -6.1 | -1.5 / -1.5 | -3.0 |
| spun-sugar | -20.9 / -20.9 | -10.7 / -10.7 | -7.8 / -7.8 | +0.0 |
| static-garden | -20.1 / -20.1 | -4.2 / -4.2 | -1.5 / -1.4 | +0.0 |
| steel-toe | -16.7 / -16.7 | -8.0 / -8.0 | -7.7 / -7.7 | +0.0 |
| tail-light | -21.2 / -21.2 | -10.3 / -10.3 | -9.2 / -9.2 | +0.0 |
| tar-pit | -20.7 / -20.7 | -13.6 / -13.6 | -13.4 / -13.4 | +0.0 |
| tin-roof | -18.8 / -18.8 | -3.4 / -3.4 | -2.2 / -2.2 | +0.0 |
| tuning-fork | -18.7 / -23.0 | -1.5 / -4.8 | -1.5 / -1.2 | -5.0 |
| turnstile | -22.0 / -23.5 | -3.1 / -4.6 | -3.1 / -4.6 | -1.6 |
| undertow | -20.3 / -20.3 | -13.1 / -13.1 | -11.2 / -11.2 | +0.0 |
| velvet-tine | -20.4 / -22.9 | -1.5 / -2.8 | -1.5 / -1.4 | -2.7 |
| willow | -16.4 / -21.3 | -1.5 / -5.9 | -1.5 / -1.5 | -5.0 |
