# Preset audio defect scan

`docs/presets/scripts/audio_defects.py` reads rendered clips (FLAC or WAV, files or directories) and flags defects with time stamps. It needs no audio device and no engine: `python3 docs/presets/scripts/audio_defects.py --allow docs/presets/scripts/audio_defects.allow docs/presets/clips`. Packages: numpy, scipy, soundfile. Self-test: `python3 docs/presets/scripts/test_audio_defects.py` (8 synthetic cases).

Why: the earlier click scan (5 ms windows, share above 3 kHz over 0.7) reported 0 for Deep Field, which has single-sample spikes and a limiter that pins at its ceiling. Only the 2nd-difference crest in a 50 ms window found them, but that crest also fires on every bass attack (values 12 to 49 on clean basses), so the scan uses isolation instead.

## Flags (ERROR fails the script, advisory only prints)

| Flag | Level | Rule |
| --- | --- | --- |
| NONFINITE | ERROR | NaN or infinity in the samples. |
| DENORMAL | ERROR | Subnormal float samples (a float WAV only; FLAC cannot hold them). |
| CLICK | ERROR | In a 50 ms window the largest 2nd difference \|x[n]-2x[n-1]+x[n-2]\| is at least 5 times the largest one more than 3 samples away, is at least -45 dBFS, and the rest is not digital silence. Smooth attacks, repeating saw edges and onsets from silence do not qualify. |
| PINNED | ERROR | Peak above -3 dBFS and at least 30 samples per active second within 0.04 dB of the peak: the output sits on its ceiling. |
| DC | advisory | Mean of the active signal above 0.01 (-40 dBFS). |
| HF_BURST | advisory | 5 ms with 30 dB more energy above 6 kHz than the local median (+-100 ms) while the full-band energy rises under 3 dB. Drum layers trigger it. |

Thresholds are constants at the top of the script. `audio_defects.allow` lists accepted findings (path suffix and flag) with a reason. It is also run by `clips.py` and `clips_keys_pluck.py` after rendering, and by CI (job `audio-defects`, offline) on `docs/presets/clips`.

## Result over master, #51 (keys group 2) and #52 (pads group 1): 68 clips, 19 with a flag

Deep Field is flagged CLICK x13 (7.263, 8.067, 11.208, 12.342, 13.304, 15.255, ...) and PINNED x95 starting 5.663 s, which covers the user-heard times 7.25, 7.90, 9.55, 11.20 and 13.30. At 13.304 s the waveform walks into -0.8414 and stays flat there for several samples: hard clipping at the limiter ceiling, not a smooth gain reduction.

| Clip | Flags | Looks real? |
| --- | --- | --- |
| pad/deep-field (#52) | CLICK x13, PINNED x95 | Yes. Flat tops at the ceiling and spikes entering them. |
| bass-lead/rubber-band | CLICK 0.246, 3.148 | Yes. At 3.148 s the signal moves 0.18 to -0.29 in 4 samples (retrigger edge). |
| keys-pluck/snowmelt (#51) | CLICK x9 (0.383, 0.425, 5.287, 5.388, 5.645, 5.747, ...), HF_BURST 10.645 | Probably. At 5.287 s the slope flips from +15 to -6 per sample at 0.72 FS, a kink of the size a limiter gain step makes. Level is -1.5 dBFS but PINNED is not raised. Needs listening. |
| bass-lead/rail-yard | CLICK 8.0 | Probably the kick onset on the drum grid (-69 to +28 in one sample). Allowed. |
| pig-iron, velvet-tine, tin-roof (#51), pad/dry-cleaner | DC | Real offsets of 0.01 to 0.015 (-40 to -36 dBFS); inaudible alone, costs headroom and can click when a note starts. |
| acid-rain, steel-toe, tar-pit, copper-wire (bass), gutter, rail-yard, slate-roof, cinder-block, undertow, dry-creek (#51), parquet-loop glide demos | HF_BURST | Mostly the layered kick or click of the preset (drum grid and attack layers). Not judged by ear. |

The other 49 clips are clean. No preset sound was changed.
