# Audio defect scan: handover (branch `claude/audio-defect-scan`)

## Task (condensed)
Build one offline script that flags impulsive discontinuities, limiter pinning/clipping, DC, NaN/denormal and sudden HF bursts in preset clips, with time stamps. Tune it so Deep Field (#52) is flagged and merged group 1 clips are not. Run it over master, #51 and #52 clips, wire it into the preset checks and CI. Do not change any preset sound.

## Done
- `docs/presets/scripts/audio_defects.py`, `audio_defects.allow`, `test_audio_defects.py` (8 synthetic tests).
- Called by `clips.py` and `clips_keys_pluck.py` after rendering; CI job `audio-defects` in `.github/workflows/ci.yml`.
- Method, thresholds and the flag table for 68 clips: `docs/presets/AUDIO-DEFECTS.md`.

## Remaining
1. Overseer: review the table; Deep Field (#52) and Snowmelt (#51) will fail the CI job on their branches until fixed or allowed in `audio_defects.allow`.
2. Rubber Band clicks (allowed with a note) need a fix in the bass-lead generator.
3. Not run through CI yet (needs a push to a PR).

## Facts
- The 2nd-difference crest over 50 ms fires on every clean bass attack (12 to 49); isolation (top vs the rest of the window) separates single-sample spikes.
- Deep Field hard-clips: the waveform sits flat at -0.8414 (the -1.5 dB ceiling) for several samples.
- CI installs numpy, scipy, soundfile with pip; soundfile wheels carry libsndfile.

## Assumptions
- Thresholds were tuned on 68 clips and were not heard by ear; HF_BURST and DC are advisory on purpose.

Standing rules: report to "Kabl overseer", do not merge, stop when done.
