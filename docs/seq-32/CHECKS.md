# seq-32 objective checks

Levels (peak / loudness dBFS, before = length clamped to 8 as master played; after = 16 steps), `presets_groove levels`:

| groove | peak before -> after | loud before -> after |
|---|---|---|
| brick-dust | -3.88 -> -3.85 | -20.34 -> -20.31 |
| copper-mile | -6.18 -> -6.21 | -21.62 -> -21.58 |
| corner-store | -2.86 -> -2.77 | -18.62 -> -18.67 |
| dune-buggy | -2.80 -> -2.85 | -18.02 -> -18.80 |
| fault-line | -3.00 -> -1.60 | -19.04 -> -18.88 |
| glass-coast | -3.51 -> -3.56 | -18.52 -> -18.56 |
| lamp-oil | -4.47 -> -3.34 | -19.87 -> -19.43 |
| ore-cart | -5.53 -> -5.30 | -20.72 -> -20.71 |
| rope-walk | -6.30 -> -6.30 | -21.86 -> -22.01 |
| scrap-metal | -5.19 -> -5.21 | -21.84 -> -21.86 |
| slab-city | -3.72 -> -3.61 | -21.34 -> -23.60 (re-leveled, see below) |
| still-water | -8.72 -> -10.18 | -23.34 -> -23.01 |
| stoop-sale | -4.03 -> -2.65 | -20.20 -> -19.76 |

Slab City got quieter by 2.26 dB (the added notes lower the active-block average), so `level_trim("slab-city") = 2.3` was added to `preset_trims.inc` and the patch regenerated: now peak -1.86, loudness -21.98 (0.64 dB from before). Peaks of fault-line (+1.4), lamp-oil (+1.1), stoop-sale (+1.4) and still-water (-1.5) moved more than 1 dB but loudness moved under 0.5 dB and all peaks stay at or under -1.6 dBFS, so they were not re-leveled.

Macro travel (`presets_groove macros patches`, all 0 / all 1 / each macro alone): every peak is at or under -1.0 dBFS (highest: corner-store all-1 -1.0, scrap-metal all-0 -1.1).

audio_defects on the 14 groove clips: only advisory HF_BURST notes, exit 0, no new allow entries.

REAPER 7.75 (`host.py`, `PH_SELF=1`, `kabl_agents` sink): the 13 changed grooves, loaded from their old (master, 8-step-era) saved state, were saved, REAPER fully quit and reopened: plugin state identical in all 13, loudness after reopen within 1.5 dB (the assert), and slab-city also re-checked with its regenerated state. Old states need no migration: the patch state is name keyed and steps 9-32 default off.
